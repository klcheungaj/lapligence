//! llg_demo — Lapligence command-line Surelog API demonstration.
//!
//! Parses a Verilog/SystemVerilog file using the Surelog C FFI wrapper,
//! prints the elaborated instance tree, and dumps semantic highlight tokens.
//!
//! Example: `llg_demo design.sv -mutestdout`.

use llg::core::tokens;
use llg::ffi::surelog;

fn visit_instance(inst: &surelog::ModuleInstance<'_>) {
    let path = inst.full_path_name();
    let file = inst.file_path();
    println!("Inst: {path}");
    println!("File: {file}");

    let n = inst.child_count();
    for i in 0..n {
        if let Some(child) = inst.child(i) {
            visit_instance(&child);
        }
    }
}

fn run(args: Vec<String>) -> i32 {
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();

    let mut exit_code: i32 = 0;

    let symbol_table = surelog::create_symbol_table();
    let errors = surelog::create_error_container(&symbol_table);
    let clp = surelog::create_command_line_parser(&errors, &symbol_table);

    clp.no_python();
    clp.set_parse();
    clp.set_write_pp_output();
    clp.set_compile();
    let success = clp.parse_command_line(&arg_refs);
    errors.print_messages(clp.mute_stdout());

    let compiler = if success && !clp.help() {
        let c = clp.start_compiler();
        if c.is_none() {
            exit_code = 1;
        }
        let fatal = errors.fatal_count();
        let syntax = errors.syntax_count();
        let error = errors.error_count();
        if !success || fatal > 0 || syntax > 0 || error > 0 {
            exit_code = 1;
        }
        c
    } else {
        None
    };

    if let Some(ref c) = compiler {
        if let Some(ref d) = c.get_design() {
            // Print instance tree.
            let n_top = d.top_instance_count();
            for i in 0..n_top {
                if let Some(top) = d.top_instance(i) {
                    visit_instance(&top);
                }
            }
        }

        // Print semantic highlight tokens from UHDM.
        if let Some(design_h) = c.get_uhdm_design() {
            let file_tokens = tokens::collect_vpi_tokens(design_h);
            for ft in &file_tokens {
                println!("\nTokens for: {}", ft.path);
                println!("  node_count={}", ft.nodes.len());
                for node in &ft.nodes {
                    println!(
                        "  {:>4}:{:<3} .. {:>4}:{:<3}  vpi_type={:<6} name={:?}",
                        node.line, node.col, node.end_line, node.end_col, node.vpi_type, node.name
                    );
                }
            }
        }
    }
    // compiler, clp, errors, symbol_table dropped here in correct reverse-declaration order.

    exit_code
}

fn main() -> std::process::ExitCode {
    std::process::ExitCode::from(run(std::env::args().collect()) as u8)
}
