//! elab_check — verify Surelog's elaboration output (UHDM) is complete.
//!
//! Runs Surelog with the full elaboration flow (parse + compile + elaborate +
//! `-elabuhdm`) and reports what the elaborated UHDM contains:
//!   - the instance tree (Surelog ModuleInstance API)
//!   - per-instance UHDM objects: ports, nets, vars, parameters, param_assigns,
//!     continuous assignments, processes, gen scopes
//!   - ref binding: the fraction of `ref_obj`/`var_select` nodes whose
//!     `vpiActual` is resolved to a concrete object
//!   - constant folding of ranges: left/right range constants per net
//!
//! Usage:
//!   elab_check [surelog args...] file.sv
//!   (pass `-top <module>` as needed; `-E -elabuhdm` are implied)

use std::os::raw::c_int;

use llg::core::compile;
use llg::core::elab;
use llg::ffi::surelog;
use llg::ffi::vpi;
use llg::ffi::vpi::VpiHandle;

// ── VPI constants (SV additions used by the UHDM model) ──────────────────────

const VPI_ELABORATED: c_int = vpi::vpiElaborated;

fn iter(type_: c_int, obj: VpiHandle) -> Vec<VpiHandle> {
    vpi::iterate(type_, obj)
        .map(|it| it.collect())
        .unwrap_or_default()
}

/// Child handle kept alive for the duration of the returned `OwnedHandle`.
/// Callers must hold the handle (or use `.raw()` on it) before it drops.
fn child_handle(type_: c_int, obj: VpiHandle) -> Option<vpi::OwnedHandle> {
    vpi::handle(type_, obj)
}

/// Invoke `f` for every child reachable via relationship `rel` — works for
/// both 1-to-many (`vpi_iterate`) and 1-to-1 (`vpi_handle`) relationships.
fn each_child<F: FnMut(VpiHandle)>(rel: c_int, obj: VpiHandle, f: &mut F) {
    if let Some(it) = vpi::iterate(rel, obj) {
        for h in it {
            f(h);
        }
    } else if let Some(h) = vpi::handle(rel, obj) {
        // `h` (OwnedHandle) stays alive until `f` returns.
        f(h.raw());
    }
}

/// Recursively collect every ref-like expression under `root` and report how
/// many resolve via `vpiActual`.
fn count_refs(root: VpiHandle, stats: &mut RefStats) {
    // Relationship set that covers the expression/statement tree.
    const RELS: [c_int; 15] = [
        vpi::vpiRhs,
        vpi::vpiLhs,
        vpi::vpiCondition,
        vpi::vpiOperand,
        vpi::vpiArgument,
        vpi::vpiIndex,
        vpi::vpiLeftRange,
        vpi::vpiRightRange,
        vpi::vpiStmt,
        vpi::vpiElseStmt,
        vpi::vpiActual,
        vpi::vpiExpr,
        vpi::vpiProcess,
        vpi::vpiContAssign,
        vpi::vpiParamAssign,
    ];
    for rel in RELS {
        each_child(rel, root, &mut |child| {
            let t = vpi::obj_type(child);
            match t {
                vpi::vpiRefObj
                | vpi::vpiVarSelect
                | vpi::vpiBitSelect
                | vpi::vpiPartSelect
                | vpi::vpiIndexedPartSelect => {
                    stats.total += 1;
                    // A ref-like node resolves when vpiActual points at a net,
                    // var, param, or module instance (i.e. is non-null).
                    if child_handle(vpi::vpiActual, child).is_some() {
                        stats.bound += 1;
                    }
                }
                _ => {}
            }
            count_refs(child, stats);
        });
    }
}

#[derive(Default)]
struct RefStats {
    total: usize,
    bound: usize,
}

#[derive(Default)]
struct InstStats {
    full_name: String,
    def_name: String,
    /// Raw VPI handle of the module instance (valid for the whole walk).
    handle: VpiHandle,
    ports: usize,
    nets: usize,
    vars: usize,
    params: usize,
    param_assigns: usize,
    cont_assigns: usize,
    processes: usize,
    gen_scopes: usize,
    // Ports whose high_conn (parent side) or low_conn (child side) is missing.
    ports_missing_high_conn: usize,
    ports_missing_low_conn: usize,
    // Ranges whose left/right bounds are not constants (unfolded).
    unfolded_ranges: usize,
    // Processes whose event control has no condition: sensitivity must be
    // derived from the body (always_comb / always @*).
    implicit_sensitivity: usize,
    // Parameters whose param_assign RHS is not a constant (unfolded expr).
    unfolded_param_exprs: usize,
    children: Vec<InstStats>,
}

