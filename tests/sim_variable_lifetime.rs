//! Procedural variable lifetime regressions over the Slang-owned database.

#[path = "support/sim.rs"]
mod sim_harness;

use llg::core::{compile::{self, LanguageEdition}, db::Db};
use llg::sim::{self, opt::OptConfig};

fn run_variants(source: &str) -> Result<Vec<(String, String)>, String> {
    run_variants_with_edition(source, LanguageEdition::SystemVerilog2009)
}

fn run_variants_with_edition(
    source: &str,
    edition: LanguageEdition,
) -> Result<Vec<(String, String)>, String> {
    sim_harness::with_frontend_temp_cwd("variable-lifetime", |dir| {
        let source_path = dir.join("variable_lifetime.sv");
        std::fs::write(&source_path, source).map_err(|error| format!("write source: {error}"))?;
        let compiled = compile::compile_checked(&compile::CompileOpts {
            files: vec![source_path.to_string_lossy().into_owned()],
            top: Some("tb".to_owned()),
            edition,
            ..Default::default()
        })
        .map_err(|error| format!("compile: {error}"))?;
        let database =
            Db::from_slang(&compiled.snapshot).map_err(|error| format!("database: {error}"))?;

        [
            ("unoptimized", OptConfig::none()),
            ("optimized", OptConfig::default()),
        ]
        .into_iter()
        .map(|(name, options)| {
            let generated = sim::codegen::generate_from_db_with_opts(&database, &options)
                .map_err(|error| format!("{name} lowering: {error}"))?;
            let executable = sim::build::build_model_cmake(
                &dir.join(name),
                &[("model.c", generated.model_c.as_str())],
            )
            .map_err(|error| format!("{name} C model build: {error}"))?;
            let output = sim_harness::run_executable(&executable)?;
            Ok((name.to_owned(), output))
        })
        .collect()
    })
}

#[test]
fn static_and_automatic_block_locals_follow_resolved_lifetime() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    // IEEE 1800-2009 §6.21: a static variable exists for the duration of
    // simulation; an automatic variable is created on each block entry.
    let source = r#"module tb;
    int iteration = 0;

    always begin
        static int retained = 0;
        automatic int fresh = 0;
        int inherited;
        if (iteration == 0)
            inherited = 20;
        retained = retained + 1;
        fresh = fresh + 1;
        inherited = inherited + 1;
        iteration = iteration + 1;
        $display(
            "iteration=%0d retained=%0d fresh=%0d inherited=%0d",
            iteration,
            retained,
            fresh,
            inherited
        );
        if (iteration == 3)
            $finish;
        #1;
    end
endmodule
"#;
    let expected = concat!(
        "iteration=1 retained=1 fresh=1 inherited=21\n",
        "iteration=2 retained=2 fresh=1 inherited=22\n",
        "iteration=3 retained=3 fresh=1 inherited=23\n",
    );
    let variants = run_variants(source).expect("procedural lifetime model should execute");
    for (name, output) in variants {
        assert_eq!(output, expected, "{name} procedural lifetime output");
    }
}

#[test]
fn static_and_automatic_locals_survive_repeat_reentry() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    // IEEE 1800-2009 §6.21 applies the same lifetime rules when a repeat
    // statement reenters its body without advancing simulation time.
    let source = r#"module tb;
    initial begin
        repeat (3) begin
            static int retained = 4;
            automatic int fresh = 4;
            retained = retained + 1;
            fresh = fresh + 1;
            $display("retained=%0d fresh=%0d", retained, fresh);
        end
        $finish;
    end
endmodule
"#;
    let expected = concat!(
        "retained=5 fresh=5\n",
        "retained=6 fresh=5\n",
        "retained=7 fresh=5\n",
    );
    let variants = run_variants(source).expect("repeat lifetime model should execute");
    for (name, output) in variants {
        assert_eq!(output, expected, "{name} repeat lifetime output");
    }
}

