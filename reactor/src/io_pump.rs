//! Finite, backpressured artifact pipeline. No polling loop or fake ACKs.

use crate::config::Settings;
use crate::guest::{read_regular_file, InputError};
use crate::shutdown::Cancellation;
use gulag::{ScanError, ScanOptions, ScanReport};
use std::io;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;
use synapse::RecvTimeoutError;

#[derive(Debug)]
pub enum FileIssue {
    Cancelled,
    Failed(String),
}

impl FileIssue {
    pub fn message(&self) -> &str {
        match self {
            Self::Cancelled => "cancelled; no complete scan for this artifact",
            Self::Failed(message) => message,
        }
    }
}

#[derive(Debug)]
pub struct FileReport {
    pub index: usize,
    pub path: PathBuf,
    pub result: Result<ScanReport, FileIssue>,
}

impl FileReport {
    pub fn status(&self) -> &'static str {
        match &self.result {
            Ok(report) if report.total_findings == 0 => "clean",
            Ok(_) => "violations",
            Err(FileIssue::Cancelled) => "cancelled",
            Err(FileIssue::Failed(_)) => "error",
        }
    }
}

#[derive(Debug, Default)]
pub struct Summary {
    pub requested: usize,
    pub completed: usize,
    pub clean: usize,
    pub violating: usize,
    pub errors: usize,
    pub cancelled: usize,
    pub total_findings: u64,
    pub reported_findings: u64,
}

impl Summary {
    fn record(&mut self, report: &FileReport) {
        self.completed += 1;
        match &report.result {
            Ok(scan) => {
                self.total_findings += scan.total_findings;
                self.reported_findings += scan.findings.len() as u64;
                if scan.total_findings == 0 {
                    self.clean += 1;
                } else {
                    self.violating += 1;
                }
            }
            Err(FileIssue::Cancelled) => self.cancelled += 1,
            Err(FileIssue::Failed(_)) => self.errors += 1,
        }
    }
}

#[derive(Debug)]
pub struct PumpOutcome {
    pub summary: Summary,
    pub complete: bool,
    pub engine_error: Option<String>,
}

fn scan_one(
    index: usize,
    path: PathBuf,
    settings: &Settings,
    cancellation: &Cancellation,
) -> FileReport {
    let result = match read_regular_file(&path, settings.max_file_bytes, || {
        cancellation.is_cancelled()
    }) {
        Ok(bytes) => gulag::scan(
            &bytes,
            settings.input_format,
            &settings.policy,
            ScanOptions {
                max_findings: settings.max_findings,
            },
            || cancellation.is_cancelled(),
        )
        .map_err(|error| match error {
            ScanError::Cancelled => FileIssue::Cancelled,
            _ => FileIssue::Failed(error.to_string()),
        }),
        Err(InputError::Cancelled) => Err(FileIssue::Cancelled),
        Err(error) => Err(FileIssue::Failed(error.to_string())),
    };
    FileReport {
        index,
        path,
        result,
    }
}

/// Process each input in argument order. The callback is the single writer.
/// Drop the result receiver before joining, releasing any blocked sender.
pub fn run(
    paths: Vec<PathBuf>,
    settings: Settings,
    cancellation: Cancellation,
    mut on_report: impl FnMut(&FileReport) -> io::Result<()>,
) -> io::Result<PumpOutcome> {
    let mut summary = Summary {
        requested: paths.len(),
        ..Summary::default()
    };
    let (mut jobs_tx, mut jobs_rx) = synapse::bounded::<(usize, PathBuf)>(settings.queue_capacity)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    let (mut results_tx, mut results_rx) = synapse::bounded::<FileReport>(2)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    let feeder_cancellation = cancellation.clone();
    let feeder = thread::Builder::new()
        .name("omsk-feeder".into())
        .spawn(move || {
            for (index, path) in paths.into_iter().enumerate() {
                if feeder_cancellation.is_cancelled() || jobs_tx.send((index, path)).is_err() {
                    break;
                }
            }
        })?;
    let worker_cancellation = cancellation.clone();
    let worker = match thread::Builder::new()
        .name("omsk-scanner".into())
        .spawn(move || loop {
            if worker_cancellation.is_cancelled() {
                break;
            }
            let (index, path) = match jobs_rx.recv_timeout(Duration::from_millis(25)) {
                Ok(job) => job,
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => break,
            };
            let report = scan_one(index, path, &settings, &worker_cancellation);
            if results_tx.send(report).is_err() {
                break;
            }
        }) {
        Ok(worker) => worker,
        Err(error) => {
            // Failed spawn drops its closure, including jobs_rx/results_tx.
            cancellation.cancel();
            drop(results_rx);
            let _ = feeder.join();
            return Err(error);
        }
    };
    let mut output_error = None;
    let mut engine_error = None;
    loop {
        if cancellation.is_cancelled() {
            break;
        }
        match results_rx.recv_timeout(Duration::from_millis(25)) {
            Ok(report) => {
                if report.index != summary.completed {
                    engine_error = Some("worker returned results out of order".into());
                    break;
                }
                summary.record(&report);
                if let Err(error) = on_report(&report) {
                    output_error = Some(error);
                    break;
                }
            }
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
    let was_cancelled = cancellation.is_cancelled();
    if was_cancelled
        || output_error.is_some()
        || engine_error.is_some()
        || summary.completed != summary.requested
    {
        cancellation.cancel();
    }
    drop(results_rx);
    if worker.join().is_err() {
        engine_error = Some("scan worker panicked".into());
    }
    if feeder.join().is_err() {
        engine_error = Some("job feeder panicked".into());
    }
    if let Some(error) = output_error {
        return Err(error);
    }
    if !was_cancelled && summary.completed != summary.requested && engine_error.is_none() {
        engine_error = Some("pipeline ended before every input received a result".into());
    }
    let complete = !was_cancelled
        && engine_error.is_none()
        && summary.completed == summary.requested
        && summary.errors == 0
        && summary.cancelled == 0;
    Ok(PumpOutcome {
        summary,
        complete,
        engine_error,
    })
}
