//! Lifecycle.

use super::*;

#[test]
fn lsp_stdio_read_only_shadow_and_clean_shutdown() {
    // The server stages unsaved buffers under a private per-process temp
    // shadow and must never create a shadow tree inside the project.
    let fixture = FixtureTree::new();
    let root_a = fixture.root("root-a");
    let path = root_a.join("navigation").join("snapshot.sv");
    let valid = fs::read_to_string(&path).expect("read snapshot fixture");
    fn tree_entries(root: &Path) -> Vec<PathBuf> {
        fn visit(root: &Path, dir: &Path, entries: &mut Vec<PathBuf>) {
            for entry in fs::read_dir(dir).expect("read fixture tree") {
                let entry = entry.expect("fixture entry");
                let path = entry.path();
                entries.push(
                    path.strip_prefix(root)
                        .expect("entry below root")
                        .to_owned(),
                );
                if entry.file_type().expect("entry type").is_dir() {
                    visit(root, &path, entries);
                }
            }
        }

        let mut entries = Vec::new();
        visit(root, root, &mut entries);
        entries.sort();
        entries
    }
    let original_entries = tree_entries(fixture.base());
    let mut client = LspProcess::spawn(&fixture.root);
    client
        .initialize(&[("root-a", &root_a)], default_init_options())
        .expect("initialize read-only workspace");
    let uri = file_uri(&path);
    client.open(&path, &valid).expect("open snapshot source");
    wait_for_diagnostics(&mut client, &uri, has_no_severity_1);
    client.shutdown();

    assert!(
        !root_a.join("target").join("lsp-shadow").exists(),
        "project tree gained a target/lsp-shadow directory"
    );
    assert!(
        !root_a.join("llg").exists(),
        "project tree gained a stray directory"
    );

    assert_eq!(
        tree_entries(fixture.base()),
        original_entries,
        "frontend analysis changed the project tree"
    );
}

/// Full lifecycle contract: init → ready → shutdown → exit must terminate the
/// process within 10 s EVEN WITH STDIN HELD OPEN, and no
/// `<tmp>/llg-{child pid}-*` staging tree may survive (review A).  The
/// shutdown request uses the conventional `"params": null` wire shape real
/// clients send, which tower-lsp rejects with -32602 before reaching the
/// backend — cleanup must happen regardless.
#[test]
fn lsp_stdio_exit_terminates_promptly_and_removes_temp_shadow_tree() {
    let fixture = FixtureTree::new();
    let root_a = fixture.root("root-a");
    let mut client = LspProcess::spawn(&fixture.root);
    let pid_before_spawn = client.pid();

    let path = root_a.join("navigation").join("snapshot.sv");
    let valid = fs::read_to_string(&path).expect("read snapshot fixture");
    client
        .initialize(&[("root-a", &root_a)], default_init_options())
        .expect("initialize lifecycle workspace");

    // Stage an unsaved buffer so the per-process shadow tree definitely
    // exists mid-session.
    client.open(&path, &valid).expect("open snapshot source");
    wait_for_diagnostics(&mut client, &file_uri(&path), has_no_severity_1);
    let pid = client.pid();
    assert_eq!(pid, pid_before_spawn, "server pid changed unexpectedly");
    let staged = tmp_llg_shadow_dirs_for(pid);
    assert_eq!(
        staged.len(),
        1,
        "exactly one llg-{pid}-* shadow tree must exist mid-session: {staged:?}"
    );

    // Shutdown with `"params": null` (the shape VS Code's languageclient and
    // the Node E2E harness send) and drain until its response arrives.  The
    // response may be an error (-32602); the cleanup side effect must run
    // either way.
    let ack_id = json!(client.next_id);
    client.next_id += 1;
    client
        .send_message(json!({
            "jsonrpc": "2.0",
            "id": ack_id,
            "method": "shutdown",
            "params": Value::Null,
        }))
        .expect("send shutdown request");
    let deadline = Instant::now() + REQUEST_TIMEOUT;
    loop {
        if let Some(index) = client
            .orphan_responses
            .iter()
            .position(|message| message.get("id") == Some(&ack_id))
        {
            let response = client.orphan_responses.remove(index);
            // tower-lsp answers the `"params": null` shape with -32602
            // ("Unexpected params"); the cleanup side effect must run either
            // way, which the final leak scan below proves.
            assert!(
                response.get("error").is_some() || response.get("result").is_some(),
                "shutdown produced neither result nor error: {response}"
            );
            break;
        }
        let message = client
            .receive_until(deadline)
            .expect("shutdown response never arrived");
        client.route_unsolicited(message).expect("route message");
    }

    // Exit notification with stdin intentionally LEFT OPEN: the server must
    // terminate on `exit` itself instead of waiting for EOF.
    client
        .send_notification("exit", Value::Null)
        .expect("send exit notification");
    let code = client
        .wait_for_exit_code(Duration::from_secs(10))
        .unwrap_or_else(|| {
            panic!("server did not exit within 10s of shutdown+exit with stdin open")
        });
    assert_eq!(
        code, 0,
        "exit code after shutdown+exit must be 0 (LSP spec)"
    );

    let remaining = tmp_llg_shadow_dirs_for(pid);
    assert!(
        remaining.is_empty(),
        "no llg-{pid}-* shadow tree may remain after a clean exit: {remaining:?}"
    );
}

