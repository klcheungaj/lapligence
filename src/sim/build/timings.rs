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

use std::io::Write;
use std::path::Path;
use std::time::{Duration, Instant};

/// Environment variable naming the file the timing lines are appended to.
/// Unset or empty disables recording.
pub const ENV: &str = "LLG_BUILD_TIMINGS";

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
}
