//! Editing one construct into a broken state and back keeps the rest of the
//! workspace analysed.

use super::*;

const LEAF: &str = concat!(
    "// llg-lsp-fixture: generated/broken-edits/leaf.sv\n",
    "module leaf(input logic a, output logic y);\n",
    "  logic never_read;\n",
    "  assign y = a;\n",
    "  initial never_read = a;\n",
    "endmodule\n",
);

const TOP: &str = concat!(
    "// llg-lsp-fixture: generated/broken-edits/top.sv\n",
    "module top(input logic clk, input logic d, output logic q, output logic o);\n",
    "  logic t;\n",
    "  leaf u_leaf(.a(d), .y(t));\n",
    "  always @(posedge clk) q <= t;\n",
    "  assign o = t;\n",
    "endmodule\n",
);

/// Edits of `TOP` that leave an invalid timing control or driver behind. Each
/// used to fail the semantic database of the whole root.
const BROKEN: &[(&str, &str)] = &[
    ("posedge clk", "ghost"),
    ("posedge clk", "posedge clk or ghost"),
    ("posedge clk", "posedge clk iff ghost"),
    ("@(posedge clk)", "#ghost"),
    ("assign o = t", "assign #ghost o = t"),
    ("assign o = t", "assign o t"),
];

fn token_positions(client: &mut LspProcess, uri: &str) -> Vec<(u64, u64)> {
    let response = client
        .request(
            "textDocument/semanticTokens/full",
            json!({ "textDocument": { "uri": uri } }),
        )
        .expect("semantic tokens");
    semantic_token_positions(&response)
}

fn definition(client: &mut LspProcess, uri: &str, at: (u64, u64)) -> (String, Value) {
    let response = client
        .request(
            "textDocument/definition",
            json!({
                "textDocument": { "uri": uri },
                "position": { "line": at.0, "character": at.1 }
            }),
        )
        .expect("definition");
    single_location(&response, "definition")
}

/// The unused-signal finding of `leaf.sv`, which only a complete analysis
/// publishes, and no database failure.
fn is_complete_leaf_publication(params: &Value) -> bool {
    params["diagnostics"].as_array().is_some_and(|diagnostics| {
        diagnostics.iter().any(|diagnostic| {
            diagnostic["source"] == "llg-lint"
                && diagnostic["message"]
                    .as_str()
                    .is_some_and(|message| message.contains("`never_read`"))
        })
    })
}

fn assert_navigation(client: &mut LspProcess, leaf_uri: &str, top_uri: &str, text: &str) {
    let start = |source: &str, needle: &str, occurrence: usize| {
        let (line, character) = position_of(source, needle, occurrence);
        json!({ "line": line, "character": character })
    };
    let module = position_of(text, "leaf u_leaf", 0);
    assert_eq!(
        definition(client, top_uri, module),
        (leaf_uri.to_owned(), start(LEAF, "leaf(", 0)),
        "{text}"
    );
    let label = position_of(text, ".y(t)", 0);
    assert_eq!(
        definition(client, top_uri, (label.0, label.1 + 1)),
        (leaf_uri.to_owned(), start(LEAF, "y);", 0)),
        "{text}"
    );
    let actual = position_of(text, "(t)", 0);
    assert_eq!(
        definition(client, top_uri, (actual.0, actual.1 + 1)),
        (top_uri.to_owned(), start(text, "t;", 0)),
        "{text}"
    );
    let tokens = token_positions(client, top_uri);
    for needle in ["clk, input", "t;", "leaf u_leaf", "u_leaf", "d), .y"] {
        assert!(
            tokens.contains(&position_of(text, needle, 0)),
            "no token at `{needle}`: {text}"
        );
    }
    let tokens = token_positions(client, leaf_uri);
    for (needle, occurrence) in [("leaf(", 0), ("never_read", 0), ("never_read", 1)] {
        assert!(
            tokens.contains(&position_of(LEAF, needle, occurrence)),
            "leaf lost `{needle}`: {text}"
        );
    }
}

#[test]
fn lsp_stdio_broken_edits_keep_the_rest_of_the_workspace_analysed() {
    for (from, to) in BROKEN {
        let broken = TOP.replacen(from, to, 1);
        assert_ne!(&broken, TOP, "{from}");
        let id = NEXT_FIXTURE_ID.fetch_add(1, Ordering::Relaxed);
        let base =
            std::env::temp_dir().join(format!("llg-lsp-broken-edits-{}-{id}", std::process::id()));
        let root = base.join("workspace");
        fs::create_dir_all(&root).expect("create workspace");
        let _cleanup = TempDirCleanup(base);
        fs::write(
            root.join(CONFIG_FILE),
            "schema_version = 1\n[sources]\ndirectories = [\".\"]\n",
        )
        .expect("write config");
        let (leaf_path, top_path) = (root.join("leaf.sv"), root.join("top.sv"));
        fs::write(&leaf_path, LEAF).expect("write leaf.sv");
        fs::write(&top_path, &broken).expect("write top.sv");
        let (leaf_uri, top_uri) = (file_uri(&leaf_path), file_uri(&top_path));

        // The first analysis is already the broken one: nothing is served
        // from an earlier complete snapshot.
        let mut client = LspProcess::spawn(&root);
        client
            .initialize(&[("broken-edits", &root)], default_init_options())
            .expect("initialize workspace");
        client.open(&top_path, &broken).expect("open top.sv");
        wait_for_diagnostics(&mut client, &leaf_uri, is_complete_leaf_publication);
        let published = wait_for_diagnostics(&mut client, &top_uri, has_severity_1);
        assert!(
            !published.to_string().contains("semantic database"),
            "{published}"
        );
        assert_navigation(&mut client, &leaf_uri, &top_uri, &broken);

        client.change(&top_path, 2, TOP).expect("repair top.sv");
        wait_for_diagnostics(&mut client, &top_uri, has_no_severity_1);
        assert_navigation(&mut client, &leaf_uri, &top_uri, TOP);

        client.change(&top_path, 3, &broken).expect("break top.sv");
        let published = wait_for_diagnostics(&mut client, &top_uri, has_severity_1);
        assert!(
            !published.to_string().contains("semantic database"),
            "{published}"
        );
        assert_navigation(&mut client, &leaf_uri, &top_uri, &broken);
        client.shutdown();
    }
}
