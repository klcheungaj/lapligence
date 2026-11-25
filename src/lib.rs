//! Lapligence — SystemVerilog simulation and language tooling built on Slang.
//!
//! - [`ffi`] contains the checked C ABI and platform memory primitives.
//! - [`core`] owns compilation, semantic capture, source analysis, values, and lint.
//! - [`sim`] lowers owned semantics into executable IR, optimizes it, emits C11,
//!   and builds models with the embedded runtime.
//! - [`config`] defines the `llg.toml` schema shared by the `llg` driver and the
//!   language server: parsing, validation, path resolution and discovery.
//! - [`memory_limit`] supplies the shared process-memory safeguard.
//!
//! LSP protocol and async dependencies remain in the `llg_ls` binary.

pub mod config;
pub mod core;
pub mod ffi;
pub mod memory_limit;
#[doc(hidden)]
pub mod profile;
pub mod sim;
pub mod stack;
mod id_map;
pub(crate) use id_map::{IdMap, IdSet};
pub(crate) mod work;

/// Seeded input generation shared with the integration-test groups
/// (`tests/support/fuzz.rs` is its one definition).
#[cfg(test)]
#[path = "../tests/support/fuzz.rs"]
mod fuzz;
