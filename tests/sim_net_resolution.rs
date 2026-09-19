//! Strength-aware wired-net resolution, true-net aliases, and explicit bounds.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

use llg::core::{compile, db::Db};
use llg::ffi::slang::DiagnosticSeverity;
use llg::sim::{self, opt::OptConfig};
use std::path::Path;

fn generate_error(tag: &str, source: &str) -> String {
    sim_harness::with_frontend_temp_cwd(tag, |dir| {
        let path = dir.join("tb.sv");
        std::fs::write(&path, source).map_err(|error| error.to_string())?;
        let compiled = compile::compile_checked(&compile::CompileOpts {
            files: vec![path.to_string_lossy().into_owned()],
            top: Some("tb".to_owned()),
            ..Default::default()
        })
        .map_err(|error| error.to_string())?;
        let db = Db::from_slang(&compiled.snapshot).map_err(|error| error.to_string())?;
        match sim::codegen::generate(&db) {
            Ok(_) => Err("wired-net design unexpectedly generated".to_owned()),
            Err(error) => Ok(error.to_string()),
        }
    })
    .expect("wired-net rejection must reach codegen")
}

#[test]
fn wired_nets_resolve_each_continuous_driver_site_with_optimizer_parity() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let source = r#"// llg-test-fixture: tests/sim_net_resolution.rs/resolution.sv
module tb;
    logic a, b;
    logic [129:0] hi, lo;
    logic [15:0] matrix_a, matrix_b;

    wand repeated_and;
    assign repeated_and = a;
    assign repeated_and = b;
    triand declared_and = a;
    assign declared_and = b;

    wor repeated_or;
    assign repeated_or = a;
    assign repeated_or = b;
    trior declared_or = a;
    assign declared_or = b;

    wand and_zero, and_one, and_x, and_z;
    assign and_zero = 1'b0; assign and_zero = 1'bx;
    assign and_one = 1'b1;  assign and_one = 1'bz;
    assign and_x = 1'bx;    assign and_x = 1'b1;
    assign and_z = 1'bz;    assign and_z = 1'bz;
    wor or_one, or_zero, or_x, or_z;
    assign or_one = 1'b1; assign or_one = 1'bx;
    assign or_zero = 1'b0; assign or_zero = 1'bz;
    assign or_x = 1'bx; assign or_x = 1'b0;
    assign or_z = 1'bz; assign or_z = 1'bz;

    wand [129:0] wide_and;
    wor [129:0] wide_or;
    assign wide_and = hi; assign wide_and = lo;
    assign wide_or = hi;  assign wide_or = lo;
    wand [15:0] matrix_and;
    wor [15:0] matrix_or;
    assign matrix_and = matrix_a; assign matrix_and = matrix_b;
    assign matrix_or = matrix_a; assign matrix_or = matrix_b;
    wand [1023:0] max_and;
    wor [1023:0] max_or;
    assign max_and = '1; assign max_and = '1;
    assign max_or = '0; assign max_or = '0;

    initial begin
        a = 1; b = 1;
        hi = '1; lo = '1;
        lo[100] = 0;
        hi[65] = 1'bx; lo[65] = 0;
        hi[64] = 1'bx; lo[64] = 1;
        hi[63] = 1'bz; lo[63] = 1'bz;
        matrix_a = 16'bzzzzxxxx11110000;
        matrix_b = 16'bzx10zx10zx10zx10;
        #1;
        $display("matrix=%b%b%b%b/%b%b%b%b", and_zero, and_one, and_x, and_z,
                 or_one, or_zero, or_x, or_z);
        $display("sites=%b%b/%b%b", repeated_and, declared_and,
                 repeated_or, declared_or);
        $display("wide=%b%b%b%b/%b%b%b%b", wide_and[100], wide_and[65],
                 wide_and[64], wide_and[63], wide_or[100], wide_or[65],
                 wide_or[64], wide_or[63]);
        $display("allpairs=%b/%b", matrix_and, matrix_or);
        $display("max=%b%b%b/%b%b%b", max_and[1023], max_and[511], max_and[0],
                 max_or[1023], max_or[511], max_or[0]);
        a = 0; b = 1; #1;
        $display("known=%b%b/%b%b", repeated_and, declared_and,
                 repeated_or, declared_or);
        a = 1'bx; b = 0; #1;
        $display("xdom=%b%b/%b%b", repeated_and, declared_and,
                 repeated_or, declared_or);
        a = 1'bz; b = 0; #1;
        $display("zneutral=%b%b/%b%b", repeated_and, declared_and,
                 repeated_or, declared_or);
        $finish;
    end
