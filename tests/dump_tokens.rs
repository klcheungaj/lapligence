//! Integration tests for `llg --dump-tokens`.
//!
//! Each test runs the built binary over a copy of `tests/fixtures/dump/proj`
//! and asserts targeted facts about the emitted rows rather than byte-golden
//! output. Slang provides one typed lexical record per occurrence, so
//! position-keyed assertions apply directly to that record.
//!
//! The asserted facts double as machine-checkable navigation-correctness
//! statements: duplicate identifiers disambiguate per module, named port
//! connections bind the LABEL to the child module's port declaration while
//! the connected ACTUAL binds to its OWN declaration in the instantiating
//! (parent) scope (labels tagged `via=label`, actuals tagged
//! `via=connection`), and unbound occurrences print `bind=-`.
//!
//! [`json_golden_matches_fixture`] additionally pins the WHOLE dump of the
//! `proj` fixture as a reviewable JSON document
//! (`tests/fixtures/dump/expected.json`, schema `llg.tokenDump/v1`): every
//! text row becomes one JSON entry enriched with module attribution, and the
//! result must deep-equal the golden (order-sensitive, sorted as produced).
//! [`module_inst_json_golden_matches_fixture`] pins the committed
//! `module_inst.v` regression fixture (parent `top` and child `adder`
//! declaring IDENTICAL port names clk/din/dout) the same way in
//! `expected.module_inst.json`.  Regenerate either with
//! `LLG_DUMP_BLESS=1 cargo test --test dump_tokens`; see the `_comment`
//! key inside each golden for the exact field conventions.
#![cfg(feature = "lsp")]

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{json, Value};

/// Copy the fixture tree into a fresh temp dir and return its root.
fn materialize_fixture() -> PathBuf {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/dump/proj");
    let root = std::env::temp_dir().join(format!(
        "llg-dump-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos()
    ));
    copy_tree(&source, &root);
    root
}

fn copy_tree(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).expect("create destination dir");
    for entry in fs::read_dir(source).expect("read fixture dir") {
        let entry = entry.expect("dir entry");
        let target = destination.join(entry.file_name());
        if entry.file_type().expect("file type").is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).expect("copy file");
        }
    }
}

struct Dump {
    lines: Vec<String>,
}

impl Dump {
    fn run(root: &Path) -> Self {
        let output = Command::new(env!("CARGO_BIN_EXE_llg_ls"))
            .arg("--dump-tokens")
            .arg(root)
            .output()
            .expect("launch llg --dump-tokens");
        assert!(output.status.success(), "dump exited {:?}", output.status);
        let text = String::from_utf8(output.stdout).expect("utf8 stdout");
        Self {
            lines: text.lines().map(str::to_owned).collect(),
        }
    }

    /// Every data row whose location starts with `loc_prefix`.
    fn rows_at(&self, loc_prefix: &str) -> Vec<&str> {
        self.lines
            .iter()
            .map(String::as_str)
            .filter(|line| line.starts_with(loc_prefix) && !line.starts_with('#'))
            .collect()
    }

    /// Asserts at least one row exists at `loc_prefix` and EVERY such row
    /// contains each of `needles`.
    fn assert_rows(&self, loc_prefix: &str, needles: &[&str]) {
        let rows = self.rows_at(loc_prefix);
        assert!(
            !rows.is_empty(),
            "no dump row at {loc_prefix} (lines: {:?})",
            self.lines
        );
        for row in rows {
            for needle in needles {
                assert!(
                    row.contains(needle),
                    "row at {loc_prefix} missing {needle:?}: {row}"
                );
            }
        }
    }

    /// Asserts at least one row exists at `loc_prefix` and NO such row
    /// contains any of `needles`.
    fn assert_rows_without(&self, loc_prefix: &str, needles: &[&str]) {
        let rows = self.rows_at(loc_prefix);
        assert!(
            !rows.is_empty(),
            "no dump row at {loc_prefix} (lines: {:?})",
            self.lines
        );
        for row in rows {
            for needle in needles {
                assert!(
                    !row.contains(needle),
                    "row at {loc_prefix} must not contain {needle:?}: {row}"
                );
            }
        }
    }

