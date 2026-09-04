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
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

const LOG_LEVEL_ENV: &str = "LLG_LOG";
const LOG_FILE_ENV: &str = "LLG_LOG_FILE";
const DEFAULT_LOG_LEVEL: Level = Level::Warn;
const BOUNDED_FIELD_MAX_BYTES: usize = 128;

static LOGGER: OnceLock<Logger> = OnceLock::new();
static NEXT_CORRELATION_ID: AtomicU64 = AtomicU64::new(1);
static MEMORY_SAMPLER: OnceLock<fn() -> Option<u64>> = OnceLock::new();

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

/// Bound a value before it is included in a lifecycle record.  Transport
/// metadata can originate in a client request, so it must not be allowed to
/// turn debug logging into another unbounded allocation.
pub(crate) fn bounded_field(value: &str) -> String {
    let mut result = String::new();
    let mut truncated = false;
    for character in value.chars() {
        let escaped = match character {
            '\n' => "\\n".to_owned(),
            '\r' => "\\r".to_owned(),
            '\t' => "\\t".to_owned(),
            character if character.is_control() => "?".to_owned(),
            character => character.to_string(),
        };
        if result.len().saturating_add(escaped.len()) > BOUNDED_FIELD_MAX_BYTES {
            truncated = true;
            break;
        }
        result.push_str(&escaped);
    }
    if truncated {
        while result.len().saturating_add(3) > BOUNDED_FIELD_MAX_BYTES {
            result.pop();
        }
        result.push_str("...");
    }
    result
}

/// Register a cheap, non-blocking current-process memory sampler.
///
/// The memory-limit layer may call this once during startup. Logging remains
/// fully functional when no sampler is installed, which keeps this module
/// independent of platform-specific memory code.
pub(crate) fn set_memory_sampler(sampler: fn() -> Option<u64>) -> bool {
    MEMORY_SAMPLER.set(sampler).is_ok()
}

fn sampled_memory_bytes() -> Option<u64> {
    MEMORY_SAMPLER.get().and_then(|sampler| sampler())
}

/// Return a process-unique, monotonically increasing lifecycle identifier.
pub(crate) fn next_correlation_id() -> u64 {
    NEXT_CORRELATION_ID.fetch_add(1, Ordering::Relaxed)
}

/// A low-overhead request, notification, job, or pipeline phase span.
///
/// Disabled spans retain no identity string and perform no formatting. Active
/// spans always emit their completion record from `Drop`, including unwind
/// and early-return paths.
pub(crate) struct LifecycleSpan {
    active: bool,
    level: Level,
    id: u64,
    parent_id: Option<u64>,
    kind: &'static str,
    name: &'static str,
    identity: Option<String>,
    root: Option<String>,
    generation: Option<u64>,
    files: Option<usize>,
    started: Instant,
    outcome: &'static str,
    cardinality: Option<usize>,
    #[cfg(test)]
    drop_probe: Option<std::sync::Arc<std::sync::atomic::AtomicUsize>>,
}

impl LifecycleSpan {
    pub(crate) fn request(name: &'static str, identity: impl FnOnce() -> String) -> Self {
        Self::start(
            Level::Info,
            "request",
            name,
            Some(identity),
            None::<fn() -> String>,
            None,
            None,
            None,
        )
    }

    pub(crate) fn notification(name: &'static str, identity: impl FnOnce() -> String) -> Self {
        Self::start(
            Level::Info,
            "notification",
            name,
            Some(identity),
            None::<fn() -> String>,
            None,
            None,
            None,
        )
    }

    pub(crate) fn analysis(
        name: &'static str,
        root: impl FnOnce() -> String,
        generation: u64,
        files: usize,
    ) -> Self {
        Self::analysis_with_parent(name, root, generation, files, None)
    }

    pub(crate) fn analysis_with_parent(
        name: &'static str,
        root: impl FnOnce() -> String,
        generation: u64,
        files: usize,
        parent_id: Option<u64>,
    ) -> Self {
        Self::start(
            Level::Info,
            "analysis",
            name,
            None::<fn() -> String>,
            Some(root),
            Some(generation),
            Some(files),
            parent_id,
        )
    }

    pub(crate) fn phase(
        name: &'static str,
        root: impl FnOnce() -> String,
        generation: u64,
        files: usize,
    ) -> Self {
        Self::phase_with_parent(name, root, generation, files, None)
    }

    pub(crate) fn phase_with_parent(
        name: &'static str,
        root: impl FnOnce() -> String,
        generation: u64,
        files: usize,
        parent_id: Option<u64>,
    ) -> Self {
        Self::start(
            Level::Debug,
            "phase",
            name,
            None::<fn() -> String>,
            Some(root),
            Some(generation),
            Some(files),
            parent_id,
        )
    }

