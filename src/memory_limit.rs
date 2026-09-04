//! Process-memory safeguard shared by the LSP and simulator executables.
//!
//! The policy owns environment parsing, warning policy, and the watchdog
//! lifecycle. OS calls and native resource ownership live in
//! [`crate::ffi::process_memory`], so this module stays target-neutral and
//! contains no low-level OS operations.

use std::env;
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::ffi::process_memory::{self, NativeLimitGuard};

const MEMORY_LIMIT_ENV: &str = "LLG_MEMORY_LIMIT_MB";
const MEMORY_WARNING_ENV: &str = "LLG_MEMORY_WARNING_PERCENT";
const MEMORY_POLL_ENV: &str = "LLG_MEMORY_POLL_MS";
const MEMORY_ADDRESS_SPACE_ENV: &str = "LLG_MEMORY_ADDRESS_SPACE_LIMIT";

const MEBIBYTE: u64 = 1024 * 1024;
const DEFAULT_WARNING_PERCENT: u8 = 80;
const DEFAULT_POLL_MS: u64 = 1_000;

/// Log severity used by the shared safeguard's small logging adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
}

impl fmt::Display for LogLevel {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Debug => "debug",
            Self::Info => "info",
            Self::Warn => "warn",
        })
    }
}

/// Callback used for non-emergency safeguard messages.
///
/// The callback receives borrowed formatting arguments, so normal status and
/// warning records do not require the policy to allocate an intermediate
/// string. The watchdog never invokes it on the over-limit termination path.
pub type LogCallback = for<'a> fn(LogLevel, fmt::Arguments<'a>);

/// A simple stderr adapter for binaries that do not have a structured logger.
pub fn stderr_logger(level: LogLevel, message: fmt::Arguments<'_>) {
    if level == LogLevel::Debug {
        return;
    }
    eprintln!("[{level}] {message}");
}

/// Enforcement state reported by the shared safeguard. `Disabled` is
/// intentionally local: the FFI enum describes native-limit outcomes, while
/// this state also needs to represent the absence of a configured watchdog.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SafeguardMode {
    Disabled,
    WatchdogOnly,
    AddressSpaceAndWatchdog,
    WindowsJobAndWatchdog,
    NativeOnly,
    Unavailable,
}

impl std::fmt::Display for SafeguardMode {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Disabled => "disabled",
            Self::WatchdogOnly => "watchdog-only",
            Self::AddressSpaceAndWatchdog => "address-space+watchdog",
            Self::WindowsJobAndWatchdog => "windows-job+watchdog",
            Self::NativeOnly => "native-only",
            Self::Unavailable => "unavailable",
        })
    }
}

/// A parsed memory-safeguard policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MemoryConfig {
    limit_bytes: Option<u64>,
    warning_percent: u8,
    poll_interval: Duration,
    address_space_limit: bool,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            limit_bytes: None,
            warning_percent: DEFAULT_WARNING_PERCENT,
            poll_interval: Duration::from_millis(DEFAULT_POLL_MS),
            address_space_limit: false,
        }
    }
}

/// A fail-soft configuration warning.  Keeping the fields static means
/// parsing never needs to retain environment strings just to report a bad
/// value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ConfigWarning {
    variable: &'static str,
    reason: &'static str,
}

#[derive(Debug)]
struct ParsedConfig {
    config: MemoryConfig,
    warnings: Vec<ConfigWarning>,
}

/// Borrowed environment values used by the pure parser and its tests.
#[derive(Clone, Copy, Debug, Default)]
struct Environment<'a> {
    limit_mb: Option<&'a str>,
    warning_percent: Option<&'a str>,
    poll_ms: Option<&'a str>,
    address_space_limit: Option<&'a str>,
}

