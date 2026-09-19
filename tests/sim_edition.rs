//! End-to-end target-edition and ordinary time-literal acceptance tests.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

use llg::core::compile::{self, CompileOpts, LanguageEdition, OwnedSource};
use llg::core::db::Db;
use std::path::Path;

#[test]
fn systemverilog_2009_rounds_time_literals_before_value_use() {
    sim_cli::run_case_with_args(
        "partial_features",
        "time_literal_rounding_2009",
        "CHECK: literal=2.0\n",
        "",
        &[],
        &["--edition", "2009"],
    );
    sim_cli::run_case_with_args(
        "partial_features",
        "time_literal_values_2009",
        concat!(
            "value=1.50 initialized=1.50 positive=1.60 negative=-1.60 arithmetic=1.50 parameter=1.50\n",
            "delay=1.60\n"
        ),
        "",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn systemverilog_2009_preserves_exact_femtosecond_literal_boundaries() {
    sim_cli::run_case_with_args(
        "partial_features",
        "time_literal_exact_2009",
        "exact=1.000001 -1.000001 0.000001 delay=1.000001\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn declaration_initialization_keeps_edition_specific_scheduling() {
    sim_cli::run_case_with_args(
        "partial_features",
        "declaration_init_edition",
        "PASS declaration_init_edition\n",
        "llg: $finish at time 0 at tb:15:9\n",
        &[],
        &["--edition", "2001"],
    );
    sim_cli::run_case_with_args(
        "partial_features",
        "declaration_init_edition",
        "PASS declaration_init_edition\n",
        "llg: $finish at time 0 at tb:15:9\n",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn verilog_2001_rejects_systemverilog_constructs() {
    sim_cli::reject_case_with_args(
        "partial_features",
        "edition_2001_sv_only",
        "always_comb",
        &["--edition", "2001"],
    );
}

#[test]
fn begin_keywords_does_not_change_the_selected_global_edition() {
    let output = compile::compile(&CompileOpts {
        sources: vec![OwnedSource::compilation_unit(
            "begin-keywords.sv",
            "`begin_keywords \"1800-2009\"\nmodule tb; reg value; initial value = 1'b0; endmodule\n`end_keywords\n",
        )],
        top: Some("tb".to_owned()),
        edition: LanguageEdition::Verilog2001,
        ..CompileOpts::default()
    })
    .expect("bridge compile");
    assert!(!output.snapshot.has_errors(), "{:?}", output.diagnostics);
    assert_eq!(output.snapshot.edition(), LanguageEdition::Verilog2001);
    let database = Db::from_slang(&output.snapshot).expect("owned database");
    assert_eq!(database.edition(), LanguageEdition::Verilog2001);
}

#[test]
fn owned_model_retains_parameter_override_provenance() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/edition_parameter_override.sv");
    let output = compile::compile_checked(&CompileOpts {
        files: vec![source.to_string_lossy().into_owned()],
        top: Some("tb".to_owned()),
        param_overrides: vec!["P=2".to_owned()],
        ..CompileOpts::default()
    })
    .expect("bridge compile");
    let database = Db::from_slang(&output.snapshot).expect("owned database");
    let parameter = database
        .node_ids()
        .find(|id| {
            matches!(
                database.node_kind(*id),
                llg::core::db::NodeKind::Param { .. }
            )
        })
        .expect("captured parameter");
    assert!(database.parameter_is_overridden(parameter));
}

/// IEEE 1800-2009 defines a `final` procedure but not the deferred
/// `assert final` of IEEE 1800-2017 §16.4. The strict 2009 profile must reject
/// the later form with a located owned diagnostic, even though the newer Slang
/// grammar accepts it.
#[test]
fn edition_later_forms_rejects_post_2009_assert_final() {
    let output = compile::compile(&CompileOpts {
        sources: vec![OwnedSource::compilation_unit(
            "assert_final.sv",
            "module tb;\n  initial begin\n    assert final (1'b1) else $display(\"bad\");\n  end\nendmodule\n",
        )],
        top: Some("tb".to_owned()),
        edition: LanguageEdition::SystemVerilog2009,
        ..CompileOpts::default()
    })
    .expect("bridge compile");
    assert!(
        !output.snapshot.has_errors(),
        "Slang accepts the later form"
    );
    assert!(!output.ok(), "the strict 2009 profile must reject it");
    let diagnostic = output
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.message.contains("assert final"))
        .expect("owned edition diagnostic");
    assert_eq!(diagnostic.severity, compile::Severity::Error);
    assert_eq!(diagnostic.file.as_deref(), Some("assert_final.sv"));
    assert_eq!(
        diagnostic.line, 3,
        "diagnostic must be located: {diagnostic:?}"
    );
}

/// The 2009 profile still accepts the deferred `assert #0` form that belongs
/// to its grammar, so the edition gate is not a blanket assertion ban.
#[test]
fn edition_later_forms_keeps_2009_deferred_assertions() {
    let output = compile::compile(&CompileOpts {
        sources: vec![OwnedSource::compilation_unit(
            "deferred.sv",
            "module tb;\n  initial begin\n    assert #0 (1'b1);\n  end\nendmodule\n",
        )],
        top: Some("tb".to_owned()),
        edition: LanguageEdition::SystemVerilog2009,
        ..CompileOpts::default()
    })
    .expect("bridge compile");
    assert!(
        output.ok(),
        "assert #0 belongs to IEEE 1800-2009: {:?}",
        output.diagnostics
    );
}

/// `$assertcontrol` is a SystemVerilog form absent from IEEE 1364-2001; the
/// strict 2001 profile must reject it with a located owned diagnostic.
#[test]
fn edition_later_forms_rejects_assertcontrol_in_verilog_2001() {
    let output = compile::compile(&CompileOpts {
        sources: vec![OwnedSource::compilation_unit(
            "assertcontrol.sv",
            "module tb;\n  initial $assertcontrol(1);\nendmodule\n",
        )],
        top: Some("tb".to_owned()),
        edition: LanguageEdition::Verilog2001,
        ..CompileOpts::default()
    })
    .expect("bridge compile");
    assert!(
        !output.ok(),
        "the strict 2001 profile must reject $assertcontrol"
    );
    let diagnostic = output
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.message.contains("$assertcontrol"))
        .expect("owned edition diagnostic");
    assert_eq!(diagnostic.severity, compile::Severity::Error);
    assert_eq!(diagnostic.file.as_deref(), Some("assertcontrol.sv"));
    assert_eq!(
        diagnostic.line, 2,
        "diagnostic must be located: {diagnostic:?}"
    );
}

/// End-to-end contrast: the CLI rejects the 2017-only form under `--edition
/// 2009` while the 2009-legal deferred form still runs.
#[test]
fn edition_later_forms_cli_contrast() {
    sim_cli::reject_case_with_args(
        "partial_features",
        "edition_assert_final",
        "is not available in IEEE 2009",
        &["--edition", "2009"],
    );
    sim_cli::reject_case_with_args(
        "partial_features",
        "edition_assertcontrol",
        "is not available in IEEE 2001",
        &["--edition", "2001"],
    );
    sim_cli::run_case_with_args(
        "partial_features",
        "deferred_assertions",
        "sampled=0 reference=1\n",
        concat!(
            "llg: $finish at time 1000 at tb:14:5\n",
            "llg: simulation statistics: processes=1\n",
        ),
        &[],
        &["--edition", "2009"],
    );
}


fn strict_profile_rejects_in_both_snapshot_modes(source: &str, edition: LanguageEdition, label: &str) {
    for library_units in [false, true] {
        let output = compile::compile(&CompileOpts {
            sources: vec![OwnedSource::compilation_unit("strict-profile.sv", source)],
            top: Some("tb".to_owned()), edition, library_units,
            ..CompileOpts::default()
        }).expect("compile strict-profile source");
        assert!(!output.ok(), "library_units={library_units}: {label} was accepted");
        let diagnostic = output.diagnostics.iter().find(|diagnostic| {
            diagnostic.message.contains("strict edition profile") && diagnostic.message.contains(label)
        }).unwrap_or_else(|| panic!("library_units={library_units}: {:?}", output.diagnostics));
        assert_eq!(diagnostic.file.as_deref(), Some("strict-profile.sv"));
        assert!(diagnostic.line > 0 && diagnostic.col > 0, "{diagnostic:?}");
    }
}

#[test]
fn one_step_clocking_skew_is_admitted_in_both_systemverilog_snapshot_modes() {
    let text = "module tb; bit clk; logic value; clocking cb @(posedge clk); default input #1step; input value; endclocking endmodule";
    for library_units in [false, true] {
        compile::compile_checked(&CompileOpts {
            sources: vec![OwnedSource::compilation_unit("one-step.sv", text)],
            top: Some("tb".to_owned()),
            edition: LanguageEdition::SystemVerilog2009,
            library_units,
            ..CompileOpts::default()
        })
        .expect("1step clocking skew is legal in IEEE 1800-2009");
    }
}

#[test]
fn assertion_controls_have_the_same_edition_policy_in_navigation_and_execution() {
    for name in ["$asserton", "$assertoff", "$assertkill", "$assertpasson", "$assertpassoff",
        "$assertfailon", "$assertfailoff", "$assertnonvacuouson", "$assertvacuousoff"] {
        let text = format!("module tb;\n  initial {name}();\nendmodule\n");
        strict_profile_rejects_in_both_snapshot_modes(&text, LanguageEdition::Verilog2001, name);
        for library_units in [false, true] {
            let output = compile::compile(&CompileOpts {
                sources: vec![OwnedSource::compilation_unit("controls.sv", &text)],
                top: Some("tb".to_owned()), edition: LanguageEdition::SystemVerilog2009,
                library_units, ..CompileOpts::default()
            }).expect("compile 2009 control task");
            assert!(output.ok(), "{name}, navigation={library_units}: {:?}", output.diagnostics);
        }
    }
}

#[test]
fn newer_builtins_cannot_leak_through_either_snapshot_profile() {
    for (name, statement) in [("$assertcontrol", "$assertcontrol(1);"),
        ("$countbits", "$display(\"%0d\", $countbits(4'b0011, 1'b1));")] {
        let text = format!("module tb;\n  initial begin {statement} end\nendmodule\n");
        for edition in [LanguageEdition::Verilog2001, LanguageEdition::SystemVerilog2009] {
            strict_profile_rejects_in_both_snapshot_modes(&text, edition, name);
        }
    }
}

#[test]
fn strict_profile_checks_actual_macro_expansions_not_unused_replacement_text() {
    let text = r#"
`define UNUSED_BAD assert final (1'b1);
`define UNUSED_CALL $countbits(4'b0000, 1'b0)
module tb;
    reg \$assertcontrol ;
    initial begin
        \$assertcontrol = 1'b0;
        $display("$assertcontrol(1); assert final; interface class; soft");
        // $countbits and assert final are not code in this comment.
`ifdef THIS_BRANCH_IS_NOT_DEFINED
        $countbits(4'b0000, 1'b0);
`endif
    end
    final $display("ordinary final is part of 2009");
endmodule
"#;
    for library_units in [false, true] {
        let output = compile::compile(&CompileOpts {
            sources: vec![OwnedSource::compilation_unit("token-context.sv", text)],
            top: Some("tb".to_owned()), library_units, ..CompileOpts::default()
        }).expect("compile token-context probe");
        assert!(output.ok(), "navigation={library_units}: {:?}", output.diagnostics);
        assert!(output.snapshot.lexical_tokens.iter().any(|token| token.is_directive));
    }
    strict_profile_rejects_in_both_snapshot_modes(
        "`define CALL $assertoff\nmodule tb; initial `CALL(); endmodule\n",
        LanguageEdition::Verilog2001, "$assertoff",
    );
}

#[test]
fn begin_keywords_cannot_admit_later_constructs() {
    for (source, label) in [
        ("`begin_keywords \"1800-2017\"\nmodule tb; initial assert final(1'b1); endmodule\n`end_keywords\n", "assert final"),
        ("`begin_keywords \"1800-2017\"\ninterface class base; endclass\nmodule tb; endmodule\n`end_keywords\n", "interface class"),
        ("`begin_keywords \"1800-2017\"\nclass C; rand int x; constraint c { soft x == 1; } endclass\nmodule tb; endmodule\n`end_keywords\n", "soft"),
    ] {
        strict_profile_rejects_in_both_snapshot_modes(source, LanguageEdition::SystemVerilog2009, label);
    }
    strict_profile_rejects_in_both_snapshot_modes(
        "`begin_keywords \"1800-2009\"\nmodule tb; logic x; endmodule\n`end_keywords\n",
        LanguageEdition::Verilog2001, "logic",
    );
}

#[test]
fn registered_system_extensions_do_not_become_standard_capabilities() {
    for edition in [LanguageEdition::Verilog2001, LanguageEdition::SystemVerilog2009] {
        for library_units in [false, true] {
            let output = compile::compile(&CompileOpts {
                sources: vec![OwnedSource::compilation_unit("registered.sv",
                    "module tb; integer result; initial result = $my_registered(3); endmodule\n")],
                top: Some("tb".to_owned()), edition, library_units,
                system_subroutines: vec!["function int $my_registered(input int x);".to_owned()],
                ..CompileOpts::default()
            }).expect("compile registered extension");
            assert!(output.ok(), "{edition}, navigation={library_units}: {:?}", output.diagnostics);
        }
    }
    strict_profile_rejects_in_both_snapshot_modes(
        "module tb; integer result; initial result = $my_registered(3); endmodule\n",
        LanguageEdition::SystemVerilog2009, "$my_registered",
    );
}

#[test]
fn standard_timing_checks_are_not_mistaken_for_system_extensions() {
    let source = "module tb(input a, b); specify $setup(posedge a, posedge b, 1); \
                  $hold(posedge b, posedge a, 1); endspecify endmodule\n";
    for edition in [LanguageEdition::Verilog2001, LanguageEdition::SystemVerilog2009] {
        for library_units in [false, true] {
            let output = compile::compile(&CompileOpts {
                sources: vec![OwnedSource::compilation_unit("timing.v", source)],
                top: Some("tb".to_owned()),
                edition,
                library_units,
                ..CompileOpts::default()
            }).expect("compile standard timing checks");
            assert!(output.ok(), "{edition}, navigation={library_units}: {:?}", output.diagnostics);
        }
    }
}
