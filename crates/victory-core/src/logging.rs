//! One tracing setup for every binary.
//!
//! Filter levels with `VICTORY_LOG` (falls back to `RUST_LOG`). The CLI can
//! additionally mirror output to `victory.log` and filter complete rendered
//! log lines with a regular expression.

use std::{
    fs::{File, OpenOptions},
    io::{self, Write},
    sync::{Arc, Mutex},
};

use anyhow::{Context, Result};
use regex::Regex;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

/// The most verbose level this build can log.
const CAP: LevelFilter = if cfg!(feature = "diagnostics") {
    LevelFilter::TRACE
} else {
    LevelFilter::INFO
};

struct Sinks {
    stderr: io::Stderr,
    file: Option<File>,
}

impl Sinks {
    fn write_line(&mut self, line: &[u8], show_in_terminal: bool) -> io::Result<()> {
        if show_in_terminal {
            self.stderr.write_all(line)?;
        }
        if let Some(file) = &mut self.file {
            file.write_all(line)?;
        }
        Ok(())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.stderr.flush()?;
        if let Some(file) = &mut self.file {
            file.flush()?;
        }
        Ok(())
    }
}

#[derive(Clone)]
struct LineMakeWriter {
    sinks: Arc<Mutex<Sinks>>,
    grep: Option<Regex>,
}

struct LineWriter {
    sinks: Arc<Mutex<Sinks>>,
    grep: Option<Regex>,
    pending: Vec<u8>,
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for LineMakeWriter {
    type Writer = LineWriter;

    fn make_writer(&'a self) -> Self::Writer {
        LineWriter {
            sinks: self.sinks.clone(),
            grep: self.grep.clone(),
            pending: Vec::with_capacity(512),
        }
    }
}

impl LineWriter {
    fn emit(&mut self, line: &[u8]) -> io::Result<()> {
        let matches = self
            .grep
            .as_ref()
            .is_none_or(|regex| regex.is_match(&String::from_utf8_lossy(line)));
        self.sinks
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .write_line(line, matches)?;
        Ok(())
    }
}

impl Drop for LineWriter {
    fn drop(&mut self) {
        let _ = self.flush();
    }
}

impl Write for LineWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.pending.extend_from_slice(bytes);
        while let Some(end) = self.pending.iter().position(|byte| *byte == b'\n') {
            let line: Vec<u8> = self.pending.drain(..=end).collect();
            self.emit(&line)?;
        }
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        if !self.pending.is_empty() {
            let line = std::mem::take(&mut self.pending);
            self.emit(&line)?;
        }
        self.sinks
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .flush()
    }
}

/// Initialise logging. `--log` writes a fresh `victory.log` in the current
/// working directory as well as keeping output in the terminal. `--grep`
/// applies a Rust regular expression to each complete rendered log line.
pub fn init(default_level: &str, log: bool, grep: Option<&str>) -> Result<()> {
    let grep = grep.map(Regex::new).transpose().context("invalid --grep regular expression")?;
    let file = if log {
        let path = std::env::current_dir()?.join("victory.log");
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&path)
            .with_context(|| format!("creating log file {}", path.display()))?;
        eprintln!("victory: writing log to {}", path.display());
        Some(file)
    } else {
        None
    };

    let sinks = Arc::new(Mutex::new(Sinks {
        stderr: io::stderr(),
        file,
    }));
    let writer = LineMakeWriter { sinks, grep };
    let filter = std::env::var("VICTORY_LOG")
        .or_else(|_| std::env::var("RUST_LOG"))
        .ok()
        .and_then(|value| EnvFilter::try_new(value).ok())
        .unwrap_or_else(|| EnvFilter::new(default_level));

    tracing_subscriber::registry()
        .with(CAP)
        .with(filter)
        .with(
            tracing_subscriber::fmt::layer()
                .with_target(true)
                .with_ansi(false)
                .with_writer(writer),
        )
        .try_init()
        .map_err(|error| anyhow::anyhow!("initialising logger: {error}"))?;
    Ok(())
}
