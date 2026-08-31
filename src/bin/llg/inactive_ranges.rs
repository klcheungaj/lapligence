//! Inactive-region computation for conditional-compilation directives.
//!
//! Evaluates `` `ifdef``/`` `ifndef``/`` `elsif``/`` `else``/`` `endif``
//! groups over one source text against the EFFECTIVE define set — the root's
//! `[compile] defines` plus in-source `` `define``/`` `undef``/
//! `` `undefineall`` applied in source order (IEEE 1800 skipped-group rules:
//! define directives inside inactive regions never take effect, while the
//! conditional directives themselves stay structurally tracked so nesting
//! balances).  The result is the list of zero-based inclusive line ranges a
//! preprocessor would SKIP under that configuration; editors render them as
//! dimmed "inactive regions".
//!
//! Fail-safe policy: broken or unrecognized structure NEVER hides code.
//! Unterminated groups at EOF discard every computed range for the file,
//! stray closers are ignored, unparseable conditions open a frame that
//! inherits the enclosing visibility and can never hide anything, and
//! comment/string content is never lexed as directives (state machine
//! mirroring the client-side folding scanner).
//!
//! The computation is a pure lexical scan: it runs no Surelog work and never
//! triggers parsing.

use std::collections::HashSet;

use serde::Serialize;

/// One dimmed region as wire data: zero-based INCLUSIVE line span.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LineRange {
    pub(crate) start_line: u32,
    pub(crate) end_line: u32,
}

/// One conditional-compilation group frame (IEEE 1800 22.5).
struct Frame {
    /// Visibility outside this group: an inactive parent keeps every branch
    /// here inactive regardless of their conditions.
    parent_visible: bool,
    /// Whether some branch of this group already matched (first match wins;
    /// later `elsif`/`else` branches cannot reactivate).
    branch_taken: bool,
    /// First line not yet accounted for by a segment decision.
    segment_start: u32,
    /// Visibility of the segment starting at [`Frame::segment_start`].
    segment_visible: bool,
}

/// Directive events the lexer yields from CODE segments only.  Conditional
/// conditions carry the parsed macro name; `None` marks an unparseable
/// condition (fail-safe frame that can never hide anything).
enum Event {
    Ifdef { name: Option<String> },
    Ifndef { name: Option<String> },
    Elsif { name: Option<String> },
    Else,
    Endif,
    Define { name: String },
    Undef { name: String },
    UndefineAll,
}

fn is_identifier_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_'
}

fn is_identifier_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_' || ch == '$'
}

fn is_identifier(text: &str) -> bool {
    let mut chars = text.chars();
    matches!(chars.next(), Some(first) if is_identifier_start(first))
        && chars.all(is_identifier_char)
}

/// External (`[compile] defines`) entries arrive as `NAME` or `NAME=VALUE`;
/// only valid identifiers join the initial define set (invalid entries are
/// already dropped with a warning at config-parse time).
fn external_define_names(defines: &[String]) -> HashSet<String> {
    defines
        .iter()
        .filter_map(|entry| {
            let name = entry.split('=').next()?.trim();
            is_identifier(name).then(|| name.to_owned())
        })
        .collect()
}

/// Read one SV identifier after skipping whitespace and one optional opening
/// paren (`ifdef (NAME)`).  Returns `None` when no identifier is present.
fn scan_condition_name(rest: &str) -> Option<String> {
    let rest = rest.trim_start();
    let rest = rest.strip_prefix('(').map(str::trim_start).unwrap_or(rest);
    let first = rest.chars().next()?;
    if !is_identifier_start(first) {
        return None;
    }
    let mut end = rest.len();
    for (index, ch) in rest.char_indices().skip(1) {
        if !is_identifier_char(ch) {
            end = index;
            break;
        }
    }
    Some(rest[..end].to_owned())
}

/// Read the macro name after `` `define``/`` `undef``; formal arguments
/// (`NAME(args)`) stay part of the consumed definition text — definedness
/// only needs the name.
fn scan_macro_name(rest: &str) -> Option<String> {
    scan_condition_name(rest)
}

/// Whether `text`, trimmed, ends with a line-continuation backslash.
fn ends_with_continuation(text: &str) -> bool {
    text.trim_end().ends_with('\\')
}

