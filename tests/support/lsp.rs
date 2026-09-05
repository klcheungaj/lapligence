use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(45);
const READY_MESSAGE: &str = "llg Verilog/SystemVerilog language server ready";

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

pub(crate) struct LspProcess {
    child: Child,
    stdin: Option<ChildStdin>,
    incoming: Receiver<ReaderMessage>,
    pub(crate) next_id: u64,
    server_requests: Vec<Value>,
    /// Cumulative record of every server-originated request (never pruned),
    /// used by tests that must observe repeated registrations.
    pub(crate) all_server_requests: Vec<Value>,
    pub(crate) notifications: Vec<Value>,
    pub(crate) orphan_responses: Vec<Value>,
}

impl LspProcess {
    pub(crate) fn spawn(cwd: &Path) -> Self {
        Self::spawn_configured(cwd, |_| {})
    }

    pub(crate) fn spawn_configured(cwd: &Path, configure: impl FnOnce(&mut Command)) -> Self {
        let mut command = Command::new(env!("CARGO_BIN_EXE_llg_ls"));
        command
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        configure(&mut command);
        let mut child = command.spawn().expect("spawn llg LSP server");
        let stdout = child.stdout.take().expect("capture llg stdout");
        let stdin = child.stdin.take().expect("capture llg stdin");
        Self {
            child,
            stdin: Some(stdin),
            incoming: spawn_reader(stdout),
            next_id: 1,
            server_requests: Vec::new(),
            all_server_requests: Vec::new(),
            notifications: Vec::new(),
            orphan_responses: Vec::new(),
        }
    }

    pub(crate) fn pid(&self) -> u32 {
        self.child.id()
    }

    pub(crate) fn send_message(&mut self, message: Value) -> Result<(), String> {
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

    pub(crate) fn request(&mut self, method: &str, params: Value) -> Result<Value, String> {
        self.request_with_timeout(method, params, REQUEST_TIMEOUT)
    }

    pub(crate) fn request_with_timeout(
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

    pub(crate) fn send_notification(&mut self, method: &str, params: Value) -> Result<(), String> {
        self.send_message(json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        }))
    }

