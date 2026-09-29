use llg::core::compile::{compile_sources_checked, CompileOpts, OwnedSource};
use llg::core::db::Db;
use llg::sim::codegen::{generate_from_db_with_codegen_options, CodegenOptions};
use llg::sim::execution::ExecutionAnalysisOptions;
use llg::sim::opt::OptConfig;

#[path = "support/sim.rs"]
mod sim_harness;

fn render_source_with_execution_options(
    name: &str,
    source: &str,
    execution: ExecutionAnalysisOptions,
) -> String {
    let compiled = compile_sources_checked(
        &[OwnedSource::compilation_unit(name, source)],
        &CompileOpts {
            top: Some("tb".to_owned()),
            ..Default::default()
        },
    )
    .expect("valid in-memory frame fixture");
    let database = Db::from_slang(&compiled.snapshot).expect("owned database for frame fixture");
    generate_from_db_with_codegen_options(
        &database,
        &CodegenOptions {
            execution,
            ..Default::default()
        },
    )
    .expect("frame fixture emits C")
    .model_c
}

fn render_fixture(relative: &str) -> String {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let path = root.join("tests/fixtures/sim").join(relative);
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    render_source_with_execution_options(relative, &source, ExecutionAnalysisOptions::default())
}

const FRAME_CALL_SOURCE: &str = r#"
module tb;
    task automatic leaf(input integer n);
        #1;
    endtask
    task automatic middle(input integer n);
        leaf(n);
    endtask
    initial middle(1);
endmodule
"#;

#[test]
fn resume_free_process_uses_a_header_only_frame_and_c_locals() {
    let c = render_source_with_execution_options(
        "resume_free_frame.sv",
        r#"
module tb;
    initial begin
        automatic integer left = 20;
        automatic integer right = 22;
        $display("%0d", left + right);
    end
endmodule
"#,
        ExecutionAnalysisOptions::default(),
    );

    assert!(
        c.contains("typedef struct {\n    llg_co_frame_t co;\n} p_tb_proc_0_frame_t;"),
        "{c}"
    );
    assert!(!c.contains("F->_llg_"), "{c}");
    assert!(
        c.contains("llg_value_scope_t* _llg_frame_base = llg_value_scope_mark();"),
        "{c}"
    );
    assert!(c.matches("sv4_t* _llg_local_").count() >= 2, "{c}");
}

#[test]
fn resume_free_nested_scope_uses_locals_beside_a_live_frame_field() {
    let c = render_source_with_execution_options(
        "narrow_nested_scope.sv",
        r#"
module tb;
    initial begin
        automatic integer across_wait = 1;
        begin
            automatic integer leaf = 2;
            across_wait = leaf;
        end
        #1;
        $display("%0d", across_wait);
    end
endmodule
"#,
        ExecutionAnalysisOptions::default(),
    );

    assert!(c.matches("sv4_t* _llg_local_").count() >= 2, "{c}");
    assert!(c.contains("F->_llg_local_"), "{c}");
}

#[test]
fn coroutine_model_emits_root_arguments_polled_frames_and_descriptors() {
    let c = render_source_with_execution_options(
        "frame_calls.sv",
        FRAME_CALL_SOURCE,
        ExecutionAnalysisOptions::default(),
    );

    assert!(c.contains("llg_co_frame_t co;"), "{c}");
    assert!(c.contains("sv4_t a0;\n    int depth;"), "{c}");
    assert!(c.contains("fn_tb_leaf_frame_t c0;"), "{c}");
    assert!(c.contains("LLG_CO_CALL(co, ch,"), "{c}");
    assert!(c.contains("fn_tb_leaf, &F->"), "{c}");
    assert!(
        c.contains("&fn_tb_leaf_desc, offsetof(fn_tb_middle_frame_t, "),
        "{c}"
    );
    assert!(c.contains("LLG_CO_ROOT_FRAME_OK(p_tb_proc_"), "{c}");
    assert!(c.contains("LLG_CO_ANCHORED_OK(fn_tb_leaf_frame_t)"), "{c}");
    assert!(
        c.contains("static const llg_co_desc_t fn_tb_leaf_desc = { fn_tb_leaf"),
        "{c}"
    );
}

