//! Definitions.

use super::*;

/// One definition location extracted from a response the server must serve as
/// a SINGLE `Location` object (never an array, never null).
pub(super) fn single_location(response: &Value, what: &str) -> (String, Value) {
    let object = response
        .as_object()
        .unwrap_or_else(|| panic!("{what} must be a single Location object: {response}"));
    let uri = object
        .get("uri")
        .and_then(Value::as_str)
        .expect("definition location URI")
        .to_owned();
    let start = object
        .get("range")
        .and_then(|range| range.get("start"))
        .cloned()
        .expect("definition location range start");
    (uri, start)
}

/// Goto-definition is binding-precise: two modules in distinct files each
/// declare an input port named `clk`, and tb connects distinct parent signals
/// to each instance. A request at every use returns exactly ONE
/// location equal to that module's declaration file+line, and a request on a
/// declaration resolves to itself.
#[test]
fn lsp_stdio_goto_definition_is_binding_precise() {
    let fixture = FixtureTree::new();
    let root_a = fixture.root("root-a");
    let nets_dir = root_a.join("bindings").join("nets");
    let ma_path = nets_dir.join("m_a.sv");
    let mb_path = nets_dir.join("m_b.sv");
    let tb_path = nets_dir.join("tb.sv");
    let ma_text = fs::read_to_string(&ma_path).expect("read m_a fixture");
    let mb_text = fs::read_to_string(&mb_path).expect("read m_b fixture");
    let tb_text = fs::read_to_string(&tb_path).expect("read tb fixture");

    let mut client = LspProcess::spawn(&fixture.root);
    client
        .initialize(&[("root-a", &root_a)], default_init_options())
        .expect("initialize binding-precision workspace");
    // Opening one file triggers the root-wide compile; a clean diagnostic
    // publication for it proves the analysis carries feature data.
    client.open(&tb_path, &tb_text).expect("open tb source");
    wait_for_diagnostics(&mut client, &file_uri(&tb_path), has_no_severity_1);

    // The declaration positions of `input logic clk` in both module files.
    let ma_decl_start = position_at(&ma_text, "input logic clk", 12);
    let mb_decl_start = position_at(&mb_text, "input logic clk", 12);

    // Definition at the `clk` use inside m_a (`observed = clk`) → exactly one
    // location: m_a's own input-port declaration.
    let response = client
        .request(
            "textDocument/definition",
            json!({
                "textDocument": { "uri": file_uri(&ma_path) },
                "position": position_at(&ma_text, "observed = clk", 11)
            }),
        )
        .expect("definition request at m_a clk use");
    let (uri, start) = single_location(&response, "definition at m_a use");
    assert_eq!(uri, file_uri(&ma_path), "m_a use → m_a decl");
    assert_eq!(start, ma_decl_start, "start: {start} vs {ma_decl_start}");
    assert_no_shadow_uris(&response);

    // Same for m_b: its use must land on ITS OWN declaration, not m_a's.
    let response = client
        .request(
            "textDocument/definition",
            json!({
                "textDocument": { "uri": file_uri(&mb_path) },
                "position": position_at(&mb_text, "observed = clk", 11)
            }),
        )
        .expect("definition request at m_b clk use");
    let (uri, start) = single_location(&response, "definition at m_b use");
    assert_eq!(uri, file_uri(&mb_path), "m_b use → m_b decl");
    assert_eq!(start, mb_decl_start, "start: {start} vs {mb_decl_start}");
    assert_no_shadow_uris(&response);

    // Definition ON the declaration itself → itself.
    let response = client
        .request(
            "textDocument/definition",
            json!({
                "textDocument": { "uri": file_uri(&ma_path) },
                "position": ma_decl_start
            }),
        )
        .expect("definition request on m_a decl");
    let (uri, start) = single_location(&response, "definition on m_a decl");
    assert_eq!(uri, file_uri(&ma_path));
    assert_eq!(start, ma_decl_start);
    assert_no_shadow_uris(&response);

    // Plain signal uses of each instance-scope connection net in tb resolve
    // to tb's own declarations.
    for (net, use_needle, decl_needle) in [("wa", "wa =", "logic wa"), ("wb", "wb =", "logic wb")] {
        let expected = position_at(&tb_text, decl_needle, 6);
        let response = client
            .request(
                "textDocument/definition",
                json!({
                    "textDocument": { "uri": file_uri(&tb_path) },
                    "position": position_at(&tb_text, use_needle, 0)
                }),
            )
            .unwrap_or_else(|error| panic!("definition request at {net} use: {error}"));
        let (uri, start) = single_location(&response, &format!("definition at {net} use"));
        assert_eq!(uri, file_uri(&tb_path), "{net} use → tb decl");
        assert_eq!(start, expected, "{net}: {start} vs {expected}");
        assert_no_shadow_uris(&response);
    }

    // Cursor normalization: repeating the m_a `clk`-use definition request at
    // the LAST character column of the same identifier (col + name_len - 1)
    // must return the identical single Location — ref bindings are keyed at
    // the token-start column, which a mid-identifier cursor now reuses.
    let clk_use_col = 11;
    let clk_name_len = "clk".chars().count();
    let response = client
        .request(
            "textDocument/definition",
            json!({
                "textDocument": { "uri": file_uri(&ma_path) },
                "position": position_at(&ma_text, "observed = clk", clk_use_col + clk_name_len - 1)
            }),
        )
        .expect("definition request at m_a clk use (last character)");
    let (uri, start) = single_location(&response, "definition at m_a use (last character)");
    assert_eq!(uri, file_uri(&ma_path), "m_a last-char click → m_a decl");
    assert_eq!(start, ma_decl_start);
    assert_no_shadow_uris(&response);
    client.shutdown();
}