/// Parse the four memory environment fields without reading process-global
/// state.  Invalid individual fields use safe defaults; an invalid limit
/// disables the safeguard because there is no safe cap to enforce.
fn parse_values(values: Environment<'_>) -> ParsedConfig {
    let mut warnings = Vec::new();
    let limit_bytes = parse_limit(values.limit_mb, &mut warnings);
    let warning_percent = parse_warning_percent(values.warning_percent, &mut warnings);
    let poll_interval = parse_poll_interval(values.poll_ms, &mut warnings);
    let address_space_limit = parse_address_space_limit(values.address_space_limit, &mut warnings);

    ParsedConfig {
        config: MemoryConfig {
            limit_bytes,
            warning_percent,
            poll_interval,
            address_space_limit,
        },
        warnings,
    }
}

fn parse_environment() -> ParsedConfig {
    let mut read_warnings = Vec::new();
    let limit_mb = read_environment_value(MEMORY_LIMIT_ENV, &mut read_warnings);
    let warning_percent = read_environment_value(MEMORY_WARNING_ENV, &mut read_warnings);
    let poll_ms = read_environment_value(MEMORY_POLL_ENV, &mut read_warnings);
    let address_space_limit = read_environment_value(MEMORY_ADDRESS_SPACE_ENV, &mut read_warnings);

    let mut parsed = parse_values(Environment {
        limit_mb: limit_mb.as_deref(),
        warning_percent: warning_percent.as_deref(),
        poll_ms: poll_ms.as_deref(),
        address_space_limit: address_space_limit.as_deref(),
    });
    parsed.warnings.extend(read_warnings);
    parsed
}

fn read_environment_value(name: &'static str, warnings: &mut Vec<ConfigWarning>) -> Option<String> {
    match env::var(name) {
        Ok(value) => Some(value),
        Err(env::VarError::NotPresent) => None,
        Err(env::VarError::NotUnicode(_)) => {
            warnings.push(ConfigWarning {
                variable: name,
                reason: "must be valid UTF-8",
            });
            None
        }
    }
}

fn parse_limit(value: Option<&str>, warnings: &mut Vec<ConfigWarning>) -> Option<u64> {
    let value = value?;
    let Some(mebibytes) = value.trim().parse::<u64>().ok() else {
        warnings.push(ConfigWarning {
            variable: MEMORY_LIMIT_ENV,
            reason: "must be a positive integer number of MiB",
        });
        return None;
    };
    if mebibytes == 0 {
        warnings.push(ConfigWarning {
            variable: MEMORY_LIMIT_ENV,
            reason: "must be greater than zero",
        });
        return None;
    }
    let Some(bytes) = mebibytes.checked_mul(MEBIBYTE) else {
        warnings.push(ConfigWarning {
            variable: MEMORY_LIMIT_ENV,
            reason: "is too large",
        });
        return None;
    };
    Some(bytes)
}

fn parse_warning_percent(value: Option<&str>, warnings: &mut Vec<ConfigWarning>) -> u8 {
    let Some(value) = value else {
        return DEFAULT_WARNING_PERCENT;
    };
    match value.trim().parse::<u16>() {
        Ok(percent) if percent <= 100 => percent as u8,
        _ => {
            warnings.push(ConfigWarning {
                variable: MEMORY_WARNING_ENV,
                reason: "must be an integer from 0 through 100",
            });
            DEFAULT_WARNING_PERCENT
        }
    }
}

fn parse_poll_interval(value: Option<&str>, warnings: &mut Vec<ConfigWarning>) -> Duration {
    let Some(value) = value else {
        return Duration::from_millis(DEFAULT_POLL_MS);
    };
    match value.trim().parse::<u64>() {
        Ok(milliseconds) if milliseconds > 0 => Duration::from_millis(milliseconds),
        _ => {
            warnings.push(ConfigWarning {
                variable: MEMORY_POLL_ENV,
                reason: "must be a positive integer number of milliseconds",
            });
            Duration::from_millis(DEFAULT_POLL_MS)
        }
    }
}

fn parse_address_space_limit(value: Option<&str>, warnings: &mut Vec<ConfigWarning>) -> bool {
    let Some(value) = value else {
        return false;
    };
    let value = value.trim();
    if value == "1"
        || value.eq_ignore_ascii_case("true")
        || value.eq_ignore_ascii_case("yes")
        || value.eq_ignore_ascii_case("on")
    {
        return true;
    }
    if value == "0"
        || value.eq_ignore_ascii_case("false")
        || value.eq_ignore_ascii_case("no")
        || value.eq_ignore_ascii_case("off")
    {
        return false;
    }
    warnings.push(ConfigWarning {
        variable: MEMORY_ADDRESS_SPACE_ENV,
        reason: "must be true/false, yes/no, on/off, or 1/0",
    });
    false
}

