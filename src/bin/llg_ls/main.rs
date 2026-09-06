//! Lapligence (llg) — Verilog/SystemVerilog Language Server
//!
//! Communicates with editors via the Language Server Protocol (LSP) over
//! stdin/stdout. Parsing is delegated to Surelog through the shared Rust core.

// Keep the server's Rust allocations on mimalloc; musl builds separately wrap
// native C allocation in the root build script.
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
