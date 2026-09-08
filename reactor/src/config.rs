use crate::guest::read_regular_file;
use crate::report::OutputFormat;
use gulag::{InputFormat, Policy};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

pub const HELP: &str = "OMSK Membrane — offline x86-64 artifact policy gate

Usage:
  omsk scan [OPTIONS] [--] FILE [FILE ...]
  omsk rules [--format text|json]
  omsk config
  omsk --help
  omsk --version

Scan options:
  --config PATH          Explicit environment file (maximum 16 KiB)
  --input-format FORMAT  auto (ELF only), elf, raw [default: auto]
  --profile PROFILE      strict, syscalls, timing [default: strict]
  --deny RULES           Comma-separated rules; REPLACES the profile
  --format FORMAT        text, json, sarif [default: text]
  --max-file-bytes N     1..1073741824 [default: 67108864]
  --max-findings N       Stored findings per file, 1..100000 [default: 1000]
  --queue-capacity N     Bounded path queue, 1..64 [default: 4]
  --output PATH          Atomically publish a NEW report; never overwrite
  --quiet                Suppress progress on stderr
  --no-quiet             Override OMSK_QUIET=true
  --help                 Show this help

Precedence: defaults < --config < OMSK_* environment < command line.
Input: explicit regular files, up to 512; no symlinks, recursion or stdin.
Only 64-bit Linux is currently supported for the CLI.
Exit: 0 no matches; 1 matches; 2 error; 130 SIGINT; 143 SIGTERM.
A clean scan is NOT a security proof or permission to execute an artifact.
";

pub const CONFIG_EXAMPLE: &str = include_str!("../../.env.example");

pub const ENV_KEYS: [&str; 9] = [
    "OMSK_PROFILE",
    "OMSK_DENY",
    "OMSK_INPUT_FORMAT",
    "OMSK_FORMAT",
    "OMSK_MAX_FILE_BYTES",
    "OMSK_MAX_FINDINGS",
    "OMSK_QUEUE_CAPACITY",
    "OMSK_QUIET",
    "OMSK_UNUSED_RESERVED",
];

#[derive(Debug, Clone)]
pub struct Settings {
    pub profile: String,
    pub policy: Policy,
    pub input_format: InputFormat,
    pub output_format: OutputFormat,
    pub max_file_bytes: usize,
    pub max_findings: usize,
    pub queue_capacity: usize,
    pub quiet: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            profile: "strict".into(),
            policy: Policy::strict(),
            input_format: InputFormat::Auto,
            output_format: OutputFormat::Text,
            max_file_bytes: 67_108_864,
            max_findings: 1000,
            queue_capacity: 4,
            quiet: false,
        }
    }
}

#[derive(Debug)]
pub enum Command {
    Help,
    Version,
    ConfigExample,
    Rules {
        json: bool,
    },
    Scan {
        settings: Settings,
        paths: Vec<PathBuf>,
        output: Option<PathBuf>,
    },
}

fn is_known_key(key: &str) -> bool {
    ENV_KEYS[..8].contains(&key)
}

pub fn environment() -> Result<BTreeMap<String, String>, String> {
    let mut result = BTreeMap::new();
    for key in &ENV_KEYS[..8] {
        match std::env::var(key) {
            Ok(value) => {
                result.insert((*key).to_owned(), value);
            }
            Err(std::env::VarError::NotPresent) => {}
            Err(std::env::VarError::NotUnicode(_)) => return Err(format!("{key} must be UTF-8")),
        }
    }
    Ok(result)
}

fn insert_once(map: &mut BTreeMap<String, String>, key: &str, value: String) -> Result<(), String> {
    if map.insert(key.to_owned(), value).is_some() {
        return Err(format!("duplicate option for {key}"));
    }
    Ok(())
}

fn next_value(args: &[String], index: &mut usize) -> Result<String, String> {
    let option = &args[*index];
    *index += 1;
    args.get(*index)
        .cloned()
        .ok_or_else(|| format!("missing value after {option}"))
}

