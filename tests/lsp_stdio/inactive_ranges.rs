//! Inactive ranges.

use super::*;

// ── llg/inactiveRanges ─────────────────────────────────────────────────────
//
// The custom inactive-range request serves the line ranges a preprocessor
// SKIPS for one document under the owner root's effective `[compile]
// defines`.  Acceptance surface: exact ranges over the staged open buffer,
// empty answer for unowned documents, and a define flip through the watched
// `[compile] defines` hot-reload path WITHOUT any server restart or buffer
// change.  Uses a self-contained generated workspace so the shared fixture
// tree stays untouched.

#[test]
fn lsp_stdio_inactive_ranges_follow_effective_defines() {
    const SOURCE: &str = "\
module m;
`ifdef FEATURE
  logic on;
`else
  logic off;
`endif
endmodule
";
    let (guard, ws) = rename_workspace("inactive-ranges", &[("top.sv", SOURCE)]);
    let path = ws.join("top.sv");
    let uri = file_uri(&path);

    let mut client = LspProcess::spawn(&ws);
    client
        .initialize(&[("inactive-ws", &ws)], default_init_options())
        .expect("initialize inactive-ranges workspace");
    client
        .open(&path, SOURCE)
        .expect("open inactive-ranges source");
    wait_for_diagnostics(&mut client, &uri, has_no_severity_1);

    // FEATURE is undefined: exactly the `ifdef branch (directive lines
    // included) is skipped; the taken `else branch and `endif stay visible.
    let initial = client
        .request("llg/inactiveRanges", json!({ "uri": uri }))
        .expect("llg/inactiveRanges request");
    assert_eq!(
        initial.get("ranges"),
        Some(&json!([{ "startLine": 1, "endLine": 2 }])),
        "unexpected inactive ranges while FEATURE is undefined: {initial}"
    );

    // An unowned document answers an EMPTY range list instead of failing.
    let outside = std::env::temp_dir().join("llg-inactive-ranges-outside-root.sv");
    let missing = client
        .request("llg/inactiveRanges", json!({ "uri": file_uri(&outside) }))
        .expect("llg/inactiveRanges request for unowned document");
    assert_eq!(
        missing.get("ranges"),
        Some(&json!([])),
        "unowned document must yield no ranges: {missing}"
    );

    // Toggle the effective define via the watched llg.toml ([compile]
    // defines hot reload): the SAME open buffer flips to the complementary
    // ranges with no didChange/didSave in between and no restart.
    fs::write(
        ws.join(CONFIG_FILE),
        "schema_version = 1\n\
         [sources]\n\
         directories = [\".\"]\n\
         include = [\"**/*.v\", \"**/*.sv\"]\n\
         [compile]\n\
         defines = [\"FEATURE\"]\n\
         [lint]\n\
         enabled = false\n",
    )
    .expect("write config enabling FEATURE");
    client
        .send_watch_event(&ws.join(CONFIG_FILE), 2)
        .expect("send config watch event");

    let deadline = Instant::now() + POLL_TIMEOUT;
    loop {
        if Instant::now() > deadline {
            panic!("inactive ranges never flipped after the defines hot reload");
        }
        thread::sleep(POLL_INTERVAL);
        let flipped = client
            .request_with_timeout(
                "llg/inactiveRanges",
                json!({ "uri": uri }),
                Duration::from_secs(5),
            )
            .expect("llg/inactiveRanges request during reload");
        if flipped.get("ranges") == Some(&json!([{ "startLine": 3, "endLine": 5 }])) {
            break;
        }
    }

    client.shutdown();
    drop(guard);
}
