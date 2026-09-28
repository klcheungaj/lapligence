use llg::core::compile::{compile_sources_checked, CompileOpts, OwnedSource};
use llg::core::db::Db;
use llg::sim::codegen::{generate_from_db_with_codegen_options, CodegenOptions};
use llg::sim::execution::ExecutionAnalysisOptions;

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
fn coroutine_model_emits_root_arguments_polled_frames_and_descriptors() {
    let c = render_source_with_execution_options(
        "frame_calls.sv",
        FRAME_CALL_SOURCE,
        ExecutionAnalysisOptions::default(),
    );

    assert!(c.contains("llg_co_frame_t co;"), "{c}");
    assert!(c.contains("sv4_t a0;\n    int depth;"), "{c}");
    assert!(c.contains("fn_tb_leaf_frame_t c0;"), "{c}");
    assert!(c.contains("fn_tb_leaf(&F->u"), "{c}");
    assert!(c.contains(".calls.c0);"), "{c}");
    assert!(
        c.contains("&fn_tb_leaf_desc, offsetof(fn_tb_middle_frame_t, u"),
        "{c}"
    );
    assert!(c.contains("LLG_CO_ROOT_FRAME_OK(p_tb_proc_"), "{c}");
    assert!(c.contains("LLG_CO_ANCHORED_OK(fn_tb_leaf_frame_t)"), "{c}");
    assert!(
        c.contains("static const llg_co_desc_t fn_tb_leaf_desc = { NULL"),
        "{c}"
    );
}

#[test]
fn poll_depth_one_emits_an_anchored_direct_libaco_call() {
    let c = render_source_with_execution_options(
        "anchored_frame.sv",
        FRAME_CALL_SOURCE,
        ExecutionAnalysisOptions {
            poll_depth_max: 1,
            ..Default::default()
        },
    );

    assert!(c.contains("LLG_CO_ANCHORED(fn_tb_leaf_frame_t) a0;"), "{c}");
    assert!(c.contains("fn_tb_leaf(&F->u"), "{c}");
    assert!(c.contains(".calls.a0.f);"), "{c}");
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

    assert!(c.contains("void* _llg_arena_call_"), "{c}");
    assert!(c.contains("llg_co_arena_push(F->arena"), "{c}");
    assert!(c.contains("LLG_CO_ANCHOR_FRAME(F->u"), "{c}");
    assert!(c.contains("._llg_arena_call_"), "{c}");
    assert!(c.contains("llg_co_arena_pop(F->arena"), "{c}");
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
    assert!(
        c.contains("static void p_tb_proc_0(llg_proc_t* self)"),
        "{c}"
    );
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

    assert!(c.contains("llg_co_arena_push(F->arena"), "{c}");
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
        .arg(format!("-I{}", root.join("vendor/libaco").display()))
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