fn visit_module_inst(mi: VpiHandle) -> InstStats {
    let mut s = InstStats {
        full_name: vpi::obj_full_name(mi),
        def_name: vpi::get_str(vpi::vpiDefName, mi),
        handle: mi,
        ..Default::default()
    };

    for port in iter(vpi::vpiPort, mi) {
        s.ports += 1;
        if child_handle(vpi::vpiHighConn, port).is_none() {
            s.ports_missing_high_conn += 1;
        }
        if child_handle(vpi::vpiLowConn, port).is_none() {
            s.ports_missing_low_conn += 1;
        }
    }

    s.nets += iter(vpi::vpiNet, mi).len();
    s.vars += iter(vpi::vpiVariables, mi).len();
    s.params += iter(vpi::vpiParameter, mi).len();

    // Parameter assignments: count how many RHS are not plain constants.
    for pa in iter(vpi::vpiParamAssign, mi) {
        s.param_assigns += 1;
        let rhs_is_constant = child_handle(vpi::vpiRhs, pa)
            .map(|h| vpi::obj_type(h.raw()) == vpi::vpiConstant)
            .unwrap_or(false);
        if !rhs_is_constant {
            s.unfolded_param_exprs += 1;
        }
    }

    s.cont_assigns += iter(vpi::vpiContAssign, mi).len();

    // Ranges: every net/var/param typespec range must fold to constants.
    for obj in iter(vpi::vpiNet, mi)
        .into_iter()
        .chain(iter(vpi::vpiVariables, mi))
        .chain(iter(vpi::vpiParameter, mi))
    {
        if let Some(ts) = child_handle(vpi::vpiTypespec, obj) {
            if let Some(range) = child_handle(vpi::vpiRange, ts.raw()) {
                let l = child_handle(vpi::vpiLeftRange, range.raw())
                    .map(|h| vpi::obj_type(h.raw()))
                    .unwrap_or(0);
                let r = child_handle(vpi::vpiRightRange, range.raw())
                    .map(|h| vpi::obj_type(h.raw()))
                    .unwrap_or(0);
                if l != vpi::vpiConstant && r != vpi::vpiConstant {
                    s.unfolded_ranges += 1;
                }
            }
        }
    }

    for proc in iter(vpi::vpiProcess, mi) {
        s.processes += 1;
        // initial/final blocks don't need a sensitivity list.  Surelog gives
        // every process vpiAlwaysType=1 (vpiAlways), so distinguish by the
        // object type (vpiInitial / vpiFinal) instead.
        let ptype = vpi::obj_type(proc);
        if ptype == vpi::vpiInitial || ptype == vpi::vpiFinal {
            continue;
        }
        // Explicit event controls (e.g. @(posedge clk)) carry a condition;
        // always_comb / always @* leave it empty.
        let mut has_condition = child_handle(vpi::vpiCondition, proc).is_some();
        if !has_condition {
            each_child(vpi::vpiStmt, proc, &mut |st| {
                if child_handle(vpi::vpiCondition, st).is_some()
                    || child_handle(vpi::vpiDelay, st).is_some()
                {
                    has_condition = true;
                }
            });
        }
        if !has_condition {
            s.implicit_sensitivity += 1;
        }
    }

    s.gen_scopes += iter(vpi::vpiGenScopeArray, mi).len();

    // Instance tree: children via vpiModule relationship.
    for child in iter(vpi::vpiModule, mi) {
        s.children.push(visit_module_inst(child));
    }

    s
}

fn print_tree(s: &InstStats, depth: usize, resolver: &mut elab::Resolver) {
    let ind = "  ".repeat(depth);
    // Top-level module_inst lacks vpiFullName in Surelog output; vpiName
    // holds the module name in that case.
    let name = if s.full_name.is_empty() {
        format!("({})", s.def_name)
    } else {
        s.full_name.clone()
    };
    println!(
        "{ind}- {name} [ports={} nets={} vars={} params={} param_assigns={} \
         cont_assigns={} processes={} gen_scopes={}]",
        s.ports,
        s.nets,
        s.vars,
        s.params,
        s.param_assigns,
        s.cont_assigns,
        s.processes,
        s.gen_scopes
    );
    if s.ports_missing_high_conn > 0 || s.ports_missing_low_conn > 0 {
        println!(
            "{ind}  !! ports missing high_conn={} low_conn={}",
            s.ports_missing_high_conn, s.ports_missing_low_conn
        );
    }
    if s.unfolded_ranges > 0 {
        println!(
            "{ind}  !! ranges not folded to constants: {}",
            s.unfolded_ranges
        );
    }
    if s.implicit_sensitivity > 0 {
        println!(
            "{ind}  !! processes w/o explicit sensitivity (derive from body): {}",
            s.implicit_sensitivity
        );
    }
    if s.unfolded_param_exprs > 0 {
        println!(
            "{ind}  !! param_assigns with unfolded RHS expression: {}",
            s.unfolded_param_exprs
        );
    }
    print_resolved_params(s.handle, &ind, resolver);
    for c in &s.children {
        print_tree(c, depth + 1, resolver);
    }
}

