//! rt — the C11 simulation runtime embedded as strings.
//!
//! [`runtime_sources`] returns the 4-state value model and event scheduler
//! (`llg_rt.h` / `llg_rt.c`), [`libaco_sources`] the vendored coroutine
//! library (`aco.h` / `aco.c` / `acosw.S`), and [`selftest_source`] the
//! runtime's C self-test.  The driver and integration tests write these into a
//! build directory and build them together with the generated model through
//! CMake (`sim::build`) — the runtime is deliberately *not* linked into the
//! Rust binaries.
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

/// The runtime's C self-test: sv4 value vectors (mirroring `core::elab` unit
/// tests) plus scheduler checks (delay ordering, NBA visibility, ping-pong).
pub fn selftest_source() -> &'static str {
    include_str!("llg_rt_selftest.c")
}
