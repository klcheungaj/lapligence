//! hellouhdm — Rust port of src/hellouhdm.cpp
//!
//! Demonstrates the Surelog/UHDM API via a C FFI wrapper.
//! Compiles a design, optionally elaborates the UHDM model, then prints:
//!   - a flat module list (ports, processes, continuous assignments)
//!   - the elaborated instance tree
//!
//! Example usage (from the Surelog repo root):
//!   cd tests/UnitElabBlock
//!   hellouhdm top.v -parse -mutestdout

use llg::ffi::surelog;
use llg::ffi::vpi;
use llg::ffi::vpi::VpiHandle;

// ── Instance-tree visitor ─────────────────────────────────────────────────────

/// Recursively print the elaborated instance tree rooted at `obj_h`.
fn inst_visit(obj_h: VpiHandle, margin: &str) -> String {
    let mut res = String::new();

    let def_name = vpi::get_str(vpi::vpiDefName, obj_h);
    let obj_name = vpi::obj_name(obj_h);
    let file = vpi::obj_file(obj_h);
    let line = vpi::get(vpi::vpiLineNo, obj_h);

    let display_def = if obj_name.is_empty() {
        def_name.clone()
    } else {
        format!("{def_name} ")
    };
    let display_obj = if obj_name.is_empty() {
        String::new()
    } else {
        format!("({obj_name})")
    };
    res.push_str(&format!(
        "{margin}+ module: {display_def}{display_obj}, file:{file}, line:{line}\n"
    ));

    let deeper = format!("  {margin}");
    let ty = vpi::obj_type(obj_h);

    if ty == vpi::vpiModule || ty == vpi::vpiGenScope {
        for sub_h in vpi::iterate(vpi::vpiModule, obj_h).into_iter().flatten() {
            res.push_str(&inst_visit(sub_h, &deeper));
        }
        for sub_h in vpi::iterate(vpi::vpiGenScopeArray, obj_h)
            .into_iter()
            .flatten()
        {
            res.push_str(&inst_visit(sub_h, &deeper));
        }
    }

    if ty == vpi::vpiGenScopeArray {
        for sub_h in vpi::iterate(vpi::vpiGenScope, obj_h).into_iter().flatten() {
            res.push_str(&inst_visit(sub_h, &deeper));
        }
    }

    res
}