endmodule
"#;
    let expected = "matrix=01xz/10xz\nsites=11/11\nwide=00xz/1x1z\n\
                    allpairs=zx10xxx01x100000/zx10xx1x11110x10\nmax=111/000\n\
                    known=00/11\nxdom=00/xx\nzneutral=00/00\n";
    sim_harness::with_frontend_temp_cwd("wired_resolution", |dir| {
        let path = dir.join("tb.sv");
        std::fs::write(&path, source).map_err(|error| error.to_string())?;
        let compiled = compile::compile_checked(&compile::CompileOpts {
            files: vec![path.to_string_lossy().into_owned()],
            top: Some("tb".to_owned()),
            ..Default::default()
        })
        .map_err(|error| error.to_string())?;
        let db = Db::from_slang(&compiled.snapshot).map_err(|error| error.to_string())?;
        for (variant, opts) in [("on", OptConfig::default()), ("off", OptConfig::none())] {
            let model = sim::codegen::generate_from_db_with_opts(&db, &opts)
                .map_err(|error| error.to_string())?;
            let exe =
                sim::build::build_model_cmake(&dir.join(variant), &[("model.c", &model.model_c)])
                    .map_err(|error| error.to_string())?;
            assert_eq!(sim_harness::run_executable(&exe)?, expected, "{variant}");
        }
        Ok(())
    })
    .expect("wired-net simulations");
}

#[test]
fn net_aliases_share_one_resolved_network_with_optimizer_parity() {
    sim_cli::run_case(
        "net_resolution",
        "true_net_alias",
        "CHECK: initial=11\nCHECK: forced=00\nCHECK: released=11\n",
        "",
        &[],
    );
}

#[test]
fn forced_true_net_alias_reacts_to_alias_rhs_changes_with_optimizer_parity() {
    sim_cli::run_case(
        "net_resolution",
        "force_alias_rhs",
        "CHECK: force_rhs=11\nCHECK: force_rhs=00\nCHECK: force_rhs=00\n",
        "",
        &[],
    );
}

#[test]
fn selected_net_aliases_preserve_bit_order_with_optimizer_parity() {
    sim_cli::run_case(
        "net_resolution",
        "selected_alias",
        "CHECK: selected=1010/zzzz1010\nCHECK: selected_force=1100/11001100\nCHECK: selected_release=1010/zzzz1010\nCHECK: selected_part_force=0110/zzzz0110\nCHECK: selected_part_release=1010/zzzz1010\nCHECK: selected_bit_force=1000/zzzz1000\nCHECK: selected_bit_release=1010/zzzz1010\n",
        "",
        &[],
    );
}

#[test]
fn indexed_net_aliases_preserve_selected_bit_order() {
    sim_cli::run_case(
        "net_resolution",
        "indexed_alias",
        "CHECK: indexed=1010/1010\n",
        "",
        &[],
    );
}

#[test]
fn true_net_alias_changes_wake_alias_dependencies() {
    sim_cli::run_case(
        "net_resolution",
        "sensitivity_alias",
        "CHECK: alias_event=0\nCHECK: alias_event=1\n",
        "",
        &[],
    );
}

#[test]
fn net_aliases_preserve_declaration_continuous_drivers() {
    sim_cli::run_case(
        "net_resolution",
        "declaration_alias",
        "CHECK: declaration=11\n",
        "",
        &[],
    );
}

#[test]
fn concatenated_net_aliases_share_drivers_with_optimizer_parity() {
    sim_cli::run_case(
        "net_resolution",
        "concat_alias",
        "CHECK: concat=1001/1001\n",
        "",
        &[],
    );
}

#[test]
fn gate_drives_both_true_net_alias_names_with_optimizer_parity() {
    sim_cli::run_case(
        "net_resolution",
        "gate_alias",
        "CHECK: gate=11\nCHECK: gate=00\n",
        "",
        &[],
    );
}

