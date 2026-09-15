//! Diagnostics.

use super::*;

// ── Contract tests ───────────────────────────────────────────────────────────

#[test]
fn lsp_stdio_slang_diagnostics_publish_utf16_and_clear() {
    let id = NEXT_FIXTURE_ID.fetch_add(1, Ordering::Relaxed);
    let base = std::env::temp_dir().join(format!(
        "llg-lsp-slang-diagnostics-{}-{id}",
        std::process::id()
    ));
    let root = base.join("workspace");
    fs::create_dir_all(&root).expect("create Slang diagnostics workspace");
    fs::write(
        root.join(CONFIG_FILE),
        "schema_version = 1\n[sources]\ndirectories = [\".\"]\n[lint]\nenabled = false\n",
    )
    .expect("write Slang diagnostics config");
    let path = root.join("broken.sv");
    let invalid = concat!(
        "// llg-lsp-fixture: generated/slang-diagnostics/broken.sv\n",
        "module broken;\n",
        "  string label = \"😀\"; logic value = ;\n",
        "endmodule\n",
    );
    let valid = concat!(
        "// llg-lsp-fixture: generated/slang-diagnostics/broken.sv\n",
        "module broken;\n",
        "  string label = \"😀\"; logic value = 1'b0;\n",
        "endmodule\n",
    );
    fs::write(&path, invalid).expect("write initial malformed source");
    let uri = file_uri(&path);
    let mut client = LspProcess::spawn(&base);
    client
        .initialize(&[("slang-diagnostics", &root)], default_init_options())
        .expect("initialize Slang diagnostics workspace");
    client.open(&path, invalid).expect("open malformed source");

    let published = wait_for_diagnostics(&mut client, &uri, |params| {
        params
            .get("diagnostics")
            .and_then(Value::as_array)
            .is_some_and(|diagnostics| {
                diagnostics.iter().any(|diagnostic| {
                    diagnostic.get("source").and_then(Value::as_str) == Some("slang-compiler")
                })
            })
    });
    assert_no_shadow_uris(&published);
    let diagnostic = published["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .find(|diagnostic| {
            diagnostic.get("source").and_then(Value::as_str) == Some("slang-compiler")
        })
        .expect("Slang compiler diagnostic");
    assert!(diagnostic
        .get("code")
        .and_then(Value::as_str)
        .is_some_and(|code| code.starts_with("slang.compiler.")));
    assert_eq!(diagnostic["range"]["start"]["line"].as_u64(), Some(2));
    let error_line = invalid.lines().nth(2).unwrap();
    let semicolon = error_line.rfind(';').unwrap();
    let expected_utf16 = error_line[..semicolon].encode_utf16().count() as u64;
    assert_eq!(
        diagnostic["range"]["start"]["character"].as_u64(),
        Some(expected_utf16),
        "diagnostic must count the supplementary character as two UTF-16 code units: {diagnostic}"
    );

    client
        .change(&path, 2, valid)
        .expect("repair malformed source");
    let cleared = wait_for_diagnostics(&mut client, &uri, |params| {
        params
            .get("diagnostics")
            .and_then(Value::as_array)
            .is_some_and(|diagnostics| {
                diagnostics.iter().all(|diagnostic| {
                    diagnostic.get("source").and_then(Value::as_str) != Some("slang-compiler")
                })
            })
    });
    assert_no_shadow_uris(&cleared);
    client.shutdown();
    let _ = fs::remove_dir_all(base);
}
