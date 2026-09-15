//! Performance.

use super::*;

// ── Debounced, latest-wins analysis scheduling ───────────────────────────────
//
// Bursts of didOpen/didChange notifications must collapse into ONE debounced
// re-analysis per root, a running analysis must never be superseded (triggers
// during a run owe exactly one follow-up), and the final state must reflect
// the newest buffer.  Run counts are observed through `LLG_LOG_FILE` — the
// server's supported lifecycle-log interface (never stdout, which stays
// pure framed JSON-RPC per this suite's contract).

/// Spawn the server with lifecycle logging redirected to `log_file`.
fn spawn_with_log_file(cwd: &Path, log_file: &Path) -> LspProcess {
    LspProcess::spawn_configured(cwd, |command| {
        command
            .env("LLG_LOG", "debug")
            .env("LLG_LOG_FILE", log_file);
    })
}

fn count_log_lines(log_file: &Path, needle: &str) -> usize {
    fs::read_to_string(log_file)
        .expect("read LLG_LOG_FILE output")
        .lines()
        .filter(|line| line.contains(needle))
        .count()
}

#[test]
fn lsp_stdio_bursty_edits_debounce_into_bounded_fresh_analyses() {
    let base = std::env::temp_dir().join(format!("llg-lsp-debounce-{}", std::process::id()));
    let ws = base.join("ws");
    fs::create_dir_all(&ws).expect("create debounce workspace");
    fs::write(
        ws.join(CONFIG_FILE),
        "schema_version = 1\n\
         [sources]\n\
         directories = [\".\"]\n\
         include = [\"**/*.v\", \"**/*.sv\"]\n\
         [lint]\n\
         enabled = false\n",
    )
    .expect("write debounce config");
    let top_path = ws.join("top.sv");
    let top_uri = file_uri(&top_path);
    /// A fresh DECLARATION (not a comment) is the freshness oracle: only a
    /// committed analysis over the newest buffer can serve its name.
    const FINAL_MARKER: &str = "final_edit_marker_sig";
    fs::write(&top_path, "module top;\nendmodule\n").expect("write top.sv");

    let log_file = base.join("server.log");
    let mut client = spawn_with_log_file(&base, &log_file);
    client
        .initialize(&[("debounce-ws", &ws)], default_init_options())
        .expect("initialize debounce workspace");

    // Wait for the initial (debounced) analysis to commit.
    wait_for_diagnostics(&mut client, &top_uri, has_no_severity_1);
    let baseline_compiling = count_log_lines(&log_file, "job compiling");
    assert!(baseline_compiling >= 1, "initial analysis must run once");

    // Storm 1: ten rapid edits of the SAME document inside one debounce
    // window (~25ms apart << 300ms) — they must coalesce into ONE run whose
    // inputs are the latest buffer.
    for i in 0..10 {
        client
            .change(
                &top_path,
                100 + i,
                &format!("module top; // storm_a_{i}\nendmodule\n"),
            )
            .expect("send storm-a didChange");
        thread::sleep(Duration::from_millis(25));
    }

    // Storm 2: sustained editing at ~50ms for three seconds.  Every event
    // must either join an armed debounce window or only mark the root dirty;
    // none may create an immediately-racing job.
    for i in 0..40 {
        client
            .change(
                &top_path,
                1000 + i,
                &format!("module top; // storm_b_{i}\nendmodule\n"),
            )
            .expect("send storm-b didChange");
        thread::sleep(Duration::from_millis(50));
    }
    // Final edit: proves no lost updates once quiescence returns.
    client
        .change(
            &top_path,
            2000,
            &format!("module top;\n  logic {FINAL_MARKER};\nendmodule\n"),
        )
        .expect("send final didChange");

    // Quiescence: wait until documentSymbol serves content containing the
    // FINAL marker (the last scheduled run committed), then allow one extra
    // debounce window for any owed follow-up to land before counting.
    let deadline = Instant::now() + POLL_TIMEOUT;
    loop {
        let symbols = client
            .request(
                "textDocument/documentSymbol",
                json!({ "textDocument": { "uri": top_uri } }),
            )
            .expect("documentSymbol during quiescence poll");
        if symbols.to_string().contains(FINAL_MARKER) {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "final edit never reached the served analysis"
        );
        thread::sleep(POLL_INTERVAL);
    }
    thread::sleep(Duration::from_millis(500));

    let total_compiling = count_log_lines(&log_file, "job compiling");
    let superseded = count_log_lines(&log_file, "job superseded");
    assert_eq!(
        superseded, 0,
        "no analysis job may race another into supersession"
    );
    // Bounded re-analyses: initial run + storms.  Every run costs at least
    // one debounce window plus compile time, so sustained editing cannot
    // produce anywhere near one-run-per-event (50 events would exceed 12
    // under the old trigger-per-job scheduling).
    assert!(
        total_compiling <= baseline_compiling + 12,
        "runaway parsing: {} analyses for ~51 events (baseline {baseline_compiling})",
        total_compiling
    );

    client.shutdown();
    let _ = fs::remove_dir_all(&base);
}

