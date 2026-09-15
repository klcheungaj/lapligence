//! Connections.

use super::*;

/// Connection navigation survives a SYNTAX-BROKEN sibling: with `broken.v`
/// keeping the whole root at a partial analysis outcome, the lexical fallback
/// still binds both sides of a named port connection
/// — the `.clk` LABEL to the child module's port declaration and the
/// ACTUAL to its own declaration in the instantiating scope — visible in
/// `llg/dumpTokens` `bind=` rows (`via=label` / `via=connection`) and
/// served by `textDocument/definition`.
#[test]
fn lsp_stdio_parse_fallback_binds_port_connections_to_child_ports() {
    let base = std::env::temp_dir().join(format!("llg-lsp-stdio-fb-{}", std::process::id()));
    let ws = base.join("fallback-ws");
    fs::create_dir_all(&ws).expect("create fallback workspace");
    let child_path = ws.join("fb_child.sv");
    let tb_path = ws.join("tb_fb.sv");
    let broken_path = ws.join("broken.v");
    fs::write(
        &child_path,
        "module fb_child(input logic clk, output logic q);\n  assign q = clk;\nendmodule\n",
    )
    .expect("write fb_child");
    let tb_text = "module tb_fb;\n  logic wa;\n  logic t_q;\n\n  fb_child u_fb(.clk(wa), .q(t_q));\nendmodule\n";
    fs::write(&tb_path, tb_text).expect("write tb_fb");
    // Unterminated module on purpose: no complete semantic model is available.
    fs::write(&broken_path, "module broken(\n   input clk\n").expect("write broken.v");
    fs::write(
        ws.join(CONFIG_FILE),
        "schema_version = 1\n\n[sources]\ndirectories = [\".\"]\ninclude = [\"**/*.v\", \"**/*.sv\"]\n",
    )
    .expect("write llg.toml");

    struct Cleanup(PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(base.clone());

    let mut client = LspProcess::spawn(&base);
    client
        .initialize(&[("fallback-ws", &ws)], default_init_options())
        .expect("initialize fallback workspace");
    client.open(&tb_path, tb_text).expect("open tb_fb");

    // Poll the dump until the parse-fallback analysis carries the connection
    // bindings (the ~300 ms debounce plus the compile delay mean the first
    // answer can predate them).
    let tb_uri = file_uri(&tb_path);
    let deadline = Instant::now() + POLL_TIMEOUT;
    let mut interval = POLL_INTERVAL;
    let lines: Vec<String> = loop {
        match client.request_with_timeout(
            "llg/dumpTokens",
            json!({ "uri": tb_uri }),
            POLL_REQUEST_TIMEOUT,
        ) {
            Ok(result) => {
                let lines: Vec<String> = result
                    .get("lines")
                    .and_then(Value::as_array)
                    .map(|a| {
                        a.iter()
                            .map(|l| l.as_str().unwrap_or_default().to_owned())
                            .collect()
                    })
                    .unwrap_or_default();
                let want_clk = "bind=fb_child.sv:0:28[clk,port]";
                let want_q = "bind=fb_child.sv:0:46[q,port]";
                let ready = lines.iter().any(|l| l.contains(want_clk))
                    && lines.iter().any(|l| l.contains(want_q));
                if ready {
                    break lines;
                }
            }
            Err(error) if error.starts_with("timed out") => {}
            Err(error) => panic!("llg/dumpTokens failed: {error}"),
        }
        assert!(
            Instant::now() < deadline,
            "parse-fallback connection bindings never appeared in the dump"
        );
        thread::sleep(interval);
        interval = interval.saturating_mul(2).min(Duration::from_secs(1));
    };

    // Dump oracle: the label row points INTO the child module, the actual
    // rows stay on tb_fb's OWN declarations — each with its provenance tag.
    let row = |prefix: &str| -> String {
        lines
            .iter()
            .find(|l| l.starts_with(prefix))
            .unwrap_or_else(|| panic!("no dump row at {prefix}: {lines:?}"))
            .clone()
    };
    let label_row = row("tb_fb.sv:4:17");
    let actual_row = row("tb_fb.sv:4:21");
    assert!(
        label_row.contains("REF")
            && label_row.contains("via=label")
            && label_row.contains("bind=fb_child.sv:0:28[clk,port]"),
        "label row must bind to the child port: {label_row}"
    );
    assert!(
        actual_row.contains("REF")
            && actual_row.contains("via=connection")
            && actual_row.contains("bind=tb_fb.sv:1:8[wa,variable]"),
        "actual row must bind to its parent-scope declaration: {actual_row}"
    );
    let q_actual_row = row("tb_fb.sv:4:29");
    assert!(
        q_actual_row.contains("via=connection")
            && q_actual_row.contains("bind=tb_fb.sv:2:8[t_q,variable]"),
        "t_q row must bind to its parent-scope declaration: {q_actual_row}"
    );

    // Wire oracle: goto-definition at the label lands on the child port,
    // at the actual on the same-file parent-scope declaration.
    let expected_clk = position_at(
        &fs::read_to_string(&child_path).expect("reread fb_child"),
        "input logic clk",
        12,
    );
    let expected_wa = position_at(tb_text, "logic wa", 6);
    for (what, position, want_uri, want_start) in [
        (
            "label",
            position_at(tb_text, ".clk", 1),
            file_uri(&child_path),
            expected_clk.clone(),
        ),
        (
            "actual",
            position_at(tb_text, "(wa)", 1),
            file_uri(&tb_path),
            expected_wa,
        ),
    ] {
        let response = client
            .request(
                "textDocument/definition",
                json!({
                    "textDocument": { "uri": tb_uri },
                    "position": position
                }),
            )
            .unwrap_or_else(|error| panic!("definition request at {what}: {error}"));
        let (uri, start) = single_location(&response, &format!("parse-mode definition at {what}"));
        assert_eq!(uri, want_uri, "{what}: wrong target file");
        assert_eq!(start, want_start, "{what}: {start} vs {want_start}");
        assert_no_shadow_uris(&response);
    }

    // The broken sibling keeps the root at outcome=parse: the trailing dump
    // summary proves this test really exercised the fallback path.
    let summary = lines.last().cloned().unwrap_or_default();
    assert!(
        summary.contains("outcome=parse"),
        "expected the parse-fallback outcome in the summary: {summary}"
    );
    client.shutdown();
}

/// Goto-definition at a named PARAMETER override resolves each SIDE of the
/// override to its own declaration: the `.W` LABEL reaches the CHILD module's
/// PARAMETER declaration while the override RHS (the `W` inside `.D(W)`)
/// stays on its OWN declaration in the instantiating (parent) scope — even
/// though that scope holds a same-named `localparam W` decoy that name-based
/// resolution would wrongly pick for the label.  Single- and multi-line
/// instantiations.
#[test]
fn lsp_stdio_goto_definition_param_label() {
    let fixture = FixtureTree::new();
    let root_a = fixture.root("root-a");
    let root_b = fixture.root("root-b");
    let top_path = root_a.join("bindings").join("params").join("top.sv");
    let child_path = root_a.join("bindings").join("params").join("child.sv");
    let top_text = fs::read_to_string(&top_path).expect("read param_top fixture");
    let child_text = fs::read_to_string(&child_path).expect("read param_child fixture");

    let mut client = LspProcess::spawn(&fixture.root);
    client
        .initialize(
            &[("root-a", &root_a), ("root-b", &root_b)],
            default_init_options(),
        )
        .expect("initialize param-label workspace");
    client.open(&top_path, &top_text).expect("open param top");
    wait_for_diagnostics(&mut client, &file_uri(&top_path), has_no_severity_1);

    // Request at the `.W` label identifier.  Expected: the child parameter
    // declaration line in child.sv — never the same-named localparam decoy.
    let expected_w = position_at(&child_text, "parameter int W", 14);
    let response = client
        .request(
            "textDocument/definition",
            json!({
                "textDocument": { "uri": file_uri(&top_path) },
                "position": position_at(&top_text, ".W", 1)
            }),
        )
        .expect("definition request at .W label");
    let (uri, start) = single_location(&response, "definition at .W label");
    assert_eq!(
        uri,
        file_uri(&child_path),
        "override label must resolve into the CHILD module file"
    );
    assert_eq!(start, expected_w, "start: {start} vs {expected_w}");
    assert_no_shadow_uris(&response);

    // The `.D` label likewise reaches the child's D parameter.
    let expected_d = position_at(&child_text, "parameter int D", 14);
    let response = client
        .request(
            "textDocument/definition",
            json!({
                "textDocument": { "uri": file_uri(&top_path) },
                "position": position_at(&top_text, ".D(", 1)
            }),
        )
        .expect("definition request at .D label");
    let (uri, start) = single_location(&response, "definition at .D label");
    assert_eq!(uri, file_uri(&child_path));
    assert_eq!(start, expected_d, "start: {start} vs {expected_d}");
    assert_no_shadow_uris(&response);

    // The override RHS `W` inside `.D(W)` resolves to the DECOY localparam
    // in the instantiating scope — the opposite direction of the label.
    let expected_decoy = position_at(&top_text, "localparam int W", 15);
    let response = client
        .request(
            "textDocument/definition",
            json!({
                "textDocument": { "uri": file_uri(&top_path) },
                "position": position_at(&top_text, ".D(W)", 3)
            }),
        )
        .expect("definition request at override RHS");
    let (uri, start) = single_location(&response, "definition at override RHS");
    assert_eq!(
        uri,
        file_uri(&top_path),
        "the override RHS must resolve to its PARENT-scope declaration"
    );
    assert_eq!(start, expected_decoy, "start: {start} vs {expected_decoy}");
    assert_no_shadow_uris(&response);

    // Multi-line variant via root-b: the `.W` label sits on a continuation
    // line below `ml_pchild #(` and still reaches the child parameter.
    let ml_top_path = root_b.join("params").join("top.sv");
    let ml_child_path = root_b.join("params").join("child.sv");
    let ml_top_text = fs::read_to_string(&ml_top_path).expect("read multiline param top fixture");
    let ml_child_text =
        fs::read_to_string(&ml_child_path).expect("read multiline param child fixture");
    client
        .open(&ml_top_path, &ml_top_text)
        .expect("open multiline param top");
    wait_for_diagnostics(&mut client, &file_uri(&ml_top_path), has_no_severity_1);

    let ml_expected_w = position_at(&ml_child_text, "parameter int W", 14);
    let response = client
        .request(
            "textDocument/definition",
            json!({
                "textDocument": { "uri": file_uri(&ml_top_path) },
                "position": position_at(&ml_top_text, ".W", 1)
            }),
        )
        .expect("definition request at multi-line .W label");
    let (uri, start) = single_location(&response, "definition at multi-line .W label");
    assert_eq!(uri, file_uri(&ml_child_path));
    assert_eq!(start, ml_expected_w, "start: {start} vs {ml_expected_w}");
    assert_no_shadow_uris(&response);

    // Multi-line RHS `W` stays on the instantiating scope's localparam.
    let ml_expected_decoy = position_at(&ml_top_text, "localparam int W", 15);
    let response = client
        .request(
            "textDocument/definition",
            json!({
                "textDocument": { "uri": file_uri(&ml_top_path) },
                "position": position_at(&ml_top_text, "(W)", 1)
            }),
        )
        .expect("definition request at multi-line override RHS");
    let (uri, start) = single_location(&response, "definition at multi-line override RHS");
    assert_eq!(
        uri,
        file_uri(&ml_top_path),
        "multi-line override RHS must resolve to its PARENT-scope declaration"
    );
    assert_eq!(
        start, ml_expected_decoy,
        "start: {start} vs {ml_expected_decoy}"
    );
    assert_no_shadow_uris(&response);
    client.shutdown();
}

/// Parameter-override navigation survives a SYNTAX-BROKEN sibling: with
/// `broken.v` keeping the whole root at a partial analysis outcome, the
/// lexical fallback still binds both sides of a named
/// parameter override — the `.PW` LABEL to the child module's parameter
/// declaration and the RHS reference to its own declaration in the
/// instantiating scope — visible in `llg/dumpTokens` `bind=` rows
/// (`via=label` / `via=connection`) and served by
/// `textDocument/definition`.
#[test]
fn lsp_stdio_parse_fallback_binds_param_overrides_to_child_params() {
    let base = std::env::temp_dir().join(format!("llg-lsp-stdio-pfb-{}", std::process::id()));
    let ws = base.join("fallback-ws");
    fs::create_dir_all(&ws).expect("create fallback workspace");
    let child_path = ws.join("fb_pchild.sv");
    let tb_path = ws.join("tb_fb2.sv");
    let broken_path = ws.join("broken.v");
    let child_text = concat!(
        "module fb_pchild #(\n",
        "  parameter int PW = 8,\n",
        "  parameter int PD = 3\n",
        ") (\n",
        "  input logic clk,\n",
        "  output logic [7:0] q\n",
        ");\n",
        "  assign q = '0;\n",
        "endmodule\n",
    );
    fs::write(&child_path, child_text).expect("write fb_pchild");
    let tb_text = concat!(
        "module tb_fb2;\n",
        "  localparam int PW = 1;\n",
        "  logic wa;\n",
        "  logic [7:0] t_q;\n",
        "\n",
        "  fb_pchild #(\n",
        "    .PW(4),\n",
        "    .PD(wa)\n",
        "  ) u_fb (\n",
        "    .clk(wa),\n",
        "    .q(t_q)\n",
        "  );\n",
        "endmodule\n",
    );
    fs::write(&tb_path, tb_text).expect("write tb_fb2");
    // Unterminated module on purpose: no complete semantic model is available.
    fs::write(&broken_path, "module broken(\n   input clk\n").expect("write broken.v");
    fs::write(
        ws.join(CONFIG_FILE),
        "schema_version = 1\n\n[sources]\ndirectories = [\".\"]\ninclude = [\"**/*.v\", \"**/*.sv\"]\n",
    )
    .expect("write llg.toml");

    struct Cleanup(PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(base.clone());

    let mut client = LspProcess::spawn(&base);
    client
        .initialize(&[("fallback-ws", &ws)], default_init_options())
        .expect("initialize param fallback workspace");
    client.open(&tb_path, tb_text).expect("open tb_fb2");

    // Expected bind targets, computed from the fixture texts.
    let pw_decl = position_at(child_text, "parameter int PW", 14);
    let pd_decl = position_at(child_text, "parameter int PD", 14);
    let wa_decl = position_at(tb_text, "logic wa", 6);
    let bind_field = |target: &Value, name: &str, kind: &str| {
        format!(
            "bind=fb_pchild.sv:{}:{}[{name},{kind}]",
            target["line"].as_u64().unwrap_or(u64::MAX),
            target["character"].as_u64().unwrap_or(u64::MAX)
        )
    };
    let want_pw = bind_field(&pw_decl, "PW", "parameter");
    let want_pd = bind_field(&pd_decl, "PD", "parameter");
    let rhs_pos = position_at(tb_text, "(wa)", 1);

    // Poll the dump until the parse-fallback analysis carries the override
    // bindings (debounce plus compile delay mean the first answer can
    // predate them).
    let tb_uri = file_uri(&tb_path);
    let deadline = Instant::now() + POLL_TIMEOUT;
    let mut interval = POLL_INTERVAL;
    let lines: Vec<String> = loop {
        match client.request_with_timeout(
            "llg/dumpTokens",
            json!({ "uri": tb_uri }),
            POLL_REQUEST_TIMEOUT,
        ) {
            Ok(result) => {
                let lines: Vec<String> = result
                    .get("lines")
                    .and_then(Value::as_array)
                    .map(|a| {
                        a.iter()
                            .map(|l| l.as_str().unwrap_or_default().to_owned())
                            .collect()
                    })
                    .unwrap_or_default();
                let ready = lines.iter().any(|l| l.contains(&want_pw))
                    && lines.iter().any(|l| l.contains(&want_pd));
                if ready {
                    break lines;
                }
            }
            Err(error) if error.starts_with("timed out") => {}
            Err(error) => panic!("llg/dumpTokens failed: {error}"),
        }
        assert!(
            Instant::now() < deadline,
            "parse-fallback override bindings never appeared in the dump"
        );
        thread::sleep(interval);
        interval = interval.saturating_mul(2).min(Duration::from_secs(1));
    };

    // Dump oracle: the label rows point INTO the child module (parameters,
    // via=label), the RHS row stays on tb_fb2's OWN declaration.
    let row = |prefix: &str| -> String {
        lines
            .iter()
            .find(|l| l.starts_with(prefix))
            .unwrap_or_else(|| panic!("no dump row at {prefix}: {lines:?}"))
            .clone()
    };
    let pw_label_row = row("tb_fb2.sv:6:5");
    assert!(
        pw_label_row.contains("REF")
            && pw_label_row.contains("via=label")
            && pw_label_row.contains(&want_pw),
        ".PW label row must bind to the child parameter: {pw_label_row}"
    );
    let pd_label_row = row("tb_fb2.sv:7:5");
    assert!(
        pd_label_row.contains("via=label") && pd_label_row.contains(&want_pd),
        ".PD label row must bind to the child parameter: {pd_label_row}"
    );
    let rhs_row = row(&format!(
        "tb_fb2.sv:{}:{}",
        rhs_pos["line"].as_u64().unwrap(),
        rhs_pos["character"].as_u64().unwrap()
    ));
    assert!(
        rhs_row.contains("via=connection")
            && rhs_row.contains(&format!(
                "bind=tb_fb2.sv:{}:{}[wa,variable]",
                wa_decl["line"].as_u64().unwrap(),
                wa_decl["character"].as_u64().unwrap()
            )),
        "RHS row must bind to its parent-scope declaration: {rhs_row}"
    );

    // Wire oracle: goto-definition at the label lands on the child
    // parameter, at the RHS on the same-file parent-scope declaration.
    let expected_pw_start = json!({
        "line": pw_decl["line"],
        "character": pw_decl["character"]
    });
    for (what, position, want_uri, want_start) in [
        (
            ".PW label",
            position_at(tb_text, ".PW", 1),
            file_uri(&child_path),
            expected_pw_start.clone(),
        ),
        (
            "RHS wa",
            position_at(tb_text, "(wa)", 1),
            file_uri(&tb_path),
            wa_decl.clone(),
        ),
    ] {
        let response = client
            .request(
                "textDocument/definition",
                json!({
                    "textDocument": { "uri": tb_uri },
                    "position": position
                }),
            )
            .unwrap_or_else(|error| panic!("definition request at {what}: {error}"));
        let (uri, start) = single_location(&response, &format!("parse-mode definition at {what}"));
        assert_eq!(uri, want_uri, "{what}: wrong target file");
        assert_eq!(start, want_start, "{what}: {start} vs {want_start}");
        assert_no_shadow_uris(&response);
    }

    // The broken sibling keeps the root at outcome=parse: the trailing dump
    // summary proves this test really exercised the fallback path.
    let summary = lines.last().cloned().unwrap_or_default();
    assert!(
        summary.contains("outcome=parse"),
        "expected the parse-fallback outcome in the summary: {summary}"
    );
    client.shutdown();
}