/// Print the resolved parameter values of an instance and of any gen scopes it
/// contains.  Resolution failures are reported inline and do not abort the walk.
fn print_resolved_params(handle: VpiHandle, ind: &str, resolver: &mut elab::Resolver) {
    let print_scope =
        |name: &str, scope: VpiHandle, ind: &str, resolver: &mut elab::Resolver| match resolver
            .scope_params(scope)
        {
            Ok(params) => {
                for (pname, val) in params {
                    println!("{ind}    param {name}{pname} = {}", val.format_verilog());
                }
            }
            Err(err) => println!("{ind}    !! param resolve failed ({name}): {err}"),
        };
    print_scope("", handle, ind, resolver);
    for gsa in iter(vpi::vpiGenScopeArray, handle) {
        for gs in iter(vpi::vpiGenScope, gsa) {
            print_scope("", gs, ind, resolver);
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: elab_check [surelog args...] file.sv");
        std::process::exit(2);
    }

    // Split off `-top <module>`; everything else goes through to Surelog
    // verbatim (files and any extra flags).
    let mut top: Option<String> = None;
    let mut rest: Vec<String> = Vec::new();
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        if a == "-top" {
            top = it.next();
        } else {
            rest.push(a);
        }
    }

    // Unified pipeline: parse + compile + elaborate + `-elabuhdm` with
    // structured diagnostics (see src/compile.rs).
    let out = match compile::compile_checked(&compile::CompileOpts {
        files: rest,
        top,
        ..Default::default()
    }) {
        Ok(out) => out,
        Err(compile::CompileError::SessionStart(e)) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
        Err(compile::CompileError::FrontendDiagnostics(diagnostics)) => {
            for d in &diagnostics {
                eprintln!(
                    "{:?}: {}:{}:{} {}",
                    d.severity,
                    d.file.as_deref().unwrap_or(""),
                    d.line,
                    d.col,
                    d.message
                );
            }
            eprintln!("surelog reported errors; aborting");
            std::process::exit(1);
        }
    };
    for diagnostic in &out.diagnostics {
        eprintln!(
            "{:?}: {}:{}:{} {}",
            diagnostic.severity,
            diagnostic.file.as_deref().unwrap_or(""),
            diagnostic.line,
            diagnostic.col,
            diagnostic.message
        );
    }

    // ── 1. Surelog Design API: instance tree ──────────────────────────────
    println!("== Surelog ModuleInstance tree ==");
    if let Some(d) = out.design() {
        for i in 0..d.top_instance_count() {
            if let Some(top) = d.top_instance(i) {
                print_surelog_inst(&top, 0);
            }
        }
    }

    // ── 2. UHDM VPI: elaborated model ─────────────────────────────────────
    let Some(design_h) = out.uhdm_design() else {
        eprintln!("no UHDM design handle");
        std::process::exit(1);
    };
    println!("\n== UHDM elaborated model ==");
    println!(
        "design vpiElaborated = {}",
        vpi::get(VPI_ELABORATED, design_h)
    );

    let mut total_refs = RefStats::default();
    let top_modules = iter(vpi::uhdmtopModules, design_h);
    println!("top modules: {}", top_modules.len());
    let mut resolver = elab::Resolver::new();
    for top in &top_modules {
        let stats = visit_module_inst(*top);
        print_tree(&stats, 0, &mut resolver);
        // Walk the whole tree for ref-binding stats.
        let mut stack = vec![*top];
        while let Some(h) = stack.pop() {
            count_refs(h, &mut total_refs);
            stack.extend(iter(vpi::vpiModule, h));
        }
    }

    println!(
        "\nref/select nodes: {} total, {} bound via vpiActual ({:.1}%)",
        total_refs.total,
        total_refs.bound,
        if total_refs.total > 0 {
            100.0 * total_refs.bound as f64 / total_refs.total as f64
        } else {
            0.0
        }
    );

    // `out` (and with it the surelog session) dropped here: compiler, clp,
    // errors and symbol_table are freed in the correct order.
}

fn print_surelog_inst(inst: &surelog::ModuleInstance<'_>, depth: usize) {
    let ind = "  ".repeat(depth);
    println!(
        "{ind}{} (file: {})",
        inst.full_path_name(),
        inst.file_path()
    );
    for i in 0..inst.child_count() {
        if let Some(child) = inst.child(i) {
            print_surelog_inst(&child, depth + 1);
        }
    }
}