// ── Idle-time self-triggering (shadow-write feedback loop) ────────────────────
//
// The compile/commit path stages open buffers and include deps into a private
// shadow tree under the OS temp dir.  When that temp dir resolves into a
// registered watcher glob (e.g. `TMPDIR` symlinked into the workspace, or a
// source/include dir covering the temp dir), VS Code delivers watched-file
// events for the staged `.sv` copies.  If the server reacted to its own
// staging writes it would reschedule the root from every staged write,
// re-stage on the next run, and loop forever at idle (the high-CPU regression).
//
// The server must IGNORE watched-file events that originate under its own
// shadow base — including paths that only match after symlink resolution —
// so an idle server stays idle regardless of where TMPDIR points.

/// Spawn the server with lifecycle logging redirected to `log_file` and the
/// process TMPDIR set to `tmpdir` (so the shadow base nests under the watched
/// globs of a workspace, reproducing the self-write feedback layout).
fn spawn_with_log_file_and_tmpdir(cwd: &Path, log_file: &Path, tmpdir: &Path) -> LspProcess {
    LspProcess::spawn_configured(cwd, |command| {
        command
            .env("LLG_LOG", "debug")
            .env("LLG_LOG_FILE", log_file)
            .env("TMPDIR", tmpdir);
    })
}

/// The deterministic shadow base dir the running server created under
/// `real_tmp` (`llg-<pid>-<rand>`), found by listing the temp tree.  `None`
/// when the server has not yet staged anything.
fn shadow_base_dir(real_tmp: &Path) -> Option<PathBuf> {
    fs::read_dir(real_tmp)
        .ok()?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .find(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("llg-"))
        })
}

#[test]
fn lsp_stdio_ignores_own_shadow_writes_so_idle_stays_idle() {
    // Build a workspace whose shadow base falls under the registered watcher
    // globs: TMPDIR is a symlink (`link.tmp`) into the workspace, resolving to
    // `real.tmp`.  The server records its base as `link.tmp/llg-...`, but the
    // files physically land (and VS Code observes them) under the resolved
    // `real.tmp/llg-...` path — the exact canonicalization mismatch that used
    // to let the server's own writes escape the shadow guard and reschedule it.
    let base = std::env::temp_dir().join(format!("llg-lsp-shadowloop-{}", std::process::id()));
    let ws = base.join("ws");
    // The shadow base is rooted INSIDE the workspace (so its staged `.sv`
    // copies fall under the registered `<ws>/**/*.sv` watcher glob AND belong
    // to the root's ownership, exactly the production self-write layout).
    let real_tmp = ws.join("real.tmp");
    let link_tmp = ws.join("link.tmp");
    fs::create_dir_all(&ws).expect("create shadow-loop workspace");
    fs::create_dir_all(&real_tmp).expect("create real tmp dir");
    std::os::unix::fs::symlink(&real_tmp, &link_tmp).expect("create tmp symlink");
    fs::write(
        ws.join(CONFIG_FILE),
        "schema_version = 1\n\
         [sources]\n\
         directories = [\".\"]\n\
         include = [\"**/*.v\", \"**/*.sv\"]\n\
         exclude = [\"real.tmp/**\", \"link.tmp/**\"]\n\
         [lint]\n\
         enabled = false\n",
    )
    .expect("write shadow-loop llg.toml");
    let top_path = ws.join("top.sv");
    let top_uri = file_uri(&top_path);
    let top_text = "// llg-lsp-fixture:\n`include \"defs.svh\"\nmodule top;\nendmodule\n";
    fs::write(&top_path, top_text).expect("write top.sv");
    fs::write(
        ws.join("defs.svh"),
        "// llg-lsp-fixture:\n`define WIDTH 8\n",
    )
    .expect("write defs.svh");

    let log_file = base.join("server.log");
    let mut client = spawn_with_log_file_and_tmpdir(&base, &log_file, &link_tmp);
    client
        .initialize(&[("shadow-loop-ws", &ws)], default_init_options())
        .expect("initialize shadow-loop workspace");
    client.open(&top_path, top_text).expect("open top.sv");
    wait_for_diagnostics(&mut client, &top_uri, has_no_severity_1);

    // The server must have staged the buffer into a shadow dir under the
    // (symlinked) temp tree; if not, the scenario cannot be exercised.
    let shadow = shadow_base_dir(&real_tmp).unwrap_or_else(|| {
        panic!(
            "server never created a shadow base under {}",
            real_tmp.display()
        )
    });
    let staged_top = shadow.join(top_path.strip_prefix("/").expect("absolute top path"));
    assert!(
        fs::metadata(&staged_top).is_ok(),
        "expected a staged copy at {}",
        staged_top.display()
    );

    // Let the didOpen re-analysis (and any discovery rescans triggered by the
    // shadow copies landing under the source dir) fully settle before taking
    // the baseline, so later growth can only come from watched events.
    thread::sleep(Duration::from_secs(2));
    let baseline_compiling = count_log_lines(&log_file, "job compiling");
    assert!(baseline_compiling >= 1, "initial analysis must run once");
    thread::sleep(Duration::from_secs(2));
    assert_eq!(
        count_log_lines(&log_file, "job compiling"),
        baseline_compiling,
        "server must be quiescent before the shadow-event drive"
    );

    // VS Code, watching `<ws>/**/*.sv`, delivers events for the staged copy
    // at its RESOLVED path (real.tmp/llg-.../ws/top.sv) — which lexically
    // does not share the recorded base (link.tmp/llg-...).  Drive those
    // events repeatedly, exactly as the editor would while the server's own
    // staging churns, and assert the server never schedules a new analysis.
    let change_type = 2u8; // FileChangeType::Changed
    for _ in 0..5 {
        client
            .send_watch_event(&staged_top, change_type)
            .expect("send shadow watched event");
        thread::sleep(Duration::from_millis(50));
    }

    // Allow any (wrongful) reschedule to land: an analysis debounces ~300 ms
    // then compiles, so a couple of seconds far exceeds any owed run.
    thread::sleep(Duration::from_secs(2));
    let after_compiling = count_log_lines(&log_file, "job compiling");
    assert_eq!(
        after_compiling, baseline_compiling,
        "the server must ignore its own shadow writes: analysis count grew {} -> {} from self-triggering watched events",
        baseline_compiling, after_compiling
    );

    client.shutdown();
    let _ = fs::remove_dir_all(&base);
}

