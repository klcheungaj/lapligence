//! llg_demo — print the owned observations produced by Slang.
//!
//! Usage: `llg_demo file.sv...`

use llg::ffi::slang::{self, CompileOptions, CompileRequest, Source};

fn main() -> std::process::ExitCode {
    std::process::ExitCode::from(run(std::env::args().skip(1).collect()) as u8)
}

fn run(paths: Vec<String>) -> i32 {
    if paths.is_empty() {
        eprintln!("usage: llg_demo file.sv...");
        return 2;
    }
    let mut inputs = Vec::with_capacity(paths.len());
    for path in paths {
        match std::fs::read_to_string(&path) {
            Ok(text) => inputs.push((path, text)),
            Err(error) => {
                eprintln!("llg_demo: cannot read {path}: {error}");
                return 1;
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
            eprintln!("llg_demo: compilation could not start: {error}");
            return 1;
        }
    };

    for file in &snapshot.files {
        println!("file {}: {} bytes", file.name, file.byte_len);
    }
    for diagnostic in &snapshot.diagnostics {
        println!(
            "diagnostic {:?} {}: {}",
            diagnostic.severity, diagnostic.name, diagnostic.message
        );
    }
    for instance in &snapshot.instances {
        println!(
            "instance {}: {} ({:?})",
            instance.name, instance.definition_name, instance.kind
        );
    }
    for parameter in &snapshot.parameters {
        println!("parameter {} ({:?})", parameter.name, parameter.kind);
    }

    i32::from(snapshot.has_errors())
}
