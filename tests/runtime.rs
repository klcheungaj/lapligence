//! Grouped integration tests: Native runtime, value-storage and probe suites (`tests/runtime_*.rs`).
//!
//! Each `mod` below is one `tests/<name>.rs` suite, compiled into this
//! single test binary so the library and Slang are linked once per group
//! instead of once per file. Register a new suite here; the
//! `test_layout` suite rejects unregistered files. See `tests/readme.md`.

#[path = "support/c_compiler.rs"]
mod c_compiler;
#[path = "support/sim.rs"]
mod sim_harness;

mod runtime_boundaries;
mod runtime_containers;
mod runtime_coroutine_library;
mod runtime_file_io;
mod runtime_gmp_closure;
mod runtime_random;
mod runtime_regions;
mod runtime_review_batch3;
mod runtime_rng;
mod runtime_stochastic;
mod runtime_value_facade;
mod runtime_value_storage;
mod runtime_values;
