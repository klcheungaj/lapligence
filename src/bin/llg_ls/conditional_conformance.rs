//! Cross-scanner conformance corpus for conditional-compilation directives.
//!
//! Test-only module (compiled under `cfg(test)` only): it feeds ONE table of
//! tricky snippets through BOTH evaluators — the core macro-table scanner
//! (`llg::core::macros`, backing macro hover) and the bin's inactive-range
//! scanner (`crate::inactive_ranges`, backing editor dimming) — and asserts
//! they agree on whether the probe region is LIVE. This is the regression net
//! for the empirically verified divergence where `` `ifdef (PAREN) `` was
//! accepted by the dimmer but rejected by hover ("undimmed code whose macro
//! hover claims not defined").
//!
//! Agreement is measured mechanically: every snippet ends with a
//! `` `CORPUS_PROBE `` usage line. Hover reports liveness as "the probe
//! usage was recorded at all" (dead regions record nothing); the dimmer
//! reports liveness as "no range covers the probe line". Both must equal the
//! expected value AND each other. Sources are always fully terminated so the
//! dimmer's whole-file fail-active rule stays out of the picture.

use llg::core::macros::build_table;

use crate::inactive_ranges::inactive_line_ranges;

const FILE: &str = "corpus.sv";
const PROBE_USAGE: &str = "`CORPUS_PROBE";

/// Asserts both scanners agree on the liveness of the probe line.
fn assert_probe_liveness(name: &str, source: &str, defines: &[&str], expect_live: bool) {
    let probe_line0 = source
        .lines()
        .position(|line| line.contains(PROBE_USAGE))
        .expect("corpus source must contain the probe usage") as u32;

    let owned_defines = defines
        .iter()
        .map(|define| (*define).to_owned())
        .collect::<Vec<_>>();
    let dimmed = inactive_line_ranges(source, &owned_defines)
        .iter()
        .any(|range| range.start_line <= probe_line0 && probe_line0 <= range.end_line);

    let args = defines
        .iter()
        .map(|define| (*define).to_owned())
        .collect::<Vec<_>>();
    let table = build_table(&args, &[(FILE, source)], None);
    // Column 1 sits inside the probe's `` `NAME `` span (backtick occupies
    // column 0); dead regions record no usage at all.
    let hover_live = table.usage_at(FILE, probe_line0, 1).is_some();

    assert_eq!(
        hover_live, !dimmed,
        "[{name}] hover and dimmer disagree about the probe line \
         (hover live={hover_live}, dimmed={dimmed})"
    );
    assert_eq!(
        hover_live, expect_live,
        "[{name}] unexpected probe liveness (hover live={hover_live}, \
         dimmed={dimmed}, expected live={expect_live})"
    );
}

/// Runs one corpus entry through both scanners.  `defines` mirrors
/// `[compile] defines` (`NAME` / `NAME=VALUE` entries).
macro_rules! corpus {
    ($($name:ident => $source:expr, $defines:expr, $live:expr);+ $(;)?) => {
        $(
            #[test]
            fn $name() {
                assert_probe_liveness(stringify!($name), $source, $defines, $live);
            }
        )+
    };
}

corpus! {
    plain_ifdef_undefined_hides =>
        "`ifdef FOO\n`CORPUS_PROBE\n`endif", &[], false;
    plain_ifdef_defined_shows =>
        "`ifdef FOO\n`CORPUS_PROBE\n`endif", &["FOO"], true;
    paren_ifdef_undefined_hides =>
        "`ifdef (FOO)\n`CORPUS_PROBE\n`endif", &[], false;
    paren_ifdef_defined_shows =>
        "`ifdef (FOO)\n`CORPUS_PROBE\n`endif", &["FOO"], true;
    paren_ifndef_defined_hides =>
        "`ifndef (FOO)\n`CORPUS_PROBE\n`endif", &["FOO"], false;
    paren_ifndef_undefined_shows =>
        "`ifndef (FOO)\n`CORPUS_PROBE\n`endif", &[], true;
    unclosed_paren_still_parses_the_identifier =>
        "`ifdef (FOO\n`CORPUS_PROBE\n`endif", &["FOO"], true;
    double_open_paren_fails_safe_active_in_both =>
        "`ifdef ((FOO)\n`CORPUS_PROBE\n`endif", &["FOO"], true;
    double_open_paren_fails_safe_active_without_define =>
        "`ifdef ((FOO)\n`CORPUS_PROBE\n`endif", &[], true;
    bare_ifdef_fails_safe_active =>
        "`ifdef\n`CORPUS_PROBE\n`endif", &[], true;
    bare_ifndef_fails_safe_active =>
        "`ifndef\n`CORPUS_PROBE\n`endif", &[], true;
    bare_elsif_fails_safe_matches_after_open_group =>
        "`ifdef MISSING\nwrong\n`elsif\n`CORPUS_PROBE\n`endif", &[], true;
    paren_elsif_takes_branch_when_defined =>
        "`ifdef A\nwrong\n`elsif (B)\n`CORPUS_PROBE\n`endif", &["B"], true;
    paren_elsif_dead_when_first_branch_taken =>
        "`ifdef A\nwrong\n`elsif (B)\n`CORPUS_PROBE\n`endif", &["A"], false;
    else_takes_remainder_when_all_conditions_fail =>
        "`ifdef A\nwrong\n`elsif (B)\nwrong\n`else\n`CORPUS_PROBE\n`endif", &[], true;
    nested_else_live_when_inner_condition_undefined =>
        concat!(
            "`ifdef (A)\n",
            "`ifdef B\nwrong\n",
            "`else\n",
            "`CORPUS_PROBE\n",
            "`endif\n",
            "`else\n",
            "wrong2\n",
            "`endif"
        ),
        &["A"],
        true;
    nested_body_hidden_when_inner_condition_undefined =>
        concat!(
            "`ifdef (A)\n",
            "`ifdef B\n",
            "`CORPUS_PROBE\n",
            "`endif\n",
            "`else\n",
            "wrong2\n",
            "`endif"
        ),
        &["A"],
        false;
    nested_inner_branch_dead_when_taken =>
        concat!(
            "`ifdef (A)\n",
            "`ifdef B\nwrong\n",
            "`else\n",
            "`CORPUS_PROBE\n",
            "`endif\n",
            "`else\n",
            "wrong2\n",
            "`endif"
        ),
        &["A", "B"],
        false;
    in_source_define_feeds_paren_condition =>
        "`define FOO 1\n`ifdef (FOO)\n`CORPUS_PROBE\n`endif", &[], true;
    in_source_undef_removes_paren_condition =>
        "`define FOO 1\n`undef FOO\n`ifdef (FOO)\n`CORPUS_PROBE\n`endif", &[], false;
    trailing_text_after_condition_agrees =>
        "`ifdef FOO extra\n`CORPUS_PROBE\n`endif", &["FOO"], true;
    comment_after_condition_agrees =>
        "`ifdef FOO // note\n`CORPUS_PROBE\n`endif", &["FOO"], true;
    visibility_restores_after_closing_directive =>
        "`ifdef FOO\nwrong\n`endif\n`CORPUS_PROBE", &[], true;
    visibility_restores_when_group_was_taken =>
        "`ifdef FOO\nwrong\n`endif\n`CORPUS_PROBE", &["FOO"], true;
}