pub fn parse(args: Vec<String>, env: BTreeMap<String, String>) -> Result<Command, String> {
    let Some(first) = args.first() else {
        return Ok(Command::Help);
    };
    match first.as_str() {
        "--help" | "-h" | "help" if args.len() == 1 => return Ok(Command::Help),
        "--version" | "-V" if args.len() == 1 => return Ok(Command::Version),
        "config" if args.len() == 1 => return Ok(Command::ConfigExample),
        "rules" => {
            let format = match args.as_slice() {
                [_] => "text",
                [_, flag, value] if flag == "--format" => value.as_str(),
                _ => return Err("usage: omsk rules [--format text|json]".into()),
            };
            return match format {
                "text" => Ok(Command::Rules { json: false }),
                "json" => Ok(Command::Rules { json: true }),
                _ => Err("rules supports only text or json output".into()),
            };
        }
        "scan" => {}
        _ => {
            return Err(format!(
                "unknown command or extra arguments: {first:?}; use --help"
            ))
        }
    }
    let mut cli = BTreeMap::new();
    let mut paths = Vec::new();
    let mut config = None;
    let mut output = None;
    let mut positional_only = false;
    let mut index = 1;
    while index < args.len() {
        let arg = &args[index];
        if positional_only || !arg.starts_with('-') || arg == "-" {
            if arg.is_empty() || arg.len() > 4096 {
                return Err("input path must contain 1..4096 UTF-8 bytes".into());
            }
            paths.push(PathBuf::from(arg));
        } else if arg == "--" {
            positional_only = true;
        } else if arg == "--help" || arg == "-h" {
            return Ok(Command::Help);
        } else {
            match arg.as_str() {
                "--config" => {
                    let value = next_value(&args, &mut index)?;
                    if config.replace(PathBuf::from(value)).is_some() {
                        return Err("duplicate --config".into());
                    }
                }
                "--output" => {
                    let value = next_value(&args, &mut index)?;
                    if output.replace(PathBuf::from(value)).is_some() {
                        return Err("duplicate --output".into());
                    }
                }
                "--quiet" => insert_once(&mut cli, "OMSK_QUIET", "true".into())?,
                "--no-quiet" => insert_once(&mut cli, "OMSK_QUIET", "false".into())?,
                option => {
                    let key = match option {
                        "--input-format" => "OMSK_INPUT_FORMAT",
                        "--profile" => "OMSK_PROFILE",
                        "--deny" => "OMSK_DENY",
                        "--format" => "OMSK_FORMAT",
                        "--max-file-bytes" => "OMSK_MAX_FILE_BYTES",
                        "--max-findings" => "OMSK_MAX_FINDINGS",
                        "--queue-capacity" => "OMSK_QUEUE_CAPACITY",
                        _ => {
                            return Err(format!(
                            "unknown option {option:?}; use -- before a filename starting with '-'"
                        ))
                        }
                    };
                    let value = next_value(&args, &mut index)?;
                    insert_once(&mut cli, key, value)?;
                }
            }
        }
        if paths.len() > 512 {
            return Err("at most 512 input files are accepted per invocation".into());
        }
        index += 1;
    }
    if paths.is_empty() {
        return Err("scan requires at least one input file".into());
    }
    let mut distinct = BTreeSet::new();
    if paths.iter().any(|path| !distinct.insert(path)) {
        return Err("duplicate input path; each argument must be unique".into());
    }
    let mut values = if let Some(path) = config {
        let bytes = read_regular_file(&path, 16_384, || false)
            .map_err(|error| format!("config {path:?}: {error}"))?;
        let text = std::str::from_utf8(&bytes).map_err(|_| "config must be UTF-8")?;
        parse_env_file(text)?
    } else {
        BTreeMap::new()
    };
    overlay(
        &mut values,
        env.into_iter()
            .filter(|(key, _)| is_known_key(key))
            .collect(),
    );
    overlay(&mut values, cli);
    Ok(Command::Scan {
        settings: settings_from_values(&values)?,
        paths,
        output,
    })
}

