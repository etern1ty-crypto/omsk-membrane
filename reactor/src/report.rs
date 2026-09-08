//! Deterministic streaming reports. No source/binary content or environment dump.

use crate::config::Settings;
use crate::io_pump::{FileReport, PumpOutcome, Summary};
use gulag::{Finding, Rule};
use std::io::{self, Write};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Text,
    Json,
    Sarif,
}

impl OutputFormat {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "text" => Ok(Self::Text),
            "json" => Ok(Self::Json),
            "sarif" => Ok(Self::Sarif),
            _ => Err(format!(
                "invalid output format {value:?}; expected text, json or sarif"
            )),
        }
    }
}

/// Escape control characters, including terminal escape bytes, in JSON strings.
pub fn json_string(writer: &mut impl Write, value: &str) -> io::Result<()> {
    writer.write_all(b"\"")?;
    for character in value.chars() {
        match character {
            '"' => writer.write_all(b"\\\"")?,
            '\\' => writer.write_all(b"\\\\")?,
            '\n' => writer.write_all(b"\\n")?,
            '\r' => writer.write_all(b"\\r")?,
            '\t' => writer.write_all(b"\\t")?,
            character if character <= '\u{1f}' => write!(writer, "\\u{:04x}", character as u32)?,
            character => {
                let mut encoded = [0u8; 4];
                writer.write_all(character.encode_utf8(&mut encoded).as_bytes())?;
            }
        }
    }
    writer.write_all(b"\"")
}

fn comma(writer: &mut impl Write, first: &mut bool) -> io::Result<()> {
    if *first {
        *first = false;
        Ok(())
    } else {
        writer.write_all(b",")
    }
}

fn json_finding(writer: &mut impl Write, finding: &Finding) -> io::Result<()> {
    writer.write_all(b"{\"rule_id\":")?;
    json_string(writer, finding.rule.id())?;
    writer.write_all(b",\"rule\":")?;
    json_string(writer, finding.rule.name())?;
    write!(
        writer,
        ",\"file_offset\":{},\"byte_length\":{},\"virtual_address\":",
        finding.file_offset, finding.byte_length
    )?;
    match finding.virtual_address {
        Some(address) => json_string(writer, &format!("0x{address:x}"))?,
        None => writer.write_all(b"null")?,
    }
    writer.write_all(b",\"region\":")?;
    json_string(writer, &finding.region)?;
    writer.write_all(b"}")
}

fn json_summary(writer: &mut impl Write, summary: &Summary) -> io::Result<()> {
    writer.write_all(b"{")?;
    write!(writer, "\"requested\":{},\"completed\":{},\"clean\":{},\"violating\":{},\"errors\":{},\"cancelled\":{},\"total_findings\":{},\"reported_findings\":{}", summary.requested, summary.completed, summary.clean, summary.violating, summary.errors, summary.cancelled, summary.total_findings, summary.reported_findings)?;
    writer.write_all(b"}")
}

pub fn write_rules(writer: &mut impl Write, json: bool) -> io::Result<()> {
    if !json {
        for rule in Rule::ALL {
            writeln!(
                writer,
                "{} {:8} {:30} {}",
                rule.id(),
                rule.name(),
                rule.signature(),
                rule.description()
            )?;
        }
        return Ok(());
    }
    writer.write_all(b"[")?;
    let mut first = true;
    for rule in Rule::ALL {
        comma(writer, &mut first)?;
        writer.write_all(b"{\"id\":")?;
        json_string(writer, rule.id())?;
        writer.write_all(b",\"name\":")?;
        json_string(writer, rule.name())?;
        writer.write_all(b",\"signature\":")?;
        json_string(writer, rule.signature())?;
        writer.write_all(b",\"description\":")?;
        json_string(writer, rule.description())?;
        writer.write_all(b"}")?;
    }
    writer.write_all(b"]\n")
}

fn artifact_uri(path: &str) -> String {
    let mut encoded = String::new();
    for byte in path.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                encoded.push(*byte as char)
            }
            _ => {
                use std::fmt::Write as _;
                let _ = write!(encoded, "%{byte:02X}");
            }
        }
    }
    if path.starts_with('/') {
        format!("file://{encoded}")
    } else {
        format!("./{encoded}")
    }
}

