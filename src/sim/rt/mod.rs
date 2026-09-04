//! rt — the C11 simulation runtime embedded as strings.
//!
//! [`runtime_sources`] returns the 4-state value model and event scheduler
//! (`llg_rt.h` / `llg_rt.c`), [`libaco_sources`] the vendored coroutine
//! library (`aco.h` / `aco.c` / `acosw.S`), [`waveform_sources`] the optional
//! asynchronous VCD/FST writer and vendored libfst sources, and
//! [`selftest_source`] the runtime's C self-test.  The driver and integration
//! tests write these into a build directory and build them together with the
//! generated model through CMake (`sim::build`) — the runtime is deliberately
//! *not* linked into the Rust binaries.
//!
//! See `llg_rt.h` (embedded below) for the value semantics and the scheduler
//! algorithm; correctness of the 4-state math mirrors `core::elab`.

/// (header, implementation) of the simulation runtime.
pub fn runtime_sources() -> (&'static str, &'static str) {
    (include_str!("llg_rt.h"), include_str!("llg_rt.c"))
}

/// (aco.h, aco.c, acosw.S) from vendor/libaco.
pub fn libaco_sources() -> (&'static str, &'static str, &'static str) {
    (
        include_str!("../../../vendor/libaco/aco.h"),
        include_str!("../../../vendor/libaco/aco.c"),
        include_str!("../../../vendor/libaco/acosw.S"),
    )
}

/// Optional waveform runtime and the official GTKWave libfst writer snapshot.
/// These are written and compiled only for models containing
/// `#define LLG_WAVEFORM 1`.
pub fn waveform_sources() -> &'static [(&'static str, &'static str)] {
    &[
        ("llg_wave.h", include_str!("llg_wave.h")),
        ("llg_wave.c", include_str!("llg_wave.c")),
        ("fstapi.c", include_str!("gtkwave/fstapi.c")),
        ("fstapi.h", include_str!("gtkwave/fstapi.h")),
        ("fastlz.c", include_str!("gtkwave/fastlz.c")),
        ("fastlz.h", include_str!("gtkwave/fastlz.h")),
        ("lz4.c", include_str!("gtkwave/lz4.c")),
        ("lz4.h", include_str!("gtkwave/lz4.h")),
        ("fst_config.h", include_str!("gtkwave/fst_config.h")),
        ("fst_win_unistd.h", include_str!("gtkwave/fst_win_unistd.h")),
        ("wavealloca.h", include_str!("gtkwave/wavealloca.h")),
    ]
}

/// Write optional waveform sources into a generated model directory.
pub(crate) fn write_waveform_sources(out_dir: &std::path::Path) -> Result<(), String> {
    for (name, content) in waveform_sources() {
        let path = out_dir.join(name);
        std::fs::write(&path, content).map_err(|e| format!("write {}: {e}", path.display()))?;
    }
    Ok(())
}

/// The runtime's C self-test: sv4 value vectors (mirroring `core::elab` unit
/// tests) plus scheduler checks (delay ordering, NBA visibility, ping-pong).
pub fn selftest_source() -> &'static str {
    include_str!("llg_rt_selftest.c")
}

/// Standalone C self-test for the waveform queue and VCD/FST writers.
pub fn waveform_selftest_source() -> &'static str {
    include_str!("llg_wave_selftest.c")
}