/// A module type and an instance identifier occupy different namespaces for
/// navigation.  In the original `foo.v` regression, `Bar Bar(...)` caused a
/// later `Bar u_bar(...)` type reference to jump to the first instance name
/// instead of the `module Bar` declaration.
#[test]
fn lsp_stdio_goto_definition_module_type_ignores_same_named_instance() {
    // Arrange
    let fixture = FixtureTree::new();
    let root_a = fixture.root("root-a");
    let source_path = root_a.join("navigation").join("foo.v");
    let source_text = fs::read_to_string(&source_path).expect("read foo.v regression fixture");
    let source_uri = file_uri(&source_path);
    let expected_module = position_at(&source_text, "module Bar", "module ".len());

    let mut client = LspProcess::spawn(fixture.base());
    client
        .initialize(&[("root-a", &root_a)], default_init_options())
        .expect("initialize module-instance collision workspace");
    client
        .open(&source_path, &source_text)
        .expect("open foo.v regression fixture");
    wait_for_diagnostics(&mut client, &source_uri, has_no_severity_1);

    // Act + Assert: both module-type occurrences resolve to the module
    // declaration, including the occurrence whose adjacent instance name is
    // also `Bar`.
    for (needle, offset) in [("Bar Bar(", 0), ("Bar u_bar(", 0)] {
        let response = client
            .request(
                "textDocument/definition",
                json!({
                    "textDocument": { "uri": source_uri },
                    "position": position_at(&source_text, needle, offset)
                }),
            )
            .unwrap_or_else(|error| panic!("definition request at {needle}: {error}"));
        let (uri, start) = single_location(&response, "definition at module type");
        assert_eq!(uri, source_uri, "{needle} must resolve within foo.v");
        assert_eq!(
            start, expected_module,
            "{needle} must resolve to module Bar"
        );
        assert_no_shadow_uris(&response);
    }

    // Control: navigation on the same-named instance identifier still uses
    // the established instance-to-module definition behavior.
    let response = client
        .request(
            "textDocument/definition",
            json!({
                "textDocument": { "uri": source_uri },
                "position": position_at(&source_text, "Bar Bar(", "Bar ".len())
            }),
        )
        .expect("definition request at same-named instance identifier");
    let (uri, start) = single_location(&response, "definition at instance name");
    assert_eq!(uri, source_uri);
    assert_eq!(start, expected_module);
    assert_no_shadow_uris(&response);
    client.shutdown();
}

