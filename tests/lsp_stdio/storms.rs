//! Edits, closes, files changed and deleted on disk and configuration
//! reloads while requests of every kind are in flight (FIN-002 step 4).
//!
//! The storms assert the invariants that hold whatever the interleaving:
//! every request is answered exactly once, with a result or with one of the
//! two errors that say it was overtaken; no range in an answer points at
//! something else than what the request asked about, in every text the
//! document had between the request and its answer; the server settles on
//! the last text and exits with 0.

use super::*;
use crate::fuzz::{cases, Fuzz};
use std::collections::{BTreeMap, HashMap, HashSet};

/// Storms per run; `LLG_FUZZ_CASES` names a longer local campaign.
const STORMS: u64 = 4;
const STEPS: u64 = 160;

const CONFIG: &str = "schema_version = 1\n";

const REQUEST_CANCELLED: i64 = -32800;
const CONTENT_MODIFIED: i64 = -32801;
const INVALID_PARAMS: i64 = -32602;

const FILES: [(&str, &str); 4] = [
    (
        "pkg.sv",
        "package pkg_name;\n  parameter int PKG_W = 4;\n  typedef logic [PKG_W-1:0] pkg_t;\n  \
         function automatic int pkg_fn(int pkg_arg);\n    return pkg_arg + PKG_W;\n  \
         endfunction\nendpackage\n",
    ),
    (
        "leaf.sv",
        "`include \"defs.svh\"\nmodule leaf_mod import pkg_name::*; \
         (input pkg_t leaf_in, output pkg_t leaf_out);\n  logic [`DEFS_W-1:0] leaf_wire;\n  \
         assign leaf_wire = leaf_in;\n  assign leaf_out = leaf_wire;\n  \
         initial $display(\"%0d\", pkg_fn(3));\nendmodule\n",
    ),
    (
        "top.sv",
        "module top_mod(input logic top_clk);\n  pkg_name::pkg_t top_a, top_b;\n  \
         leaf_mod u_leaf(.leaf_in(top_a), .leaf_out(top_b));\n  \
         always @(posedge top_clk) top_a <= top_b;\nendmodule\n",
    ),
    ("defs.svh", "`define DEFS_W 4\n"),
];

const METHODS: [&str; 9] = [
    "textDocument/hover",
    "textDocument/definition",
    "textDocument/references",
    "textDocument/prepareRename",
    "textDocument/rename",
    "textDocument/completion",
    "textDocument/semanticTokens/full",
    "textDocument/documentSymbol",
    "textDocument/documentColor",
];

/// A text of one file: shifted, broken, cut or whole.
fn variant(fuzz: &mut Fuzz, base: &str, name: &str) -> String {
    let mut text = base.to_owned();
    match fuzz.below(9) {
        0 => text = text.replace("\n  ", "\n /* \u{e9}\u{1F600} */  "),
        1 => text = text.replacen(";\nendm", "\nendm", 1),
        2 => text = text.replacen("assign", "assig", 1),
        3 => text = text.replace("endmodule", ""),
        4 => text.clear(),
        5 => text = text.replace("leaf_wire", "leaf_wire_2"),
        _ => {}
    }
    let comments = fuzz.below(5);
    (0..comments)
        .map(|line| format!("// {name} {line} \u{e9}\u{1F600}\n"))
        .collect::<String>()
        + &text
}

fn is_word(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// The byte offset of a UTF-16 column in `line`.
fn byte_column(line: &str, column: u64) -> usize {
    let mut units = 0;
    for (offset, character) in line.char_indices() {
        if units >= column {
            return offset;
        }
        units += character.len_utf16() as u64;
    }
    line.len()
}

/// The text of a one-line range, if the document has it.
fn spelled<'a>(text: &'a str, range: &Value) -> Option<&'a str> {
    let line = range["start"]["line"].as_u64()?;
    if range["end"]["line"].as_u64()? != line {
        return None;
    }
    let line = text.split('\n').nth(line as usize)?;
    let start = byte_column(line, range["start"]["character"].as_u64()?);
    let end = byte_column(line, range["end"]["character"].as_u64()?);
    line.get(start..end)
}