struct ArtifactEntry {
    uri: String,
    status: &'static str,
    scanned_bytes: u64,
    total_findings: u64,
    omitted_findings: u64,
}

struct Notification {
    level: &'static str,
    message: String,
}

pub struct Reporter<'a, W: Write> {
    writer: &'a mut W,
    format: OutputFormat,
    first_file: bool,
    first_result: bool,
    artifacts: Vec<ArtifactEntry>,
    notifications: Vec<Notification>,
}

impl<'a, W: Write> Reporter<'a, W> {
    pub fn begin(writer: &'a mut W, settings: &Settings) -> io::Result<Self> {
        match settings.output_format {
            OutputFormat::Text => {
                writeln!(
                    writer,
                    "OMSK Membrane {VERSION} | byte-pattern lint | policy={}",
                    settings.profile
                )?;
                writeln!(
                    writer,
                    "No matches is not a security proof. Offsets refer to file bytes."
                )?;
            }
            OutputFormat::Json => {
                writer
                    .write_all(b"{\"schema_version\":1,\"tool\":{\"name\":\"omsk\",\"version\":")?;
                json_string(writer, VERSION)?;
                writer.write_all(b"},\"analysis\":\"byte-pattern-lint\",\"input_format\":")?;
                json_string(writer, settings.input_format.name())?;
                writer.write_all(b",\"policy\":{\"name\":")?;
                json_string(writer, &settings.profile)?;
                writer.write_all(b",\"rules\":[")?;
                let mut first = true;
                for rule in settings.policy.rules() {
                    comma(writer, &mut first)?;
                    json_string(writer, rule.name())?;
                }
                writer.write_all(b"]},\"files\":[")?;
            }
            OutputFormat::Sarif => {
                writer.write_all(b"{\"version\":\"2.1.0\",\"$schema\":\"https://json.schemastore.org/sarif-2.1.0.json\",\"runs\":[{\"tool\":{\"driver\":{\"name\":\"OMSK Membrane\",\"version\":")?;
                json_string(writer, VERSION)?;
                writer.write_all(b",\"rules\":[")?;
                let mut first = true;
                for rule in Rule::ALL {
                    comma(writer, &mut first)?;
                    writer.write_all(b"{\"id\":")?;
                    json_string(writer, rule.id())?;
                    writer.write_all(b",\"name\":")?;
                    json_string(writer, rule.name())?;
                    writer.write_all(b",\"shortDescription\":{\"text\":")?;
                    json_string(writer, rule.description())?;
                    writer.write_all(b"},\"fullDescription\":{\"text\":")?;
                    json_string(writer, &format!("Conservative byte signature: {}. A match may be data inside an executable region, not a reachable instruction. This is not an execution-safety proof.", rule.signature()))?;
                    writer.write_all(b"}}")?;
                }
                writer.write_all(
                    b"]}},\"properties\":{\"analysisMode\":\"byte-pattern-lint\",\"policy\":",
                )?;
                json_string(writer, &settings.profile)?;
                writer.write_all(b",\"enabledRules\":[")?;
                let mut first = true;
                for rule in settings.policy.rules() {
                    comma(writer, &mut first)?;
                    json_string(writer, rule.id())?;
                }
                writer.write_all(b"]},\"results\":[")?;
            }
        }
        Ok(Self {
            writer,
            format: settings.output_format,
            first_file: true,
            first_result: true,
            artifacts: Vec::new(),
            notifications: Vec::new(),
        })
    }

    pub fn file(&mut self, file: &FileReport) -> io::Result<()> {
        match self.format {
            OutputFormat::Text => self.text_file(file),
            OutputFormat::Json => self.json_file(file),
            OutputFormat::Sarif => self.sarif_file(file),
        }
    }

