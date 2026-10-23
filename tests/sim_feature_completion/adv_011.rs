//! ADV-011: the SDF input grammar (OVI SDF 2.1 and 3.0) and the owned
//! annotation plan. The corpus of this directory is parsed, validated and
//! resolved against the annotation targets of `cells.v`; the expected plans,
//! warnings and errors are derived by hand in the fixture readme from the
//! OVI SDF 3.0 grammar and IEEE 1800-2009 32.
use super::sim_cli;
use llg::core::{compile, db::Db};
use llg::sim::codegen::{sdf_targets, CodegenOptions};
use llg::sim::ir::IrTimingCheckOptions;
use llg::sim::sdf::{resolve, SdfLimits, SdfTargets};
use std::path::Path;

const SUITE: &str = "feature_completion/adv_011";

macro_rules! fixture {
    ($name:literal) => {
        include_str!(concat!(
            "../fixtures/sim/feature_completion/adv_011/",
            $name
        ))
    };
}

/// Compare `actual` with checked-in fixture `name`. With
/// `LLG_SDF_BLESS=1` the fixture is rewritten instead; review the diff
/// against the readme's derivation before committing it.
fn check_golden(name: &str, actual: &str, expected: &str) {
    if std::env::var("LLG_SDF_BLESS").as_deref() == Ok("1") {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/sim/feature_completion/adv_011")
            .join(name);
        std::fs::write(&path, actual).expect("write golden");
        return;
    }
    assert_eq!(actual, expected, "{name}");
}

/// The annotation targets of `cells.v`, elaborated from `tb`.
fn cell_targets() -> SdfTargets {
    cell_targets_with(&CodegenOptions::default())
}

fn cell_targets_with(options: &CodegenOptions) -> SdfTargets {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/feature_completion/adv_011/cells.v");
    let compiled = compile::compile_checked(&compile::CompileOpts {
        files: vec![source.to_string_lossy().into_owned()],
        top: Some("tb".to_owned()),
        ..Default::default()
    })
    .expect("cells.v compiles");
    let database = Db::from_slang(&compiled.snapshot).expect("owned database");
    drop(compiled);
    sdf_targets(&database, options).expect("annotation targets")
}

/// Resolve `text` from scope `root` and render the plan, or the error.
fn plan(targets: &SdfTargets, root: &str, text: &str) -> String {
    let root = targets.find(root).expect("root scope");
    match resolve(text.as_bytes(), targets, root, SdfLimits::default()) {
        Ok(plan) => plan.render(targets),
        Err(error) => format!("error {error}\n"),
    }
}

/// The design the corpus is resolved against simulates as written: SDF
/// support adds no behaviour to a model without an annotation.
#[test]
fn cells_simulate_without_annotation() {
    for edition in ["v2001", "sv2009"] {
        sim_cli::run_case_backend_parity(
            SUITE,
            "cells.v",
            fixture!("cells.out"),
            &["--edition", edition],
            &[],
        );
    }
}

/// Every scope, port, path and timing check an SDF file can name.
#[test]
fn component_annotation_targets() {
    let targets = cell_targets();
    check_golden("targets.txt", &targets.render(), fixture!("targets.txt"));
}

/// Accepted files: the plan (operations in file order, then warnings) each
/// one resolves to.
#[test]
fn component_accepted_corpus() {
    let targets = cell_targets();
    for (name, text, expected) in [
        (
            "pos_v30.plan",
            fixture!("pos_v30.sdf"),
            fixture!("pos_v30.plan"),
        ),
        (
            "pos_v21.plan",
            fixture!("pos_v21.sdf"),
            fixture!("pos_v21.plan"),
        ),
        (
            "pos_unmatched.plan",
            fixture!("pos_unmatched.sdf"),
            fixture!("pos_unmatched.plan"),
        ),
        (
            "pos_lenient.plan",
            fixture!("pos_lenient.sdf"),
            fixture!("pos_lenient.plan"),
        ),
        (
            "pos_lenient_v21.plan",
            fixture!("pos_lenient_v21.sdf"),
            fixture!("pos_lenient_v21.plan"),
        ),
    ] {
        check_golden(name, &plan(&targets, "tb", text), expected);
    }
}