/// The identifier at a position (its end included).
fn word_at<'a>(text: &'a str, position: &Value) -> Option<&'a str> {
    let line = text.split('\n').nth(position["line"].as_u64()? as usize)?;
    let at = byte_column(line, position["character"].as_u64()?);
    let bytes = line.as_bytes();
    let start = (0..at)
        .rev()
        .take_while(|&index| is_word(bytes[index]))
        .last()
        .unwrap_or(at);
    let end = (at..bytes.len())
        .find(|&index| !is_word(bytes[index]))
        .unwrap_or(bytes.len());
    (start < end).then(|| &line[start..end])
}

/// A position inside an identifier of `text`, or anywhere.
fn position(fuzz: &mut Fuzz, text: &str) -> Value {
    let words: Vec<Value> = text
        .split('\n')
        .enumerate()
        .flat_map(|(number, line)| {
            let bytes = line.as_bytes();
            (0..bytes.len())
                .filter(|&index| is_word(bytes[index]))
                .map(|index| {
                    let column = line[..index].encode_utf16().count();
                    json!({ "line": number, "character": column })
                })
                .collect::<Vec<_>>()
        })
        .collect();
    if words.is_empty() || fuzz.chance(1, 12) {
        return json!({ "line": fuzz.below(12), "character": fuzz.below(60) });
    }
    fuzz.pick(&words).clone()
}

#[derive(Debug)]
struct Pending {
    method: &'static str,
    file: Option<usize>,
    position: Value,
    step: u64,
}

struct Storm {
    client: LspProcess,
    label: String,
    directory: PathBuf,
    /// Per file, the texts the editor had, with the step that set them;
    /// `None` while the file is closed.
    history: Vec<Vec<(u64, Option<String>)>>,
    versions: [i64; FILES.len()],
    pending: HashMap<u64, Pending>,
    answered: HashSet<u64>,
    step: u64,
}

impl Storm {
    fn path(&self, file: usize) -> PathBuf {
        self.directory.join(FILES[file].0)
    }

    fn text(&self, file: usize) -> Option<&str> {
        self.history[file]
            .last()
            .and_then(|(_, text)| text.as_deref())
    }

    /// Every state a document had from `step` on.
    fn states(&self, file: usize, step: u64) -> Vec<Option<&str>> {
        let history = &self.history[file];
        let first = history
            .partition_point(|(set, _)| *set <= step)
            .saturating_sub(1);
        history[first..]
            .iter()
            .map(|(_, text)| text.as_deref())
            .collect()
    }

    fn set_text(&mut self, file: usize, text: String) {
        let path = self.path(file);
        self.versions[file] += 1;
        if self.text(file).is_some() {
            self.client.change(&path, self.versions[file], &text)
        } else {
            self.client.open(&path, &text)
        }
        .expect("set the text");
        self.history[file].push((self.step, Some(text)));
    }

