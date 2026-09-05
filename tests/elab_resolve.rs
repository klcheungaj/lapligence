//! Integration tests for the parameter resolver (`elab`), driving the full
//! Surelog parse→compile→elaborate→`-elabuhdm` pipeline on small designs.
//!
//! Surelog writes `slpp_all/` into the process working directory, so every
//! test runs with `std::env::set_current_dir` pointing at a fresh temp dir.
//! That state is process-global, so all tests are serialized through a mutex
//! and the assertions run inside a closure that keeps the Surelog session (and
//! therefore the UHDM design handle) alive.

use std::collections::HashMap;
use std::sync::Mutex;

use llg::core::elab;
use llg::ffi::{surelog, vpi};

static SURELOG_LOCK: Mutex<()> = Mutex::new(());

/// Content of the `param_top` test design.
const PARAMS_SV: &str = r#"module param_child #(
    parameter int W = 4,
    parameter logic [3:0] INIT = 4'h0
) (
    output logic [W-1:0] o
);
    localparam int W2 = W + 1;
    localparam logic [W2-1:0] MASK = '1;
    localparam int DEPTH = 1 << W;
    localparam logic [7:0] EX = {2{INIT}};
    localparam int CB = $clog2(DEPTH);
    localparam logic [W2-1:0] SEL = W2 * 2;
    localparam logic [3:0] XZ = 4'b10xz;
    localparam int SNEG = -3;
    localparam int SNEG2 = SNEG + 1;
    assign o = INIT;
endmodule

module param_top (
    output logic [7:0] o0,
    output logic [15:0] o1
);
    param_child #(.W(8), .INIT(4'h5)) u0 (.o(o0));
    param_child #(.W(16), .INIT(4'ha)) u1 (.o(o1));
endmodule
"#;

/// Cyclic parameters.  `parameter int A = B + 1; parameter int B = A + 1;`
/// overflows Surelog's own evaluator at elaboration, so the direct-ref form
/// (`A = B; B = A`) is used instead: Surelog survives it (reporting
/// EL0542 "Expression loop" errors) and still emits a UHDM design.
const CYCLE_SV: &str = r#"module cyc ();
    parameter int A = B;
    parameter int B = A;
endmodule

module cyc_top ();
    cyc u ();
endmodule
"#;

/// Unsized fill literals.
const FILL_SV: &str = r#"module fill_mod ();
    localparam logic [7:0] F = '1;
    localparam logic [7:0] G = 'x;
endmodule

module fill_top ();
    fill_mod u ();
endmodule
"#;

/// Run `f` with the elaborated UHDM design handle for `src` (top module
/// `top`), then clean up.  The design handle is only valid while `f` runs.
fn with_design<R>(
    top: &str,
    src: &str,
    check_errors: bool,
    f: impl for<'session> FnOnce(vpi::VpiHandle<'session>) -> R,
) -> R {
    let _guard = SURELOG_LOCK.lock().unwrap();
    let dir = std::env::temp_dir().join(format!("llg_elab_test_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    std::fs::write(dir.join("test.sv"), src).expect("write source");
    let orig_cwd = std::env::current_dir().expect("current dir");
    std::env::set_current_dir(&dir).expect("chdir to temp dir");

    let symbol_table = surelog::create_symbol_table();
    let errors = surelog::create_error_container(&symbol_table);
    let clp = surelog::create_command_line_parser(&errors, &symbol_table);
    clp.no_python();
    clp.set_parse();
    clp.set_write_pp_output();
    clp.set_compile();
    clp.set_elaborate();
    clp.set_elab_uhdm();

    let file = dir.join("test.sv");
    let args = [
        "elab_test".to_string(),
        "-top".to_string(),
        top.to_string(),
        file.to_string_lossy().into_owned(),
    ];
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let success = clp.parse_command_line(&arg_refs);
    errors.print_messages(1);
    let compiler = clp
        .start_compiler()
        .expect("Surelog failed to create compiler");
    assert!(success && !clp.help(), "Surelog command line rejected");
    if check_errors {
        assert_eq!(errors.fatal_count(), 0, "fatal errors during compile");
        assert_eq!(errors.syntax_count(), 0, "syntax errors during compile");
        assert_eq!(errors.error_count(), 0, "errors during compile");
    }
    let design = compiler
        .get_uhdm_design()
        .expect("no UHDM design after elaboration");
    let result = f(design);
    drop(compiler);
    std::env::set_current_dir(&orig_cwd).expect("restore cwd");
    let _ = std::fs::remove_dir_all(&dir);
    result
}

/// Resolve all parameters of instance `inst` into a `(name, val)` map.
fn resolve(design: vpi::VpiHandle<'_>, inst: &str) -> HashMap<String, elab::Val> {
    let h = find_instance(design, inst);
    let mut resolver = elab::Resolver::new();
    resolver
        .scope_params(h)
        .expect("resolve parameters")
        .into_iter()
        .collect()
}

fn collect_instances<'session>(design: vpi::VpiHandle<'session>) -> Vec<vpi::VpiHandle<'session>> {
    let mut out = Vec::new();
    if let Some(tops) = vpi::iterate(vpi::uhdmtopModules, design) {
        for top in tops {
            let mut stack = vec![top];
            while let Some(h) = stack.pop() {
                out.push(h);
                if let Some(children) = vpi::iterate(vpi::vpiModule, h) {
                    stack.extend(children);
                }
            }
        }
    }
    out
}

