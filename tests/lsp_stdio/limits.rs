//! Limits.

use super::*;

#[test]
fn lsp_stdio_source_limits_log_errors_and_publish_configuration_guidance() {
    let fixture = FixtureTree::new();
    let root = fixture.root("resource-limits");
    fs::create_dir_all(&root).unwrap();
    let source = "module small; endmodule\n";
    let first = root.join("small.sv");
    fs::write(&first, source).unwrap();
    fs::write(root.join("other.sv"), source.replace("small", "other")).unwrap();
    for (label, per_file, total) in [
        ("per-file", source.len() - 1, 1024),
        ("total", 1024, source.len() * 2 - 1),
    ] {
        fs::write(root.join("llg.toml"), format!(
            "schema_version = 1\n[analysis]\nmax_file_bytes = {per_file}\nmax_total_input_bytes = {total}\n"
        )).unwrap();
        let log_path = fixture.root.join(format!("source-{label}.log"));
        let mut client = LspProcess::spawn_configured(&fixture.root, |command| {
            command
                .env("LLG_LOG", "error")
                .env("LLG_LOG_FILE", &log_path);
        });
        client
            .initialize(&[("resource-limits", &root)], default_init_options())
            .unwrap();
        let diagnostics = client
            .wait_for_notification_where("textDocument/publishDiagnostics", |value| {
                value.to_string().contains("input-size-limit")
            })
            .expect("source-limit diagnostic");
        let message = diagnostics.to_string();
        for expected in [
            "[sources].exclude",
            "llg.toml",
            "[analysis].max_file_bytes",
            "max_total_input_bytes",
            "LLG_MEMORY_LIMIT_MB",
        ] {
            assert!(
                message.contains(expected),
                "{label}: missing {expected}: {message}"
            );
        }
        client.shutdown();
        let log = fs::read_to_string(&log_path).unwrap();
        for expected in [
            "[ERROR] event=analysis.input_limit",
            "configured_limit=",
            "[sources].exclude",
            "llg.toml",
            "that setting alone does not raise source-size limits",
        ] {
            assert!(log.contains(expected), "{label}: missing {expected}: {log}");
        }
    }
}
