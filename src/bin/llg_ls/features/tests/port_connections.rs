//! Port connections.

use super::*;

/// Real compile of `tests/elaboration/top3.sv`: line 34 instantiates
/// `hier_ref u_hier (.clk(clk), .o(o));` inside module `tb`, which itself
/// declares ports named `clk`/`o`.  The labels must resolve to the CHILD
/// module's ports (hier_ref), not the enclosing module's same-named ones;
/// the inner expression refs must keep resolving to the enclosing scope.
#[test]
fn analyze_full_pipeline_named_ports_resolve_to_child() {
    let _guards = analysis_guards();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/elaboration/top3.sv");
    if !path.exists() {
        return;
    }
    // Path-mode compiles name files by their resolved native spelling; the
    // joined path keeps a literal `/` on Windows and would match nothing.
    let path = llg::ffi::platform::canonicalize(&path).expect("resolve fixture path");
    let path_str = path.to_string_lossy().into_owned();
    let opts = CompileOpts {
        files: vec![path_str.clone()],
        top: None,
        ..Default::default()
    };
    let a = analyze(&opts);
    assert!(
        !a.diagnostics.iter().any(|d| matches!(
            d.severity,
            Severity::Fatal | Severity::Syntax | Severity::Error
        )),
        "unexpected diagnostics: {:?}",
        a.diagnostics
    );
    // `.clk` label at 0-based (33, 22) → hier_ref's clk port decl
    // (20, 16), NOT tb's clk port decl (29, 16).
    let loc = definition_at(&a, &path_str, 33, 22).expect("definition of .clk label");
    assert_eq!(loc.range.start, Position::new(20, 16), "loc: {loc:?}");
    // `.o` label at (33, 33) → hier_ref's o port decl (21, 23).
    let loc = definition_at(&a, &path_str, 33, 33).expect("definition of .o label");
    assert_eq!(loc.range.start, Position::new(21, 23), "loc: {loc:?}");
    // The inner `clk` ref (the connection ACTUAL) resolves to the
    // ACTUAL signal's own declaration in the instantiating scope —
    // tb's clk port decl (29, 16), NOT the child module's same-named
    // port, even though both modules declare `clk`.
    let loc = definition_at(&a, &path_str, 33, 26).expect("definition of inner clk ref");
    assert_eq!(loc.range.start, Position::new(29, 16), "loc: {loc:?}");
    assert_ne!(
        loc.range.start,
        Position::new(20, 16),
        "the actual must not jump into the child module"
    );
    // The label and the actual of the SAME connection resolve to
    // DIFFERENT declarations (child port vs parent-scope decl).
    let label_loc = definition_at(&a, &path_str, 33, 22).expect("label location");
    assert_ne!(label_loc.range.start, loc.range.start);
    // Hover on the `.o` label shows the child port.
    let hover = hover_at(&a, &path_str, 33, 33).expect("hover on .o label");
    let value = match hover.contents {
        HoverContents::Markup(m) => m.value,
        _ => panic!("expected markup hover"),
    };
    assert!(value.contains("output"), "value: {value}");
    // References of hier_ref's `o` port decl include the `.o` label site.
    let refs = references_at(&a, &path_str, 21, 23);
    assert!(
        refs.iter().any(|l| l.range.start == Position::new(33, 33)),
        "missing .o label site: {refs:?}"
    );
}

