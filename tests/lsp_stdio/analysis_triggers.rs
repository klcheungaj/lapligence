//! Which notifications start an analysis and which cancel one
//! (`docs/config.md`, "LSP analysis snapshots"), read from the lifecycle log.

use super::rename::TempDirCleanup;
use super::*;

const CONFIG: &str = "schema_version = 1\n[sources]\ndirectories = [\".\"]\n\
                      include = [\"**/*.sv\"]\n[lint]\nenabled = false\n";

const CREATED: u8 = 1;
const CHANGED: u8 = 2;
const DELETED: u8 = 3;

/// The server's `MAX_CONSECUTIVE_CANCELLATIONS`.
const MAX_CONSECUTIVE_CANCELLATIONS: usize = 3;

struct Session {
    client: LspProcess,
    root: PathBuf,
    log: PathBuf,
    version: i64,
    _cleanup: TempDirCleanup,
}

impl Session {
    /// A server over a root of `files`, before its first analysis.
    fn start(name: &str, files: &[(&str, &str)]) -> Self {
        let base = std::env::temp_dir().join(format!("llg-lsp-{name}-{}", std::process::id()));
        let cleanup = TempDirCleanup(base.clone());
        let root = base.join("ws");
        fs::create_dir_all(&root).expect("create the workspace");
        fs::write(root.join(CONFIG_FILE), CONFIG).expect("write the configuration");
        for (file, text) in files {
            fs::write(root.join(file), text).expect("write a source");
        }
        let log = base.join("server.log");
        let mut client = LspProcess::spawn_configured(&root, |command| {
            command.env("LLG_LOG", "debug").env("LLG_LOG_FILE", &log);
        });
        client
            .initialize_with_token_refresh(&[(name, &root)], default_init_options())
            .expect("initialize");
        Self {
            client,
            root,
            log,
            version: 1,
            _cleanup: cleanup,
        }
    }

    /// [`Self::start`], after the first analysis.
    fn ready(name: &str, files: &[(&str, &str)]) -> Self {
        let mut session = Self::start(name, files);
        session.client.wait_ready().expect("the first analysis");
        session.wait_outcomes(1);
        session
    }

    fn path(&self, file: &str) -> PathBuf {
        self.root.join(file)
    }

    fn uri(&self, file: &str) -> String {
        file_uri(&self.path(file))
    }

    fn logged(&self, needles: &[&str]) -> Vec<String> {
        fs::read_to_string(&self.log)
            .expect("read the log")
            .lines()
            .filter(|line| needles.iter().all(|needle| line.contains(needle)))
            .map(str::to_owned)
            .collect()
    }

    /// Wait for `count` lines, answering the server's requests meanwhile.
    fn wait_logged(&mut self, needles: &[&str], count: usize) -> Vec<String> {
        let deadline = Instant::now() + POLL_TIMEOUT;
        loop {
            let lines = self.logged(needles);
            if lines.len() >= count {
                return lines;
            }
            assert!(
                Instant::now() < deadline,
                "{needles:?}: {} of {count} lines",
                lines.len()
            );
            let soon = Instant::now() + Duration::from_millis(5);
            match self.client.receive_until(soon) {
                Ok(message) => self.client.route_unsolicited(message).expect("route"),
                Err(error) if error.starts_with("timed out") => {}
                // The server ended.
                Err(_) => thread::sleep(Duration::from_millis(5)),
            }
        }
    }

    /// The analyses that reached the frontend or an earlier step of their job.
    fn begun(&self) -> usize {
        self.logged(&["event=root_job.begin "]).len()
    }

    /// Wait until the `count`th analysis runs.
    fn wait_begun(&mut self, count: usize) {
        self.wait_logged(&["event=slang.compile.begin"], count);
    }

    /// The outcomes of the first `count` analyses, once they ended.
    fn wait_outcomes(&mut self, count: usize) -> Vec<String> {
        self.wait_logged(&["event=root_job.end "], count)
            .iter()
            .map(|line| field(line, "outcome=").to_owned())
            .collect()
    }

