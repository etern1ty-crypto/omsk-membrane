//! The rule set is a list of byte signatures, not a complete x86 decoder.

use std::fmt;

/// Stable rule identifiers. Changing an identifier is a report-schema change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Rule {
    Syscall,
    Sysenter,
    Int80,
    Wrpkru,
    Xrstor,
    Xrstors,
    Rdtsc,
    Rdtscp,
}

impl Rule {
    /// Canonical order, also used when a custom policy is supplied.
    pub const ALL: [Self; 8] = [
        Self::Syscall,
        Self::Sysenter,
        Self::Int80,
        Self::Wrpkru,
        Self::Xrstor,
        Self::Xrstors,
        Self::Rdtsc,
        Self::Rdtscp,
    ];

    pub const fn id(self) -> &'static str {
        match self {
            Self::Syscall => "OMSK001",
            Self::Sysenter => "OMSK002",
            Self::Int80 => "OMSK003",
            Self::Wrpkru => "OMSK004",
            Self::Xrstor => "OMSK005",
            Self::Xrstors => "OMSK006",
            Self::Rdtsc => "OMSK007",
            Self::Rdtscp => "OMSK008",
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Syscall => "syscall",
            Self::Sysenter => "sysenter",
            Self::Int80 => "int80",
            Self::Wrpkru => "wrpkru",
            Self::Xrstor => "xrstor",
            Self::Xrstors => "xrstors",
            Self::Rdtsc => "rdtsc",
            Self::Rdtscp => "rdtscp",
        }
    }

    pub const fn signature(self) -> &'static str {
        match self {
            Self::Syscall => "0F 05",
            Self::Sysenter => "0F 34",
            Self::Int80 => "CD 80",
            Self::Wrpkru => "0F 01 EF",
            Self::Xrstor => "0F AE /5 (memory ModRM)",
            Self::Xrstors => "0F C7 /3 (memory ModRM)",
            Self::Rdtsc => "0F 31",
            Self::Rdtscp => "0F 01 F9",
        }
    }

    pub const fn description(self) -> &'static str {
        match self {
            Self::Syscall => "Direct system-call entry byte pattern",
            Self::Sysenter => "Fast legacy system-call entry byte pattern",
            Self::Int80 => "Legacy interrupt 0x80 byte pattern",
            Self::Wrpkru => "Protection-key register write byte pattern",
            Self::Xrstor => "Extended-state restore byte pattern; may restore PKRU",
            Self::Xrstors => "Supervisor extended-state restore byte pattern",
            Self::Rdtsc => "Timestamp-counter read byte pattern",
            Self::Rdtscp => "Timestamp-counter and processor-ID read byte pattern",
        }
    }

    /// Matched signature length, NOT the full decoded instruction length.
    pub const fn signature_len(self) -> usize {
        match self {
            Self::Syscall | Self::Sysenter | Self::Int80 | Self::Rdtsc => 2,
            _ => 3,
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|rule| rule.name() == name)
    }

    pub(crate) fn matches(self, data: &[u8]) -> bool {
        match self {
            Self::Syscall => data.starts_with(&[0x0f, 0x05]),
            Self::Sysenter => data.starts_with(&[0x0f, 0x34]),
            Self::Int80 => data.starts_with(&[0xcd, 0x80]),
            Self::Wrpkru => data.starts_with(&[0x0f, 0x01, 0xef]),
            Self::Rdtsc => data.starts_with(&[0x0f, 0x31]),
            Self::Rdtscp => data.starts_with(&[0x0f, 0x01, 0xf9]),
            Self::Xrstor | Self::Xrstors => {
                let (opcode, extension) = if self == Self::Xrstor {
                    (0xae, 5)
                } else {
                    (0xc7, 3)
                };
                data.starts_with(&[0x0f, opcode])
                    && data
                        .get(2)
                        .is_some_and(|modrm| modrm >> 6 != 3 && (modrm >> 3) & 7 == extension)
            }
        }
    }
}

impl fmt::Display for Rule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// A nonempty set of rules in deterministic order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Policy {
    rules: Vec<Rule>,
}

impl Policy {
    pub fn strict() -> Self {
        Self {
            rules: Rule::ALL.to_vec(),
        }
    }

    pub fn from_profile(profile: &str) -> Result<Self, String> {
        match profile {
            "strict" => Ok(Self::strict()),
            "syscalls" => Self::new(&[Rule::Syscall, Rule::Sysenter, Rule::Int80]),
            "timing" => Self::new(&[Rule::Rdtsc, Rule::Rdtscp]),
            _ => Err(format!(
                "unknown profile {profile:?}; expected strict, syscalls or timing"
            )),
        }
    }

    pub fn new(rules: &[Rule]) -> Result<Self, String> {
        if rules.is_empty() {
            return Err("a policy must contain at least one rule".into());
        }
        Ok(Self {
            rules: Rule::ALL
                .into_iter()
                .filter(|rule| rules.contains(rule))
                .collect(),
        })
    }

    pub fn from_csv(value: &str) -> Result<Self, String> {
        let mut rules = Vec::new();
        for part in value.split(',') {
            let name = part.trim();
            let rule = Rule::from_name(name)
                .ok_or_else(|| format!("unknown or empty rule name {name:?}"))?;
            if rules.contains(&rule) {
                return Err(format!("duplicate rule {name:?}"));
            }
            rules.push(rule);
        }
        Self::new(&rules)
    }

    pub fn rules(&self) -> &[Rule] {
        &self.rules
    }
}

impl Default for Policy {
    fn default() -> Self {
        Self::strict()
    }
}