    // One internal constructor centralizes all optional lifecycle dimensions;
    // public helpers supply the meaningful subsets and avoid formatting work.
    #[allow(clippy::too_many_arguments)]
    fn start<I, R>(
        level: Level,
        kind: &'static str,
        name: &'static str,
        identity: Option<I>,
        root: Option<R>,
        generation: Option<u64>,
        files: Option<usize>,
        parent_id: Option<u64>,
    ) -> Self
    where
        I: FnOnce() -> String,
        R: FnOnce() -> String,
    {
        let active = enabled(level);
        let id = next_correlation_id();
        let identity = active.then(|| identity.map(|make| make())).flatten();
        let root = active.then(|| root.map(|make| make())).flatten();
        if active {
            write(
                level,
                format_args!(
                    "lifecycle=start kind={kind} name={name} method={name} feature={name} id={id} parent_id={} identity={} document={} root={} generation={} file_count={} files={} response_size=- memory_bytes={}",
                    OptionalU64(parent_id),
                    identity.as_deref().unwrap_or("-"),
                    identity.as_deref().unwrap_or("-"),
                    root.as_deref().unwrap_or("-"),
                    OptionalU64(generation),
                    OptionalUsize(files),
                    OptionalUsize(files),
                    OptionalU64(sampled_memory_bytes()),
                ),
            );
        }
        Self {
            active,
            level,
            id,
            parent_id,
            kind,
            name,
            identity,
            root,
            generation,
            files,
            started: Instant::now(),
            outcome: "early-return",
            cardinality: None,
            #[cfg(test)]
            drop_probe: None,
        }
    }

    pub(crate) fn id(&self) -> u64 {
        self.id
    }

    pub(crate) fn outcome(&mut self, outcome: &'static str) {
        self.outcome = outcome;
    }

    pub(crate) fn cardinality(&mut self, cardinality: usize) {
        self.cardinality = Some(cardinality);
    }

    /// Attach a root identity after a request has resolved its document.  The
    /// closure is deliberately lazy so disabled logging does not format paths.
    pub(crate) fn set_root(&mut self, root: impl FnOnce() -> String) {
        if self.active {
            self.root = Some(root());
        }
    }

    pub(crate) fn complete(&mut self, outcome: &'static str, cardinality: usize) {
        self.outcome(outcome);
        self.cardinality(cardinality);
    }

    #[cfg(test)]
    fn with_drop_probe(mut self, probe: std::sync::Arc<std::sync::atomic::AtomicUsize>) -> Self {
        self.drop_probe = Some(probe);
        self
    }
}

impl Drop for LifecycleSpan {
    fn drop(&mut self) {
        #[cfg(test)]
        if let Some(probe) = &self.drop_probe {
            probe.fetch_add(1, Ordering::Relaxed);
        }
        if !self.active {
            return;
        }
        write(
            self.level,
            format_args!(
                "lifecycle=end kind={} name={} method={} feature={} id={} parent_id={} identity={} document={} root={} generation={} file_count={} files={} outcome={} elapsed_us={} elapsed_ms={} cardinality={} response_size={} memory_bytes={}",
                self.kind,
                self.name,
                self.name,
                self.name,
                self.id,
                OptionalU64(self.parent_id),
                self.identity.as_deref().unwrap_or("-"),
                self.identity.as_deref().unwrap_or("-"),
                self.root.as_deref().unwrap_or("-"),
                OptionalU64(self.generation),
                OptionalUsize(self.files),
                OptionalUsize(self.files),
                self.outcome,
                self.started.elapsed().as_micros(),
                self.started.elapsed().as_millis(),
                OptionalUsize(self.cardinality),
                OptionalUsize(self.cardinality),
                OptionalU64(sampled_memory_bytes()),
            ),
        );
    }
}

struct OptionalU64(Option<u64>);

impl std::fmt::Display for OptionalU64 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            Some(value) => value.fmt(formatter),
            None => formatter.write_str("-"),
        }
    }
}

struct OptionalUsize(Option<usize>);

impl std::fmt::Display for OptionalUsize {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            Some(value) => value.fmt(formatter),
            None => formatter.write_str("-"),
        }
    }
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
    use super::{log_path, next_correlation_id, parse_level, LifecycleSpan};
    use std::ffi::OsStr;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

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

    #[test]
    fn lifecycle_ids_are_monotonic() {
        let first = next_correlation_id();
        let second = next_correlation_id();
        assert!(second > first);
    }

    #[test]
    fn lifecycle_span_drop_completes_early_return_paths() {
        let probe = Arc::new(AtomicUsize::new(0));
        {
            let span = LifecycleSpan::request("test/request", || "document".to_owned())
                .with_drop_probe(Arc::clone(&probe));
            drop(span);
        }
        assert_eq!(probe.load(Ordering::Relaxed), 1);
    }
}
