//! rt — the C11 simulation runtime embedded as strings.
//!
//! [`value_sources`] returns the scheduler-independent value types, operations,
//! and conversions (`llg_value.h` / `llg_value.c`), [`random_sources`] the
//! scheduler-independent legacy probabilistic functions (`llg_random.h` /
//! `llg_random.c`), [`rng_sources`] the
//! scheduler-independent random-stream service (`llg_rng.h` / `llg_rng.c`),
//! [`vpi_sources`] the bounded public VPI declarations and plugin bridge
//! (`vpi_user.h` / `llg_vpi.c`),
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

/// The bounded public VPI declarations and generated-model bridge.
pub fn vpi_sources() -> (&'static str, &'static str) {
    (include_str!("vpi_user.h"), include_str!("llg_vpi.c"))
}

/// Internal bridge header included by generated models.  The public plugin
/// header is emitted as `vpi_user.h`; keeping this header separate avoids
/// exposing model metadata structures to applications.
pub fn vpi_bridge_header() -> &'static str {
    include_str!("llg_vpi.h")
}

/// Scheduler-independent deterministic random-stream service.  It owns the
/// PCG stream, hierarchy derivation, unbiased ranges, and state serialization.
pub fn rng_sources() -> (&'static str, &'static str) {
    (include_str!("llg_rng.h"), include_str!("llg_rng.c"))
}

/// (header, implementation) of the event scheduler and runtime facade.
/// Compile together with [`value_sources`] and [`libaco_sources`].
pub fn runtime_sources() -> (&'static str, &'static str) {
    (
        include_str!("llg_rt.h"),
        concat!(
            include_str!("llg_rt_prelude.c"),
            include_str!("scheduler/storage.c"),
            include_str!("scheduler/state.c"),
            include_str!("scheduler/policy.c"),
            include_str!("scheduler/process_registry.c"),
            include_str!("scheduler/activations.c"),
            include_str!("scheduler/wait_queues.c"),
            include_str!("scheduler/deferred_assertions.c"),
            include_str!("scheduler/wakeup.c"),
            include_str!("scheduler/mailboxes.c"),
            include_str!("scheduler/named_events.c"),
            include_str!("scheduler/process_control.c"),
            include_str!("scheduler/forks.c"),
            include_str!("scheduler/dependencies.c"),
            include_str!("scheduler/force.c"),
            include_str!("scheduler/lifecycle.c"),
            include_str!("scheduler/plusargs.c"),
            include_str!("scheduler/simulation_control.c"),
            include_str!("scheduler/sampling.c"),
            include_str!("scheduler/time.c"),
            include_str!("scheduler/process_waits.c"),
            include_str!("scheduler/event_waits.c"),
            include_str!("scheduler/nonblocking.c"),
            include_str!("scheduler/stochastic.c"),
            include_str!("scheduler/reference_writes.c"),
            include_str!("scheduler/nets.c"),
            include_str!("scheduler/nba_commit.c"),
            include_str!("scheduler/formatting.c"),
            include_str!("scheduler/file_io.c"),
            include_str!("scheduler/scanning.c"),
            include_str!("scheduler/memory_io.c"),
            include_str!("scheduler/assertion_control.c"),
            include_str!("scheduler/sequences.c"),
            include_str!("scheduler/concurrent_assertions.c"),
            include_str!("scheduler/monitors.c"),
            include_str!("scheduler/scheduler.c"),
            include_str!("scheduler/output.c"),
        ),
    )
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
        concat!(
            include_str!("llg_container_prelude.c"),
            include_str!("container/value_descriptors.c"),
            include_str!("container/dynamic_values.c"),
            include_str!("container/queue_values.c"),
            include_str!("container/queue_value_mutations.c"),
            include_str!("container/dynamic_arrays.c"),
            include_str!("container/queues.c"),
            include_str!("container/methods.c"),
            include_str!("container/queue_references.c"),
            include_str!("container/associative_arrays.c"),
            include_str!("container/associative_values.c"),
            include_str!("container/associative_value_queries.c"),
        ),
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

#[cfg(test)]
mod tests;