    /// Send a notification and wait until its handler has finished; what it
    /// decided.
    fn notify(&mut self, method: &str, params: Value) -> String {
        let name = format!("name={method} ");
        let needles = ["lifecycle=end kind=notification ", &name];
        let before = self.logged(&needles).len();
        self.client
            .send_notification(method, params)
            .expect("send the notification");
        let lines = self.wait_logged(&needles, before + 1);
        field(&lines[before], "outcome=").to_owned()
    }

    fn document(&self, file: &str) -> Value {
        json!({ "textDocument": { "uri": self.uri(file) } })
    }

    fn open(&mut self, file: &str, text: &str) -> String {
        let uri = self.uri(file);
        self.notify(
            "textDocument/didOpen",
            json!({ "textDocument": {
                "uri": uri, "languageId": "systemverilog", "version": 1, "text": text
            } }),
        )
    }

    fn change(&mut self, file: &str, text: &str) -> String {
        self.version += 1;
        let uri = self.uri(file);
        let version = self.version;
        self.notify(
            "textDocument/didChange",
            json!({
                "textDocument": { "uri": uri, "version": version },
                "contentChanges": [{ "text": text }]
            }),
        )
    }

    /// The notification of a save, without a write of the file.
    fn save(&mut self, file: &str) -> String {
        self.notify("textDocument/didSave", self.document(file))
    }

    fn close(&mut self, file: &str) -> String {
        self.notify("textDocument/didClose", self.document(file))
    }

    fn event(&mut self, file: &str, kind: u8) -> String {
        let uri = self.uri(file);
        self.notify(
            "workspace/didChangeWatchedFiles",
            json!({ "changes": [{ "uri": uri, "type": kind }] }),
        )
    }

    fn write(&self, file: &str, text: &str) {
        fs::write(self.path(file), text).expect("write a source");
    }

    /// Every diagnostic message published for `file` so far.
    fn messages(&mut self, file: &str) -> Vec<String> {
        // A round trip, so that everything sent before is received.
        let query = json!({ "query": "no such symbol" });
        self.client
            .request("workspace/symbol", query)
            .expect("workspace/symbol");
        let uri = self.uri(file);
        self.client
            .notifications
            .iter()
            .filter(|message| {
                message["method"] == "textDocument/publishDiagnostics"
                    && message["params"]["uri"] == uri.as_str()
            })
            .flat_map(|message| {
                message["params"]["diagnostics"]
                    .as_array()
                    .into_iter()
                    .flatten()
            })
            .map(|diagnostic| {
                diagnostic["message"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned()
            })
            .collect()
    }

    /// What the server serves for `file`, without its cache counters.
    fn tokens(&mut self, file: &str) -> Vec<String> {
        let dump = self
            .client
            .request("llg/dumpTokens", json!({ "uri": self.uri(file) }))
            .expect("llg/dumpTokens");
        dump["lines"]
            .as_array()
            .expect("lines")
            .iter()
            .map(|line| line.as_str().unwrap_or_default().to_owned())
            .filter(|line| !line.starts_with("# request-cache"))
            .collect()
    }
}

/// The word after `key` in a log line.
fn field<'a>(line: &'a str, key: &str) -> &'a str {
    line.split_once(key)
        .and_then(|(_, rest)| rest.split_whitespace().next())
        .unwrap_or_else(|| panic!("no {key} in {line}"))
}

fn module(name: &str, body: &str) -> String {
    format!("module {name};\n{body}endmodule\n")
}

/// A design whose analysis takes long enough to be met halfway, even in an
/// optimized build.
fn slow_module() -> String {
    let body: String = (0..1500)
        .map(|n| {
            format!(
                "  logic [7:0] a{n}, b{n};\n  always_comb begin\n    if (a{n}[0]) b{n} = a{n} + 8'd{};\n    else b{n} = {{a{n}[3:0], a{n}[7:4]}} ^ a{n};\n  end\n",
                n % 256
            )
        })
        .collect();
    module("slow", &body)
}

