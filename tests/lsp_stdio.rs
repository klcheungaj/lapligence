//! Process-level LSP stdio contract tests.
//!
//! These tests deliberately use only the wire protocol.  They are the
//! acceptance surface for `llg.toml`-driven workspace discovery, multi-root
//! indexing, dynamic file watching, config reload, include authorization and
//! the last-good navigation behavior used while a buffer has an error.  The
//! backend must add `serde_json` as a dev dependency for this integration
//! test.
#![cfg(feature = "lsp")]

use std::fs;
#[path = "lsp_stdio/genvar.rs"]
mod genvar;
mod support;

use std::io;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};
use support::lsp::{default_init_options, file_uri, LspProcess};

use serde_json::{json, Value};

#[path = "lsp_stdio/diagnostics.rs"]
mod diagnostics;
#[path = "lsp_stdio/workspace.rs"]
mod workspace;
#[path = "lsp_stdio/configuration.rs"]
mod configuration;
#[path = "lsp_stdio/dependencies.rs"]
mod dependencies;
#[path = "lsp_stdio/recovery.rs"]
mod recovery;
#[path = "lsp_stdio/semantic_tokens.rs"]
mod semantic_tokens;
#[path = "lsp_stdio/limits.rs"]
mod limits;
#[path = "lsp_stdio/lifecycle.rs"]
mod lifecycle;
#[path = "lsp_stdio/hover_completion.rs"]
mod hover_completion;
#[path = "lsp_stdio/definitions.rs"]
mod definitions;
use definitions::single_location;
#[path = "lsp_stdio/connections.rs"]
mod connections;
#[path = "lsp_stdio/tokens.rs"]
mod tokens;
#[path = "lsp_stdio/module_graph.rs"]
mod module_graph;
#[path = "lsp_stdio/lint.rs"]
mod lint;
#[path = "lsp_stdio/rename.rs"]
mod rename;
use rename::{TempDirCleanup, rename_workspace, edits_for, start_key};
#[path = "lsp_stdio/performance.rs"]
mod performance;
use performance::memo_cache_stats;
#[path = "lsp_stdio/parameter_hover.rs"]
mod parameter_hover;
use parameter_hover::{param_hover_markup, value_line};
#[path = "lsp_stdio/inactive_ranges.rs"]
mod inactive_ranges;
#[path = "lsp_stdio/macro_hover.rs"]
mod macro_hover;


const FIXTURE_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/lsp");
const MODULE_EXPLORER_FIXTURE_DIR: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/lsp/module-explorer"
);
const SOURCE_HEADER: &str = "// llg-lsp-fixture:";
const CONFIG_FILE: &str = "llg.toml";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(45);
const POLL_TIMEOUT: Duration = Duration::from_secs(60);
const POLL_REQUEST_TIMEOUT: Duration = Duration::from_millis(750);
const POLL_INTERVAL: Duration = Duration::from_millis(100);

static NEXT_FIXTURE_ID: AtomicU64 = AtomicU64::new(0);

// ── Fixture materialization ──────────────────────────────────────────────────

struct FixtureTree {
    root: PathBuf,
}

impl FixtureTree {
    fn new() -> Self {
        Self::from_source(Path::new(FIXTURE_DIR), validate_fixture)
    }

    fn module_explorer() -> Self {
        Self::from_source(
            Path::new(MODULE_EXPLORER_FIXTURE_DIR),
            validate_module_explorer_fixture,
        )
    }

    fn from_source(source: &Path, validate: fn(&Path)) -> Self {
        validate(source);

        let id = NEXT_FIXTURE_ID.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!("llg-lsp-stdio-{}-{id}", std::process::id()));
        fs::create_dir_all(&root).expect("create LSP fixture directory");
        copy_tree(source, &root).expect("copy LSP fixture tree");
        Self { root }
    }

    fn root(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }

    /// The fixture tree base — also the CWD the spawned server inherits.
    fn base(&self) -> &Path {
        &self.root
    }
}

