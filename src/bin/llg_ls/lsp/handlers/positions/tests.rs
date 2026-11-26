//! Requests about edited documents, compared with those of a server that
//! analysed the same text.

use super::*;
use crate::features::TEST_PROCESS_SHADOW_LOCK;
use crate::position_map::is_word;
use llg::core::tokens::SourcePositions;
use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, Instant};
use tower_lsp::{LanguageServer, LspService};

const DEFS: &str = "defs.svh";
const TOP: &str = "top.sv";

const PROJECT: [(&str, &str); 4] = [
    (
        DEFS,
        "`ifndef DEFS_SVH
`define DEFS_SVH
// définitions 𝄞
`define WIDTH 8
`define MAX(a, b) ((a) > (b) ? (a) : (b))
`define SUM(x) \\
  ((x) + `WIDTH)
typedef logic [`WIDTH-1:0] word_t;
localparam int LIMIT = `MAX(3, 5);
`endif
",
    ),
    (
        "pkg.sv",
        "package pkg;
`include \"defs.svh\"
  parameter int DEPTH = 4;
  typedef enum logic [1:0] { IDLE, BUSY } state_t;
  typedef struct packed { word_t data; logic valid; } item_t;
  function automatic int twice(int v);
    return v * 2;
  endfunction
  function automatic int thrice(bit v);
    return v * 3;
  endfunction

  class Counter;
    int count;
    function new(); count = 0; endfunction
    function void step(int by); count += by; endfunction
  endclass
endpackage
",
    ),
    (
        "sub.sv",
        "// A register.
module sub #(parameter int W = 8) (
  input  logic clk,
  input  logic [W-1:0] din,
  output logic [W-1:0] dout
);
  always_ff @(posedge clk)
  begin
    dout <= din;
  end
endmodule

module other (input logic clk, output logic q);
  always_ff @(posedge clk)
  begin
    q <= !q;
  end
endmodule
",
    ),
    (
        TOP,
        "`include \"defs.svh\"
module top;
  import pkg::*;
  string text = \"é𝄞 // no comment\"; /* 𝄞 */ int after;
  logic clk;
  logic [`WIDTH-1:0] a, b;
  wire \\esc+name ; assign \\esc+name = a[0];
  wire flag = \\esc+name ;
  item_t item; state_t state;

  sub #(.W(`WIDTH)) u_sub (.clk(clk), .din(a), .dout(b));
  sub u_plain (.clk, .din(b), .dout());
  other u_other (.clk(clk), .q());

  genvar g;
  generate
    for (g = 0; g < DEPTH; g++) begin : lanes
      logic [LIMIT-1:0] lane;
      assign lane = g;
    end
  endgenerate

  task automatic run(input int n);
    int total;
    total = twice(n) + thrice(1'b1);
    total = `MAX(total, `SUM(n));
  endtask

  Counter counter;
  initial begin
    counter = new();
    counter.step(2);
    state = BUSY;
    item.data = a;
    item.valid = 1'b1;
    after = `WIDTH;
    run(3);
`ifdef WIDTH
    $display(\"%s %0d\", text, after);
`endif
  end
endmodule
",
    ),
];

/// A server in this process over its own copy of the project, with every file
/// open.
struct Server {
    service: LspService<Backend>,
    root: PathBuf,
    texts: BTreeMap<&'static str, String>,
}

impl Server {
    async fn start(root: PathBuf, project: &[(&'static str, &str)]) -> Self {
        std::fs::create_dir_all(&root).expect("create the project");
        for (name, text) in project {
            std::fs::write(root.join(name), text).expect("write the project");
        }
        let (service, _socket) = LspService::new(Backend::new);
        let mut server = Self {
            service,
            root,
            texts: BTreeMap::new(),
        };
        let uri = Url::from_directory_path(&server.root).expect("root URI");
        let backend = server.backend();
        backend
            .initialize(InitializeParams {
                workspace_folders: Some(vec![WorkspaceFolder {
                    uri,
                    name: "project".to_owned(),
                }]),
                ..InitializeParams::default()
            })
            .await
            .expect("initialize");
        for (name, text) in project {
            server.texts.insert(name, (*text).to_owned());
            server
                .backend()
                .did_open(DidOpenTextDocumentParams {
                    text_document: TextDocumentItem {
                        uri: server.uri(name),
                        language_id: "systemverilog".to_owned(),
                        version: 1,
                        text: (*text).to_owned(),
                    },
                })
                .await;
        }
        server.backend().initialized(InitializedParams {}).await;
        server.settle().await;
        server
    }

    fn backend(&self) -> &Backend {
        self.service.inner()
    }

    fn uri(&self, name: &str) -> Url {
        Url::from_file_path(self.root.join(name)).expect("document URI")
    }

    /// Wait until the last analysis is one of the current texts.
    async fn settle(&self) {
        let start = Instant::now();
        loop {
            let settled = {
                let state = self.backend().lock_state();
                let mut roots = state.roots.values();
                roots.all(|root| {
                    root.running.is_none()
                        && state.documents.files().all(|(path, text)| {
                            root.analysed_texts
                                .get(path)
                                .is_some_and(|analysed| Arc::ptr_eq(analysed, text))
                        })
                })
            };
            if settled {
                return;
            }
            assert!(
                start.elapsed() < Duration::from_secs(60),
                "no analysis of {:#?}",
                self.texts
            );
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    }

    async fn change(&mut self, name: &'static str, text: &str) {
        self.texts.insert(name, text.to_owned());
        self.backend()
            .did_change(DidChangeTextDocumentParams {
                text_document: VersionedTextDocumentIdentifier {
                    uri: self.uri(name),
                    version: 2,
                },
                content_changes: vec![TextDocumentContentChangeEvent {
                    range: None,
                    range_length: None,
                    text: text.to_owned(),
                }],
            })
            .await;
    }

    async fn save(&self, name: &str) {
        self.backend()
            .did_save(DidSaveTextDocumentParams {
                text_document: TextDocumentIdentifier {
                    uri: self.uri(name),
                },
                text: None,
            })
            .await;
    }

    fn at(&self, name: &str, position: Position) -> TextDocumentPositionParams {
        TextDocumentPositionParams {
            text_document: TextDocumentIdentifier {
                uri: self.uri(name),
            },
            position,
        }
    }

    /// [`without_root`] of this server's root: every comparison of answer
    /// text that may name a file goes through it.
    fn without_root(&self, text: &str) -> String {
        without_root(&self.root, text)
    }

    /// A location with the name of its file.
    fn place(&self, location: &Location) -> Place {
        let path = location.uri.to_file_path().expect("a file");
        let relative = path
            .strip_prefix(&self.root)
            .unwrap_or_else(|_| panic!("a file of the project: {path:?}"));
        // Components, so a file in a subdirectory has one name on every host.
        let name: Vec<&str> = relative
            .iter()
            .map(|part| part.to_str().expect("UTF-8"))
            .collect();
        (name.join("/"), key(location.range))
    }

    async fn hover(&self, name: &str, position: Position) -> Option<(RangeKey, String)> {
        let hover = self
            .backend()
            .hover(HoverParams {
                text_document_position_params: self.at(name, position),
                work_done_progress_params: Default::default(),
            })
            .await
            .expect("hover")?;
        let HoverContents::Markup(contents) = hover.contents else {
            panic!("markup expected");
        };
        Some((
            key(hover.range.expect("a hover range")),
            self.without_root(&contents.value),
        ))
    }

    async fn references(&self, name: &str, position: Position) -> BTreeSet<Place> {
        self.backend()
            .references(ReferenceParams {
                text_document_position: self.at(name, position),
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
                context: ReferenceContext {
                    include_declaration: true,
                },
            })
            .await
            .expect("references")
            .unwrap_or_default()
            .iter()
            .map(|location| self.place(location))
            .collect()
    }

    async fn definitions(&self, name: &str, position: Position) -> BTreeSet<Place> {
        let answer = self
            .backend()
            .goto_definition(GotoDefinitionParams {
                text_document_position_params: self.at(name, position),
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            })
            .await
            .expect("definition");
        match answer {
            None => BTreeSet::new(),
            Some(GotoDefinitionResponse::Scalar(location)) => {
                BTreeSet::from([self.place(&location)])
            }
            Some(GotoDefinitionResponse::Array(locations)) => locations
                .iter()
                .map(|location| self.place(location))
                .collect(),
            Some(GotoDefinitionResponse::Link(_)) => panic!("links are not used"),
        }
    }

    async fn document_symbols(&self, name: &str) -> Vec<Symbol> {
        let answer = self
            .backend()
            .document_symbol(DocumentSymbolParams {
                text_document: TextDocumentIdentifier {
                    uri: self.uri(name),
                },
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            })
            .await
            .expect("documentSymbol");
        let mut symbols = Vec::new();
        match answer {
            None => {}
            Some(DocumentSymbolResponse::Nested(nested)) => flatten(nested, name, &mut symbols),
            Some(DocumentSymbolResponse::Flat(_)) => panic!("nested symbols expected"),
        }
        symbols
    }

    async fn workspace_symbols(&self) -> BTreeSet<(String, Place)> {
        self.backend()
            .symbol(WorkspaceSymbolParams::default())
            .await
            .expect("workspace/symbol")
            .unwrap_or_default()
            .iter()
            .map(|symbol| (symbol.name.clone(), self.place(&symbol.location)))
            .collect()
    }

    /// The entries of the module explorer that have a place, by their way in
    /// the tree.
    async fn explorer(&self) -> BTreeSet<(String, Place)> {
        let snapshot = self
            .backend()
            .module_explorer(Default::default())
            .await
            .expect("moduleExplorer");
        let mut found = BTreeSet::new();
        let value = serde_json::to_value(&snapshot).expect("a snapshot");
        self.explorer_places(&value, String::new(), &mut found);
        found
    }

    fn explorer_places(
        &self,
        value: &serde_json::Value,
        way: String,
        found: &mut BTreeSet<(String, Place)>,
    ) {
        match value {
            serde_json::Value::Array(values) => {
                for value in values {
                    self.explorer_places(value, way.clone(), found);
                }
            }
            serde_json::Value::Object(fields) => {
                let named = ["name", "instanceName"]
                    .iter()
                    .find_map(|name| fields.get(*name)?.as_str());
                let way = match named {
                    Some(name) => format!("{way}/{name}"),
                    None => way,
                };
                let uri = fields.get("uri").and_then(|uri| uri.as_str());
                let range = fields.get("range").and_then(|range| range.as_object());
                if let (Some(uri), Some(range)) = (uri, range) {
                    let number = |name: &str| range[name].as_u64().expect("a number") as u32;
                    let location = Location::new(
                        Url::parse(uri).expect("a URI"),
                        Range::new(
                            Position::new(number("startLine"), number("startCharacter")),
                            Position::new(number("endLine"), number("endCharacter")),
                        ),
                    );
                    found.insert((way.clone(), self.place(&location)));
                }
                for (field, value) in fields {
                    self.explorer_places(value, format!("{way}.{field}"), found);
                }
            }
            _ => {}
        }
    }

    async fn dump(&self, name: &str) -> Vec<String> {
        let mut lines = self
            .backend()
            .dump_tokens(DumpTokensParams {
                uri: self.uri(name),
            })
            .await
            .expect("dumpTokens")
            .lines;
        lines.retain(|line| !line.starts_with('#'));
        lines.iter().map(|line| self.without_root(line)).collect()
    }

    async fn completions(&self, name: &str, position: Position) -> BTreeSet<String> {
        let answer = self
            .backend()
            .completion(CompletionParams {
                text_document_position: self.at(name, position),
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
                context: None,
            })
            .await
            .expect("completion");
        match answer {
            Some(CompletionResponse::Array(items)) => {
                items.into_iter().map(|item| item.label).collect()
            }
            _ => BTreeSet::new(),
        }
    }

    /// The text at a place of the project.
    fn spelled(&self, (name, (start, end)): &Place) -> String {
        let text = &self.texts[name.as_str()];
        let line = SourcePositions::new(text)
            .lines()
            .nth(start.0 as usize)
            .unwrap_or_else(|| panic!("no line {start:?} in {name}: {text}"));
        assert_eq!(start.0, end.0, "a name in one line");
        let units: Vec<u16> = line.encode_utf16().collect();
        assert!(
            end.1 as usize <= units.len(),
            "{name} {start:?} {end:?}: {line}"
        );
        String::from_utf16_lossy(&units[start.1 as usize..end.1 as usize])
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// `text` without `root` and the separator after it wherever a path in it
/// starts with the root, in either separator (Windows hovers spell paths with
/// `\`), so that answers of servers over different roots compare equal on
/// every host. Other backslashes (escaped identifiers) stay.
fn without_root(root: &std::path::Path, text: &str) -> String {
    let root = root.to_str().expect("UTF-8");
    let root = root.trim_end_matches(['/', '\\']);
    let mut text = text.to_owned();
    for spelling in [root.replace('\\', "/"), root.replace('/', "\\")] {
        for separator in ['/', '\\'] {
            text = text.replace(&format!("{spelling}{separator}"), "");
        }
    }
    text
}

#[test]
fn answers_compare_without_their_root_in_either_separator() {
    let root = std::path::Path::new("/tmp/llg_positions_edited_1");
    for text in [
        "defined at /tmp/llg_positions_edited_1/defs.svh:2",
        "defined at \\tmp\\llg_positions_edited_1\\defs.svh:2",
    ] {
        assert_eq!(without_root(root, text), "defined at defs.svh:2");
    }
    assert_eq!(without_root(root, "\\esc+name"), "\\esc+name");
    assert_eq!(
        without_root(root, "defined at /tmp/llg_positions_fresh_1/defs.svh:2"),
        "defined at /tmp/llg_positions_fresh_1/defs.svh:2"
    );
}

type RangeKey = (PosKey, PosKey);
type Place = (String, RangeKey);

/// A document symbol's name, the place of the name and the range.
type Symbol = (String, Place, RangeKey);

fn key(range: Range) -> RangeKey {
    (pos_key(range.start), pos_key(range.end))
}

fn flatten(symbols: Vec<DocumentSymbol>, file: &str, found: &mut Vec<Symbol>) {
    for symbol in symbols {
        let name = (file.to_owned(), key(symbol.selection_range));
        found.push((symbol.name, name, key(symbol.range)));
        flatten(symbol.children.unwrap_or_default(), file, found);
    }
}

/// Numbers that look random (xorshift).
struct Random(u64);

impl Random {
    fn new(seed: u64) -> Self {
        Self(0x9e37_79b9_7f4a_7c15_u64.wrapping_mul(seed + 1))
    }

    fn below(&mut self, end: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 % end.max(1) as u64) as usize
    }
}

/// The places of the words in a text.
fn words(text: &str) -> Vec<RangeKey> {
    let mut words = Vec::new();
    for (number, line) in SourcePositions::new(text).lines().enumerate() {
        let mut column = 0;
        let mut start = None;
        for character in line.chars().chain([' ']) {
            match (is_word(character) || character == '\\', start) {
                (true, None) => start = Some(column),
                (false, Some(first)) => {
                    let line = number as u32;
                    words.push(((line, first), (line, column)));
                    start = None;
                }
                _ => {}
            }
            column += character.len_utf16() as u32;
        }
    }
    words
}

/// The lines of a text that occur once in it and once in `other`, so they were
/// not edited.
fn kept_lines(text: &str, other: &str) -> BTreeSet<u32> {
    let mut counts: HashMap<&str, [usize; 2]> = HashMap::new();
    for (index, text) in [text, other].into_iter().enumerate() {
        for line in SourcePositions::new(text).lines() {
            counts.entry(line.trim()).or_default()[index] += 1;
        }
    }
    SourcePositions::new(text)
        .lines()
        .enumerate()
        .filter(|(_, line)| counts[line.trim()] == [1, 1] && line.contains(is_word))
        .map(|(number, _)| number as u32)
        .collect()
}

fn lines_of(text: &str) -> (Vec<String>, &'static str) {
    let ending = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let mut lines: Vec<String> = SourcePositions::new(text)
        .lines()
        .map(str::to_owned)
        .collect();
    lines.pop();
    (lines, ending)
}

fn text_of(lines: &[String], ending: &str) -> String {
    lines.iter().flat_map(|line| [line, ending]).collect()
}

/// Whether another line may not come between the line before `at` and the one
/// there.
fn continues(lines: &[String], at: usize) -> bool {
    at > 0 && lines[at - 1].ends_with('\\')
}

fn is_comment(line: &str) -> bool {
    line.trim().is_empty() || line.trim_start().starts_with("//")
}

/// Edit a text without a change to what it says.
fn rearrange(text: &str, random: &mut Random, fresh: &mut usize) -> String {
    let (mut lines, ending) = lines_of(text);
    *fresh += 1;
    let at = random.below(lines.len());
    let directive = lines[at].trim_start().starts_with('`');
    let continued = lines[at].ends_with('\\') || continues(&lines, at);
    match random.below(10) {
        0 | 1 if !continues(&lines, at) => {
            lines.insert(at, format!("  // nöte 𝄞 {fresh}"));
        }
        2 if !continues(&lines, at) => lines.insert(at, String::new()),
        3 => {
            if let Some(comment) = (0..lines.len())
                .map(|offset| (at + offset) % lines.len())
                .find(|line| is_comment(&lines[*line]))
            {
                lines.remove(comment);
            }
        }
        4 if !continued => lines[at].push_str(&format!(" // é{fresh}")),
        5 if !continued => {
            let trimmed = lines[at].trim_start().to_owned();
            lines[at] = format!("{}{trimmed}", ["", "\t", "      "][random.below(3)]);
        }
        6 if !directive && !continued => {
            let indentation = lines[at].len() - lines[at].trim_start().len();
            lines[at].insert_str(indentation, &format!("/* 𝄞{fresh} */ "));
        }
        7 if !directive && !is_comment(&lines[at]) => {
            // Not in the string.
            if let Some(split) = lines[at].find("; ").filter(|_| !lines[at].contains('"')) {
                let rest = lines[at].split_off(split + 1);
                lines.insert(at + 1, rest);
            }
        }
        8 if at + 1 < lines.len() => {
            let joins = |line: &str| !line.contains('`') && !line.contains("//");
            if joins(&lines[at]) && joins(&lines[at + 1]) {
                let next = lines.remove(at + 1);
                lines[at].push(' ');
                lines[at].push_str(&next);
            }
        }
        9 => {
            let ending = if ending == "\n" { "\r\n" } else { "\n" };
            return text_of(&lines, ending);
        }
        _ => {}
    }
    text_of(&lines, ending)
}

/// Edit a text in any way.
fn edit(text: &str, random: &mut Random, fresh: &mut usize) -> String {
    let (mut lines, ending) = lines_of(text);
    *fresh += 1;
    if lines.is_empty() {
        return text.to_owned();
    }
    let at = random.below(lines.len());
    match random.below(8) {
        0 => {
            lines.remove(at);
        }
        1 => {
            let line = lines[random.below(lines.len())].clone();
            lines.insert(at, line);
        }
        2 => {
            let other = random.below(lines.len());
            lines.swap(at, other);
        }
        3 => {
            let end = at + random.below(lines.len() - at).min(6);
            let moved: Vec<String> = lines.drain(at..end).collect();
            let to = random.below(lines.len() + 1);
            lines.splice(to..to, moved);
        }
        4 => lines.insert(at, format!("  logic fresh{fresh};")),
        _ => {
            let characters: Vec<char> = lines[at].chars().collect();
            let from = random.below(characters.len() + 1);
            let to = (from + random.below(4)).min(characters.len());
            let put = ["", "x", "\\", "é", " ", "`", "a_b"][random.below(7)];
            lines[at] = characters[..from]
                .iter()
                .copied()
                .chain(put.chars())
                .chain(characters[to..].iter().copied())
                .collect();
        }
    }
    text_of(&lines, ending)
}

/// The hover without the place of the definition, and that place.
fn split_hover(hover: &str) -> (String, Vec<&str>) {
    let is_place = |line: &&str| line.starts_with("defined at ");
    let places = hover.lines().filter(is_place).collect();
    let rest = hover
        .lines()
        .filter(|line| !is_place(line) && !line.is_empty())
        .collect();
    (rest, places)
}

fn run(test: impl std::future::Future<Output = ()>) {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("test runtime")
        .block_on(test);
}

fn project_root(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("llg_positions_{name}_{}", std::process::id()))
}

