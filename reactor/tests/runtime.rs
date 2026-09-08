use reactor::config::Settings;
use reactor::guest::read_regular_file;
use reactor::io_pump;
use reactor::output::Output;
use reactor::shutdown::Cancellation;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Directory(PathBuf);

impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "omsk-runtime-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn input_is_bounded_and_snapshot_is_owned() {
    let directory = Directory::new();
    let path = directory.0.join("input.bin");
    fs::write(&path, [0x90, 0xc3]).unwrap();
    assert!(read_regular_file(&path, 1, || false).is_err());
    let bytes = read_regular_file(&path, 2, || false).unwrap();
    fs::write(&path, [0x0f, 0x05]).unwrap();
    assert_eq!(bytes, [0x90, 0xc3]);
    assert!(read_regular_file(&path, 2, || true).is_err());
    assert!(read_regular_file(&directory.0, 100, || false).is_err());
}

#[cfg(unix)]
#[test]
fn rejects_final_symlinks() {
    let directory = Directory::new();
    let path = directory.0.join("input.bin");
    let link = directory.0.join("link.bin");
    fs::write(&path, [0x90]).unwrap();
    std::os::unix::fs::symlink(&path, &link).unwrap();
    assert!(read_regular_file(&link, 100, || false).is_err());
}

#[test]
fn atomic_report_appears_only_on_commit() {
    let directory = Directory::new();
    let path = directory.0.join("report.json");
    let mut output = Output::open(Some(&path)).unwrap();
    output.write_all(b"{\"complete\":true}\n").unwrap();
    assert!(!path.exists());
    output.commit().unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "{\"complete\":true}\n");
    assert!(Output::open(Some(&path)).is_err());
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 1);
}

#[test]
fn dropped_output_does_not_publish_partial_file() {
    let directory = Directory::new();
    let path = directory.0.join("report.json");
    {
        let mut output = Output::open(Some(&path)).unwrap();
        output.write_all(b"partial").unwrap();
    }
    assert!(!path.exists());
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 0);
}

#[test]
fn output_race_cannot_clobber_another_writer() {
    let directory = Directory::new();
    let path = directory.0.join("report.json");
    let mut output = Output::open(Some(&path)).unwrap();
    output.write_all(b"ours").unwrap();
    fs::write(&path, b"theirs").unwrap();
    assert!(output.commit().is_err());
    assert_eq!(fs::read(&path).unwrap(), b"theirs");
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 1);
}

fn raw_settings() -> Settings {
    Settings {
        input_format: gulag::InputFormat::Raw,
        queue_capacity: 1,
        ..Settings::default()
    }
}

#[test]
fn pipeline_is_ordered_and_has_real_results() {
    let directory = Directory::new();
    let paths: Vec<PathBuf> = (0..100)
        .map(|index| {
            let path = directory.0.join(format!("file-{index}.bin"));
            fs::write(&path, [0x0f, 0x05]).unwrap();
            path
        })
        .collect();
    let mut seen = Vec::new();
    let outcome = io_pump::run(paths, raw_settings(), Cancellation::default(), |report| {
        seen.push(report.index);
        assert_eq!(report.result.as_ref().unwrap().total_findings, 1);
        Ok(())
    })
    .unwrap();
    assert!(outcome.complete);
    assert_eq!(outcome.summary.total_findings, 100);
    assert_eq!(seen, (0..100).collect::<Vec<_>>());
}

#[test]
fn output_failure_releases_backpressure_and_joins() {
    let directory = Directory::new();
    let paths: Vec<PathBuf> = (0..100)
        .map(|index| {
            let path = directory.0.join(format!("file-{index}.bin"));
            fs::write(&path, [0x90]).unwrap();
            path
        })
        .collect();
    let result = io_pump::run(paths, raw_settings(), Cancellation::default(), |_| {
        Err(io::Error::new(
            io::ErrorKind::BrokenPipe,
            "test consumer stopped",
        ))
    });
    assert_eq!(result.unwrap_err().kind(), io::ErrorKind::BrokenPipe);
}

#[test]
fn cancelled_pipeline_is_never_complete() {
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let outcome = io_pump::run(
        vec![PathBuf::from("not-read.bin")],
        raw_settings(),
        cancellation,
        |_| panic!("cancelled job must not run"),
    )
    .unwrap();
    assert!(!outcome.complete);
    assert_eq!(outcome.summary.completed, 0);
}

#[test]
fn unreadable_input_is_not_reported_as_passing() {
    let outcome = io_pump::run(
        vec![PathBuf::from("/nonexistent/omsk-no-such-artifact")],
        raw_settings(),
        Cancellation::default(),
        |_| Ok(()),
    )
    .unwrap();
    assert!(!outcome.complete);
    assert_eq!(outcome.summary.errors, 1);
}