/// Return the warning boundary for a byte limit.
///
/// The result rounds up.  Therefore a warning cannot be emitted while usage
/// is still strictly below the configured percentage, and the calculation is
/// overflow-safe for every `u64` limit.
pub(crate) fn warning_threshold_bytes(limit_bytes: u64, warning_percent: u8) -> u64 {
    let percent = u128::from(warning_percent.min(100));
    let scaled = u128::from(limit_bytes) * percent;
    let threshold = scaled.div_ceil(100);
    threshold.min(u128::from(u64::MAX)) as u64
}

pub(crate) fn warning_threshold_reached(
    physical_bytes: u64,
    limit_bytes: u64,
    warning_percent: u8,
) -> bool {
    physical_bytes >= warning_threshold_bytes(limit_bytes, warning_percent)
}

pub(crate) fn memory_limit_exceeded(physical_bytes: u64, limit_bytes: u64) -> bool {
    physical_bytes > limit_bytes
}

/// The result of installing the optional native enforcement and watchdog.
///
/// The guard is part of the result so callers cannot accidentally discard the
/// native handle or stop signal immediately after startup.  An installation
/// failure is represented as a string because the platform error itself is
/// owned by the lower-level call and is logged at the boundary.
pub struct InstallReport {
    pub guard: MemoryLimitGuard,
    pub mode: SafeguardMode,
    pub error: Option<String>,
}

/// Owns the watchdog thread and any native process limit.
pub struct MemoryLimitGuard {
    native: Option<NativeLimitGuard>,
    watchdog: Option<WatchdogHandle>,
}

impl Drop for MemoryLimitGuard {
    fn drop(&mut self) {
        // Stop and join before restoring RLIMIT_AS.  This makes the guard's
        // lifetime a strict superset of the watchdog's lifetime.
        drop(self.watchdog.take());
        drop(self.native.take());
    }
}

struct WatchdogHandle {
    stop: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
}

impl Drop for WatchdogHandle {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(join) = self.join.take() {
            // Wake a thread parked for its polling interval so dropping the
            // application guard never waits for a full interval.
            join.thread().unpark();
            let _ = join.join();
        }
    }
}

/// Install the configured safeguard with a stderr logger.
pub fn install() -> InstallReport {
    install_with_logger(stderr_logger)
}

/// Install the configured safeguard using the caller's non-emergency logger.
///
/// Every failure is fail-soft: malformed configuration disables only the
/// affected option, native-limit failures retain the portable watchdog, and a
/// thread-spawn failure leaves any native limit in place while reporting the
/// issue through the callback.
pub fn install_with_logger(logger: LogCallback) -> InstallReport {
    let parsed = parse_environment();
    for warning in &parsed.warnings {
        logger(
            LogLevel::Warn,
            format_args!(
                "memory safeguard: {} {}; using a safe fallback",
                warning.variable, warning.reason
            ),
        );
    }
    let report = install_config_with_logger(parsed.config, logger);
    logger(
        LogLevel::Debug,
        format_args!(
            "memory safeguard: installation result mode={} error_present={}",
            report.mode,
            report.error.is_some(),
        ),
    );
    report
}

#[cfg(test)]
fn install_config(config: MemoryConfig) -> InstallReport {
    install_config_with_logger(config, stderr_logger)
}