/// Compare what both servers answer, of which one analysed the texts and the
/// other analysed `analysed`.
async fn compare(
    edited: &Server,
    fresh: &Server,
    analysed: &BTreeMap<&'static str, String>,
    random: &mut Random,
    context: &str,
) -> usize {
    let kept: BTreeMap<&str, BTreeSet<u32>> = analysed
        .iter()
        .map(|(name, text)| (*name, kept_lines(&edited.texts[name], text)))
        .collect();
    let is_kept = |place: &Place| {
        kept[place.0.as_str()].contains(&place.1 .0 .0) && place.1 .0 .0 == place.1 .1 .0
    };
    let same_places = |what: &str, old: &BTreeSet<Place>, new: &BTreeSet<Place>| {
        let wrong: Vec<&Place> = old.difference(new).collect();
        assert!(wrong.is_empty(), "{context}: {what}: not {wrong:?}");
        let missing: Vec<&Place> = new.difference(old).filter(|place| is_kept(place)).collect();
        assert!(missing.is_empty(), "{context}: {what}: missing {missing:?}");
    };
    let mut answered = 0;
    for (name, text) in &edited.texts {
        let words = words(text);
        for _ in 0..40 {
            let (start, end) = words[random.below(words.len())];
            let column = start.1 + random.below((end.1 - start.1 + 1) as usize) as u32;
            let position = Position::new(start.0, column);
            let what = format!("{name} {start:?}..{end:?} at {position:?}");
            let query = ((*name).to_owned(), (start, end));
            let old = edited.hover(name, position).await;
            let new = fresh.hover(name, position).await;
            match (&old, &new) {
                (Some((range, old)), Some((expected, new))) => {
                    assert_eq!(range, expected, "{context}: hover range of {what}");
                    let ((old, place), (new, expected)) = (split_hover(old), split_hover(new));
                    assert_eq!(old, new, "{context}: hover of {what}");
                    assert!(
                        place.is_empty() || place == expected,
                        "{context}: hover of {what}: {place:?} not {expected:?}"
                    );
                    answered += 1;
                }
                (Some(old), None) => panic!("{context}: hover of {what}: {old:?}"),
                (None, Some(new)) => {
                    assert!(!is_kept(&query), "{context}: hover of {what}: no {new:?}");
                }
                (None, None) => {}
            }
            let old = edited.references(name, position).await;
            let new = fresh.references(name, position).await;
            if is_kept(&query) {
                same_places(&format!("references of {what}"), &old, &new);
            } else {
                let wrong: Vec<&Place> = old.difference(&new).collect();
                assert!(
                    wrong.is_empty(),
                    "{context}: references of {what}: not {wrong:?}"
                );
            }
            let old = edited.definitions(name, position).await;
            let new = fresh.definitions(name, position).await;
            same_places(&format!("definitions of {what}"), &old, &new);
        }
        let old = edited.document_symbols(name).await;
        let new = fresh.document_symbols(name).await;
        for symbol in &old {
            assert!(
                new.iter()
                    .any(|other| symbol.0 == other.0 && symbol.1 == other.1),
                "{context}: documentSymbol {symbol:?} not among {new:?}"
            );
        }
        for symbol in new.iter().filter(|symbol| is_kept(&symbol.1)) {
            let found = old
                .iter()
                .find(|other| symbol.0 == other.0 && symbol.1 == other.1);
            let Some(found) = found else {
                panic!("{context}: documentSymbol {symbol:?} missing in {old:?}");
            };
            let (start, end) = symbol.2;
            assert_eq!(found.2 .0, start, "{context}: start of {symbol:?}");
            // `endmodule` is in an unedited line or the end was cut.
            if kept[*name].contains(&end.0) {
                assert_eq!(found.2 .1, end, "{context}: end of {symbol:?}");
            } else {
                assert!(found.2 .1 <= end, "{context}: end of {symbol:?}: {found:?}");
            }
        }
        let old = edited.dump(name).await;
        let new: BTreeSet<String> = fresh.dump(name).await.into_iter().collect();
        for row in &old {
            assert!(new.contains(row), "{context}: dumpTokens {row}");
        }
        let position = Position::new(0, 0);
        assert_eq!(
            edited.completions(name, position).await,
            fresh.completions(name, position).await,
            "{context}: completion in {name}"
        );
    }
    let named = |found: BTreeSet<(String, Place)>| -> BTreeMap<String, BTreeSet<Place>> {
        let mut places: BTreeMap<String, BTreeSet<Place>> = BTreeMap::new();
        for (name, place) in found {
            places.entry(name).or_default().insert(place);
        }
        places
    };
    for (request, old, new) in [
        (
            "workspace/symbol",
            edited.workspace_symbols().await,
            fresh.workspace_symbols().await,
        ),
        (
            "moduleExplorer",
            edited.explorer().await,
            fresh.explorer().await,
        ),
    ] {
        let (old, new) = (named(old), named(new));
        for name in old.keys().chain(new.keys()) {
            let none = BTreeSet::new();
            let (old, new) = (
                old.get(name).unwrap_or(&none),
                new.get(name).unwrap_or(&none),
            );
            if request == "moduleExplorer"
                && old
                    .iter()
                    .chain(new)
                    .any(|(_, range)| range.0 .0 != range.1 .0)
            {
                // A module, whose end may be cut.
                let starts = |places: &BTreeSet<Place>| -> BTreeSet<(String, PosKey)> {
                    places
                        .iter()
                        .map(|place| (place.0.clone(), place.1 .0))
                        .collect()
                };
                assert!(starts(old).is_subset(&starts(new)), "{context}: {name}");
                continue;
            }
            same_places(&format!("{request} {name}"), old, new);
        }
    }
    answered
}