impl Drop for FixtureTree {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn copy_tree(source: &Path, destination: &Path) -> io::Result<()> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_tree(&source_path, &destination_path)?;
        } else {
            fs::copy(source_path, destination_path)?;
        }
    }
    Ok(())
}

fn validate_fixture(root: &Path) {
    validate_fixture_manifest(root, "stdio-workspace", 3, 2);
}

fn validate_module_explorer_fixture(root: &Path) {
    validate_fixture_manifest(root, "module-explorer", 1, 1);
}

fn validate_fixture_manifest(
    root: &Path,
    expected_name: &str,
    expected_root_count: usize,
    expected_initial_roots: usize,
) {
    let manifest_path = root.join("test.json");
    let manifest_text = fs::read_to_string(&manifest_path).expect("read LSP fixture manifest");
    let manifest: Value = serde_json::from_str(&manifest_text).expect("parse LSP fixture manifest");
    let object = manifest
        .as_object()
        .expect("LSP fixture manifest must be an object");
    assert_eq!(
        object.get("schema").and_then(Value::as_str),
        Some("llg.lsp.fixture/v1"),
        "unsupported LSP fixture schema"
    );
    assert_eq!(
        object.get("name").and_then(Value::as_str),
        Some(expected_name),
        "unexpected LSP fixture name"
    );
    assert_eq!(
        object.get("source_header").and_then(Value::as_str),
        Some(SOURCE_HEADER),
        "source-header convention changed without updating the harness"
    );

    let roots = object
        .get("roots")
        .and_then(Value::as_array)
        .expect("LSP fixture roots must be an array");
    assert_eq!(
        roots.len(),
        expected_root_count,
        "fixture root count changed without updating the harness"
    );

    let mut initial_roots = 0;
    for root_spec in roots {
        let root_spec = root_spec
            .as_object()
            .expect("each fixture root must be an object");
        let path = root_spec
            .get("path")
            .and_then(Value::as_str)
            .expect("fixture root path");
        let root_path = relative_path(root, path);
        assert!(root_path.is_dir(), "fixture root does not exist: {path}");
        let config_path = root_path.join(CONFIG_FILE);
        assert!(
            config_path.is_file(),
            "fixture root must ship an effective {CONFIG_FILE}: {path}"
        );
        if root_spec
            .get("initial")
            .and_then(Value::as_bool)
            .expect("fixture root initial flag")
        {
            initial_roots += 1;
        }
    }
    assert_eq!(
        expected_initial_roots, initial_roots,
        "fixture initial-root count changed without updating the harness"
    );

    validate_source_headers(root, root);
}

fn relative_path(base: &Path, relative: &str) -> PathBuf {
    let path = Path::new(relative);
    assert!(
        !path.is_absolute(),
        "fixture paths must be relative: {relative}"
    );
    assert!(
        path.components()
            .all(|component| !matches!(component, Component::ParentDir)),
        "fixture paths must not escape their root: {relative}"
    );
    base.join(path)
}

fn validate_source_headers(root: &Path, relative_to: &Path) {
    for entry in fs::read_dir(root).expect("read fixture directory") {
        let entry = entry.expect("read fixture entry");
        // The module-explorer fixture has its own manifest and is validated
        // relative to that manifest by its dedicated acceptance test.
        if root == relative_to && entry.file_name() == "module-explorer" {
            continue;
        }
        let path = entry.path();
        if entry.file_type().expect("fixture entry type").is_dir() {
            validate_source_headers(&path, relative_to);
        } else if is_hdl_extension(&path) {
            let relative = path
                .strip_prefix(relative_to)
                .expect("fixture path under fixture root")
                .to_string_lossy()
                .replace(std::path::MAIN_SEPARATOR, "/");
            assert_source_header(&path, &relative);
        }
    }
}

fn is_hdl_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| matches!(ext, "v" | "sv" | "vh" | "svh"))
}