    fn request(&mut self, method: &'static str, file: Option<usize>, params: Value) -> u64 {
        let id = self.client.next_id;
        self.client.next_id += 1;
        let pending = Pending {
            method,
            file,
            position: params["position"].clone(),
            step: self.step,
        };
        self.client
            .send_message(json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }))
            .expect("send the request");
        self.pending.insert(id, pending);
        id
    }

    /// Take what the server sent until it is quiet for `wait`.
    fn drain(&mut self, wait: Duration) {
        while let Ok(message) = self.client.receive_until(Instant::now() + wait) {
            self.client
                .route_unsolicited(message)
                .expect("route a message");
        }
        for response in std::mem::take(&mut self.client.orphan_responses) {
            let id = response["id"].as_u64().expect("a response id");
            assert!(
                self.answered.insert(id),
                "{}: {id} is answered twice",
                self.label
            );
            let pending = self.pending.remove(&id).expect("an answer to a request");
            self.check(&pending, &response);
        }
    }

    fn check(&self, pending: &Pending, response: &Value) {
        let method = pending.method;
        if let Some(error) = response.get("error") {
            let code = error["code"].as_i64().unwrap_or_default();
            assert!(
                matches!(code, REQUEST_CANCELLED | CONTENT_MODIFIED)
                    || (code == INVALID_PARAMS && method == "textDocument/rename"),
                "{}: {method}: {error}",
                self.label
            );
            return;
        }
        let result = &response["result"];
        let Some(file) = pending.file.filter(|_| !result.is_null()) else {
            return;
        };
        let names: HashSet<&str> = self
            .states(file, pending.step)
            .into_iter()
            .flatten()
            .filter_map(|text| word_at(text, &pending.position))
            .collect();
        let named = |uri: &str, range: &Value, any_word: bool| {
            self.spells(pending, uri, &[range], |text| {
                // A macro usage is marked with its back quote.
                let text = text.strip_prefix('`').unwrap_or(text);
                names.contains(text) || (any_word && text.bytes().all(is_word))
            });
        };
        let uri = file_uri(&self.path(file));
        match method {
            "textDocument/hover" | "textDocument/prepareRename" => {
                if let Some(range) = result.get("range") {
                    named(&uri, range, false);
                }
            }
            "textDocument/definition" => {
                let one = std::slice::from_ref(result);
                for location in result.as_array().map_or(one, Vec::as_slice) {
                    let range = &location["range"];
                    if range["start"] != range["end"] {
                        named(location["uri"].as_str().expect("uri"), range, true);
                    }
                }
            }
            "textDocument/references" => {
                for location in result.as_array().expect("references") {
                    named(
                        location["uri"].as_str().expect("uri"),
                        &location["range"],
                        false,
                    );
                }
            }
            "textDocument/rename" => {
                for (uri, edits) in result["changes"].as_object().expect("changes") {
                    let ranges: Vec<&Value> = edits
                        .as_array()
                        .expect("edits")
                        .iter()
                        .map(|edit| &edit["range"])
                        .collect();
                    self.spells(pending, uri, &ranges, |text| names.contains(text));
                }
            }
            "textDocument/semanticTokens/full" => self.check_tokens(pending, file, result),
            _ => {}
        }
    }

    /// All ranges of one document hold in one text it had since the request.
    fn spells(
        &self,
        pending: &Pending,
        uri: &str,
        ranges: &[&Value],
        accept: impl Fn(&str) -> bool,
    ) {
        let file = (0..FILES.len())
            .find(|&file| file_uri(&self.path(file)) == uri)
            .unwrap_or_else(|| panic!("{}: unknown document {uri}", self.label));
        let states = self.states(file, pending.step);
        // The text of a closed document is the one on disk, which the storm
        // changes under the server.
        let holds = states.iter().any(|state| {
            state.is_none_or(|text| {
                ranges
                    .iter()
                    .all(|range| spelled(text, range).is_some_and(&accept))
            })
        });
        assert!(
            holds,
            "{}: {} at {} answers {uri} {ranges:?}, stale in {states:#?}",
            self.label, pending.method, pending.position
        );
    }

    fn check_tokens(&self, pending: &Pending, file: usize, result: &Value) {
        let [Some(text)] = self.states(file, pending.step)[..] else {
            return;
        };
        let (mut line, mut column) = (0, 0);
        for token in result["data"].as_array().expect("tokens").chunks(5) {
            let field = |index: usize| token[index].as_u64().expect("a number");
            if field(0) > 0 {
                line += field(0);
                column = 0;
            }
            column += field(1);
            let range = json!({
                "start": { "line": line, "character": column },
                "end": { "line": line, "character": column + field(2) },
            });
            assert!(
                spelled(text, &range).is_some_and(|lexeme| {
                    !lexeme.is_empty()
                        && (lexeme.starts_with(['"', '/']) || !lexeme.contains(char::is_whitespace))
                }),
                "{}: token {range} of {text}",
                self.label
            );
        }
    }

    fn watched(&mut self, path: &Path, kind: u8) {
        self.client
            .send_watch_event(path, kind)
            .expect("watch event");
    }

    fn run_step(&mut self, fuzz: &mut Fuzz) {
        self.step += 1;
        let file = fuzz.index(FILES.len());
        let path = self.path(file);
        match fuzz.below(100) {
            0..=21 => {
                let text = variant(fuzz, FILES[file].1, FILES[file].0);
                self.set_text(file, text);
            }
            22..=26 if self.text(file).is_some() => {
                self.history[file].push((self.step, None));
                self.client
                    .send_notification(
                        "textDocument/didClose",
                        json!({ "textDocument": { "uri": file_uri(&path) } }),
                    )
                    .expect("close");
            }
            27..=29 => {
                if path.exists() && fuzz.chance(3, 5) {
                    fs::remove_file(&path).expect("delete a file");
                    self.watched(&path, 3);
                } else {
                    let kind = if path.exists() { 2 } else { 1 };
                    fs::write(&path, variant(fuzz, FILES[file].1, FILES[file].0)).expect("write");
                    self.watched(&path, kind);
                }
            }
            30..=31 => {
                let config = self.directory.join(CONFIG_FILE);
                let text = format!(
                    "{CONFIG}{}",
                    fuzz.pick(&[
                        "",
                        "[compile\n",
                        "[lint]\nenabled = false\n",
                        "[x]\na = 1\n"
                    ])
                );
                fs::write(&config, text).expect("write the configuration");
                self.watched(&config, 2);
            }
            32..=35 => {
                let id = fuzz.range(1, self.client.next_id + 2);
                self.cancel(id);
            }
            36..=39 => {
                let query = *fuzz.pick(&["", "leaf", "zz"]);
                self.request("workspace/symbol", None, json!({ "query": query }));
            }
            _ => {
                let open: Vec<usize> = (0..FILES.len())
                    .filter(|&file| self.text(file).is_some())
                    .collect();
                if open.is_empty() {
                    return;
                }
                let file = *fuzz.pick(&open);
                let method = *fuzz.pick(&METHODS);
                let position = position(fuzz, self.text(file).expect("an open file"));
                let id = self.request(
                    method,
                    Some(file),
                    json!({
                        "textDocument": { "uri": file_uri(&self.path(file)) },
                        "position": position,
                        "context": {
                            "includeDeclaration": fuzz.chance(1, 2),
                            "triggerKind": 1,
                        },
                        "newName": *fuzz.pick(&["renamed", "module", "1x", ""]),
                    }),
                );
                if fuzz.chance(1, 10) {
                    self.cancel(id);
                }
            }
        }
        if fuzz.chance(1, 7) {
            thread::sleep(Duration::from_millis(fuzz.below(100)));
        }
        self.drain(Duration::from_millis(1));
    }

    fn cancel(&mut self, id: u64) {
        self.client
            .send_notification("$/cancelRequest", json!({ "id": id }))
            .expect("cancel");
    }

    /// Back to the original texts: the answers are those of a fresh server.
    fn settle(&mut self) {
        self.answer_all();
        self.step += 1;
        let config = self.directory.join(CONFIG_FILE);
        fs::write(&config, CONFIG).expect("write the configuration");
        self.watched(&config, 2);
        for (file, (_, text)) in FILES.iter().enumerate() {
            let path = self.path(file);
            fs::write(&path, text).expect("write a file");
            self.watched(&path, 2);
            self.set_text(file, (*text).to_owned());
        }
        let params = json!({
            "textDocument": { "uri": file_uri(&self.path(1)) },
            "position": position_at(FILES[1].1, "leaf_in", 0),
            "context": { "includeDeclaration": true },
        });
        let deadline = Instant::now() + REQUEST_TIMEOUT;
        loop {
            let found = self
                .client
                .request("textDocument/references", params.clone())
                .expect("references");
            if found
                .as_array()
                .is_some_and(|locations| locations.len() == 3)
            {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "{}: settled on {found}",
                self.label
            );
            thread::sleep(POLL_INTERVAL);
        }
    }
}

