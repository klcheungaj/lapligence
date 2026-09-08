//! Lapligence (llg) — Verilog/SystemVerilog simulation and language tooling built on
//! Surelog + UHDM.
//!
//! The crate exposes the shared frontend, owned analysis, simulator pipeline,
//! and process safeguard used by the `llg` and `llg_ls` binaries:
//!
//! - [`ffi`] — Rust↔C(++) FFI layer.  `ffi::surelog` manages Surelog compile
//!   sessions (parse/compile/elaborate flags, structured diagnostics);
//!   `ffi::vpi` is the safe wrapper over the UHDM VPI traversal API.
//! - [`core`] — shared processing layer on top of the FFI.  `core::compile`
//!   is the unified compile pipeline, `core::model` the owned design model
//!   (instances, signals, ports, resolved parameters), `core::elab` the
//!   parameter/constant-expression resolver, and `core::tokens` /
//!   `core::vobject_types` collect VPI + parse-tree objects for semantic
//!   highlighting.
//! - [`sim`] — owned-database lowering, typed IR, optimization, C11 emission,
//!   model building, and the embedded runtime sources.
//! - [`memory_limit`] provides the shared process-memory safeguard used by
//!   both executable frontends.
//!
//! LSP protocol and async dependencies remain in the `llg_ls` binary.
//! The opt-in `slang` feature adds `core::compile::slang` for owned Slang
//! diagnostic and hierarchy observations during frontend migration. It does
//! not replace the simulator's Surelog/UHDM capture yet.

pub mod core;
pub mod ffi;
pub mod memory_limit;
pub mod sim;
