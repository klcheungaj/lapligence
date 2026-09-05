//! Lapligence (llg) — Verilog/SystemVerilog Language Server
//!
//! Communicates with editors via the Language Server Protocol (LSP) over
//! stdin/stdout.  Parsing is delegated to Surelog via a C FFI wrapper.
//!
//! See `src/bin/llg_demo.rs` for the original command-line demo that
//! illustrates raw Surelog API usage.

// Replace musl's default allocator (and the system allocator on all targets)
// with mimalloc.  Because this is the final link point the override is also
// effective for all statically linked C/C++ code (Surelog, UHDM, ANTLR, …).
use mimalloc::MiMalloc;
#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

mod config;
mod dump;
mod features;
mod inactive_ranges;
mod logging;
mod lsp;
mod module_explorer;
mod rename;
mod request_cache;
mod scheduler;
mod semantic_tokens;
mod workspace;

// Cross-scanner corpus pinning `core::macros` and `inactive_ranges` together;
// compiled only for tests.
#[cfg(test)]
mod conditional_conformance;

mod transport;

#[tokio::main]
async fn main() -> std::process::ExitCode {
    std::process::ExitCode::from(transport::run().await as u8)
}