/// Full compile of a two-file design whose instantiation is spread over
/// several lines: `m u0(\n  .clk(clk),\n  .o(o)\n);` inside module `top`,
/// which itself declares signals named `clk`/`o`.  The continuation-line
/// labels must resolve to the CHILD module's ports in a.sv, not the
/// enclosing module's same-named signals.
#[test]
fn analyze_full_pipeline_multiline_named_ports_resolve_to_child() {
    let _guards = analysis_guards();
    let dir = resolved_temp_dir(&format!("llg_llg_bin_mlport_{}", std::process::id()));
    let orig_cwd = std::env::current_dir().expect("current dir");
    let _restore = TempDirGuard {
        dir: dir.clone(),
        orig: orig_cwd,
    };
    std::env::set_current_dir(&dir).expect("chdir to temp dir");
    let a_sv = dir.join("a.sv");
    let b_sv = dir.join("b.sv");
    std::fs::write(
        &a_sv,
        "module m(\n  input logic clk,\n  output logic [3:0] o\n);\n  assign o = clk;\nendmodule\n",
    )
    .expect("write a.sv");
    std::fs::write(
        &b_sv,
        "module top;\n  logic clk;\n  logic [3:0] o;\n  m u0(\n    .clk(clk),\n    .o(o)\n  );\nendmodule\n",
    )
    .expect("write b.sv");
    let opts = CompileOpts {
        files: vec![
            a_sv.to_string_lossy().into_owned(),
            b_sv.to_string_lossy().into_owned(),
        ],
        top: None,
        ..Default::default()
    };
    let a = analyze(&opts);
    assert!(
        !a.diagnostics.iter().any(|d| matches!(
            d.severity,
            Severity::Fatal | Severity::Syntax | Severity::Error
        )),
        "unexpected diagnostics: {:?}",
        a.diagnostics
    );
    let b_path = a
        .tokens
        .iter()
        .find(|ft| ft.path.ends_with("b.sv"))
        .expect("tokens for b.sv")
        .path
        .clone();
    let ft = file_tokens(&a, &b_path).expect("b.sv tokens");
    // The port-connection labels are the only tokens in b.sv carrying the
    // classifier's connection-label synthetic type; find them by name and
    // continuation line rather than hard-coding positions.
    let label = |name: &str| -> TokenInfo {
        ft.nodes
            .iter()
            .find(|n| {
                n.kind == tokens::TOKEN_SLANG_PORT_CONNECTION_LABEL
                    && n.name.as_deref() == Some(name)
                    && n.line > 2
            })
            .expect("label token")
            .clone()
    };
    let clk_label = label("clk");
    let o_label = label("o");
    assert!(
        o_label.line > clk_label.line,
        "labels must be on consecutive continuation lines: {clk_label:?} {o_label:?}"
    );
    let clk_pos = (clk_label.line - 1, clk_label.col - 1);
    let o_pos = (o_label.line - 1, o_label.col - 1);
    // `.clk` → m's clk port decl in a.sv (1-based (2,15) → 0-based
    // (1,14)), NOT top's `logic clk` (0-based (1,8)).
    let loc = definition_at(&a, &b_path, clk_pos.0, clk_pos.1).expect("definition of .clk label");
    assert_eq!(loc.uri, Url::from_file_path(&a_sv).unwrap());
    assert_eq!(loc.range.start, Position::new(1, 14), "loc: {loc:?}");
    // `.o` → m's o port decl in a.sv (1-based (3,22) → 0-based (2,21)).
    let loc = definition_at(&a, &b_path, o_pos.0, o_pos.1).expect("definition of .o label");
    assert_eq!(loc.uri, Url::from_file_path(&a_sv).unwrap());
    assert_eq!(loc.range.start, Position::new(2, 21), "loc: {loc:?}");
    // Hover on the `.o` label shows the child port.
    let hover = hover_at(&a, &b_path, o_pos.0, o_pos.1).expect("hover on .o label");
    let value = match hover.contents {
        HoverContents::Markup(m) => m.value,
        _ => panic!("expected markup hover"),
    };
    assert!(value.contains("output"), "value: {value}");
    assert!(value.contains("o"), "value: {value}");
}

/// 0-based `(line, col)` of the `occurrence`-th (0-based) `needle` in
/// `text` — the same convention as the stdio suite's `position_at`.
pub(super) fn pos_of(text: &str, needle: &str, occurrence: usize) -> (u32, u32) {
    let mut start = 0;
    for _ in 0..=occurrence {
        let found = text[start..]
            .find(needle)
            .unwrap_or_else(|| panic!("needle {needle:?} not found"));
        start += found;
    }
    let line = text[..start].matches('\n').count() as u32;
    let line_start = text[..start].rfind('\n').map_or(0, |i| i + 1);
    (line, (start - line_start) as u32)
}

