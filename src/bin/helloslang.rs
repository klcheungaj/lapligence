//! helloslang — low-level Slang snapshot demonstration.
//!
//! Usage: `helloslang [--top module] file.sv...`

use llg::ffi::slang::{self, CompileOptions, CompileRequest, Source};

fn main() -> std::process::ExitCode {
    std::process::ExitCode::from(run(std::env::args().skip(1).collect()) as u8)
}

fn run(args: Vec<String>) -> i32 {
    let (top, paths) = match parse_args(args) {
        Ok(parsed) => parsed,
        Err(message) => {
            eprintln!("helloslang: {message}");
            eprintln!("usage: helloslang [--top module] file.sv...");
            return 2;
        }
    };

    let mut inputs = Vec::with_capacity(paths.len());
    for path in paths {
        match std::fs::read_to_string(&path) {
            Ok(text) => inputs.push((path, text)),
            Err(error) => {
                eprintln!("helloslang: cannot read {path}: {error}");
                return 1;
            }
        }
    }
    let sources: Vec<_> = inputs
        .iter()
        .map(|(name, text)| Source::compilation_unit(name, text))
        .collect();
    let options = CompileOptions {
        top_modules: top.into_iter().collect(),
        ..CompileOptions::default()
    };
    let snapshot = match slang::compile(&CompileRequest {
        sources: &sources,
        options: &options,
    }) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            eprintln!("helloslang: compilation could not start: {error}");
            return 1;
        }
    };

    for diagnostic in &snapshot.diagnostics {
        eprintln!(
            "{:?} {}: {}",
            diagnostic.severity, diagnostic.name, diagnostic.message
        );
    }
    if snapshot.has_errors() {
        return 1;
    }

    println!("Files: {}", snapshot.files.len());
    println!("Instance Tree:");
    for instance in snapshot
        .instances
        .iter()
        .filter(|instance| instance.parent_id.is_none())
    {
        print_instance(&snapshot, instance.id, 0);
    }
    println!("Parameters: {}", snapshot.parameters.len());
    println!("Types: {}", snapshot.types.len());
    0
}

fn parse_args(args: Vec<String>) -> Result<(Option<String>, Vec<String>), &'static str> {
    let mut top = None;
    let mut paths = Vec::new();
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--top" | "-top" => top = Some(args.next().ok_or("--top requires a module name")?),
            _ if arg.starts_with('-') => return Err("unknown option"),
            _ => paths.push(arg),
        }
    }
    if paths.is_empty() {
        return Err("no source files given");
    }
    Ok((top, paths))
}

fn print_instance(snapshot: &slang::Snapshot, id: u64, depth: usize) {
    let Some(instance) = snapshot.instances.iter().find(|instance| instance.id == id) else {
        return;
    };
    println!(
        "{}+ {:?}: {} ({})",
        "  ".repeat(depth),
        instance.kind,
        instance.definition_name,
        instance.name
    );
    for child in snapshot
        .instances
        .iter()
        .filter(|candidate| candidate.parent_id == Some(id))
    {
        print_instance(snapshot, child.id, depth + 1);
    }
}
