//! Small, stdio-safe logger for the LSP binary.
//!
//! `LLG_LOG` accepts `off`, `error`, `warn`, `info`, `debug`, and `trace`;
//! the default is `warn`.  `LLG_LOG_FILE` selects an append-only destination;
//! invalid or unwritable paths fall back to stderr.  The logger never writes
//! to stdout because stdout carries LSP frames.
//!
//! Note: `LLG_LOG_FILE` is shared across processes that point it at the same
//! path; concurrent servers append interleaved lines.  Use a distinct path per
//! process when that matters.
#![allow(dead_code)]

use std::env;
use std::ffi::OsStr;
use std::fs::{File, OpenOptions};
use std::io::{self, BufWriter, Write};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

const LOG_LEVEL_ENV: &str = "LLG_LOG";
const LOG_FILE_ENV: &str = "LLG_LOG_FILE";
const DEFAULT_LOG_LEVEL: Level = Level::Warn;

static LOGGER: OnceLock<Logger> = OnceLock::new();

/// Installs the process-wide logger using the current environment.
pub(crate) fn init() {
    LOGGER.get_or_init(Logger::from_environment);
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum Level {
    Off,
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

struct Logger {
    level: Level,
    sink: Mutex<Sink>,
}

impl Logger {
    fn from_environment() -> Self {
        let level = env::var(LOG_LEVEL_ENV)
            .ok()
            .and_then(|value| parse_level(&value))
            .unwrap_or(DEFAULT_LOG_LEVEL);
        let sink = if level == Level::Off {
            Sink::stderr()
        } else {
            sink_from_path(log_path(env::var_os(LOG_FILE_ENV).as_deref()))
        };

        Self {
            level,
            sink: Mutex::new(sink),
        }
    }
}

pub(crate) fn enabled(level: Level) -> bool {
    LOGGER
        .get_or_init(Logger::from_environment)
        .level
        .allows(level)
}

pub(crate) fn write(level: Level, message: std::fmt::Arguments<'_>) {
    let logger = LOGGER.get_or_init(Logger::from_environment);
    if !logger.level.allows(level) {
        return;
    }

    let Ok(mut sink) = logger.sink.lock() else {
        return;
    };
    let _ = writeln!(sink, "[{}] {}", level, message);
    let _ = sink.flush();
}

impl Level {
    fn allows(self, message: Self) -> bool {
        message != Self::Off && self >= message
    }
}

impl std::fmt::Display for Level {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Self::Off => "OFF",
            Self::Error => "ERROR",
            Self::Warn => "WARN",
            Self::Info => "INFO",
            Self::Debug => "DEBUG",
            Self::Trace => "TRACE",
        };
        formatter.write_str(name)
    }
}

enum Sink {
    Stderr(io::Stderr),
    File(BufWriter<File>),
}

impl Sink {
    fn stderr() -> Self {
        Self::Stderr(io::stderr())
    }
}

impl Write for Sink {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        match self {
            Self::Stderr(stderr) => stderr.write(bytes),
            Self::File(file) => file.write(bytes),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            Self::Stderr(stderr) => stderr.flush(),
            Self::File(file) => file.flush(),
        }
    }
}

fn parse_level(value: &str) -> Option<Level> {
    let value = value.trim();
    if value.eq_ignore_ascii_case("off") {
        Some(Level::Off)
    } else if value.eq_ignore_ascii_case("error") {
        Some(Level::Error)
    } else if value.eq_ignore_ascii_case("warn") || value.eq_ignore_ascii_case("warning") {
        Some(Level::Warn)
    } else if value.eq_ignore_ascii_case("info") {
        Some(Level::Info)
    } else if value.eq_ignore_ascii_case("debug") {
        Some(Level::Debug)
    } else if value.eq_ignore_ascii_case("trace") {
        Some(Level::Trace)
    } else {
        None
    }
}

fn log_path(value: Option<&OsStr>) -> Option<PathBuf> {
    let value = value?;
    if value.to_string_lossy().trim().is_empty() {
        None
    } else {
        Some(PathBuf::from(value))
    }
}

fn sink_from_path(path: Option<PathBuf>) -> Sink {
    let Some(path) = path else {
        return Sink::stderr();
    };

    let file = OpenOptions::new().create(true).append(true).open(path);
    match file {
        Ok(file) => Sink::File(BufWriter::new(file)),
        Err(_) => Sink::stderr(),
    }
}

#[macro_export]
macro_rules! llg_trace {
    ($($arg:tt)+) => {
        if $crate::logging::enabled($crate::logging::Level::Trace) {
            $crate::logging::write(
                $crate::logging::Level::Trace,
                format_args!($($arg)+),
            );
        }
    };
}

#[macro_export]
macro_rules! llg_debug {
    ($($arg:tt)+) => {
        if $crate::logging::enabled($crate::logging::Level::Debug) {
            $crate::logging::write(
                $crate::logging::Level::Debug,
                format_args!($($arg)+),
            );
        }
    };
}

#[macro_export]
macro_rules! llg_info {
    ($($arg:tt)+) => {
        if $crate::logging::enabled($crate::logging::Level::Info) {
            $crate::logging::write(
                $crate::logging::Level::Info,
                format_args!($($arg)+),
            );
        }
    };
}

#[macro_export]
macro_rules! llg_warn {
    ($($arg:tt)+) => {
        if $crate::logging::enabled($crate::logging::Level::Warn) {
            $crate::logging::write(
                $crate::logging::Level::Warn,
                format_args!($($arg)+),
            );
        }
    };
}

#[macro_export]
macro_rules! llg_error {
    ($($arg:tt)+) => {
        if $crate::logging::enabled($crate::logging::Level::Error) {
            $crate::logging::write(
                $crate::logging::Level::Error,
                format_args!($($arg)+),
            );
        }
    };
}

#[macro_export]
macro_rules! llg_log {
    (trace, $($arg:tt)+) => {
        $crate::llg_trace!($($arg)+);
    };
    (debug, $($arg:tt)+) => {
        $crate::llg_debug!($($arg)+);
    };
    (info, $($arg:tt)+) => {
        $crate::llg_info!($($arg)+);
    };
    (warn, $($arg:tt)+) => {
        $crate::llg_warn!($($arg)+);
    };
    (error, $($arg:tt)+) => {
        $crate::llg_error!($($arg)+);
    };
}

#[cfg(test)]
mod tests {
    use super::{log_path, parse_level};
    use std::ffi::OsStr;
    use std::path::PathBuf;

    #[test]
    fn parses_supported_levels_and_aliases() {
        assert_eq!(parse_level("OFF"), Some(super::Level::Off));
        assert_eq!(parse_level("error"), Some(super::Level::Error));
        assert_eq!(parse_level("warning"), Some(super::Level::Warn));
        assert_eq!(parse_level("Info"), Some(super::Level::Info));
        assert_eq!(parse_level("debug"), Some(super::Level::Debug));
        assert_eq!(parse_level("trace"), Some(super::Level::Trace));
        assert_eq!(parse_level("verbose"), None);
    }

    #[test]
    fn ignores_empty_log_paths() {
        assert_eq!(log_path(None), None);
        assert_eq!(log_path(Some(OsStr::new(""))), None);
        assert_eq!(log_path(Some(OsStr::new(" \t"))), None);
    }

    #[test]
    fn preserves_configured_log_paths() {
        assert_eq!(
            log_path(Some(OsStr::new("target/llg.log"))),
            Some(PathBuf::from("target/llg.log"))
        );
    }
}
