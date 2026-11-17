//! Runtime uniqueness and priority diagnostics through both optimizer modes.

use crate::sim_cli;

#[test]
fn unique_case_reports_overlapping_items_but_keeps_first_branch() {
    let warning = "overlap.sv:6:9 unique violation: multiple matching items";
    sim_cli::run_case(
        "unique_priority",
        "overlap",
        "first\n",
        "INFO: overlap.sv:10:9: [SIM-001] (tb @ 0 ps) $finish called\nINFO: [SIM-000] 0 error(s), 1 warning(s), 1 info(s)\n",
        &[warning],
    );
}

#[test]
fn unique0_case_reports_overlapping_items_without_no_match_warning() {
    let warning = "unique0_overlap.sv:6:9 unique0 violation: multiple matching items";
    sim_cli::run_case(
        "unique_priority",
        "unique0_overlap",
        "unique0-first\n",
        "INFO: unique0_overlap.sv:10:9: [SIM-001] (tb @ 0 ps) $finish called\nINFO: [SIM-000] 0 error(s), 1 warning(s), 1 info(s)\n",
        &[warning],
    );
}

#[test]
fn qualifiers_distinguish_no_match_and_default_or_else_suppression() {
    let unique_case = "no_match.sv:7:9 unique violation: no matching item";
    let priority_case = "no_match.sv:13:9 priority violation: no matching item";
    let unique_if = "no_match.sv:21:9 unique violation: no matching item";
    let priority_if = "no_match.sv:25:9 priority violation: no matching item";
    sim_cli::run_case(
        "unique_priority",
        "no_match",
        "case-default\nif-else\n",
        "INFO: no_match.sv:31:9: [SIM-001] (tb @ 0 ps) $finish called\nINFO: [SIM-000] 0 error(s), 4 warning(s), 1 info(s)\n",
        &[
            unique_case,
            priority_case,
            unique_if,
            priority_if,
        ],
    );
}

#[test]
fn case_flavors_keep_four_state_matching_for_qualifier_checks() {
    let exact = "wildcards.sv:6:9 unique violation: no matching item";
    sim_cli::run_case(
        "unique_priority",
        "wildcards",
        "casez\ncasex\n",
        "INFO: wildcards.sv:17:9: [SIM-001] (tb @ 0 ps) $finish called\nINFO: [SIM-000] 0 error(s), 1 warning(s), 1 info(s)\n",
        &[exact],
    );
}

#[test]
fn qualified_if_ladder_reports_when_every_condition_is_false_or_unknown() {
    let warning = "ladder.sv:8:9 unique violation: no matching item";
    sim_cli::run_case(
        "unique_priority",
        "ladder",
        "",
        "INFO: ladder.sv:12:9: [SIM-001] (tb @ 0 ps) $finish called\nINFO: [SIM-000] 0 error(s), 1 warning(s), 1 info(s)\n",
        &[warning],
    );
}

#[test]
fn qualified_if_ladder_reports_multiple_true_conditions() {
    let warning = "if_overlap.sv:8:9 unique violation: multiple matching items";
    sim_cli::run_case(
        "unique_priority",
        "if_overlap",
        "first\n",
        "INFO: if_overlap.sv:12:9: [SIM-001] (tb @ 0 ps) $finish called\nINFO: [SIM-000] 0 error(s), 1 warning(s), 1 info(s)\n",
        &[warning],
    );
}

#[test]
fn qualified_real_case_reports_overlaps_after_capturing_selector() {
    let warning = "real_case.sv:6:9 unique violation: multiple matching items";
    sim_cli::run_case(
        "unique_priority",
        "real_case",
        "real-first\n",
        "INFO: real_case.sv:10:9: [SIM-001] (tb @ 0 ps) $finish called\nINFO: [SIM-000] 0 error(s), 1 warning(s), 1 info(s)\n",
        &[warning],
    );
}

#[test]
fn qualified_case_inside_counts_overlapping_membership_groups() {
    let warning = "inside.sv:6:9 unique violation: multiple matching items";
    sim_cli::run_case(
        "unique_priority",
        "inside",
        "inside-first\n",
        "INFO: inside.sv:11:9: [SIM-001] (tb @ 0 ps) $finish called\nINFO: [SIM-000] 0 error(s), 1 warning(s), 1 info(s)\n",
        &[warning],
    );
}