    fn summary(&self) -> &str {
        self.lines
            .iter()
            .map(String::as_str)
            .find(|line| line.starts_with("# analysis:"))
            .expect("summary line")
    }
}

#[test]
fn dump_runs_deterministically_and_reports_the_project() {
    let root = materialize_fixture();
    let first = Dump::run(&root);
    let second = Dump::run(&root);
    assert_eq!(first.lines, second.lines, "dump must be deterministic");
    assert_eq!(
        first.lines.first().map(String::as_str),
        Some("# llg token dump root=. files=4")
    );
    let summary = first.summary();
    assert!(
        summary.contains("outcome=valid") && summary.contains("modules=3"),
        "unexpected summary: {summary}"
    );
    fs::remove_dir_all(&root).ok();
}

/// THE goto-definition complaint, expressed as data: identical identifiers in
/// different modules must bind to THEIR OWN module's declaration.
#[test]
fn duplicate_identifiers_bind_to_their_own_module() {
    let root = materialize_fixture();
    let dump = Dump::run(&root);

    // `clk` USE inside m_a.sv (assign line) → m_a's port decl only.
    dump.assert_rows("m_a.sv:3:13", &["REF", "bind=m_a.sv:1:23[clk"]);
    // …and the same-named use inside m_b.sv → m_b's port decl only.
    dump.assert_rows("m_b.sv:3:17", &["REF", "bind=m_b.sv:1:23[clk"]);

    // Same story for the duplicated `busy` signals.
    dump.assert_rows("m_a.sv:3:20", &["REF", "bind=m_a.sv:2:8[busy"]);
    dump.assert_rows("m_b.sv:3:9", &["REF", "bind=m_b.sv:2:8[busy"]);
    fs::remove_dir_all(&root).ok();
}

/// Port-list navigation: the LABEL of a named port connection (`.clk`)
/// navigates to the CHILD module's port declaration (`via=label`) while the
/// connected ACTUAL signal navigates to its OWN declaration in the
/// instantiating (parent) scope (`via=connection`) — never into the child
/// module, even where parent and child declare identical names.
///
/// Semantic surface: the label rows carry the `connectionLabel` modifier
/// (`sym=function/connectionLabel`) while the connected ACTUAL rows stay
/// plain `variable` with no `connectionLabel`.
#[test]
fn port_connections_bind_labels_to_child_ports_and_actuals_to_parent_scope() {
    let root = materialize_fixture();
    let dump = Dump::run(&root);

    // u_a(.clk(wa), .q(t_q)) — labels → m_a ports …
    dump.assert_rows(
        "tb.sv:6:11",
        &["REF", "via=label", "bind=m_a.sv:1:23[clk,port]"],
    );
    dump.assert_rows(
        "tb.sv:6:21",
        &["REF", "via=label", "bind=m_a.sv:1:41[q,port]"],
    );
    // … while the actuals resolve to tb.sv's OWN declarations.
    dump.assert_rows(
        "tb.sv:6:15",
        &["REF", "via=connection", "bind=tb.sv:2:8[wa,variable]"],
    );
    dump.assert_rows(
        "tb.sv:6:23",
        &["REF", "via=connection", "bind=tb.sv:4:8[t_q,variable]"],
    );
    // u_b(.clk(wb)) — same split for the second instantiation.
    dump.assert_rows(
        "tb.sv:7:11",
        &["REF", "via=label", "bind=m_b.sv:1:23[clk,port]"],
    );
    dump.assert_rows(
        "tb.sv:7:15",
        &["REF", "via=connection", "bind=tb.sv:3:8[wb,variable]"],
    );

    // Highlighting: labels carry `connectionLabel`, actuals do not.
    for loc in ["tb.sv:6:11", "tb.sv:6:21", "tb.sv:7:11"] {
        dump.assert_rows(loc, &["sym=function/connectionLabel"]);
    }
    for loc in ["tb.sv:6:15", "tb.sv:6:23", "tb.sv:7:15"] {
        dump.assert_rows_without(loc, &["connectionLabel"]);
        dump.assert_rows(loc, &["sym=variable"]);
    }

    fs::remove_dir_all(&root).ok();
}