fn assert_source_header(path: &Path, relative: &str) {
    let text = fs::read_to_string(path).expect("read HDL fixture source");
    let first_line = text.lines().next().unwrap_or_default();
    assert_eq!(
        first_line,
        format!("{SOURCE_HEADER} {relative}"),
        "HDL fixture source header must identify its repository-relative path: {}",
        path.display()
    );
}

/// Build an initialization payload with explicit per-root config-file
/// overrides.  Each entry is `(workspace_uri, config_path)`.
fn init_options_with_config_files(config_files: &[(&str, &str)]) -> Value {
    json!({
        "llg": {
            "protocolVersion": 1,
            "configFiles": config_files.iter().map(|(uri, path)| json!({
                "workspaceUri": uri,
                "path": path
            })).collect::<Vec<_>>()
        }
    })
}

fn names(value: &Value) -> Vec<String> {
    let mut result = Vec::new();
    collect_names(value, &mut result);
    result
}

fn collect_names(value: &Value, result: &mut Vec<String>) {
    match value {
        Value::Object(object) => {
            if let Some(name) = object.get("name").and_then(Value::as_str) {
                result.push(name.to_owned());
            }
            for child in object.values() {
                collect_names(child, result);
            }
        }
        Value::Array(array) => {
            for child in array {
                collect_names(child, result);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

fn assert_has_name(value: &Value, expected: &str) {
    let names = names(value);
    assert!(
        names.iter().any(|name| name == expected),
        "expected symbol {expected:?}, got {names:?}; response={value}"
    );
}

fn is_shadow_uri(uri: &str) -> bool {
    // The process shadow base is `<tmp>/llg-<pid>-<rand>/...`, distinct from
    // the fixture dir `<tmp>/llg-lsp-stdio-<pid>-<id>/...`.  Detect a path
    // component shaped like the shadow base (`llg-` + digits + `-` + digits).
    let Some(path) = Path::new(uri.trim_start_matches("file://")).to_str() else {
        return false;
    };
    path.split(std::path::MAIN_SEPARATOR).any(|component| {
        let Some(rest) = component.strip_prefix("llg-") else {
            return false;
        };
        let Some((first, second)) = rest.split_once('-') else {
            return false;
        };
        !first.is_empty() && first.bytes().all(|b| b.is_ascii_digit()) && !second.is_empty()
    })
}

fn assert_no_shadow_uris(value: &Value) {
    match value {
        Value::Object(object) => {
            if let Some(uri) = object.get("uri").and_then(Value::as_str) {
                assert!(
                    uri.starts_with("file:"),
                    "LSP location is not a file URI: {uri}"
                );
                assert!(
                    !is_shadow_uri(uri) && !uri.contains("lsp-shadow"),
                    "shadow URI leaked to client: {uri}"
                );
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

/// Paths of `<tmp>/llg-{pid}-*` shadow-staging directories for `pid`.
///
/// Mirrors the leak scan the Node E2E performs (`listTmpLlgShadowDirs`
/// filtered by the server pid).
fn tmp_llg_shadow_dirs_for(pid: u32) -> Vec<PathBuf> {
    let prefix = format!("llg-{pid}-");
    let mut found = Vec::new();
    if let Ok(entries) = fs::read_dir(std::env::temp_dir()) {
        for entry in entries.flatten() {
            if entry.file_name().to_string_lossy().starts_with(&prefix) {
                found.push(entry.path());
            }
        }
    }
    found
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

fn has_location_start(value: &Value, start: &Value) -> bool {
    value.as_array().is_some_and(|locations| {
        locations.iter().any(|location| {
            location
                .get("range")
                .and_then(|range| range.get("start"))
                .is_some_and(|candidate| candidate == start)
        })
    })
}

fn has_lint_rule(params: &Value, rule: &str) -> bool {
    params
        .get("diagnostics")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .any(|diagnostic| {
            diagnostic.get("source").and_then(Value::as_str) == Some("llg-lint")
                && diagnostic.get("code").and_then(Value::as_str) == Some(rule)
        })
}

fn lint_severity(params: &Value, rule: &str) -> Option<u64> {
    params
        .get("diagnostics")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find(|diagnostic| {
            diagnostic.get("source").and_then(Value::as_str) == Some("llg-lint")
                && diagnostic.get("code").and_then(Value::as_str) == Some(rule)
        })
        .and_then(|diagnostic| diagnostic.get("severity"))
        .and_then(Value::as_u64)
}

fn semantic_token_positions(result: &Value) -> Vec<(u64, u64)> {
    let data = result
        .get("data")
        .and_then(Value::as_array)
        .expect("semantic token result data");
    assert_eq!(
        data.len() % 5,
        0,
        "semantic token data must contain complete 5-tuples"
    );
    let mut line = 0;
    let mut character = 0;
    let mut positions = Vec::with_capacity(data.len() / 5);
    for token in data.as_chunks::<5>().0 {
        let delta_line = token[0].as_u64().expect("semantic token deltaLine");
        let delta_start = token[1].as_u64().expect("semantic token deltaStart");
        if delta_line == 0 {
            character += delta_start;
        } else {
            line += delta_line;
            character = delta_start;
        }
        positions.push((line, character));
    }
    positions
}

/// One decoded absolute-position semantic-token row with its legend names.
#[derive(Debug)]
struct SemanticRow {
    line: u64,
    character: u64,
    token_type: String,
    modifiers: Vec<String>,
}

/// Legend strings of one `legend` object (`tokenTypes` / `tokenModifiers`).
fn legend_names(legend: &Value, key: &str) -> Vec<String> {
    legend
        .get(key)
        .and_then(Value::as_array)
        .expect("legend array")
        .iter()
        .map(|value| value.as_str().expect("legend string").to_owned())
        .collect()
}

/// Decode a delta-encoded semantic-tokens response into absolute rows,
/// resolving type/modifier indices through the server's reported legend.
fn semantic_token_rows(
    result: &Value,
    legend_types: &[String],
    legend_modifiers: &[String],
) -> Vec<SemanticRow> {
    let data = result
        .get("data")
        .and_then(Value::as_array)
        .expect("semantic token result data");
    assert_eq!(
        data.len() % 5,
        0,
        "semantic token data must contain complete 5-tuples"
    );
    let mut line = 0;
    let mut character = 0;
    let mut rows = Vec::with_capacity(data.len() / 5);
    for token in data.as_chunks::<5>().0 {
        let delta_line = token[0].as_u64().expect("semantic token deltaLine");
        let delta_start = token[1].as_u64().expect("semantic token deltaStart");
        if delta_line == 0 {
            character += delta_start;
        } else {
            line += delta_line;
            character = delta_start;
        }
        let type_index = token[3].as_u64().expect("semantic token type") as usize;
        let modifier_bits = token[4].as_u64().expect("semantic token modifiers");
        rows.push(SemanticRow {
            line,
            character,
            token_type: legend_types[type_index].clone(),
            modifiers: legend_modifiers
                .iter()
                .enumerate()
                .filter(|(bit, _)| modifier_bits & (1 << bit) != 0)
                .map(|(_, name)| name.clone())
                .collect(),
        });
    }
    rows
}

fn row_at(rows: &[SemanticRow], line: u64, character: u64) -> &SemanticRow {
    rows.iter()
        .find(|row| row.line == line && row.character == character)
        .unwrap_or_else(|| panic!("no semantic token at {line}:{character}: {rows:?}"))
}

/// 0-based `(line, character)` of the `occurrence`-th (0-based) match of
/// `needle` in `text`, counting characters like LSP columns.
fn position_of(text: &str, needle: &str, occurrence: usize) -> (u64, u64) {
    let mut starts = Vec::new();
    let mut from = 0;
    while let Some(relative) = text[from..].find(needle) {
        let at = from + relative;
        starts.push(at);
        from = at + 1;
    }
    let byte_index = *starts
        .get(occurrence)
        .unwrap_or_else(|| panic!("missing occurrence {occurrence} of {needle:?}"));
    let line = text[..byte_index]
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count();
    let line_start = text[..byte_index].rfind('\n').map_or(0, |index| index + 1);
    (
        line as u64,
        text[line_start..byte_index].chars().count() as u64,
    )
}

fn wait_for_workspace_symbols<F>(client: &mut LspProcess, query: &str, predicate: F) -> Value
where
    F: Fn(&Value) -> bool,
{
    let deadline = Instant::now() + POLL_TIMEOUT;
    let mut interval = POLL_INTERVAL;
    loop {
        match client.request_with_timeout(
            "workspace/symbol",
            json!({ "query": query }),
            POLL_REQUEST_TIMEOUT,
        ) {
            Ok(result) if predicate(&result) => return result,
            Ok(_) => {}
            Err(error) if error.starts_with("timed out") => {}
            Err(error) => panic!("workspace/symbol failed: {error}"),
        }
        assert!(
            Instant::now() < deadline,
            "workspace/symbol did not reach the expected state for {query:?}"
        );
        thread::sleep(interval);
        interval = interval.saturating_mul(2).min(Duration::from_secs(1));
    }
}

/// Poll `textDocument/documentSymbol` until `predicate` accepts the response.
/// Returns the raw response (which may be `null` when the server has no
/// servable snapshot for the document's root).
fn wait_for_document_symbols<F>(client: &mut LspProcess, uri: &str, predicate: F) -> Value
where
    F: Fn(&Value) -> bool,
{
    let deadline = Instant::now() + POLL_TIMEOUT;
    let mut interval = POLL_INTERVAL;
    loop {
        match client.request_with_timeout(
            "textDocument/documentSymbol",
            json!({ "textDocument": { "uri": uri } }),
            POLL_REQUEST_TIMEOUT,
        ) {
            Ok(result) if predicate(&result) => return result,
            Ok(_) => {}
            Err(error) if error.starts_with("timed out") => {}
            Err(error) => panic!("documentSymbol failed: {error}"),
        }
        assert!(
            Instant::now() < deadline,
            "documentSymbol did not reach the expected state for {uri}"
        );
        thread::sleep(interval);
        interval = interval.saturating_mul(2).min(Duration::from_secs(1));
    }
}

fn wait_for_diagnostics<F>(client: &mut LspProcess, uri: &str, predicate: F) -> Value
where
    F: Fn(&Value) -> bool,
{
    client
        .wait_for_notification_where("textDocument/publishDiagnostics", |params| {
            params.get("uri").and_then(Value::as_str) == Some(uri) && predicate(params)
        })
        .unwrap_or_else(|error| panic!("diagnostics notification failed for {uri}: {error}"))
        .get("params")
        .cloned()
        .unwrap_or(Value::Null)
}

fn has_no_severity_1(params: &Value) -> bool {
    params
        .get("diagnostics")
        .and_then(Value::as_array)
        .is_some_and(|diagnostics| {
            diagnostics
                .iter()
                .all(|diagnostic| diagnostic.get("severity").and_then(Value::as_u64) != Some(1))
        })
}

fn has_severity_1(params: &Value) -> bool {
    params
        .get("diagnostics")
        .and_then(Value::as_array)
        .is_some_and(|diagnostics| {
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.get("severity").and_then(Value::as_u64) == Some(1))
        })
}

/// At least one error (1) or warning (2) entry: a Problems-panel-worthy
/// finding on a URI.
fn has_error_or_warning(params: &Value) -> bool {
    params
        .get("diagnostics")
        .and_then(Value::as_array)
        .is_some_and(|diagnostics| {
            diagnostics.iter().any(|diagnostic| {
                matches!(
                    diagnostic.get("severity").and_then(Value::as_u64),
                    Some(1) | Some(2)
                )
            })
        })
}