/// Edits that change nothing a request tells: whatever is answered below them
/// is what a server that analysed the text answers, and what is in unedited
/// lines is answered.
#[test]
fn rearranged_texts_answer_like_analysed_ones() {
    let _shadow = TEST_PROCESS_SHADOW_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    run(async {
        for seed in 0..3 {
            let mut random = Random::new(seed);
            let mut edited = Server::start(project_root("edited"), &PROJECT).await;
            let mut fresh = Server::start(project_root("fresh"), &PROJECT).await;
            let analysed = edited.texts.clone();
            let (mut number, mut answered) = (0, 0);
            for step in 0..5 {
                for _ in 0..1 + random.below(4) {
                    let (name, _) = PROJECT[random.below(PROJECT.len())];
                    let text = match random.below(12) {
                        // Undo.
                        0 => analysed[name].clone(),
                        _ => rearrange(&edited.texts[name], &mut random, &mut number),
                    };
                    edited.change(name, &text).await;
                    fresh.change(name, &text).await;
                }
                for (name, _) in PROJECT {
                    fresh.save(name).await;
                }
                fresh.settle().await;
                let context = format!("seed {seed} step {step}");
                answered += compare(&edited, &fresh, &analysed, &mut random, &context).await;
            }
            assert!(answered > 100, "seed {seed}: only {answered} hovers");
            for (name, _) in PROJECT {
                let text = analysed[name].clone();
                edited.change(name, &text).await;
            }
            edited.settle().await;
        }
    });
}