#[test]
fn declarations_are_marked_and_sorted() {
    let root = materialize_fixture();
    let dump = Dump::run(&root);

    // Spot-check declarations carry DECL with sane coordinates.
    dump.assert_rows("m_a.sv:1:7", &["DECL"]); // module m_a
    dump.assert_rows("m_a.sv:1:23", &["DECL"]); // input logic clk
    dump.assert_rows("m_a.sv:2:8", &["DECL"]); // logic busy
    dump.assert_rows("a_pkg.sv:2:16", &["DECL"]); // parameter W

    // Row order is non-decreasing by (file, line0, col0).
    let data_rows: Vec<&String> = dump
        .lines
        .iter()
        .filter(|line| !line.starts_with('#'))
        .collect();
    let mut keys: Vec<(String, u32, u32)> = Vec::new();
    for row in &data_rows {
        let (loc, _rest) = row.split_once('\t').expect("tab-separated row");
        let (file, pos) = loc.split_once(':').expect("file:line:col");
        let mut parts = pos.splitn(2, ':');
        let line: u32 = parts.next().unwrap().parse().unwrap();
        let col: u32 = parts
            .next()
            .unwrap()
            .split('-')
            .next()
            .unwrap()
            .parse()
            .unwrap();
        keys.push((file.to_owned(), line, col));
    }
    let mut sorted = keys.clone();
    sorted.sort();
    assert_eq!(keys, sorted, "rows must be sorted by (file, line, col)");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn unbound_occurrences_print_an_explicit_marker() {
    let root = materialize_fixture();
    let dump = Dump::run(&root);

    // Declarations have no target binding and render an explicit `bind=-`
    // rather than an empty field. Module type uses, connection actuals, and
    // labels all resolve to their semantic declarations.
    let module_declarations = dump.rows_at("m_a.sv:1:7");
    assert!(
        module_declarations.iter().any(|row| row.contains("bind=-")),
        "module declaration should be explicit about having no target binding"
    );
    fs::remove_dir_all(&root).ok();
}

/// THE production regression: a syntax-broken sibling file must NOT starve
/// the other files' token surface.  With `top.v` next to an unterminated
/// module, Slang's recovery snapshot must still surface all seven identifier
/// occurrences of top.v —
/// module name, both port declarations, the reg declaration, and three
/// references — each with the correct DECL/REF role.
#[test]
fn syntax_broken_sibling_still_dumps_all_top_v_tokens() {
    let root = std::env::temp_dir().join(format!(
        "llg-dump-mixed-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos()
    ));
    fs::create_dir_all(&root).expect("create mixed fixture dir");
    fs::write(
        root.join("top.v"),
        "\nmodule top(\n    input clk,\n    output dat\n);\n\nreg[31:0] counter;\n\nalways @(posedge clk) begin\n    counter <= dat;\nend\n\nendmodule\n",
    )
    .expect("write top.v");
    // Unterminated module on purpose: this is the single syntax fault.
    fs::write(root.join("broken.v"), "module broken(\n   input clk\n").expect("write broken.v");

    let output = Command::new(env!("CARGO_BIN_EXE_llg_ls"))
        .arg("--dump-tokens")
        .arg(&root)
        .output()
        .expect("launch llg --dump-tokens");
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).expect("utf8 stdout");
    let dump = Dump {
        lines: text.lines().map(str::to_owned).collect(),
    };

    assert!(
        dump.summary().contains("outcome=parse"),
        "{:?}",
        dump.summary()
    );

    // All four declaration sites.
    dump.assert_rows("top.v:1:7", &["top", "DECL"]);
    dump.assert_rows("top.v:2:10", &["clk", "DECL"]);
    dump.assert_rows("top.v:3:11", &["dat", "DECL"]);
    dump.assert_rows("top.v:6:10", &["counter", "DECL"]);

    // All three references, correctly marked REF (never DECL).
    dump.assert_rows("top.v:8:17", &["clk", "REF"]);
    dump.assert_rows("top.v:9:4", &["counter", "REF"]);
    dump.assert_rows("top.v:9:15", &["dat", "REF"]);

    // Slang retains exact bindings for the valid unit even though its sibling
    // is syntax-broken. Declarations remain binding targets, while each
    // reference points to its declaration in `top`.
    let top_rows = dump.rows_at("top.v:");
    assert!(top_rows.len() >= 7, "expected ≥7 rows, got {:?}", top_rows);
    dump.assert_rows("top.v:1:7", &["DECL", "bind=-"]);
    dump.assert_rows("top.v:2:10", &["DECL", "bind=-"]);
    dump.assert_rows("top.v:3:11", &["DECL", "bind=-"]);
    dump.assert_rows("top.v:6:10", &["DECL", "bind=-"]);
    dump.assert_rows("top.v:8:17", &["REF", "bind=top.v:2:10[clk,port]"]);
    dump.assert_rows("top.v:9:4", &["REF", "bind=top.v:6:10[counter,variable]"]);
    dump.assert_rows("top.v:9:15", &["REF", "bind=top.v:3:11[dat,port]"]);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn single_file_invocation_filters_rows() {
    let root = materialize_fixture();
    let output = Command::new(env!("CARGO_BIN_EXE_llg_ls"))
        .arg("--dump-tokens")
        .arg(root.join("m_b.sv"))
        .output()
        .expect("launch llg --dump-tokens <file>");
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).expect("utf8 stdout");
    assert!(
        text.contains("\nm_b.sv:") || text.contains("m_b.sv:1:"),
        "expected m_b rows: {text}"
    );
    assert!(!text.contains("\nm_a.sv:"), "m_a rows must be filtered out");
    fs::remove_dir_all(&root).ok();
}