fn find_instance<'session>(
    design: vpi::VpiHandle<'session>,
    name: &str,
) -> vpi::VpiHandle<'session> {
    collect_instances(design)
        .into_iter()
        .find(|h| vpi::obj_name(*h) == name)
        .unwrap_or_else(|| panic!("instance {name} not found"))
}

/// Assert a parameter is a clean integer with the given value and width.
fn assert_bits(params: &HashMap<String, elab::Val>, name: &str, value: u64, width: usize) {
    let val = params
        .get(name)
        .unwrap_or_else(|| panic!("missing param {name}"));
    let elab::Val::Bits(v) = val else {
        panic!("param {name} is a string, expected bits");
    };
    assert_eq!(v.width(), width, "width of {name}");
    assert_eq!(
        v.to_u64().unwrap_or_else(|| panic!("{name} has X/Z")),
        value,
        "value of {name}"
    );
}

/// Assert a parameter is a clean signed integer with the given value.
fn assert_signed(params: &HashMap<String, elab::Val>, name: &str, value: i64) {
    let val = params
        .get(name)
        .unwrap_or_else(|| panic!("missing param {name}"));
    let elab::Val::Bits(v) = val else {
        panic!("param {name} is a string, expected bits");
    };
    assert_eq!(
        v.to_i64().unwrap_or_else(|| panic!("{name} has X/Z")),
        value,
        "value of {name}"
    );
}

#[test]
fn resolves_param_top_instances() {
    with_design("param_top", PARAMS_SV, true, |design| {
        let u0 = resolve(design, "u0");
        assert_bits(&u0, "W", 8, 32);
        assert_bits(&u0, "INIT", 5, 4);
        assert_bits(&u0, "W2", 9, 32);
        assert_bits(&u0, "MASK", 0x1FF, 9);
        assert_bits(&u0, "DEPTH", 256, 32);
        assert_bits(&u0, "EX", 0x55, 8);
        assert_bits(&u0, "CB", 8, 32);
        assert_bits(&u0, "SEL", 18, 9);
        assert_signed(&u0, "SNEG", -3);
        assert_signed(&u0, "SNEG2", -2);
        let xz = u0.get("XZ").expect("XZ param");
        match xz {
            elab::Val::Bits(v) => {
                assert!(v.is_unknown(), "XZ must contain unknown bits");
                assert_eq!(v.format_verilog(), "4'h10xz");
                assert_eq!(v.bit_lsb(0), elab::Bit::Z);
                assert_eq!(v.bit_lsb(1), elab::Bit::X);
                assert_eq!(v.bit_lsb(2), elab::Bit::Zero);
                assert_eq!(v.bit_lsb(3), elab::Bit::One);
            }
            _ => panic!("XZ must be bits"),
        }

        let u1 = resolve(design, "u1");
        assert_bits(&u1, "W", 16, 32);
        assert_bits(&u1, "INIT", 10, 4);
        assert_bits(&u1, "W2", 17, 32);
        assert_bits(&u1, "MASK", 0x1FFFF, 17);
        assert_bits(&u1, "DEPTH", 65536, 32);
        assert_bits(&u1, "EX", 0xAA, 8);
        assert_bits(&u1, "CB", 16, 32);
        assert_bits(&u1, "SEL", 34, 17);
        assert_signed(&u1, "SNEG", -3);
        assert_signed(&u1, "SNEG2", -2);
    });
}

#[test]
fn cycle_detected() {
    // check_errors = false: Surelog reports EL0542 "Expression loop" errors
    // but still emits the UHDM design, which is exactly what we want to probe.
    with_design("cyc_top", CYCLE_SV, false, |design| {
        let h = find_instance(design, "u");
        let mut resolver = elab::Resolver::new();
        let result = resolver.scope_params(h);
        assert!(
            result.is_err(),
            "cyclic parameters must fail to resolve, got {} params",
            result.map(|r| r.len()).unwrap_or(0)
        );
    });
}

#[test]
fn unsized_fill_literals() {
    with_design("fill_top", FILL_SV, true, |design| {
        let params = resolve(design, "u");
        match params.get("F").expect("F param") {
            elab::Val::Bits(v) => {
                assert_eq!(v.width(), 8);
                assert_eq!(v.to_u64(), Some(0xFF), "F = '1 should be all ones");
            }
            _ => panic!("F must be bits"),
        }
        match params.get("G").expect("G param") {
            elab::Val::Bits(v) => {
                assert_eq!(v.width(), 8);
                assert!(v.is_unknown(), "G = 'x must be all X");
                assert!(v.bits.iter().all(|b| *b == elab::Bit::X), "G must be all X");
            }
            _ => panic!("G must be bits"),
        }
    });
}