/// Edits of the project that change what some names mean.
const CHANGES: [(&str, &str, &str); 12] = [
    (TOP, "  logic clk;\n", "  logic clk;\n  logic fresh;\n"),
    (TOP, "    run(3);\n", ""),
    (TOP, "total", "sum_total"),
    (TOP, "u_plain", "u_pl"),
    (TOP, "  genvar g;\n", "  genvar g;\n  int v;\n  initial v = LIMIT;\n"),
    (
        "pkg.sv",
        "  function automatic int thrice",
        "  function automatic int quad(int v);\n    return v * 4;\n  endfunction\n  function automatic int thrice",
    ),
    ("pkg.sv", "twice(int v);\n", "twice(int v);\n    int half;\n"),
    ("pkg.sv", "    return v * 2;\n", "    return 2 * v + v;\n"),
    ("sub.sv", "    q <= !q;\n", "    q <= ~q;\n"),
    ("sub.sv", "// A register.\n", "// A register.\nmodule empty;\nendmodule\n\n"),
    (DEFS, "`define WIDTH 8\n", "`define WIDTH 8\n`define HEIGHT 4\n"),
    (DEFS, "localparam int LIMIT", "localparam int LOW = 1;\nlocalparam int LIMIT"),
];

/// The words of the lines that an edit of a project touched, and of those that
/// occur more than once.
fn touched_words(
    analysed: &BTreeMap<&'static str, String>,
    current: &BTreeMap<&'static str, String>,
) -> BTreeSet<String> {
    let mut touched = BTreeSet::new();
    for (name, text) in analysed {
        for (text, other) in [(text, &current[name]), (&current[name], text)] {
            let kept = kept_lines(text, other);
            for (number, line) in SourcePositions::new(text).lines().enumerate() {
                if !kept.contains(&(number as u32)) {
                    let words = line.split(|character| !is_word(character));
                    touched.extend(words.map(str::to_owned));
                }
            }
        }
    }
    touched
}

