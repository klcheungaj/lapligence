//! Hover completion.

use super::*;

/// Hover over a declaration returns markup content, and completion returns a
/// valid non-empty response shape (review G: these requests had no stdio
/// coverage).
#[test]
fn lsp_stdio_hover_and_completion_return_valid_shapes() {
    let fixture = FixtureTree::new();
    let root_a = fixture.root("root-a");
    let path = root_a.join("navigation").join("snapshot.sv");
    let valid = fs::read_to_string(&path).expect("read snapshot fixture");
    let uri = file_uri(&path);
    let mut client = LspProcess::spawn(&fixture.root);
    client
        .initialize(&[("root-a", &root_a)], default_init_options())
        .expect("initialize hover workspace");
    client.open(&path, &valid).expect("open snapshot source");
    wait_for_diagnostics(&mut client, &uri, has_no_severity_1);

    // Hover over the module name of the module DECLARATION.
    let hover = client
        .request(
            "textDocument/hover",
            json!({
                "textDocument": { "uri": uri },
                "position": position_at(&valid, "module SnapshotTop", 9)
            }),
        )
        .expect("hover request");
    let contents = hover
        .as_object()
        .and_then(|hover| hover.get("contents"))
        .expect("hover result must carry contents");
    let value = match contents.get("value").and_then(Value::as_str) {
        Some(markup) => markup.to_owned(),
        None => contents.to_string(),
    };
    assert!(
        value.contains("SnapshotTop"),
        "hover over the module name must mention it: {value:?}"
    );
    assert_no_shadow_uris(&hover);

    // Completion anywhere in the indexed file yields a well-formed,
    // non-empty item list (array form or {items:[...]}).
    let completion = client
        .request(
            "textDocument/completion",
            json!({
                "textDocument": { "uri": uri },
                "position": position_at(&valid, "endmodule", 0)
            }),
        )
        .expect("completion request");
    let items = match &completion {
        Value::Array(items) => items.clone(),
        Value::Object(object) => object
            .get("items")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default(),
        other => panic!("completion returned an invalid shape: {other}"),
    };
    assert!(
        !items.is_empty(),
        "completion in an indexed file must return items: {completion}"
    );
    assert!(
        items.iter().all(|item| item.get("label").is_some()),
        "every completion item needs a label: {items:?}"
    );
    assert_no_shadow_uris(&completion);
    client.shutdown();
}
