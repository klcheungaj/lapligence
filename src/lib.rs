//! Lapligence (llg) — Verilog/SystemVerilog simulation and language tooling built on
//! Surelog + UHDM.
//!
//! The crate is organised into two layers, both shared by the LSP server
//! (`src/bin/llg/`) and the simulator tooling (`src/bin/elab_check.rs`):
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
//! - [`memory_limit`] provides the shared process-memory safeguard used by
//!   both executable frontends.
//!
//! Binary targets consume the same modules; nothing LSP- or simulator-specific
//! lives in this library.

pub mod core;
pub mod ffi;
pub mod memory_limit;
pub mod sim;
