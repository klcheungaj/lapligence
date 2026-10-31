//! Capture contract of the names in connection actuals that Slang never
//! binds (ABI v61, docs/semantic_tokens.md ST-76): a named connection to a
//! port the definition does not declare (PortDoesNotExist) or repeating a
//! port (DuplicatePortConnection). Lexical bindings only: no semantic node
//! and no diagnostic beyond Slang's own come from these actuals.

use llg::ffi::slang::{
    self, CompileOptions, CompileRequest, LexicalToken, SemanticKind, Snapshot, Source,
};

const SOURCE: &str = "module uc_probe(input logic clk, input logic [31:0] a0);\n\
endmodule\n\
module uc_top;\n\
  typedef struct packed { logic [7:0] f; } rec_t;\n\
  logic clk;\n\
  reg st [2];\n\
  logic [31:0] bw [4];\n\
  logic [63:0] cnt [4];\n\
  rec_t rec;\n\
  uc_probe u (.clk(clk), .a0(bw[0]), .a1(bw[1]),\n\
    .st({st[0], st[1]}), .sum(cnt[0] + cnt[3]), .oob(bw[7]), .m(rec.f),\n\
    .a0(bw[2]));\n\
endmodule\n";

fn capture(source: &str, library_units: bool) -> Snapshot {
    let sources = [Source::compilation_unit("unbound-connections.sv", source)];
    let options = CompileOptions {
        library_units,
        ..Default::default()
    };
    slang::compile(&CompileRequest {
        sources: &sources,
        library_sources: &[],
        options: &options,
    })
    .expect("connection source yields a snapshot")
}

fn offset(needle: &str, skip: usize) -> u64 {
    (SOURCE
        .find(needle)
        .unwrap_or_else(|| panic!("{needle:?} in source"))
        + skip) as u64
}

fn token_at(snapshot: &Snapshot, start: u64) -> &LexicalToken {
    snapshot
        .lexical_tokens
        .iter()
        .find(|token| {
            !token.is_missing
                && !token.is_skipped
                && token.range.is_some_and(|range| range.start == start)
        })
        .unwrap_or_else(|| panic!("no lexical token at {start}"))
}

/// The declaration start of the semantic node a token binds to.
fn bound_start(snapshot: &Snapshot, token: &LexicalToken) -> Option<u64> {
    let id = token.semantic_id?;
    let node = snapshot.semantic_nodes.iter().find(|node| node.id == id)?;
    Some(node.range?.start)
}

/// Each unbound actual's names, with the declaration they reach.
const UNBOUND_NAMES: &[(&str, usize, &str, usize)] = &[
    ("bw[1]", 0, "bw [4]", 0),
    ("st[0], st[1]", 0, "st [2]", 0),
    ("st[0], st[1]", 7, "st [2]", 0),
    ("cnt[0] + cnt[3]", 0, "cnt [4]", 0),
    ("cnt[0] + cnt[3]", 9, "cnt [4]", 0),
    ("bw[7]", 0, "bw [4]", 0),
    ("rec.f)", 0, "rec;", 0),
    ("rec.f)", 4, "f; }", 0),
    ("bw[2]", 0, "bw [4]", 0),
];

#[test]
fn unbound_connection_actual_names_bind_in_both_capture_modes() {
    for library_units in [false, true] {
        let snapshot = capture(SOURCE, library_units);
        let mode = if library_units {
            "declaration-only"
        } else {
            "full"
        };
        for (needle, skip, declaration, declaration_skip) in UNBOUND_NAMES {
            let token = token_at(&snapshot, offset(needle, *skip));
            assert_eq!(
                bound_start(&snapshot, token),
                Some(offset(declaration, *declaration_skip)),
                "{mode} {needle:?}+{skip}"
            );
        }
        if library_units {
            continue;
        }
        // Slang's diagnostics only: the five missing ports and the repeat,
        // never the out-of-range select of an actual it did not bind.
        let mut names: Vec<&str> = snapshot
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.name.as_str())
            .filter(|name| !name.starts_with("Unused") && !name.starts_with("Unassigned"))
            .collect();
        names.sort_unstable();
        assert_eq!(
            names,
            [
                "DuplicatePortConnection",
                "PortDoesNotExist",
                "PortDoesNotExist",
                "PortDoesNotExist",
                "PortDoesNotExist",
                "PortDoesNotExist",
            ],
            "{:?}",
            snapshot.diagnostics
        );
        // The semantic graph keeps no expression of an unbound actual.
        for (needle, skip, _, _) in UNBOUND_NAMES {
            let start = offset(needle, *skip);
            assert!(
                !snapshot.semantic_nodes.iter().any(|node| {
                    node.kind == SemanticKind::Expression
                        && node.range.is_some_and(|range| range.start == start)
                }),
                "an expression node at {needle:?}+{skip}"
            );
        }
        // The bound actuals keep theirs.
        let bound = offset("bw[0]", 0);
        assert!(
            snapshot.semantic_nodes.iter().any(|node| {
                node.kind == SemanticKind::Expression
                    && node.range.is_some_and(|range| range.start == bound)
            }),
            "the a0 actual is an expression"
        );
    }
}
