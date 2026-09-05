//! End-to-end simulator tests for inline SystemVerilog `for` declarations and
//! `foreach` over fixed unpacked arrays.
//!
//! Coverage includes lexical shadowing, nested loops, ascending and descending
//! array ranges, multidimensional traversal, break/continue behavior, and
//! optimizer parity. Surelog compilation uses the shared serialized temporary
//! CWD harness because the frontend writes process-global artifacts.

#[path = "support/sim.rs"]
mod sim_harness;

use llg::core::{compile, db};
use llg::sim;
use llg::sim::opt::OptConfig;

fn run_variants(source: &str, tag: &str) -> Result<(String, String), String> {
    sim_harness::with_surelog_temp_cwd(tag, |dir| {
        let source_path = dir.join("tb.sv");
        std::fs::write(&source_path, source).map_err(|error| format!("write source: {error}"))?;
        let compiled = compile::compile_checked(&compile::CompileOpts {
            files: vec![source_path.to_string_lossy().into_owned()],
            top: Some("tb".to_owned()),
            ..Default::default()
        })
        .map_err(|error| format!("compile: {error}"))?;
        let design = compiled.uhdm_design().ok_or("no UHDM design")?;
        let database = db::Db::build(design).map_err(|error| format!("db: {error}"))?;
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

fn codegen_error(source: &str, tag: &str) -> Result<String, String> {
    sim_harness::with_surelog_temp_cwd(tag, |dir| {
        let source_path = dir.join("tb.sv");
        std::fs::write(&source_path, source).map_err(|error| error.to_string())?;
        let compiled = compile::compile_checked(&compile::CompileOpts {
            files: vec![source_path.to_string_lossy().into_owned()],
            top: Some("tb".to_owned()),
            ..Default::default()
        })
        .map_err(|error| error.to_string())?;
        let design = compiled.uhdm_design().ok_or("no UHDM design")?;
        match sim::codegen::generate(design) {
            Ok(_) => Err("codegen unexpectedly succeeded".to_owned()),
            Err(error) => Ok(error.to_string()),
        }
    })
}

#[test]
fn inline_loop_variable_fork_capture_is_rejected() {
    let error = codegen_error(
        r#"module tb;
initial begin
    for (int i = 0; i < 2; i++) fork
        $display("%0d", i);
    join
end
endmodule
"#,
        "loop_fork_capture",
    )
    .expect("fork capture should reach codegen rejection");
    assert!(
        error.contains("fork branch capture of inline loop variable `i`"),
        "{error}"
    );
}

#[test]
fn inline_loop_variable_strobe_capture_is_rejected() {
    let error = codegen_error(
        r#"module tb;
initial begin
    for (int i = 0; i < 1; i++) $strobe("%0d", i);
    #1 $finish;
end
endmodule
"#,
        "loop_strobe_capture",
    )
    .expect("strobe capture should reach codegen rejection");
    assert!(
        error.contains("$strobe cannot defer a reference to inline loop variable `i`"),
        "{error}"
    );
}