fn install_config_with_logger(config: MemoryConfig, logger: LogCallback) -> InstallReport {
    let Some(limit_bytes) = config.limit_bytes else {
        return InstallReport {
            guard: MemoryLimitGuard {
                native: None,
                watchdog: None,
            },
            mode: SafeguardMode::Disabled,
            error: None,
        };
    };

    let (native, native_mode, native_error) = match process_memory::install_native_limit(
        limit_bytes,
        config.address_space_limit,
    ) {
        Ok(guard) => {
            let mode = safeguard_mode(guard.mode());
            (Some(guard), mode, None)
        }
        Err(error) => {
            let message = error.to_string();
            logger(
                LogLevel::Warn,
                format_args!(
                    "memory safeguard: native enforcement unavailable: {message}; continuing with watchdog-only enforcement"
                ),
            );
            (None, SafeguardMode::WatchdogOnly, Some(message))
        }
    };

    let stop = Arc::new(AtomicBool::new(false));
    let watchdog_config = config;
    let watchdog_stop = Arc::clone(&stop);
    let watchdog_logger = logger;
    let thread = thread::Builder::new()
        .name("llg-memory-watchdog".to_owned())
        .spawn(move || watchdog_loop_with_logger(watchdog_config, watchdog_stop, watchdog_logger));

    let (watchdog, watchdog_error) = match thread {
        Ok(join) => (
            Some(WatchdogHandle {
                stop,
                join: Some(join),
            }),
            None,
        ),
        Err(error) => {
            logger(
                LogLevel::Warn,
                format_args!(
                    "memory safeguard: watchdog thread unavailable: {error}; native enforcement, if installed, remains active"
                ),
            );
            (None, Some(error.to_string()))
        }
    };

    // `install_native_limit` also returns a no-op guard for the normal Unix
    // watchdog-only configuration.  Use its mode, rather than guard
    // presence, to distinguish real native enforcement from that placeholder.
    let native_is_effective = !matches!(native_mode, SafeguardMode::WatchdogOnly);
    let mode = match (native_is_effective, watchdog.is_some()) {
        (true, true) => native_mode,
        (false, true) => SafeguardMode::WatchdogOnly,
        (true, false) => SafeguardMode::NativeOnly,
        (false, false) => SafeguardMode::Unavailable,
    };
    let error = native_error.or(watchdog_error);

    logger(
        LogLevel::Info,
        format_args!(
            "memory safeguard: enabled limit_bytes={} warning_percent={} poll_ms={} address_space_limit={} mode={mode}",
            limit_bytes,
            config.warning_percent,
            config.poll_interval.as_millis(),
            config.address_space_limit,
        ),
    );

    InstallReport {
        guard: MemoryLimitGuard { native, watchdog },
        mode,
        error,
    }
}

/// Return the current physical-memory sample for the process, when the
/// platform backend can measure it. This is useful to an optional structured
/// logger without making the logger a dependency of the policy.
pub fn current_physical_bytes() -> Option<u64> {
    process_memory::current_usage()
        .ok()
        .map(|usage| usage.physical_bytes)
}

fn safeguard_mode(mode: process_memory::EnforcementMode) -> SafeguardMode {
    match mode {
        process_memory::EnforcementMode::WatchdogOnly => SafeguardMode::WatchdogOnly,
        process_memory::EnforcementMode::AddressSpaceAndWatchdog => {
            SafeguardMode::AddressSpaceAndWatchdog
        }
        process_memory::EnforcementMode::WindowsJobAndWatchdog => {
            SafeguardMode::WindowsJobAndWatchdog
        }
    }
}

#[cfg(test)]
fn watchdog_loop(config: MemoryConfig, stop: Arc<AtomicBool>) {
    watchdog_loop_with_logger(config, stop, stderr_logger);
}