/// Compute the sorted, merged, non-overlapping inactive line ranges for one
/// source text under `external_defines` (the `[compile] defines` names).
pub(crate) fn inactive_line_ranges(source: &str, external_defines: &[String]) -> Vec<LineRange> {
    let lines = split_lines(source);
    let mut defined = external_define_names(external_defines);
    let mut stack: Vec<Frame> = Vec::new();
    let mut spans: Vec<(u32, u32)> = Vec::new();

    let mut in_block_comment = false;
    let mut line_index = 0usize;
    while line_index < lines.len() {
        let line = lines[line_index];
        // A `` `define`` body consumes its continuation lines verbatim; the
        // lexer never sees directive-looking text inside macro bodies.
        let mut skip_to = line_index + 1;
        let mut i = 0usize;
        while i < line.len() {
            let Some(ch) = line[i..].chars().next() else {
                break;
            };
            if in_block_comment {
                if ch == '*' && line[i + 1..].starts_with('/') {
                    in_block_comment = false;
                    i += 2;
                } else {
                    i += ch.len_utf8();
                }
                continue;
            }
            if ch == '/' && line[i + 1..].starts_with('/') {
                break;
            }
            if ch == '/' && line[i + 1..].starts_with('*') {
                in_block_comment = true;
                i += 2;
                continue;
            }
            if ch == '"' {
                // Verilog strings never span a raw newline; escapes keep
                // embedded quotes from ending the string early.
                i += 1;
                while i < line.len() {
                    if line[i..].starts_with('\\') {
                        let escaped = line[i + 1..].chars().next().map_or(1, char::len_utf8);
                        i += 1 + escaped;
                        continue;
                    }
                    if line[i..].starts_with('"') {
                        i += 1;
                        break;
                    }
                    i += line[i..].chars().next().map_or(1, char::len_utf8);
                }
                continue;
            }
            if ch != '`' {
                i += ch.len_utf8();
                continue;
            }
            let word_start = i + '`'.len_utf8();
            let mut j = word_start;
            while let Some(word_ch) = line[j..].chars().next() {
                if is_identifier_char(word_ch) {
                    j += word_ch.len_utf8();
                } else {
                    break;
                }
            }
            let word = &line[word_start..j];
            if word.is_empty() {
                // Advance by the next character's UTF-8 length: a multi-byte
                // character right after a lone backtick must never land `i`
                // on a continuation byte, which would panic the char-boundary
                // slicing below (`x = \`模块;`).
                i = j + line[j..].chars().next().map_or(1, char::len_utf8);
                continue;
            }
            let rest = &line[j..];
            let directive_line = line_index as u32;
            match word {
                "ifdef" | "ifndef" | "elsif" | "else" | "endif" => {
                    let event = match word {
                        "ifdef" => Event::Ifdef {
                            name: scan_condition_name(rest),
                        },
                        "ifndef" => Event::Ifndef {
                            name: scan_condition_name(rest),
                        },
                        "elsif" => Event::Elsif {
                            name: scan_condition_name(rest),
                        },
                        "else" => Event::Else,
                        _ => Event::Endif,
                    };
                    apply_event(event, directive_line, &mut defined, &mut stack, &mut spans);
                    i = j;
                }
                "define" => {
                    if let Some(name) = scan_macro_name(rest) {
                        apply_event(
                            Event::Define { name },
                            directive_line,
                            &mut defined,
                            &mut stack,
                            &mut spans,
                        );
                    }
                    // Consume the body (plus backslash continuations) so its
                    // content is never lexed as directives; the remainder of
                    // THIS line is body too.
                    if ends_with_continuation(rest) {
                        let mut last = line_index + 1;
                        while last < lines.len() && ends_with_continuation(lines[last]) {
                            last += 1;
                        }
                        skip_to = (last + 1).min(lines.len());
                    }
                    break;
                }
                "undef" => {
                    if let Some(name) = scan_macro_name(rest) {
                        apply_event(
                            Event::Undef { name },
                            directive_line,
                            &mut defined,
                            &mut stack,
                            &mut spans,
                        );
                    }
                    break;
                }
                "undefineall" => {
                    apply_event(
                        Event::UndefineAll,
                        directive_line,
                        &mut defined,
                        &mut stack,
                        &mut spans,
                    );
                    i = j;
                }
                _ => {
                    // Macro use or unsupported directive: irrelevant to the
                    // conditional structure.
                    i = j;
                }
            }
        }
        line_index = skip_to.max(line_index + 1);
    }

    // Structural failure: unterminated conditional groups mean the ranges
    // computed so far cannot be trusted — fail ACTIVE for the whole file.
    if !stack.is_empty() {
        return Vec::new();
    }

    normalize(spans)
}