#[test]
fn multi_output_gate_true_net_aliases_keep_independent_driver_slots() {
    sim_cli::run_case(
        "net_resolution",
        "multi_output_gate_alias",
        "CHECK: multi_gate=11\nCHECK: multi_gate=00\n",
        "",
        &[],
    );
}

#[test]
fn inout_port_true_net_aliases_collapse_into_one_network_with_optimizer_parity() {
    sim_cli::run_case(
        "net_resolution",
        "inout_alias",
        "CHECK: inout=11\n",
        "",
        &[],
    );
}

#[test]
fn input_port_true_net_aliases_use_link_driver_with_optimizer_parity() {
    sim_cli::run_case(
        "net_resolution",
        "input_alias",
        "CHECK: input=11\n",
        "",
        &[],
    );
}

#[test]
fn ascending_range_net_aliases_preserve_logical_order_with_optimizer_parity() {
    sim_cli::run_case(
        "net_resolution",
        "ascending_alias",
        "CHECK: ascending=1010/1010\n",
        "",
        &[],
    );
}

#[test]
fn output_port_from_true_net_alias_uses_alias_value_with_optimizer_parity() {
    sim_cli::run_case(
        "net_resolution",
        "output_alias",
        "CHECK: output=11\n",
        "",
        &[],
    );
}

#[test]
fn conflicting_true_net_alias_drivers_resolve_to_unknown() {
    sim_cli::run_case(
        "net_resolution",
        "conflicting_alias",
        "CHECK: conflict=xx\n",
        "",
        &[],
    );
}

#[test]
fn invalid_true_net_alias_width_is_rejected() {
    sim_cli::reject_case(
        "net_resolution",
        "invalid_alias_width",
        "all aliased nets must have the same width",
    );
}

#[test]
fn ordinary_wire_selected_continuous_drivers_resolve_with_optimizer_parity() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/net_resolution/selected_continuous_wire.v");
    sim_harness::with_frontend_temp_cwd("selected_continuous_wire", |dir| {
        let source = dir.join("selected_continuous_wire.v");
        std::fs::copy(&fixture, &source).map_err(|error| format!("copy fixture: {error}"))?;
        let compiled = compile::compile_checked(&compile::CompileOpts {
            files: vec![source.to_string_lossy().into_owned()],
            top: Some("tb".to_owned()),
            ..Default::default()
        })
        .map_err(|error| format!("compile fixture: {error}"))?;
        let db = Db::from_slang(&compiled.snapshot).map_err(|error| error.to_string())?;
        for (variant, options) in [
            ("unoptimized", OptConfig::none()),
            ("optimized", OptConfig::default()),
        ] {
            let model = sim::codegen::generate_from_db_with_opts(&db, &options)
                .map_err(|error| format!("{variant} lowering: {error}"))?;
            let executable = sim::build::build_model_cmake(
                &dir.join(variant),
                &[("model.c", model.model_c.as_str())],
            )
            .map_err(|error| format!("{variant} C model build: {error}"))?;
            assert_eq!(
                sim_harness::run_executable(&executable)?,
                "PASS selected_continuous_wire\n",
                "{variant}"
            );
        }
        Ok(())
    })
    .expect("selected ordinary-wire continuous assignments");
}

#[test]
fn ordinary_wire_continuous_assignment_rejects_variable_target_select() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/net_resolution/variable_selected_continuous_wire.sv");
    let source = std::fs::read_to_string(&fixture).expect("read variable-select fixture");
    let diagnostics =
        sim_harness::frontend_diagnostics(&source, "tb").expect("compile variable target select");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.severity == DiagnosticSeverity::Error
                && diagnostic.name == "ConstEvalNonConstVariable"
        }),
        "variable target select must report ConstEvalNonConstVariable: {diagnostics:?}"
    );
}

