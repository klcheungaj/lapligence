//! rt — the C11 simulation runtime embedded as strings.
//!
//! [`value_sources`] returns the scheduler-independent value types, operations,
//! and conversions (`llg_value.h` / `llg_value.c`), [`random_sources`] the
//! scheduler-independent legacy probabilistic functions (`llg_random.h` /
//! `llg_random.c`), [`rng_sources`] the
//! scheduler-independent random-stream service (`llg_rng.h` / `llg_rng.c`),
//! [`coroutine_sources`] the stackless coroutine library (`llg_co.h` /
//! `llg_co.c`),
//! [`vpi_sources`] the bounded public VPI declarations and plugin bridge
//! (`vpi_user.h` / `llg_vpi.c`),
//! [`container_sources`] and [`string_sources`] the dynamically sized value
//! stores, [`runtime_sources`] the event scheduler (`llg_rt.h` / `llg_rt.c`),
//! [`waveform_sources`] the optional
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

/// Stackless coroutine frame, chain, anchor, and arena support.
pub fn coroutine_sources() -> (&'static str, &'static str) {
    (include_str!("llg_co.h"), include_str!("llg_co.c"))
}

/// (header, implementation) of the event scheduler and runtime facade.
/// Compile together with [`value_sources`] and [`coroutine_sources`].
pub fn runtime_sources() -> (&'static str, &'static str) {
    (
        include_str!("llg_rt.h"),
        concat!(
            include_str!("llg_rt_prelude.c"),
            include_str!("scheduler/storage.c"),
            include_str!("scheduler/state.c"),
            include_str!("scheduler/policy.c"),
            include_str!("scheduler/output_files.c"),
            include_str!("scheduler/process_registry.c"),
            include_str!("scheduler/activations.c"),
            include_str!("scheduler/value_scopes.c"),
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
            include_str!("scheduler/fixed_arrays.c"),
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
            include_str!("scheduler/destinations.c"),
        ),
    )
}

/// (header, implementation) of scheduler-independent value operations and casts.
/// Write the legacy [`value_backend_sources`] beside these files. This C11
/// module can be compiled independently, linking only the math library.
pub fn value_sources() -> (&'static str, &'static str) {
    value_sources_for(super::value_backend::ValueBackend::Legacy)
}

/// Selected facade and implementation. Compact units remain separate so only
/// kernels.c includes GMP, and private implementation names cannot collide.
/// Write [`value_backend_sources`] beside these files and compile all units with
/// the same backend and kernel selectors.
pub fn value_sources_for(
    backend: super::value_backend::ValueBackend,
) -> (&'static str, &'static str) {
    let source = match backend {
        super::value_backend::ValueBackend::Legacy => concat!(
            include_str!("llg_value_prelude.c"),
            include_str!("value/storage.c"),
            include_str!("value/operations.c"),
            include_str!("value/array_conditional.c"),
            include_str!("value/selection_plan.c"),
            include_str!("value/udp.c"),
            include_str!("value/references.c"),
            include_str!("value/destinations.c"),
        ),
        super::value_backend::ValueBackend::Compact => concat!(
            "#include \"llg_value.h\"\n",
            include_str!("value/destinations.c"),
        ),
    };
    (include_str!("llg_value.h"), source)
}

/// Selected header dependencies and compact implementation units in build order.
pub fn value_backend_sources(
    backend: super::value_backend::ValueBackend,
) -> &'static [(&'static str, &'static str)] {
    match backend {
        super::value_backend::ValueBackend::Legacy => &[
            ("value/backend.h", include_str!("value/backend.h")),
            (
                "value/consumer_bridge.h",
                include_str!("value/consumer_bridge.h"),
            ),
            ("value/bridge.h", include_str!("value/bridge.h")),
            ("value/destinations.h", include_str!("value/destinations.h")),
        ],
        super::value_backend::ValueBackend::Compact => &[
            ("value/bridge.h", include_str!("value/bridge.h")),
            ("value/destinations.h", include_str!("value/destinations.h")),
            ("value_gmp/backend.h", include_str!("value_gmp/backend.h")),
            (
                "value_gmp/reference_types.h",
                include_str!("value_gmp/reference_types.h"),
            ),
            (
                "value_gmp/selection_inline.h",
                include_str!("value_gmp/selection_inline.h"),
            ),
            (
                "value_gmp/consumer_inline.h",
                include_str!("value_gmp/consumer_inline.h"),
            ),
            ("value_gmp/internal.h", include_str!("value_gmp/internal.h")),
            ("value_gmp/ranges.h", include_str!("value_gmp/ranges.h")),
            ("value_gmp/storage.c", include_str!("value_gmp/storage.c")),
            ("value_gmp/logic.c", include_str!("value_gmp/logic.c")),
            (
                "value_gmp/arithmetic.c",
                include_str!("value_gmp/arithmetic.c"),
            ),
            (
                "value_gmp/shifts_reductions.c",
                include_str!("value_gmp/shifts_reductions.c"),
            ),
            (
                "value_gmp/comparison_membership.c",
                include_str!("value_gmp/comparison_membership.c"),
            ),
            (
                "value_gmp/selections.c",
                include_str!("value_gmp/selections.c"),
            ),
            (
                "value_gmp/references.c",
                include_str!("value_gmp/references.c"),
            ),
            ("value_gmp/assembly.c", include_str!("value_gmp/assembly.c")),
            (
                "value_gmp/consumer_bridge.c",
                include_str!("value_gmp/consumer_bridge.c"),
            ),
            ("value_gmp/kernels.c", include_str!("value_gmp/kernels.c")),
            (
                "value_gmp/net_adapters.c",
                include_str!("value_gmp/net_adapters.c"),
            ),
            (
                "value_gmp/real_time.c",
                include_str!("value_gmp/real_time.c"),
            ),
            (
                "value_gmp/format_index.c",
                include_str!("value_gmp/format_index.c"),
            ),
        ],
    }
}

