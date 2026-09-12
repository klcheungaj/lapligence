//! rt — the C11 simulation runtime embedded as strings.
//!
//! [`value_sources`] returns the scheduler-independent value types, operations,
//! and conversions (`llg_value.h` / `llg_value.c`), [`random_sources`] the
//! scheduler-independent legacy probabilistic functions (`llg_random.h` /
//! `llg_random.c`), [`rng_sources`] the
//! scheduler-independent random-stream service (`llg_rng.h` / `llg_rng.c`),
//! [`container_sources`] and [`string_sources`] the dynamically sized value
//! stores, [`runtime_sources`] the event scheduler (`llg_rt.h` / `llg_rt.c`),
//! [`libaco_sources`] the vendored coroutine
//! library (`aco.h` / `aco.c` / `acosw.S`), [`waveform_sources`] the optional
//! asynchronous VCD/FST writer and vendored libfst sources, and
//! [`selftest_source`] the runtime's C self-test.  The driver and integration
//! tests write these into a build directory and build them together with the
//! generated model through CMake (`sim::build`) — the runtime is deliberately
//! *not* linked into the Rust binaries.
//!
//! See `llg_value.h` for value semantics and `llg_rt.h` for the scheduler API;
//! correctness of the 4-state math mirrors `core::elab`.

/// Scheduler-independent deterministic random-stream service.  It owns the
/// PCG stream, hierarchy derivation, unbiased ranges, and state serialization.
pub fn rng_sources() -> (&'static str, &'static str) {
    (include_str!("llg_rng.h"), include_str!("llg_rng.c"))
}

/// (header, implementation) of the event scheduler and runtime facade.
/// Compile together with [`value_sources`] and [`libaco_sources`].
pub fn runtime_sources() -> (&'static str, &'static str) {
    (include_str!("llg_rt.h"), include_str!("llg_rt.c"))
}

/// (header, implementation) of scheduler-independent value operations and casts.
/// This C11 module can be compiled independently, linking only the math library.
pub fn value_sources() -> (&'static str, &'static str) {
    (include_str!("llg_value.h"), include_str!("llg_value.c"))
}

/// Scheduler-independent legacy `$random` and `$dist_*` implementations.
/// Compile together with generated models or as a standalone C11 module.
pub fn random_sources() -> (&'static str, &'static str) {
    (include_str!("llg_random.h"), include_str!("llg_random.c"))
}

/// Scheduler-independent dynamic-array, queue, and associative-array storage.
pub fn container_sources() -> (&'static str, &'static str) {
    (
        include_str!("llg_container.h"),
        include_str!("llg_container.c"),
    )
}

/// Scheduler-independent owned SystemVerilog string values and operations.
pub fn string_sources() -> (&'static str, &'static str) {
    (include_str!("llg_string.h"), include_str!("llg_string.c"))
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
pub(crate) fn write_waveform_sources(
    out_dir: &std::path::Path,
) -> Result<(), super::build::BuildError> {
    for (name, content) in waveform_sources() {
        let path = out_dir.join(name);
        std::fs::write(&path, content).map_err(|source| super::build::BuildError::Io {
            action: "write",
            path,
            source,
        })?;
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