/// A text that reports `marker` in one diagnostic.
fn broken(marker: &str) -> String {
    module("top", &format!("  assign {marker} = missing;\n"))
}

#[test]
fn lsp_stdio_an_analysis_starts_only_for_texts_the_newest_one_did_not_read() {
    let first = module("top", "  logic first;\n");
    let mut session = Session::ready("triggers", &[("top.sv", &first)]);
    assert_eq!(session.open("top.sv", &first), "covered");

    // An edit waits for its save, which starts one analysis however often the
    // write is reported.
    let second = module("top", "  logic second;\n");
    assert_eq!(session.change("top.sv", &second), "admitted");
    session.write("top.sv", &second);
    assert_eq!(session.save("top.sv"), "scheduled");
    assert_eq!(session.wait_outcomes(2)[1], "ok");
    assert_eq!(session.event("top.sv", CHANGED), "no-op");
    assert_eq!(session.save("top.sv"), "covered");

    // The same when the event comes first or while the analysis is to start.
    let third = module("top", "  logic third;\n");
    session.change("top.sv", &third);
    session.write("top.sv", &third);
    assert_eq!(session.event("top.sv", CHANGED), "ok");
    session.save("top.sv");
    session.wait_outcomes(3);

    // Edits that are thrown away were never read.
    assert_eq!(session.change("top.sv", &broken("thrown")), "admitted");
    assert_eq!(session.close("top.sv"), "covered");

    // Unsaved text that an editor restores is, and so is its loss.
    assert_eq!(session.open("top.sv", &broken("restored")), "scheduled");
    session.wait_outcomes(4);
    assert_eq!(session.close("top.sv"), "scheduled");
    session.wait_outcomes(5);

    // Closed files: the events of writes that change something.
    assert_eq!(session.event("top.sv", CHANGED), "no-op");
    session.write("top.sv", &module("top", "  logic written;\n"));
    assert_eq!(session.event("top.sv", CHANGED), "ok");
    session.wait_outcomes(6);
    session.write("added.sv", &module("added", ""));
    assert_eq!(session.event("added.sv", CREATED), "ok");
    session.wait_outcomes(7);
    assert_eq!(session.event("added.sv", CREATED), "no-op");
    fs::remove_file(session.path("added.sv")).expect("remove a source");
    assert_eq!(session.event("added.sv", DELETED), "ok");
    let outcomes = session.wait_outcomes(8);
    assert_eq!(session.event("added.sv", DELETED), "no-op");

    // The configuration.
    assert_eq!(session.event(CONFIG_FILE, CHANGED), "no-op");
    session.write(
        CONFIG_FILE,
        &CONFIG.replace("enabled = false", "enabled = true"),
    );
    assert_eq!(session.event(CONFIG_FILE, CHANGED), "ok");
    session.wait_outcomes(9);

    assert_eq!(session.begun(), 9);
    assert!(
        outcomes.iter().all(|outcome| outcome == "ok"),
        "{outcomes:?}"
    );
    session.client.shutdown();
}

#[test]
fn lsp_stdio_a_document_that_takes_an_external_write_over_is_analysed() {
    let first = module("top", "  logic first;\n");
    let mut session = Session::ready("reloads", &[("top.sv", &first)]);
    session.open("top.sv", &first);

    // The client loads the file after the event of its write.
    let second = module("top", "  logic second;\n");
    session.write("top.sv", &second);
    assert_eq!(session.event("top.sv", CHANGED), "no-op");
    assert_eq!(session.change("top.sv", &second), "reloaded");
    session.wait_outcomes(2);

    // Or before it: also what a client that announces no saves does.
    let third = module("top", "  logic third;\n");
    assert_eq!(session.change("top.sv", &third), "admitted");
    session.write("top.sv", &third);
    assert_eq!(session.event("top.sv", CHANGED), "ok");
    session.wait_outcomes(3);

    // The user keeps typing after a save, whose event arrives late.
    let fourth = module("top", "  logic fourth;\n");
    session.change("top.sv", &fourth);
    session.write("top.sv", &fourth);
    assert_eq!(session.save("top.sv"), "scheduled");
    session.wait_outcomes(4);
    assert_eq!(session.change("top.sv", &broken("typed")), "admitted");
    assert_eq!(session.event("top.sv", CHANGED), "no-op");
    assert_eq!(session.change("top.sv", &fourth), "admitted");
    assert_eq!(session.begun(), 4);
    session.client.shutdown();
}