    fn text_file(&mut self, file: &FileReport) -> io::Result<()> {
        writeln!(
            self.writer,
            "\n{} {:?}",
            file.status().to_uppercase(),
            file.path
        )?;
        match &file.result {
            Ok(scan) => {
                writeln!(
                    self.writer,
                    "  {} | {} scanned bytes | {} match(es)",
                    scan.kind.name(),
                    scan.scanned_bytes,
                    scan.total_findings
                )?;
                for finding in &scan.findings {
                    writeln!(
                        self.writer,
                        "  {} {:8} offset=0x{:x} length={} region={}",
                        finding.rule.id(),
                        finding.rule.name(),
                        finding.file_offset,
                        finding.byte_length,
                        finding.region
                    )?;
                }
                if scan.omitted_findings() > 0 {
                    writeln!(self.writer, "  {} additional matches omitted by --max-findings; still counted and failing", scan.omitted_findings())?;
                }
            }
            Err(error) => writeln!(self.writer, "  {:?}", error.message())?,
        }
        Ok(())
    }

    fn json_file(&mut self, file: &FileReport) -> io::Result<()> {
        comma(self.writer, &mut self.first_file)?;
        self.writer.write_all(b"{\"path\":")?;
        json_string(self.writer, &file.path.to_string_lossy())?;
        self.writer.write_all(b",\"status\":")?;
        json_string(self.writer, file.status())?;
        match &file.result {
            Ok(scan) => {
                self.writer.write_all(b",\"kind\":")?;
                json_string(self.writer, scan.kind.name())?;
                write!(self.writer, ",\"file_bytes\":{},\"scanned_bytes\":{},\"region_count\":{},\"total_findings\":{},\"omitted_findings\":{},\"findings\":[", scan.file_bytes, scan.scanned_bytes, scan.region_count, scan.total_findings, scan.omitted_findings())?;
                let mut first = true;
                for finding in &scan.findings {
                    comma(self.writer, &mut first)?;
                    json_finding(self.writer, finding)?;
                }
                self.writer.write_all(b"]")?;
            }
            Err(error) => {
                self.writer.write_all(b",\"error\":")?;
                json_string(self.writer, error.message())?;
            }
        }
        self.writer.write_all(b"}")
    }

    fn sarif_file(&mut self, file: &FileReport) -> io::Result<()> {
        let uri = artifact_uri(&file.path.to_string_lossy());
        let mut artifact = ArtifactEntry {
            uri: uri.clone(),
            status: file.status(),
            scanned_bytes: 0,
            total_findings: 0,
            omitted_findings: 0,
        };
        match &file.result {
            Ok(scan) => {
                artifact.scanned_bytes = scan.scanned_bytes;
                artifact.total_findings = scan.total_findings;
                artifact.omitted_findings = scan.omitted_findings();
                for finding in &scan.findings {
                    comma(self.writer, &mut self.first_result)?;
                    self.writer.write_all(b"{\"ruleId\":")?;
                    json_string(self.writer, finding.rule.id())?;
                    self.writer.write_all(
                        b",\"level\":\"error\",\"kind\":\"fail\",\"message\":{\"text\":",
                    )?;
                    json_string(self.writer, &format!("{} byte pattern at file offset 0x{:x} in {}; review in a disassembler before treating it as an instruction", finding.rule.name(), finding.file_offset, finding.region))?;
                    self.writer.write_all(
                        b"},\"locations\":[{\"physicalLocation\":{\"artifactLocation\":{\"uri\":",
                    )?;
                    json_string(self.writer, &uri)?;
                    write!(self.writer, ",\"index\":{}", file.index)?;
                    self.writer.write_all(b"},\"region\":{")?;
                    write!(
                        self.writer,
                        "\"byteOffset\":{},\"byteLength\":{}",
                        finding.file_offset, finding.byte_length
                    )?;
                    self.writer.write_all(b"}}}]}")?;
                }
                if scan.omitted_findings() > 0 {
                    self.notifications.push(Notification {
                        level: "warning",
                        message: format!(
                            "{:?}: {} matches omitted by max-findings; full match count retained",
                            file.path,
                            scan.omitted_findings()
                        ),
                    });
                }
            }
            Err(error) => self.notifications.push(Notification {
                level: "error",
                message: format!("{:?}: {}", file.path, error.message()),
            }),
        }
        self.artifacts.push(artifact);
        Ok(())
    }

