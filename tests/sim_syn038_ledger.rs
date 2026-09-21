//! Structural checks for the finite SYN-038 grammar/context evidence ledger.
//!
//! This test does not execute HDL. It keeps the documented denominator
//! reviewable: every selected row has a stable ID, an edition, an expected
//! outcome, and a checked-in fixture, while the profile and historical group
//! tables remain complete.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

const LEDGER_START: &str = "### SYN-038 selected Core grammar-by-context ledger";
const DISPOSITION_START: &str = "#### SYN-038 72-group disposition";
const LEDGER_END: &str = "### Compilation-unit grouping";

fn section<'a>(document: &'a str, start: &str, end: &str) -> &'a str {
    let start_at = document
        .find(start)
        .unwrap_or_else(|| panic!("missing SYN-038 section marker {start:?}"));
    let body = &document[start_at..];
    let end_at = body
        .find(end)
        .unwrap_or_else(|| panic!("missing SYN-038 section end marker {end:?}"));
    &body[..end_at]
}

fn table_cells(line: &str) -> Vec<&str> {
    line.trim_matches('|').split('|').map(str::trim).collect()
}

fn code_spans(value: &str) -> impl Iterator<Item = &str> {
    value
        .split('`')
        .enumerate()
        .filter_map(|(index, part)| (index % 2 == 1).then_some(part))
}

fn fixture_paths(value: &str) -> Vec<PathBuf> {
    code_spans(value)
        .filter(|span| span.starts_with("tests/"))
        .map(PathBuf::from)
        .collect()
}

fn assert_fixture_exists(root: &Path, row_id: &str, value: &str) {
    let paths = fixture_paths(value);
    assert!(
        !paths.is_empty(),
        "{row_id} must name a checked-in fixture path in backticks"
    );
    for path in paths {
        assert!(
            root.join(&path).is_file(),
            "{row_id} fixture does not exist: {}",
            path.display()
        );
    }
}

#[test]
fn selected_rows_are_traceable_and_unique() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let document = fs::read_to_string(root.join("docs/sim_features.md"))
        .expect("read the maintained simulator feature document");
    let ledger = section(&document, LEDGER_START, LEDGER_END);
    let selected = ledger
        .lines()
        .filter(|line| line.starts_with("| SYN038-CORE-"))
        .collect::<Vec<_>>();
    assert!(
        selected.len() >= 70,
        "SYN-038 needs the selected Core grammar rows, found {}",
        selected.len()
    );

    let mut ids = HashSet::new();
    for line in selected {
        let cells = table_cells(line);
        assert_eq!(cells.len(), 6, "malformed selected SYN-038 row: {line}");
        let id = cells[0];
        assert!(ids.insert(id), "duplicate selected SYN-038 ID: {id}");
        assert!(
            matches!(cells[1], "V2001" | "SV2009" | "V2001/SV2009"),
            "{id} has no exact edition gate: {}",
            cells[1]
        );
        assert!(
            cells[2].contains("B.") || cells[2].contains("Annex A") || cells[2].contains("SYN-"),
            "{id} has no Annex A/B production or named extension: {}",
            cells[2]
        );
        assert!(
            matches!(cells[5], "PASS" | "REJECT"),
            "{id} has no explicit expected outcome: {}",
            cells[5]
        );
        assert!(
            !cells[2].contains("TBD") && !cells[2].contains("unassigned"),
            "{id} leaves its selected production unresolved"
        );
        assert_fixture_exists(&root, id, cells[4]);
    }
}