/// Edits that change what some names mean: the other names are answered as by a
/// server that analysed the text.
#[test]
fn unchanged_names_are_answered_like_analysed_ones() {
    let _shadow = TEST_PROCESS_SHADOW_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    run(async {
        let mut edited = Server::start(project_root("changed"), &PROJECT).await;
        let mut fresh = Server::start(project_root("changed_fresh"), &PROJECT).await;
        let analysed = edited.texts.clone();
        for seed in 0..10 {
            let mut random = Random::new(seed);
            let mut number = 0;
            let mut texts = analysed.clone();
            for _ in 0..2 + random.below(4) {
                let (name, from, to) = CHANGES[random.below(CHANGES.len())];
                let text = texts.get_mut(name).expect("a file");
                if !text.contains(to) {
                    *text = text.replace(from, to);
                }
                let (name, _) = PROJECT[random.below(PROJECT.len())];
                let text = texts.get_mut(name).expect("a file");
                *text = rearrange(text, &mut random, &mut number);
            }
            for (name, text) in &texts {
                edited.change(name, text).await;
                fresh.change(name, text).await;
                fresh.save(name).await;
            }
            fresh.settle().await;
            let touched = touched_words(&analysed, &texts);
            let mut compared = 0;
            for (name, text) in &texts {
                let kept = kept_lines(text, &analysed[name]);
                for (start, end) in words(text) {
                    let place = ((*name).to_owned(), (start, end));
                    let word = plain(&edited.spelled(&place)).to_owned();
                    if !kept.contains(&start.0) || touched.contains(&word) {
                        continue;
                    }
                    let position = Position::new(start.0, start.1);
                    let what = format!("seed {seed}: {word} at {name} {start:?}");
                    let old = edited.hover(name, position).await;
                    let new = fresh.hover(name, position).await;
                    match (&old, &new) {
                        (Some((range, old)), Some((expected, new))) => {
                            assert_eq!(range, expected, "{what}");
                            assert_eq!(split_hover(old), split_hover(new), "{what}");
                            compared += 1;
                        }
                        (None, None) => {}
                        _ => panic!("{what}: {old:?} {new:?}"),
                    }
                    assert_eq!(
                        edited.references(name, position).await,
                        fresh.references(name, position).await,
                        "{what}"
                    );
                }
            }
            assert!(compared > 20, "seed {seed}: only {compared} hovers");
        }
    });
}

