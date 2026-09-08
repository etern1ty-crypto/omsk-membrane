use crate::ScanError;

const MAX_TABLE_ENTRIES: usize = 4096;
const ELF_HEADER_SIZE: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputFormat {
    /// Only a supported ELF is accepted. Unknown data is an error.
    Auto,
    Elf,
    /// Explicitly scan every supplied byte; no file-format validation.
    Raw,
}

impl InputFormat {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "auto" => Ok(Self::Auto),
            "elf" => Ok(Self::Elf),
            "raw" => Ok(Self::Raw),
            _ => Err(format!(
                "invalid input format {value:?}; expected auto, elf or raw"
            )),
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Elf => "elf",
            Self::Raw => "raw",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageKind {
    Raw,
    ElfExecutable,
    ElfShared,
    ElfObject,
}

impl ImageKind {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Raw => "raw",
            Self::ElfExecutable => "elf64-executable",
            Self::ElfShared => "elf64-shared",
            Self::ElfObject => "elf64-object",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Region {
    pub name: String,
    pub file_offset: usize,
    pub length: usize,
    /// Relocatable object sections and raw input have no load address here.
    pub virtual_address: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    pub kind: ImageKind,
    pub regions: Vec<Region>,
}

fn invalid(message: impl Into<String>) -> ScanError {
    ScanError::InvalidInput(message.into())
}

fn range(data: &[u8], offset: usize, length: usize) -> Result<&[u8], ScanError> {
    offset
        .checked_add(length)
        .and_then(|end| data.get(offset..end))
        .ok_or_else(|| invalid("ELF range overflows or extends past the end of the file"))
}

fn u16_at(data: &[u8], offset: usize) -> Result<u16, ScanError> {
    let bytes = range(data, offset, 2)?;
    Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
}

fn u32_at(data: &[u8], offset: usize) -> Result<u32, ScanError> {
    let bytes = range(data, offset, 4)?;
    Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn u64_at(data: &[u8], offset: usize) -> Result<u64, ScanError> {
    let bytes = range(data, offset, 8)?;
    Ok(u64::from_le_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
    ]))
}

fn usize_at(data: &[u8], offset: usize) -> Result<usize, ScanError> {
    usize::try_from(u64_at(data, offset)?)
        .map_err(|_| invalid("ELF offset is not representable on this host"))
}

fn check_table(
    data: &[u8],
    offset: usize,
    count: usize,
    entry_size: usize,
    expected_size: usize,
) -> Result<(), ScanError> {
    if count == 0 || count > MAX_TABLE_ENTRIES {
        return Err(invalid(
            "ELF table must have 1..4096 entries; extended numbering is unsupported",
        ));
    }
    if entry_size != expected_size || offset < ELF_HEADER_SIZE {
        return Err(invalid("ELF table has an invalid entry size or offset"));
    }
    let length = count
        .checked_mul(entry_size)
        .ok_or_else(|| invalid("ELF table size overflows"))?;
    range(data, offset, length)?;
    Ok(())
}

/// Parse a deliberately limited, validated ELF64 little-endian AMD64 subset.
///
/// Executables and shared objects use executable PT_LOAD segments, not section
/// names, so stripping the section table does not bypass scanning. ET_REL uses
/// file-backed SHF_EXECINSTR sections. Unsupported layouts fail closed.
pub fn parse_image(data: &[u8], format: InputFormat) -> Result<Image, ScanError> {
    if data.is_empty() {
        return Err(invalid("empty input is not an artifact"));
    }
    if format == InputFormat::Raw {
        return Ok(Image {
            kind: ImageKind::Raw,
            regions: vec![Region {
                name: "raw".into(),
                file_offset: 0,
                length: data.len(),
                virtual_address: None,
            }],
        });
    }
    if !data.starts_with(b"\x7fELF") {
        return Err(invalid("unsupported input; expected ELF64 AMD64 (use --input-format raw explicitly for code bytes)"));
    }
    range(data, 0, ELF_HEADER_SIZE)?;
    if data[4] != 2 || data[5] != 1 || data[6] != 1 {
        return Err(invalid(
            "only ELF64, little-endian, ELF version 1 is supported",
        ));
    }
    if u16_at(data, 18)? != 62 || u32_at(data, 20)? != 1 {
        return Err(invalid("only AMD64 ELF machine 62, version 1 is supported"));
    }
    if u16_at(data, 52)? as usize != ELF_HEADER_SIZE {
        return Err(invalid("invalid ELF64 header size"));
    }
    let kind = match u16_at(data, 16)? {
        1 => ImageKind::ElfObject,
        2 => ImageKind::ElfExecutable,
        3 => ImageKind::ElfShared,
        _ => return Err(invalid("only ELF ET_REL, ET_EXEC and ET_DYN are supported")),
    };
    let mut regions = if kind == ImageKind::ElfObject {
        object_regions(data)?
    } else {
        load_regions(data)?
    };
    if regions.is_empty() {
        return Err(invalid(
            "ELF contains no nonempty file-backed executable regions",
        ));
    }
    regions.sort_by_key(|region| region.file_offset);
    for adjacent in regions.windows(2) {
        if adjacent[0].file_offset + adjacent[0].length > adjacent[1].file_offset {
            return Err(invalid(
                "overlapping executable file ranges are unsupported",
            ));
        }
    }
    if kind != ImageKind::ElfObject {
        validate_virtual_ranges(&regions)?;
        // Coalesce adjacent file and virtual ranges so a signature straddling
        // two headers is not missed. ET_REL sections remain independent.
        let mut merged: Vec<Region> = Vec::new();
        for region in regions {
            if let Some(previous) = merged.last_mut() {
                let contiguous_memory = previous
                    .virtual_address
                    .map(|base| Some(base + previous.length as u64) == region.virtual_address)
                    == Some(true);
                if previous.file_offset + previous.length == region.file_offset && contiguous_memory
                {
                    previous.length += region.length;
                    // Keep the label bounded even for thousands of tiny headers.
                    previous.name = "coalesced PT_LOAD".into();
                    continue;
                }
            }
            merged.push(region);
        }
        regions = merged;
    }
    Ok(Image { kind, regions })
}

fn load_regions(data: &[u8]) -> Result<Vec<Region>, ScanError> {
    let table = usize_at(data, 32)?;
    let count = u16_at(data, 56)? as usize;
    let entry_size = u16_at(data, 54)? as usize;
    check_table(data, table, count, entry_size, 56)?;
    let mut regions = Vec::new();
    for index in 0..count {
        let entry = table + index * entry_size;
        if u32_at(data, entry)? != 1 {
            continue;
        }
        let flags = u32_at(data, entry + 4)?;
        let offset = usize_at(data, entry + 8)?;
        let address = u64_at(data, entry + 16)?;
        let size = usize_at(data, entry + 32)?;
        let memory_size = u64_at(data, entry + 40)?;
        let alignment = u64_at(data, entry + 48)?;
        if size as u64 > memory_size {
            return Err(invalid("ELF PT_LOAD file size exceeds memory size"));
        }
        address
            .checked_add(memory_size)
            .ok_or_else(|| invalid("ELF virtual address range overflows"))?;
        if alignment > 1
            && (!alignment.is_power_of_two() || address % alignment != offset as u64 % alignment)
        {
            return Err(invalid(
                "ELF PT_LOAD alignment or address congruence is invalid",
            ));
        }
        range(data, offset, size)?;
        if flags & 1 != 0 {
            if memory_size != size as u64 {
                return Err(invalid(
                    "executable zero-fill PT_LOAD tails are unsupported",
                ));
            }
            if size != 0 {
                regions.push(Region {
                    name: format!("segment[{index}]"),
                    file_offset: offset,
                    length: size,
                    virtual_address: Some(address),
                });
            }
        }
    }
    Ok(regions)
}

fn object_regions(data: &[u8]) -> Result<Vec<Region>, ScanError> {
    let table = usize_at(data, 40)?;
    let count = u16_at(data, 60)? as usize;
    let entry_size = u16_at(data, 58)? as usize;
    check_table(data, table, count, entry_size, 64)?;
    let mut regions = Vec::new();
    for index in 0..count {
        let entry = table + index * entry_size;
        let section_type = u32_at(data, entry + 4)?;
        let flags = u64_at(data, entry + 8)?;
        let offset = usize_at(data, entry + 24)?;
        let size = usize_at(data, entry + 32)?;
        let alignment = u64_at(data, entry + 48)?;
        if alignment > 1 && !alignment.is_power_of_two() {
            return Err(invalid("ELF section alignment is not a power of two"));
        }
        if section_type != 8 && section_type != 0 {
            range(data, offset, size)?;
        }
        if flags & 4 != 0 && size != 0 {
            if section_type != 1 || flags & 0x800 != 0 {
                return Err(invalid(
                    "executable sections must be uncompressed SHT_PROGBITS",
                ));
            }
            regions.push(Region {
                name: format!("section[{index}]"),
                file_offset: offset,
                length: size,
                virtual_address: None,
            });
        }
    }
    Ok(regions)
}

fn validate_virtual_ranges(regions: &[Region]) -> Result<(), ScanError> {
    let mut sorted: Vec<&Region> = regions.iter().collect();
    sorted.sort_by_key(|region| region.virtual_address);
    for adjacent in sorted.windows(2) {
        let (Some(left), Some(right)) = (adjacent[0].virtual_address, adjacent[1].virtual_address)
        else {
            return Err(invalid("internal ELF virtual address invariant failed"));
        };
        let end = left + adjacent[0].length as u64;
        if end > right {
            return Err(invalid(
                "overlapping executable virtual ranges are unsupported",
            ));
        }
        if end == right && adjacent[0].file_offset + adjacent[0].length != adjacent[1].file_offset {
            return Err(invalid(
                "adjacent executable virtual ranges must be contiguous in the file",
            ));
        }
    }
    Ok(())
}
