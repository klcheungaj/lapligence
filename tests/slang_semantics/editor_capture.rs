//! Editor capture omits the facts only simulation reads, and adds the names
//! only tokens and navigation need.

use super::*;
use llg::core::db::Db;
use llg::core::tokens;

const SOURCE: &str = "\
package p;
  typedef enum {IDLE, BUSY} state_t;
  typedef class derived;
  class base;
    int count;
    function int get(); return count; endfunction
  endclass
  class derived extends base;
    derived next;
    function int twice(); return count + get(); endfunction
  endclass
endpackage
(* keep *) module top;
  import p::*;
  (* async_reg = \"true\" *) state_t state;
  derived d;
  initial begin
    d = new;
    state = BUSY;
    d.count = d.twice();
  end
endmodule
";

fn capture(editor: bool) -> slang::Snapshot {
    let sources = [Source::compilation_unit("editor.sv", SOURCE)];
    let options = CompileOptions {
        editor,
        ..CompileOptions::default()
    };
    let snapshot = slang::compile(&CompileRequest {
        sources: &sources,
        library_sources: &[],
        options: &options,
    })
    .expect("compile");
    assert!(!snapshot.has_errors(), "{:?}", snapshot.diagnostics);
    snapshot
}

fn has_role(snapshot: &slang::Snapshot, role: SemanticEdgeRole) -> bool {
    snapshot.semantic_edges.iter().any(|edge| edge.role == role)
}

fn inherited_aliases(snapshot: &slang::Snapshot) -> usize {
    snapshot
        .semantic_nodes
        .iter()
        .filter(|node| {
            node.detail == "TransparentMember" && ["count", "get"].contains(&node.name.as_str())
        })
        .count()
}

fn token_fields(token: &tokens::TokenInfo) -> (u32, u32, i32, Option<&str>) {
    (token.line, token.col, token.kind, token.name.as_deref())
}

/// Every token with its definition.
fn defined_tokens(projection: &tokens::Projection) -> Vec<(&tokens::TokenInfo, String)> {
    projection
        .tokens
        .iter()
        .flat_map(|file| &file.nodes)
        .map(|token| {
            let definition = projection.bindings.definition(token);
            (token, format!("{definition:?}"))
        })
        .collect()
}

#[test]
fn editor_capture_omits_only_simulation_facts() {
    let full = capture(false);
    let editor = capture(true);

    assert!(!full.attributes.is_empty());
    assert!(has_role(&full, SemanticEdgeRole::Imported));
    assert!(has_role(&full, SemanticEdgeRole::ForwardDeclaration));
    assert_eq!(inherited_aliases(&full), 2);

    assert!(editor.attributes.is_empty());
    assert!(!has_role(&editor, SemanticEdgeRole::Imported));
    assert!(!has_role(&editor, SemanticEdgeRole::ForwardDeclaration));
    assert_eq!(inherited_aliases(&editor), 0);

    // Every other node is captured, the aliases of enum values included.
    let aliases = full
        .semantic_nodes
        .iter()
        .filter(|node| {
            node.detail == "TransparentMember" && !["IDLE", "BUSY"].contains(&node.name.as_str())
        })
        .count();
    assert_eq!(
        editor.semantic_nodes.len() + aliases,
        full.semantic_nodes.len()
    );
    Db::from_slang(&full).expect("full database");
    Db::from_slang(&editor).expect("editor database");

    // Every token with a definition is the same, and the base class name of a
    // class header gains one.
    let full = tokens::project_slang(&full, None, &[]);
    let editor = tokens::project_slang(&editor, None, &[]);
    let (full_tokens, editor_tokens) = (defined_tokens(&full), defined_tokens(&editor));
    assert_eq!(full_tokens.len(), editor_tokens.len());
    let added: Vec<_> = full_tokens
        .iter()
        .zip(&editor_tokens)
        .filter(|(full, editor)| {
            (token_fields(full.0), &full.1) != (token_fields(editor.0), &editor.1)
        })
        .collect();
    let [(before, after)] = added.as_slice() else {
        panic!("{added:?}");
    };
    assert_eq!(before.0.kind, tokens::TOKEN_SLANG_IDENTIFIER);
    assert_eq!(after.0.kind, tokens::TOKEN_SLANG_CLASS);
    assert_eq!(after.0.name.as_deref(), Some("base"));
    assert_eq!((after.0.line, after.0.col), (8, 25));
    assert!(after.1.contains("line0: 3"), "{after:?}");
    assert!(!editor.bindings.references.is_empty());
    assert_eq!(
        editor.bindings.references.len(),
        full.bindings.references.len() + 1
    );
    assert_eq!(editor.bindings.navigation, full.bindings.navigation);
    assert_eq!(editor.details, full.details);
}
