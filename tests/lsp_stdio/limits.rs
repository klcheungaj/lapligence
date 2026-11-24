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

/// Each iteration of the loop generate elaborates to more than 50 semantic
/// nodes, so the default analysis holds over 100,000 of them.
const GENERATED_REGISTERS: &str = "\
module registers #(parameter int N = 2500) (input logic clk);
  wire [N-1:0] d;
  integer ones;
  genvar i;
  for (i = 0; i < N; i = i + 1) begin : workers
    logic never_read;
    assign d[i] = i[0];
    always @(posedge clk) begin
      never_read <= d[i];
      if (ones == 3) ones = ones + (d[i] === 1'b1) + (d[i] !== 1'b0);
    end
  end
endmodule
";

/// The value that follows `key` in `text`, up to the next delimiter.
fn number_after(text: &str, key: &str) -> u64 {
    text.split(key)
        .nth(1)
        .and_then(|rest| rest.split(|c: char| !c.is_ascii_digit()).next())
        .and_then(|digits| digits.parse().ok())
        .unwrap_or_else(|| panic!("missing {key}: {text}"))
}

#[test]
fn lsp_stdio_semantic_node_budget_covers_large_designs_and_is_configurable() {
    const OLD_FIXED_BUDGET: u64 = 100_000;
    const LOW_BUDGET: u64 = 2_000;
    const REDUCED: &str = "serving bounded declaration-level navigation";

    let fixture = FixtureTree::new();
    let root = fixture.root("node-budget");
    fs::create_dir_all(&root).unwrap();
    let path = root.join("registers.sv");
    fs::write(&path, GENERATED_REGISTERS).unwrap();
    let uri = file_uri(&path);
    let config = root.join("llg.toml");
    let write_budget = |budget: u64| {
        fs::write(
            &config,
            format!("schema_version = 1\n[analysis]\nmax_semantic_nodes = {budget}\n"),
        )
        .unwrap();
    };
    let spawn = |log_path: &Path| {
        let mut client = LspProcess::spawn_configured(&fixture.root, |command| {
            command
                .env("LLG_LOG", "debug")
                .env("LLG_LOG_FILE", log_path);
        });
        client
            .initialize(&[("node-budget", &root)], default_init_options())
            .unwrap();
        client
    };
    let is_reduced = |params: &Value| params.to_string().contains(REDUCED);

    // The default budget elaborates the whole design.
    fs::write(&config, "schema_version = 1\n").unwrap();
    let log_path = fixture.root.join("default-budget.log");
    let mut client = spawn(&log_path);
    let diagnostics = wait_for_diagnostics(&mut client, &uri, |params| {
        has_lint_rule(params, "unused-signal") || is_reduced(params)
    });
    assert!(has_lint_rule(&diagnostics, "unused-signal"));
    assert!(!is_reduced(&diagnostics));
    client.shutdown();
    let log = fs::read_to_string(&log_path).unwrap();
    assert!(!log.contains("event=slang.resource_limit"), "{log}");
    let nodes = number_after(&log, "semantic_nodes=");
    assert!(nodes > OLD_FIXED_BUDGET, "{nodes}");

    // A configured budget below that reduces the analysis and says what to do.
    write_budget(LOW_BUDGET);
    let log_path = fixture.root.join("low-budget.log");
    let mut client = spawn(&log_path);
    let diagnostics = wait_for_diagnostics(&mut client, &uri, is_reduced);
    assert!(!has_lint_rule(&diagnostics, "unused-signal"));
    let message = diagnostics.to_string();
    for text in [&message, &fs::read_to_string(&log_path).unwrap()] {
        assert!(
            text.contains(&format!(
                "semantic node limit exceeded (limit={LOW_BUDGET}, needs about "
            )),
            "{text}"
        );
        assert!(text.contains("[analysis].max_semantic_nodes"), "{text}");
    }
    let log = fs::read_to_string(&log_path).unwrap();
    assert!(
        log.contains("[ERROR] event=slang.resource_limit")
            && log.contains(&format!("max_semantic_nodes={LOW_BUDGET} ")),
        "{log}"
    );

    // Raising the key as the message says restores the full analysis.
    let needed = number_after(&message, "needs about ");
    assert!(needed.abs_diff(nodes) <= nodes / 20, "{needed} {nodes}");
    write_budget(needed + needed / 10);
    client.send_watch_event(&config, 2).unwrap();
    let diagnostics = wait_for_diagnostics(&mut client, &uri, |params| {
        has_lint_rule(params, "unused-signal")
    });
    assert!(!is_reduced(&diagnostics));
    client.shutdown();
}
