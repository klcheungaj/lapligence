//! Capture contract of the names in sequence, property and let declarations,
//! sequence and property instances and invalid event controls (ABI v60,
//! docs/semantic_tokens.md ST-71, ST-72, ST-74): lexical bindings only, the
//! semantic graph the simulator imports keeps Slang's instance expansion.

use llg::ffi::slang::{
    self, CompileOptions, CompileRequest, LexicalKind, LexicalRole, LexicalToken, SemanticKind,
    Snapshot, Source,
};

const SOURCE: &str = "module an_cap(input logic clk, input logic a, input logic b);\n\
  sequence s_cap(x);\n\
    @(posedge clk) x ##1 b;\n\
  endsequence\n\
  property p_unused(z);\n\
    @(posedge clk) z |-> a;\n\
  endproperty\n\
  c_cap: cover property (s_cap(a));\n\
endmodule\n";

/// The same names with one unresolved name in each event control.
const INVALID_SOURCE: &str = "module an_bad(input logic clk, input logic a);\n\
  logic q;\n\
  always @(a or ghost_one) q = a;\n\
  initial @(posedge clk iff ghost_two) q = a;\n\
endmodule\n";

fn capture(name: &str, source: &str, library_units: bool) -> Snapshot {
    let sources = [Source::compilation_unit(name, source)];
    let options = CompileOptions {
        library_units,
        ..Default::default()
    };
    slang::compile(&CompileRequest {
        sources: &sources,
        library_sources: &[],
        options: &options,
    })
    .expect("assertion source yields a snapshot")
}

fn offset(source: &str, needle: &str, skip: usize) -> u64 {
    (source
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

#[test]
fn assertion_declaration_names_bind_in_the_declaration_scope_in_both_capture_modes() {
    // Full capture (simulation, project analysis) and declaration-only
    // capture (library-unit recovery) bind the same tokens.
    for library_units in [false, true] {
        let snapshot = capture("assertion-names.sv", SOURCE, library_units);
        assert!(
            !snapshot.has_errors(),
            "library_units={library_units}: {:?}",
            snapshot.diagnostics
        );
        let mode = if library_units {
            "declaration-only"
        } else {
            "full"
        };
        // ST-71: declaration and instance names are functions.
        for (needle, skip, role) in [
            ("s_cap(x)", 0, LexicalRole::Declaration),
            ("p_unused(z)", 0, LexicalRole::Declaration),
            ("s_cap(a)", 0, LexicalRole::Reference),
        ] {
            let token = token_at(&snapshot, offset(SOURCE, needle, skip));
            assert_eq!(
                (token.kind, token.role),
                (LexicalKind::Function, role),
                "{mode} {needle:?}"
            );
        }
        let instance = token_at(&snapshot, offset(SOURCE, "s_cap(a)", 0));
        assert_eq!(
            bound_start(&snapshot, instance),
            Some(offset(SOURCE, "s_cap(x)", 0)),
            "{mode}: the instance binds to the sequence"
        );
        // ST-72: a formal reference binds to the formal, also in a property
        // no assertion instantiates; other body names keep their targets.
        for (needle, skip, declaration, declaration_skip) in [
            ("x ##1 b", 0, "s_cap(x)", 6),
            ("x ##1 b", 6, "input logic b", 12),
            ("z |-> a", 0, "p_unused(z)", 9),
            ("z |-> a", 6, "input logic a", 12),
            ("(posedge clk) x", 9, "input logic clk", 12),
        ] {
            let token = token_at(&snapshot, offset(SOURCE, needle, skip));
            assert_eq!(
                token.role,
                LexicalRole::Reference,
                "{mode} {needle:?}+{skip}"
            );
            assert_eq!(
                bound_start(&snapshot, token),
                Some(offset(SOURCE, declaration, declaration_skip)),
                "{mode} {needle:?}+{skip}"
            );
        }
        if !library_units {
            // The simulator's instance expansion is unchanged: the expression
            // Slang built for the formal reference still names the actual.
            let formal = offset(SOURCE, "x ##1 b", 0);
            let expression = snapshot
                .semantic_nodes
                .iter()
                .find(|node| {
                    node.kind == SemanticKind::Expression
                        && node.target_id().is_some()
                        && node.range.is_some_and(|range| range.start == formal)
                })
                .expect("expansion of the formal reference");
            let target = snapshot
                .semantic_nodes
                .iter()
                .find(|node| Some(node.id) == expression.target_id())
                .expect("expansion target");
            assert_eq!(
                target.range.map(|range| range.start),
                Some(offset(SOURCE, "input logic a", 12)),
                "the expansion of x names the actual a"
            );
        }
    }
}

#[test]
fn resolved_names_of_an_invalid_event_control_stay_bound_in_both_capture_modes() {
    for library_units in [false, true] {
        let snapshot = capture("assertion-invalid.sv", INVALID_SOURCE, library_units);
        assert!(snapshot.has_errors(), "the ghost names are errors");
        let mode = if library_units {
            "declaration-only"
        } else {
            "full"
        };
        for (needle, skip, declaration, declaration_skip) in [
            ("@(a or ghost_one)", 2, "input logic a", 12),
            ("(posedge clk iff", 9, "input logic clk", 12),
        ] {
            let token = token_at(&snapshot, offset(INVALID_SOURCE, needle, skip));
            assert_eq!(
                bound_start(&snapshot, token),
                Some(offset(INVALID_SOURCE, declaration, declaration_skip)),
                "{mode} {needle:?}+{skip}"
            );
        }
        for ghost in ["ghost_one", "ghost_two"] {
            let token = token_at(&snapshot, offset(INVALID_SOURCE, ghost, 0));
            assert_eq!(token.semantic_id, None, "{mode} {ghost}");
        }
    }
}