// ── Request memoization: repeat goto-definition serves from cache ────────────
//
// Contract (see `src/bin/llg_ls/request_cache.rs`):
// * an identical repeat request (same uri + position, unchanged buffer
//   content, unchanged analysis snapshot) returns the IDENTICAL result and is
//   observable as a request-cache HIT through the `# request-cache:` line the
//   `llg/dumpTokens` payload carries before its trailing `# analysis:` summary;
// * any buffer edit bumps the analysis epoch at commit time, so the next
//   request is recomputed (MISS) and reflects the new content;
// * an identical full-text `didChange` carries no information and must NOT
//   reschedule the root (no epoch bump), so repeats stay HITs.

pub(super) fn memo_cache_stats(client: &mut LspProcess, uri: &str) -> (u64, u64) {
    let result = client
        .request("llg/dumpTokens", json!({ "uri": uri }))
        .expect("llg/dumpTokens for cache stats");
    let lines = result
        .get("lines")
        .and_then(Value::as_array)
        .expect("dumpTokens result.lines array");
    let stats_line = lines
        .iter()
        .filter_map(|line| line.as_str())
        .find(|line| line.starts_with("# request-cache:"))
        .unwrap_or_else(|| {
            panic!("dumpTokens must carry a # request-cache: stats line: {lines:?}")
        });
    let parse = |marker: &str| -> u64 {
        stats_line
            .split_whitespace()
            .find_map(|part| part.strip_prefix(marker))
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or_else(|| panic!("unparseable cache stat {marker} in {stats_line}"))
    };
    (parse("hits="), parse("misses="))
}

fn memo_definition(client: &mut LspProcess, uri: &str, position: &Value) -> Value {
    client
        .request(
            "textDocument/definition",
            json!({ "textDocument": { "uri": uri }, "position": position }),
        )
        .expect("definition request")
}

/// Waits until the navigation-request miss counter grows past `baseline`,
/// which proves a commit bumped the analysis epoch (invalidation landed)
/// without depending on wall-clock timing or diagnostic payloads.
///
/// Each poll issues one probe request at `probe` because miss counters move
/// only when a memoized request runs against the NEW snapshot — stats reads
/// alone can never observe an epoch bump.
fn memo_wait_for_epoch_bump(client: &mut LspProcess, uri: &str, probe: &Value, baseline: u64) {
    let deadline = Instant::now() + POLL_TIMEOUT;
    loop {
        let _ = memo_definition(client, uri, probe);
        let (_, misses) = memo_cache_stats(client, uri);
        if misses > baseline {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "buffer edit never invalidated the request cache (misses stuck at {baseline})"
        );
        thread::sleep(POLL_INTERVAL);
    }
}

