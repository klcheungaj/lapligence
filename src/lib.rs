//! Lapligence — SystemVerilog simulation and language tooling built on Slang.
//!
//! - [`ffi`] contains the checked C ABI and platform memory primitives.
//! - [`core`] owns compilation, semantic capture, source analysis, values, and lint.
//! - [`sim`] lowers owned semantics into executable IR, optimizes it, emits C11,
//!   and builds models with the embedded runtime.
//! - [`memory_limit`] supplies the shared process-memory safeguard.
//!
//! LSP protocol and async dependencies remain in the `llg_ls` binary.

pub mod core;
pub mod ffi;
pub mod memory_limit;
pub mod sim;
