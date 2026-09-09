//! Lapligence (llg) — Verilog/SystemVerilog Language Server
//!
//! Communicates with editors via the Language Server Protocol (LSP) over
//! stdin/stdout. Compilation is delegated to Slang through the shared Rust core.

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

fn main() -> std::process::ExitCode {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if let Some(first) = args.first() {
        if args.len() == 1 && (first == "--help" || first == "-h") {
            println!(
                "Lapligence Verilog/SystemVerilog language server

Usage: llg_ls [OPTIONS]

With no arguments, serve LSP over stdin/stdout.

Options:
  -h, --help                Print help and exit
  -V, --version             Print the package version and exit
      --stdio               Serve LSP over stdin/stdout (default)
      --dump-tokens <PATH>  Print token bindings for a file or directory"
            );
            return std::process::ExitCode::SUCCESS;
        }
        if args.len() == 1 && (first == "--version" || first == "-V") {
            println!("llg_ls {}", env!("CARGO_PKG_VERSION"));
            return std::process::ExitCode::SUCCESS;
        }
        if !(args.len() == 1 && first == "--stdio" || args.len() == 2 && first == "--dump-tokens") {
            eprintln!("llg_ls: invalid arguments; use --help for usage");
            return std::process::ExitCode::from(2);
        }
    }
    std::process::ExitCode::from(run_server() as u8)
}

#[tokio::main]
async fn run_server() -> i32 {
    transport::run().await
}