impl Storm {
    /// A server on a fresh copy of [`FILES`]; the directory goes with the
    /// returned guard.
    fn start(name: &str) -> (rename::TempDirCleanup, Self) {
        let directory =
            std::env::temp_dir().join(format!("llg-lsp-storm-{}-{name}", std::process::id()));
        fs::create_dir_all(&directory).expect("storm directory");
        fs::write(directory.join(CONFIG_FILE), CONFIG).expect("write the configuration");
        for (name, text) in FILES {
            fs::write(directory.join(name), text).expect("write a file");
        }
        let mut client = LspProcess::spawn(&directory);
        client
            .initialize(&[("storm", &directory)], default_init_options())
            .expect("initialize");
        let storm = Self {
            client,
            label: name.to_owned(),
            directory: directory.clone(),
            history: vec![vec![(0, None)]; FILES.len()],
            versions: [1; FILES.len()],
            pending: HashMap::new(),
            answered: HashSet::new(),
            step: 0,
        };
        (rename::TempDirCleanup(directory), storm)
    }

    fn answer_all(&mut self) {
        let deadline = Instant::now() + REQUEST_TIMEOUT;
        while !self.pending.is_empty() {
            assert!(
                Instant::now() < deadline,
                "{}: unanswered {:?}",
                self.label,
                self.pending
            );
            self.drain(Duration::from_millis(50));
        }
    }

