//! Bounded immutable input snapshots. No guest execution or executable mapping.

use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::{self, Read};
use std::path::Path;

#[derive(Debug)]
pub enum InputError {
    Cancelled,
    Io(io::Error),
    Rejected(String),
}

impl fmt::Display for InputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cancelled => f.write_str("input read cancelled"),
            Self::Io(error) => write!(f, "{error}"),
            Self::Rejected(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for InputError {}

impl From<io::Error> for InputError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

pub fn read_regular_file(
    path: &Path,
    max_bytes: usize,
    is_cancelled: impl Fn() -> bool,
) -> Result<Vec<u8>, InputError> {
    if is_cancelled() {
        return Err(InputError::Cancelled);
    }
    let before_open = fs::symlink_metadata(path)?;
    if !before_open.file_type().is_file() {
        return Err(InputError::Rejected(
            "input must be a regular file, not a symlink, directory, device or pipe".into(),
        ));
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::OpenOptionsExt;
        // Linux UAPI values: O_NOFOLLOW | O_NONBLOCK. O_NONBLOCK prevents
        // blocking if a regular-file path is swapped for a FIFO before open.
        options.custom_flags(0x20000 | 0x800);
    }
    let mut file = options.open(path)?;
    let initial = file.metadata()?;
    if !initial.file_type().is_file() {
        return Err(InputError::Rejected(
            "opened input is not a regular file".into(),
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if before_open.dev() != initial.dev() || before_open.ino() != initial.ino() {
            return Err(InputError::Rejected(
                "input was replaced while opening".into(),
            ));
        }
    }
    if initial.len() > max_bytes as u64 {
        return Err(InputError::Rejected(format!(
            "input exceeds maximum of {max_bytes} bytes"
        )));
    }
    let mut data = Vec::new();
    let mut chunk = [0u8; 65_536];
    loop {
        if is_cancelled() {
            return Err(InputError::Cancelled);
        }
        let length = match file.read(&mut chunk) {
            Ok(length) => length,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error.into()),
        };
        if length == 0 {
            break;
        }
        if length > max_bytes.saturating_sub(data.len()) {
            return Err(InputError::Rejected(format!(
                "input grew beyond maximum of {max_bytes} bytes"
            )));
        }
        data.try_reserve(length)
            .map_err(|_| InputError::Rejected("could not allocate input buffer".into()))?;
        data.extend_from_slice(&chunk[..length]);
    }
    let final_metadata = file.metadata()?;
    if initial.len() != final_metadata.len()
        || initial.len() != data.len() as u64
        || initial.modified().ok() != final_metadata.modified().ok()
    {
        return Err(InputError::Rejected(
            "input changed during the read; use immutable build artifacts".into(),
        ));
    }
    if is_cancelled() {
        return Err(InputError::Cancelled);
    }
    Ok(data)
}
