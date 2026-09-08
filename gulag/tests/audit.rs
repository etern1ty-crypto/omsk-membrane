use gulag::{parse_image, scan, ImageKind, InputFormat, Policy, Rule, ScanError, ScanOptions};
use std::cell::Cell;

fn analyze(bytes: &[u8]) -> gulag::ScanReport {
    scan(
        bytes,
        InputFormat::Raw,
        &Policy::strict(),
        ScanOptions::default(),
        || false,
    )
    .unwrap()
}

fn put16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}
fn put32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}
fn put64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn elf(payload: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0; 128 + payload.len()];
    bytes[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
    put16(&mut bytes, 16, 3);
    put16(&mut bytes, 18, 62);
    put32(&mut bytes, 20, 1);
    put64(&mut bytes, 32, 64);
    put16(&mut bytes, 52, 64);
    put16(&mut bytes, 54, 56);
    put16(&mut bytes, 56, 1);
    put32(&mut bytes, 64, 1);
    put32(&mut bytes, 68, 5);
    put64(&mut bytes, 72, 128);
    put64(&mut bytes, 80, 0x400080);
    put64(&mut bytes, 96, payload.len() as u64);
    put64(&mut bytes, 104, payload.len() as u64);
    put64(&mut bytes, 112, 1);
    bytes[128..].copy_from_slice(payload);
    bytes
}

fn scan_elf(bytes: &[u8]) -> Result<gulag::ScanReport, ScanError> {
    scan(
        bytes,
        InputFormat::Auto,
        &Policy::strict(),
        ScanOptions::default(),
        || false,
    )
}

#[test]
fn detects_every_rule_at_the_last_possible_offset() {
    let cases: &[(Rule, &[u8])] = &[
        (Rule::Syscall, &[0x0f, 0x05]),
        (Rule::Sysenter, &[0x0f, 0x34]),
        (Rule::Int80, &[0xcd, 0x80]),
        (Rule::Wrpkru, &[0x0f, 0x01, 0xef]),
        (Rule::Xrstor, &[0x0f, 0xae, 0x28]),
        (Rule::Xrstors, &[0x0f, 0xc7, 0x18]),
        (Rule::Rdtsc, &[0x0f, 0x31]),
        (Rule::Rdtscp, &[0x0f, 0x01, 0xf9]),
    ];
    for (rule, payload) in cases {
        let mut bytes = vec![0x90; 9];
        bytes.extend_from_slice(payload);
        let report = analyze(&bytes);
        assert_eq!(report.total_findings, 1);
        assert_eq!(report.findings[0].rule, *rule);
        assert_eq!(report.findings[0].file_offset, 9);
        assert_eq!(report.findings[0].byte_length, payload.len());
    }
}

#[test]
fn xrstor_modrm_is_memory_only_and_uses_correct_extension() {
    for modrm in 0..=u8::MAX {
        let report = analyze(&[0x0f, 0xae, modrm]);
        let expected = modrm >> 6 != 3 && (modrm >> 3) & 7 == 5;
        assert_eq!(
            report.total_findings,
            u64::from(expected),
            "ModRM={modrm:02x}"
        );
    }
}

#[test]
fn xrstors_modrm_is_memory_only_and_uses_correct_extension() {
    for modrm in 0..=u8::MAX {
        let report = analyze(&[0x0f, 0xc7, modrm]);
        let expected = modrm >> 6 != 3 && (modrm >> 3) & 7 == 3;
        assert_eq!(
            report.total_findings,
            u64::from(expected),
            "ModRM={modrm:02x}"
        );
    }
}

#[test]
fn accepts_nonmatching_bytes_and_one_byte_input() {
    assert_eq!(analyze(&[0x90, 0xc3]).total_findings, 0);
    assert_eq!(analyze(&[0x0f]).total_findings, 0);
}

#[test]
fn rejects_empty_artifact() {
    assert!(scan(
        &[],
        InputFormat::Raw,
        &Policy::strict(),
        ScanOptions::default(),
        || false
    )
    .is_err());
}

#[test]
fn explicitly_reports_immediate_byte_matches() {
    // MOV eax, 0x0000050f; RET. A byte lint deliberately also matches data.
    let report = analyze(&[0xb8, 0x0f, 0x05, 0, 0, 0xc3]);
    assert_eq!(report.total_findings, 1);
    assert_eq!(report.findings[0].file_offset, 1);
}