/// Rejected files: the exact first error, and no plan.
#[test]
fn component_rejected_corpus() {
    let targets = cell_targets();
    let corpus = fixture!("rejections.txt");
    let mut cases = 0;
    let mut failures = Vec::new();
    for case in corpus.split("\n=== ").skip(1) {
        let (name, rest) = case.split_once('\n').expect("a case name line");
        let (error, text) = rest.split_once('\n').unwrap_or((rest, ""));
        let expected = error.strip_prefix("error: ").expect("an error line");
        let actual = plan(&targets, "tb", text);
        if actual != format!("error {expected}\n") {
            failures.push(format!(
                "{name}:\n  expected error {expected}\n  actual   {actual}"
            ));
        }
        cases += 1;
    }
    assert!(cases >= 30, "{cases} cases");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// With negative timing checks, every `$setuphold` and `$recrem` takes part
/// in its module's delayed signal solve, so the catalog marks it: an
/// annotated limit must re-solve the delays, which generation fixed. The
/// rest of the catalog does not change.
#[test]
fn component_negative_checks_mark_delayed_signal_solves() {
    let targets = cell_targets_with(&CodegenOptions {
        timing_checks: IrTimingCheckOptions {
            negative: true,
            ..Default::default()
        },
        ..Default::default()
    });
    let rendered = targets.render();
    let expected = fixture!("targets.txt")
        .lines()
        .map(|line| {
            if line.starts_with("  check ")
                && (line.contains("$setuphold") || line.contains("$recrem"))
            {
                format!("{}\n", line.replacen(" rows [", " delayed rows [", 1))
            } else {
                format!("{line}\n")
            }
        })
        .collect::<String>();
    assert_eq!(rendered, expected);
}

/// A file resolves from any scope, named in the notation of the plan:
/// escaped characters and element indices select the scope.
#[test]
fn component_resolution_scope() {
    let targets = cell_targets();
    let cell = |cell_type: &str, instance: &str| {
        format!(
            "(DELAYFILE (SDFVERSION \"3.0\") (CELL (CELLTYPE \"{cell_type}\") (INSTANCE {instance}) \
             (DELAY (ABSOLUTE (IOPATH a y (1))))))"
        )
    };
    assert_eq!(
        plan(&targets, "tb.g[1]", &cell("AND2", "u")),
        "SDF 3.0 timescale 1ns from tb.g[1]\n\
         1:86: path delay absolute tb.g[1].u path 0 (a => y) = (1)\n"
    );
    assert_eq!(
        plan(&targets, "tb.esc\\.inst", &cell("AND2", "")),
        "SDF 3.0 timescale 1ns from tb.esc\\.inst\n\
         1:85: path delay absolute tb.esc\\.inst path 0 (a => y) = (1)\n"
    );
    // The wildcard stays inside the scope.
    assert_eq!(
        plan(&targets, "tb.g[0]", &cell("AND2", "*")),
        "SDF 3.0 timescale 1ns from tb.g[0]\n\
         1:86: path delay absolute tb.g[0].u path 0 (a => y) = (1)\n"
    );
    for path in [
        "tb.esc.inst",
        "tb.g",
        "tb.g[2]",
        "tb..ff1",
        "tb.ff1[0]",
        "tb.g[0]x",
    ] {
        assert_eq!(targets.find(path), None, "{path}");
    }
}

/// The same file and design always give the same plan.
#[test]
fn component_plans_are_deterministic() {
    let first = plan(&cell_targets(), "tb", fixture!("pos_v30.sdf"));
    let second = plan(&cell_targets(), "tb", fixture!("pos_v30.sdf"));
    assert_eq!(first, second);
}
