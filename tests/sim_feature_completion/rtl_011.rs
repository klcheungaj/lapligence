use super::{sim_cli, sim_harness};
use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

const SUITE: &str = "feature_completion/rtl_011";

/// Lowering warnings carry an absolute source location. Compare their text
/// up to the location; the tie warning also names whichever same-depth port
/// is declared first, so that name is masked to keep permuted twins equal.
fn assert_collapse_warnings(fixture: &str, expected: &str, warnings: &[&str]) {
    assert!(
        llg::sim::build::cmake_available(),
        "CLI tests require CMake"
    );
    for optimized in [false, true] {
        let output = sim_cli::invoke_with_env(SUITE, fixture, optimized, &[], &[], &[]);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let label = format!("{fixture}, optimized={optimized}");
        assert!(output.status.success(), "{label}: {stderr}");
        assert_eq!(String::from_utf8_lossy(&output.stdout), expected, "{label}");
        let mut actual = Vec::new();
        for line in stderr.lines() {
            if let Some(warning) = line.strip_prefix("llg: warning: ") {
                let (body, location) = warning
                    .rsplit_once(" at ")
                    .unwrap_or_else(|| panic!("{label}: unlocated warning {warning}"));
                assert!(location.contains(".sv:"), "{label}: {warning}");
                let body = if body.contains("same-depth connections") {
                    let (head, tail) = body.split_once('`').expect("port name");
                    let (_, tail) = tail.split_once('`').expect("port name end");
                    format!("{head}`*`{tail}")
                } else {
                    body.to_owned()
                };
                actual.push(body);
            } else {
                assert!(line.starts_with("Warning: "), "{label}: {stderr}");
            }
        }
        actual.sort_unstable();
        let mut expected_warnings = warnings.iter().map(|w| w.to_string()).collect::<Vec<_>>();
        expected_warnings.sort_unstable();
        assert_eq!(actual, expected_warnings, "{label}");
    }
}

const CHAIN_WARNINGS: &[&str] = &[
    "dissimilar inout port `tb.mb.m`: internal Tri0, external Wor; table choice Wor (IEEE 1800-2009 Table 23-1)",
    "dissimilar inout port `tb.mb.leaf.p`: internal Wor, external Tri0; table choice Tri0 (IEEE 1800-2009 Table 23-1)",
    "dissimilar inout port `tb.mn.leaf.p`: internal Wand, external Wor; table choice Wor (IEEE 1800-2009 Table 23-1)",
    "dissimilar inout port `*`: same-depth connections choose Wand among non-dominating [Wand, Wor] by column order (IEEE 1800-2009 Table 23-1)",
];

#[test]
fn inout_chains_resolve_independently_of_declaration_order() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_011/inout_chains.out");
    for fixture in ["inout_chains", "inout_chains_permuted"] {
        assert_collapse_warnings(fixture, expected, CHAIN_WARNINGS);
    }
    sim_cli::run_case_backend_parity(SUITE, "inout_chains", expected, &[], &[]);
    sim_cli::run_case_backend_parity(SUITE, "inout_chains_permuted", expected, &[], &[]);
}

#[test]
fn net_array_rows_and_cells_collapse_through_three_levels() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_011/net_array_chains.out");
    assert_collapse_warnings(
        "net_array_chains",
        expected,
        &[
            "dissimilar inout port `tb.u.l0.q`: internal Wor, external Wand; table choice Wand (IEEE 1800-2009 Table 23-1)",
            "dissimilar inout port `tb.u.l1.q`: internal Wor, external Wand; table choice Wand (IEEE 1800-2009 Table 23-1)",
        ],
    );
    sim_cli::run_case_backend_parity(SUITE, "net_array_chains", expected, &[], &[]);
}

#[test]
fn uwire_actuals_collapse_with_one_driver() {
    let witness = include_str!("../fixtures/sim/feature_completion/rtl_011/uwire_inout.out");
    sim_cli::run_case(SUITE, "uwire_inout", witness, "", &[]);
    sim_cli::run_case_after_db_drop(SUITE, "uwire_inout", witness);
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_011/uwire_collapse.out");
    assert_collapse_warnings(
        "uwire_collapse",
        expected,
        &["dissimilar inout port `tb.u_wand.w`: internal Wand, external Uwire; table choice Uwire (IEEE 1800-2009 Table 23-1)"],
    );
    sim_cli::run_case_backend_parity(SUITE, "uwire_collapse", expected, &[], &[]);
    let formal = include_str!("../fixtures/sim/feature_completion/rtl_011/uwire_inout_formal.out");
    sim_cli::run_case(SUITE, "uwire_inout_formal", formal, "", &[]);
    sim_cli::run_case_after_db_drop(SUITE, "uwire_inout_formal", formal);
}

#[test]
fn neg_uwire_drivers_and_switches() {
    const DRIVERS: &str = "a collapsed uwire net has 2 drivers";
    sim_cli::reject_case(SUITE, "neg_uwire_collapsed_drivers", DRIVERS);
    sim_cli::reject_case(SUITE, "neg_uwire_cell_drivers", DRIVERS);
    sim_cli::reject_case(
        SUITE,
        "neg_uwire_two_drivers",
        "'uwire' net 'a' cannot have multiple drivers",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_uwire_pass_switch",
        "'uwire' net 'a' cannot be connected to 'inout' port",
    );
}

