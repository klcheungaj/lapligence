//! Opt-in pipeline timing markers for external process-memory sampling.

use std::sync::OnceLock;
use std::time::Instant;

/// A diagnostic stage, enabled only by `LLG_PROFILE_STAGES=1`.
///
/// Markers go to stderr and do not change exported semantic or model data.
pub struct Stage {
    name: &'static str,
    started: Option<Instant>,
}

impl Stage {
    /// Start a stage; dropping the guard emits its elapsed wall time.
    pub fn new(name: &'static str) -> Self {
        static ENABLED: OnceLock<bool> = OnceLock::new();
        let enabled = *ENABLED.get_or_init(|| {
            std::env::var_os("LLG_PROFILE_STAGES").is_some_and(|value| value == "1")
        });
        let started = enabled.then(|| {
            eprintln!("llg-profile begin {name} pid={}", std::process::id());
            Instant::now()
        });
        Self { name, started }
    }
}

impl Drop for Stage {
    fn drop(&mut self) {
        if let Some(started) = self.started {
            eprintln!(
                "llg-profile end {} seconds={:.6}",
                self.name,
                started.elapsed().as_secs_f64()
            );
        }
    }
}