#[test]
fn lsp_stdio_repeats_identical_definition_requests_from_memo_cache() {
    let text = "module memo_mod;\n\
                \x20 logic clk;\n\
                \x20 assign clk = 1'b0;\n\
                endmodule\n";
    let (_cleanup, ws) = rename_workspace("memo", &[("memo_mod.sv", text)]);
    let path = ws.join("memo_mod.sv");
    let uri = file_uri(&path);

    let mut client = LspProcess::spawn(&ws);
    client
        .initialize(&[("memo-ws", &ws)], default_init_options())
        .expect("initialize memo workspace");
    client.open(&path, text).expect("open memo_mod");
    wait_for_diagnostics(&mut client, &uri, has_no_severity_1);

    let clk_decl_start = position_at(text, "logic clk", 6);
    let clk_use = position_at(text, "assign clk", 7);

    // Cold request computes; the identical repeat answers from the cache with
    // a byte-identical response.
    let cold = memo_definition(&mut client, &uri, &clk_use);
    let warm = memo_definition(&mut client, &uri, &clk_use);
    assert_eq!(
        warm, cold,
        "the identical repeat request must return the identical response"
    );
    let (target_uri, target_start) = single_location(&cold, "cold definition");
    assert_eq!(target_uri, uri, "definition must stay inside the document");
    assert_eq!(
        target_start, clk_decl_start,
        "use site must hit its declaration"
    );

    let (hits_cold, misses_cold) = memo_cache_stats(&mut client, &uri);
    assert!(
        hits_cold >= 1,
        "repeat request was not served from the cache"
    );
    assert!(misses_cold >= 1, "cold request was never computed");

    // A REAL buffer edit must invalidate: the next request observes the new
    // content (epoch bump proven by the growing miss counter, not by timing).
    // sig2 goes INSIDE the module (before endmodule) so its use site has an
    // enclosing scope to resolve against.
    let edited = text.replace(
        "endmodule\n",
        "\x20 logic sig2;\n\x20 assign sig2 = 1'b1;\nendmodule\n",
    );
    client.change(&path, 2, &edited).expect("edit adds sig2");
    memo_wait_for_epoch_bump(&mut client, &uri, &clk_use, misses_cold);

    let sig2_decl_start = position_at(&edited, "logic sig2", 6);
    let sig2_use = position_at(&edited, "assign sig2", 7);
    let sig2_target = memo_definition(&mut client, &uri, &sig2_use);
    let (_, sig2_start) = single_location(&sig2_target, "sig2 definition");
    assert_eq!(
        sig2_start, sig2_decl_start,
        "definition after the edit must reflect the NEW buffer content"
    );

    // Restoring the original text invalidates again; the same query as at the
    // start returns exactly the original answer (never a stale intermediate).
    client
        .change(&path, 3, text)
        .expect("restore original text");
    let (_, misses_before_restore) = memo_cache_stats(&mut client, &uri);
    memo_wait_for_epoch_bump(&mut client, &uri, &clk_use, misses_before_restore);
    let restored = memo_definition(&mut client, &uri, &clk_use);
    assert_eq!(
        restored, cold,
        "after restore, the original query must answer exactly like before"
    );

    // An identical full-text didChange carries no information: it must NOT
    // reschedule the root, so the epoch stays put and the repeat stays a HIT.
    // If suppression ever breaks, the scheduled re-run commits a new epoch and
    // the very next request shows up as a MISS instead.
    let (hits_before_noop, misses_before_noop) = memo_cache_stats(&mut client, &uri);
    client.change(&path, 4, text).expect("identical re-send");
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        let warm_noop = memo_definition(&mut client, &uri, &clk_use);
        let (hits_after_noop, misses_after_noop) = memo_cache_stats(&mut client, &uri);
        assert_eq!(warm_noop, cold, "response drifted without any input change");
        assert!(
            misses_after_noop == misses_before_noop,
            "identical full-text didChange rescheduled the root ({} -> {} misses)",
            misses_before_noop,
            misses_after_noop
        );
        if hits_after_noop > hits_before_noop || Instant::now() >= deadline {
            assert!(
                hits_after_noop > hits_before_noop,
                "repeats stopped hitting the cache entirely"
            );
            break;
        }
        thread::sleep(POLL_INTERVAL);
    }

    client.shutdown();
}
