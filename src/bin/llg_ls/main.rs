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
#[cfg(test)]
mod test_paths;

mod transport;

fn main() -> std::process::ExitCode {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if let [first] = args.as_slice() {
        if first == "--help" || first == "-h" {
            println!(
                "Lapligence Verilog/SystemVerilog language server

Usage: llg_ls [OPTIONS]

With no arguments, serve LSP over stdin/stdout.

Options:
  -h, --help                Print help and exit
  -V, --version             Print the package version and exit
      --stdio               Serve LSP over stdin/stdout (default)
      --dump-tokens <PATH>  Print token bindings for a file or directory
      --staging-dir <DIR>   Stage unsaved buffers under DIR (default: OS temp dir)"
            );
            return std::process::ExitCode::SUCCESS;
        }
        if first == "--version" || first == "-V" {
            println!("llg_ls {}", env!("CARGO_PKG_VERSION"));
            return std::process::ExitCode::SUCCESS;
        }
    }
    let Some(options) = parse_args(args) else {
        eprintln!("llg_ls: invalid arguments; use --help for usage");
        return std::process::ExitCode::from(2);
    };
    if let Some(dir) = options.staging_dir {
        if let Err(code) = select_staging_dir(dir) {
            return std::process::ExitCode::from(code);
        }
    }
    std::process::ExitCode::from(run_server(options.dump_target) as u8)
}

struct ServerOptions {
    /// `--dump-tokens <PATH>`: run the offline dump instead of serving LSP.
    dump_target: Option<std::path::PathBuf>,
    staging_dir: Option<std::path::PathBuf>,
}

/// Accept `--stdio`, `--dump-tokens <PATH>` and `--staging-dir <DIR>`, each at
/// most once. `None` means a usage error.
fn parse_args(args: Vec<std::ffi::OsString>) -> Option<ServerOptions> {
    let mut options = ServerOptions {
        dump_target: None,
        staging_dir: None,
    };
    let mut stdio = false;
    let mut it = args.into_iter();
    while let Some(arg) = it.next() {
        if arg == "--stdio" && !stdio {
            stdio = true;
        } else if arg == "--dump-tokens" && options.dump_target.is_none() {
            options.dump_target = Some(it.next().filter(|v| !v.is_empty())?.into());
        } else if arg == "--staging-dir" && options.staging_dir.is_none() {
            options.staging_dir = Some(it.next().filter(|v| !v.is_empty())?.into());
        } else {
            return None;
        }
    }
    (!(stdio && options.dump_target.is_some())).then_some(options)
}

/// Create `dir` and make it the parent of this process's staging base. The
/// path is made absolute because shadow paths are compared with absolute
/// editor paths.
fn select_staging_dir(dir: std::path::PathBuf) -> Result<(), u8> {
    let dir = match std::path::absolute(&dir) {
        Ok(dir) => dir,
        Err(error) => {
            eprintln!("llg_ls: --staging-dir {}: {error}", dir.display());
            return Err(2);
        }
    };
    if let Err(error) = std::fs::create_dir_all(&dir) {
        eprintln!("llg_ls: create staging dir {}: {error}", dir.display());
        return Err(1);
    }
    features::set_staging_root(dir);
    Ok(())
}

#[tokio::main]
async fn run_server(dump_target: Option<std::path::PathBuf>) -> i32 {
    transport::run(dump_target).await
}
