//! End-to-end simulator tests for inline SystemVerilog `for` declarations and
//! `foreach` over mixed packed/unpacked arrays and resizable containers.
//!
//! Coverage includes lexical shadowing, nested loops, ascending and descending
//! array ranges, multidimensional traversal, break/continue behavior, and
//! optimizer parity. The Slang compilation uses the shared serialized temporary
//! CWD harness because the frontend writes process-global artifacts.

use crate::sim_cli;
use crate::sim_harness;

use llg::core::{compile, db};
use llg::ffi::slang::DiagnosticSeverity;
use llg::sim;
use llg::sim::opt::OptConfig;

fn run_variants(source: &str, tag: &str) -> Result<(String, String), String> {
    sim_harness::with_frontend_temp_cwd(tag, |dir| {
        let source_path = dir.join("tb.sv");
        std::fs::write(&source_path, source).map_err(|error| format!("write source: {error}"))?;
        let compiled = compile::compile_checked(&compile::CompileOpts {
            files: vec![source_path.to_string_lossy().into_owned()],
            top: Some("tb".to_owned()),
            ..Default::default()
        })
        .map_err(|error| format!("compile: {error}"))?;
        let database =
            db::Db::from_slang(&compiled.snapshot).map_err(|error| format!("db: {error}"))?;
        let optimized = sim::codegen::generate_from_db_with_opts(&database, &OptConfig::default())
            .map_err(|error| format!("optimized codegen: {error}"))?;
        let unoptimized = sim::codegen::generate_from_db_with_opts(&database, &OptConfig::none())
            .map_err(|error| format!("unoptimized codegen: {error}"))?;

        let run = |name: &str, model: &str| -> Result<String, String> {
            let executable = sim::build::build_model_cmake(&dir.join(name), &[("model.c", model)])
                .map_err(|error| format!("cmake({name}): {error}"))?;
            sim_harness::run_executable(&executable)
                .map_err(|error| format!("run({name}): {error}"))
        };
        Ok((
            run("optimized", &optimized.model_c)?,
            run("unoptimized", &unoptimized.model_c)?,
        ))
    })
}

#[test]
fn inline_for_and_foreach_preserve_loop_scope_and_control_flow() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let source = r#"`timescale 1ns/1ps
module tb;
    integer i;
    integer inline_sum;
    integer foreach_sum;
    integer edge_count;
    int descending [3:1];
    int matrix [-1:1][2:3];
    int max_index [2147483647:2147483647];
    int min_index [-2147483648:-2147483648];

    function automatic integer shadow_count;
        begin : fn_body
            integer n;
            n = 0;
            for (int i = 0; i < 2; i++) begin : inner
                integer i;
                i = 10;
                n++;
            end
            shadow_count = n;
        end
    endfunction

    initial begin
        i = 41;
        inline_sum = 0;
        for (int i = 0; i < 5; i++) begin
            if (i == 1) continue;
            for (int i = 0; i < 4; i++) begin
                if (i == 2) break;
                inline_sum += i;
            end
            inline_sum += 10 * i;
        end

        foreach_sum = 0;
        foreach (descending[i]) begin
            if (i == 2) continue;
            foreach_sum = foreach_sum * 10 + i;
        end
        foreach (matrix[row, col]) begin
            if (row == 0) continue;
            if (row == 1 && col == 3) break;
            foreach_sum += (row + 2) * 10 + col;
        end

        edge_count = 0;
        foreach (max_index[idx]) begin
            edge_count++;
            if (edge_count == 2) break;
        end
        foreach (min_index[idx]) begin
            edge_count++;
            if (edge_count == 3) break;
        end

        $display("outer=%0d inline=%0d foreach=%0d edge=%0d shadow=%0d",
                 i, inline_sum, foreach_sum, edge_count, shadow_count());
        $finish;
    end
endmodule
"#;

    let (optimized, unoptimized) =
        run_variants(source, "sim_loops").expect("both loop variants should run");
    assert_eq!(optimized, "outer=41 inline=94 foreach=88 edge=2 shadow=2\n");
    assert_eq!(unoptimized, optimized);
}

#[test]
fn inline_loop_variable_fork_capture_is_independent() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let source = r#"module tb;
initial begin
    for (int i = 0; i < 2; i++) fork
        $display("%0d", i);
    join
end
endmodule
"#;
    let (optimized, unoptimized) =
        run_variants(source, "loop_fork_capture").expect("fork capture should run");
    assert_eq!(optimized, "0\n1\n");
    assert_eq!(unoptimized, optimized);
}

#[test]
fn inline_loop_variable_strobe_capture_is_rejected() {
    let source = r#"module tb;
initial begin
    for (int i = 0; i < 1; i++) $strobe("%0d", i);
    #1 $finish;
end
endmodule
"#;
    let diagnostics =
        sim_harness::frontend_diagnostics(source, "tb").expect("compile strobe capture");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.severity == DiagnosticSeverity::Error && diagnostic.name == "AutoVarTraced"
        }),
        "automatic strobe argument must report AutoVarTraced: {diagnostics:?}"
    );
}