/// Goto-definition at a named port-connection resolves each SIDE of the
/// connection to its own declaration: the `.clk` LABEL reaches the CHILD
/// module's PORT declaration while the connected signal (the ACTUAL) stays
/// on its OWN declaration in the instantiating (parent) scope — including
/// through a multi-line instantiation.
#[test]
fn lsp_stdio_goto_definition_port_label() {
    let fixture = FixtureTree::new();
    let root_a = fixture.root("root-a");
    let root_b = fixture.root("root-b");
    let top_path = root_a.join("bindings").join("labels").join("top.sv");
    let child_path = root_a.join("bindings").join("labels").join("child.sv");
    let top_text = fs::read_to_string(&top_path).expect("read label_top fixture");
    let child_text = fs::read_to_string(&child_path).expect("read label_child fixture");

    let mut client = LspProcess::spawn(&fixture.root);
    client
        .initialize(
            &[("root-a", &root_a), ("root-b", &root_b)],
            default_init_options(),
        )
        .expect("initialize port-label workspace");
    client.open(&top_path, &top_text).expect("open label_top");
    wait_for_diagnostics(&mut client, &file_uri(&top_path), has_no_severity_1);

    // Request at the `.clk` label column (the identifier after the dot).
    // Expected: the child port declaration line in child.sv.
    let expected_port = position_at(&child_text, "input logic clk", 12);
    let response = client
        .request(
            "textDocument/definition",
            json!({
                "textDocument": { "uri": file_uri(&top_path) },
                "position": position_at(&top_text, ".clk", 1)
            }),
        )
        .expect("definition request at .clk label");
    let (uri, start) = single_location(&response, "definition at .clk label");
    assert_eq!(
        uri,
        file_uri(&child_path),
        "label must resolve into the CHILD module file"
    );
    assert_eq!(start, expected_port, "start: {start} vs {expected_port}");
    assert_no_shadow_uris(&response);

    // Request at the connected signal (`wa` inside `.clk(wa)`): the ACTUAL
    // resolves to its OWN declaration in the instantiating (parent) scope —
    // `logic wa` in this very file — never into the child module.
    let parent_wa = position_at(&top_text, "logic wa", 6);
    let response = client
        .request(
            "textDocument/definition",
            json!({
                "textDocument": { "uri": file_uri(&top_path) },
                "position": position_at(&top_text, "(wa)", 1)
            }),
        )
        .expect("definition request at connection actual");
    let (uri, start) = single_location(&response, "definition at connection actual");
    assert_eq!(
        uri,
        file_uri(&top_path),
        "the connected signal must resolve to its PARENT-scope declaration"
    );
    assert_eq!(start, parent_wa, "start: {start} vs {parent_wa}");
    assert_no_shadow_uris(&response);

    // Multi-line variant via the existing root-b ports fixture:
    // `.clk\n      (clk)` under `port_child u_child (`.
    let ml_top_path = root_b.join("ports").join("top.sv");
    let ml_child_path = root_b.join("ports").join("child.sv");
    let ml_top_text = fs::read_to_string(&ml_top_path).expect("read multiline top fixture");
    let ml_child_text = fs::read_to_string(&ml_child_path).expect("read multiline child fixture");
    let ml_expected_port = position_at(&ml_child_text, "input logic clk", 12);
    let response = client
        .request(
            "textDocument/definition",
            json!({
                "textDocument": { "uri": file_uri(&ml_top_path) },
                "position": position_at(&ml_top_text, ".clk", 1)
            }),
        )
        .expect("definition request at multi-line .clk label");
    let (uri, start) = single_location(&response, "definition at multi-line .clk label");
    assert_eq!(uri, file_uri(&ml_child_path));
    assert_eq!(
        start, ml_expected_port,
        "start: {start} vs {ml_expected_port}"
    );
    assert_no_shadow_uris(&response);

    // Multi-line ACTUAL (`clk` inside the continuation-line `(clk)`): the
    // ACTUAL stays on port_top's OWN `logic clk` declaration — same file,
    // never the child module.
    let ml_parent_clk = position_at(&ml_top_text, "logic clk", 6);
    let response = client
        .request(
            "textDocument/definition",
            json!({
                "textDocument": { "uri": file_uri(&ml_top_path) },
                "position": position_at(&ml_top_text, "(clk)", 1)
            }),
        )
        .expect("definition request at multi-line connection actual");
    let (uri, start) = single_location(&response, "definition at multi-line connection actual");
    assert_eq!(
        uri,
        file_uri(&ml_top_path),
        "multi-line actual must resolve to its PARENT-scope declaration"
    );
    assert_eq!(start, ml_parent_clk, "start: {start} vs {ml_parent_clk}");
    assert_no_shadow_uris(&response);
    client.shutdown();
}
