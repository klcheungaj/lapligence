//! Constructs the simulator rejects by design (ADV-032) are still valid
//! source for the language server: analysis serves diagnostics, symbols and
//! tokens for them instead of failing.

use super::*;

const UNSUPPORTED_FIXTURES: &[&str] = &[
    "mos_primitives.v",
    "trireg_nets.v",
    "directive_default_decay_time.v",
    "directive_delay_mode_zero.v",
    "dumpports.v",
    "inspection_tasks.v",
    "pla_tasks.v",
    "queue_add_array_element.v",
    "assign_in_task.v",
    "assign_struct.sv",
];

#[test]
fn lsp_stdio_serves_files_with_simulator_unsupported_constructs() {
    let id = NEXT_FIXTURE_ID.fetch_add(1, Ordering::Relaxed);
    let base = std::env::temp_dir().join(format!(
        "llg-lsp-legacy-constructs-{}-{id}",
        std::process::id()
    ));
    let _cleanup = TempDirCleanup(base.clone());
    let root = base.join("workspace");
    fs::create_dir_all(&root).expect("create legacy-constructs workspace");
    fs::write(
        root.join(CONFIG_FILE),
        "schema_version = 1\n[sources]\ndirectories = [\".\"]\ninclude = [\"**/*.v\", \"**/*.sv\"]\n",
    )
    .expect("write legacy-constructs config");
    let fixtures =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sim/feature_completion/adv_032");
    let mut files = Vec::new();
    for name in UNSUPPORTED_FIXTURES {
        let text = fs::read_to_string(fixtures.join(name)).expect("read fixture");
        let path = root.join(name);
        fs::write(&path, &text).expect("copy fixture");
        files.push((path, text));
    }

    let mut client = LspProcess::spawn(&base);
    client
        .initialize(&[("legacy-constructs", &root)], default_init_options())
        .expect("initialize legacy-constructs workspace");
    for (path, text) in &files {
        client.open(path, text).expect("open fixture");
    }
    for (path, text) in &files {
        let uri = file_uri(path);
        let published = wait_for_diagnostics(&mut client, &uri, |_| true);
        let errors: Vec<_> = published["diagnostics"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|diagnostic| diagnostic.get("severity").and_then(Value::as_u64) == Some(1))
            .collect();
        assert!(
            errors.is_empty(),
            "{}: the server must not report simulator-only limits as errors: {errors:?}",
            path.display()
        );
        let symbols = client
            .request(
                "textDocument/documentSymbol",
                json!({ "textDocument": { "uri": uri } }),
            )
            .unwrap_or_else(|error| panic!("{}: documentSymbol: {error}", path.display()));
        assert!(
            symbols
                .as_array()
                .is_some_and(|symbols| !symbols.is_empty()),
            "{}: no symbols served: {symbols}",
            path.display()
        );
        let tokens = client
            .request(
                "textDocument/semanticTokens/full",
                json!({ "textDocument": { "uri": uri } }),
            )
            .unwrap_or_else(|error| panic!("{}: semanticTokens: {error}", path.display()));
        assert!(
            tokens["data"]
                .as_array()
                .is_some_and(|data| !data.is_empty()),
            "{}: no tokens served for {} bytes: {tokens}",
            path.display(),
            text.len()
        );
    }
    client.shutdown();
}