fn watchdog_loop_with_logger(config: MemoryConfig, stop: Arc<AtomicBool>, logger: LogCallback) {
    let Some(limit_bytes) = config.limit_bytes else {
        return;
    };
    let warning_bytes = warning_threshold_bytes(limit_bytes, config.warning_percent);
    let mut warning_reported = false;
    let mut measurement_error_reported = false;

    while !stop.load(Ordering::Acquire) {
        match process_memory::current_usage() {
            Ok(usage) => {
                // This branch must stay allocation-free and lock-free.  In
                // particular, do not call the logger before the emergency
                // termination: formatting/logging can require more memory.
                if memory_limit_exceeded(usage.physical_bytes, limit_bytes) {
                    process_memory::write_emergency_stderr(
                        b"llg: process memory limit exceeded; terminating\n",
                    );
                    process_memory::terminate_immediately(1);
                }

                if !warning_reported
                    && warning_threshold_reached(
                        usage.physical_bytes,
                        limit_bytes,
                        config.warning_percent,
                    )
                {
                    logger(
                        LogLevel::Warn,
                        format_args!(
                            "memory safeguard: physical usage {} bytes reached {}% of limit {} bytes",
                            usage.physical_bytes,
                            config.warning_percent,
                            limit_bytes,
                        ),
                    );
                    warning_reported = true;
                } else if warning_reported && usage.physical_bytes < warning_bytes {
                    warning_reported = false;
                }
                measurement_error_reported = false;
            }
            Err(error) if !measurement_error_reported => {
                logger(
                    LogLevel::Warn,
                    format_args!("memory safeguard: process measurement failed: {error}"),
                );
                measurement_error_reported = true;
            }
            Err(_) => {}
        }

        thread::park_timeout(config.poll_interval);
    }
}

#[cfg(test)]
mod tests {
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    use std::process::{Command, Stdio};
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    use std::sync::atomic::AtomicBool;
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    use std::sync::Arc;
    use std::time::Duration;
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    use std::time::Instant;

    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    use super::watchdog_loop;
    use super::{
        install_config, memory_limit_exceeded, parse_values, warning_threshold_bytes,
        warning_threshold_reached, Environment, MemoryConfig, DEFAULT_POLL_MS,
        DEFAULT_WARNING_PERCENT, MEBIBYTE,
    };

    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    const WATCHDOG_CHILD_ENV: &str = "LLG_MEMORY_WATCHDOG_TEST_CHILD";
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    const WATCHDOG_CHILD_FILTER: &str =
        "memory_limit::tests::watchdog_loop_terminates_when_usage_exceeds_limit";
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    const WATCHDOG_TIMEOUT: Duration = Duration::from_secs(5);
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    const EMERGENCY_TEXT: &str = "llg: process memory limit exceeded; terminating";

    static CALLBACK_COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