/// Split keeping exactly one entry per physical line so line numbers stay
/// exact (CRLF counts as one separator).
fn split_lines(source: &str) -> Vec<&str> {
    let mut lines: Vec<&str> = Vec::new();
    let mut current = source;
    while let Some(break_at) = current.find(['\r', '\n']) {
        lines.push(&current[..break_at]);
        if current[break_at..].starts_with("\r\n") {
            current = &current[break_at + 2..];
        } else {
            current = &current[break_at + 1..];
        }
    }
    lines.push(current);
    lines
}

/// Apply one directive event to the evaluator state.
fn apply_event(
    event: Event,
    line: u32,
    defined: &mut HashSet<String>,
    stack: &mut Vec<Frame>,
    spans: &mut Vec<(u32, u32)>,
) {
    fn visible(stack: &[Frame]) -> bool {
        stack.last().map_or(true, |frame| frame.segment_visible)
    }
    fn close_segment(frame: &mut Frame, until: u32, spans: &mut Vec<(u32, u32)>) {
        if !frame.segment_visible && until >= frame.segment_start {
            spans.push((frame.segment_start, until));
        }
    }
    fn condition_holds(name: &Option<String>, ifndef: bool, defined: &HashSet<String>) -> bool {
        match name {
            None => false,
            Some(name) => {
                let holds = defined.contains(name);
                if ifndef {
                    !holds
                } else {
                    holds
                }
            }
        }
    }
    fn open_group(
        name: Option<String>,
        ifndef: bool,
        line: u32,
        defined: &HashSet<String>,
        stack: &mut Vec<Frame>,
        spans: &mut Vec<(u32, u32)>,
    ) {
        let parent_visible = visible(stack);
        if let Some(frame) = stack.last_mut() {
            close_segment(frame, line.saturating_sub(1), spans);
        }
        // Unparseable conditions fail safe: their frame inherits the parent
        // visibility and stays untaken, so neither this body nor any sibling
        // `else` branch can be hidden by the broken group.
        let condition = condition_holds(&name, ifndef, defined);
        let branch_visible = if name.is_some() { condition } else { true };
        let wins = parent_visible && branch_visible;
        stack.push(Frame {
            parent_visible,
            branch_taken: parent_visible && condition,
            segment_start: line,
            segment_visible: wins,
        });
    }
    match event {
        Event::Ifdef { name } => open_group(name, false, line, defined, stack, spans),
        Event::Ifndef { name } => open_group(name, true, line, defined, stack, spans),
        Event::Elsif { name } => {
            let Some(frame) = stack.last_mut() else {
                return;
            };
            close_segment(frame, line.saturating_sub(1), spans);
            // Unparseable conditions fail safe: treated as matching whenever
            // no real branch has matched yet, never hiding anything extra.
            let matches =
                name.is_some() && condition_holds(&name, false, defined) || name.is_none();
            let wins = frame.parent_visible && !frame.branch_taken && matches;
            frame.segment_visible = wins;
            if matches && wins {
                frame.branch_taken = true;
            }
            frame.segment_start = line;
        }
        Event::Else => {
            let Some(frame) = stack.last_mut() else {
                return;
            };
            close_segment(frame, line.saturating_sub(1), spans);
            let wins = frame.parent_visible && !frame.branch_taken;
            frame.segment_visible = wins;
            if wins {
                frame.branch_taken = true;
            }
            frame.segment_start = line;
        }
        Event::Endif => {
            let Some(mut frame) = stack.pop() else {
                return;
            };
            // The closing directive bounds an inactive tail segment, so it is
            // part of the dimmed range when the final branch is inactive.
            close_segment(&mut frame, line, spans);
            if let Some(outer) = stack.last_mut() {
                outer.segment_start = line + 1;
            }
        }
        Event::Define { name } => {
            if visible(stack) {
                defined.insert(name);
            }
        }
        Event::Undef { name } => {
            if visible(stack) {
                defined.remove(&name);
            }
        }
        Event::UndefineAll => {
            if visible(stack) {
                defined.clear();
            }
        }
    }
}

