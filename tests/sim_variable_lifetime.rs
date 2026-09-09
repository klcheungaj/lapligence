//! Procedural variable lifetime regressions over the Slang-owned database.

#[path = "support/sim.rs"]
mod sim_harness;

use llg::core::{compile, db::Db};
use llg::sim::{self, opt::OptConfig};

fn run_variants(source: &str) -> Result<Vec<(String, String)>, String> {
    sim_harness::with_frontend_temp_cwd("variable-lifetime", |dir| {
        let source_path = dir.join("variable_lifetime.sv");
        std::fs::write(&source_path, source).map_err(|error| format!("write source: {error}"))?;
        let compiled = compile::compile_checked(&compile::CompileOpts {
            files: vec![source_path.to_string_lossy().into_owned()],
            top: Some("tb".to_owned()),
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