#[test]
fn poll_depth_one_emits_an_anchored_stackless_call() {
    let c = render_source_with_execution_options(
        "anchored_frame.sv",
        FRAME_CALL_SOURCE,
        ExecutionAnalysisOptions {
            poll_depth_max: 1,
            ..Default::default()
        },
    );

    assert!(c.contains("LLG_CO_ANCHORED(fn_tb_leaf_frame_t) a0;"), "{c}");
    assert!(c.contains("LLG_CO_CALL_ANCHOR(co, ch,"), "{c}");
    assert!(c.contains("&fn_tb_leaf_desc, &F->"), "{c}");
    assert!(
        !c.contains("&fn_tb_leaf_desc, offsetof(fn_tb_middle_frame_t"),
        "{c}"
    );
}

#[test]
fn recursive_timing_task_uses_the_process_arena() {
    let c = render_source_with_execution_options(
        "recursive_frame.sv",
        r#"
module tb;
    task automatic recurse(input integer n);
        if (n) begin
            #1;
            recurse(n - 1);
        end
    endtask
    initial recurse(2);
endmodule
"#,
        ExecutionAnalysisOptions::default(),
    );

    assert!(c.contains("llg_co_anchor_t* _llg_arena_call_"), "{c}");
    assert!(
        c.contains("LLG_CO_ARENA_ENTER(ch, &fn_tb_recurse_desc"),
        "{c}"
    );
    assert!(c.contains("LLG_CO_ANCHOR_FRAME(F->"), "{c}");
    assert!(c.contains("LLG_CO_CALL_ARENA(co, ch,"), "{c}");
    assert!(!c.contains("F->arena"), "{c}");
    assert!(c.contains("if (F->depth >= 256)"), "{c}");
}

#[test]
fn inline_event_task_storage_is_hoisted_into_its_host_frame() {
    let c = render_source_with_execution_options(
        "inline_event_frame.sv",
        r#"
module tb;
    event wake;
    task automatic await_event(event ev);
        integer local_value;
        @(ev);
        local_value = 1;
    endtask
    initial await_event(wake);
endmodule
"#,
        ExecutionAnalysisOptions::default(),
    );

    assert!(!c.contains("fn_tb_await_event_frame_t"), "{c}");
    assert!(c.contains("sv4_t* _llg_local_"), "{c}");
    assert!(c.contains("LLG_CO_ROOT_FRAME_OK(p_tb_proc_"), "{c}");
}

#[test]
fn final_blocks_keep_c_stack_storage() {
    let c = render_source_with_execution_options(
        "final_stack.sv",
        r#"
module tb;
    final begin
        integer value = 1;
        $display("%0d", value);
    end
endmodule
"#,
        ExecutionAnalysisOptions::default(),
    );

    assert!(!c.contains("p_tb_proc_0_frame_t"), "{c}");
    assert!(c.contains("static void p_tb_proc_0(void)"), "{c}");
    assert!(c.contains("llg_spawn_final(p_tb_proc_0"), "{c}");
}