    pub fn finish(self, outcome: &PumpOutcome, exit_code: u8) -> io::Result<()> {
        match self.format {
            OutputFormat::Text => {
                writeln!(self.writer, "\nSummary: completed={}/{} clean={} violating={} errors={} cancelled={} matches={} stored={} complete={} exit={}", outcome.summary.completed, outcome.summary.requested, outcome.summary.clean, outcome.summary.violating, outcome.summary.errors, outcome.summary.cancelled, outcome.summary.total_findings, outcome.summary.reported_findings, outcome.complete, exit_code)?;
                if let Some(error) = &outcome.engine_error {
                    writeln!(self.writer, "Engine error: {error:?}")?;
                }
                Ok(())
            }
            OutputFormat::Json => {
                self.writer.write_all(b"],\"summary\":")?;
                json_summary(self.writer, &outcome.summary)?;
                write!(
                    self.writer,
                    ",\"complete\":{},\"exit_code\":{},\"engine_error\":",
                    outcome.complete, exit_code
                )?;
                match &outcome.engine_error {
                    Some(error) => json_string(self.writer, error)?,
                    None => self.writer.write_all(b"null")?,
                }
                self.writer.write_all(b"}\n")
            }
            OutputFormat::Sarif => {
                self.writer.write_all(b"],\"artifacts\":[")?;
                let mut first = true;
                for artifact in &self.artifacts {
                    comma(self.writer, &mut first)?;
                    self.writer.write_all(b"{\"location\":{\"uri\":")?;
                    json_string(self.writer, &artifact.uri)?;
                    self.writer.write_all(
                        b"},\"roles\":[\"analysisTarget\"],\"properties\":{\"status\":",
                    )?;
                    json_string(self.writer, artifact.status)?;
                    write!(
                        self.writer,
                        ",\"scannedBytes\":{},\"totalFindings\":{},\"omittedFindings\":{}",
                        artifact.scanned_bytes, artifact.total_findings, artifact.omitted_findings
                    )?;
                    self.writer.write_all(b"}}")?;
                }
                self.writer.write_all(b"],\"invocations\":[{")?;
                write!(
                    self.writer,
                    "\"executionSuccessful\":{},\"exitCode\":{},\"toolExecutionNotifications\":[",
                    outcome.complete, exit_code
                )?;
                let mut first = true;
                for notification in &self.notifications {
                    comma(self.writer, &mut first)?;
                    self.writer.write_all(b"{\"level\":")?;
                    json_string(self.writer, notification.level)?;
                    self.writer.write_all(b",\"message\":{\"text\":")?;
                    json_string(self.writer, &notification.message)?;
                    self.writer.write_all(b"}}")?;
                }
                if !outcome.complete {
                    comma(self.writer, &mut first)?;
                    self.writer
                        .write_all(b"{\"level\":\"error\",\"message\":{\"text\":")?;
                    json_string(
                        self.writer,
                        outcome
                            .engine_error
                            .as_deref()
                            .unwrap_or("incomplete scan; do not treat this report as passing"),
                    )?;
                    self.writer.write_all(b"}}")?;
                }
                self.writer.write_all(b"],\"properties\":{\"complete\":")?;
                write!(self.writer, "{},\"summary\":", outcome.complete)?;
                json_summary(self.writer, &outcome.summary)?;
                self.writer.write_all(b"}}]}]}\n")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_controls_quotes_and_unicode() {
        let mut data = Vec::new();
        json_string(&mut data, "\"\\\n\r\t\u{0}\u{1b}тест").unwrap();
        assert_eq!(
            String::from_utf8(data).unwrap(),
            "\"\\\"\\\\\\n\\r\\t\\u0000\\u001bтест\""
        );
    }

    #[test]
    fn uris_encode_reserved_characters() {
        assert_eq!(artifact_uri("a b#c?.so"), "./a%20b%23c%3F.so");
        assert_eq!(artifact_uri("/tmp/a.so"), "file:///tmp/a.so");
        assert_eq!(artifact_uri("a:b"), "./a%3Ab");
    }
}