/// What the server has for a document.
fn placement(server: &Server, name: &str) -> &'static str {
    let mut state = server.backend().lock_state();
    let root = state.roots.values().next().expect("a root");
    let texts = Arc::clone(&root.analysed_texts);
    match state.placement(&texts, &server.root.join(name)) {
        Placement::Same => "same",
        Placement::Edited(_) => "edited",
        Placement::Unknown => "unknown",
    }
}

fn after(text: &str, needle: &str) -> Position {
    let offset = text.find(needle).expect("the needle") + needle.len();
    let lines: Vec<&str> = SourcePositions::new(&text[..offset]).lines().collect();
    let last = lines[lines.len() - 1].encode_utf16().count();
    Position::new((lines.len() - 1) as u32, last as u32)
}

/// Only the texts that the analysis compiled relate positions: an edit that
/// lands while it runs is one of its text, a document that another program or
/// an earlier session left different has none until it is analysed, and one
/// whose text was rejected is not the one that the server has.
#[test]
fn positions_follow_the_text_that_was_analysed() {
    let _shadow = TEST_PROCESS_SHADOW_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    run(async {
        let first = "module m;\n  logic [7:0] a;\nendmodule\n";
        let mut server = Server::start(project_root("followed"), &[("m.sv", first)]).await;
        assert_eq!(placement(&server, "m.sv"), "same");
        server.change("m.sv", &first.replacen("7", "3", 1)).await;
        server.save("m.sv").await;
        let start = Instant::now();
        while server
            .backend()
            .lock_state()
            .roots
            .values()
            .all(|root| root.running.is_none())
        {
            assert!(start.elapsed() < Duration::from_secs(60), "no analysis");
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
        let text = format!("// note\n{}", first.replacen("7", "3", 1));
        server.change("m.sv", &text).await;
        while server
            .backend()
            .lock_state()
            .roots
            .values()
            .any(|root| root.running.is_some())
        {
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
        assert_eq!(placement(&server, "m.sv"), "edited");
        let hover = server.hover("m.sv", after(&text, "] ")).await;
        assert!(
            hover
                .as_ref()
                .is_some_and(|hover| hover.1.contains("[3:0]")),
            "{hover:?}"
        );

        // Too large for the server, which keeps the text before it.
        let large = " ".repeat(2 << 20);
        let before = server.texts.clone();
        server.change("m.sv", &large).await;
        assert_eq!(placement(&server, "m.sv"), "unknown");
        assert_eq!(server.hover("m.sv", after(&text, "] ")).await, None);
        assert!(server.document_symbols("m.sv").await.is_empty());
        assert!(server
            .completions("m.sv", Position::new(0, 0))
            .await
            .is_empty());
        server.texts = before;
        server.change("m.sv", &text).await;
        assert_eq!(placement(&server, "m.sv"), "edited");
        assert_eq!(server.hover("m.sv", after(&text, "] ")).await, hover);

        // Closed, the analysed text is forgotten, and so is the edit.
        let document = |server: &Server| TextDocumentIdentifier {
            uri: server.uri("m.sv"),
        };
        let close = DidCloseTextDocumentParams {
            text_document: document(&server),
        };
        server.backend().did_close(close).await;
        assert_eq!(placement(&server, "m.sv"), "same");
        let open = |text: &str| DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: server.uri("m.sv"),
                language_id: "systemverilog".to_owned(),
                version: 1,
                text: text.to_owned(),
            },
        };
        server.backend().did_open(open(&text)).await;
        assert_eq!(placement(&server, "m.sv"), "unknown");
        assert_eq!(server.hover("m.sv", after(&text, "] ")).await, None);
        server.settle().await;
        assert_eq!(placement(&server, "m.sv"), "same");
        assert!(server.hover("m.sv", after(&text, "] ")).await.is_some());

        // A document that opens with the analysed text needs no analysis.
        let close = DidCloseTextDocumentParams {
            text_document: document(&server),
        };
        server.backend().did_close(close).await;
        server.backend().did_open(open(&text)).await;
        assert_eq!(placement(&server, "m.sv"), "same");
    });
}

