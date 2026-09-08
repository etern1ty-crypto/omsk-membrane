use reactor::config::{self, Command, Settings};
use reactor::io_pump;
use reactor::output::Output;
use reactor::report::{self, Reporter, VERSION};
use reactor::shutdown::{self, Cancellation, SignalGuard};
use std::collections::BTreeMap;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

fn scan(settings: Settings, paths: Vec<PathBuf>, destination: Option<PathBuf>) -> io::Result<u8> {
    let _signals = SignalGuard::install()?;
    let mut output = Output::open(destination.as_deref())?;
    let mut reporter = Reporter::begin(&mut output, &settings)?;
    let quiet = settings.quiet;
    if !quiet {
        let _ = writeln!(
            io::stderr().lock(),
            "level=info event=scan_start files={} profile={} max_file_bytes={}",
            paths.len(),
            settings.profile,
            settings.max_file_bytes
        );
    }
    let mut outcome = io_pump::run(paths, settings, Cancellation::default(), |file| {
        reporter.file(file)?;
        if !quiet {
            let _ = writeln!(
                io::stderr().lock(),
                "level=info event=file_complete status={} path={:?}",
                file.status(),
                file.path
            );
        }
        Ok(())
    })?;
    let exit_code = if let Some(code) = shutdown::requested_exit_code() {
        outcome.complete = false;
        code
    } else if !outcome.complete {
        2
    } else if outcome.summary.total_findings > 0 {
        1
    } else {
        0
    };
    reporter.finish(&outcome, exit_code)?;
    output.commit()?;
    if !quiet {
        let _ = writeln!(
            io::stderr().lock(),
            "level=info event=scan_end complete={} exit_code={} total_findings={}",
            outcome.complete,
            exit_code,
            outcome.summary.total_findings
        );
    }
    Ok(exit_code)
}

fn execute() -> Result<u8, String> {
    let args: Vec<String> = std::env::args_os()
        .skip(1)
        .map(|arg| {
            arg.into_string()
                .map_err(|_| "arguments and paths must be UTF-8".to_owned())
        })
        .collect::<Result<_, _>>()?;
    let help_requested = args
        .iter()
        .take_while(|arg| arg.as_str() != "--")
        .any(|arg| arg == "--help" || arg == "-h");
    let env = if args.first().is_some_and(|arg| arg == "scan") && !help_requested {
        config::environment()?
    } else {
        BTreeMap::new()
    };
    let command = config::parse(args, env)?;
    let result = match command {
        Command::Help => io::stdout()
            .lock()
            .write_all(config::HELP.as_bytes())
            .map(|_| 0),
        Command::Version => writeln!(io::stdout().lock(), "omsk {VERSION}").map(|_| 0),
        Command::ConfigExample => io::stdout()
            .lock()
            .write_all(config::CONFIG_EXAMPLE.as_bytes())
            .map(|_| 0),
        Command::Rules { json } => report::write_rules(&mut io::stdout().lock(), json).map(|_| 0),
        Command::Scan {
            settings,
            paths,
            output,
        } => scan(settings, paths, output),
    };
    result.map_err(|error| error.to_string())
}

fn main() -> ExitCode {
    match execute() {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            // Debug-escaped message prevents filenames/OS errors from injecting
            // terminal controls. A closed stderr must not panic during cleanup.
            let _ = writeln!(
                io::stderr().lock(),
                "level=error event=command_failed message={error:?}"
            );
            ExitCode::from(shutdown::requested_exit_code().unwrap_or(2))
        }
    }
}
