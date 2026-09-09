use llg::core::compile::{compile_checked, CompileOpts, OwnedSource};
use llg::core::db::Db;
use llg::sim::semantic::{Origin, SemanticModel, SynthesisIssueKind, SynthesisProfile};

#[path = "support/sim.rs"]
mod sim_harness;

fn owned_design(text: &str) -> Db {
    let compiled = compile_checked(&CompileOpts {
        sources: vec![OwnedSource::compilation_unit("semantic-profile.sv", text)],
        ..Default::default()
    })
    .expect("compile semantic profile fixture");
    Db::from_slang(&compiled.snapshot).expect("import owned semantic design")
}

#[test]
fn elaborated_combinational_design_has_a_portable_rtl_view() {
    let db = owned_design(
        "module top(input logic [7:0] a, b, output logic [7:0] y);\n\
         always_comb y = a + b;\nendmodule",
    );
    let semantic = SemanticModel::from_db(&db);
    let result = semantic.validate_synthesizable(SynthesisProfile::PortableRtl);
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn simulation_timing_remains_owned_and_reports_its_source_origin() {
    let db = owned_design("module top;\nlogic q;\ninitial begin\n#2 q = 1'b1;\nend\nendmodule");
    let semantic = SemanticModel::from_db(&db);
    let issues = semantic
        .validate_synthesizable(SynthesisProfile::PortableRtl)
        .expect_err("testbench timing is outside portable RTL");
    assert!(issues.iter().any(|issue| {
        issue.kind == SynthesisIssueKind::TimingControl
            && matches!(semantic.origin(issue.origin), Some(Origin::Source { path, line: 4, .. })
                if path.ends_with("semantic-profile.sv"))
    }));
    llg::sim::codegen::generate(&db).expect("simulation lowering preserves testbench timing");
}

#[test]
fn typed_member_selects_preserve_offsets_ranges_and_state_domains() {
    use llg::sim::{build, codegen, opt::OptConfig};
    if !build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let db = owned_design(
        "module top;\n\
         struct packed { logic [3:0] high; bit [7:4] low; } value;\n\
         initial begin value = '0; value.high[2] = 1; value.low[5] = 1;\n\
         value.low[7:6] = 'x;\n\
         $display(\"value=%h selected=%b\", value, value.low[5:4]); $finish; end\n\
         endmodule",
    );
    let directory = sim_harness::TempDir::new("semantic-member-selects").unwrap();
    for (name, options) in [
        ("plain", OptConfig::none()),
        ("optimized", OptConfig::default()),
    ] {
        let model = codegen::generate_from_db_with_opts(&db, &options).unwrap();
        let executable =
            build::build_model_cmake(&directory.path().join(name), &[("model.c", &model.model_c)])
                .unwrap();
        assert_eq!(
            sim_harness::run_executable(&executable).unwrap(),
            "value=42 selected=10\n",
            "{name}"
        );
    }
}