    /// The last diagnostics of every file hold no error.
    fn wait_until_clean(&mut self) {
        let deadline = Instant::now() + REQUEST_TIMEOUT;
        loop {
            let last: BTreeMap<&str, &Value> = self
                .client
                .notifications
                .iter()
                .filter(|message| message["method"] == "textDocument/publishDiagnostics")
                .filter_map(|message| {
                    Some((message["params"]["uri"].as_str()?, &message["params"]))
                })
                .collect();
            if last.values().all(|params| has_no_severity_1(params)) {
                return;
            }
            assert!(Instant::now() < deadline, "{}: {last:#?}", self.label);
            self.drain(POLL_INTERVAL);
        }
    }

    fn exit(mut self) {
        let _ = self.client.request("shutdown", Value::Null);
        let _ = self.client.send_notification("exit", Value::Null);
        self.client.close_stdin();
        assert_eq!(
            self.client.wait_for_exit_code(Duration::from_secs(20)),
            Some(0),
            "{}",
            self.label
        );
    }
}

#[test]
fn a_storm_of_edits_and_requests_gets_no_stale_answer() {
    for case in cases(STORMS) {
        let (_directory, mut storm) = Storm::start(&format!("storm-{case}"));
        let mut fuzz = Fuzz::new("lsp_storm", case);
        for _ in 0..STEPS {
            storm.run_step(&mut fuzz);
        }
        storm.settle();
        storm.wait_until_clean();
        storm.exit();
    }
}

