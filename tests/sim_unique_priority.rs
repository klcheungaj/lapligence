//! Runtime uniqueness and priority diagnostics through both optimizer modes.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

#[test]
fn unique_case_reports_overlapping_items_but_keeps_first_branch() {
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/unique_priority/overlap.sv");
    let warning = format!(
        "unique violation at {}:6:9: multiple matching items",
        source.display()
    );
    sim_cli::run_case(
        "unique_priority",
        "overlap",
        "first\n",
        "llg: $finish at time 0 at tb:10:9\n",
        &[warning.as_str()],
    );
}

#[test]
fn unique0_case_reports_overlapping_items_without_no_match_warning() {
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/unique_priority/unique0_overlap.sv");
    let warning = format!(
        "unique0 violation at {}:6:9: multiple matching items",
        source.display()
    );
    sim_cli::run_case(
        "unique_priority",
        "unique0_overlap",
        "unique0-first\n",
        "llg: $finish at time 0 at tb:10:9\n",
        &[warning.as_str()],
    );
}

#[test]
fn qualifiers_distinguish_no_match_and_default_or_else_suppression() {
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/unique_priority/no_match.sv");
    let unique_case = format!(
        "unique violation at {}:7:9: no matching item",
        source.display()
    );
    let priority_case = format!(
        "priority violation at {}:13:9: no matching item",
        source.display()
    );
    let unique_if = format!(
        "unique violation at {}:21:9: no matching item",
        source.display()
    );
    let priority_if = format!(
        "priority violation at {}:25:9: no matching item",
        source.display()
    );
    sim_cli::run_case(
        "unique_priority",
        "no_match",
        "case-default\nif-else\n",
        "llg: $finish at time 0 at tb:31:9\n",
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
        source.display()
    );
    sim_cli::run_case(
        "unique_priority",
        "wildcards",
        "casez\ncasex\n",
        "llg: $finish at time 0 at tb:17:9\n",
        &[exact.as_str()],
    );
}

#[test]
fn qualified_if_ladder_reports_when_every_condition_is_false_or_unknown() {
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/unique_priority/ladder.sv");
    let warning = format!(
        "unique violation at {}:8:9: no matching item",
        source.display()
    );
    sim_cli::run_case(
        "unique_priority",
        "ladder",
        "",
        "llg: $finish at time 0 at tb:12:9\n",
        &[warning.as_str()],
    );
}

#[test]
fn qualified_if_ladder_reports_multiple_true_conditions() {
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/unique_priority/if_overlap.sv");
    let warning = format!(
        "unique violation at {}:8:9: multiple matching items",
        source.display()
    );
    sim_cli::run_case(
        "unique_priority",
        "if_overlap",
        "first\n",
        "llg: $finish at time 0 at tb:12:9\n",
        &[warning.as_str()],
    );
}

#[test]
fn qualified_real_case_reports_overlaps_after_capturing_selector() {
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/unique_priority/real_case.sv");
    let warning = format!(
        "unique violation at {}:6:9: multiple matching items",
        source.display()
    );
    sim_cli::run_case(
        "unique_priority",
        "real_case",
        "real-first\n",
        "llg: $finish at time 0 at tb:10:9\n",
        &[warning.as_str()],
    );
}

#[test]
fn qualified_case_inside_counts_overlapping_membership_groups() {
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/unique_priority/inside.sv");
    let warning = format!(
        "unique violation at {}:6:9: multiple matching items",
        source.display()
    );
    sim_cli::run_case(
        "unique_priority",
        "inside",
        "inside-first\n",
        "llg: $finish at time 0 at tb:11:9\n",
        &[warning.as_str()],
    );
}