// ── JSON golden (schema llg.tokenDump/v1) ────────────────────────────────────

/// Reviewed golden document compared against a freshly computed one.
fn golden_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/dump/expected.json")
}

/// Build the golden-schema document from a fresh text dump over `root`.
///
/// The converter is deliberately test-side: it consumes ONLY the stable CLI
/// text format, so the golden cannot drift from what users actually see.
/// Module attribution recovers the enclosing scope spans lexically from the
/// fixture sources (flat files, one declaration per scope), matching the
/// containment semantics of the model's module spans.
fn golden_document(dump: &Dump, root: &Path, fixture: &str, comment: &str) -> Value {
    let mut scopes: HashMap<String, Vec<ScopeSpan>> = HashMap::new();
    let mut tokens = Vec::new();
    for line in &dump.lines {
        if line.starts_with('#') {
            continue;
        }
        let mut token = data_row_to_json(line);
        let file = token["file"].as_str().expect("file field").to_owned();
        let line0 = token["line"].as_u64().expect("line field") as u32;
        let spans = scopes.entry(file.clone()).or_insert_with(|| {
            let source = fs::read_to_string(root.join(file)).expect("fixture source");
            scan_scope_spans(&source)
        });
        token["module"] = json!(enclosing_scope_name(spans, line0));
        tokens.push(token);
    }
    let summary = parse_summary(dump.summary());
    json!({
        "_comment": comment,
        "schema": "llg.tokenDump/v1",
        "fixture": fixture,
        "summary": summary,
        "tokens": tokens,
    })
}

/// Conventions of a golden document, shipped inside it as `_comment`.
fn golden_comment(golden_test: &str) -> String {
    format!(
        "Golden for tests/dump_tokens.rs::{golden_test} \
(schema llg.tokenDump/v1); regenerate with LLG_DUMP_BLESS=1 cargo test --test dump_tokens. \
Conventions: coordinates are the SAME 0-based values the --dump-tokens text prints; one source \
occurrence appears once as a typed Slang lexical record; entries are in \
text-dump emission order (sorted by file, line0, col0); \"module\" is the innermost module or \
package body in the same file whose line range contains the entry — package bodies render as \
\"package:<name>\", rows outside every scope render \"\"; \"sym\" is the semantic-token legend \
string \"type[/mod1+mod2...]\" or null when no token covers the position; \"binding\" is null \
for unbound rows (bind=-) and otherwise the resolved declaration target, with viaLabel marking \
named-port-connection label folds (target = the child module's port) and viaConnection marking \
the connected-signal side of the same fold (target = the actual's own declaration in the \
instantiating/parent scope). Entries carrying \"knownIssue\": true reproduce \
suspicious-but-current output verbatim (assessments live in the repository report); the \
comparator ignores this marker."
    )
}