#[test]
fn wired_nets_reject_unimplemented_driver_paths() {
    let lowering_cases = [
        (
            "interface",
            "interface bus; wand w; endinterface module tb; bus b(); endmodule",
            "declared in an interface",
        ),
        (
            "array",
            "module tb; wand w[0:1]; assign w[0]=1'b1; endmodule",
            "unpacked wired-net array",
        ),
        (
            "strength_vector",
            "module tb; logic [1:0] a; wand [1:0] w; assign (strong0, strong1) w=a; endmodule",
            "drive strength on non-scalar net",
        ),
    ];
    for (tag, source, expected) in lowering_cases {
        let source =
            format!("// llg-test-fixture: tests/sim_net_resolution.rs/{tag}.sv\n{source}\n");
        let error = generate_error(&format!("wired_{tag}"), &source);
        assert!(error.contains(expected), "{tag}: {error}");
    }

    let frontend_cases = [
        (
            "blocking",
            "module tb; logic a; wand w; initial w=a; endmodule",
            "AssignToNet",
        ),
        (
            "nba",
            "module tb; logic a; wand w; initial w<=a; endmodule",
            "AssignToNet",
        ),
        (
            "pca",
            "module tb; logic a; wand w; initial assign w=a; endmodule",
            "BadProceduralAssign",
        ),
        (
            "task_output",
            "module tb; wand w; task t(output logic x); x=1; endtask initial t(w); endmodule",
            "AssignToNet",
        ),
        (
            "function_output",
            "module tb; wand w; function automatic logic f(output logic x); begin x=1; f=0; end endfunction initial $display(f(w)); endmodule",
            "AssignToNet",
        ),
        (
            "highz_strength",
            "module tb; logic a; wand w; assign (highz0, highz1) w=a; endmodule",
            "DriveStrengthHighZ",
        ),
    ];
    for (tag, source, expected_name) in frontend_cases {
        let diagnostics =
            sim_harness::frontend_diagnostics(source, "tb").expect("compile invalid net write");
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic.severity == DiagnosticSeverity::Error && diagnostic.name == expected_name
            }),
            "{tag}: expected {expected_name}: {diagnostics:?}"
        );
    }
}

#[test]
fn hierarchical_and_concatenated_wired_lhs_resolve_driver_sites() {
    // A resolved hierarchical LHS (top-self or a selected net in a child) is a
    // structural driver site, not a procedural write; each contribution keeps
    // its own resolved slot and strength. A concatenated LHS that contains a
    // wired net contributes only that net's bits; its other bits stay high-Z.
    sim_cli::run_case(
        "net_resolution",
        "hierarchical_wired_lhs",
        "CHECK: self=1 child=01 concat=10zz cv=10 cx=1\n\
         CHECK: self=0 child=10 concat=10zz cv=10 cx=1\n\
         CHECK: self=z child=1z concat=10zz cv=10 cx=1\n",
        "",
        &[],
    );
}

#[test]
fn mixed_structural_drivers_and_selected_cross_hierarchy_resolve_with_optimizer_parity() {
    sim_cli::run_case(
        "net_resolution",
        "mixed_structural",
        "mixed=x01/0101\nselected=0f\nmixed=x01/0101\nselected=0z\nmixed=zzz/0101\n",
        "",
        &[],
    );
}

#[test]
fn output_port_net_strength_survives_parent_net_resolution() {
    sim_cli::run_case(
        "net_resolution",
        "port_strength_inout",
        "low=1\nhigh=1\nreleased=x\n",
        "",
        &[],
    );
}

#[test]
fn mixed_structural_drivers_cover_biased_nets_with_optimizer_parity() {
    sim_cli::run_case(
        "net_resolution",
        "mixed_biased_structural",
        "first=xxx01\nagree=11101\nreleased=z0101\n",
        "",
        &[],
    );
}

#[test]
fn port_cycle_does_not_refire_a_resolved_net_for_equal_driver_values() {
    sim_cli::run_case(
        "net_resolution",
        "port_cycle",
        "cycle=0/events=1\n",
        "",
        &[],
    );
}

#[test]
fn unequal_strength_gate_wired_and_collapsed_inout_drivers_resolve_with_optimizer_parity() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let source = r#"// llg-test-fixture: tests/sim_net_resolution.rs/strength_structural.sv
module child(inout wire p, input wire d);
    assign (weak0, weak1) p = d;
endmodule