fn overlay(base: &mut BTreeMap<String, String>, layer: BTreeMap<String, String>) {
    // An explicit higher-priority profile resets a lower-priority custom list.
    // If both are set in the same layer, the custom list wins.
    if layer.contains_key("OMSK_PROFILE") && !layer.contains_key("OMSK_DENY") {
        base.remove("OMSK_DENY");
    }
    base.extend(layer);
}

pub fn parse_env_file(text: &str) -> Result<BTreeMap<String, String>, String> {
    let mut values = BTreeMap::new();
    for (index, original) in text.trim_start_matches('\u{feff}').lines().enumerate() {
        let line = original.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (key, raw) = line
            .split_once('=')
            .ok_or_else(|| format!("config line {}: expected KEY=value", index + 1))?;
        let key = key.trim();
        if !is_known_key(key) {
            return Err(format!("config line {}: unknown key {key:?}", index + 1));
        }
        let raw = raw.trim();
        let value = if raw.starts_with('"') || raw.starts_with('\'') {
            let quote = raw.as_bytes()[0] as char;
            let rest = &raw[1..];
            let close = rest
                .find(quote)
                .ok_or_else(|| format!("config line {}: unclosed quote", index + 1))?;
            let trailing = rest[close + 1..].trim();
            if !trailing.is_empty() && !trailing.starts_with('#') {
                return Err(format!(
                    "config line {}: unexpected text after quote",
                    index + 1
                ));
            }
            &rest[..close]
        } else {
            raw.split('#').next().unwrap_or("").trim()
        };
        if value
            .chars()
            .any(|ch| matches!(ch, '\\' | '$' | '`' | '\'' | '"'))
        {
            return Err(format!(
                "config line {}: escapes and shell expansion are unsupported",
                index + 1
            ));
        }
        if values.insert(key.to_owned(), value.to_owned()).is_some() {
            return Err(format!("config line {}: duplicate key {key}", index + 1));
        }
    }
    Ok(values)
}

fn number(
    values: &BTreeMap<String, String>,
    key: &str,
    default: usize,
    max: usize,
) -> Result<usize, String> {
    let Some(text) = values.get(key) else {
        return Ok(default);
    };
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(format!("{key} must be a positive decimal integer"));
    }
    let value = text
        .parse::<usize>()
        .map_err(|_| format!("{key} overflows this host's integer range"))?;
    if !(1..=max).contains(&value) {
        return Err(format!("{key} must be between 1 and {max}"));
    }
    Ok(value)
}