#[test]
fn mixed_subroutine_lifetimes_are_reentrant() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    // IEEE 1800-2009 §§6.21 and 13.4.2: an explicit static local remains
    // shared in an automatic routine, while an automatic local is recreated
    // for each recursive activation.
    let source = r#"module tb;
    function automatic integer recurse(input integer depth);
        static integer shared = 0;
        automatic integer fresh = 0;
        shared = shared + 1;
        fresh = fresh + 1;
        if (depth == 0)
            recurse = shared * 100 + fresh;
        else
            recurse = recurse(depth - 1);
    endfunction

    integer first;
    integer second;

    initial begin
        first = recurse(1);
        second = recurse(0);
        $display("first=%0d second=%0d", first, second);
        $finish;
    end
endmodule
"#;
    let variants = run_variants(source).expect("mixed subroutine lifetime model should execute");
    for (name, output) in variants {
        assert_eq!(output, "first=201 second=301\n", "{name} mixed lifetime output");
    }
}

#[test]
fn automatic_delay_tasks_keep_explicit_static_locals() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    // A delay-bearing automatic task is inlined, but its explicit static
    // local still belongs to the elaborated task instance rather than an
    // inline activation.
    let source = r#"module tb;
    task automatic tick;
        static integer shared = 0;
        automatic integer fresh = 0;
        shared = shared + 1;
        fresh = fresh + 1;
        #1;
        $display("shared=%0d fresh=%0d", shared, fresh);
    endtask

    initial begin
        tick();
        tick();
        #1;
        $finish;
    end
endmodule
"#;
    let variants = run_variants(source).expect("mixed delay-task lifetime model should execute");
    for (name, output) in variants {
        assert_eq!(
            output,
            "shared=1 fresh=1\nshared=2 fresh=1\n",
            "{name} delay-task lifetime output"
        );
    }
}

#[test]
fn static_block_locals_are_distinct_per_module_instance() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    // IEEE 1800-2009 §6.21: static procedural storage is per elaborated
    // module instance even when the declaration node is shared by both.
    let source = r#"module child #(parameter integer ID = 0);
    initial begin
        static integer count = 0;
        count = count + 1;
        $display("child=%0d count=%0d", ID, count);
    end
endmodule

module tb;
    child #(.ID(0)) first();
    child #(.ID(1)) second();
    initial begin
        #1;
        $finish;
    end
endmodule
"#;
    let variants = run_variants(source).expect("per-instance block lifetime model should execute");
    for (name, output) in variants {
        assert_eq!(
            output,
            "child=0 count=1\nchild=1 count=1\n",
            "{name} per-instance static output"
        );
    }
}

#[test]
fn declaration_initializers_follow_source_order() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    // IEEE 1800-2009 §§6.8 and 10.5: scalar declaration initializers retain
    // their owned identity and execute in source order before the process.
    let source = r#"module tb;
    int seed = 3;
    int first = seed + 1;
    int second = first + 1;

    initial begin
        $display("seed=%0d first=%0d second=%0d", seed, first, second);
        $finish;
    end
endmodule
"#;
    let variants = run_variants(source).expect("initializer ordering model should execute");
    for (name, output) in variants {
        assert_eq!(
            output,
            "seed=3 first=4 second=5\n",
            "{name} initializer ordering output"
        );
    }
}

#[test]
fn verilog_2001_initializer_race_accepts_only_permitted_values() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    // IEEE 1364-2001 §6.2.1 permits a declaration assignment to race with an
    // active process; do not require the SystemVerilog pre-process ordering
    // for the explicitly selected Verilog edition.
    let source = r#"module tb;
    reg source;
    reg initialized = source;

    initial begin
        source = 1'b1;
        $display("initialized=%b", initialized);
        $finish;
    end
endmodule
"#;
    for edition in [
        LanguageEdition::Verilog2001,
        LanguageEdition::SystemVerilog2009,
    ] {
        let variants = run_variants_with_edition(source, edition)
            .expect("edition-specific initializer model should execute");
        for (name, output) in variants {
            assert!(
                matches!(output.as_str(), "initialized=x\n" | "initialized=1\n"),
                "edition={edition:?} {name} initializer output: {output:?}"
            );
        }
    }
}
