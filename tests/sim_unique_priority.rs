//! Runtime uniqueness and priority diagnostics through both optimizer modes.

use crate::sim_cli;
use crate::sim_harness;

#[test]
fn unique_case_reports_overlapping_items_but_keeps_first_branch() {
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/unique_priority/overlap.sv");
    let warning = format!(
        "unique violation at {}:6:9: multiple matching items",
        sim_harness::source_display(&source)
    );
    sim_cli::run_case(
        "unique_priority",
        "overlap",
        "first\n",
        "INFO: overlap.sv:10:9: [SIM-001] (tb @ 0 ps) $finish called\nINFO: [SIM-000] 0 error(s), 1 warning(s), 1 info(s)\n",
        &[warning.as_str()],
    );
}

#[test]
fn unique0_case_reports_overlapping_items_without_no_match_warning() {
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/unique_priority/unique0_overlap.sv");
    let warning = format!(
        "unique0 violation at {}:6:9: multiple matching items",
        sim_harness::source_display(&source)
    );
    sim_cli::run_case(
        "unique_priority",
        "unique0_overlap",
        "unique0-first\n",
        "INFO: unique0_overlap.sv:10:9: [SIM-001] (tb @ 0 ps) $finish called\nINFO: [SIM-000] 0 error(s), 1 warning(s), 1 info(s)\n",
        &[warning.as_str()],
    );
}

#[test]
fn qualifiers_distinguish_no_match_and_default_or_else_suppression() {
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/unique_priority/no_match.sv");
    let unique_case = format!(
        "unique violation at {}:7:9: no matching item",
        sim_harness::source_display(&source)
    );
    let priority_case = format!(
        "priority violation at {}:13:9: no matching item",
        sim_harness::source_display(&source)
    );
    let unique_if = format!(
        "unique violation at {}:21:9: no matching item",
        sim_harness::source_display(&source)
    );
    let priority_if = format!(
        "priority violation at {}:25:9: no matching item",
        sim_harness::source_display(&source)
    );
    sim_cli::run_case(
        "unique_priority",
        "no_match",
        "case-default\nif-else\n",
        "INFO: no_match.sv:31:9: [SIM-001] (tb @ 0 ps) $finish called\nINFO: [SIM-000] 0 error(s), 4 warning(s), 1 info(s)\n",
        &[
            unique_case.as_str(),
            priority_case.as_str(),
            unique_if.as_str(),
            priority_if.as_str(),
        ],
    );
}

#[test]
fn case_flavors_keep_four_state_matching_for_qualifier_checks() {
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/unique_priority/wildcards.sv");
    let exact = format!(
        "unique violation at {}:6:9: no matching item",
        sim_harness::source_display(&source)
    );
    sim_cli::run_case(
        "unique_priority",
        "wildcards",
        "casez\ncasex\n",
        "INFO: wildcards.sv:17:9: [SIM-001] (tb @ 0 ps) $finish called\nINFO: [SIM-000] 0 error(s), 1 warning(s), 1 info(s)\n",
        &[exact.as_str()],
    );
}

#[test]
fn qualified_if_ladder_reports_when_every_condition_is_false_or_unknown() {
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/unique_priority/ladder.sv");
    let warning = format!(
        "unique violation at {}:8:9: no matching item",
        sim_harness::source_display(&source)
    );
    sim_cli::run_case(
        "unique_priority",
        "ladder",
        "",
        "INFO: ladder.sv:12:9: [SIM-001] (tb @ 0 ps) $finish called\nINFO: [SIM-000] 0 error(s), 1 warning(s), 1 info(s)\n",
        &[warning.as_str()],
    );
}

#[test]
fn qualified_if_ladder_reports_multiple_true_conditions() {
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/unique_priority/if_overlap.sv");
    let warning = format!(
        "unique violation at {}:8:9: multiple matching items",
        sim_harness::source_display(&source)
    );
    sim_cli::run_case(
        "unique_priority",
        "if_overlap",
        "first\n",
        "INFO: if_overlap.sv:12:9: [SIM-001] (tb @ 0 ps) $finish called\nINFO: [SIM-000] 0 error(s), 1 warning(s), 1 info(s)\n",
        &[warning.as_str()],
    );
}

#[test]
fn qualified_real_case_reports_overlaps_after_capturing_selector() {
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/unique_priority/real_case.sv");
    let warning = format!(
        "unique violation at {}:6:9: multiple matching items",
        sim_harness::source_display(&source)
    );
    sim_cli::run_case(
        "unique_priority",
        "real_case",
        "real-first\n",
        "INFO: real_case.sv:10:9: [SIM-001] (tb @ 0 ps) $finish called\nINFO: [SIM-000] 0 error(s), 1 warning(s), 1 info(s)\n",
        &[warning.as_str()],
    );
}

#[test]
fn qualified_case_inside_counts_overlapping_membership_groups() {
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/unique_priority/inside.sv");
    let warning = format!(
        "unique violation at {}:6:9: multiple matching items",
        sim_harness::source_display(&source)
    );
    sim_cli::run_case(
        "unique_priority",
        "inside",
        "inside-first\n",
        "INFO: inside.sv:11:9: [SIM-001] (tb @ 0 ps) $finish called\nINFO: [SIM-000] 0 error(s), 1 warning(s), 1 info(s)\n",
        &[warning.as_str()],
    );
}