/// A name in a connection is looked up where the instance is, not in another
/// module of the file or in the instantiated one; an undeclared one has no
/// definition, and its label keeps the port.
#[test]
fn actuals_name_the_object_of_their_own_module() {
    let _guards = analysis_guards();
    const TEXT: &str = concat!(
        "`default_nettype none\n",
        "module child(input wire clk);\n",
        "endmodule\n",
        "module other;\n",
        "  logic wa;\n",
        "endmodule\n",
        "module parent;\n",
        "  logic wa, wb;\n",
        "  child u0(.clk(wa));\n",
        "  child u1(.clk(wb));\n",
        "  child u2(.clk(ghost));\n",
        "endmodule\n",
    );
    let a = analyze_sources(&[("/x/c.sv", TEXT)]);
    let site = |marked| definition_site(&a, hp("/x/c.sv"), TEXT, marked);
    let at = |marked| {
        let (line, col) = cursor(TEXT, marked);
        Some((hp("/x/c.sv").to_owned(), Position::new(line, col)))
    };
    assert_eq!(site(".clk(|wa)"), at("logic |wa, wb"));
    assert_eq!(site(".clk(|wb)"), at("wa, |wb;"));
    assert_eq!(site(".clk(|ghost)"), None);
    let (line, col) = cursor(TEXT, ".clk(|ghost)");
    let ghost = a.tokens[0]
        .nodes
        .iter()
        .find(|token| (token.line, token.col) == (line + 1, col + 1))
        .expect("token of the undeclared name");
    let rule = tokens::token_class(ghost.kind).rule();
    assert_eq!(rule, tokens::TokenRule::Unresolved);
    for label in [".|clk(wa)", ".|clk(wb)", ".|clk(ghost)"] {
        assert_eq!(site(label), at("input wire |clk)"), "{label}");
    }
}

/// Connection labels lead to the port or parameter in every form that declares
/// one, also when the label is the first text of its line.
#[test]
fn labels_name_ports_and_parameters_however_they_are_declared() {
    let _guards = analysis_guards();
    const TEXT: &str = concat!(
        "`define PORT(name) input logic name\n",
        "module listed(a, y);\n",
        "  parameter W = 8;\n",
        "  parameter type T = logic;\n",
        "  input [W-1:0] a;\n",
        "  output T y;\n",
        "endmodule\n",
        "module built(`PORT(p), `PORT(q));\n",
        "endmodule\n",
        "interface bus;\n",
        "endinterface\n",
        "module with_bus(bus b);\n",
        "endmodule\n",
        "module top;\n",
        "  logic [3:0] x;\n",
        "  bit z;\n",
        "  bus ifc();\n",
        "  listed #(.W(4), .T(bit)) u0(.\n",
        "a(x), .y(z));\n",
        "  built u1(.p(z), .q(z));\n",
        "  with_bus u2(.b(ifc));\n",
        "endmodule\n",
    );
    let a = analyze_sources(&[("/x/c.sv", TEXT)]);
    assert!(a.is_valid(), "{:?}", a.diagnostics);
    for (label, declaration) in [
        ("|a(x)", "[W-1:0] |a;"),
        (".|y(z)", "T |y;"),
        (".|W(4)", "parameter |W ="),
        (".|T(bit)", "type |T ="),
        (".|p(z)", "`PORT(|p)"),
        (".|q(z)", "`PORT(|q)"),
        (".|b(ifc)", "bus |b)"),
    ] {
        let (line, col) = cursor(TEXT, declaration);
        assert_eq!(
            definition_site(&a, hp("/x/c.sv"), TEXT, label),
            Some((hp("/x/c.sv").to_owned(), Position::new(line, col))),
            "{label}"
        );
    }
}