#[test]
fn json_golden_matches_fixture() {
    let root = materialize_fixture();
    let dump = Dump::run(&root);
    let computed = golden_document(
        &dump,
        &root,
        "proj",
        &golden_comment("json_golden_matches_fixture"),
    );

    let golden = golden_path();
    if bless_requested() {
        write_golden(&golden, &computed);
        fs::remove_dir_all(&root).ok();
        return;
    }

    compare_golden(&golden, &computed);
    fs::remove_dir_all(&root).ok();
}

/// The committed single-file regression fixture for the connection-ACTUAL
/// semantics: parent `top` and child `adder` declare IDENTICAL port names
/// clk/din/dout, so labels must still reach the child ports while the
/// actuals must stay on top's own declarations.
fn materialize_module_inst_fixture() -> PathBuf {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/dump/module_inst.v");
    let root = std::env::temp_dir().join(format!(
        "llg-dump-modinst-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos()
    ));
    fs::create_dir_all(&root).expect("create fixture dir");
    fs::copy(&source, root.join("module_inst.v")).expect("copy module_inst.v");
    root
}

fn module_inst_golden_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/dump/expected.module_inst.json")
}

/// Golden for the reference `module_inst.v` fixture: pins label≠actual
/// targets for every `.port(actual)` of the instantiation.
#[test]
fn module_inst_json_golden_matches_fixture() {
    let root = materialize_module_inst_fixture();
    let dump = Dump::run(&root);
    let computed = golden_document(
        &dump,
        &root,
        "module_inst",
        &golden_comment("module_inst_json_golden_matches_fixture"),
    );

    // Hand-checks over the computed document BEFORE blessing/comparison:
    // labels → adder ports, actuals → top's own ports.
    let rows = |line: u64, col: u64| -> Vec<&Value> {
        computed["tokens"]
            .as_array()
            .expect("tokens array")
            .iter()
            .filter(|t| t["file"] == "module_inst.v" && t["line"] == line && t["col"] == col)
            .collect()
    };
    let assert_binding = |line: u64, col: u64, want_line: u64, want_col: u64, want_via: &str| {
        for token in rows(line, col) {
            let binding = token["binding"]
                .as_object()
                .unwrap_or_else(|| panic!("expected binding at {line}:{col}: {token}"));
            assert_eq!(
                (binding["line"].as_u64(), binding["col"].as_u64()),
                (Some(want_line), Some(want_col)),
                "wrong target at {line}:{col}: {token}"
            );
            let (label, connection, kind, sym) = match want_via {
                "label" => (
                    true,
                    false,
                    "port-connection-label",
                    "function/connectionLabel",
                ),
                "connection" => (false, true, "port", "parameter/readonly"),
                _ => panic!("unknown via {want_via}"),
            };
            assert_eq!(
                (&binding["viaLabel"], &binding["viaConnection"]),
                (&json!(label), &json!(connection)),
                "wrong provenance at {line}:{col}: {token}"
            );
            // Both modules declare ports here. Pin their source-level identity
            // before blessing can accidentally accept the internal net kind.
            assert_eq!(binding["targetKind"], "port", "wrong target kind: {token}");
            assert_eq!(token["tokenKind"], kind, "wrong token kind: {token}");
            assert_eq!(token["sym"], sym, "wrong semantic token: {token}");
        }
        assert!(!rows(line, col).is_empty(), "no rows at {line}:{col}");
    };
    // Labels `.clk/.din/.dout` (0-based 25:5 / 26:5 / 27:5) → adder ports.
    assert_binding(25, 5, 2, 10, "label");
    assert_binding(26, 5, 3, 10, "label");
    assert_binding(27, 5, 4, 11, "label");
    // Actuals clk/din/dout (25:9 / 26:9 / 27:10) → TOP's own ports.
    assert_binding(25, 9, 18, 10, "connection");
    assert_binding(26, 9, 19, 10, "connection");
    assert_binding(27, 10, 20, 11, "connection");

    let golden = module_inst_golden_path();
    if bless_requested() {
        write_golden(&golden, &computed);
        fs::remove_dir_all(&root).ok();
        return;
    }

    compare_golden(&golden, &computed);
    fs::remove_dir_all(&root).ok();
}

