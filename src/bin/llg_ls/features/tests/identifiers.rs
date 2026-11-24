//! Highlighting and go-to-definition read one table of identifiers.

use super::*;
use crate::features::IncludeContext;
use llg::core::tokens::{token_class, Bindings, Definition, TokenRule, TARGET_FILE};
use std::collections::BTreeMap;

macro_rules! fixture {
    ($file:literal) => {
        (
            concat!("/identifiers/", $file),
            include_str!(concat!(
                "../../../../../tests/fixtures/semantic_tokens/identifiers/",
                $file
            )),
        )
    };
}

const UNITS: [(&str, &str); 7] = [
    fixture!("names.sv"),
    fixture!("macros.sv"),
    fixture!("shared_a.sv"),
    fixture!("shared_b.sv"),
    fixture!("classes.sv"),
    fixture!("dup_a.sv"),
    fixture!("dup_b.sv"),
];
const HEADERS: [(&str, &str); 2] = [fixture!("inc/defs.svh"), fixture!("inc/shared.svh")];

fn defines() -> Vec<String> {
    vec!["CONFIG_W=3".to_owned()]
}

/// The analyzed fixture and the context of its isolated parses.
fn project() -> (Analysis, IncludeContext) {
    let includes: Vec<_> = HEADERS
        .iter()
        .map(|(name, text)| (hp(name).to_owned(), Arc::new((*text).to_owned())))
        .collect();
    let units = UNITS
        .iter()
        .map(|(name, text)| (hp(name).to_owned(), Arc::new((*text).to_owned())));
    let packages = crate::features::PackageContext::new(units, &includes);
    let context = IncludeContext {
        include_dirs: vec![hp("/identifiers/inc").to_owned()],
        includes,
        packages,
    };
    let mut sources: Vec<_> = UNITS
        .iter()
        .map(|(name, text)| compile::OwnedSource::compilation_unit(hp(name), *text))
        .collect();
    sources.extend(context.sources());
    let analysis = analyze(&CompileOpts {
        sources,
        include_dirs: context.include_dirs.clone(),
        defines: defines(),
        editor: true,
        ..Default::default()
    });
    (analysis, context)
}

/// The start of every token that `encode` sends, as 0-based positions.
fn highlighted(nodes: &[TokenInfo]) -> HashSet<(u32, u32)> {
    let (mut line, mut col) = (0, 0);
    crate::semantic_tokens::encode(nodes)
        .data
        .iter()
        .map(|token| {
            if token.delta_line > 0 {
                line += token.delta_line;
                col = 0;
            }
            col += token.delta_start;
            (line, col)
        })
        .collect()
}

/// Every token follows the rule of its kind. Returns the kinds and targets by
/// position.
fn check_rules(
    nodes: &[TokenInfo],
    bindings: &Bindings,
    what: &str,
) -> BTreeMap<(u32, u32), (i32, Vec<DeclTarget>)> {
    let highlighted = highlighted(nodes);
    let mut out = BTreeMap::new();
    for token in nodes {
        let position = (token.line - 1, token.col - 1);
        let sent = highlighted.contains(&position);
        let definition = bindings.definition(token);
        let at = format!("{what}: {token:?}: {definition:?}");
        match token_class(token.kind).rule() {
            TokenRule::Unresolved => assert!(!sent && definition.is_none(), "{at}"),
            TokenRule::Undeclared => assert!(sent && definition.is_none(), "{at}"),
            TokenRule::Declaration | TokenRule::Reference => {
                assert!(sent && definition.is_some(), "{at}")
            }
            TokenRule::Lexical => assert!(
                definition.is_none_or(|definition| matches!(
                    definition,
                    Definition::Targets([target]) if target.kind == TARGET_FILE
                )),
                "{at}"
            ),
        }
        let mut targets = match definition {
            Some(Definition::Targets(targets)) => targets.to_vec(),
            _ => Vec::new(),
        };
        for target in &mut targets {
            // How the name is written, which only references read.
            target.via_label = false;
            target.via_connection = false;
        }
        out.insert(position, (token.kind, targets));
    }
    out
}

#[test]
fn every_token_follows_the_rule_of_its_kind_in_the_project_and_open_buffer_tables() {
    let (analysis, context) = project();
    for (name, text) in UNITS {
        let file = hp(name);
        let tokens = &file_tokens(&analysis, file).expect("project tokens").nodes;
        let project = check_rules(tokens, &analysis.bindings, name);
        assert!(!project.is_empty(), "{name}");

        let isolated =
            open_document_identifiers(file, &defines(), text, &context).expect("isolated parse");
        let (nodes, bindings) = isolated.parts();
        let open = check_rules(nodes, bindings, &format!("open {name}"));
        for (position, project) in &project {
            assert_eq!(
                open.get(position),
                Some(project),
                "{name}:{position:?}: the two tables differ"
            );
        }
        assert_eq!(open.len(), project.len(), "{name}");
    }
    for (name, _) in HEADERS {
        let tokens = &file_tokens(&analysis, hp(name))
            .expect("header tokens")
            .nodes;
        assert!(!check_rules(tokens, &analysis.bindings, name).is_empty());
    }
}

/// A position that no token covers, or that names nothing, has no definition,
/// whatever its spelling.
#[test]
fn words_without_a_token_have_no_definition() {
    let (analysis, _) = project();
    for (name, text) in UNITS {
        let file = hp(name);
        let nodes = &file_tokens(&analysis, file).expect("project tokens").nodes;
        for (line, line_text) in text.lines().enumerate() {
            for col in 0..=line_text.len() {
                let position = (line as u32, col as u32);
                let covered = nodes.iter().any(|token| {
                    token_class(token.kind).rule() != TokenRule::Unresolved
                        && (token.line - 1, token.col - 1) <= position
                        && position <= (token.end_line - 1, token.end_col - 1)
                });
                if !covered {
                    let definitions = definitions_at(&analysis, file, position.0, position.1);
                    assert!(
                        definitions.is_empty(),
                        "{name}:{position:?}: {definitions:?}"
                    );
                }
            }
        }
    }
}

