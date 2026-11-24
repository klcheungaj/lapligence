//! Cross file.

use super::*;

/// A module with ports, its body, and a package.
pub(super) const A_SV: &str = concat!(
    "module m(input logic clk, output logic [3:0] o);\n",
    "  assign o = clk;\n",
    "endmodule\n",
    "package p; endpackage\n",
);

/// An instance of `m` whose connections span lines, below objects of `top`
/// with the names of `m`'s ports.
pub(super) const B_SV: &str = concat!(
    "module top;\n",
    "  logic clk;\n",
    "  logic [3:0] o;\n",
    "  m u0(\n",
    "    .clk(clk),\n",
    "    .o(o)\n",
    "  );\n",
    "endmodule\n",
);

/// `/x/a.sv` ([`A_SV`]) and `/x/b.sv` ([`B_SV`]). The caller holds
/// [`analysis_guards`].
pub(super) fn cross_file_analysis() -> Analysis {
    let a = analyze_sources(&[("/x/a.sv", A_SV), ("/x/b.sv", B_SV)]);
    assert!(a.is_valid(), "{:?}", a.diagnostics);
    a
}

/// The declaration at `marked` in [`A_SV`].
fn in_a(marked: &str) -> Option<(String, Position)> {
    let (line, col) = cursor(A_SV, marked);
    Some((hp("/x/a.sv").to_owned(), Position::new(line, col)))
}

/// The declaration at `marked` in [`B_SV`].
fn in_b(marked: &str) -> Option<(String, Position)> {
    let (line, col) = cursor(B_SV, marked);
    Some((hp("/x/b.sv").to_owned(), Position::new(line, col)))
}

/// Where the name at `marked` in [`B_SV`] is defined.
fn from_b(a: &Analysis, marked: &str) -> Option<(String, Position)> {
    definition_site(a, hp("/x/b.sv"), B_SV, marked)
}

#[test]
fn definition_on_a_module_type_and_an_instance_name_is_the_module() {
    let _guards = analysis_guards();
    let a = cross_file_analysis();
    assert_eq!(from_b(&a, "  |m u0("), in_a("module |m("));
    assert_eq!(from_b(&a, "  m |u0("), in_a("module |m("));
    assert_eq!(
        definition_site(&a, hp("/x/a.sv"), A_SV, "module |m("),
        in_a("module |m(")
    );
}

#[test]
fn definition_on_a_module_type_ignores_a_same_named_instance() {
    let _guards = analysis_guards();
    const LEAF: &str = "module Bar; endmodule\n";
    const TOP: &str = "module top;\n  Bar Bar();\n  Bar u_bar();\nendmodule\n";
    let a = analyze_sources(&[("/x/leaf.sv", LEAF), ("/x/top.sv", TOP)]);
    let module = Some((hp("/x/leaf.sv").to_owned(), Position::new(0, 7)));
    for marked in [
        "  |Bar Bar()",
        "  Bar |Bar()",
        "  |Bar u_bar()",
        "Bar |u_bar()",
    ] {
        assert_eq!(
            definition_site(&a, hp("/x/top.sv"), TOP, marked),
            module,
            "{marked}"
        );
    }
}

#[test]
fn connection_labels_name_the_child_port_and_actuals_the_parent_object() {
    let _guards = analysis_guards();
    let a = cross_file_analysis();
    assert_eq!(from_b(&a, ".|clk(clk)"), in_a("logic |clk,"));
    assert_eq!(from_b(&a, ".clk(|clk)"), in_b("logic |clk;"));
    assert_eq!(from_b(&a, ".|o(o)"), in_a("[3:0] |o)"));
    assert_eq!(from_b(&a, ".o(|o)"), in_b("[3:0] |o;"));
}

#[test]
fn hover_on_a_connection_label_describes_the_child_port() {
    let _guards = analysis_guards();
    let a = cross_file_analysis();
    let (line, col) = cursor(B_SV, ".|o(o)");
    let value = hover_text(hover_at(&a, hp("/x/b.sv"), line, col));
    assert!(value.contains("output") && value.contains('o'), "{value}");
}

#[test]
fn a_reference_in_the_body_names_the_port() {
    let _guards = analysis_guards();
    let a = cross_file_analysis();
    for (marked, declaration) in [("o = |clk;", "logic |clk,"), ("assign |o =", "[3:0] |o)")] {
        assert_eq!(
            definition_site(&a, hp("/x/a.sv"), A_SV, marked),
            in_a(declaration)
        );
    }
}

#[test]
fn references_span_files() {
    let _guards = analysis_guards();
    let a = cross_file_analysis();
    let references = |marked: &str| -> Vec<(String, Position)> {
        let (line, col) = cursor(A_SV, marked);
        references_at(&a, hp("/x/a.sv"), line, col)
            .into_iter()
            .map(|location| {
                (
                    uri_file_path(&location.uri).to_string_lossy().into_owned(),
                    location.range.start,
                )
            })
            .collect()
    };
    assert_eq!(
        references("module |m("),
        [in_a("module |m("), in_b("  |m u0(")].map(Option::unwrap)
    );
    assert_eq!(
        references("[3:0] |o)"),
        [in_a("[3:0] |o)"), in_a("assign |o ="), in_b(".|o(o)")].map(Option::unwrap)
    );
}

#[test]
fn the_index_holds_declarations_at_their_positions() {
    let _guards = analysis_guards();
    let a = cross_file_analysis();
    let (line, col) = cursor(A_SV, "logic |clk,");
    let port = a.index.decl_at(hp("/x/a.sv"), line, col).expect("port");
    assert_eq!((port.name.as_str(), port.kind), ("clk", SymKind::Port));
    assert_eq!(port.scope.as_deref(), Some("m"));
    let (line, col) = cursor(B_SV, "m |u0(");
    let instance = a.index.decl_at(hp("/x/b.sv"), line, col).expect("instance");
    assert_eq!(
        (instance.name.as_str(), instance.kind),
        ("u0", SymKind::Instance)
    );
    assert_eq!(instance.scope.as_deref(), Some("top"));
    let (line, col) = cursor(B_SV, "  |m u0(");
    assert!(a.index.decl_at(hp("/x/b.sv"), line, col).is_none());
}

#[test]
fn workspace_symbols_filters_by_query() {
    let _guards = analysis_guards();
    let a = cross_file_analysis();
    let syms = workspace_symbols(&a, "m");
    assert_eq!(syms.len(), 1, "syms: {syms:?}");
    assert_eq!(syms[0].name, "m");
    assert_eq!(syms[0].kind, SymbolKind::MODULE);
    assert_eq!(
        syms[0].location.uri,
        Url::from_file_path(hp("/x/a.sv")).unwrap()
    );
    assert!(workspace_symbols(&a, "TOP")
        .iter()
        .any(|symbol| symbol.name == "top" && symbol.kind == SymbolKind::MODULE));
}

#[test]
fn symbol_index_merge_unions_and_dedupes_shared_occurrences() {
    let _guards = analysis_guards();
    let a = cross_file_analysis();
    // Merging an index with itself must dedupe every declaration and
    // reference by position while keeping the lookup maps functional.
    let merged = SymbolIndex::merge([&a.index, &a.index]);
    assert_eq!(merged.decls.len(), a.index.decls.len());
    assert_eq!(merged.refs.len(), a.index.refs.len());
    let (line, col) = cursor(A_SV, "logic |clk,");
    let port = merged.decl_at(hp("/x/a.sv"), line, col).expect("port");
    assert_eq!((port.name.as_str(), port.kind), ("clk", SymKind::Port));
}