/// The platform layer: `llg_compiler.h` (compiler attributes, safe for
/// generated models) and the runtime-private `llg_platform.h` /
/// `llg_platform_native.h`, which own every operating-system conditional.
pub fn platform_headers() -> &'static [(&'static str, &'static str)] {
    &[
        ("llg_compiler.h", include_str!("llg_compiler.h")),
        ("llg_platform.h", include_str!("llg_platform.h")),
        (
            "llg_platform_native.h",
            include_str!("llg_platform_native.h"),
        ),
    ]
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
            include_str!("container/destinations.c"),
        ),
    )
}

/// Scheduler-independent owned SystemVerilog string values and operations.
pub fn string_sources() -> (&'static str, &'static str) {
    (include_str!("llg_string.h"), include_str!("llg_string.c"))
}

/// zlib v1.3.2 files from the `vendor/zlib` submodule that libfst needs
/// (deflate/inflate, `compress2`/`uncompress` and the `gz*` stream API). They
/// are written under `zlib/` and compiled into the waveform runtime with
/// `Z_PREFIX`, so generated models need no system zlib and cannot clash with
/// one linked by user DPI libraries.
macro_rules! zlib_source {
    ($name:literal) => {
        (
            concat!("zlib/", $name),
            include_str!(concat!("../../../vendor/zlib/", $name)),
        )
    };
}

/// libfst files from `vendor/libfst`, which the build script has patched in
/// place with `patches/libfst` (the repository holds the pristine snapshot).
macro_rules! libfst_source {
    ($name:literal) => {
        (
            $name,
            include_str!(concat!("../../../vendor/libfst/", $name)),
        )
    };
}

/// Optional waveform runtime, the official GTKWave libfst writer snapshot and
/// the bundled zlib it uses. These are written and compiled only for models
/// containing `#define LLG_WAVEFORM 1`.
pub fn waveform_sources() -> &'static [(&'static str, &'static str)] {
    &[
        ("llg_wave.h", include_str!("llg_wave.h")),
        ("llg_wave.c", include_str!("llg_wave.c")),
        libfst_source!("fstapi.c"),
        libfst_source!("fstapi.h"),
        libfst_source!("fastlz.c"),
        libfst_source!("fastlz.h"),
        libfst_source!("lz4.c"),
        libfst_source!("lz4.h"),
        libfst_source!("fst_config.h"),
        libfst_source!("fst_win_unistd.h"),
        libfst_source!("wavealloca.h"),
        zlib_source!("zlib.h"),
        zlib_source!("zconf.h"),
        zlib_source!("zutil.h"),
        zlib_source!("zutil.c"),
        zlib_source!("adler32.c"),
        zlib_source!("crc32.h"),
        zlib_source!("crc32.c"),
        zlib_source!("deflate.h"),
        zlib_source!("deflate.c"),
        zlib_source!("trees.h"),
        zlib_source!("trees.c"),
        zlib_source!("inflate.h"),
        zlib_source!("inflate.c"),
        zlib_source!("inffast.h"),
        zlib_source!("inffast.c"),
        zlib_source!("inffixed.h"),
        zlib_source!("inftrees.h"),
        zlib_source!("inftrees.c"),
        zlib_source!("compress.c"),
        zlib_source!("uncompr.c"),
        zlib_source!("gzguts.h"),
        zlib_source!("gzlib.c"),
        zlib_source!("gzclose.c"),
        zlib_source!("gzread.c"),
        zlib_source!("gzwrite.c"),
    ]
}

/// Write optional waveform sources into a generated model directory.
pub(crate) fn write_waveform_sources(
    out_dir: &std::path::Path,
) -> Result<(), super::build::BuildError> {
    for (name, content) in waveform_sources() {
        let path = out_dir.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| super::build::BuildError::Io {
                action: "create",
                path: parent.to_path_buf(),
                source,
            })?;
        }
        std::fs::write(&path, content).map_err(|source| super::build::BuildError::Io {
            action: "write",
            path,
            source,
        })?;
    }
    Ok(())
}

/// Fenced pre-migration C fixture (not runnable until P05): sv4 value vectors (mirroring `core::elab` unit
/// tests) plus scheduler checks (delay ordering, NBA visibility, ping-pong).
pub fn selftest_source() -> &'static str {
    include_str!("llg_rt_selftest.c")
}

/// Explicit-frame helper included by [`selftest_source`].
pub fn selftest_support_source() -> &'static str {
    include_str!("selftest_co.h")
}

/// Fenced pre-migration waveform fixture; active tests are in runtime_value_storage.
pub fn waveform_selftest_source() -> &'static str {
    include_str!("llg_wave_selftest.c")
}

#[cfg(test)]
mod tests;
