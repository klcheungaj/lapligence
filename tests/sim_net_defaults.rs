//! Standalone pull-default and supply-net resolution with optimizer parity.

#[path = "support/sim.rs"]
mod sim_harness;

use llg::core::{compile, db::Db};
use llg::sim::{self, opt::OptConfig};

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
            Ok(_) => Err("net-default design unexpectedly generated".to_owned()),
            Err(error) => Ok(error.to_string()),
        }
    })
    .expect("net-default rejection must reach codegen")
}

#[test]
fn pull_defaults_and_supply_sources_preserve_strength_ordering() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let source = r#"// llg-test-fixture: tests/sim_net_defaults.rs/defaults.sv
module tb;
    logic a, b;
    logic [129:0] wide_driver;

    tri0 pulled_zero;
    assign pulled_zero = a;
    assign pulled_zero = b;
    tri1 pulled_one;
    assign pulled_one = a;
    assign pulled_one = b;
    tri0 declared_zero = a;
    assign declared_zero = b;
    tri1 declared_one = a;
    assign declared_one = b;

    supply0 ground;
    assign ground = a;
    assign ground = b;
    supply1 power;
    assign power = a;
    assign power = b;

    tri0 undriven_zero;
    tri1 undriven_one;
    supply0 undriven_ground;
    supply1 undriven_power;
    tri0 signed [64:0] signed_zero;
    tri1 [129:0] wide_one;
    supply0 [1023:0] max_ground;
    supply1 [1023:0] max_power;
    tri0 [129:0] driven_wide;
    assign driven_wide = wide_driver;

    initial begin
        $display("initial=%b%b%b%b signed=%b wide=%b%b max=%b%b",
                 undriven_zero, undriven_one, undriven_ground, undriven_power,
                 signed_zero[64], wide_one[129], wide_one[0],
                 max_ground[1023], max_power[1023]);
        a = 1'bz; b = 1'bz; wide_driver = 'z; #1;
        $display("released=%b%b/%b%b supply=%b%b wide=%b%b%b",
                 pulled_zero, declared_zero, pulled_one, declared_one,
                 ground, power, driven_wide[129], driven_wide[65], driven_wide[0]);
        a = 1; b = 1'bz; wide_driver = '1; #1;
        $display("one=%b%b/%b%b supply=%b%b wide=%b%b",
                 pulled_zero, declared_zero, pulled_one, declared_one,
                 ground, power, driven_wide[129], driven_wide[0]);
        a = 0; b = 1'bz; #1;
        $display("zero=%b%b/%b%b supply=%b%b",
                 pulled_zero, declared_zero, pulled_one, declared_one,
                 ground, power);
        a = 1'bx; b = 1'bz; #1;
        $display("unknown=%b%b/%b%b supply=%b%b",
                 pulled_zero, declared_zero, pulled_one, declared_one,
                 ground, power);
        a = 0; b = 1; #1;
        $display("conflict=%b%b/%b%b supply=%b%b",
                 pulled_zero, declared_zero, pulled_one, declared_one,
                 ground, power);
        a = 1'bz; b = 1'bz; #1;
        $display("rereleased=%b%b/%b%b supply=%b%b",
                 pulled_zero, declared_zero, pulled_one, declared_one,
                 ground, power);
        $finish;
    end
endmodule
"#;
    let expected = "initial=0101 signed=0 wide=11 max=01\n\
                    released=00/11 supply=01 wide=000\n\
                    one=11/11 supply=01 wide=11\n\
                    zero=00/00 supply=01\n\
                    unknown=xx/xx supply=01\n\
                    conflict=xx/xx supply=01\n\
                    rereleased=00/11 supply=01\n";
    sim_harness::with_frontend_temp_cwd("net_defaults", |dir| {
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
    .expect("net-default simulations");
}

#[test]
fn pull_and_supply_nets_reject_explicit_drive_strengths() {
    for (tag, declaration) in [("tri0", "tri0 w;"), ("supply1", "supply1 w;")] {
        let source = format!(
            "// llg-test-fixture: tests/sim_net_defaults.rs/{tag}_strength.sv\n\
             module tb; logic a; {declaration} assign (strong0, strong1) w=a; endmodule\n"
        );
        let error = generate_error(&format!("{tag}_strength"), &source);
        assert!(
            error.contains("drive-strength continuous assignment"),
            "{tag}: {error}"
        );
    }
}

#[test]
fn unchanged_defaults_and_supplies_do_not_refire_monitors() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let source = r#"// llg-test-fixture: tests/sim_net_defaults.rs/no_refire.sv
module tb;
    logic a;
    logic counting;
    integer pull_events, supply_events;
    tri0 pulled;
    supply0 ground;
    assign pulled = a;
    assign ground = a;
    always @(pulled) if (counting) pull_events = pull_events + 1;
    always @(ground) if (counting) supply_events = supply_events + 1;
    initial begin
        counting = 0;
        pull_events = 0;
        supply_events = 0;
        a = 1'bz;
        #0;
        counting = 1;
        $monitor("nets=%b%b", pulled, ground);
        #1 a = 0;
        #1 a = 1;
        #1 a = 1'bz;
        #1;
        $display("events=%0d/%0d", pull_events, supply_events);
        $finish;
    end
endmodule
"#;
    sim_harness::with_frontend_temp_cwd("net_defaults_no_refire", |dir| {
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
            let exe = sim::build::build_model_cmake(
                &dir.join(format!("no-refire-{variant}")),
                &[("model.c", &model.model_c)],
            )
            .map_err(|error| error.to_string())?;
            assert_eq!(
                sim_harness::run_executable(&exe)?,
                "nets=00\nnets=10\nnets=00\nevents=2/0\n",
                "{variant}"
            );
        }
        Ok(())
    })
    .expect("net-default no-refire simulations");
}
