//! Connection tokens.

use super::*;

const CHILD: &str = concat!(
    "module m #(parameter int W = 4, parameter type T = logic) (input logic clk);\n",
    "endmodule\n",
);
const TOP: &str = concat!(
    "module top;\n",
    "  localparam int w = 2;\n",
    "  logic c;\n",
    "  m #(.W(w), .T(byte)) u0 (.clk(c));\n",
    "endmodule\n",
);

/// Labels of connections and parameter overrides are references to the child
/// module's declarations and the only `connectionLabel` tokens; what they
/// connect is a reference to the parent's declaration.
#[test]
fn connection_label_tokens_index_as_references_and_highlight_as_labels() {
    let _guards = analysis_guards();
    let a = analyze_sources(&[("/x/a.sv", CHILD), ("/x/b.sv", TOP)]);
    assert!(a.is_valid(), "{:?}", a.diagnostics);
    let data = semantic_tokens_for(&a, hp("/x/b.sv")).data;
    let cases = [
        (
            ".|clk(c)",
            "/x/a.sv",
            CHILD,
            "logic |clk)",
            "function/connectionLabel",
        ),
        (
            ".|W(w)",
            "/x/a.sv",
            CHILD,
            "int |W =",
            "function/readonly+connectionLabel",
        ),
        (
            ".|T(byte)",
            "/x/a.sv",
            CHILD,
            "type |T =",
            "function/readonly+connectionLabel",
        ),
        (".clk(|c)", "/x/b.sv", TOP, "logic |c;", "variable"),
        (".W(|w)", "/x/b.sv", TOP, "int |w =", "variable/readonly"),
    ];
    for (marked, file, text, declaration, token) in cases {
        let (line, col) = cursor(text, declaration);
        assert_eq!(
            definition_site(&a, hp("/x/b.sv"), TOP, marked),
            Some((hp(file).to_owned(), Position::new(line, col))),
            "{marked}"
        );
        let (line, col) = cursor(TOP, marked);
        assert_eq!(token_symbol(&data, line, col), token, "{marked}");
        assert!(
            a.index.decl_at(hp("/x/b.sv"), line, col).is_none(),
            "{marked}"
        );
    }
}

/// The token at the position as `type/modifier+modifier`.
fn token_symbol(data: &[tower_lsp::lsp_types::SemanticToken], line: u32, col: u32) -> String {
    let legend = crate::semantic_tokens::legend();
    let (mut token_line, mut token_col) = (0, 0);
    for token in data {
        token_line += token.delta_line;
        token_col = if token.delta_line == 0 {
            token_col + token.delta_start
        } else {
            token.delta_start
        };
        if (token_line, token_col) != (line, col) {
            continue;
        }
        let base = legend.token_types[token.token_type as usize].as_str();
        let modifiers: Vec<&str> = legend
            .token_modifiers
            .iter()
            .enumerate()
            .filter(|(bit, _)| token.token_modifiers_bitset & (1u32 << bit) != 0)
            .map(|(_, name)| name.as_str())
            .collect();
        return if modifiers.is_empty() {
            base.to_owned()
        } else {
            format!("{base}/{}", modifiers.join("+"))
        };
    }
    panic!("no semantic token at {line}:{col}");
}
