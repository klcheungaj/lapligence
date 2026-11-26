//! Source that nests as deep as `llg::stack::MAX_NESTING_DEPTH` allows, and
//! deeper.

use super::*;

/// A module whose blocks, operators and parentheses each nest `levels` deep.
fn nested(levels: usize) -> String {
    format!(
        "module deep(input logic [7:0] a, output logic [7:0] y, z, w);\n\
         \x20 always_comb y = a{};\n\
         \x20 always_comb z = {}a{};\n\
         \x20 always_comb {}w = a;{}\n\
         endmodule\n",
        " + a".repeat(levels),
        "(".repeat(levels),
        ")".repeat(levels),
        "begin ".repeat(levels),
        " end".repeat(levels),
    )
}

#[test]
fn lsp_stdio_deeply_nested_source_is_served_or_refused() {
    const USER: &str = "\
module user(input logic [7:0] i, output logic [7:0] o);
  deep u(.a(i), .y(o), .z(), .w());
endmodule
";
    let limit = usize::try_from(llg::stack::MAX_NESTING_DEPTH).unwrap();
    let deep = nested(limit - 16);
    let (_guard, ws) = rename_workspace("deep-nesting", &[("deep.sv", &deep), ("user.sv", USER)]);
    let path = ws.join("deep.sv");
    let uri = file_uri(&path);
    let user = ws.join("user.sv");

    let mut client = LspProcess::spawn(&ws);
    client
        .initialize(&[("deep-nesting", &ws)], default_init_options())
        .expect("initialize the workspace");
    client.open(&user, USER).expect("open the instantiation");
    client.open(&path, &deep).expect("open the nested source");
    wait_for_diagnostics(&mut client, &uri, has_no_severity_1);

    // A published state can be served a moment later.
    let wait_for_definition = |client: &mut LspProcess| {
        let deadline = Instant::now() + POLL_TIMEOUT;
        loop {
            let found = client
                .request(
                    "textDocument/definition",
                    json!({
                        "textDocument": { "uri": file_uri(&user) },
                        "position": position_at(USER, "deep u", 0),
                    }),
                )
                .expect("definition")
                .to_string();
            if found.contains("deep.sv") {
                return;
            }
            assert!(Instant::now() < deadline, "{found}");
            thread::sleep(POLL_INTERVAL);
        }
    };
    wait_for_definition(&mut client);
    let document = json!({ "textDocument": { "uri": uri } });
    let tokens = client
        .request("textDocument/semanticTokens/full", document.clone())
        .expect("semantic tokens");
    assert!(
        tokens["data"].as_array().expect("token data").len() > 5 * limit,
        "one token per operand of the chain"
    );
    let symbols = client
        .request("textDocument/documentSymbol", document)
        .expect("document symbols");
    assert_has_name(&symbols, "deep");
    let hover = client
        .request(
            "textDocument/hover",
            json!({
                "textDocument": { "uri": uri },
                "position": { "line": 1, "character": "  always_comb y = a + a + ".len() + 4 * (limit / 2) },
            }),
        )
        .expect("hover");
    assert!(hover.to_string().contains("input"), "{hover}");

    // Above the limit the source is refused where it exceeds it, and the
    // server goes on.
    client
        .change_and_save(&path, 2, &nested(limit + 16))
        .expect("nest deeper");
    let refused = wait_for_diagnostics(&mut client, &uri, has_severity_1);
    assert!(
        refused
            .to_string()
            .contains("language constructs are too deeply nested"),
        "{refused}"
    );
    client
        .change_and_save(&path, 3, &deep)
        .expect("restore the source");
    wait_for_diagnostics(&mut client, &uri, has_no_severity_1);
    wait_for_definition(&mut client);
    client.shutdown();
}

/// A class that specializes itself without end is an error where it names
/// itself, and the server serves the rest of the source.
#[test]
fn lsp_stdio_endless_specialization_is_reported() {
    const ENDLESS: &str = "\
class c #(int N = 1);
  c #(N + 1) v;
  c #(2 * N) w;
endclass
module user;
  c #(1) handle;
  int counted = 3;
  initial $display(counted, handle.v == null);
endmodule
";
    let ended = ENDLESS
        .replace("N + 1", "N > 0 ? N - 1 : 0")
        .replace("2 * N", "N / 2");
    let (_guard, ws) = rename_workspace("endless-specialization", &[("endless.sv", ENDLESS)]);
    let path = ws.join("endless.sv");
    let uri = file_uri(&path);
    let mut client = LspProcess::spawn(&ws);
    client
        .initialize(&[("endless-specialization", &ws)], default_init_options())
        .expect("initialize the workspace");
    client.open(&path, ENDLESS).expect("open the source");
    let reported = wait_for_diagnostics(&mut client, &uri, has_severity_1).to_string();
    assert!(
        reported.contains("potentially infinitely recursive specialization of class 'c'"),
        "{reported}"
    );
    let hover = client
        .request(
            "textDocument/hover",
            json!({
                "textDocument": { "uri": uri },
                "position": position_at(ENDLESS, "counted, ", 1),
            }),
        )
        .expect("hover");
    assert!(hover.to_string().contains("counted"), "{hover}");
    client
        .change_and_save(&path, 2, &ended)
        .expect("end the chain");
    wait_for_diagnostics(&mut client, &uri, has_no_severity_1);
    client.shutdown();
}
