//! Variable-shadowing navigation contract tests.
//!
//! SystemVerilog lets declarations in an inner scope shadow outer-scope
//! declarations of the same name.  These tests pin the navigation behavior
//! for each inner-scope kind: named `begin : blk … end` blocks, function and
//! task locals, named generate blocks, two-level nesting, nested module
//! instances' namespaces, a cross-file variant, and an opened buffer whose
//! inner block was added client-side.  Like `lsp_stdio.rs`, they speak only
//! framed LSP JSON-RPC over the spawned server's stdio; every test writes its
//! own inline temp workspace (no shared fixtures).
//!
//! Expectations that the current llg build did NOT satisfy used to be kept
//! as `#[ignore = "known gap: …"]` tests; the shadowing-resolution follow-up
//! closed those gaps, the ignores are gone, and the whole suite runs as
//! ordinary contract tests.
use std::fs;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(45);
/// Generous because the suite spawns one server per test and cargo runs them
/// in parallel: every server compiles behind Surelog's blocking frontend, so
/// a busy machine stretches each analysis well past the debounce window.
const POLL_TIMEOUT: Duration = Duration::from_secs(180);
const POLL_REQUEST_TIMEOUT: Duration = Duration::from_millis(750);
const POLL_INTERVAL: Duration = Duration::from_millis(100);
const READY_MESSAGE: &str = "llg Verilog/SystemVerilog language server ready";
const CONFIG_FILE_NAME: &str = "llg.toml";
const CONFIG_TEXT: &str = "schema_version = 1\n\n[sources]\ndirectories = [\".\"]\ninclude = [\"**/*.v\", \"**/*.sv\"]\n\n[lint]\nenabled = false\n";

// ── Inline temp workspace ────────────────────────────────────────────────────

/// A throwaway workspace root plus the base dir the spawned server inherits
/// as CWD.  Removed on drop.
struct Workspace {
    base: PathBuf,
}

impl Workspace {
    fn new(tag: &str) -> Self {
        let base = std::env::temp_dir().join(format!(
            "llg-shadowing-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock after epoch")
                .as_nanos()
        ));
        fs::create_dir_all(&base).expect("create shadowing workspace");
        fs::create_dir_all(base.join("ws")).expect("create shadowing analysis root");
        Self { base }
    }

    /// The single analysis root handed to the server.
    fn root(&self) -> PathBuf {
        self.base.join("ws")
    }

    fn write(&self, name: &str, text: &str) {
        let path = self.root().join(name);
        fs::write(&path, text).expect("write shadowing source");
    }
}

impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}

// ── Framed JSON-RPC transport ────────────────────────────────────────────────

type ReaderMessage = Result<Value, String>;

fn read_frame(reader: &mut BufReader<ChildStdout>) -> io::Result<Option<Value>> {
    let mut content_length = None;
    let mut line = String::new();
    loop {
        line.clear();
        let bytes = reader.read_line(&mut line)?;
        if bytes == 0 {
            if content_length.is_none() {
                return Ok(None);
            }
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "EOF while reading LSP headers",
            ));
        }
        if line == "\r\n" || line == "\n" {
            break;
        }
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "malformed LSP header"))?;
        if name.eq_ignore_ascii_case("Content-Length") {
            content_length = Some(value.trim().parse::<usize>().map_err(|error| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("invalid Content-Length: {error}"),
                )
            })?);
        }
    }

    let length = content_length.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "LSP message has no Content-Length",
        )
    })?;
    let mut body = vec![0; length];
    reader.read_exact(&mut body)?;
    serde_json::from_slice(&body)
        .map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("invalid LSP JSON: {error}"),
            )
        })
        .map(Some)
}

fn spawn_reader(stdout: ChildStdout) -> Receiver<ReaderMessage> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        loop {
            match read_frame(&mut reader) {
                Ok(Some(message)) => {
                    if sender.send(Ok(message)).is_err() {
                        return;
                    }
                }
                Ok(None) => {
                    let _ = sender.send(Err("LSP server stdout closed".to_owned()));
                    return;
                }
                Err(error) => {
                    let _ = sender.send(Err(error.to_string()));
                    return;
                }
            }
        }
    });
    receiver
}

struct LspProcess {
    child: Child,
    stdin: Option<ChildStdin>,
    incoming: Receiver<ReaderMessage>,
    next_id: u64,
    notifications: Vec<Value>,
    orphan_responses: Vec<Value>,
}