#[test]
fn concurrent_assertion_actions_register_their_root_descriptors() {
    let c = render_source_with_execution_options(
        "assertion_action_frame.sv",
        r#"
module tb;
    logic clk;
    always #1 clk = ~clk;
    initial begin
        clk = 1'b0;
        #2 $finish;
    end
    check: assert property (@(posedge clk) 1'b1)
        $display("PASS");
endmodule
"#,
        ExecutionAnalysisOptions::default(),
    );

    let registration = c
        .lines()
        .find(|line| line.contains("llg_assertion_register("))
        .expect("concurrent assertion registration");
    assert!(registration.contains("_desc"), "{registration}");
    assert!(!registration.contains("llg_libaco_desc"), "{registration}");
}

#[test]
fn frames_above_the_embed_limit_are_forced_to_the_arena() {
    let c = render_source_with_execution_options(
        "oversized_frame.sv",
        FRAME_CALL_SOURCE,
        ExecutionAnalysisOptions {
            embed_limit: 1,
            ..Default::default()
        },
    );

    assert!(c.contains("LLG_CO_ARENA_ENTER(ch, &fn_tb_leaf_desc"), "{c}");
    assert!(!c.contains("fn_tb_leaf_frame_t c0;"), "{c}");
    assert!(
        !c.contains("LLG_CO_ANCHORED(fn_tb_leaf_frame_t) a0;"),
        "{c}"
    );
}

#[test]
fn generated_coroutines_pass_gcc_jump_initialization_check() {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let Ok(version) = Command::new("gcc").arg("--version").output() else {
        eprintln!("SKIP: gcc not available");
        return;
    };
    if !version.status.success() {
        eprintln!("SKIP: gcc not available");
        return;
    }
    let c = render_source_with_execution_options(
        "jump_initialization.sv",
        FRAME_CALL_SOURCE,
        ExecutionAnalysisOptions {
            poll_depth_max: 1,
            ..Default::default()
        },
    );
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut child = Command::new("gcc")
        .args([
            "-x",
            "c",
            "-std=c11",
            "-O2",
            "-Wall",
            "-Wno-unused-function",
            "-Werror=jump-misses-init",
            "-fsyntax-only",
            "-",
        ])
        .arg(format!("-I{}", root.join("src/sim/rt").display()))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start GCC coroutine-frame check");
    child
        .stdin
        .as_mut()
        .expect("GCC stdin")
        .write_all(c.as_bytes())
        .expect("write generated model to GCC");
    let output = child.wait_with_output().expect("wait for GCC frame check");
    assert!(
        output.status.success(),
        "generated coroutine model failed -Werror=jump-misses-init:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn suspension_sites_emit_the_reviewed_one_shot_arms() {
    let mut emitted = render_source_with_execution_options(
        "basic_arms.sv",
        r#"
module tb;
    logic a, b, c;
    event first, second;
    initial begin
        #1;
        @(posedge a);
        @(a);
        @(a or b);
        @(first);
        @(first or second);
        @(a or first);
    end
    always @(a or b) b = a;
    assign c = a & b;
endmodule
"#,
        ExecutionAnalysisOptions::default(),
    );
    for fixture in [
        "coroutine_semantics/event_waits.sv",
        "partial_features/clocking_h14_zero.sv",
        "function/timed_task_fork_joins.sv",
        "process_control/control.sv",
        "semaphore/suspend.sv",
        "mailboxes/blocking.sv",
        "concurrent_assertions/expect.sv",
        "partial_features/stop_resume.sv",
    ] {
        emitted.push_str(&render_fixture(fixture));
    }

    for arm in [
        "llg_arm_time(",
        "llg_arm_any(",
        "llg_arm_any_dependencies(",
        "llg_arm_any_events(",
        "llg_arm_edge(",
        "llg_arm_event(",
        "llg_arm_events(",
        "llg_arm_event_triggered(",
        "llg_arm_assertion(",
        "llg_arm_order(",
        "llg_arm_mixed(",
        "llg_arm_clocking_cycle(",
        "llg_arm_join(",
        "llg_arm_wait_fork(",
        "llg_arm_process_suspend(",
        "llg_arm_process_await(",
        "llg_arm_semaphore_get(",
        "llg_arm_mailbox_put_value(",
        "llg_arm_mailbox_get_value(",
        "llg_arm_stop(",
    ] {
        assert!(emitted.contains(arm), "missing generated arm {arm}");
    }
    assert_eq!(
        emitted.matches("LLG_CO_AWAIT(co, ch,").count(),
        emitted.matches("llg_arm_").count(),
        "every emitted arm must have exactly one numbered await"
    );
}

#[test]
fn coroutine_and_plain_termination_checks_use_their_abi_forms() {
    let c = render_source_with_execution_options(
        "termination_checks.sv",
        r#"
module tb;
    function automatic integer plain_finish(input integer value);
        if (value) $finish;
        plain_finish = value;
    endfunction
    task automatic timed_finish;
        #1;
        $finish;
    endtask
    initial begin
        integer value = plain_finish(0);
        timed_finish();
    end
endmodule
"#,
        ExecutionAnalysisOptions::default(),
    );

    let plain = &c[c
        .find("static sv4_t fn_tb_plain_finish(sv4_t a0, int depth) {")
        .unwrap()..];
    let plain = &plain[..plain.find("\n}\n").unwrap()];
    assert!(plain.contains("llg_rt_finish_with_level("), "{plain}");
    assert!(
        plain.contains("llg_rt_exiting())) goto _llg_return;"),
        "{plain}"
    );

    let timed = &c[c
        .find("static llg_co_status_t fn_tb_timed_finish(llg_co_frame_t* co, llg_co_chain_t* ch) {")
        .unwrap()..];
    let timed = &timed[..timed.find("\n}\n").unwrap()];
    assert!(timed.contains("LLG_CO_EXIT_CHECK(ch);"), "{timed}");
    assert!(timed.contains("return LLG_CO_DONE;"), "{timed}");
}

#[test]
fn loop_budget_points_propagate_exit_in_both_function_shapes() {
    let c = render_source_with_execution_options(
        "budget_checks.sv",
        r#"
module tb;
    function automatic integer plain_loop(input integer limit);
        integer i;
        plain_loop = 0;
        for (i = 0; i < limit; i++) plain_loop += i;
    endfunction
    task automatic timed_loop(input integer limit);
        integer i;
        for (i = 0; i < limit; i++) #1;
    endtask
    initial begin
        integer value = plain_loop(2);
        timed_loop(value);
    end
endmodule
"#,
        ExecutionAnalysisOptions::default(),
    );

    let plain = &c[c
        .find("static sv4_t fn_tb_plain_loop(sv4_t a0, int depth) {")
        .unwrap()..];
    let plain = &plain[..plain.find("\n}\n").unwrap()];
    assert!(
        plain.contains("llg_budget_point(\"fn_tb_plain_loop\")"),
        "{plain}"
    );
    assert!(plain.contains("goto _llg_return;"), "{plain}");

    let timed = &c[c
        .find("static llg_co_status_t fn_tb_timed_loop(llg_co_frame_t* co, llg_co_chain_t* ch) {")
        .unwrap()..];
    let timed = &timed[..timed.find("\n}\n").unwrap()];
    assert!(
        timed.contains("llg_budget_point(\"fn_tb_timed_loop\")"),
        "{timed}"
    );
    assert!(timed.contains("return LLG_CO_EXIT;"), "{timed}");
}

fn deep_overlay_source(depth: usize) -> String {
    let mut source = String::from("module tb;\ninteger total;\ninitial begin\n    total = 0;\n");
    for _ in 0..depth {
        source.push_str("    begin\n");
    }
    source.push_str(
        "    automatic integer deepest = 70;\n    #1;\n    total += deepest;\n    if (1) begin\n        automatic integer left = 3;\n        #1;\n        total += left;\n    end else begin\n        automatic integer unused_left = 30;\n        #1;\n        total += unused_left;\n    end\n    if (0) begin\n        automatic integer unused_right = 40;\n        #1;\n        total += unused_right;\n    end else begin\n        automatic integer right = 4;\n        #1;\n        total += right;\n    end\n",
    );
    for _ in 0..depth {
        source.push_str("    end\n");
    }
    source.push_str("    $display(\"PASS %0d\", total);\nend\nendmodule\n");
    source
}

fn assert_strict_c11(model: &str) {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut child = Command::new("gcc")
        .args([
            "-x",
            "c",
            "-std=c11",
            "-Wall",
            "-Wextra",
            "-pedantic",
            "-Werror",
            "-fsyntax-only",
            "-",
        ])
        .arg(format!("-I{}", root.join("src/sim/rt").display()))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start strict C11 frame check");
    child
        .stdin
        .as_mut()
        .expect("strict C11 stdin")
        .write_all(model.as_bytes())
        .expect("write strict C11 model");
    let output = child.wait_with_output().expect("wait for strict C11 check");
    assert!(
        output.status.success(),
        "deep overlay model failed strict C11 compilation:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

const REPEATED_INSTANCES_SOURCE: &str = r#"
module tb;
    logic clk = 0;
    logic [3:0] d = 4'b1011;
    logic [3:0] q;
    real r0 = 1.5;
    real r1 = 2.5;
    genvar i;
    for (i = 0; i < 4; i = i + 1) begin : g
        always @(posedge clk) q[i] <= d[i];
    end
    initial begin
        #1 clk = 1;
        #1 $display("q=%b r=%0.1f", q, r0 + r1);
        $finish;
    end
endmodule
"#;

/// Every coroutine's `F` cast names a frame type that has exactly one typedef.
fn assert_frame_casts_resolve(c: &str) {
    for line in c.lines() {
        if let Some((ty, _)) = line.trim().split_once("* F = (") {
            assert_eq!(
                c.matches(&format!("\n}} {ty};\n")).count(),
                1,
                "frame type `{ty}` must be defined exactly once"
            );
        }
    }
}

#[test]
fn repeated_instances_share_frames_and_table_driven_startup_runs() {
    std::thread::Builder::new()
        .name("repeated-instances".to_owned())
        .spawn(run_repeated_instances)
        .expect("spawn repeated-instance test")
        .join()
        .expect("repeated-instance test panicked");
}

fn run_repeated_instances() {
    let compiled = compile_sources_checked(
        &[OwnedSource::compilation_unit(
            "repeated_instances.sv",
            REPEATED_INSTANCES_SOURCE,
        )],
        &CompileOpts {
            top: Some("tb".to_owned()),
            ..Default::default()
        },
    )
    .expect("repeated-instance fixture compiles");
    let database = Db::from_slang(&compiled.snapshot).expect("repeated-instance database");

    sim_harness::with_temp_cwd("repeated-instances", |directory| {
        for (variant, options) in [
            ("default", OptConfig::default()),
            ("no-opt", OptConfig::none()),
        ] {
            let c = llg::sim::codegen::generate_from_db_with_opts(&database, &options)
                .map_err(|error| format!("{variant} codegen: {error}"))?
                .model_c;
            // The four generated `always` instances share one frame typedef.
            let typedefs = c
                .matches("typedef struct {\n    llg_co_frame_t co;")
                .count();
            let descriptors = c.matches("static const llg_co_desc_t p_").count();
            assert!(
                typedefs < descriptors,
                "{variant}: {typedefs} typedefs for {descriptors} coroutines\n{c}"
            );
            assert!(c.contains("llg_shared_frame_0_t"), "{variant}:\n{c}");
            assert_frame_casts_resolve(&c);
            // Plain static storage and spawns are table-driven loops.
            assert!(
                c.contains("static sv4_t* const llg_storage_"),
                "{variant}:\n{c}"
            );
            assert!(c.contains("sv4_destroy(llg_storage_"), "{variant}:\n{c}");
            assert!(
                c.contains("static double* const llg_storage_"),
                "{variant}:\n{c}"
            );
            assert!(
                c.contains("llg_spawn_in_region(llg_model_startup_"),
                "{variant}:\n{c}"
            );
            assert_strict_c11(&c);
            let executable = llg::sim::build::build_model_cmake(
                &directory.join(variant),
                &[("model.c", c.as_str())],
            )
            .map_err(|error| format!("{variant} build: {error}"))?;
            let output = sim_harness::run_executable(&executable)?;
            if output != "q=1011 r=4.0\n" {
                return Err(format!("{variant}: unexpected output {output:?}"));
            }
        }
        Ok::<(), String>(())
    })
    .expect("repeated-instance fixture runs");
}

#[test]
fn deep_single_child_blocks_compile_and_run_in_both_modes() {
    std::thread::Builder::new()
        .name("deep-overlay".to_owned())
        .stack_size(32 * 1024 * 1024)
        .spawn(run_deep_single_child_blocks)
        .expect("spawn deep overlay test")
        .join()
        .expect("deep overlay test panicked");
}

fn run_deep_single_child_blocks() {
    const DEPTH: usize = 70;
    let source = deep_overlay_source(DEPTH);
    let compiled = compile_sources_checked(
        &[OwnedSource::compilation_unit("deep_overlay.sv", &source)],
        &CompileOpts {
            top: Some("tb".to_owned()),
            ..Default::default()
        },
    )
    .expect("deep overlay fixture compiles");
    let database = Db::from_slang(&compiled.snapshot).expect("deep overlay database");

    sim_harness::with_temp_cwd("deep-overlay", |directory| {
        for (variant, options) in [
            ("default", OptConfig::default()),
            ("no-opt", OptConfig::none()),
        ] {
            let generated = llg::sim::codegen::generate_from_db_with_opts(&database, &options)
                .map_err(|error| format!("{variant} codegen: {error}"))?;
            assert_strict_c11(&generated.model_c);
            let executable = llg::sim::build::build_model_cmake(
                &directory.join(variant),
                &[("model.c", generated.model_c.as_str())],
            )
            .map_err(|error| format!("{variant} build: {error}"))?;
            let output = sim_harness::run_executable(&executable)?;
            if output != "PASS 77\n" {
                return Err(format!(
                    "{variant}: expected {:?}, got {output:?}",
                    "PASS 77\n"
                ));
            }
        }
        Ok::<(), String>(())
    })
    .expect("deep overlay fixture runs");
}