fn assert_exit_after_immediate_stdin_close(shutdown_first: bool) {
    let fixture = FixtureTree::new();
    let root_a = fixture.root("root-a");
    let mut client = LspProcess::spawn(&fixture.root);
    client
        .initialize(&[("root-a", &root_a)], default_init_options())
        .expect("initialize immediate-EOF lifecycle workspace");

    let path = root_a.join("navigation").join("snapshot.sv");
    let valid = fs::read_to_string(&path).expect("read snapshot fixture");
    client.open(&path, &valid).expect("open snapshot source");
    let uri = file_uri(&path);
    wait_for_diagnostics(&mut client, &uri, has_no_severity_1);

    let pid = client.pid();
    let staged = tmp_llg_shadow_dirs_for(pid);
    assert_eq!(
        staged.len(),
        1,
        "exactly one llg-{pid}-* shadow tree must exist before EOF exit: {staged:?}"
    );

    if shutdown_first {
        // Use the conventional params:null request shape. The wrapper records
        // shutdown before tower-lsp may reject that shape, which is the same
        // lifecycle path used by real clients.
        let _ = client.request("shutdown", Value::Null);
    }
    client
        .send_notification("exit", Value::Null)
        .expect("send exit notification before closing stdin");
    client.close_stdin();

    let code = client
        .wait_for_exit_code(Duration::from_secs(10))
        .unwrap_or_else(|| panic!("server did not exit after exit+immediate EOF"));
    assert_eq!(
        code,
        if shutdown_first { 0 } else { 1 },
        "exit+immediate EOF returned the wrong lifecycle status"
    );

    let remaining = tmp_llg_shadow_dirs_for(pid);
    assert!(
        remaining.is_empty(),
        "no llg-{pid}-* shadow tree may remain after exit+immediate EOF: {remaining:?}"
    );
}

#[test]
fn lsp_stdio_exit_and_immediate_stdin_close_without_shutdown_returns_one() {
    assert_exit_after_immediate_stdin_close(false);
}

#[test]
fn lsp_stdio_exit_and_immediate_stdin_close_after_shutdown_returns_zero() {
    assert_exit_after_immediate_stdin_close(true);
}

#[test]
fn lsp_stdio_shutdown_notification_does_not_authorize_zero_exit_status() {
    let fixture = FixtureTree::new();
    let root_a = fixture.root("root-a");
    let mut client = LspProcess::spawn(&fixture.root);
    client
        .initialize(&[("root-a", &root_a)], default_init_options())
        .expect("initialize notification-shaped shutdown workspace");

    client
        .send_notification("shutdown", Value::Null)
        .expect("send invalid shutdown notification");
    client
        .send_notification("exit", Value::Null)
        .expect("send exit notification");
    client.close_stdin();

    let code = client
        .wait_for_exit_code(Duration::from_secs(10))
        .unwrap_or_else(|| panic!("server did not exit after shutdown notification + exit"));
    assert_eq!(
        code, 1,
        "a notification-shaped shutdown must not count as a shutdown request"
    );
}
