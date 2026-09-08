//! Non-executing, bounded byte-pattern policy analysis.
//!
//! A clean report means only that the selected signatures were absent from the
//! scanned regions. It does not establish instruction boundaries, reachability,
//! memory isolation, deterministic behavior or safety of execution.
//!
//! ```
//! use gulag::{scan, InputFormat, Policy, ScanOptions};
//!
//! let report = scan(
//!     &[0x90, 0x0f, 0x05, 0xc3],
//!     InputFormat::Raw,
//!     &Policy::strict(),
//!     ScanOptions::default(),
//!     || false,
//! ).unwrap();
//! assert_eq!(report.total_findings, 1);
//! assert_eq!(report.findings[0].file_offset, 1);
//! ```

#![forbid(unsafe_code)]

mod image;
mod rules;

pub use image::{parse_image, Image, ImageKind, InputFormat, Region};
pub use rules::{Policy, Rule};

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScanError {
    InvalidInput(String),
    Cancelled,
    AllocationFailed,
}

impl fmt::Display for ScanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(message) => f.write_str(message),
            Self::Cancelled => f.write_str("scan cancelled; no complete result for this file"),
            Self::AllocationFailed => f.write_str("could not allocate bounded result storage"),
        }
    }
}

impl std::error::Error for ScanError {}

/// Maximum stored findings. The scanner still counts every match after this cap.
#[derive(Debug, Clone, Copy)]
pub struct ScanOptions {
    pub max_findings: usize,
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self { max_findings: 1000 }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub rule: Rule,
    /// Offset in the original file, not a source line or decoded instruction index.
    pub file_offset: u64,
    /// Signature length, not the complete instruction length.
    pub byte_length: usize,
    pub virtual_address: Option<u64>,
    pub region: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanReport {
    pub kind: ImageKind,
    pub file_bytes: u64,
    pub scanned_bytes: u64,
    pub region_count: usize,
    pub total_findings: u64,
    pub findings: Vec<Finding>,
}

impl ScanReport {
    pub fn omitted_findings(&self) -> u64 {
        self.total_findings - self.findings.len() as u64
    }
}

/// Scan each byte offset in the selected executable regions.
///
/// This is deliberately conservative: a signature in an immediate or embedded
/// constant also matches. Cancellation is checked at least every 4096 bytes.
/// Invalid/unsupported ELF never falls back to raw input.
pub fn scan(
    data: &[u8],
    input_format: InputFormat,
    policy: &Policy,
    options: ScanOptions,
    is_cancelled: impl Fn() -> bool,
) -> Result<ScanReport, ScanError> {
    if !(1..=100_000).contains(&options.max_findings) {
        return Err(ScanError::InvalidInput(
            "max_findings must be between 1 and 100000".into(),
        ));
    }
    if is_cancelled() {
        return Err(ScanError::Cancelled);
    }
    let image = parse_image(data, input_format)?;
    let mut report = ScanReport {
        kind: image.kind,
        file_bytes: data.len() as u64,
        scanned_bytes: 0,
        region_count: image.regions.len(),
        total_findings: 0,
        findings: Vec::new(),
    };
    report
        .findings
        .try_reserve(options.max_findings.min(data.len()))
        .map_err(|_| ScanError::AllocationFailed)?;

    for region in image.regions {
        let bytes = &data[region.file_offset..region.file_offset + region.length];
        for offset in 0..bytes.len() {
            if offset & 4095 == 0 && is_cancelled() {
                return Err(ScanError::Cancelled);
            }
            for &rule in policy.rules() {
                if rule.matches(&bytes[offset..]) {
                    report.total_findings += 1;
                    if report.findings.len() < options.max_findings {
                        report.findings.push(Finding {
                            rule,
                            file_offset: (region.file_offset + offset) as u64,
                            byte_length: rule.signature_len(),
                            virtual_address: region
                                .virtual_address
                                .map(|base| base + offset as u64),
                            region: region.name.clone(),
                        });
                    }
                }
            }
        }
        report.scanned_bytes += bytes.len() as u64;
    }
    if is_cancelled() {
        return Err(ScanError::Cancelled);
    }
    Ok(report)
}