impl LspProcess {
    fn spawn(cwd: &Path) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_llg_ls"))
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("spawn llg LSP server");
        let stdout = child.stdout.take().expect("capture llg stdout");
        let stdin = child.stdin.take().expect("capture llg stdin");
        Self {
            child,
            stdin: Some(stdin),
            incoming: spawn_reader(stdout),
            next_id: 1,
            notifications: Vec::new(),
            orphan_responses: Vec::new(),
        }
    }

    fn send_message(&mut self, message: Value) -> Result<(), String> {
        let body = serde_json::to_vec(&message).map_err(|error| error.to_string())?;
        let stdin = self
            .stdin
            .as_mut()
            .ok_or_else(|| "LSP stdin is closed".to_owned())?;
        write!(stdin, "Content-Length: {}\r\n\r\n", body.len())
            .map_err(|error| error.to_string())?;
        stdin.write_all(&body).map_err(|error| error.to_string())?;
        stdin.flush().map_err(|error| error.to_string())
    }

    fn request_with_timeout(
        &mut self,
        method: &str,
        params: Value,
        timeout: Duration,
    ) -> Result<Value, String> {
        let id = json!(self.next_id);
        self.next_id += 1;
        self.send_message(json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        }))?;

        let deadline = Instant::now() + timeout;
        loop {
            if let Some(index) = self
                .orphan_responses
                .iter()
                .position(|message| message.get("id") == Some(&id))
            {
                let message = self.orphan_responses.remove(index);
                if let Some(error) = message.get("error") {
                    return Err(format!("LSP request {method} failed: {error}"));
                }
                return Ok(message.get("result").cloned().unwrap_or(Value::Null));
            }
            let message = self.receive_until(deadline)?;
            if message.get("id") == Some(&id) && message.get("method").is_none() {
                if let Some(error) = message.get("error") {
                    return Err(format!("LSP request {method} failed: {error}"));
                }
                return Ok(message.get("result").cloned().unwrap_or(Value::Null));
            }
            self.route_unsolicited(message)?;
        }
    }

    fn request(&mut self, method: &str, params: Value) -> Result<Value, String> {
        self.request_with_timeout(method, params, REQUEST_TIMEOUT)
    }

    fn send_notification(&mut self, method: &str, params: Value) -> Result<(), String> {
        self.send_message(json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        }))
    }

    fn receive_until(&self, deadline: Instant) -> Result<Value, String> {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err("timed out waiting for an LSP message".to_owned());
        }
        match self.incoming.recv_timeout(remaining) {
            Ok(Ok(message)) => Ok(message),
            Ok(Err(error)) => Err(error),
            Err(RecvTimeoutError::Timeout) => Err(format!(
                "timed out after {:?} waiting for an LSP message",
                remaining
            )),
            Err(RecvTimeoutError::Disconnected) => Err("LSP reader thread disconnected".to_owned()),
        }
    }

    fn route_unsolicited(&mut self, message: Value) -> Result<(), String> {
        if message.get("method").and_then(Value::as_str).is_some() {
            if let Some(id) = message.get("id").filter(|id| !id.is_null()).cloned() {
                // Answer server-originated requests (registerCapability) so
                // the server can proceed.
                self.send_message(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": Value::Null,
                }))?;
            } else {
                self.notifications.push(message);
            }
        } else {
            self.orphan_responses.push(message);
        }
        Ok(())
    }

    fn initialize(&mut self, root: &Path) -> Result<(), String> {
        self.request(
            "initialize",
            json!({
                "processId": std::process::id(),
                "rootUri": Value::Null,
                "workspaceFolders": [{ "uri": file_uri(root), "name": "ws" }],
                "initializationOptions": default_init_options(),
                "clientInfo": { "name": "llg-shadowing-tests", "version": "1" },
                "capabilities": {
                    "workspace": {
                        "configuration": true,
                        "workspaceFolders": true,
                        "didChangeWatchedFiles": { "dynamicRegistration": false },
                        "didChangeWorkspaceFolders": { "dynamicRegistration": false }
                    },
                    "textDocument": {
                        "publishDiagnostics": { "relatedInformation": true },
                        "semanticTokens": { "dynamicRegistration": false, "requests": { "range": true, "full": true }, "tokenTypes": [], "tokenModifiers": [], "formats": ["relative"] }
                    }
                }
            }),
        )?;
        self.send_notification("initialized", json!({}))?;
        let deadline = Instant::now() + REQUEST_TIMEOUT;
        loop {
            let message = self.receive_until(deadline)?;
            if message.get("method").and_then(Value::as_str) == Some("window/logMessage")
                && message.pointer("/params/message").and_then(Value::as_str) == Some(READY_MESSAGE)
            {
                return Ok(());
            }
            self.route_unsolicited(message)?;
        }
    }

    fn open(&mut self, path: &Path, text: &str) -> Result<(), String> {
        self.send_notification(
            "textDocument/didOpen",
            json!({
                "textDocument": {
                    "uri": file_uri(path),
                    "languageId": "systemverilog",
                    "version": 1,
                    "text": text
                }
            }),
        )
    }

    fn change(&mut self, path: &Path, version: i64, text: &str) -> Result<(), String> {
        self.send_notification(
            "textDocument/didChange",
            json!({
                "textDocument": { "uri": file_uri(path), "version": version },
                "contentChanges": [{ "text": text }]
            }),
        )
    }

    fn shutdown(&mut self) {
        let _ = self.request("shutdown", Value::Null);
        let _ = self.send_notification("exit", Value::Null);
        self.stdin.take();
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            match self.child.try_wait() {
                Ok(Some(_)) | Err(_) => return,
                Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
                Ok(None) => {
                    let _ = self.child.kill();
                    let _ = self.child.wait();
                    return;
                }
            }
        }
    }
}

impl Drop for LspProcess {
    fn drop(&mut self) {
        self.stdin.take();
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

// ── Small assertion / position helpers ───────────────────────────────────────

fn file_uri(path: &Path) -> String {
    let mut uri = String::from("file://");
    for byte in path.to_string_lossy().bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'-' | b'_' | b'.' | b'~' | b':') {
            uri.push(byte as char);
        } else {
            uri.push_str(&format!("%{byte:02X}"));
        }
    }
    uri
}

fn default_init_options() -> Value {
    json!({
        "llg": {
            "protocolVersion": 1,
            "configFiles": []
        }
    })
}

fn position_at(text: &str, needle: &str, offset: usize) -> Value {
    let byte_index = text.find(needle).expect("needle in fixture source") + offset;
    let line = text[..byte_index]
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count();
    let line_start = text[..byte_index].rfind('\n').map_or(0, |index| index + 1);
    let character = text[line_start..byte_index].chars().count();
    json!({ "line": line, "character": character })
}

/// One definition location extracted from a response the server must serve as
/// a SINGLE `Location` object (never an array, never null).
fn single_location(response: &Value, what: &str) -> (String, Value) {
    let object = response
        .as_object()
        .unwrap_or_else(|| panic!("{what} must be a single Location object: {response}"));
    let uri = object
        .get("uri")
        .and_then(Value::as_str)
        .expect("definition location URI")
        .to_owned();
    let start = object
        .get("range")
        .and_then(|range| range.get("start"))
        .cloned()
        .expect("definition location range start");
    (uri, start)
}

