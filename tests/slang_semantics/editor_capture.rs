//! Editor capture omits the facts only simulation reads and nothing else.

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

fn token_kinds(files: &[tokens::FileTokens]) -> Vec<(u32, u32, i32, Option<&str>)> {
    files
        .iter()
        .flat_map(|file| &file.nodes)
        .map(|token| (token.line, token.col, token.kind, token.name.as_deref()))
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

    // Tokens and every reference binding are the same.
    let sources = [("editor.sv", SOURCE)];
    let full = tokens::project_slang(&full, &sources);
    let editor = tokens::project_slang(&editor, &sources);
    assert_eq!(token_kinds(&editor.0), token_kinds(&full.0));
    assert!(!editor.1.is_empty());
    assert_eq!(editor.1, full.1);
    assert_eq!(editor.2, full.2);
    assert_eq!(editor.4, full.4);
}
