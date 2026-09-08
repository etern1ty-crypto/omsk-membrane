//! Streaming stdout or atomic, no-clobber publication on a local filesystem.

use std::fs::{self, File, OpenOptions};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

enum Sink {
    Stdout(BufWriter<io::Stdout>),
    File {
        writer: Option<BufWriter<File>>,
        temporary: PathBuf,
        destination: PathBuf,
    },
}

pub struct Output {
    sink: Sink,
}

impl Output {
    pub fn open(path: Option<&Path>) -> io::Result<Self> {
        let Some(destination) = path else {
            return Ok(Self {
                sink: Sink::Stdout(BufWriter::new(io::stdout())),
            });
        };
        if destination.file_name().is_none() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "output must name a file",
            ));
        }
        // symlink_metadata detects dangling symlinks as existing output too.
        match fs::symlink_metadata(destination) {
            Ok(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "output already exists; choose a new report path",
                ))
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        let parent = destination
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        for _ in 0..64 {
            let number = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let temporary =
                parent.join(format!(".omsk-report-{}-{number}.tmp", std::process::id()));
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            match options.open(&temporary) {
                Ok(file) => {
                    return Ok(Self {
                        sink: Sink::File {
                            writer: Some(BufWriter::new(file)),
                            temporary,
                            destination: destination.to_owned(),
                        },
                    });
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "could not reserve a temporary report file",
        ))
    }

    /// A complete report is made visible with a hard link, which cannot replace
    /// an existing destination, even if it appeared after Output::open.
    /// Requires a filesystem supporting hard links in a trusted parent directory.
    pub fn commit(mut self) -> io::Result<()> {
        match &mut self.sink {
            Sink::Stdout(writer) => writer.flush(),
            Sink::File {
                writer,
                temporary,
                destination,
            } => {
                if let Some(mut buffered) = writer.take() {
                    buffered.flush()?;
                    buffered.get_ref().sync_all()?;
                    drop(buffered);
                }
                fs::hard_link(&*temporary, &*destination)?;
                fs::remove_file(&*temporary)?;
                #[cfg(unix)]
                {
                    let parent = destination
                        .parent()
                        .filter(|p| !p.as_os_str().is_empty())
                        .unwrap_or(Path::new("."));
                    File::open(parent)?.sync_all()?;
                }
                Ok(())
            }
        }
    }
}

impl Write for Output {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        match &mut self.sink {
            Sink::Stdout(writer) => writer.write(bytes),
            Sink::File {
                writer: Some(writer),
                ..
            } => writer.write(bytes),
            Sink::File { writer: None, .. } => Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "report is already finalized",
            )),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match &mut self.sink {
            Sink::Stdout(writer) => writer.flush(),
            Sink::File {
                writer: Some(writer),
                ..
            } => writer.flush(),
            Sink::File { writer: None, .. } => Ok(()),
        }
    }
}

impl Drop for Output {
    fn drop(&mut self) {
        if let Sink::File {
            writer, temporary, ..
        } = &mut self.sink
        {
            drop(writer.take());
            // The temp is absent after a successful commit. On cancellation or
            // an output error, no incomplete destination is published.
            let _ = fs::remove_file(temporary);
        }
    }
}
