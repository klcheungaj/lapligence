//! Tokens.

use super::*;

/// The custom `llg/dumpTokens` request serves the CLI dump rows for one open
/// document (workspace-relative positions, `bind=` fields) plus the trailing
/// `# analysis:` summary line.
#[test]
fn lsp_stdio_serves_custom_dump_tokens_request() {
    let fixture = FixtureTree::new();
    let root_a = fixture.root("root-a");
    let target_path = root_a.join("bindings").join("nets").join("m_a.sv");
    let target_uri = file_uri(&target_path);
    let target_text = fs::read_to_string(&target_path).expect("read m_a fixture");

    let mut client = LspProcess::spawn(&fixture.root);
    client
        .initialize(&[("root-a", &root_a)], default_init_options())
        .expect("initialize dump-tokens workspace");
    client.open(&target_path, &target_text).expect("open m_a");
    wait_for_diagnostics(&mut client, &target_uri, has_no_severity_1);

    let result = client
        .request("llg/dumpTokens", json!({ "uri": target_uri }))
        .expect("llg/dumpTokens request");
    let lines: Vec<String> = result
        .get("lines")
        .and_then(Value::as_array)
        .expect("dumpTokens result.lines array")
        .iter()
        .map(|line| line.as_str().expect("dumpTokens line string").to_owned())
        .collect();
    assert!(!lines.is_empty(), "dumpTokens returned no lines: {result}");
    assert!(
        lines
            .iter()
            .any(|line| line.starts_with("bindings/nets/m_a.sv:")),
        "dumpTokens rows must use workspace-relative file positions (no shadow paths): {lines:?}"
    );
    assert!(
        lines.iter().any(|line| line.contains("bind=")),
        "dumpTokens rows must carry the bind= field: {lines:?}"
    );
    assert!(
        lines
            .last()
            .is_some_and(|line| line.starts_with("# analysis:")),
        "dumpTokens response must end with the # analysis summary line: {lines:?}"
    );

    // A document owned by NO workspace root answers with an error line
    // instead of failing the request.  The path never has to exist; it only
    // must stay outside every root, and the pid suffix keeps concurrent test
    // processes from sharing one name.
    let outside = std::env::temp_dir().join(format!(
        "llg-dump-tokens-outside-root-{}.sv",
        std::process::id()
    ));
    let missing = client
        .request("llg/dumpTokens", json!({ "uri": file_uri(&outside) }))
        .expect("llg/dumpTokens request for unowned document");
    let missing_lines = missing.get("lines").and_then(Value::as_array);
    assert!(
        missing_lines.is_some_and(|lines| lines.len() == 1
            && lines[0]
                .as_str()
                .unwrap_or_default()
                .starts_with("# error:")),
        "unknown document must yield a single # error line: {missing}"
    );
    client.shutdown();
}