/// A character before a name makes it another one: `` `a `` is a macro, `\a;` an
/// escaped identifier that goes on to white space.
#[test]
fn a_prefix_makes_another_name() {
    let _shadow = TEST_PROCESS_SHADOW_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    run(async {
        let text = "module m;\n  logic a;\n  assign a = 1'b0;\nendmodule\n";
        let mut server = Server::start(project_root("prefixed"), &[("m.sv", text)]).await;
        let position = after(text, "assign a");
        assert!(server.hover("m.sv", position).await.is_some());
        for prefix in ["`", "\\", "b", "$", "1"] {
            let edited = text.replacen("assign a", &format!("assign {prefix}a"), 1);
            server.change("m.sv", &edited).await;
            let position = after(&edited, &format!("assign {prefix}a"));
            assert_eq!(server.hover("m.sv", position).await, None, "{edited}");
            assert!(server.references("m.sv", position).await.is_empty());
            let declaration = after(&edited, "logic a");
            let references = server.references("m.sv", declaration).await;
            assert_eq!(references.len(), 1, "{edited}: {references:?}");
        }
        server
            .change("m.sv", &text.replacen("assign a", "assign  a", 1))
            .await;
        assert!(server
            .hover("m.sv", after(text, "assign a"))
            .await
            .is_some());
    });
}

/// Over the budget the document that was just edited loses its analysed text,
/// and nothing is answered for it.
#[test]
fn analysed_texts_stay_within_their_budget() {
    let large = ANALYSED_TEXT_BUDGET / 2 + 1;
    let root = PathBuf::from(crate::test_paths::host_path("/r"));
    let paths = [root.join("a.sv"), root.join("b.sv")];
    let uris = paths
        .each_ref()
        .map(|path| Url::from_file_path(path).expect("a URI"));
    let config = root.join(crate::config::CONFIG_FILE);
    let (root_state, _) = Backend::root_state(root.clone(), "r".to_owned(), config);
    let mut state = BackendState::default();
    state.roots.insert(root, root_state);
    for uri in &uris {
        state.set_document(uri.clone(), "a".repeat(large));
    }
    let texts: AnalysedTexts = paths
        .iter()
        .map(|path| {
            (
                path.clone(),
                Arc::clone(state.documents.of_file(path).expect("open")),
            )
        })
        .collect();
    let analysed = |state: &BackendState| {
        let root = state.roots.values().next().expect("a root");
        paths
            .each_ref()
            .map(|path| root.analysed_texts.contains_key(path))
    };
    state
        .roots
        .values_mut()
        .next()
        .expect("a root")
        .analysed_texts = Arc::new(texts.clone());

    state.set_document(uris[0].clone(), "b".to_owned());
    assert_eq!(analysed(&state), [true, true]);
    state.set_document(uris[1].clone(), "b".to_owned());
    assert_eq!(analysed(&state), [true, false]);
    let texts = Arc::clone(&state.roots.values().next().expect("a root").analysed_texts);
    assert!(matches!(
        state.placement(&texts, &paths[0]),
        Placement::Edited(_)
    ));
    assert!(matches!(
        state.placement(&texts, &paths[1]),
        Placement::Unknown
    ));

    // Undone, the first one costs nothing again.
    state.set_document(uris[0].clone(), "a".repeat(large));
    assert!(state.placement(&texts, &paths[0]).is_same());
}