#[test]
fn edition_gates_match_sv_only_boundaries_and_witnesses() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let document = fs::read_to_string(root.join("docs/sim_features.md"))
        .expect("read the maintained simulator feature document");
    let ledger = section(&document, LEDGER_START, LEDGER_END);

    // These rows either name an SV-only production or use a witness whose
    // syntax is SV-only. Keep their edition gate strict so a shared V/SV label
    // cannot accidentally make the PASS evidence look legal in V2001.
    let sv_only_rows = [
        "SYN038-CORE-LX-02",
        "SYN038-CORE-LX-03",
        "SYN038-CORE-LX-05",
        "SYN038-CORE-LX-06",
        "SYN038-CORE-TY-01",
        "SYN038-CORE-TY-02",
        "SYN038-CORE-TY-03",
        "SYN038-CORE-TY-04",
        "SYN038-CORE-TY-09",
        "SYN038-CORE-EX-01",
        "SYN038-CORE-EX-05",
        "SYN038-CORE-EX-06",
        "SYN038-CORE-EX-07",
        "SYN038-CORE-EX-08",
        "SYN038-CORE-AS-01",
        "SYN038-CORE-AS-02",
        "SYN038-CORE-AS-03",
        "SYN038-CORE-AS-09",
        "SYN038-CORE-PR-02",
        "SYN038-CORE-PR-07",
        "SYN038-CORE-PR-08",
        "SYN038-CORE-SB-01",
        "SYN038-CORE-SB-02",
        "SYN038-CORE-SB-04",
        "SYN038-CORE-SB-07",
        "SYN038-CORE-SB-08",
        "SYN038-CORE-HY-01",
        "SYN038-CORE-HY-02",
        "SYN038-CORE-HY-03",
        "SYN038-CORE-HY-04",
        "SYN038-CORE-HY-05",
        "SYN038-CORE-HY-06",
        "SYN038-CORE-HY-10",
        "SYN038-CORE-ED-05",
    ];
    for id in sv_only_rows {
        let line = ledger
            .lines()
            .find(|line| line.starts_with(&format!("| {id} |")))
            .unwrap_or_else(|| panic!("missing edition-audit row: {id}"));
        let cells = table_cells(line);
        assert_eq!(cells.len(), 6, "malformed edition-audit row: {line}");
        assert_eq!(cells[1], "SV2009", "{id} overclaims V2001 legality");
    }

    // A V2001/SV2009 PASS row may use a .sv witness only when that witness
    // has been manually audited as dual-edition syntax. New mixed-edition
    // rows must use a .v witness until they receive the same review.
    let dual_edition_sv_witnesses = [
        "SYN038-CORE-LX-01",
        "SYN038-CORE-LX-04",
        "SYN038-CORE-EX-02",
        "SYN038-CORE-PR-01",
        "SYN038-CORE-PR-04",
        "SYN038-CORE-PR-09",
        "SYN038-CORE-PR-10",
        "SYN038-CORE-SB-06",
        "SYN038-CORE-PI-01",
        "SYN038-CORE-PI-03",
        "SYN038-CORE-PI-04",
    ];
    for line in ledger
        .lines()
        .filter(|line| line.starts_with("| SYN038-CORE-") && line.contains("| V2001/SV2009 |"))
    {
        let cells = table_cells(line);
        let id = cells[0];
        for path in fixture_paths(cells[4]) {
            if path.extension().and_then(|extension| extension.to_str()) == Some("sv") {
                assert!(
                    dual_edition_sv_witnesses.contains(&id),
                    "{id} uses an unaudited .sv witness for a V2001/SV2009 row"
                );
            }
        }
    }
}

#[test]
fn exclusions_and_context_axes_are_explicit() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let document = fs::read_to_string(root.join("docs/sim_features.md"))
        .expect("read the maintained simulator feature document");
    let ledger = section(&document, LEDGER_START, LEDGER_END);

    for axis in [
        "Declaration/type",
        "Shape",
        "Width",
        "Storage/lifetime",
        "Expression consumer",
        "Destination",
        "Driver/process",
        "Values/effects",
        "Pipeline",
        "Interaction chain",
    ] {
        assert!(
            ledger.contains(&format!("| {axis} |")),
            "SYN-038 context axis is not represented: {axis}"
        );
    }

    let exclusions = ledger
        .lines()
        .filter(|line| line.starts_with("| SYN038-EX-"))
        .collect::<Vec<_>>();
    assert!(
        exclusions.len() >= 12,
        "selected profile exclusions must remain explicit"
    );
    let mut exclusion_ids = HashSet::new();
    for line in exclusions {
        let cells = table_cells(line);
        assert_eq!(cells.len(), 5, "malformed SYN-038 exclusion row: {line}");
        assert!(
            exclusion_ids.insert(cells[0]),
            "duplicate exclusion ID: {}",
            cells[0]
        );
        assert!(
            matches!(cells[1], "V2001" | "SV2009" | "V2001/SV2009"),
            "{} has no exact exclusion edition: {}",
            cells[0],
            cells[1]
        );
        assert!(
            !cells[2].is_empty(),
            "{} has no excluded production",
            cells[0]
        );
        assert!(!cells[4].is_empty(), "{} has no exclusion reason", cells[0]);
        let paths = fixture_paths(cells[3]);
        for path in paths {
            assert!(
                root.join(&path).is_file(),
                "{} exclusion fixture does not exist: {}",
                cells[0],
                path.display()
            );
        }
    }
}

#[test]
fn all_historical_groups_have_one_disposition() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let document = fs::read_to_string(root.join("docs/sim_features.md"))
        .expect("read the maintained simulator feature document");
    let ledger = section(&document, LEDGER_START, LEDGER_END);
    let disposition = ledger
        .split_once(DISPOSITION_START)
        .map(|(_, rest)| rest)
        .expect("SYN-038 disposition marker");

    let mut ids = Vec::new();
    let allowed = ["CORE", "RETAIN", "EXT", "POLICY", "CAPACITY", "OUTSIDE"];
    for line in disposition.lines().filter(|line| line.starts_with('|')) {
        let cells = table_cells(line);
        if cells.len() != 4 || cells[0] == "Old ID" || cells[0].starts_with("---") {
            continue;
        }
        let id = cells[0]
            .parse::<u8>()
            .unwrap_or_else(|_| panic!("invalid old group ID in row: {line}"));
        assert!((1..=72).contains(&id), "old group ID outside 1..=72: {id}");
        assert!(
            allowed.iter().any(|code| cells[2].contains(code)),
            "old group {id} has no plan disposition: {}",
            cells[2]
        );
        assert!(
            !cells[3].is_empty(),
            "old group {id} has no evidence boundary"
        );
        ids.push(id);
    }
    ids.sort_unstable();
    assert_eq!(ids.len(), 72, "SYN-038 must classify all 72 old groups");
    assert_eq!(ids, (1..=72).collect::<Vec<_>>());
}