    pub(crate) fn receive_until(&self, deadline: Instant) -> Result<Value, String> {
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

    pub(crate) fn route_unsolicited(&mut self, message: Value) -> Result<(), String> {
        if message.get("method").and_then(Value::as_str).is_some() {
            if let Some(id) = message.get("id").filter(|id| !id.is_null()).cloned() {
                // The client must answer server-originated requests, especially
                // client/registerCapability, before the server can continue.
                self.all_server_requests.push(message.clone());
                self.server_requests.push(message);
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

    /// Wait until `predicate` matches a server-originated request; returns the
    /// matching request from the cumulative history (nothing is pruned).
    pub(crate) fn wait_for_server_request_where<F>(&mut self, predicate: F) -> Result<Value, String>
    where
        F: Fn(&Value) -> bool,
    {
        let deadline = Instant::now() + REQUEST_TIMEOUT;
        loop {
            if let Some(message) = self
                .all_server_requests
                .iter()
                .find(|message| predicate(message))
            {
                return Ok(message.clone());
            }
            let message = self.receive_until(deadline)?;
            self.route_unsolicited(message)?;
        }
    }

    /// Wait for the process to exit within `timeout`, returning its exit code.
    /// stdin is intentionally left open by callers that exercise the exit
    /// notification path.
    pub(crate) fn wait_for_exit_code(&mut self, timeout: Duration) -> Option<i32> {
        let deadline = Instant::now() + timeout;
        loop {
            match self.child.try_wait() {
                Ok(Some(status)) => return status.code(),
                Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(25)),
                _ => return None,
            }
        }
    }

    pub(crate) fn close_stdin(&mut self) {
        self.stdin.take();
    }

    pub(crate) fn wait_for_server_request(&mut self, method: &str) -> Result<Value, String> {
        let deadline = Instant::now() + REQUEST_TIMEOUT;
        loop {
            if let Some(index) = self
                .server_requests
                .iter()
                .position(|message| message.get("method").and_then(Value::as_str) == Some(method))
            {
                return Ok(self.server_requests.remove(index));
            }
            let message = self.receive_until(deadline)?;
            self.route_unsolicited(message)?;
        }
    }

    pub(crate) fn wait_for_notification_where<F>(
        &mut self,
        method: &str,
        predicate: F,
    ) -> Result<Value, String>
    where
        F: Fn(&Value) -> bool,
    {
        let deadline = Instant::now() + REQUEST_TIMEOUT;
        loop {
            if let Some(index) = self.notifications.iter().position(|message| {
                message.get("method").and_then(Value::as_str) == Some(method)
                    && predicate(message.get("params").unwrap_or(&Value::Null))
            }) {
                return Ok(self.notifications.remove(index));
            }
            let message = self.receive_until(deadline)?;
            self.route_unsolicited(message)?;
        }
    }

    pub(crate) fn initialize(
        &mut self,
        roots: &[(&str, &Path)],
        options: Value,
    ) -> Result<Value, String> {
        self.initialize_with_dynamic_registration(roots, options, true)
    }

    pub(crate) fn initialize_static(
        &mut self,
        roots: &[(&str, &Path)],
        options: Value,
    ) -> Result<Value, String> {
        self.initialize_with_dynamic_registration(roots, options, false)
    }

    fn initialize_with_dynamic_registration(
        &mut self,
        roots: &[(&str, &Path)],
        options: Value,
        dynamic_registration: bool,
    ) -> Result<Value, String> {
        let workspace_folders: Vec<_> = roots
            .iter()
            .map(|(name, path)| json!({ "uri": file_uri(path), "name": name }))
            .collect();
        let result = self.request(
            "initialize",
            json!({
                "processId": std::process::id(),
                "rootUri": Value::Null,
                "workspaceFolders": workspace_folders,
                "initializationOptions": options,
                "clientInfo": { "name": "llg-stdio-contract-tests", "version": "1" },
                "capabilities": {
                    "workspace": {
                        "configuration": true,
                        "workspaceFolders": true,
                        "didChangeWatchedFiles": { "dynamicRegistration": dynamic_registration },
                        "didChangeWorkspaceFolders": { "dynamicRegistration": dynamic_registration }
                    },
                    "textDocument": {
                        "publishDiagnostics": { "relatedInformation": true },
                        "semanticTokens": { "dynamicRegistration": false, "requests": { "range": true, "full": true }, "tokenTypes": [], "tokenModifiers": [], "formats": ["relative"] }
                    }
                }
            }),
        )?;
        self.send_notification("initialized", json!({}))?;
        self.wait_for_notification_where("window/logMessage", |params| {
            params.get("message").and_then(Value::as_str) == Some(READY_MESSAGE)
        })?;
        Ok(result)
    }

    pub(crate) fn open(&mut self, path: &Path, text: &str) -> Result<(), String> {
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

    pub(crate) fn change(&mut self, path: &Path, version: i64, text: &str) -> Result<(), String> {
        self.send_notification(
            "textDocument/didChange",
            json!({
                "textDocument": { "uri": file_uri(path), "version": version },
                "contentChanges": [{ "text": text }]
            }),
        )
    }

    pub(crate) fn send_watch_event(&mut self, path: &Path, event_type: u8) -> Result<(), String> {
        self.send_notification(
            "workspace/didChangeWatchedFiles",
            json!({
                "changes": [{ "uri": file_uri(path), "type": event_type }]
            }),
        )
    }

    pub(crate) fn send_workspace_folder_change(
        &mut self,
        added: &[(&str, &Path)],
        removed: &[(&str, &Path)],
    ) -> Result<(), String> {
        self.send_notification(
            "workspace/didChangeWorkspaceFolders",
            json!({
                "event": {
                    "added": added.iter().map(|(name, path)| json!({
                        "uri": file_uri(path), "name": name
                    })).collect::<Vec<_>>(),
                    "removed": removed.iter().map(|(name, path)| json!({
                        "uri": file_uri(path), "name": name
                    })).collect::<Vec<_>>()
                }
            }),
        )
    }

    pub(crate) fn shutdown(&mut self) {
        let _ = self.request("shutdown", Value::Null);
        let _ = self.send_notification("exit", Value::Null);
        self.stdin.take();
        self.wait_for_exit();
    }

    pub(crate) fn wait_for_exit(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            match self.child.try_wait() {
                Ok(Some(_)) => return,
                Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
                Ok(None) => {
                    let _ = self.child.kill();
                    let _ = self.child.wait();
                    return;
                }
                Err(_) => return,
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

pub(crate) fn file_uri(path: &Path) -> String {
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

/// The default client-to-server initialization payload: protocol v1 with no
/// config-file overrides (so each root uses `<root>/llg.toml`).
pub(crate) fn default_init_options() -> Value {
    json!({
        "llg": {
            "protocolVersion": 1,
            "configFiles": []
        }
    })
}
