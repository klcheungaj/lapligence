//! helloworld — minimal Slang frontend demonstration.
//!
//! Usage: `helloworld file.sv...`

use llg::ffi::slang::{self, CompileOptions, CompileRequest, Source};
use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

fn main() -> std::process::ExitCode {
    let paths: Vec<_> = std::env::args().skip(1).collect();
    if paths.is_empty() {
        eprintln!("usage: helloworld file.sv...");
        return std::process::ExitCode::from(2);
    }

    let mut inputs = Vec::with_capacity(paths.len());
    for path in paths {
        match std::fs::read_to_string(&path) {
            Ok(text) => inputs.push((path, text)),
            Err(error) => {
                eprintln!("helloworld: cannot read {path}: {error}");
                return std::process::ExitCode::FAILURE;
            }
        }
    }
    let sources: Vec<_> = inputs
        .iter()
        .map(|(name, text)| Source::compilation_unit(name, text))
        .collect();
    let options = CompileOptions::default();
    let snapshot = match slang::compile(&CompileRequest {
        sources: &sources,
        options: &options,
    }) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            eprintln!("helloworld: compilation could not start: {error}");
            return std::process::ExitCode::FAILURE;
        }
    };
    for diagnostic in &snapshot.diagnostics {
        eprintln!(
            "{:?} {}: {}",
            diagnostic.severity, diagnostic.name, diagnostic.message
        );
    }
    if snapshot.has_errors() {
        return std::process::ExitCode::FAILURE;
    }
    for instance in &snapshot.instances {
        println!("{}: {}", instance.name, instance.definition_name);
    }
    std::process::ExitCode::SUCCESS
}