/// The name in the text of a token: `` `name `` is the use of a macro, and
/// white space ends `\name `.
fn plain(spelled: &str) -> &str {
    spelled.trim_start_matches(['`', '\\'])
}

/// Whether a place spells the name. The places of escaped identifiers begin
/// with the backslash, and some have the length of the name without it.
fn spells(spelled: &str, name: &str) -> bool {
    let name = plain(name);
    plain(spelled) == name
        || spelled
            .strip_prefix('\\')
            .is_some_and(|cut| name.len() == cut.len() + 1 && name.starts_with(cut))
}

/// Edits of every kind: an answer is about the identifier at the position that
/// was asked about, and what it names has that name in the current texts.
#[test]
fn no_edit_makes_an_answer_name_another_identifier() {
    let _shadow = TEST_PROCESS_SHADOW_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    run(async {
        let mut server = Server::start(project_root("any_edit"), &PROJECT).await;
        let analysed = server.texts.clone();
        let facts = hover_facts(&server).await;
        for seed in 0..30 {
            let mut random = Random::new(seed);
            let (mut number, mut answered) = (0, 0);
            for (name, text) in &analysed {
                server.change(name, text).await;
            }
            for step in 0..6 {
                for _ in 0..1 + random.below(3) {
                    let (name, _) = PROJECT[random.below(PROJECT.len())];
                    let text = edit(&server.texts[name], &mut random, &mut number);
                    server.change(name, &text).await;
                }
                let context = format!("seed {seed} step {step}");
                answered += check_names(&server, &facts, &mut random, &context).await;
            }
            assert!(answered > 0, "seed {seed}: no answer");
        }
        for (name, text) in &analysed {
            server.change(name, text).await;
        }
        server.settle().await;
    });
}

/// What an unedited project says about the names in it, by name.
async fn hover_facts(server: &Server) -> BTreeSet<(String, String)> {
    let mut facts = BTreeSet::new();
    for (name, text) in &server.texts {
        for (start, _) in words(text) {
            let position = Position::new(start.0, start.1);
            if let Some((range, value)) = server.hover(name, position).await {
                let spelled = server.spelled(&((*name).to_owned(), range));
                facts.insert((plain(&spelled).to_owned(), split_hover(&value).0));
            }
        }
    }
    facts
}

async fn check_names(
    server: &Server,
    facts: &BTreeSet<(String, String)>,
    random: &mut Random,
    context: &str,
) -> usize {
    let mut answered = 0;
    for (name, text) in &server.texts {
        let words = words(text);
        if words.is_empty() {
            continue;
        }
        for _ in 0..12 {
            let (start, end) = words[random.below(words.len())];
            let column = start.1 + random.below((end.1 - start.1 + 1) as usize) as u32;
            let position = Position::new(start.0, column);
            let what = format!("{context}: {name} {position:?} in\n{text}");
            let hover = server.hover(name, position).await;
            let references = server.references(name, position).await;
            if let Some((range, value)) = &hover {
                answered += 1;
                assert!(
                    range.0 <= pos_key(position) && pos_key(position) <= range.1,
                    "{what}: hover at {range:?}"
                );
                let spelled = server.spelled(&((*name).to_owned(), *range));
                let word = plain(&spelled);
                let fact = (word.to_owned(), split_hover(value).0);
                assert!(facts.contains(&fact), "{what}: hover {fact:?}");
                for place in &references {
                    assert!(
                        spells(&server.spelled(place), word),
                        "{what}: reference {place:?} of {spelled}"
                    );
                }
            }
            let asked = server.spelled(&((*name).to_owned(), (start, end)));
            for place in &references {
                // The word may be one of several in an escaped identifier.
                let spelled = server.spelled(place);
                assert!(
                    spells(&spelled, &asked) || text.contains('\\'),
                    "{what}: reference {place:?} of {asked}"
                );
            }
            for place in server.definitions(name, position).await {
                // Its file is the target of an include.
                if place.1 != ((0, 0), (0, 0)) {
                    server.spelled(&place);
                }
            }
        }
        for (symbol, place, range) in server.document_symbols(name).await {
            assert!(
                range.0 <= place.1 .0 && place.1 .1 <= range.1,
                "{context}: {symbol} {place:?} {range:?}"
            );
            let spelled = server.spelled(&place);
            assert!(
                spells(&spelled, &symbol),
                "{context}: documentSymbol {symbol} at {spelled} in\n{text}"
            );
        }
        for row in server.dump(name).await {
            let (place, rest) = row.split_once('\t').expect("a name");
            let word = rest.split('\t').next().expect("a name");
            let (_, columns) = place.rsplit_once(':').expect("columns");
            let (before, _) = place.rsplit_once(':').expect("a line");
            let (_, line) = before.rsplit_once(':').expect("a line");
            let (start, end) = columns.split_once('-').expect("columns");
            let number = |text: &str| text.parse::<u32>().expect("a number");
            let line = number(line);
            let place = ((line, number(start)), (line, number(end)));
            assert_eq!(
                server.spelled(&((*name).to_owned(), place)),
                word,
                "{context}: {row}"
            );
        }
    }
    for (symbol, place) in server.workspace_symbols().await {
        let spelled = server.spelled(&place);
        assert!(
            spells(&spelled, &symbol),
            "{context}: workspace/symbol {symbol} at {spelled}"
        );
    }
    for (way, place) in server.explorer().await {
        // A module's range goes on after its name.
        if place.1 .0 .0 == place.1 .1 .0 && !way.starts_with(".modules/") {
            server.spelled(&place);
        }
    }
    answered
}