#[test]
fn lsp_stdio_a_save_cancels_the_analysis_of_older_texts() {
    let slow = slow_module();
    let first = module("top", "  logic first;\n");
    let files = [("slow.sv", slow.as_str()), ("top.sv", first.as_str())];
    let mut session = Session::ready("cancels", &files);
    session.open("top.sv", &first);

    session.change("top.sv", &broken("superseded_name"));
    session.save("top.sv");
    session.wait_begun(2);
    session.change("top.sv", &broken("newest_name"));
    // An edit leaves the analysis alone; its answers stay those of the last
    // one that completed.
    assert!(session.logged(&["event=root_job.cancel "]).is_empty());
    assert_eq!(session.save("top.sv"), "scheduled");
    assert_eq!(session.wait_outcomes(3), ["ok", "cancelled", "ok"]);
    assert_eq!(session.logged(&["event=root_job.cancel "]).len(), 1);
    assert_eq!(
        session
            .logged(&["event=root_job.end outcome=cancelled"])
            .len(),
        1
    );

    let top = session.uri("top.sv");
    wait_for_diagnostics(&mut session.client, &top, has_severity_1);
    let published = session.messages("top.sv");
    assert!(
        !published
            .iter()
            .any(|message| message.contains("superseded_name")),
        "{published:?}"
    );
    assert!(
        !published.iter().any(|message| message.contains("cancel")),
        "{published:?}"
    );

    // What it serves is what a fresh server does.
    session.write("top.sv", &broken("newest_name"));
    let served: Vec<_> = ["top.sv", "slow.sv"]
        .map(|file| session.tokens(file))
        .into();
    session.client.shutdown();
    let mut fresh = Session::ready("cancels-fresh", &files);
    fresh.open("top.sv", &broken("newest_name"));
    fresh.wait_outcomes(2);
    for (file, served) in ["top.sv", "slow.sv"].into_iter().zip(served) {
        let fresh = fresh.tokens(file);
        let paths = |lines: &[String], root: &Path| -> Vec<String> {
            let root = root.to_string_lossy().into_owned();
            lines
                .iter()
                .map(|line| line.replace(&root, "ROOT"))
                .collect()
        };
        assert_eq!(
            paths(&served, &session.root),
            paths(&fresh, &fresh_root(&session.root)),
            "{file}"
        );
    }
    fresh.client.shutdown();
}

/// The root of the `cancels-fresh` session beside the one of `cancels`.
fn fresh_root(root: &Path) -> PathBuf {
    PathBuf::from(
        root.to_string_lossy()
            .replace("llg-lsp-cancels-", "llg-lsp-cancels-fresh-"),
    )
}

#[test]
fn lsp_stdio_continuous_saves_let_an_analysis_finish() {
    let slow = slow_module();
    let mut session = Session::ready("storm-of-saves", &[("slow.sv", &slow)]);
    session.open("slow.sv", &slow);
    let saves = MAX_CONSECUTIVE_CANCELLATIONS + 2;
    for save in 1..=saves {
        session.change("slow.sv", &format!("{slow}// {save}\n"));
        session.save("slow.sv");
        if save < saves {
            session.wait_begun(save + 1);
        }
    }
    let mut expected = vec!["ok"];
    expected.extend(["cancelled"; MAX_CONSECUTIVE_CANCELLATIONS]);
    expected.extend(["ok-follow-up", "ok"]);
    assert_eq!(session.wait_outcomes(expected.len()), expected);
    session.client.shutdown();
}