fn settings_from_values(values: &BTreeMap<String, String>) -> Result<Settings, String> {
    let mut settings = Settings::default();
    if let Some(profile) = values.get("OMSK_PROFILE") {
        settings.policy = Policy::from_profile(profile)?;
        settings.profile.clone_from(profile);
    }
    if let Some(deny) = values.get("OMSK_DENY") {
        settings.policy = Policy::from_csv(deny)?;
        settings.profile = "custom".into();
    }
    if let Some(format) = values.get("OMSK_INPUT_FORMAT") {
        settings.input_format = InputFormat::parse(format)?;
    }
    if let Some(format) = values.get("OMSK_FORMAT") {
        settings.output_format = OutputFormat::parse(format)?;
    }
    if let Some(quiet) = values.get("OMSK_QUIET") {
        settings.quiet = match quiet.as_str() {
            "true" => true,
            "false" => false,
            _ => return Err("OMSK_QUIET must be true or false".into()),
        };
    }
    settings.max_file_bytes = number(
        values,
        "OMSK_MAX_FILE_BYTES",
        settings.max_file_bytes,
        1_073_741_824,
    )?;
    settings.max_findings = number(values, "OMSK_MAX_FINDINGS", settings.max_findings, 100_000)?;
    settings.queue_capacity = number(values, "OMSK_QUEUE_CAPACITY", settings.queue_capacity, 64)?;
    Ok(settings)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(args: &[&str]) -> Result<Command, String> {
        parse(
            args.iter().map(|value| (*value).to_owned()).collect(),
            BTreeMap::new(),
        )
    }

    #[test]
    fn help_and_version_do_not_need_files() {
        assert!(matches!(parsed(&[]).unwrap(), Command::Help));
        assert!(matches!(parsed(&["--version"]).unwrap(), Command::Version));
        assert!(matches!(
            parsed(&["scan", "--help"]).unwrap(),
            Command::Help
        ));
    }

    #[test]
    fn rejects_unknown_duplicate_and_missing_options() {
        for args in [
            vec!["scan"],
            vec!["scan", "--bogus", "a"],
            vec!["scan", "--max-findings"],
            vec!["scan", "--quiet", "--quiet", "a"],
            vec!["scan", "a", "a"],
            vec!["scan", "--profile", "unknown", "a"],
            vec!["scan", "--deny", "", "a"],
            vec!["scan", "--max-findings", "0", "a"],
            vec!["scan", "--max-file-bytes", "9999999999999999999999999", "a"],
        ] {
            assert!(parsed(&args).is_err(), "accepted {args:?}");
        }
    }

    #[test]
    fn separator_accepts_option_like_filename() {
        let Command::Scan { paths, .. } = parsed(&["scan", "--", "--help"]).unwrap() else {
            panic!("wrong command")
        };
        assert_eq!(paths, vec![PathBuf::from("--help")]);
    }

    #[test]
    fn environment_is_overridden_by_cli() {
        let env = BTreeMap::from([
            ("OMSK_PROFILE".into(), "timing".into()),
            ("OMSK_QUIET".into(), "true".into()),
        ]);
        let args = ["scan", "--profile", "syscalls", "--no-quiet", "a"]
            .map(String::from)
            .to_vec();
        let Command::Scan { settings, .. } = parse(args, env).unwrap() else {
            panic!("wrong command")
        };
        assert_eq!(settings.profile, "syscalls");
        assert!(!settings.quiet);
    }

    #[test]
    fn configuration_comments_quotes_and_crlf() {
        let values = parse_env_file(
            "\u{feff}# comment\r\nOMSK_PROFILE='timing' # policy\r\nOMSK_MAX_FINDINGS=2\r\n",
        )
        .unwrap();
        let settings = settings_from_values(&values).unwrap();
        assert_eq!(settings.profile, "timing");
        assert_eq!(settings.max_findings, 2);
    }

    #[test]
    fn configuration_is_not_a_shell() {
        for text in [
            "export OMSK_PROFILE=strict",
            "OMSK_UNKNOWN=1",
            "OMSK_PROFILE=$(echo strict)",
            "OMSK_PROFILE=strict\nOMSK_PROFILE=timing",
            "OMSK_PROFILE=\"strict\" trailing",
        ] {
            assert!(parse_env_file(text).is_err(), "accepted {text:?}");
        }
    }

    #[test]
    fn higher_priority_profile_resets_custom_rules() {
        let env = BTreeMap::from([("OMSK_DENY".into(), "syscall".into())]);
        let args = ["scan", "--profile", "timing", "a"]
            .map(String::from)
            .to_vec();
        let Command::Scan { settings, .. } = parse(args, env).unwrap() else {
            panic!("wrong command")
        };
        assert_eq!(settings.profile, "timing");
        assert_eq!(settings.policy, Policy::from_profile("timing").unwrap());
    }

    #[test]
    fn shipped_example_is_valid() {
        let values = parse_env_file(CONFIG_EXAMPLE).unwrap();
        let settings = settings_from_values(&values).unwrap();
        assert_eq!(settings.max_file_bytes, 67_108_864);
        assert_eq!(settings.policy, Policy::strict());
    }
}