#[test]
fn recognizes_signatures_after_rex_or_legacy_prefix() {
    assert_eq!(analyze(&[0x48, 0x0f, 0xae, 0x28]).total_findings, 1);
    assert_eq!(analyze(&[0x66, 0x0f, 0x05]).total_findings, 1);
}

#[test]
fn bounded_storage_never_changes_failure_count() {
    let bytes = [0x0f, 0x05].repeat(100);
    let report = scan(
        &bytes,
        InputFormat::Raw,
        &Policy::strict(),
        ScanOptions { max_findings: 2 },
        || false,
    )
    .unwrap();
    assert_eq!(report.total_findings, 100);
    assert_eq!(report.findings.len(), 2);
    assert_eq!(report.omitted_findings(), 98);
    assert_eq!(report.scanned_bytes, 200);
}

#[test]
fn rejects_invalid_result_limits() {
    for limit in [0, 100_001, usize::MAX] {
        assert!(scan(
            &[0x90],
            InputFormat::Raw,
            &Policy::strict(),
            ScanOptions {
                max_findings: limit
            },
            || false
        )
        .is_err());
    }
}

#[test]
fn profiles_and_custom_policy_are_nonempty_and_canonical() {
    assert!(Policy::new(&[]).is_err());
    assert!(Policy::from_csv("").is_err());
    assert!(Policy::from_csv("syscall,syscall").is_err());
    assert!(Policy::from_csv("SYSCALL").is_err());
    assert!(Policy::from_profile("sandbox").is_err());
    assert_eq!(
        Policy::from_csv("rdtscp,syscall").unwrap().rules(),
        &[Rule::Syscall, Rule::Rdtscp]
    );
    assert_eq!(
        Policy::from_profile("timing").unwrap().rules(),
        &[Rule::Rdtsc, Rule::Rdtscp]
    );
}

#[test]
fn profile_filters_rules() {
    let report = scan(
        &[0x0f, 0x05, 0x0f, 0x31],
        InputFormat::Raw,
        &Policy::from_profile("timing").unwrap(),
        ScanOptions::default(),
        || false,
    )
    .unwrap();
    assert_eq!(report.total_findings, 1);
    assert_eq!(report.findings[0].rule, Rule::Rdtsc);
}

#[test]
fn cancellation_before_work_is_not_a_pass() {
    assert_eq!(
        scan(
            &[0x90],
            InputFormat::Raw,
            &Policy::strict(),
            ScanOptions::default(),
            || true
        ),
        Err(ScanError::Cancelled)
    );
}

#[test]
fn cancellation_during_scan_is_observed() {
    let calls = Cell::new(0);
    let bytes = vec![0x90; 100_000];
    let result = scan(
        &bytes,
        InputFormat::Raw,
        &Policy::strict(),
        ScanOptions::default(),
        || {
            calls.set(calls.get() + 1);
            calls.get() >= 3
        },
    );
    assert_eq!(result, Err(ScanError::Cancelled));
}

#[test]
fn stripped_elf_uses_executable_segments() {
    let report = scan_elf(&elf(&[0x90, 0x0f, 0x05, 0xc3])).unwrap();
    assert_eq!(report.kind, ImageKind::ElfShared);
    assert_eq!(report.scanned_bytes, 4);
    assert_eq!(report.findings[0].file_offset, 129);
    assert_eq!(report.findings[0].virtual_address, Some(0x400081));
}

#[test]
fn does_not_scan_non_executable_file_padding() {
    let mut bytes = elf(&[0x90, 0xc3]);
    bytes.extend_from_slice(&[0x0f, 0x05]);
    assert_eq!(scan_elf(&bytes).unwrap().total_findings, 0);
}

#[test]
fn rejects_missing_executable_regions() {
    let mut bytes = elf(&[0x90]);
    put32(&mut bytes, 68, 4);
    assert!(scan_elf(&bytes).is_err());
}

#[test]
fn auto_never_falls_back_to_raw() {
    for bytes in [b"\x90\xc3".as_slice(), b"MZbad", b"\x7fELFbad"] {
        assert!(parse_image(bytes, InputFormat::Auto).is_err());
    }
}