#[test]
fn foreach_omissions_containers_and_real_locals_follow_source_order() {
    sim_cli::run_case(
        "loops",
        "foreach_extended",
        "first=345 count=3 last=21 omitted=17 dynamic=15 queue=24 assoc=159 string_assoc=123 remaining=0 real=3.000000 shadow=221.000000 control=2.000000 fn=3 capture=3.000000\n",
        "llg: $finish at time 0 at tb:124:9\n",
        &[],
    );
}

#[test]
fn foreach_mixed_order_reads_and_writes_every_logical_dimension() {
    sim_cli::run_case_with_args(
        "loops",
        "foreach_mixed_order",
        concat!(
            "0:3 0:2 0:1 0:0 1:3 1:2 1:1 1:0 ones=5\n",
            "-1:3:2:0 -1:3:2:1 -1:3:1:0 -1:3:1:1 ",
            "-1:2:2:0 -1:2:2:1 -1:2:1:0 -1:2:1:1 ",
            "0:3:2:0 0:3:2:1 0:3:1:0 0:3:1:1 ",
            "0:2:2:0 0:2:2:1 0:2:1:0 0:2:1:1 visits=16\n",
            "readback=8 words=0101,0101,0101,0101\n",
        ),
        "",
        &[],
        &["--edition", "sv2009"],
    );
}

#[test]
fn foreach_mixed_omissions_keep_original_dimension_positions() {
    sim_cli::run_case_with_args(
        "loops",
        "foreach_mixed_omissions",
        "middle=40 leading=8 trailing=5 prefix=5 omitted=17 errors=0\n",
        "",
        &[],
        &["--edition", "sv2009"],
    );
}

#[test]
fn foreach_mixed_control_keeps_source_loop_jumps_and_signed_endpoints() {
    sim_cli::run_case_with_args(
        "loops",
        "foreach_mixed_control",
        "1:2 1:0 1:-1 0:2 visits=4 outer=55 nested=68 endpoints=7\n",
        "",
        &[],
        &["--edition", "sv2009"],
    );
}

#[test]
fn foreach_mixed_calls_use_formal_and_automatic_local_dimensions() {
    sim_cli::run_case_with_args(
        "loops",
        "foreach_mixed_calls",
        "source=1010,0100 result=0101,1011 counts=3,5 local=8\n",
        "",
        &[],
        &["--edition", "sv2009"],
    );
}

#[test]
fn foreach_mixed_types_keep_integer_record_enum_and_singleton_dimensions() {
    sim_cli::run_case_with_args(
        "loops",
        "foreach_mixed_types",
        "bytes=32 integers=64 records=8 enums=12 enum_indices=30 scalars=2 singletons=2 packed=4 data=aaaa,aaaa\n",
        "",
        &[],
        &["--edition", "sv2009"],
    );
}

#[test]
fn foreach_mixed_ports_preserve_formal_bounds_and_combinational_reads() {
    sim_cli::run_case_with_args(
        "loops",
        "foreach_mixed_ports",
        "sum=66\nsum=45\nsum=69\n",
        "",
        &[],
        &["--edition", "sv2009"],
    );
}

#[test]
fn foreach_mixed_too_many_iterators_remain_illegal() {
    sim_cli::reject_case_with_args(
        "loops",
        "foreach_mixed_too_many",
        "too many loop variables",
        &["--edition", "sv2009"],
    );
}

#[test]
fn foreach_mixed_scalar_elements_do_not_create_an_extra_dimension() {
    sim_cli::reject_case_with_args(
        "loops",
        "foreach_mixed_scalar_extra",
        "too many loop variables",
        &["--edition", "sv2009"],
    );
}

#[test]
fn foreach_mixed_iterators_remain_readonly() {
    sim_cli::reject_case_with_args(
        "loops",
        "foreach_mixed_readonly",
        "cannot assign to read-only variable",
        &["--edition", "sv2009"],
    );
}

#[test]
fn syn_037_finite_control_preserves_local_targets_and_copyout() {
    sim_cli::run_case_with_args(
        "loops",
        "syn_037_finite_control",
        concat!(
            "for=24 repeat=5 while=13 do=5 foreach=21 named=306 duplicate=11 ",
            "function=104 task=11 endpoints=2\n",
        ),
        "",
        &[],
        &["--edition", "sv2009"],
    );
}

#[test]
fn syn_037_verilog_2001_local_disable_preserves_loop_and_copyout() {
    for edition in ["v2001", "sv2009"] {
        sim_cli::run_case_with_args(
            "loops",
            "syn_037_finite_control_2001",
            "value=31 body=4 repeat=4 while=8 forever=3 function=3 task=6\n",
            "",
            &[],
            &["--edition", edition],
        );
    }
}

#[test]
fn syn_037_function_steps_preserve_order_copyout_and_automatic_owners() {
    sim_cli::run_case_with_args(
        "loops",
        "syn_037_function_steps",
        concat!(
            "steps=3 audit=123 copy=13 sum=3 discarded=2 calls=3 shadow=99 ",
            "task=4 function=25\n",
        ),
        "",
        &[],
        &["--edition", "sv2009"],
    );
}

#[test]
fn syn_037_task_calls_are_not_admitted_as_function_steps() {
    sim_cli::reject_case_with_args(
        "loops",
        "syn_037_task_step_rejected",
        "requires a function call, not a task",
        &["--edition", "sv2009"],
    );
}