// ── main ─────────────────────────────────────────────────────────────────────

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();

    let mut exit_code = 0i32;

    // ── Create core Surelog objects ───────────────────────────────────────────
    let symbol_table = surelog::create_symbol_table();
    let errors = surelog::create_error_container(&symbol_table);
    let clp = surelog::create_command_line_parser(&errors, &symbol_table);

    clp.no_python();
    clp.set_parse();
    clp.set_write_pp_output();
    clp.set_compile();
    clp.set_elaborate();

    let success = clp.parse_command_line(&arg_refs);
    errors.print_messages(clp.mute_stdout());

    let mut compiler: Option<surelog::Compiler> = None;
    let mut the_design: Option<VpiHandle> = None;

    if success && !clp.help() {
        compiler = clp.start_compiler();
        if let Some(ref c) = compiler {
            the_design = c.get_uhdm_design();
        } else {
            exit_code = 1;
        }

        let fatal = errors.fatal_count();
        let syntax = errors.syntax_count();
        let error = errors.error_count();
        if !success || fatal > 0 || syntax > 0 || error > 0 {
            exit_code = 1;
        }
    }

    if let Some(design) = the_design {
        // ── Optionally elaborate the UHDM model ──────────────────────────────
        if vpi::get(vpi::vpiElaborated, design) == 0 {
            println!("UHDM Elaboration...");
            surelog::uhdm_elaborate(design);
        }

        let mut result = String::new();

        // ── Design name ───────────────────────────────────────────────────────
        if vpi::get(vpi::vpiType, design) == vpi::vpiDesign {
            let name = vpi::obj_name(design);
            result.push_str(&format!("Design name (C++): {name}\n"));
        }
        let vpi_name = vpi::obj_name(design);
        result.push_str(&format!("Design name (VPI): {vpi_name}\n"));

        // ── Flat module list ──────────────────────────────────────────────────
        result.push_str("Module List:\n");

        for obj_h in vpi::iterate(vpi::uhdmallModules, design)
            .into_iter()
            .flatten()
        {
            if vpi::obj_type(obj_h) != vpi::vpiModule {
                result.push_str("ERROR: this is not a module\n");
            }

            let def_name = vpi::get_str(vpi::vpiDefName, obj_h);
            let obj_name = vpi::obj_name(obj_h);
            let display_def = if obj_name.is_empty() {
                def_name.clone()
            } else {
                format!("{def_name} ")
            };
            let display_obj = if obj_name.is_empty() {
                String::new()
            } else {
                format!("({obj_name})")
            };
            let file = vpi::obj_file(obj_h);
            let line = vpi::get(vpi::vpiLineNo, obj_h);
            result.push_str(&format!(
                "+ module: {display_def}{display_obj}, file:{file}, line:{line}"
            ));

            for sub_h in vpi::iterate(vpi::vpiProcess, obj_h).into_iter().flatten() {
                let sub_file = vpi::obj_file(sub_h);
                let sub_line = vpi::get(vpi::vpiLineNo, sub_h);
                result.push_str(&format!(
                    "\n    \\_ process stmt, file:{sub_file}, line:{sub_line}"
                ));
            }

            for sub_h in vpi::iterate(vpi::vpiContAssign, obj_h)
                .into_iter()
                .flatten()
            {
                let sub_file = vpi::obj_file(sub_h);
                let sub_line = vpi::get(vpi::vpiLineNo, sub_h);
                result.push_str(&format!(
                    "\n    \\_ assign stmt, file:{sub_file}, line:{sub_line}"
                ));
            }

            for sub_h in vpi::iterate(vpi::vpiNets, obj_h).into_iter().flatten() {
                let sub_file = vpi::obj_file(sub_h);
                let sub_line = vpi::get(vpi::vpiLineNo, sub_h);
                let sub_char = vpi::get(vpi::vpiSize, sub_h);
                result.push_str(&format!(
                    "\n    \\_ nets stmt, file:{sub_file}, line:{sub_line}, size:{sub_char}"
                ));
            }

            for sub_h in vpi::iterate(vpi::vpiNet, obj_h).into_iter().flatten() {
                let sub_file = vpi::obj_file(sub_h);
                let sub_line = vpi::get(vpi::vpiLineNo, sub_h);
                let column: i32 = vpi::get(vpi::vpiColumnNo, sub_h);
                let end_line = vpi::get(vpi::vpiEndLineNo, sub_h);
                let end_column = vpi::get(vpi::vpiEndColumnNo, sub_h);
                let def_name = vpi::get_str(vpi::vpiDefName, sub_h);
                let obj_name = vpi::obj_name(sub_h);
                result.push_str(&format!("\n    \\_ net stmt, def name:{def_name}, obj name:{obj_name}, start={sub_line}:{column}, end={end_line}:{end_column}"));
            }

            for sub_h in vpi::iterate(vpi::vpiParameters, obj_h)
                .into_iter()
                .flatten()
            {
                let sub_file = vpi::obj_file(sub_h);
                let sub_line = vpi::get(vpi::vpiLineNo, sub_h);
                result.push_str(&format!(
                    "\n    \\_ parameter stmt, file:{sub_file}, line:{sub_line}"
                ));
            }

            for sub_h in vpi::iterate(vpi::vpiLogicVar, obj_h).into_iter().flatten() {
                let sub_file = vpi::obj_file(sub_h);
                let sub_line = vpi::get(vpi::vpiLineNo, sub_h);
                result.push_str(&format!(
                    "\n    \\_ logic var stmt, file:{sub_file}, line:{sub_line}"
                ));
            }
            for sub_h in vpi::iterate(vpi::vpiNetType, obj_h).into_iter().flatten() {
                let sub_file = vpi::obj_file(sub_h);
                let sub_line = vpi::get(vpi::vpiLineNo, sub_h);
                result.push_str(&format!(
                    "\n    \\_ net stmt, file:{sub_file}, line:{sub_line}"
                ));
            }
            for sub_h in vpi::iterate(vpi::vpiWire, obj_h).into_iter().flatten() {
                let sub_file = vpi::obj_file(sub_h);
                let sub_line = vpi::get(vpi::vpiLineNo, sub_h);
                result.push_str(&format!(
                    "\n    \\_ wire stmt, file:{sub_file}, line:{sub_line}"
                ));
            }
            for sub_h in vpi::iterate(vpi::vpiReg, obj_h).into_iter().flatten() {
                let sub_file = vpi::obj_file(sub_h);
                let sub_line = vpi::get(vpi::vpiLineNo, sub_h);
                result.push_str(&format!(
                    "\n    \\_ reg stmt, file:{sub_file}, line:{sub_line}"
                ));
            }
            for sub_h in vpi::iterate(vpi::vpiRegBit, obj_h).into_iter().flatten() {
                let sub_file = vpi::obj_file(sub_h);
                let sub_line = vpi::get(vpi::vpiLineNo, sub_h);
                result.push_str(&format!(
                    "\n    \\_ reg bit stmt, file:{sub_file}, line:{sub_line}"
                ));
            }
            for sub_h in vpi::iterate(vpi::vpiDriver, obj_h).into_iter().flatten() {
                let sub_file = vpi::obj_file(sub_h);
                let sub_line = vpi::get(vpi::vpiLineNo, sub_h);
                result.push_str(&format!(
                    "\n    \\_ driver stmt, file:{sub_file}, line:{sub_line}"
                ));
            }
            for sub_h in vpi::iterate(vpi::vpiVariables, obj_h).into_iter().flatten() {
                let sub_file = vpi::obj_file(sub_h);
                let sub_line = vpi::get(vpi::vpiLineNo, sub_h);
                result.push_str(&format!(
                    "\n    \\_ variable stmt, file:{sub_file}, line:{sub_line}"
                ));
            }
            for sub_h in vpi::iterate(vpi::vpiRegArray, obj_h).into_iter().flatten() {
                let sub_file = vpi::obj_file(sub_h);
                let sub_line = vpi::get(vpi::vpiLineNo, sub_h);
                result.push_str(&format!(
                    "\n    \\_ reg array stmt, file:{sub_file}, line:{sub_line}"
                ));
            }

            result.push('\n');
        }

        // ── Elaborated instance tree ──────────────────────────────────────────
        result.push_str("Instance Tree:\n");
        for top_h in vpi::iterate(vpi::uhdmtopModules, design)
            .into_iter()
            .flatten()
        {
            result.push_str(&inst_visit(top_h, ""));
        }

        println!("{result}");
    }

    // Drop order: compiler must be dropped before clp / errors / symbol_table.
    drop(compiler);

    std::process::exit(exit_code);
}