module tb;
    logic a, b;
    wire w;
    wand wa;
    tri0 pulled;
    wire collapsed;
    wand forced;

    assign (pull0, pull1) w = a;
    buf (strong0, strong1) gw(w, b);
    assign (strong0, strong1) wa = a;
    buf (pull0, pull1) ga(wa, b);
    assign (strong0, strong1) collapsed = a;
    child c(.p(collapsed), .d(b));
    assign (pull0, pull1) pulled = a;
    assign (strong0, strong1) forced = a;

    initial begin
        a = 0; b = 1; #1;
        $display("first=%b%b%b%b", w, wa, collapsed, pulled);
        a = 1; b = 0; #1;
        $display("second=%b%b%b%b", w, wa, collapsed, pulled);
        a = 1'bz; b = 1'bz; #1;
        $display("released=%b%b%b%b", w, wa, collapsed, pulled);
        force forced = 1'b1;
        #1;
        $display("forced=%b", forced);
        a = 1'b0;
        #1;
        release forced;
        $display("restored=%b", forced);
        $finish;
    end
endmodule
"#;
    let expected = "first=1000\nsecond=011x\nreleased=zzz0\nforced=1\nrestored=0\n";
    sim_harness::with_frontend_temp_cwd("strength_structural", |dir| {
        let path = dir.join("tb.sv");
        std::fs::write(&path, source).map_err(|error| error.to_string())?;
        let compiled = compile::compile_checked(&compile::CompileOpts {
            files: vec![path.to_string_lossy().into_owned()],
            top: Some("tb".to_owned()),
            ..Default::default()
        })
        .map_err(|error| error.to_string())?;
        let db = Db::from_slang(&compiled.snapshot).map_err(|error| error.to_string())?;
        for (variant, opts) in [
            ("unoptimized", OptConfig::none()),
            ("optimized", OptConfig::default()),
        ] {
            let model = sim::codegen::generate_from_db_with_opts(&db, &opts)
                .map_err(|error| format!("{variant} lowering: {error}"))?;
            let executable = sim::build::build_model_cmake(
                &dir.join(variant),
                &[("model.c", model.model_c.as_str())],
            )
            .map_err(|error| format!("{variant} C model build: {error}"))?;
            assert_eq!(
                sim_harness::run_executable(&executable)?,
                expected,
                "{variant}"
            );
        }
        Ok(())
    })
    .expect("strength-aware structural-driver simulation");
}

#[test]
fn wired_nets_resolve_more_than_sixteen_continuous_driver_sites() {
    // 18 structural drivers per net. The retired 16-slot runtime ceiling must
    // not reject legal net connectivity; resolution stays strength-aware.
    sim_cli::run_case(
        "net_resolution",
        "driver_growth",
        "CHECK: w=x wa=0 wo=1\n",
        "",
        &[],
    );
}

#[test]
fn net_resolution_truth_matrix() {
    // Exhaustive {0,1,x,z} x {0,1,x,z} table over wire, wand, wor, and a
    // strong/weak pair. The expected rows are derived from the LRM resolution
    // tables, not from the implementation under test.
    sim_cli::run_case(
        "net_resolution",
        "truth_matrix",
        "0 0 | 0 0 0 0\n0 1 | x 0 1 0\n0 x | x 0 x 0\n0 z | 0 0 0 0\n\
         1 0 | x 0 1 1\n1 1 | 1 1 1 1\n1 x | x x 1 1\n1 z | 1 1 1 1\n\
         x 0 | x 0 x x\nx 1 | x x 1 x\nx x | x x x x\nx z | x x x x\n\
         z 0 | 0 0 0 0\nz 1 | 1 1 1 1\nz x | x x x x\nz z | z z z z\n",
        "",
        &[],
    );
}

#[test]
fn net_disjoint_array_ports() {
    sim_cli::run_case(
        "net_resolution",
        "disjoint_array_ports",
        "CHECK: a5 3c 5c\nCHECK: 00 3c 0c\nCHECK: 00 f0 00\n",
        "",
        &[],
    );
}

#[test]
fn unresolved_net_multiple_drivers() {
    sim_cli::run_case(
        "net_resolution",
        "unresolved_multiple_drivers",
        "CHECK: conflict=x wired=0\nCHECK: conflict=1 wired=1\nCHECK: conflict=z wired=z\n",
        "",
        &[],
    );
}

#[test]
fn alias_partial_chain() {
    sim_cli::run_case(
        "net_resolution",
        "alias_partial_chain",
        "CHECK: a=fa b=a c=2\nCHECK: a=f5 b=5 c=1\nCHECK: a=f3 b=3 c=3\n",
        "",
        &[],
    );
}