#[test]
fn lsp_stdio_closes_and_configurations_cancel_and_requests_do_not() {
    let slow = slow_module();
    let first = module("top", "  logic first;\n");
    let mut session = Session::ready("races", &[("slow.sv", &slow), ("top.sv", &first)]);
    session.open("top.sv", &first);

    // The close of a document whose unsaved text is being analysed.
    session.change("top.sv", &broken("unsaved"));
    session.save("top.sv");
    session.wait_begun(2);
    assert_eq!(session.close("top.sv"), "scheduled");
    session.wait_begun(3);
    assert_eq!(session.wait_outcomes(2)[1], "cancelled");

    // A cancelled request and the unused configuration notification leave the
    // analysis alone.
    let id = session.client.next_id;
    session.client.next_id += 1;
    session
        .client
        .send_message(json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "textDocument/semanticTokens/full",
            "params": session.document("slow.sv"),
        }))
        .expect("request tokens");
    session
        .client
        .send_notification("$/cancelRequest", json!({ "id": id }))
        .expect("cancel the request");
    assert_eq!(
        session.notify(
            "workspace/didChangeConfiguration",
            json!({ "settings": {} })
        ),
        "ignored"
    );
    assert_eq!(session.wait_outcomes(3)[2], "ok");

    // A configuration does not: the next analysis is the new one's.
    session.write("top.sv", &module("top", "  logic written;\n"));
    session.event("top.sv", CHANGED);
    session.wait_begun(4);
    session.write(
        CONFIG_FILE,
        &CONFIG.replace("enabled = false", "enabled = true"),
    );
    assert_eq!(session.event(CONFIG_FILE, CHANGED), "ok");
    assert_eq!(session.wait_outcomes(5)[3..], ["cancelled", "ok"]);

    // The removal of the root ends its analysis and starts no other.
    session.write("top.sv", &module("top", "  logic again;\n"));
    session.event("top.sv", CHANGED);
    session.wait_begun(6);
    let root = session.root.clone();
    session
        .client
        .send_workspace_folder_change(&[], &[("races", &root)])
        .expect("remove the root");
    assert_eq!(session.wait_outcomes(6)[5], "cancelled");
    session
        .client
        .request("workspace/symbol", json!({ "query": "slow" }))
        .expect("the server goes on");
    assert_eq!(session.begun(), 6);

    // Adding it again analyses it.
    session
        .client
        .send_workspace_folder_change(&[("races", &root)], &[])
        .expect("add the root");
    assert_eq!(session.wait_outcomes(7)[6], "ok");
    session.client.shutdown();
}

