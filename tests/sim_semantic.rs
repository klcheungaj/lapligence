use llg::core::compile::{compile_checked, CompileOpts, OwnedSource};
use llg::core::db::{CaseKind, Db, NodeKind, ObjectType, StmtKind};
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
fn fixed_unpacked_types_have_known_sizes_and_a_portable_rtl_view() {
    let db = owned_design(
        "typedef struct { logic [7:0] lanes[2]; bit valid; } packet_t;\n\
         module top(input packet_t a, output packet_t y);\n\
         always_comb y = a;\nendmodule",
    );
    let descriptor = db.node_ids().find_map(|node| {
        let descriptor = db.type_descriptor(node)?;
        (descriptor.info.kind == "struct").then_some(descriptor)
    }).expect("captured unpacked struct");
    assert_eq!(descriptor.fixed_size_bits(), Some(17));
    assert!(!descriptor.two_state);
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

/// G1-02 `coverage_all_condition_roles`: every ordinary case condition role is
/// preserved in the owned database, and a pattern case is tagged as an
/// explicitly unsupported pattern instead of an empty ordinary case.
#[test]
fn coverage_preserves_case_condition_roles_and_pattern_identity() {
    let db = owned_design(
        "module top(input logic [1:0] x, output logic [3:0] y);\n\
         always_comb begin\n\
         case (x) inside\n\
         2'b00: y = 4'd1;\n\
         [2'b01:2'b10]: y = 4'd2;\n\
         default: y = 4'd0;\n\
         endcase\n\
         casez (x)\n\
         2'b0?: y = 4'd3;\n\
         default: y = 4'd0;\n\
         endcase\n\
         casex (x)\n\
         2'b0x: y = 4'd4;\n\
         default: y = 4'd0;\n\
         endcase\n\
         case (x)\n\
         2'b11: y = 4'd5;\n\
         default: y = 4'd0;\n\
         endcase\n\
         case (x) matches\n\
         2'b0?: y = 4'd6;\n\
         default: y = 4'd0;\n\
         endcase\n\
         end\n\
         endmodule",
    );
    let mut conditions = Vec::new();
    let mut pattern_cases = 0usize;
    let mut empty_cases = 0usize;
    for id in db.node_ids() {
        match db.node_kind(id) {
            NodeKind::Stmt(StmtKind::Case {
                case_type, items, ..
            }) => {
                conditions.push(*case_type);
                if items.is_empty() {
                    empty_cases += 1;
                }
            }
            NodeKind::Stmt(StmtKind::Unsupported { object_type })
                if *object_type == ObjectType::PatternCaseStatement =>
            {
                pattern_cases += 1;
            }
            _ => {}
        }
    }
    for expected in [CaseKind::Exact, CaseKind::X, CaseKind::Z, CaseKind::Inside] {
        assert!(
            conditions.contains(&expected),
            "missing case condition {expected:?} in {conditions:?}"
        );
    }
    assert_eq!(pattern_cases, 1, "pattern case must keep its own identity");
    assert_eq!(empty_cases, 0, "no case may lose its items");
    // The claimed pattern is reachable and has no lowering contract, so the
    // public IR is stopped before emitter indexing rather than executed.
    let semantic = SemanticModel::from_db(&db);
    assert!(semantic.validate_simulation().is_err());
}

/// G1-30 `classification_three_axes`: a legal runtime loop stays supported for
/// simulation while synthesis policy reports it as unproven.
#[test]
fn classification_separates_simulation_support_from_loop_synthesis_policy() {
    let db = owned_design(
        "module top(input logic [7:0] a, output logic [7:0] y);\n\
         integer i;\n\
         always_comb begin\n\
         y = 8'd0;\n\
         i = 0;\n\
         while (i < a) begin\n\
         y = y + 8'd1;\n\
         i = i + 1;\n\
         end\n\
         end\n\
         endmodule",
    );
    let semantic = SemanticModel::from_db(&db);
    semantic
        .validate_simulation()
        .expect("the runtime loop is executable");
    let issues = semantic
        .validate_synthesizable(SynthesisProfile::PortableRtl)
        .expect_err("a runtime-bounded loop has no static trip count");
    assert!(issues
        .iter()
        .any(|issue| issue.kind == SynthesisIssueKind::UnprovenLoop));
}

/// G1-30: constant `repeat` counts and static `foreach` extents are bounded by
/// elaboration; a runtime condition is not.
#[test]
fn classification_admits_only_resolved_loop_bounds() {
    let repeat_db = owned_design(
        "module top(output logic [3:0] y);\n\
         always_comb begin\n\
         y = 4'd0;\n\
         repeat (4) y = y + 4'd1;\n\
         end\n\
         endmodule",
    );
    SemanticModel::from_db(&repeat_db)
        .validate_synthesizable(SynthesisProfile::PortableRtl)
        .expect("a constant repeat count is a fixed bound");

    let foreach_db = owned_design(
        "module top(output logic [7:0] y);\n\
         logic [7:0] a [0:3];\n\
         integer i;\n\
         always_comb begin\n\
         y = 8'd0;\n\
         foreach (a[i]) y = y + a[i];\n\
         end\n\
         endmodule",
    );
    SemanticModel::from_db(&foreach_db)
        .validate_synthesizable(SynthesisProfile::PortableRtl)
        .expect("a static array extent is a fixed bound");
}

/// G1-30 `classification_rtl_helper`: a fixed zero-time pure helper and its
/// caller agree, and a helper with a runtime service keeps both unproven.
#[test]
fn classification_requires_consistent_zero_time_helper_calls() {
    let clean = owned_design(
        "module top(input logic [7:0] a, b, output logic [7:0] y);\n\
         function automatic logic [7:0] inc(input logic [7:0] v);\n\
         inc = v + 8'd1;\n\
         endfunction\n\
         always_comb y = inc(a) + inc(b);\n\
         endmodule",
    );
    SemanticModel::from_db(&clean)
        .validate_synthesizable(SynthesisProfile::PortableRtl)
        .expect("a fixed zero-time pure helper is admitted");

    let noisy = owned_design(
        "module top(input logic [7:0] a, output logic [7:0] y);\n\
         function automatic logic [7:0] noisy(input logic [7:0] v);\n\
         $display(\"noisy\");\n\
         noisy = v;\n\
         endfunction\n\
         always_comb y = noisy(a);\n\
         endmodule",
    );
    let issues = SemanticModel::from_db(&noisy)
        .validate_synthesizable(SynthesisProfile::PortableRtl)
        .expect_err("a helper with a runtime service is not zero-time pure");
    assert!(issues
        .iter()
        .any(|issue| issue.kind == SynthesisIssueKind::UnprovenCall));
    assert!(issues
        .iter()
        .any(|issue| issue.kind == SynthesisIssueKind::RuntimeService));
}

/// G1-30 `classification_no_keyword_shortcut`: the target-dependent initial
/// preload form is recognized as storage initialization but still rejected by
/// portable RTL; an ordinary initial block stays a simulation process.
#[test]
fn classification_does_not_admit_initial_by_keyword() {
    let preload = owned_design(
        "module top(output logic [7:0] q);\n\
         logic [7:0] mem [0:1];\n\
         initial begin\n\
         mem[0] = 8'hAB;\n\
         mem[1] = 8'hCD;\n\
         end\n\
         assign q = mem[0];\n\
         endmodule",
    );
    let semantic = SemanticModel::from_db(&preload);
    semantic
        .validate_simulation()
        .expect("preload initialization is legal simulation");
    let issues = semantic
        .validate_synthesizable(SynthesisProfile::PortableRtl)
        .expect_err("portable RTL has no target-independent preload proof");
    assert!(issues
        .iter()
        .any(|issue| issue.kind == SynthesisIssueKind::StorageInitialization));
    assert!(!issues
        .iter()
        .any(|issue| issue.kind == SynthesisIssueKind::SimulationProcess));

    let testbench = owned_design(
        "module top;\n\
         initial $display(\"hello\");\n\
         endmodule",
    );
    let issues = SemanticModel::from_db(&testbench)
        .validate_synthesizable(SynthesisProfile::PortableRtl)
        .expect_err("a testbench initial block is not synthesizable");
    assert!(issues
        .iter()
        .any(|issue| issue.kind == SynthesisIssueKind::SimulationProcess));
}
