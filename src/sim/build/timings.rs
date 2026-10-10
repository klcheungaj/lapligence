//! Opt-in per-phase timings of model builds.
//!
//! When `$LLG_BUILD_TIMINGS` names a file, every model build appends one
//! tab-separated `name=value` line to it: wall time per phase in
//! milliseconds (`generate`, `probe`, `runtime`, `seed`, `configure`,
//! `build`, `total`) and what each cache did (`runtime=hit|built`,
//! `seed=applied|unseeded|existing-tree`, `configure_retry=0|1`), plus
//! `result=ok|error`. Processes running in parallel append to the same file;
//! each line is one write. CI uses it to show where model-build time goes on
//! each platform (`scripts/ci_build_timings.py`). Write failures are ignored:
//! the timings are diagnostics and never change a build's outcome.
//!
//! When `$LLG_CMAKE_PROFILE_DIR` names a directory, every model and runtime
//! configure also writes a CMake `--profiling-format=google-trace` file there
//! (CMake 3.18 or newer), so a slow configure shows which CMake commands took
//! the time. Profiles do not change the configured tree.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Environment variable naming the file the timing lines are appended to.
/// Unset or empty disables recording.
pub const ENV: &str = "LLG_BUILD_TIMINGS";

/// Environment variable naming the directory that receives one CMake
/// configure profile per configure. Unset or empty disables profiling.
pub const PROFILE_DIR_ENV: &str = "LLG_CMAKE_PROFILE_DIR";

/// Distinguishes profiles of configures started by one process.
static PROFILE_SEQUENCE: AtomicUsize = AtomicUsize::new(0);

/// CMake arguments that write a configure profile into `$LLG_CMAKE_PROFILE_DIR`,
/// or nothing when it is unset. The name is unique per process, configure and
/// start time, because parallel test processes share the directory.
pub(super) fn cmake_profile_args() -> Vec<String> {
    let Some(dir) = std::env::var_os(PROFILE_DIR_ENV).filter(|value| !value.is_empty()) else {
        return Vec::new();
    };
    let dir = PathBuf::from(dir);
    // A missing directory would fail the configure; profiling never may.
    if std::fs::create_dir_all(&dir).is_err() {
        return Vec::new();
    }
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos());
    profile_args(
        &dir,
        std::process::id(),
        PROFILE_SEQUENCE.fetch_add(1, Ordering::Relaxed),
        nanos,
    )
}

fn profile_args(dir: &Path, pid: u32, sequence: usize, nanos: u128) -> Vec<String> {
    let file = dir.join(format!("configure-{pid}-{sequence}-{nanos}.json"));
    vec![
        "--profiling-format=google-trace".to_owned(),
        format!("--profiling-output={}", file.display()),
    ]
}

/// Phase durations and cache outcomes of one model build.
#[derive(Debug, Default)]
pub(super) struct BuildTimings {
    phases: Vec<(&'static str, Duration)>,
    notes: Vec<(&'static str, &'static str)>,
}

impl BuildTimings {
    /// Add the time since `started` to `phase`.
    pub(super) fn record(&mut self, phase: &'static str, started: Instant) {
        let elapsed = started.elapsed();
        match self.phases.iter_mut().find(|(name, _)| *name == phase) {
            Some((_, total)) => *total += elapsed,
            None => self.phases.push((phase, elapsed)),
        }
    }

    /// Set the outcome `name=value`, replacing an earlier value.
    pub(super) fn note(&mut self, name: &'static str, value: &'static str) {
        match self.notes.iter_mut().find(|(stored, _)| *stored == name) {
            Some((_, stored)) => *stored = value,
            None => self.notes.push((name, value)),
        }
    }

    /// The record line for a build of `out_dir` that took `total`.
    fn line(&self, out_dir: &Path, total: Duration, ok: bool) -> String {
        let mut line = format!(
            "llg-build\tresult={}\ttotal_ms={}",
            if ok { "ok" } else { "error" },
            total.as_millis()
        );
        for (name, duration) in &self.phases {
            line.push_str(&format!("\t{name}_ms={}", duration.as_millis()));
        }
        for (name, value) in &self.notes {
            line.push_str(&format!("\t{name}={value}"));
        }
        // Tabs and newlines in a path would break the record format.
        let dir = out_dir
            .display()
            .to_string()
            .replace(['\t', '\n', '\r'], " ");
        line.push_str(&format!("\tdir={dir}\n"));
        line
    }

    /// Append the record line to `$LLG_BUILD_TIMINGS` when it is set.
    pub(super) fn finish(&self, out_dir: &Path, started: Instant, ok: bool) {
        let Some(path) = std::env::var_os(ENV).filter(|value| !value.is_empty()) else {
            return;
        };
        let line = self.line(out_dir, started.elapsed(), ok);
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        {
            let _ = file.write_all(line.as_bytes());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_accumulates_phases_and_keeps_the_last_note() {
        let mut timings = BuildTimings::default();
        let started = Instant::now();
        timings.record("configure", started);
        timings.record("build", started);
        timings.record("configure", started);
        timings.note("seed", "unseeded");
        timings.note("seed", "applied");
        let line = timings.line(Path::new("out\tdir"), Duration::from_millis(1234), true);
        assert!(
            line.starts_with("llg-build\tresult=ok\ttotal_ms=1234\t"),
            "{line}"
        );
        assert!(line.ends_with("\tseed=applied\tdir=out dir\n"), "{line}");
        assert_eq!(line.matches("configure_ms=").count(), 1, "{line}");
        assert!(line.contains("\tbuild_ms="), "{line}");
        let failed = BuildTimings::default().line(Path::new("x"), Duration::ZERO, false);
        assert_eq!(failed, "llg-build\tresult=error\ttotal_ms=0\tdir=x\n");
    }

    #[test]
    fn profile_files_are_unique_per_process_and_configure() {
        let dir = Path::new("profiles");
        let args = profile_args(dir, 42, 3, 7);
        assert_eq!(args[0], "--profiling-format=google-trace");
        assert_eq!(
            args[1],
            format!(
                "--profiling-output={}",
                dir.join("configure-42-3-7.json").display()
            )
        );
        assert_ne!(profile_args(dir, 42, 4, 7)[1], args[1]);
        assert_ne!(profile_args(dir, 43, 3, 7)[1], args[1]);
    }
}
