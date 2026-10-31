//! Capture contract of same-named definitions in several files: the frontend
//! elaborates one definition per name, and every other body is captured as
//! an uninstantiated instance (never a top, no diagnostics of its own) so
//! the names it declares bind within the file that declares them.

use llg::ffi::slang::{
    self, CompileOptions, CompileRequest, DiagnosticSeverity, LexicalToken, SemanticKind, Snapshot,
    Source,
};

// `dup_leaf` is instantiated (the shadowed body is reachable from no
// instance) and `dup_top` is the automatic top (the shadowed one is a second
// top candidate); the two variants differ in layout and expressions.
const FIRST: &str = "\
module dup_leaf(input logic a, output logic y);
  logic t;
  assign t = a;
  assign y = t;
endmodule

module dup_top(input logic i, output logic o, output logic q);
  logic n;
  dup_leaf u(.a(i), .y(n));
  assign o = n;
  assign q = o;
endmodule
";

const SECOND: &str = "\
// second variant
module dup_leaf(input logic a, output logic y);

  logic t;
  assign t = ~a;
  assign y = t;
endmodule
module dup_top(input logic i, output logic o, output logic q);

  logic n;
  dup_leaf u(.a(i), .y(n));
  assign o = ~n;
  assign q = o;
endmodule
";

/// Uses whose declaration is in the same body: (use needle, skip into it,
/// declaration needle, skip into it), present in both variants.
const LOCAL_USES: &[(&str, usize, &str, usize)] = &[
    ("a;\n", 0, "input logic a", 12),
    ("t;\nendmodule", 0, "logic t;", 6),
    ("t = ", 0, "logic t;", 6),
    ("(i)", 1, "input logic i", 12),
    ("(n)", 1, "logic n;", 6),
    ("n;\n  assign q", 0, "logic n;", 6),
    ("o;\nendmodule", 0, "output logic o", 13),
];

fn capture(library_units: bool) -> Snapshot {
    let sources = [
        Source::compilation_unit("dup_first.sv", FIRST),
        Source::compilation_unit("dup_second.sv", SECOND),
    ];
    let options = CompileOptions {
        library_units,
        ..Default::default()
    };
    slang::compile(&CompileRequest {
        sources: &sources,
        library_sources: &[],
        options: &options,
    })
    .expect("duplicate definitions yield a snapshot")
}

fn file_id(snapshot: &Snapshot, name: &str) -> u64 {
    snapshot
        .files
        .iter()
        .find(|file| file.name.ends_with(name))
        .unwrap_or_else(|| panic!("no file {name}"))
        .id
}

fn offset(source: &str, needle: &str, skip: usize) -> u64 {
    (source
        .find(needle)
        .unwrap_or_else(|| panic!("{needle:?} in source"))
        + skip) as u64
}

fn token_at(snapshot: &Snapshot, file: u64, start: u64) -> &LexicalToken {
    snapshot
        .lexical_tokens
        .iter()
        .find(|token| {
            !token.is_missing
                && !token.is_skipped
                && token
                    .range
                    .is_some_and(|range| range.file_id == file && range.start == start)
        })
        .unwrap_or_else(|| panic!("no lexical token at {file}:{start}"))
}

/// The declaration `(file, start)` of the semantic node a token binds to.
fn bound(snapshot: &Snapshot, token: &LexicalToken) -> Option<(u64, u64)> {
    let id = token.semantic_id?;
    let node = snapshot.semantic_nodes.iter().find(|node| node.id == id)?;
    let range = node.range?;
    Some((range.file_id, range.start))
}

#[test]
fn shadowed_definition_bodies_bind_within_their_own_file_in_both_capture_modes() {
    for library_units in [false, true] {
        let snapshot = capture(library_units);
        let mode = if library_units {
            "declaration-only"
        } else {
            "full"
        };
        for (name, source) in [("dup_first.sv", FIRST), ("dup_second.sv", SECOND)] {
            let file = file_id(&snapshot, name);
            for (needle, skip, declaration, declaration_skip) in LOCAL_USES {
                let token = token_at(&snapshot, file, offset(source, needle, *skip));
                assert_eq!(
                    bound(&snapshot, token),
                    Some((file, offset(source, declaration, *declaration_skip))),
                    "{mode} {name} {needle:?}+{skip}"
                );
            }
        }
        if library_units {
            continue;
        }
        // One elaborated top; the shadowed bodies are never tops and add no
        // diagnostics beyond the frontend's duplicate-definition warnings.
        let tops: Vec<&str> = snapshot
            .semantic_nodes
            .iter()
            .filter(|node| node.kind == SemanticKind::Instance && node.is_top())
            .map(|node| node.name.as_str())
            .collect();
        assert_eq!(tops, ["dup_top"]);
        let mut findings: Vec<&str> = snapshot
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity != DiagnosticSeverity::Note)
            .map(|diagnostic| diagnostic.name.as_str())
            .filter(|name| !name.starts_with("Unused"))
            .collect();
        findings.sort_unstable();
        assert_eq!(
            findings,
            ["DuplicateDefinition", "DuplicateDefinition"],
            "{:?}",
            snapshot.diagnostics
        );
        let database = llg::core::db::Db::from_slang(&snapshot).expect("owned database");
        assert_eq!(database.tops().len(), 1);
    }
}