fn bless_requested() -> bool {
    std::env::var("LLG_DUMP_BLESS").as_deref() == Ok("1")
}

fn write_golden(path: &Path, document: &Value) {
    let pretty = serde_json::to_string_pretty(document).expect("serialize golden");
    fs::write(path, pretty + "\n").expect("write golden");
    println!("blessed token-dump golden: {}", path.display());
}

fn compare_golden(path: &Path, computed: &Value) {
    let raw = fs::read_to_string(path).expect("golden file (bless first: LLG_DUMP_BLESS=1)");
    let mut expected: Value = serde_json::from_str(&raw).expect("golden parses as JSON");
    strip_known_issue_markers(&mut expected);
    assert_eq!(
        serde_json::to_string_pretty(computed).expect("serialize computed"),
        serde_json::to_string_pretty(&expected).expect("serialize expected"),
        "dump output diverged from {}",
        path.display()
    );
}

/// Hand-curated `knownIssue` flags live only in the committed golden; both
/// sides are compared without them so re-blessing stays lossless apart from
/// re-reviewing flagged entries.
fn strip_known_issue_markers(document: &mut Value) {
    if let Some(tokens) = document.get_mut("tokens").and_then(Value::as_array_mut) {
        for token in tokens {
            if let Some(object) = token.as_object_mut() {
                object.remove("knownIssue");
            }
        }
    }
}

// ── Text-row decoding ─────────────────────────────────────────────────────────

/// Convert one text dump row (`file:l:c-e\tname\tkind=…\tDECL|REF\tsym=…\t
/// bind=…[\tvias]`) into its golden-schema JSON object.
fn data_row_to_json(row: &str) -> Value {
    let fields: Vec<&str> = row.split('\t').collect();

    let (file, position) = fields[0].split_once(':').expect("file:line:col-end");
    let (line, span) = position.split_once(':').expect("line:col-end");
    let (col, end_col) = span.split_once('-').expect("col-end");

    let token_kind = fields
        .iter()
        .find_map(|field| field.strip_prefix("kind="))
        .expect("kind field")
        .to_owned();
    let role = fields
        .iter()
        .find(|field| **field == "DECL" || **field == "REF")
        .copied()
        .expect("DECL|REF field");
    let sym = fields
        .iter()
        .find_map(|field| field.strip_prefix("sym="))
        .map(|value| {
            if value == "-" {
                Value::Null
            } else {
                Value::String(value.to_owned())
            }
        })
        .expect("sym field");

    let bind_raw = fields
        .iter()
        .find_map(|field| field.strip_prefix("bind="))
        .expect("bind field");
    let via_label = fields.contains(&"via=label");
    let via_connection = fields.contains(&"via=connection");
    let binding = binding_to_json(bind_raw, via_label, via_connection);

    json!({
        "identifier": fields[1],
        "file": file,
        "line": line.parse::<u32>().expect("numeric line"),
        "col": col.parse::<u32>().expect("numeric col"),
        "endCol": end_col.parse::<u32>().expect("numeric endCol"),
        "tokenKind": token_kind,
        "role": role,
        "module": "",
        "sym": sym,
        "binding": binding,
    })
}