/// Sort by start and merge overlapping or adjacent spans into contiguous
/// ranges, guaranteeing the non-overlapping output contract.
fn normalize(mut spans: Vec<(u32, u32)>) -> Vec<LineRange> {
    spans.sort_by_key(|&(start, end)| (start, end));
    let mut merged: Vec<LineRange> = Vec::with_capacity(spans.len());
    for (start, end) in spans {
        match merged.last_mut() {
            Some(last) if start <= last.end_line.saturating_add(1) => {
                last.end_line = last.end_line.max(end);
            }
            _ => merged.push(LineRange {
                start_line: start,
                end_line: end,
            }),
        }
    }
    merged
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(source: &str, defines: &[&str]) -> Vec<(u32, u32)> {
        inactive_line_ranges(
            source,
            &defines.iter().map(|d| (*d).to_owned()).collect::<Vec<_>>(),
        )
        .into_iter()
        .map(|range| (range.start_line, range.end_line))
        .collect()
    }

    #[test]
    fn ifdef_false_hides_branch_including_directive_lines() {
        let source = "\
module m;
`ifdef FOO
  logic a;
`endif
endmodule";
        assert_eq!(
            lines(source, &[]),
            vec![(1, 3)],
            "the `ifdef line, body and `endif dim as one span when undefined"
        );
        assert_eq!(lines(source, &["FOO"]), vec![]);
    }

    #[test]
    fn ifndef_selects_the_inverse_branch() {
        let source = "\
`ifndef GUARD
active
`endif
tail";
        assert_eq!(lines(source, &[]), vec![]);
        assert_eq!(lines(source, &["GUARD"]), vec![(0, 2)]);
    }

    #[test]
    fn elsif_chain_first_match_wins() {
        let source = "\
`ifdef A
a
`elsif B
b
`elsif C
c
`else
d
`endif
end";
        // Every skipped segment dims; an inactive tail branch includes the
        // closing `endif line while an active one keeps it visible.  Adjacent
        // skipped segments merge into one contiguous span.
        assert_eq!(lines(source, &[]), vec![(0, 5)]);
        assert_eq!(lines(source, &["A"]), vec![(2, 8)]);
        assert_eq!(lines(source, &["B"]), vec![(0, 1), (4, 8)]);
        assert_eq!(lines(source, &["C"]), vec![(0, 3), (6, 8)]);
        assert_eq!(
            lines(source, &["A", "B", "C"]),
            vec![(2, 8)],
            "A defined: later branches cannot reactivate"
        );
        assert_eq!(
            lines(source, &["B", "C"]),
            vec![(0, 1), (4, 8)],
            "B beats C"
        );
    }

    #[test]
    fn nested_groups_track_visibility_and_structure() {
        let source = "\
`ifdef A
outer-active
`ifdef B
inner
`endif
after-inner
`else
outer-inactive
`endif
done";
        assert_eq!(
            lines(source, &["A"]),
            vec![(2, 4), (6, 8)],
            "B undefined hides the inner group incl. its endif; A's else hides too"
        );
        assert_eq!(
            lines(source, &["A", "B"]),
            vec![(6, 8)],
            "both defined: only A's inactive `else` branch dims"
        );
        assert_eq!(
            lines(source, &[]),
            vec![(0, 5)],
            "inactive parent hides everything through `after-inner`; the taken \
             `else` branch and its `endif stay visible; adjacent spans merge"
        );
        assert_eq!(lines(source, &["B"]), vec![(0, 5)]);
    }

    #[test]
    fn unterminated_group_fails_active_for_whole_file() {
        let source = "\
`ifdef FOO
module m;
endmodule";
        assert_eq!(lines(source, &["FOO"]), vec![]);
        assert_eq!(
            lines(source, &[]),
            vec![],
            "broken structure must never produce dimmed ranges"
        );
    }

    #[test]
    fn stray_closers_are_ignored() {
        let source = "`endif\n`else\nactive\n`endif";
        assert_eq!(lines(source, &[]), vec![]);
    }

    #[test]
    fn comment_content_is_never_a_directive() {
        let source = "\
// `ifdef NOPE
real code
/* `ifdef NOPE
still comment
*/
more code";
        assert_eq!(lines(source, &[]), vec![]);
    }

    #[test]
    fn string_content_is_never_a_directive() {
        let source = "\ninitial $display(\"`ifdef STRAY\");\n`ifdef REAL\nhidden\n`endif";
        assert_eq!(lines(source, &[]), vec![(2, 4)]);
    }

    #[test]
    fn external_defines_select_branches_and_value_form_counts() {
        let source = "\
`ifdef WIDTH
wide
`endif";
        assert_eq!(lines(source, &[]), vec![(0, 2)]);
        assert_eq!(lines(source, &["WIDTH"]), vec![]);
        assert_eq!(lines(source, &["WIDTH=8"]), vec![], "NAME=VALUE defines");
    }

    #[test]
    fn in_source_define_undef_and_redefine_ordering() {
        let before_use = "\
`define FEATURE
`ifdef FEATURE
defined-by-source
`endif
`undef FEATURE
`ifdef FEATURE
gone
`endif";
        assert_eq!(lines(before_use, &[]), vec![(5, 7)]);

        let undefineall = "\
`define A
`undefineall
`ifdef A
cleared
`endif";
        assert_eq!(lines(undefineall, &[]), vec![(2, 4)]);
        assert_eq!(
            lines(undefineall, &["B"]),
            vec![(2, 4)],
            "`undefineall clears the external set too"
        );
    }

    #[test]
    fn define_inside_inactive_region_never_takes_effect() {
        let source = "\
`ifdef UNDEFINED
`define HIDDEN
`endif
`ifdef HIDDEN
must-stay-visible
`endif";
        // Lines 0-2 (broken group) and 3-5 (HIDDEN never defined) are all
        // skipped and adjacent, so they merge into one span.
        assert_eq!(lines(source, &[]), vec![(0, 5)]);
    }

    #[test]
    fn define_with_formal_arguments_records_definedness() {
        let source = "\
`define ADD(a, b) ((a) + (b))
`ifdef ADD
has-macro
`endif";
        assert_eq!(lines(source, &[]), vec![]);
    }

    #[test]
    fn unparseable_condition_fails_safe() {
        let source = "\
`ifdef
body shown
`else
also shown
`endif
shown too";
        assert_eq!(lines(source, &[]), vec![]);
    }

    #[test]
    fn parenthesized_condition_is_accepted() {
        let source = "\
`ifdef (PAREN)
hidden
`endif";
        assert_eq!(lines(source, &[]), vec![(0, 2)]);
        assert_eq!(lines(source, &["PAREN"]), vec![]);
    }

    #[test]
    fn output_is_sorted_non_overlapping_with_gaps_preserved() {
        let source = "\
`ifdef X
a
`endif
keep
`ifdef Y
b
`endif
keep
`ifdef Z
c
`endif";
        let result = lines(source, &[]);
        assert_eq!(result, vec![(0, 2), (4, 6), (8, 10)]);
        for pair in result.windows(2) {
            assert!(
                pair[0].1 + 1 < pair[1].0,
                "ranges separated by active code must stay distinct: {result:?}"
            );
        }
    }

    #[test]
    fn touching_ranges_merge_into_one_contiguous_span() {
        let source = "\
`ifdef X
`endif
`ifdef Y
`endif";
        assert_eq!(lines(source, &[]), vec![(0, 3)]);
    }

    #[test]
    fn multiline_define_body_consumes_directive_looking_text() {
        let source = "\
`define BLOCK \\
initial begin \\
  // looks like: `ifdef INSIDE
end
`ifdef BLOCK
uses-block
`endif";
        assert_eq!(
            lines(source, &[]),
            vec![],
            "BLOCK is defined and body text never lexes as directives"
        );
    }

    #[test]
    fn crlf_sources_keep_line_numbers_exact() {
        let source = "`ifdef FOO\r\nhidden\r\n`endif\r\ntail";
        assert_eq!(lines(source, &[]), vec![(0, 2)]);
    }

    #[test]
    fn multibyte_char_after_lone_backtick_does_not_panic() {
        // Regression: the empty-word branch used to advance one BYTE onto a
        // UTF-8 continuation byte and panic on char-boundary slicing. In
        // production that panic was swallowed to empty ranges, silently
        // killing dimming for the whole file.
        let source = "x = `模块;\n`ifdef FOO\nhidden\n`endif";
        assert_eq!(lines(source, &[]), vec![(1, 3)]);

        let accented = "`é\n`ifdef FOO\nhidden\n`endif";
        assert_eq!(lines(accented, &[]), vec![(1, 3)]);

        let trailing_backtick = "x = y + `";
        assert_eq!(lines(trailing_backtick, &[]), vec![]);
    }

    #[test]
    fn multibyte_char_after_directive_keyword_fails_safe() {
        // The directive word parses; its condition is unparseable (the first
        // condition character is multi-byte), so the group fails ACTIVE and
        // nothing dims.
        let source = "`ifdef 模块\nshown\n`else\nalso shown\n`endif\ntail";
        assert_eq!(lines(source, &[]), vec![]);
    }
}
