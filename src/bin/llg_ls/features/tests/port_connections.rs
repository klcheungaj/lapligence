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
    let dir = std::env::temp_dir().join(format!("llg_llg_bin_mlport_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
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
    // Both continuation-line labels are registered as port labels.
    assert!(
        a.index
            .port_labels
            .contains_key(&(b_path.clone(), clk_pos.0, clk_pos.1)),
        "port_labels: {:?}",
        a.index.port_labels
    );
    assert!(
        a.index
            .port_labels
            .contains_key(&(b_path.clone(), o_pos.0, o_pos.1)),
        "port_labels: {:?}",
        a.index.port_labels
    );
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