#[test]
fn member_aliases_share_projected_bits() {
    let witness = include_str!("../fixtures/sim/feature_completion/rtl_011/alias_member.out");
    sim_cli::run_case(SUITE, "alias_member", witness, "", &[]);
    sim_cli::run_case_after_db_drop(SUITE, "alias_member", witness);
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_011/alias_projections.out");
    sim_cli::run_case(SUITE, "alias_projections", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "alias_projections", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "alias_projections", expected);
}

#[test]
fn generated_wide_collapse_composes_with_member_alias() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_011/composition.out");
    sim_cli::run_case(SUITE, "composition", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "composition", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "composition", expected);
}

/// Last value per VCD identifier at each timestamp, keyed by hierarchical
/// name, with consecutive repeats removed.
fn vcd_histories(vcd: &str) -> BTreeMap<String, Vec<String>> {
    let mut scopes = Vec::new();
    let mut names = BTreeMap::new();
    let mut lines = vcd.lines();
    for line in lines.by_ref() {
        let fields = line.split_whitespace().collect::<Vec<_>>();
        match fields.as_slice() {
            ["$scope", _, name, "$end"] => scopes.push(name.to_string()),
            ["$upscope", "$end"] => {
                scopes.pop();
            }
            ["$var", _, _, id, name, "$end"] => {
                names.insert(id.to_string(), format!("{}.{name}", scopes.join(".")));
            }
            ["$enddefinitions", "$end"] => break,
            _ => {}
        }
    }
    let mut histories: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut step: BTreeMap<String, String> = BTreeMap::new();
    let mut flush = |step: &mut BTreeMap<String, String>| {
        for (id, value) in std::mem::take(step) {
            let history = histories.entry(names[&id].clone()).or_default();
            if history.last() != Some(&value) {
                history.push(value);
            }
        }
    };
    for line in lines {
        if line.starts_with('#') {
            flush(&mut step);
        } else if let Some(rest) = line.strip_prefix('b') {
            let (value, id) = rest.split_once(' ').expect("vector change");
            step.insert(id.to_owned(), value.to_owned());
        }
    }
    flush(&mut step);
    histories
}

#[test]
fn alias_identity_covers_writes_force_release_and_dumps() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_011/alias_identity.out");
    sim_cli::run_case(SUITE, "alias_identity", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "alias_identity", expected, &[], &[]);
    // No Db-drop run: that helper executes the model in the test process's
    // working directory, where `$dumpfile` would leave a waveform behind.

    // The waveform of every view follows the same network values as the
    // monitored output: s, v, m and the child port c.p, in that column order.
    let mut columns: [Vec<String>; 4] = Default::default();
    for line in expected.lines() {
        for (column, value) in line.split_whitespace().skip(1).enumerate() {
            if columns[column].last().map(String::as_str) != Some(value) {
                columns[column].push(value.to_owned());
            }
        }
    }
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/feature_completion/rtl_011/alias_identity.sv");
    for optimized in [false, true] {
        let directory = sim_harness::TempDir::new("rtl_011_alias_vcd").expect("VCD directory");
        let mut command = Command::new(env!("CARGO_BIN_EXE_llg"));
        command.current_dir(directory.path()).args(["--top", "tb"]);
        if !optimized {
            command.arg("--no-opt");
        }
        command.arg(&source);
        let output = sim_harness::run_command(&mut command, Duration::from_secs(180))
            .expect("run alias_identity");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let vcd = std::fs::read_to_string(directory.path().join("alias_identity.vcd"))
            .expect("alias_identity.vcd");
        let histories = vcd_histories(&vcd);
        for (name, column) in ["tb.s", "tb.v", "tb.m", "tb.c.p"].iter().zip(&columns) {
            // The first dump records the pre-initialization state; the
            // settled history must end with exactly the monitored sequence.
            let history = histories.get(*name).expect("dumped alias view");
            assert!(
                history.ends_with(column),
                "{name}, optimized={optimized}: {history:?} does not end with {column:?}"
            );
        }
    }
}

#[test]
fn neg_aliases_keep_language_restrictions() {
    const SELF: &str = "cannot alias a net to itself";
    sim_cli::reject_case(SUITE, "neg_alias_member_self", SELF);
    sim_cli::reject_case(SUITE, "neg_alias_member_overlap", SELF);
    sim_cli::reject_case(
        SUITE,
        "neg_alias_member_duplicate",
        "cannot specify an alias between the same bits of the same nets more than once",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_alias_member_variable",
        "'x' is not a net and so cannot be used in a net alias statement",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_alias_variable",
        "'a' is not a net and so cannot be used in a net alias statement",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_alias_member_net_type",
        "all nets in a net alias statement must have a common nettype",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_alias_member_width",
        "all aliased nets must have the same width",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_alias_member_runtime",
        "reference to non-constant variable 'i' is not allowed in a constant expression",
    );
    const CROSS_SCOPE: &str = "cannot use hierarchical references in net alias statements";
    sim_cli::reject_case(SUITE, "neg_alias_cross_scope", CROSS_SCOPE);
    sim_cli::reject_case(SUITE, "neg_alias_member_cross_scope", CROSS_SCOPE);
}