/// Decode `bind=<file>:<line>:<col>[<name>,<kind>]` (or `-` → null).
fn binding_to_json(raw: &str, via_label: bool, via_connection: bool) -> Value {
    if raw == "-" {
        return Value::Null;
    }
    let (target, bracketed) = raw.split_once('[').expect("bind target[detail]");
    let detail = bracketed.strip_suffix(']').expect("closing bracket");
    let (name, kind) = detail.split_once(',').expect("name,kind");
    let mut parts = target.rsplitn(3, ':');
    let col = parts.next().expect("bind col");
    let line = parts.next().expect("bind line");
    let file = parts.next().expect("bind file");
    json!({
        "file": file,
        "line": line.parse::<u32>().expect("numeric bind line"),
        "col": col.parse::<u32>().expect("numeric bind col"),
        "identifier": name,
        "targetKind": kind,
        "viaLabel": via_label,
        "viaConnection": via_connection,
    })
}

/// `# analysis: outcome=… modules=… bindings=…` → summary object.
fn parse_summary(line: &str) -> Value {
    let payload = line.strip_prefix("# analysis: ").expect("analysis prefix");
    let mut outcome = "";
    let mut modules = 0u64;
    let mut bindings = 0u64;
    for field in payload.split_whitespace() {
        if let Some(value) = field.strip_prefix("outcome=") {
            outcome = value;
        } else if let Some(value) = field.strip_prefix("modules=") {
            modules = value.parse().expect("numeric modules");
        } else if let Some(value) = field.strip_prefix("bindings=") {
            bindings = value.parse().expect("numeric bindings");
        }
    }
    json!({ "outcome": outcome, "modules": modules, "bindings": bindings })
}

// ── Module attribution ────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
enum ScopeKind {
    Module,
    Package,
}

/// Contiguous 0-based inclusive line range of one `module`/`package` body.
struct ScopeSpan {
    kind: ScopeKind,
    name: String,
    first_line: u32,
    last_line: u32,
}

/// Recover module/package line ranges from fixture source text.
///
/// Dump fixtures are flat (one declaration per scope, no nesting), so a
/// lexical scan of the declaration/end keywords reproduces the same
/// containment spans the model carries for its modules/packages.
fn scan_scope_spans(source: &str) -> Vec<ScopeSpan> {
    const OPENERS: &[(&str, ScopeKind)] = &[
        ("macromodule", ScopeKind::Module),
        ("module", ScopeKind::Module),
        ("package", ScopeKind::Package),
    ];
    let mut open: Vec<ScopeSpan> = Vec::new();
    let mut closed: Vec<ScopeSpan> = Vec::new();
    for (index, raw_line) in source.lines().enumerate() {
        let code = raw_line.split("//").next().unwrap_or("");
        let words: Vec<&str> = code
            .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .filter(|word| !word.is_empty())
            .collect();
        for (position, word) in words.iter().enumerate() {
            if let Some((_, kind)) = OPENERS.iter().find(|(opener, _)| opener == word) {
                if let Some(name) = words.get(position + 1) {
                    open.push(ScopeSpan {
                        kind: *kind,
                        name: (*name).to_owned(),
                        first_line: index as u32,
                        last_line: index as u32,
                    });
                }
            } else if *word == "endmodule" || *word == "endpackage" {
                if let Some(span) = open.pop() {
                    closed.push(span);
                }
            }
        }
        for span in &mut open {
            span.last_line = index as u32;
        }
    }
    closed.append(&mut open);
    closed.sort_by_key(|span| (span.first_line, span.last_line));
    closed
}

/// Name of the innermost scope whose line range contains `line`: packages
/// render as `package:<name>`, modules bare, nothing → `""`.
fn enclosing_scope_name(spans: &[ScopeSpan], line: u32) -> String {
    let mut best: Option<&ScopeSpan> = None;
    for span in spans {
        if span.first_line <= line && line <= span.last_line {
            let tighter = best.is_none_or(|best: &ScopeSpan| {
                span.last_line - span.first_line < best.last_line - best.first_line
            });
            if tighter {
                best = Some(span);
            }
        }
    }
    match best {
        Some(span) => match span.kind {
            ScopeKind::Package => format!("package:{}", span.name),
            ScopeKind::Module => span.name.clone(),
        },
        None => String::new(),
    }
}
