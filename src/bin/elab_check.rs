//! elab_check — validate and summarize Slang's owned semantic database.
//!
//! Usage: `elab_check [--top module] file.sv...`

use llg::core::{compile, db};
use llg::ffi::slang::{ConstantValue, Snapshot};

fn main() -> std::process::ExitCode {
    std::process::ExitCode::from(run(std::env::args().skip(1).collect()) as u8)
}

fn run(args: Vec<String>) -> i32 {
    let (top, files) = match parse_args(args) {
        Ok(parsed) => parsed,
        Err(message) => {
            eprintln!("elab_check: {message}");
            eprintln!("usage: elab_check [--top module] file.sv...");
            return 2;
        }
    };
    let out = match compile::compile_checked(&compile::CompileOpts {
        files,
        top,
        ..compile::CompileOpts::default()
    }) {
        Ok(out) => out,
        Err(compile::CompileError::Startup(error)) => {
            eprintln!("elab_check: compilation could not start: {error}");
            return 1;
        }
        Err(compile::CompileError::FrontendDiagnostics(diagnostics)) => {
            print_diagnostics(&diagnostics);
            eprintln!("elab_check: Slang reported errors; aborting");
            return 1;
        }
    };
    print_diagnostics(&out.diagnostics);

    let database = match db::Db::from_slang(&out.snapshot) {
        Ok(database) => database,
        Err(error) => {
            eprintln!("elab_check: semantic database validation failed: {error}");
            return 1;
        }
    };

    println!("== Slang elaborated hierarchy ==");
    for instance in out
        .snapshot
        .instances
        .iter()
        .filter(|instance| instance.parent_id.is_none())
    {
        print_instance(&out.snapshot, instance.id, 0);
    }

    let mut references = 0_usize;
    let mut bound_references = 0_usize;
    for id in database.node_ids() {
        if let db::NodeKind::Expr(db::ExprKind::Ref { target }) = database.node_kind(id) {
            references += 1;
            bound_references += usize::from(target.is_some());
        }
    }
    println!("\n== Owned semantic database ==");
    println!("nodes: {}", database.node_ids().len());
    println!(
        "references: {references} total, {bound_references} bound ({:.1}%)",
        if references == 0 {
            100.0
        } else {
            100.0 * bound_references as f64 / references as f64
        }
    );
    if bound_references != references {
        eprintln!(
            "elab_check: semantic database contains {} unbound reference(s)",
            references - bound_references
        );
        return 1;
    }
    0
}

fn parse_args(args: Vec<String>) -> Result<(Option<String>, Vec<String>), &'static str> {
    let mut top = None;
    let mut files = Vec::new();
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--top" | "-top" => top = Some(args.next().ok_or("--top requires a module name")?),
            _ if arg.starts_with('-') => return Err("unknown option"),
            _ => files.push(arg),
        }
    }
    if files.is_empty() {
        return Err("no source files given");
    }
    Ok((top, files))
}

fn print_diagnostics(diagnostics: &[compile::Diag]) {
    for diagnostic in diagnostics {
        eprintln!(
            "{:?}: {}:{}:{} {}",
            diagnostic.severity,
            diagnostic.file.as_deref().unwrap_or(""),
            diagnostic.line,
            diagnostic.col,
            diagnostic.message
        );
    }
}

fn print_instance(snapshot: &Snapshot, id: u64, depth: usize) {
    let Some(instance) = snapshot.instances.iter().find(|instance| instance.id == id) else {
        return;
    };
    let indent = "  ".repeat(depth);
    println!(
        "{indent}{} ({}, {:?})",
        instance.name, instance.definition_name, instance.kind
    );
    for parameter in snapshot
        .parameters
        .iter()
        .filter(|parameter| parameter.owner_instance_id == id)
    {
        let value = parameter
            .constant_id
            .and_then(|constant_id| usize::try_from(constant_id).ok())
            .and_then(|constant_id| snapshot.constants.get(constant_id))
            .map(|constant| format_constant(&constant.value))
            .unwrap_or_else(|| "<type>".to_owned());
        println!("{indent}  parameter {} = {value}", parameter.name);
    }
    for child in snapshot
        .instances
        .iter()
        .filter(|candidate| candidate.parent_id == Some(id))
    {
        print_instance(snapshot, child.id, depth + 1);
    }
}

fn format_constant(value: &ConstantValue) -> String {
    match value {
        ConstantValue::None => "<none>".to_owned(),
        ConstantValue::Integer {
            bit_width,
            value_words,
            unknown_words,
            ..
        } => format!("{bit_width}'h{:x?}/unknown={unknown_words:x?}", value_words),
        ConstantValue::Real(value) => value.to_string(),
        ConstantValue::ShortReal(value) => value.to_string(),
        ConstantValue::String(value) => String::from_utf8_lossy(value).into_owned(),
        ConstantValue::Other(value) => value.clone(),
    }
}