#[test]
fn alias_force_and_strength() {
    sim_cli::run_case(
        "net_resolution",
        "alias_force_and_strength",
        "CHECK: base=11\nCHECK: forced=00\nCHECK: released=11\n",
        "",
        &[],
    );
}

#[test]
fn alias_bad_type_or_edition() {
    sim_cli::reject_case(
        "net_resolution",
        "alias_incompatible_type",
        "all nets in a net alias statement must have a common nettype",
    );
    sim_cli::reject_case_with_args(
        "net_resolution",
        "true_net_alias",
        "use of undeclared identifier 'alias'",
        &["--edition", "2001"],
    );
}

#[test]
fn inout_selected_or_array_actual_is_a_retained_boundary() {
    // A 1-bit inout port whose actual is a fixed-array element (`lane[0]`) is
    // legal SV, but the whole-net inout collapse only tracks plain-net
    // actuals. Retain the precise lowering diagnostic and assign the
    // bit-level array-element collapse to a follow-up instead of silently
    // accepting it; the diagnostic distinguishes this shape from a syntax
    // error.
    sim_cli::reject_case(
        "net_resolution",
        "inout_array_element",
        "has a selected or concatenated actual that cannot be resolved safely",
    );
}

#[test]
fn net_alias_chain_grows_past_the_old_driver_and_alias_limits() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    // A 258-net alias chain collapses into one canonical electrical network:
    // 259 structural driver slots and 257 alias descriptors bound to the same
    // group. This exercises both retired per-net ceilings (16 drivers, 256
    // aliases) through checked growth rather than duplicate alias statements,
    // which the frontend correctly rejects.
    let n = 258usize;
    let mut source = String::from(
        "// llg-test-fixture: tests/sim_net_resolution.rs/alias_chain.sv\nmodule tb;\nwire n0",
    );
    for i in 1..n {
        source.push_str(&format!(", n{i}"));
    }
    source.push_str(";\n");
    for i in 0..n - 1 {
        source.push_str(&format!("alias n{i} = n{};\n", i + 1));
    }
    source.push_str(&format!(
        "assign n0 = 1'b1;\ninitial begin #1; $display(\"CHECK: last=%b\", n{}); $finish(0); end\nendmodule\n",
        n - 1
    ));
    let expected = "CHECK: last=1\n";
    sim_harness::with_frontend_temp_cwd("alias_chain", |dir| {
        let path = dir.join("tb.sv");
        std::fs::write(&path, &source).map_err(|error| error.to_string())?;
        let compiled = compile::compile_checked(&compile::CompileOpts {
            files: vec![path.to_string_lossy().into_owned()],
            top: Some("tb".to_owned()),
            ..Default::default()
        })
        .map_err(|error| error.to_string())?;
        let db = Db::from_slang(&compiled.snapshot).map_err(|error| error.to_string())?;
        for (variant, opts) in [("on", OptConfig::default()), ("off", OptConfig::none())] {
            let model = sim::codegen::generate_from_db_with_opts(&db, &opts)
                .map_err(|error| error.to_string())?;
            let exe =
                sim::build::build_model_cmake(&dir.join(variant), &[("model.c", &model.model_c)])
                    .map_err(|error| error.to_string())?;
            assert_eq!(sim_harness::run_executable(&exe)?, expected, "{variant}");
        }
        Ok(())
    })
    .expect("alias registry growth");
}

#[test]
fn homogeneous_wired_inout_ports_preserve_resolution() {
    sim_cli::run_case(
        "regression_81",
        "wired_inout_resolution",
        "resolved=11\nresolved=01\nresolved=zz\nresolved=x1\nresolved=0x\n",
        "",
        &[],
    );
}

#[test]
fn force_release_of_a_variable_does_not_target_an_unrelated_wired_net() {
    sim_cli::run_case(
        "regression_81",
        "force_unrelated_to_wired_net",
        "forced=1 wired=1\nreleased=1 wired=1\nassigned=0\n",
        "",
        &[],
    );
}

#[test]
fn force_release_selected_collapsed_inout_preserves_underlying_drivers() {
    sim_cli::run_case(
        "net_resolution",
        "force_selected_inout",
        "base=0011\nforced=0000\nunderlying=zz00\nlower_release=zz0z\nall_release=zzzz\n",
        "",
        &[],
    );
}