/// The macro hover and the definition of a macro name describe one
/// definition: the one in effect at the name.
#[test]
fn macro_hover_and_definition_read_one_macro_table() {
    let text = "\
`define W 4
`define ADD(a, b) ((a) + (b) + `W)
module m;
  int first = `W;
`define W 8
  int second = `ADD(1, 2);
`undef W
`ifdef W
`endif
  int third = `W;
endmodule
";
    let a = analyze_sources(&[("/x/m.sv", text)]);
    let file = hp("/x/m.sv");
    let site = |marked: &str| {
        definition_site(&a, file, text, marked).map(|(_, start)| (start.line, start.character))
    };
    let hover = |marked: &str| {
        let (line, col) = cursor(text, marked);
        hover_at(&a, file, line, col).map(|hover| hover_text(Some(hover)))
    };
    assert_eq!(site("first = `|W"), Some((0, 8)));
    let first = hover("first = `|W").expect("hover");
    assert!(first.contains("macro W = 4"), "{first}");
    assert_eq!(site("`|ADD(1, 2)"), Some((1, 8)));
    let add = hover("`|ADD(1, 2)").expect("hover");
    assert!(add.contains("((a) + (b) + `W)"), "{add}");
    // The body is expanded where the macro is used.
    assert_eq!(site("(b) + `|W)"), Some((4, 8)));
    assert_eq!(site("`undef |W"), Some((4, 8)));
    let undef = hover("`undef |W").expect("hover");
    assert!(undef.contains("macro W = 8"), "{undef}");
    assert_eq!(site("`ifdef |W"), None);
    assert_eq!(site("third = `|W"), None);
    assert!(
        references_at(&a, file, 4, 8).is_empty(),
        "a macro is no reference of the design"
    );
}

/// The path of an `` `include ``, and no other string, has a definition.
#[test]
fn include_path_has_a_definition_at_every_column() {
    let (analysis, _) = project();
    let (name, text) = UNITS[1];
    let file = hp(name);
    let columns = |operand: &str| {
        let (line, start) = pos_of(text, operand, 0);
        (start..=start + operand.len() as u32).map(move |col| (line, col))
    };
    for (line, col) in columns("\"defs.svh\"") {
        let location = definition_at(&analysis, file, line, col).expect("include target");
        assert_eq!(
            uri_file_path(&location.uri).to_string_lossy(),
            hp("/identifiers/inc/defs.svh")
        );
        assert_eq!(location.range, Range::default());
    }
    for (line, col) in columns("\"missing.svh\"") {
        assert!(definitions_at(&analysis, file, line, col).is_empty());
    }
}

/// Attribute names take the `decorator` type and lead nowhere, in both
/// tables, also when they spell a declared name; a clocking item, which the
/// frontend parses like an attribute, declares its signal.
#[test]
fn attribute_names_are_decorators_and_attribute_values_are_expressions() {
    let text = "\
module m (input logic clk, data);
  localparam int N = 2;
  (* data, depth = N, note = \"clk\" *) logic [N-1:0] q;
  (* full_case *) always_comb case (q)
    default: ;
  endcase
  clocking cb @(posedge clk);
    input data;
  endclocking
endmodule
";
    let file = hp("/x/attr.sv");
    let a = analyze_sources(&[("/x/attr.sv", text)]);
    let isolated =
        open_document_identifiers(file, &[], text, &Default::default()).expect("isolated parse");
    let project = &file_tokens(&a, file).expect("tokens").nodes;
    for (what, nodes, bindings) in [
        ("project", project.as_slice(), &a.bindings),
        ("open", isolated.parts().0, isolated.parts().1),
    ] {
        check_rules(nodes, bindings, what);
        let token = |marked: &str| {
            let (line, col) = cursor(text, marked);
            tokens::token_at(nodes, line, col).expect("token")
        };
        for marked in ["(* |data,", "|depth = N", "|note = ", "(* |full_case"] {
            let token = token(marked);
            assert_eq!(
                token.kind,
                tokens::TOKEN_SLANG_ATTRIBUTE,
                "{what}: {marked}"
            );
            assert_eq!(bindings.definition(token), None, "{what}: {marked}");
        }
        let value = token("depth = |N");
        assert!(
            matches!(
                bindings.definition(value),
                Some(Definition::Targets([target])) if (target.line0, target.col0) == (1, 17)
            ),
            "{what}"
        );
        assert_eq!(
            token_class(token("input |data;").kind).rule(),
            TokenRule::Declaration,
            "{what}"
        );
    }
}

/// A name that several syntax trees or macro expansions resolve differently
/// leads to every declaration, and is no reference of any of them.
#[test]
fn a_name_with_several_declarations_leads_to_each() {
    let text = "\
`define SET(value) target = value;
module a;
  int target;
  initial `SET(1)
endmodule
module b;
  int target;
  initial `SET(2)
endmodule
";
    let a = analyze_sources(&[("/x/several.sv", text)]);
    let file = hp("/x/several.sv");
    let (line, col) = cursor(text, "`define SET(value) |target");
    let mut lines: Vec<_> = definitions_at(&a, file, line, col)
        .iter()
        .map(|location| location.range.start.line)
        .collect();
    lines.sort_unstable();
    assert_eq!(lines, [2, 6]);
    let references = references_at_with_options(&a, file, 2, 6, false);
    assert!(references.is_empty(), "{references:?}");
}
