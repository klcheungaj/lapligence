//! Strength-aware standalone wired-net resolution and explicit subset bounds.

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
            "hierarchical",
            "module tb; logic a; wand w; assign tb.w=a; endmodule",
            "hierarchical continuous assignment",
        ),
        (
            "concat_lhs",
            "module tb; logic x; wand w; assign {w,x}=2'b11; endmodule",
            "concatenated/complex continuous-assignment LHS",
        ),
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
        (
            "highz_strength",
            "module tb; logic a; wand w; assign (highz0, highz1) w=a; endmodule",
            "high impedance for both logic values",
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
fn mixed_structural_drivers_and_selected_cross_hierarchy_resolve_with_optimizer_parity() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let source = r#"// llg-test-fixture: tests/sim_net_resolution.rs/mixed_structural.sv
module source(
    input wire i,
    output wire out_wire,
    output wand out_wand,
    output wor out_wor
);
    assign out_wire = i;
    assign out_wand = i;
    assign out_wor = i;
endmodule

module selected_source(input wire en, output wire [7:0] out);
    assign out[3:0] = en ? 4'hf : 4'hz;
endmodule

module tb;
    reg drive, gate_drive, port_drive, en;
    wire w;
    wand wa;
    wor wo;
    tri0 t0;
    tri1 t1;
    supply0 s0;
    supply1 s1;
    wire [7:0] selected;

    assign w = drive;
    buf gw(w, gate_drive);
    assign wa = drive;
    buf ga(wa, gate_drive);
    assign wo = drive;
    buf go(wo, gate_drive);
    source u(.i(port_drive), .out_wire(w), .out_wand(wa), .out_wor(wo));
    assign t0 = 1'bz;
    assign t1 = 1'bz;
    assign s0 = 1'bz;
    assign s1 = 1'bz;
    assign selected[7:4] = 4'h0;
    selected_source us(.en(en), .out(selected));

    initial begin
        drive = 1'b1;
        gate_drive = 1'b0;
        port_drive = 1'b1;
        en = 1'b1;
        #1 $display("mixed=%b%b%b/%b%b%b%b", w, wa, wo, t0, t1, s0, s1);
        $display("selected=%h", selected);
        drive = 1'b0;
        gate_drive = 1'b1;
        port_drive = 1'b0;
        en = 1'b0;
        #1 $display("mixed=%b%b%b/%b%b%b%b", w, wa, wo, t0, t1, s0, s1);
        $display("selected=%h", selected);
        drive = 1'bz;
        gate_drive = 1'bz;
        port_drive = 1'bz;
        #1 $display("mixed=%b%b%b/%b%b%b%b", w, wa, wo, t0, t1, s0, s1);
        $finish(0);
    end
endmodule
"#;
    let expected = "mixed=x01/0101\nselected=0f\n\
mixed=x01/0101\nselected=00\n\
mixed=zzz/0101\n";
    sim_harness::with_frontend_temp_cwd("mixed_structural", |dir| {
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
    .expect("mixed structural-driver simulation");
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
    let expected = "first=1100\nsecond=001x\nreleased=zzz0\nforced=1\nrestored=0\n";
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
fn wired_nets_reject_more_than_sixteen_driver_sites() {
    let mut source = String::from(
        "// llg-test-fixture: tests/sim_net_resolution.rs/driver_limit.sv\nmodule tb; wand w;\n",
    );
    for _ in 0..17 {
        source.push_str("assign w = 1'b1;\n");
    }
    source.push_str("endmodule\n");
    let error = generate_error("wired_driver_limit", &source);
    assert!(
        error.contains("17 continuous driver sites") && error.contains("16 driver-slot limit"),
        "{error}"
    );
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