/// A rename changes the texts the analysis read, so it needs an analysis of the
/// current ones; a save starts it.
#[test]
fn lsp_stdio_a_rename_waits_for_the_save_and_its_analysis() {
    const CONTENT_MODIFIED: &str = "-32801";
    let slow = slow_module();
    let first = module("top", "  logic named;\n  assign named = 1'b0;\n");
    let files = [("slow.sv", slow.as_str()), ("top.sv", first.as_str())];
    let mut session = Session::ready("renames", &files);
    session.open("top.sv", &first);
    let use_of = |line: u32| {
        json!({
            "textDocument": { "uri": session.uri("top.sv") },
            "position": { "line": line, "character": 9 },
        })
    };
    let (old_use, new_use) = (use_of(2), use_of(3));
    let rename = |use_of: &Value| {
        let mut params = use_of.clone();
        params["newName"] = json!("renamed");
        params
    };
    let refused = |session: &mut Session, name: &Value| {
        for (method, params) in [
            ("textDocument/prepareRename", name.clone()),
            ("textDocument/rename", rename(name)),
        ] {
            let error = session.client.request(method, params).expect_err(method);
            assert!(error.contains(CONTENT_MODIFIED), "{error}");
            assert!(error.contains("save the edited files"), "{error}");
        }
    };
    // Both answer before any edit.
    let edit = session
        .client
        .request("textDocument/rename", rename(&old_use));
    let top = session.uri("top.sv");
    assert_eq!(
        edit.expect("rename")["changes"][&top]
            .as_array()
            .map(Vec::len),
        Some(2)
    );

    // Other requests answer where the text still has the name.
    let below = module(
        "top",
        "  logic named;\n  assign named = 1'b0;\n  logic other;\n",
    );
    session.change("top.sv", &below);
    refused(&mut session, &old_use);
    for (method, context) in [
        ("textDocument/hover", Value::Null),
        ("textDocument/definition", Value::Null),
        (
            "textDocument/references",
            json!({ "includeDeclaration": true }),
        ),
    ] {
        let mut params = old_use.clone();
        if !context.is_null() {
            params["context"] = context;
        }
        let answer = session.client.request(method, params).expect(method);
        assert!(
            !answer.is_null() && answer != json!([]),
            "{method}: {answer}"
        );
    }

    // A save whose analysis has not ended is not enough, its end is.
    let above = module(
        "top",
        "  logic other;\n  logic named;\n  assign named = 1'b0;\n",
    );
    session.change("top.sv", &above);
    assert_eq!(session.save("top.sv"), "scheduled");
    session.wait_begun(2);
    refused(&mut session, &new_use);
    session.wait_outcomes(2);
    let prepared = session
        .client
        .request("textDocument/prepareRename", new_use.clone());
    assert_eq!(prepared.expect("prepareRename")["placeholder"], "named");
    let edit = session
        .client
        .request("textDocument/rename", rename(&new_use))
        .expect("rename");
    let lines: std::collections::BTreeSet<_> = edit["changes"][&top]
        .as_array()
        .expect("edits")
        .iter()
        .map(|edit| edit["range"]["start"]["line"].as_u64())
        .collect();
    assert_eq!(lines.into_iter().collect::<Vec<_>>(), [Some(2), Some(3)]);
    session.client.shutdown();
}

/// One root owns the header and a second one includes it.
#[test]
fn lsp_stdio_a_saved_header_starts_every_root_that_reads_it() {
    const USER: &str = "module top;\n`include \"defs.svh\"\nendmodule\n";
    const READ_BY: &[&str] = &["of_the_owner", "of_the_second"];
    let header = |value: &str| {
        format!(
            "`ifdef SECOND\nassign of_the_second = {value};\n\
             `else\nassign of_the_owner = {value};\n`endif\n"
        )
    };
    let files = [("top.sv", USER), ("defs.svh", &header("1'b0"))];
    let mut session = Session::ready("headers", &files);
    let second = session.root.with_file_name("second");
    fs::create_dir_all(&second).expect("create the second root");
    let config = format!("{CONFIG}[compile]\ninclude_dirs = [\"../ws\"]\n");
    fs::write(second.join(CONFIG_FILE), config).expect("write the configuration");
    fs::write(
        second.join("user.sv"),
        format!("`define SECOND\n{USER}`undef SECOND\n"),
    )
    .expect("write a source");
    session
        .client
        .send_workspace_folder_change(&[("second", &second)], &[])
        .expect("add the second root");
    session.wait_outcomes(2);

    // What is published for the header is what every root says about it, also
    // when it is opened and nothing is analysed for that.
    let published = session.messages("defs.svh").len();
    assert_eq!(session.open("defs.svh", &header("1'b0")), "covered");
    let messages = session.messages("defs.svh").split_off(published);
    for name in READ_BY {
        assert!(
            messages.iter().any(|message| message.contains(name)),
            "{name}: {messages:?}"
        );
    }

    session.change("defs.svh", &header("1'b1"));
    session.write("defs.svh", &header("1'b1"));
    assert_eq!(session.save("defs.svh"), "scheduled");
    let outcomes = session.wait_outcomes(4);
    assert!(
        outcomes.iter().all(|outcome| outcome == "ok"),
        "{outcomes:?}"
    );
    let begun = session.logged(&["event=root_job.begin "]);
    for root in [&session.root, &second] {
        let root = root.to_string_lossy().into_owned();
        assert_eq!(
            begun.iter().filter(|line| line.contains(&root)).count(),
            2,
            "{root}: {begun:?}"
        );
    }
    assert_eq!(session.save("defs.svh"), "covered");
    assert_eq!(session.event("defs.svh", CHANGED), "no-op");
    assert_eq!(session.begun(), 4);
    session.client.shutdown();
}