/// Requests that follow an edit at once, at the position the identifier had
/// and at the one it has, are not answered from the analysis of the old text.
#[test]
fn requests_behind_an_edit_get_no_position_of_the_old_text() {
    let (_directory, mut storm) = Storm::start("edits");
    let old = FILES[1].1;
    storm.set_text(1, old.to_owned());
    storm.set_text(2, FILES[2].1.to_owned());
    storm.settle();
    let edits = [
        format!("// one\n// two\n{old}"),
        old.replace("leaf_wire", "other_wire"),
        old.replacen(";\nendmodule", "", 1),
        format!("// one\n{}", old.replace("leaf_in", "in")),
        String::new(),
    ];
    for edited in edits {
        storm.step += 1;
        storm.set_text(1, old.to_owned());
        storm.settle();
        storm.step += 1;
        storm.set_text(1, edited.clone());
        for (text, word) in [(old, "leaf_wire"), (old, "leaf_in"), (old, "pkg_fn")]
            .into_iter()
            .chain(
                ["leaf_wire", "leaf_in", "pkg_fn"]
                    .into_iter()
                    .filter(|word| edited.contains(word))
                    .map(|word| (edited.as_str(), word)),
            )
        {
            for method in METHODS {
                storm.request(
                    method,
                    Some(1),
                    json!({
                        "textDocument": { "uri": file_uri(&storm.path(1)) },
                        "position": position_at(text, word, 0),
                        "context": { "includeDeclaration": true, "triggerKind": 1 },
                        "newName": "renamed",
                    }),
                );
            }
        }
        // The other document's answers name positions of the edited one.
        for method in METHODS {
            storm.request(
                method,
                Some(2),
                json!({
                    "textDocument": { "uri": file_uri(&storm.path(2)) },
                    "position": position_at(FILES[2].1, "leaf_in", 0),
                    "context": { "includeDeclaration": true, "triggerKind": 1 },
                    "newName": "renamed",
                }),
            );
        }
        storm.answer_all();
    }
    storm.settle();
    storm.wait_until_clean();
    storm.exit();
}

/// An open document outlives its file: the server answers from the editor's
/// text, whatever happens on disk.
#[test]
fn a_document_whose_file_is_deleted_is_answered_from_its_text() {
    let (_directory, mut storm) = Storm::start("deleted");
    for (file, (_, text)) in FILES.iter().enumerate() {
        storm.set_text(file, (*text).to_owned());
    }
    storm.settle();
    for file in 0..FILES.len() {
        let path = storm.path(file);
        fs::remove_file(&path).expect("delete a file");
        storm.watched(&path, 3);
    }
    let edited = format!("// one\n{}", FILES[1].1);
    storm.step += 1;
    storm.set_text(1, edited.clone());
    let params = json!({
        "textDocument": { "uri": file_uri(&storm.path(1)) },
        "position": position_at(&edited, "leaf_wire", 0),
        "context": { "includeDeclaration": true },
    });
    let deadline = Instant::now() + REQUEST_TIMEOUT;
    loop {
        let found = storm
            .client
            .request("textDocument/references", params.clone())
            .expect("references");
        let found = found.as_array().map_or(0, Vec::len);
        if found == 3 {
            break;
        }
        assert!(Instant::now() < deadline, "{found} references");
        thread::sleep(POLL_INTERVAL);
    }
    storm.settle();
    storm.wait_until_clean();
    storm.exit();
}

/// The server's own requests wait for the client while the client sends more:
/// a handler that waited for the reply would keep the server from reading it.
#[test]
fn a_client_that_answers_late_gets_every_answer() {
    let (_directory, mut storm) = Storm::start("late");
    for (file, (_, text)) in FILES.iter().enumerate() {
        storm.set_text(file, (*text).to_owned());
    }
    let config = storm.directory.join(CONFIG_FILE);
    for step in 0..60 {
        // Every reload changes the watched directories.
        let text = format!("{CONFIG}[sources]\ndirectories = [\".\", \"sub{step}\"]\n");
        fs::create_dir_all(storm.directory.join(format!("sub{step}"))).expect("directory");
        fs::write(&config, text).expect("write the configuration");
        storm.watched(&config, 2);
        for method in ["textDocument/hover", "textDocument/documentSymbol"] {
            storm.request(
                method,
                Some(1),
                json!({
                    "textDocument": { "uri": file_uri(&storm.path(1)) },
                    "position": position_at(FILES[1].1, "leaf_in", 0),
                }),
            );
        }
        thread::sleep(Duration::from_millis(5));
    }
    storm.answer_all();
    storm.settle();
    storm.exit();
}
