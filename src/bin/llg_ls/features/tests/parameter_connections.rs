//! Parameter connections.

use super::*;

/// Real compile of a design with a named PARAMETER override:
/// `child u0 #(.W(4), .D(W)) (.clk(clk), .q(t_q));` inside module `top`,
/// which declares a DECOY same-name `localparam int W`.  The `.W`/`.D`
/// labels must resolve to the CHILD module's parameter declarations while
/// the RHS reference `W` inside `.D(W)` stays on the decoy localparam in
/// the instantiating scope — never crossing namespaces in either
/// direction.
#[test]
fn analyze_full_pipeline_named_param_overrides_resolve_to_child() {
    let _guards = analysis_guards();
    let dir = resolved_temp_dir(&format!("llg_llg_bin_param_{}", std::process::id()));
    let orig_cwd = std::env::current_dir().expect("current dir");
    let _restore = TempDirGuard {
        dir: dir.clone(),
        orig: orig_cwd,
    };
    std::env::set_current_dir(&dir).expect("chdir to temp dir");
    let child_sv = dir.join("child.sv");
    let top_sv = dir.join("top.sv");
    let child_text = concat!(
        "module child #(\n",
        "  parameter int W = 8,\n",
        "  parameter int D = 3\n",
        ") (\n",
        "  input logic clk,\n",
        "  output logic [7:0] q\n",
        ");\n",
        "  assign q = '0;\n",
        "endmodule\n",
    );
    let top_text = concat!(
        "module top;\n",
        "  localparam int W = 1;\n",
        "  logic clk;\n",
        "  logic [7:0] t_q;\n",
        "  child #(.W(4), .D(W)) u0 (.clk(clk), .q(t_q));\n",
        "endmodule\n",
    );
    std::fs::write(&child_sv, child_text).expect("write child.sv");
    std::fs::write(&top_sv, top_text).expect("write top.sv");
    let opts = CompileOpts {
        files: vec![
            child_sv.to_string_lossy().into_owned(),
            top_sv.to_string_lossy().into_owned(),
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
    let top_path = a
        .tokens
        .iter()
        .find(|ft| ft.path.ends_with("top.sv"))
        .expect("tokens for top.sv")
        .path
        .clone();
    // The override labels sit inside the `#(...)` clause, which in valid
    // SV PRECEDES the instance name.
    let (wl, wc) = pos_of(top_text, "#(.W", 0);
    let w_label = (wl, wc + 3);
    let (dl, dc) = pos_of(top_text, ", .D", 0);
    let d_label = (dl, dc + 3);
    assert_eq!(w_label, (4, 11), "sanity: .W label position");
    // `.W` label → CHILD module's parameter declaration in child.sv
    // (1-based (2,17) → 0-based (1,16)), NOT the decoy localparam.
    let (dcl, dcc) = pos_of(top_text, "localparam int W", 0);
    let decoy = (dcl, dcc + 15);
    assert_eq!(decoy, (1, 17), "sanity: decoy position");
    let loc = definition_at(&a, &top_path, w_label.0, w_label.1).expect(".W definition");
    assert_eq!(loc.uri, Url::from_file_path(&child_sv).unwrap());
    assert_eq!(loc.range.start, Position::new(1, 16), "loc: {loc:?}");
    assert_ne!(loc.range.start, Position::new(decoy.0, decoy.1));
    // `.D` label → child's D parameter (0-based (2,16)).
    let loc = definition_at(&a, &top_path, d_label.0, d_label.1).expect(".D definition");
    assert_eq!(loc.uri, Url::from_file_path(&child_sv).unwrap());
    assert_eq!(loc.range.start, Position::new(2, 16), "loc: {loc:?}");
    // The RHS `W` inside `.D(W)` resolves to the DECOY localparam in the
    // instantiating scope — the exact opposite direction of the label.
    let (rl, rc) = pos_of(top_text, "(W)", 0);
    let rhs = (rl, rc + 1);
    let loc = definition_at(&a, &top_path, rhs.0, rhs.1).expect(".D RHS definition");
    assert_eq!(loc.uri, Url::from_file_path(&top_sv).unwrap());
    assert_eq!(
        loc.range.start,
        Position::new(decoy.0, decoy.1),
        "loc: {loc:?}"
    );
    // Hover on the `.W` label shows the child parameter.
    let hover = hover_at(&a, &top_path, w_label.0, w_label.1).expect("hover on .W label");
    let value = match hover.contents {
        HoverContents::Markup(m) => m.value,
        _ => panic!("expected markup hover"),
    };
    assert!(value.contains("parameter"), "value: {value}");
}

/// Multi-line variant of the parameter override navigation: the labels
/// sit on continuation lines below `child u0 #(` and must still resolve
/// to the CHILD module's parameters, while the RHS stays parent-scope.
#[test]
fn analyze_full_pipeline_multiline_named_param_overrides_resolve_to_child() {
    let _guards = analysis_guards();
    let dir = resolved_temp_dir(&format!("llg_llg_bin_mlparam_{}", std::process::id()));
    let orig_cwd = std::env::current_dir().expect("current dir");
    let _restore = TempDirGuard {
        dir: dir.clone(),
        orig: orig_cwd,
    };
    std::env::set_current_dir(&dir).expect("chdir to temp dir");
    let child_sv = dir.join("child.sv");
    let top_sv = dir.join("top.sv");
    let child_text = concat!(
        "module child #(\n",
        "  parameter int W = 8,\n",
        "  parameter int D = 3\n",
        ") (\n",
        "  input logic clk,\n",
        "  output logic [7:0] q\n",
        ");\n",
        "  assign q = '0;\n",
        "endmodule\n",
    );
    let top_text = concat!(
        "module top;\n",
        "  localparam int W = 1;\n",
        "  logic clk;\n",
        "  logic [7:0] t_q;\n",
        "  child #(\n",
        "    .W(4),\n",
        "    .D(W)\n",
        "  ) u0 (\n",
        "    .clk(clk),\n",
        "    .q(t_q)\n",
        "  );\n",
        "endmodule\n",
    );
    std::fs::write(&child_sv, child_text).expect("write child.sv");
    std::fs::write(&top_sv, top_text).expect("write top.sv");
    let opts = CompileOpts {
        files: vec![
            child_sv.to_string_lossy().into_owned(),
            top_sv.to_string_lossy().into_owned(),
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
    let top_path = a
        .tokens
        .iter()
        .find(|ft| ft.path.ends_with("top.sv"))
        .expect("tokens for top.sv")
        .path
        .clone();
    let (wl, wc) = pos_of(top_text, ".W(", 0);
    let w_label = (wl, wc + 1);
    let (dl, dc) = pos_of(top_text, ".D(", 0);
    let d_label = (dl, dc + 1);
    assert_eq!(w_label, (5, 5), "sanity: continuation-line .W position");
    let (dcl, dcc) = pos_of(top_text, "localparam int W", 0);
    let decoy = (dcl, dcc + 15);
    // `.W` → child's W parameter in child.sv (0-based (1,16)).
    let loc = definition_at(&a, &top_path, w_label.0, w_label.1).expect(".W definition");
    assert_eq!(loc.uri, Url::from_file_path(&child_sv).unwrap());
    assert_eq!(loc.range.start, Position::new(1, 16), "loc: {loc:?}");
    // `.D` → child's D parameter (0-based (2,16)).
    let loc = definition_at(&a, &top_path, d_label.0, d_label.1).expect(".D definition");
    assert_eq!(loc.uri, Url::from_file_path(&child_sv).unwrap());
    assert_eq!(loc.range.start, Position::new(2, 16), "loc: {loc:?}");
    // RHS `W` stays on the decoy localparam in the instantiating scope.
    let (rl, rc) = pos_of(top_text, "(W)", 0);
    let rhs = (rl, rc + 1);
    let loc = definition_at(&a, &top_path, rhs.0, rhs.1).expect(".D RHS definition");
    assert_eq!(loc.uri, Url::from_file_path(&top_sv).unwrap());
    assert_eq!(
        loc.range.start,
        Position::new(decoy.0, decoy.1),
        "loc: {loc:?}"
    );
}

/// A label the instantiated module does not declare names nothing, whatever
/// the instantiating scope declares under its name.
#[test]
fn unresolved_connection_labels_have_no_definition() {
    let _guards = analysis_guards();
    const TEXT: &str = concat!(
        "module child(input logic a);\n",
        "endmodule\n",
        "module top;\n",
        "  localparam int W = 1;\n",
        "  logic clk;\n",
        "  child #(.W(W)) u0(.clk(clk));\n",
        "endmodule\n",
    );
    let a = analyze_sources(&[("/x/b.sv", TEXT)]);
    let site = |marked| definition_site(&a, hp("/x/b.sv"), TEXT, marked);
    assert_eq!(site(".|W(W)"), None);
    assert_eq!(site(".|clk(clk)"), None);
    let at = |marked| {
        let (line, col) = cursor(TEXT, marked);
        Some((hp("/x/b.sv").to_owned(), Position::new(line, col)))
    };
    assert_eq!(site(".W(|W)"), at("int |W = 1"));
    assert_eq!(site(".clk(|clk)"), at("logic |clk;"));
}