/// Several times the design of the other tests.
fn slow_files() -> Vec<(String, String)> {
    (0..8)
        .map(|copy| {
            let name = format!("slow{copy}");
            (
                format!("{name}.sv"),
                slow_module().replace("module slow", &format!("module {name}")),
            )
        })
        .collect()
}

/// The end of the server does not wait for an analysis that takes longer than
/// these `bound`s.
#[test]
fn lsp_stdio_the_end_of_the_server_cancels_the_analysis() {
    let texts = slow_files();
    let files: Vec<(&str, &str)> = texts
        .iter()
        .map(|(file, text)| (file.as_str(), text.as_str()))
        .collect();
    let bound = Duration::from_secs(10);

    let mut session = Session::start("shutdown", &files);
    session.wait_begun(1);
    session
        .client
        .request("shutdown", Value::Null)
        .unwrap_or_default();
    session
        .client
        .send_notification("exit", Value::Null)
        .expect("exit");
    assert_eq!(session.client.wait_for_exit_code(bound), Some(0));
    let ended = session.logged(&["event=slang.compile.end "]);
    assert!(
        ended.iter().all(|line| line.contains("outcome=cancelled")),
        "{ended:?}"
    );

    let mut session = Session::start("exit", &files);
    session.wait_begun(1);
    session
        .client
        .send_notification("exit", Value::Null)
        .expect("exit");
    assert_eq!(session.client.wait_for_exit_code(bound), Some(1));

    let mut session = Session::start("eof", &files);
    session.wait_begun(1);
    session.client.close_stdin();
    assert_eq!(session.client.wait_for_exit_code(bound), Some(0));
}

/// The cleanup of `shutdown` and `exit` waits for the isolated parse of an
/// edited document, so it is cancelled.
#[test]
fn lsp_stdio_the_end_of_the_server_cancels_an_isolated_parse() {
    // Not more than the default `analysis.max_file_bytes` admits.
    let edited: String = slow_files()
        .into_iter()
        .take(2)
        .map(|(_, text)| text)
        .collect();
    for end in ["shutdown", "exit"] {
        let name = format!("isolated-{end}");
        let first = module("top", "");
        let mut session = Session::ready(&name, &[("top.sv", &first)]);
        session.open("top.sv", &first);
        session.change("top.sv", &edited);
        let document = session.document("top.sv");
        session
            .client
            .send_request("textDocument/semanticTokens/full", document)
            .expect("send the request");
        session.wait_logged(&["event=semantic_tokens.parse_lock.end "], 1);
        if end == "shutdown" {
            session
                .client
                .request("shutdown", Value::Null)
                .unwrap_or_default();
        }
        session
            .client
            .send_notification("exit", Value::Null)
            .expect("exit");
        let code = session.client.wait_for_exit_code(Duration::from_secs(20));
        assert_eq!(code, Some(i32::from(end == "exit")), "{end}");
        let parses = session.logged(&["event=semantic_tokens.open_parse.end "]);
        let outcomes: Vec<_> = parses.iter().map(|line| field(line, "outcome=")).collect();
        assert_eq!(outcomes, ["cancelled"], "{end}");
    }
}