#[test]
fn malformed_headers_offsets_sizes_and_architectures_fail_closed() {
    let original = elf(&[0x90, 0xc3]);
    for length in 0..original.len() {
        assert!(
            scan_elf(&original[..length]).is_err(),
            "accepted truncated length {length}"
        );
    }
    for (offset, value) in [(4, 1), (5, 2), (6, 0), (18, 3), (52, 1), (54, 1), (56, 0)] {
        let mut bytes = original.clone();
        bytes[offset] = value;
        assert!(
            scan_elf(&bytes).is_err(),
            "accepted malformed offset {offset}"
        );
    }
    for offset in [32, 72, 80, 96, 104] {
        let mut bytes = original.clone();
        put64(&mut bytes, offset, u64::MAX);
        assert!(
            scan_elf(&bytes).is_err(),
            "accepted overflowing field {offset}"
        );
    }
}

#[test]
fn rejects_extended_program_header_numbering() {
    let mut bytes = elf(&[0x90]);
    put16(&mut bytes, 56, 0xffff);
    assert!(scan_elf(&bytes).is_err());
}

#[test]
fn rejects_invalid_alignment_and_executable_zero_fill() {
    let mut bytes = elf(&[0x90]);
    put64(&mut bytes, 112, 3);
    assert!(scan_elf(&bytes).is_err());
    let mut bytes = elf(&[0x90]);
    put64(&mut bytes, 104, 2);
    assert!(scan_elf(&bytes).is_err());
}

fn two_segments(second_offset: usize, second_address: u64) -> Vec<u8> {
    let mut bytes = elf(&[0x90]);
    bytes.resize(256, 0x90);
    put16(&mut bytes, 56, 2);
    put64(&mut bytes, 72, 192);
    put64(&mut bytes, 80, 0x4000c0);
    let header: Vec<u8> = bytes[64..120].to_vec();
    bytes[120..176].copy_from_slice(&header);
    put64(&mut bytes, 128, second_offset as u64);
    put64(&mut bytes, 136, second_address);
    put64(&mut bytes, 152, 2);
    put64(&mut bytes, 160, 2);
    bytes[192] = 0x0f;
    if second_offset < 255 {
        bytes[second_offset] = 0x05;
    }
    bytes
}

#[test]
fn joins_contiguous_regions_to_find_boundary_signature() {
    let report = scan_elf(&two_segments(193, 0x4000c1)).unwrap();
    assert_eq!(report.region_count, 1);
    assert_eq!(report.findings[0].file_offset, 192);
}

#[test]
fn rejects_ambiguous_or_overlapping_executable_layout() {
    assert!(scan_elf(&two_segments(192, 0x4000c1)).is_err());
    assert!(scan_elf(&two_segments(200, 0x4000c0)).is_err());
    assert!(scan_elf(&two_segments(200, 0x4000c1)).is_err());
}

#[test]
fn relocatable_object_scans_executable_progbits() {
    let mut bytes = elf(&[0x90]);
    bytes.resize(195, 0);
    put16(&mut bytes, 16, 1);
    put64(&mut bytes, 32, 0);
    put64(&mut bytes, 40, 64);
    put16(&mut bytes, 56, 0);
    put16(&mut bytes, 58, 64);
    put16(&mut bytes, 60, 2);
    bytes[64..192].fill(0);
    put32(&mut bytes, 132, 1);
    put64(&mut bytes, 136, 6);
    put64(&mut bytes, 152, 192);
    put64(&mut bytes, 160, 3);
    bytes[192..].copy_from_slice(&[0x0f, 0x05, 0xc3]);
    let report = scan_elf(&bytes).unwrap();
    assert_eq!(report.kind, ImageKind::ElfObject);
    assert_eq!(report.findings[0].file_offset, 192);
    assert_eq!(report.findings[0].virtual_address, None);
    put32(&mut bytes, 132, 8);
    assert!(scan_elf(&bytes).is_err());
}

#[test]
fn random_input_and_header_mutation_never_panics() {
    let mut state = 0x91u64;
    for length in 1..512 {
        let mut bytes = vec![0; length];
        for byte in &mut bytes {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            *byte = (state >> 24) as u8;
        }
        let _ = scan_elf(&bytes);
        let _ = analyze(&bytes);
    }
    let original = elf(&[0x90, 0xc3]);
    for offset in 0..original.len() {
        for value in [0, 1, 127, 255] {
            let mut bytes = original.clone();
            bytes[offset] = value;
            let _ = scan_elf(&bytes);
        }
    }
}