    fn count_callback(_: super::LogLevel, _: std::fmt::Arguments<'_>) {
        CALLBACK_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    #[test]
    fn missing_limit_disables_the_safeguard() {
        let parsed = parse_values(Environment::default());
        assert_eq!(parsed.config.limit_bytes, None);
        assert_eq!(parsed.config.warning_percent, DEFAULT_WARNING_PERCENT);
        assert_eq!(
            parsed.config.poll_interval.as_millis(),
            DEFAULT_POLL_MS as u128
        );
        assert!(!parsed.config.address_space_limit);
        assert!(parsed.warnings.is_empty());

        let report = install_config(MemoryConfig::default());
        assert_eq!(report.mode, super::SafeguardMode::Disabled);
        assert!(report.error.is_none());
        drop(report);
    }

    #[test]
    fn custom_logger_receives_installation_status() {
        CALLBACK_COUNT.store(0, std::sync::atomic::Ordering::Relaxed);
        let report = super::install_config_with_logger(
            MemoryConfig {
                limit_bytes: Some(u64::MAX),
                warning_percent: 100,
                poll_interval: Duration::from_secs(3_600),
                address_space_limit: false,
            },
            count_callback,
        );

        assert_ne!(report.mode, super::SafeguardMode::Disabled);
        assert!(
            CALLBACK_COUNT.load(std::sync::atomic::Ordering::Relaxed) > 0,
            "installation status should use the supplied callback"
        );
        drop(report);
    }

    #[test]
    fn valid_environment_values_are_converted_without_loss() {
        let parsed = parse_values(Environment {
            limit_mb: Some("128"),
            warning_percent: Some("75"),
            poll_ms: Some("25"),
            address_space_limit: Some("yes"),
        });
        assert_eq!(parsed.config.limit_bytes, Some(128 * MEBIBYTE));
        assert_eq!(parsed.config.warning_percent, 75);
        assert_eq!(parsed.config.poll_interval.as_millis(), 25);
        assert!(parsed.config.address_space_limit);
        assert!(parsed.warnings.is_empty());
    }

    #[test]
    fn invalid_values_warn_and_fall_back_without_panicking() {
        let parsed = parse_values(Environment {
            limit_mb: Some("0"),
            warning_percent: Some("101"),
            poll_ms: Some("not-a-duration"),
            address_space_limit: Some("maybe"),
        });
        assert_eq!(parsed.config.limit_bytes, None);
        assert_eq!(parsed.config.warning_percent, DEFAULT_WARNING_PERCENT);
        assert_eq!(
            parsed.config.poll_interval.as_millis(),
            DEFAULT_POLL_MS as u128
        );
        assert!(!parsed.config.address_space_limit);
        assert_eq!(parsed.warnings.len(), 4);
    }

    #[test]
    fn invalid_optional_values_preserve_a_valid_memory_cap() {
        let parsed = parse_values(Environment {
            limit_mb: Some("64"),
            warning_percent: Some("101"),
            poll_ms: Some("0"),
            address_space_limit: Some("maybe"),
        });
        assert_eq!(parsed.config.limit_bytes, Some(64 * MEBIBYTE));
        assert_eq!(parsed.config.warning_percent, DEFAULT_WARNING_PERCENT);
        assert_eq!(
            parsed.config.poll_interval.as_millis(),
            DEFAULT_POLL_MS as u128
        );
        assert!(!parsed.config.address_space_limit);
        assert_eq!(parsed.warnings.len(), 3);
    }

    #[test]
    fn watchdog_guard_stops_without_waiting_for_a_long_poll_interval() {
        let report = install_config(MemoryConfig {
            limit_bytes: Some(u64::MAX),
            warning_percent: 100,
            poll_interval: Duration::from_secs(3_600),
            address_space_limit: false,
        });
        assert_ne!(report.mode, super::SafeguardMode::Disabled);
        drop(report);
    }

    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    #[test]
    fn watchdog_loop_terminates_when_usage_exceeds_limit() {
        if std::env::var_os(WATCHDOG_CHILD_ENV).is_some() {
            watchdog_loop(
                MemoryConfig {
                    limit_bytes: Some(1),
                    warning_percent: 100,
                    poll_interval: Duration::from_millis(1),
                    address_space_limit: false,
                },
                Arc::new(AtomicBool::new(false)),
            );
            return;
        }

        let mut child = Command::new(std::env::current_exe().expect("locate test executable"))
            .args(["--exact", WATCHDOG_CHILD_FILTER])
            .env(WATCHDOG_CHILD_ENV, "1")
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn watchdog child test");
        let deadline = Instant::now() + WATCHDOG_TIMEOUT;
        loop {
            match child.try_wait().expect("poll watchdog child") {
                Some(_) => break,
                None if Instant::now() >= deadline => {
                    let _ = child.kill();
                    let _ = child.wait();
                    panic!("watchdog child did not terminate within {WATCHDOG_TIMEOUT:?}");
                }
                None => std::thread::sleep(Duration::from_millis(10)),
            }
        }

        let output = child
            .wait_with_output()
            .expect("collect watchdog child output");
        assert_eq!(
            output.status.code(),
            Some(1),
            "watchdog child should exit with status 1; stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(EMERGENCY_TEXT),
            "watchdog child stderr should contain {EMERGENCY_TEXT:?}; stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn warning_threshold_is_overflow_safe_and_rounds_up() {
        assert_eq!(warning_threshold_bytes(1_000, 80), 800);
        assert_eq!(warning_threshold_bytes(1, 80), 1);
        assert_eq!(warning_threshold_bytes(u64::MAX, 100), u64::MAX);
        assert_eq!(warning_threshold_bytes(100, 255), 100);
        assert!(warning_threshold_reached(800, 1_000, 80));
        assert!(!warning_threshold_reached(799, 1_000, 80));
    }

    #[test]
    fn limit_boundary_is_not_exceeded_until_usage_is_above_it() {
        assert!(!memory_limit_exceeded(100, 100));
        assert!(memory_limit_exceeded(101, 100));
    }
}