fn location_starts(response: &Value) -> Vec<Value> {
    response
        .as_array()
        .map(|locations| {
            locations
                .iter()
                .filter_map(|location| location.pointer("/range/start").cloned())
                .collect()
        })
        .unwrap_or_default()
}

fn assert_no_shadow_uris(value: &Value) {
    match value {
        Value::Object(object) => {
            if let Some(uri) = object.get("uri").and_then(Value::as_str) {
                assert!(
                    uri.starts_with("file:"),
                    "LSP location is not a file URI: {uri}"
                );
                let path = uri.trim_start_matches("file://");
                let leaked = Path::new(path).components().any(|component| {
                    let name = component.as_os_str().to_string_lossy();
                    name.starts_with("llg-")
                        && name.split('-').nth(1).is_some_and(|first| {
                            !first.is_empty() && first.bytes().all(|b| b.is_ascii_digit())
                        })
                });
                assert!(!leaked, "shadow URI leaked to client: {uri}");
            }
            for child in object.values() {
                assert_no_shadow_uris(child);
            }
        }
        Value::Array(array) => {
            for child in array {
                assert_no_shadow_uris(child);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

// ── Session helpers ──────────────────────────────────────────────────────────

/// Spawn a server over `ws`, initialize it, open `path` with `text`, then poll
/// `llg/dumpTokens` until at least one row carries `want_binding` (the
/// ~300 ms debounce plus compile latency mean early answers predate the
/// shadowing bindings).  Returns the final dump lines for extra oracle checks.
fn open_and_wait_for_binding(
    client: &mut LspProcess,
    ws: &Workspace,
    path_name: &str,
    text: &str,
    want_binding: &str,
) -> Vec<String> {
    let path = ws.root().join(path_name);
    let uri = file_uri(&path);
    client.open(&path, text).expect("open shadowing source");
    let deadline = Instant::now() + POLL_TIMEOUT;
    let mut interval = POLL_INTERVAL;
    loop {
        match client.request_with_timeout(
            "llg/dumpTokens",
            json!({ "uri": uri }),
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
                if lines.iter().any(|l| l.contains(want_binding)) {
                    break lines;
                }
            }
            Err(error) if error.starts_with("timed out") => {}
            Err(error) => panic!("llg/dumpTokens failed: {error}"),
        }
        assert!(
            Instant::now() < deadline,
            "binding `{want_binding}` never appeared in the dump"
        );
        thread::sleep(interval);
        interval = interval.saturating_mul(2).min(Duration::from_secs(1));
    }
}

fn definition_starts_at(
    client: &mut LspProcess,
    path: &Path,
    text: &str,
    needle: &str,
    offset: usize,
) -> (String, Value) {
    let response = client
        .request(
            "textDocument/definition",
            json!({
                "textDocument": { "uri": file_uri(path) },
                "position": position_at(text, needle, offset)
            }),
        )
        .unwrap_or_else(|error| panic!("definition request at {needle:?}: {error}"));
    assert_no_shadow_uris(&response);
    single_location(&response, &format!("definition at {needle:?}"))
}

fn hover_markup(
    client: &mut LspProcess,
    path: &Path,
    text: &str,
    needle: &str,
    offset: usize,
) -> String {
    let hover = client
        .request(
            "textDocument/hover",
            json!({
                "textDocument": { "uri": file_uri(path) },
                "position": position_at(text, needle, offset)
            }),
        )
        .unwrap_or_else(|error| panic!("hover request at {needle:?}: {error}"));
    let contents = hover
        .as_object()
        .and_then(|hover| hover.get("contents"))
        .unwrap_or_else(|| panic!("hover must carry contents at {needle:?}: {hover}"));
    contents
        .get("value")
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("hover markup value missing: {contents}"))
        .to_owned()
}

fn reference_starts(
    client: &mut LspProcess,
    path: &Path,
    text: &str,
    needle: &str,
    offset: usize,
    include_declaration: bool,
) -> Vec<Value> {
    let response = client
        .request(
            "textDocument/references",
            json!({
                "textDocument": { "uri": file_uri(path) },
                "position": position_at(text, needle, offset),
                "context": { "includeDeclaration": include_declaration }
            }),
        )
        .unwrap_or_else(|error| panic!("references request at {needle:?}: {error}"));
    assert_no_shadow_uris(&response);
    location_starts(&response)
}

// ── Shared design sources ────────────────────────────────────────────────────

/// Outer `[7:0] val` vs block-local `[3:0] val`; one use inside the named
/// block and one at module level.
const BLK_SV: &str = "\
module shadow_blk;
  logic [7:0] val;
  logic en;

  always_comb begin : blk
    logic [3:0] val;
    val = en ? 4'h1 : 4'h0;
  end

  assign val = 8'h00;
endmodule
";

/// Function-local `[3:0] dat` shadows module-level `[7:0] dat`; the call-site
/// argument must keep using the module-level signal.
const FUNC_SV: &str = "\
module shadow_func;
  logic [7:0] dat;
  logic [7:0] out;

  function automatic logic [3:0] f_sh(input logic [1:0] sel);
    logic [3:0] dat;
    dat = {2'b00, sel};
    return dat;
  endfunction

  assign out = {4'h0, f_sh(dat[1:0])};
endmodule
";

/// Task-local `[15:0] cap` shadows module-level `[7:0] cap`.
const TASK_SV: &str = "\
module shadow_task;
  logic [7:0] cap;
  logic go;

  task automatic t_sh(input logic [3:0] arg);
    logic [15:0] cap;
    cap = {12'h0, arg};
  endtask

  initial begin
    t_sh(go ? 4'h1 : 4'h0);
    cap = 8'h00;
  end
endmodule
";

/// Generate-block-local `[3:0] val` shadows module-level `[7:0] val`; uses
/// before AND after the generate stay on the outer declaration.
const GEN_SV: &str = "\
module shadow_gen;
  logic [7:0] val;
  logic [7:0] pre;
  logic [7:0] post;

  assign pre = val;

  genvar g;
  generate
    for (g = 0; g < 2; g = g + 1) begin : gen_blk
      logic [3:0] val;
      assign val = 4'h0;
    end
  endgenerate

  assign post = val;
endmodule
";

/// Two-level nesting: module `[15:0] sig` ← outer_blk `[7:0] sig` ← inner_blk
/// `[3:0] sig`.  Each use site belongs to its own level.
const TWO_SV: &str = "\
module shadow_two;
  logic [15:0] sig;
  logic en;

  always_comb begin : outer_blk
    logic [7:0] sig;
    begin : inner_blk
      logic [3:0] sig;
      sig = {3'b000, en};
    end
    sig = en ? 8'h01 : 8'h00;
  end

  assign sig = 16'h0000;
endmodule
";

const LEAF_SV: &str = "\
module leaf(input logic mode, output logic q);
  assign q = mode;
endmodule
";

/// tb and its child instance declare the SAME net name (`mode`): each side
/// must navigate inside its own instance namespace.  The tb also shadows
/// `mode` inside a named block.
const TB_NESTED_SV: &str = "\
module tb_nested;
  logic mode;
  logic q;

  leaf u0(.mode(mode), .q(q));

  always_comb begin : blk
    logic [3:0] mode;
    mode = 1'b0;
  end

  assign mode = 1'b1;
endmodule
";

const CROSS_LEAF_SV: &str = "\
module cross_leaf(input logic sel, output logic y);
  assign y = sel;
endmodule
";

/// Cross-file variant: `sel` is declared in this file AND in cross_leaf.sv's
/// port list; the block-local `sel` shadows only the local one.
const TB_CROSS_SV: &str = "\
module tb_cross;
  logic sel;
  logic y;

  cross_leaf u0(.sel(sel), .y(y));

  always_comb begin : blk
    logic [3:0] sel;
    sel = 1'b0;
  end

  assign sel = 1'b1;
endmodule
";

/// On-disk baseline WITHOUT the inner block; the test opens an edited buffer
/// that adds `logic [3:0] val;` inside the named block.
const EDIT_BASE_SV: &str = "\
module shadow_edit;
  logic [7:0] val;
  logic en;
  always_comb begin : blk
    val = en ? 4'hF : 4'h0;
  end
  assign val = 8'h00;
endmodule
";

const EDIT_BUFFER_SV: &str = "\
module shadow_edit;
  logic [7:0] val;
  logic en;
  always_comb begin : blk
    logic [3:0] val;
    val = en ? 4'hF : 4'h0;
  end
  assign val = 8'h00;
endmodule
";

// ── Contract tests ───────────────────────────────────────────────────────────

/// Named `begin : blk` block shadowing a module-level reg/wire: UHDM captures
/// elaboration bindings for both use sites, so goto-definition is
/// binding-precise — refs INSIDE the block reach the INNER declaration and
/// refs OUTSIDE keep using the OUTER declaration.  (A request on the inner
/// declaration itself is covered by `shadowing_declaration_resolves_to_itself`.)
#[test]
fn named_begin_block_shadow_resolves_inner_and_outer_definitions() {
    let ws = Workspace::new("blk");
    ws.write(CONFIG_FILE_NAME, CONFIG_TEXT);
    ws.write("blk.sv", BLK_SV);
    let path = ws.root().join("blk.sv");

    let mut client = LspProcess::spawn(&ws.base);
    client.initialize(&ws.root()).expect("initialize workspace");
    let lines = open_and_wait_for_binding(
        &mut client,
        &ws,
        "blk.sv",
        BLK_SV,
        "bind=blk.sv:5:16[val,var]",
    );
    assert!(
        lines
            .iter()
            .any(|l| l.contains("bind=blk.sv:1:14[val,net]")),
        "outer-use binding must also be captured: {lines:?}"
    );

    // Use INSIDE the block → the block-local declaration (5:16).
    let (uri, start) = definition_starts_at(&mut client, &path, BLK_SV, "val = en", 1);
    assert_eq!(uri, file_uri(&path), "inner use must stay in blk.sv");
    assert_eq!(
        start,
        position_at(BLK_SV, "[3:0] val", 6),
        "inner use → inner decl"
    );

    // Use OUTSIDE the block → the module-level declaration (1:14).
    let (uri, start) = definition_starts_at(&mut client, &path, BLK_SV, "assign val", 7);
    assert_eq!(uri, file_uri(&path));
    assert_eq!(
        start,
        position_at(BLK_SV, "[7:0] val", 6),
        "outer use → outer decl"
    );

    // Definition ON the OUTER declaration resolves to itself.
    let (uri, start) = definition_starts_at(&mut client, &path, BLK_SV, "[7:0] val", 6);
    assert_eq!(uri, file_uri(&path));
    assert_eq!(
        start,
        position_at(BLK_SV, "[7:0] val", 6),
        "outer decl → itself"
    );
    client.shutdown();
}

/// A request on the SHADOWING declaration itself must resolve to that
/// declaration, not fall through to the same-named outer declaration.
#[test]
fn shadowing_declaration_resolves_to_itself() {
    let ws = Workspace::new("blk-self");
    ws.write(CONFIG_FILE_NAME, CONFIG_TEXT);
    ws.write("blk.sv", BLK_SV);
    let path = ws.root().join("blk.sv");

    let mut client = LspProcess::spawn(&ws.base);
    client.initialize(&ws.root()).expect("initialize workspace");
    open_and_wait_for_binding(
        &mut client,
        &ws,
        "blk.sv",
        BLK_SV,
        "bind=blk.sv:5:16[val,var]",
    );

    let (uri, start) = definition_starts_at(&mut client, &path, BLK_SV, "[3:0] val", 6);
    assert_eq!(uri, file_uri(&path));
    assert_eq!(
        start,
        position_at(BLK_SV, "[3:0] val", 6),
        "inner decl → itself"
    );
    client.shutdown();
}

/// The unshadowed control variable in the same design navigates normally
/// even when a sibling name is shadowed (no collateral damage).
#[test]
fn unshadowed_sibling_variable_navigates_normally() {
    let ws = Workspace::new("blk-sibling");
    ws.write(CONFIG_FILE_NAME, CONFIG_TEXT);
    ws.write("blk.sv", BLK_SV);
    let path = ws.root().join("blk.sv");

    let mut client = LspProcess::spawn(&ws.base);
    client.initialize(&ws.root()).expect("initialize workspace");
    open_and_wait_for_binding(
        &mut client,
        &ws,
        "blk.sv",
        BLK_SV,
        "bind=blk.sv:2:8[en,net]",
    );

    // `en` is used both inside and outside the block and is never shadowed:
    // both uses resolve to the same module-level declaration.
    for (needle, offset) in [("en ?", 0), ("= en", 2)] {
        let (uri, start) = definition_starts_at(&mut client, &path, BLK_SV, needle, offset);
        assert_eq!(uri, file_uri(&path), "{needle:?} use must stay in blk.sv");
        assert_eq!(
            start,
            position_at(BLK_SV, "logic en", 6),
            "{needle:?} use → `logic en` decl"
        );
    }
    client.shutdown();
}

/// Hover inside the named block describes the INNER declaration's type
/// (`logic [3:0] val`), and hovering the block-local declaration itself does
/// not fall back to the module-level signal either.
#[test]
fn named_begin_block_hover_shows_the_shadowing_declaration_type() {
    let ws = Workspace::new("blk-hover");
    ws.write(CONFIG_FILE_NAME, CONFIG_TEXT);
    ws.write("blk.sv", BLK_SV);
    let path = ws.root().join("blk.sv");

    let mut client = LspProcess::spawn(&ws.base);
    client.initialize(&ws.root()).expect("initialize workspace");
    open_and_wait_for_binding(
        &mut client,
        &ws,
        "blk.sv",
        BLK_SV,
        "bind=blk.sv:5:16[val,var]",
    );

    let markup = hover_markup(&mut client, &path, BLK_SV, "val = en", 2);
    assert!(
        markup.contains("[3:0]"),
        "hover inside the block must show the INNER width: {markup:?}"
    );
    let markup = hover_markup(&mut client, &path, BLK_SV, "[3:0] val", 6);
    assert!(
        markup.contains("[3:0]"),
        "hover ON the block-local declaration must show its own width: {markup:?}"
    );
    let markup = hover_markup(&mut client, &path, BLK_SV, "assign val", 8);
    assert!(
        markup.contains("[7:0]"),
        "hover outside the block must keep the OUTER width: {markup:?}"
    );
    client.shutdown();
}

/// Find-references respect shadow scopes — the OUTER declaration's
/// references are exactly itself + the outer use, the INNER declaration's
/// references exactly itself + the inner use.
#[test]
fn named_begin_block_references_respect_shadow_scopes() {
    let ws = Workspace::new("blk-refs");
    ws.write(CONFIG_FILE_NAME, CONFIG_TEXT);
    ws.write("blk.sv", BLK_SV);
    let path = ws.root().join("blk.sv");

    let mut client = LspProcess::spawn(&ws.base);
    client.initialize(&ws.root()).expect("initialize workspace");
    open_and_wait_for_binding(
        &mut client,
        &ws,
        "blk.sv",
        BLK_SV,
        "bind=blk.sv:5:16[val,var]",
    );

    let inner_decl = position_at(BLK_SV, "[3:0] val", 6);
    let inner_use = position_at(BLK_SV, "val = en", 0);
    let outer_decl = position_at(BLK_SV, "[7:0] val", 6);
    let outer_use = position_at(BLK_SV, "assign val", 7);

    let mut starts = reference_starts(&mut client, &path, BLK_SV, "[3:0] val", 6, true);
    starts.sort_by_key(|start| (start["line"].as_u64(), start["character"].as_u64()));
    let mut expected = vec![inner_decl.clone(), inner_use.clone()];
    expected.sort_by_key(|start| (start["line"].as_u64(), start["character"].as_u64()));
    assert_eq!(starts, expected, "INNER decl refs = decl + inner use only");

    let mut starts = reference_starts(&mut client, &path, BLK_SV, "[7:0] val", 6, true);
    starts.sort_by_key(|start| (start["line"].as_u64(), start["character"].as_u64()));
    let mut expected = vec![outer_decl, outer_use];
    expected.sort_by_key(|start| (start["line"].as_u64(), start["character"].as_u64()));
    assert_eq!(starts, expected, "OUTER decl refs = decl + outer use only");
    client.shutdown();
}

/// Function-local variable shadowing a module signal: the call-site argument
/// keeps referring to the MODULE-level declaration (this direction already
/// works via the index fallback), and the function-body reads must resolve to
/// the LOCAL declaration (separate ignored expectation below).
#[test]
fn function_local_shadow_call_site_stays_on_module_signal() {
    let ws = Workspace::new("func");
    ws.write(CONFIG_FILE_NAME, CONFIG_TEXT);
    ws.write("func.sv", FUNC_SV);
    let path = ws.root().join("func.sv");

    let mut client = LspProcess::spawn(&ws.base);
    client.initialize(&ws.root()).expect("initialize workspace");
    open_and_wait_for_binding(
        &mut client,
        &ws,
        "func.sv",
        FUNC_SV,
        "bind=func.sv:2:14[out,net]",
    );

    // Call-site arg `dat[1:0]` inside f_sh(...) → the module-level `dat`.
    let (uri, start) = definition_starts_at(&mut client, &path, FUNC_SV, "f_sh(dat", 5);
    assert_eq!(uri, file_uri(&path));
    assert_eq!(
        start,
        position_at(FUNC_SV, "[7:0] dat", 6),
        "call-site arg → module-level dat"
    );
    client.shutdown();
}

/// Reads INSIDE the function body resolve to the function-local declaration.
#[test]
fn function_local_use_resolves_to_the_function_local_declaration() {
    let ws = Workspace::new("func-body");
    ws.write(CONFIG_FILE_NAME, CONFIG_TEXT);
    ws.write("func.sv", FUNC_SV);
    let path = ws.root().join("func.sv");

    let mut client = LspProcess::spawn(&ws.base);
    client.initialize(&ws.root()).expect("initialize workspace");
    open_and_wait_for_binding(
        &mut client,
        &ws,
        "func.sv",
        FUNC_SV,
        "bind=func.sv:2:14[out,net]",
    );

    for (needle, offset) in [("dat = {2'b00", 1), ("return dat", 8)] {
        let (uri, start) = definition_starts_at(&mut client, &path, FUNC_SV, needle, offset);
        assert_eq!(uri, file_uri(&path), "{needle:?}");
        assert_eq!(
            start,
            position_at(FUNC_SV, "[3:0] dat", 6),
            "{needle:?} must reach the FUNCTION-LOCAL declaration"
        );
    }
    client.shutdown();
}

/// Task-local variable shadowing a module signal: uses outside the task
/// (including the call-site argument) stay on the module-level declaration.
#[test]
fn task_local_shadow_outside_uses_stay_on_module_signal() {
    let ws = Workspace::new("task");
    ws.write(CONFIG_FILE_NAME, CONFIG_TEXT);
    ws.write("task.sv", TASK_SV);
    let path = ws.root().join("task.sv");

    let mut client = LspProcess::spawn(&ws.base);
    client.initialize(&ws.root()).expect("initialize workspace");
    open_and_wait_for_binding(
        &mut client,
        &ws,
        "task.sv",
        TASK_SV,
        "bind=task.sv:2:8[go,net]",
    );

    // Module-level use AFTER the task call → the module-level `cap`.
    let (uri, start) = definition_starts_at(&mut client, &path, TASK_SV, "cap = 8'h00", 1);
    assert_eq!(uri, file_uri(&path));
    assert_eq!(
        start,
        position_at(TASK_SV, "[7:0] cap", 6),
        "post-task use → module-level cap"
    );

    // Unshadowed call-site argument still hits its own declaration.
    let (uri, start) = definition_starts_at(&mut client, &path, TASK_SV, "t_sh(go", 6);
    assert_eq!(uri, file_uri(&path));
    assert_eq!(
        start,
        position_at(TASK_SV, "logic go", 6),
        "call-site arg → `logic go` decl"
    );
    client.shutdown();
}

/// Writes INSIDE the task body resolve to the task-local declaration.
#[test]
fn task_local_use_resolves_to_the_task_local_declaration() {
    let ws = Workspace::new("task-body");
    ws.write(CONFIG_FILE_NAME, CONFIG_TEXT);
    ws.write("task.sv", TASK_SV);
    let path = ws.root().join("task.sv");

    let mut client = LspProcess::spawn(&ws.base);
    client.initialize(&ws.root()).expect("initialize workspace");
    open_and_wait_for_binding(
        &mut client,
        &ws,
        "task.sv",
        TASK_SV,
        "bind=task.sv:2:8[go,net]",
    );

    let (uri, start) = definition_starts_at(&mut client, &path, TASK_SV, "cap = {12'h0", 1);
    assert_eq!(uri, file_uri(&path));
    assert_eq!(
        start,
        position_at(TASK_SV, "[15:0] cap", 7),
        "task-body write must reach the TASK-LOCAL declaration"
    );
    client.shutdown();
}

/// Named generate block shadowing a module signal: uses BEFORE and AFTER the
/// generate keep resolving to the OUTER declaration (UHDM binds those).
#[test]
fn generate_block_shadow_outer_uses_before_and_after_generate() {
    let ws = Workspace::new("gen");
    ws.write(CONFIG_FILE_NAME, CONFIG_TEXT);
    ws.write("gen.sv", GEN_SV);
    let path = ws.root().join("gen.sv");

    let mut client = LspProcess::spawn(&ws.base);
    client.initialize(&ws.root()).expect("initialize workspace");
    open_and_wait_for_binding(
        &mut client,
        &ws,
        "gen.sv",
        GEN_SV,
        "bind=gen.sv:1:14[val,net]",
    );

    let (uri, start) = definition_starts_at(&mut client, &path, GEN_SV, "pre = val", 7);
    assert_eq!(uri, file_uri(&path));
    assert_eq!(
        start,
        position_at(GEN_SV, "[7:0] val", 6),
        "use BEFORE generate → outer decl"
    );

    let (uri, start) = definition_starts_at(&mut client, &path, GEN_SV, "post = val", 7);
    assert_eq!(uri, file_uri(&path));
    assert_eq!(
        start,
        position_at(GEN_SV, "[7:0] val", 6),
        "use AFTER generate → outer decl"
    );
    client.shutdown();
}

/// A use INSIDE the named generate block resolves to the generate-block-local
/// declaration.
#[test]
fn generate_block_interior_use_resolves_to_the_genblk_local_declaration() {
    let ws = Workspace::new("gen-inside");
    ws.write(CONFIG_FILE_NAME, CONFIG_TEXT);
    ws.write("gen.sv", GEN_SV);
    let path = ws.root().join("gen.sv");

    let mut client = LspProcess::spawn(&ws.base);
    client.initialize(&ws.root()).expect("initialize workspace");
    open_and_wait_for_binding(
        &mut client,
        &ws,
        "gen.sv",
        GEN_SV,
        "bind=gen.sv:1:14[val,net]",
    );

    let (uri, start) = definition_starts_at(&mut client, &path, GEN_SV, "assign val = 4'h0", 7);
    assert_eq!(uri, file_uri(&path));
    assert_eq!(
        start,
        position_at(GEN_SV, "[3:0] val", 6),
        "genblk-interior use must reach the GENBLK-LOCAL declaration"
    );
    client.shutdown();
}

/// Two-level nesting (block inside block): the middle scope wins over the
/// module level, the innermost wins inside the inner block, and the
/// module-level use keeps the outer net.  All three directions are
/// binding-precise today because UHDM captured every use site.
#[test]
fn two_level_nesting_each_scope_wins_inside_itself() {
    let ws = Workspace::new("two");
    ws.write(CONFIG_FILE_NAME, CONFIG_TEXT);
    ws.write("two.sv", TWO_SV);
    let path = ws.root().join("two.sv");

    let mut client = LspProcess::spawn(&ws.base);
    client.initialize(&ws.root()).expect("initialize workspace");
    let lines = open_and_wait_for_binding(
        &mut client,
        &ws,
        "two.sv",
        TWO_SV,
        "bind=two.sv:7:18[sig,var]",
    );
    for want in ["bind=two.sv:5:16[sig,var]", "bind=two.sv:1:15[sig,net]"] {
        assert!(
            lines.iter().any(|l| l.contains(want)),
            "missing oracle binding {want}: {lines:?}"
        );
    }

    // Inside inner_blk → innermost declaration (7:18).
    let (uri, start) = definition_starts_at(&mut client, &path, TWO_SV, "sig = {3'b000", 1);
    assert_eq!(uri, file_uri(&path));
    assert_eq!(
        start,
        position_at(TWO_SV, "[3:0] sig", 6),
        "innermost scope wins"
    );

    // In outer_blk but outside inner_blk → middle declaration (5:16).
    let (uri, start) = definition_starts_at(&mut client, &path, TWO_SV, "sig = en ?", 1);
    assert_eq!(uri, file_uri(&path));
    assert_eq!(
        start,
        position_at(TWO_SV, "[7:0] sig", 6),
        "middle scope wins over outer"
    );

    // Module level → the module-level net (1:15).
    let (uri, start) = definition_starts_at(&mut client, &path, TWO_SV, "assign sig = 16", 7);
    assert_eq!(uri, file_uri(&path));
    assert_eq!(
        start,
        position_at(TWO_SV, "[15:0] sig", 7),
        "module-level use → outer net"
    );
    client.shutdown();
}

/// Hover inside the nested blocks reflects the innermost/middle widths rather
/// than the module-level type.
#[test]
fn two_level_hover_shows_the_innermost_scope_type() {
    let ws = Workspace::new("two-hover");
    ws.write(CONFIG_FILE_NAME, CONFIG_TEXT);
    ws.write("two.sv", TWO_SV);
    let path = ws.root().join("two.sv");

    let mut client = LspProcess::spawn(&ws.base);
    client.initialize(&ws.root()).expect("initialize workspace");
    open_and_wait_for_binding(
        &mut client,
        &ws,
        "two.sv",
        TWO_SV,
        "bind=two.sv:7:18[sig,var]",
    );

    let markup = hover_markup(&mut client, &path, TWO_SV, "sig = {3'b000", 2);
    assert!(
        markup.contains("[3:0]"),
        "hover in inner_blk must show [3:0]: {markup:?}"
    );
    let markup = hover_markup(&mut client, &path, TWO_SV, "sig = en ?", 2);
    assert!(
        markup.contains("[7:0]"),
        "hover in outer_blk must show [7:0]: {markup:?}"
    );
    let markup = hover_markup(&mut client, &path, TWO_SV, "assign sig = 16", 8);
    assert!(
        markup.contains("[15:0]"),
        "hover at module level must show [15:0]: {markup:?}"
    );
    client.shutdown();
}

/// Nested module instances' namespaces: parent and child declare the SAME
/// net name (`mode`).  A use inside the CHILD module must resolve to the
/// child's own port, the connection label `.mode` to the child port, and the
/// connected ACTUAL to the tb's own declaration — never across instances.
#[test]
fn nested_instance_namespaces_bind_within_their_own_module() {
    let ws = Workspace::new("inst");
    ws.write(CONFIG_FILE_NAME, CONFIG_TEXT);
    ws.write("leaf.sv", LEAF_SV);
    ws.write("tb_nested.sv", TB_NESTED_SV);
    let leaf_path = ws.root().join("leaf.sv");
    let tb_path = ws.root().join("tb_nested.sv");

    let mut client = LspProcess::spawn(&ws.base);
    client.initialize(&ws.root()).expect("initialize workspace");
    open_and_wait_for_binding(&mut client, &ws, "tb_nested.sv", TB_NESTED_SV, "via=label");

    // Child-internal use → the child's OWN port (per-instance clone).
    let leaf_text = fs::read_to_string(&leaf_path).expect("read leaf.sv");
    let (uri, start) = definition_starts_at(&mut client, &leaf_path, &leaf_text, "= mode", 3);
    assert_eq!(
        uri,
        file_uri(&leaf_path),
        "child-internal use stays in leaf.sv"
    );
    assert_eq!(start, position_at(LEAF_SV, "input logic mode", 12));

    // Label `.mode` → child port; actual `(mode)` → tb's OWN declaration.
    let (uri, start) = definition_starts_at(&mut client, &tb_path, TB_NESTED_SV, ".mode(mode)", 2);
    assert_eq!(uri, file_uri(&leaf_path), "label reaches the child port");
    assert_eq!(start, position_at(LEAF_SV, "input logic mode", 12));

    let (uri, start) = definition_starts_at(&mut client, &tb_path, TB_NESTED_SV, ".mode(mode)", 7);
    assert_eq!(
        uri,
        file_uri(&tb_path),
        "actual stays in the instantiating module"
    );
    assert_eq!(start, position_at(TB_NESTED_SV, "logic mode", 6));

    // tb's own block-local shadow + module-level use (already covered by the
    // named-block semantics; kept here to prove coexistence with instances).
    let (uri, start) = definition_starts_at(&mut client, &tb_path, TB_NESTED_SV, "mode = 1'b0", 1);
    assert_eq!(uri, file_uri(&tb_path));
    assert_eq!(start, position_at(TB_NESTED_SV, "[3:0] mode", 6));
    client.shutdown();
}

/// Cross-file variant: the instantiating module shadows a name that ALSO
/// exists as the child's port in another file.  Block-local defs stay in the
/// instantiating file; label/actual keep their documented split across files.
#[test]
fn cross_file_shadowing_stays_within_the_instantiating_module() {
    let ws = Workspace::new("cross");
    ws.write(CONFIG_FILE_NAME, CONFIG_TEXT);
    ws.write("cross_leaf.sv", CROSS_LEAF_SV);
    ws.write("tb_cross.sv", TB_CROSS_SV);
    let leaf_path = ws.root().join("cross_leaf.sv");
    let tb_path = ws.root().join("tb_cross.sv");

    let mut client = LspProcess::spawn(&ws.base);
    client.initialize(&ws.root()).expect("initialize workspace");
    open_and_wait_for_binding(&mut client, &ws, "tb_cross.sv", TB_CROSS_SV, "via=label");

    // Connection LABEL `.sel` → the CHILD module's port in cross_leaf.sv.
    let (uri, start) = definition_starts_at(&mut client, &tb_path, TB_CROSS_SV, ".sel(sel)", 2);
    assert_eq!(
        uri,
        file_uri(&leaf_path),
        "label crosses into the child file"
    );
    assert_eq!(start, position_at(CROSS_LEAF_SV, "input logic sel", 12));

    // Connected ACTUAL → tb's OWN module-level declaration.
    let (uri, start) = definition_starts_at(&mut client, &tb_path, TB_CROSS_SV, ".sel(sel)", 7);
    assert_eq!(uri, file_uri(&tb_path));
    assert_eq!(start, position_at(TB_CROSS_SV, "logic sel", 6));

    // Block-local use → the block-local declaration in the SAME file.
    let (uri, start) = definition_starts_at(&mut client, &tb_path, TB_CROSS_SV, "sel = 1'b0", 1);
    assert_eq!(uri, file_uri(&tb_path));
    assert_eq!(start, position_at(TB_CROSS_SV, "[3:0] sel", 6));

    // Module-level use → the tb's own declaration again.
    let (uri, start) =
        definition_starts_at(&mut client, &tb_path, TB_CROSS_SV, "assign sel = 1'b1", 7);
    assert_eq!(uri, file_uri(&tb_path));
    assert_eq!(start, position_at(TB_CROSS_SV, "logic sel", 6));
    client.shutdown();
}

/// An OPENED document whose inner shadowing block was added client-side (the
/// on-disk file lacks it) analyzes the staged buffer: definitions inside the
/// freshly added block bind to the added declaration, uses outside keep the
/// module-level one.
#[test]
fn opened_buffer_client_side_block_shadows_module_signal() {
    let ws = Workspace::new("edit");
    ws.write(CONFIG_FILE_NAME, CONFIG_TEXT);
    ws.write("edit.sv", EDIT_BASE_SV);
    let path = ws.root().join("edit.sv");
    let uri = file_uri(&path);

    let mut client = LspProcess::spawn(&ws.base);
    client.initialize(&ws.root()).expect("initialize workspace");
    client
        .open(&path, EDIT_BASE_SV)
        .expect("open baseline buffer");
    client
        .change(&path, 2, EDIT_BUFFER_SV)
        .expect("add inner block client-side");

    // Wait until the staged-buffer analysis carries the NEW inner-binding row
    // (positions shifted by the inserted line vs the disk baseline).
    let deadline = Instant::now() + POLL_TIMEOUT;
    let mut interval = POLL_INTERVAL;
    loop {
        match client.request_with_timeout(
            "llg/dumpTokens",
            json!({ "uri": uri }),
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
                let ready = lines
                    .iter()
                    .any(|l| l.contains("bind=edit.sv:4:16[val,var]"))
                    && lines
                        .iter()
                        .any(|l| l.contains("bind=edit.sv:1:14[val,net]"));
                if ready {
                    break;
                }
            }
            Err(error) if error.starts_with("timed out") => {}
            Err(error) => panic!("llg/dumpTokens failed: {error}"),
        }
        assert!(
            Instant::now() < deadline,
            "staged-buffer shadowing bindings never appeared"
        );
        thread::sleep(interval);
        interval = interval.saturating_mul(2).min(Duration::from_secs(1));
    }

    // Use inside the CLIENT-SIDE block → the added local declaration (4:16).
    let (def_uri, start) = definition_starts_at(&mut client, &path, EDIT_BUFFER_SV, "val = en", 1);
    assert_eq!(def_uri, uri, "definitions must map back to the real URI");
    assert_eq!(start, position_at(EDIT_BUFFER_SV, "[3:0] val", 6));

    // Use outside the block → the module-level declaration (1:14).
    let (_, start) = definition_starts_at(&mut client, &path, EDIT_BUFFER_SV, "assign val", 7);
    assert_eq!(start, position_at(EDIT_BUFFER_SV, "[7:0] val", 6));
    client.shutdown();
}
