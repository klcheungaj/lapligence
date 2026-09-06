//! Process-level LSP stdio contract tests.
//!
//! These tests deliberately use only the wire protocol.  They are the
//! acceptance surface for `llg.toml`-driven workspace discovery, multi-root
//! indexing, dynamic file watching, config reload, include authorization and
//! the last-good navigation behavior used while a buffer has an error.  The
//! backend must add `serde_json` as a dev dependency for this integration
//! test.

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

// ── Contract tests ───────────────────────────────────────────────────────────

#[test]
fn lsp_stdio_discovers_multi_root_workspace_and_handles_watchers() {
    let fixture = FixtureTree::new();
    let root_a = fixture.root("root-a");
    let root_b = fixture.root("root-b");
    let root_c = fixture.root("root-c");
    let mut client = LspProcess::spawn(&fixture.root);

    // No didOpen is sent before this request: discovery must index `.v`/`.sv`
    // compilation units from both workspace folders on disk.  `.vh`/`.svh`
    // files are include-only and must NOT appear as standalone symbols.
    let initialize = client
        .initialize(
            &[("root-a", &root_a), ("root-b", &root_b)],
            default_init_options(),
        )
        .expect("initialize multi-root workspace");
    // Specific providers must be present, not just an arbitrary capabilities
    // object (mirrors the Node E2E capability contract).
    let capabilities = initialize
        .get("capabilities")
        .and_then(Value::as_object)
        .expect("initialize response must carry a capabilities object")
        .clone();
    for flag in [
        "hoverProvider",
        "definitionProvider",
        "referencesProvider",
        "documentSymbolProvider",
        "workspaceSymbolProvider",
        "completionProvider",
        "semanticTokensProvider",
    ] {
        assert!(
            capabilities.get(flag).is_some_and(|value| !value.is_null()),
            "capability {flag} must be advertised: {capabilities:?}"
        );
    }
    let trigger_characters = capabilities
        .get("completionProvider")
        .and_then(|provider| provider.get("triggerCharacters"))
        .and_then(Value::as_array)
        .expect("completionProvider.triggerCharacters")
        .iter()
        .filter_map(|value| value.as_str().map(str::to_owned))
        .collect::<Vec<_>>();
    for trigger in [".", ":", "`"] {
        assert!(
            trigger_characters
                .iter()
                .any(|candidate| candidate == trigger),
            "completion trigger character `{trigger}` must be advertised: {trigger_characters:?}"
        );
    }
    let legend_token_types = capabilities
        .get("semanticTokensProvider")
        .and_then(|provider| provider.get("legend"))
        .and_then(|legend| legend.get("tokenTypes"))
        .and_then(Value::as_array)
        .expect("semanticTokensProvider.legend.tokenTypes");
    assert!(
        !legend_token_types.is_empty(),
        "semantic token legend must list token types"
    );
    let full_sync = match capabilities.get("textDocumentSync") {
        Some(Value::Number(number)) => number.as_i64() == Some(1),
        Some(Value::Object(object)) => object.get("change").and_then(Value::as_i64) == Some(1),
        _ => false,
    };
    assert!(full_sync, "full text sync must be advertised");
    assert_eq!(
        capabilities
            .get("workspace")
            .and_then(|workspace| workspace.get("workspaceFolders"))
            .and_then(|folders| folders.get("supported"))
            .and_then(Value::as_bool),
        Some(true),
        "workspace folder support must be advertised"
    );

    let registration = client
        .wait_for_server_request("client/registerCapability")
        .expect("server must dynamically register watched-file support");
    assert_eq!(
        registration.get("method").and_then(Value::as_str),
        Some("client/registerCapability")
    );
    let registration_text = registration.to_string();
    assert!(
        registration_text.contains("workspace/didChangeWatchedFiles"),
        "watched-file registration missing: {registration}"
    );
    // Watchers cover the effective config and `.v`/`.sv` units; headers and
    // arbitrary-extension includes are covered only once resolved as deps.
    assert!(
        registration_text.contains("llg.toml"),
        "watch registration does not cover the config file: {registration}"
    );
    assert!(
        registration_text.contains("**/*.v") && registration_text.contains("**/*.sv"),
        "watch registration does not cover .v/.sv units: {registration}"
    );

    let discovery = wait_for_workspace_symbols(&mut client, "Discovery", |result| {
        let names = names(result);
        ["DiscoveryV", "DiscoverySv"]
            .iter()
            .all(|expected| names.iter().any(|name| name == expected))
            && names.iter().all(|name| {
                name != "ExcludedShouldNotAppear" && name != "DiscoveryVh" && name != "DiscoverySvh"
            })
    });
    assert_no_shadow_uris(&discovery);

    let merged = wait_for_workspace_symbols(&mut client, "RootBDiscovery", |result| {
        names(result).iter().any(|name| name == "RootBDiscovery")
    });
    assert_no_shadow_uris(&merged);

    let top_path = root_b.join("ports").join("top.sv");
    let top_text = fs::read_to_string(&top_path).expect("read multiline-port fixture");
    let document_symbols = client
        .request(
            "textDocument/documentSymbol",
            json!({ "textDocument": { "uri": file_uri(&top_path) } }),
        )
        .expect("document symbols request");
    assert_has_name(&document_symbols, "port_top");
    assert_no_shadow_uris(&document_symbols);

    let semantic_tokens = client
        .request(
            "textDocument/semanticTokens/full",
            json!({ "textDocument": { "uri": file_uri(&top_path) } }),
        )
        .expect("semantic tokens request");
    let token_data = semantic_tokens
        .get("data")
        .and_then(Value::as_array)
        .expect("semantic token result data");
    assert!(!token_data.is_empty(), "semantic token result is empty");
    assert_eq!(
        token_data.len() % 5,
        0,
        "semantic token data is not 5-tuples"
    );

    // The `.data` label is deliberately separated from its actual connection
    // expression.  The result must point to the child port in the real source.
    let definition = client
        .request(
            "textDocument/definition",
            json!({
                "textDocument": { "uri": file_uri(&top_path) },
                "position": position_at(&top_text, ".data", 1)
            }),
        )
        .expect("multiline named-port definition request");
    assert_no_shadow_uris(&definition);
    let definition_uri = definition
        .get("uri")
        .and_then(Value::as_str)
        .expect("definition location URI");
    assert_eq!(
        definition_uri,
        file_uri(&root_b.join("ports").join("child.sv")),
        "named port definition must resolve to the child declaration"
    );

    // Add and remove an independent root through the standard workspace
    // notification.  The existing roots must remain indexed after removal.
    client
        .send_workspace_folder_change(&[("root-c", &root_c)], &[])
        .expect("add workspace folder");
    let added = wait_for_workspace_symbols(&mut client, "FolderAdded", |result| {
        names(result).iter().any(|name| name == "FolderAdded")
    });
    assert_no_shadow_uris(&added);
    client
        .send_workspace_folder_change(&[], &[("root-c", &root_c)])
        .expect("remove workspace folder");
    wait_for_workspace_symbols(&mut client, "FolderAdded", |result| {
        names(result).iter().all(|name| name != "FolderAdded")
    });
    let roots_after_remove = wait_for_workspace_symbols(&mut client, "RootBDiscovery", |result| {
        names(result).iter().any(|name| name == "RootBDiscovery")
    });
    assert_no_shadow_uris(&roots_after_remove);

    // Exercise create/change/delete events without opening the watched file.
    let watched_dir = root_a.join("watched");
    fs::create_dir_all(&watched_dir).expect("create watched fixture directory");
    let watched_path = watched_dir.join("dynamic.sv");
    let created_text =
        format!("{SOURCE_HEADER} root-a/watched/dynamic.sv\nmodule WatchedCreated; endmodule\n");
    fs::write(&watched_path, &created_text).expect("create watched source");
    client
        .send_watch_event(&watched_path, 1)
        .expect("send watched create event");
    wait_for_workspace_symbols(&mut client, "WatchedCreated", |result| {
        names(result).iter().any(|name| name == "WatchedCreated")
    });

    let changed_text =
        format!("{SOURCE_HEADER} root-a/watched/dynamic.sv\nmodule WatchedChanged; endmodule\n");
    fs::write(&watched_path, &changed_text).expect("change watched source");
    client
        .send_watch_event(&watched_path, 2)
        .expect("send watched change event");
    wait_for_workspace_symbols(&mut client, "WatchedChanged", |result| {
        let symbol_names = names(result);
        symbol_names.iter().any(|name| name == "WatchedChanged")
            && symbol_names.iter().all(|name| name != "WatchedCreated")
    });

    fs::remove_file(&watched_path).expect("delete watched source");
    client
        .send_watch_event(&watched_path, 3)
        .expect("send watched delete event");
    wait_for_workspace_symbols(&mut client, "WatchedChanged", |result| {
        names(result).iter().all(|name| name != "WatchedChanged")
    });
    client.shutdown();
}

#[test]
fn lsp_stdio_keeps_navigation_snapshot_during_syntax_error_and_recovers() {
    let fixture = FixtureTree::new();
    let root_a = fixture.root("root-a");
    let path = root_a.join("navigation").join("snapshot.sv");
    let valid = fs::read_to_string(&path).expect("read snapshot fixture");
    let invalid = valid.replace(
        "assign snapshot_signal = 1'b0;",
        "assign snapshot_signal = ;",
    );
    let uri = file_uri(&path);
    let mut client = LspProcess::spawn(&fixture.root);
    client
        .initialize(&[("root-a", &root_a)], default_init_options())
        .expect("initialize snapshot workspace");

    client.open(&path, &valid).expect("open snapshot source");
    let valid_diagnostics = wait_for_diagnostics(&mut client, &uri, has_no_severity_1);
    assert!(!has_lint_rule(&valid_diagnostics, "unused-signal"));

    let symbol_request = json!({ "textDocument": { "uri": uri } });
    let before_symbols = client
        .request("textDocument/documentSymbol", symbol_request.clone())
        .expect("document symbols before syntax error");
    assert_has_name(&before_symbols, "SnapshotTop");
    assert_no_shadow_uris(&before_symbols);

    let before_definition = client
        .request(
            "textDocument/definition",
            json!({
                "textDocument": { "uri": uri },
                "position": position_at(&valid, "assign snapshot_signal", "assign ".len())
            }),
        )
        .expect("definition before syntax error");
    assert_no_shadow_uris(&before_definition);
    assert_eq!(
        before_definition.get("uri").and_then(Value::as_str),
        Some(uri.as_str()),
        "baseline navigation must use the real URI"
    );

    let declaration_position = position_at(&valid, "snapshot_signal", 0);
    let references_with_declaration = client
        .request(
            "textDocument/references",
            json!({
                "textDocument": { "uri": uri },
                "position": declaration_position,
                "context": { "includeDeclaration": true }
            }),
        )
        .expect("references including declaration");
    let references_without_declaration = client
        .request(
            "textDocument/references",
            json!({
                "textDocument": { "uri": uri },
                "position": declaration_position,
                "context": { "includeDeclaration": false }
            }),
        )
        .expect("references excluding declaration");
    assert!(
        has_location_start(&references_with_declaration, &declaration_position),
        "declaration missing when includeDeclaration=true: {references_with_declaration}"
    );
    assert!(
        !has_location_start(&references_without_declaration, &declaration_position),
        "declaration returned when includeDeclaration=false: {references_without_declaration}"
    );
    assert_eq!(
        references_with_declaration.as_array().map(Vec::len),
        references_without_declaration
            .as_array()
            .map(|locations| locations.len() + 1),
        "includeDeclaration should only add the declaration"
    );

    client
        .change(&path, 2, &invalid)
        .expect("introduce syntax error");
    let syntax_diagnostics = wait_for_diagnostics(&mut client, &uri, has_severity_1);
    assert!(
        syntax_diagnostics
            .get("diagnostics")
            .and_then(Value::as_array)
            .is_some_and(|diagnostics| diagnostics
                .iter()
                .any(|diagnostic| diagnostic.get("severity").and_then(Value::as_u64) == Some(1))),
        "syntax error did not publish diagnostics"
    );

    // A failed recompile must not destroy the last good feature snapshot.
    let stale_symbols = client
        .request("textDocument/documentSymbol", symbol_request)
        .expect("document symbols during syntax error");
    assert_has_name(&stale_symbols, "SnapshotTop");
    assert_no_shadow_uris(&stale_symbols);
    let stale_definition = client
        .request(
            "textDocument/definition",
            json!({
                "textDocument": { "uri": uri },
                "position": position_at(&valid, "assign snapshot_signal", "assign ".len())
            }),
        )
        .expect("definition during syntax error");
    assert_no_shadow_uris(&stale_definition);
    assert_eq!(
        stale_definition.get("uri").and_then(Value::as_str),
        Some(uri.as_str()),
        "stale navigation must remain mapped to the real URI"
    );

    client
        .change(&path, 3, &valid)
        .expect("recover snapshot source");
    let recovered_diagnostics = wait_for_diagnostics(&mut client, &uri, has_no_severity_1);
    assert!(
        recovered_diagnostics
            .get("diagnostics")
            .and_then(Value::as_array)
            .is_some_and(|diagnostics| diagnostics
                .iter()
                .all(|diagnostic| diagnostic.get("severity").and_then(Value::as_u64) != Some(1))),
        "diagnostics did not clear after recovery"
    );
    client.shutdown();
}

#[test]
fn lsp_stdio_rejects_include_escape_and_keeps_last_good_navigation() {
    let fixture = FixtureTree::new();
    let root_a = fixture.root("root-a");
    let path = root_a.join("navigation").join("snapshot.sv");
    let valid = fs::read_to_string(&path).expect("read snapshot fixture");
    let uri = file_uri(&path);
    let mut client = LspProcess::spawn(&fixture.root);
    client
        .initialize(&[("root-a", &root_a)], default_init_options())
        .expect("initialize include workspace");
    client.open(&path, &valid).expect("open include source");
    wait_for_diagnostics(&mut client, &uri, has_no_severity_1);

    let relative_escape = valid.replace(
        "module SnapshotTop;",
        "`include \"../../outside.svh\"\nmodule SnapshotTop;",
    );
    client
        .change(&path, 2, &relative_escape)
        .expect("introduce relative include escape");
    let relative_diagnostics = wait_for_diagnostics(&mut client, &uri, |params| {
        params
            .get("diagnostics")
            .and_then(Value::as_array)
            .is_some_and(|diagnostics| {
                diagnostics.iter().any(|diagnostic| {
                    diagnostic.get("severity").and_then(Value::as_u64) == Some(1)
                        && diagnostic
                            .get("message")
                            .and_then(Value::as_str)
                            .is_some_and(|message| {
                                message.contains("escapes configured source/include directories")
                            })
                })
            })
    });
    assert!(
        relative_diagnostics
            .get("diagnostics")
            .and_then(Value::as_array)
            .is_some_and(|diagnostics| diagnostics.iter().any(|diagnostic| {
                diagnostic
                    .get("message")
                    .and_then(Value::as_str)
                    .is_some_and(|message| message.contains("../../outside.svh"))
            })),
        "relative include failure was not published: {relative_diagnostics}"
    );
    let stale_symbols = client
        .request(
            "textDocument/documentSymbol",
            json!({ "textDocument": { "uri": uri } }),
        )
        .expect("navigation during include failure");
    assert_has_name(&stale_symbols, "SnapshotTop");

    client
        .change(&path, 3, &valid)
        .expect("recover after relative include failure");
    wait_for_diagnostics(&mut client, &uri, has_no_severity_1);

    let absolute_escape = valid.replace(
        "module SnapshotTop;",
        &format!(
            "`include \"{}\"\nmodule SnapshotTop;",
            fixture.root("outside.svh").display()
        ),
    );
    client
        .change(&path, 4, &absolute_escape)
        .expect("introduce absolute include escape");
    let absolute_diagnostics = wait_for_diagnostics(&mut client, &uri, |params| {
        params
            .get("diagnostics")
            .and_then(Value::as_array)
            .is_some_and(|diagnostics| {
                diagnostics.iter().any(|diagnostic| {
                    diagnostic.get("severity").and_then(Value::as_u64) == Some(1)
                        && diagnostic
                            .get("message")
                            .and_then(Value::as_str)
                            .is_some_and(|message| {
                                message.contains("escapes configured source/include directories")
                            })
                })
            })
    });
    assert!(
        absolute_diagnostics
            .get("diagnostics")
            .and_then(Value::as_array)
            .is_some_and(|diagnostics| diagnostics.iter().any(|diagnostic| {
                diagnostic
                    .get("message")
                    .and_then(Value::as_str)
                    .is_some_and(|message| message.contains("outside.svh"))
            })),
        "absolute include failure was not published: {absolute_diagnostics}"
    );
    client.shutdown();
}

#[test]
fn lsp_stdio_revalidates_nested_root_candidates_after_ownership() {
    let fixture = FixtureTree::new();
    let root_a = fixture.root("root-a");
    let nested = root_a.join("nested");
    fs::create_dir_all(&nested).expect("create nested workspace root");
    let nested_config = nested.join(CONFIG_FILE);
    fs::write(
        &nested_config,
        "schema_version = 1\n\
         [sources]\n\
         directories = [\".\"]\n\
         include = [\"**/*.sv\", \"**/*.v\"]\n\
         exclude = [\"**/owned.sv\"]\n",
    )
    .expect("write nested workspace config");
    let nested_source = nested.join("owned.sv");
    fs::write(
        &nested_source,
        "// llg-lsp-fixture: root-a/nested/owned.sv\nmodule NestedOwnership; endmodule\n",
    )
    .expect("write nested workspace source");

    let mut client = LspProcess::spawn(&fixture.root);
    client
        .initialize(
            &[("root-a", &root_a), ("nested", &nested)],
            default_init_options(),
        )
        .expect("initialize nested workspace");

    // The nested root structurally owns `owned.sv` (longest prefix) but its
    // discovery excludes it, so it must not be indexed by either root yet.
    wait_for_workspace_symbols(&mut client, "NestedOwnership", |result| {
        names(result).iter().all(|name| name != "NestedOwnership")
    });

    client
        .send_workspace_folder_change(&[], &[("nested", &nested)])
        .expect("remove nested workspace root");
    let parent_owned = wait_for_workspace_symbols(&mut client, "NestedOwnership", |result| {
        names(result).iter().any(|name| name == "NestedOwnership")
    });
    assert_no_shadow_uris(&parent_owned);
    client.shutdown();
}

#[test]
fn lsp_stdio_cross_root_include_under_configured_dir_is_allowed() {
    // The "cannot cross workspace root" blanket rule is removed: an include
    // that stays under a configured source directory is allowed even when it
    // points into another workspace root's tree.
    let fixture = FixtureTree::new();
    let root_a = fixture.root("root-a");
    let nested = root_a.join("nested");
    let nested_include_dir = nested.join("include");
    fs::create_dir_all(&nested_include_dir).expect("create nested workspace root");
    let nested_config = nested.join(CONFIG_FILE);
    fs::write(
        &nested_config,
        "schema_version = 1\n\
         [sources]\n\
         directories = [\".\"]\n\
         include = [\"**/*.sv\", \"**/*.v\", \"**/*.svh\"]\n",
    )
    .expect("write nested workspace config");
    let nested_include = nested_include_dir.join("shared_defs.svh");
    fs::write(
        &nested_include,
        "// llg-lsp-fixture: root-a/nested/include/shared_defs.svh\n`define NESTED_SHARED\n",
    )
    .expect("write nested include");
    let outer_dir = root_a.join("outer");
    fs::create_dir_all(&outer_dir).expect("create outer source directory");
    let outer = outer_dir.join("nested_include.sv");
    let outer_text = "// llg-lsp-fixture: root-a/outer/nested_include.sv\n`include \"../nested/include/shared_defs.svh\"\nmodule OuterInclude;\n  logic [NESTED_SHARED-1:0] v;\nendmodule\n";
    fs::write(&outer, outer_text).expect("write outer source");

    let mut client = LspProcess::spawn(&fixture.root);
    client
        .initialize(
            &[("root-a", &root_a), ("nested", &nested)],
            default_init_options(),
        )
        .expect("initialize nested include workspace");
    let outer_uri = file_uri(&outer);
    client.open(&outer, outer_text).expect("open outer source");
    let diagnostics = wait_for_diagnostics(&mut client, &outer_uri, |params| {
        params
            .get("diagnostics")
            .and_then(Value::as_array)
            .is_some_and(|diagnostics| {
                !diagnostics.iter().any(|diagnostic| {
                    diagnostic
                        .get("message")
                        .and_then(Value::as_str)
                        .is_some_and(|message| message.contains("escapes configured"))
                })
            })
    });
    assert!(
        diagnostics
            .get("diagnostics")
            .and_then(Value::as_array)
            .is_some_and(|diagnostics| !diagnostics.iter().any(|d| d
                .get("message")
                .and_then(Value::as_str)
                .is_some_and(|m| m.contains("escapes configured")))),
        "configured-dir include was rejected: {diagnostics}"
    );
    client.shutdown();
}

#[test]
fn lsp_stdio_unsaved_top_resolves_unopened_disk_include() {
    let fixture = FixtureTree::new();
    let root_a = fixture.root("root-a");
    let include_dir = root_a.join("unsaved");
    fs::create_dir_all(&include_dir).expect("create unsaved include directory");
    let top = include_dir.join("top.sv");
    let include = include_dir.join("defs.inc");
    let disk_top = "// llg-lsp-fixture: root-a/unsaved/top.sv\nmodule DiskTop; endmodule\n";
    let open_top = "// llg-lsp-fixture: root-a/unsaved/top.sv\n`include \"defs.inc\"\nmodule UnsavedIncludeTop;\n  logic [HEADER_WIDTH-1:0] value;\nendmodule\n";
    fs::write(&top, disk_top).expect("write on-disk top");
    fs::write(
        &include,
        "// llg-lsp-fixture: root-a/unsaved/defs.inc\n`define HEADER_WIDTH 8\n",
    )
    .expect("write unopened include");

    let mut client = LspProcess::spawn(&fixture.root);
    client
        .initialize(&[("root-a", &root_a)], default_init_options())
        .expect("initialize unsaved include workspace");
    let top_uri = file_uri(&top);
    client.open(&top, open_top).expect("open unsaved top");
    let diagnostics = wait_for_diagnostics(&mut client, &top_uri, has_no_severity_1);
    assert!(
        diagnostics
            .get("diagnostics")
            .and_then(Value::as_array)
            .is_some_and(|diagnostics| diagnostics
                .iter()
                .all(|diagnostic| diagnostic.get("severity").and_then(Value::as_u64) != Some(1))),
        "unopened include caused a compile error: {diagnostics}"
    );
    let symbols = wait_for_workspace_symbols(&mut client, "UnsavedIncludeTop", |result| {
        names(result).iter().any(|name| name == "UnsavedIncludeTop")
            && names(result).iter().all(|name| name != "DiskTop")
    });
    assert_no_shadow_uris(&symbols);
    client.shutdown();
}

#[test]
fn lsp_stdio_open_unsaved_inc_overrides_disk_include_for_open_top() {
    let fixture = FixtureTree::new();
    let root_a = fixture.root("root-a");
    let include_dir = root_a.join("unsaved_inc");
    fs::create_dir_all(&include_dir).expect("create unsaved include directory");
    let top = include_dir.join("top.sv");
    let include = include_dir.join("defs.inc");
    let disk_top = format!(
        "{SOURCE_HEADER} root-a/unsaved_inc/top.sv\n`include \"defs.inc\"\n`ifdef UNSAVED_INCLUDE\nmodule UnsavedIncTop;\n`else\nmodule DiskIncTop;\n`endif\nendmodule\n"
    );
    let open_top = disk_top.clone();
    let disk_include =
        format!("{SOURCE_HEADER} root-a/unsaved_inc/defs.inc\n`define DISK_INCLUDE\n");
    let open_include =
        format!("{SOURCE_HEADER} root-a/unsaved_inc/defs.inc\n`define UNSAVED_INCLUDE\n");
    fs::write(&top, &disk_top).expect("write on-disk include top");
    fs::write(&include, &disk_include).expect("write on-disk include");

    let mut client = LspProcess::spawn(&fixture.root);
    client
        .initialize(&[("root-a", &root_a)], default_init_options())
        .expect("initialize unsaved include workspace");
    client.open(&top, &open_top).expect("open include top");
    client
        .open(&include, &open_include)
        .expect("open unsaved include buffer");

    let symbols = wait_for_workspace_symbols(&mut client, "IncTop", |result| {
        let symbol_names = names(result);
        symbol_names.iter().any(|name| name == "UnsavedIncTop")
            && symbol_names.iter().all(|name| name != "DiskIncTop")
    });
    assert_no_shadow_uris(&symbols);
    client.shutdown();
}

#[test]
fn lsp_stdio_open_unsaved_excluded_svh_overrides_disk_include_for_open_top() {
    let fixture = FixtureTree::new();
    let root_a = fixture.root("root-a");
    let top_dir = root_a.join("unsaved_excluded");
    let include_dir = root_a.join("excluded");
    fs::create_dir_all(&top_dir).expect("create excluded include top directory");
    fs::create_dir_all(&include_dir).expect("create excluded include directory");
    let top = top_dir.join("top.sv");
    let include = include_dir.join("open_defs.svh");
    let disk_top = format!(
        "{SOURCE_HEADER} root-a/unsaved_excluded/top.sv\n`include \"../excluded/open_defs.svh\"\n`ifdef UNSAVED_EXCLUDED_INCLUDE\nmodule UnsavedExcludedSvhTop;\n`else\nmodule DiskExcludedSvhTop;\n`endif\nendmodule\n"
    );
    let open_top = disk_top.clone();
    let disk_include =
        format!("{SOURCE_HEADER} root-a/excluded/open_defs.svh\n`define DISK_EXCLUDED_INCLUDE\n");
    let open_include = format!(
        "{SOURCE_HEADER} root-a/excluded/open_defs.svh\n`define UNSAVED_EXCLUDED_INCLUDE\n"
    );
    fs::write(&top, &disk_top).expect("write excluded include top");
    fs::write(&include, &disk_include).expect("write excluded include");

    let mut client = LspProcess::spawn(&fixture.root);
    client
        .initialize(&[("root-a", &root_a)], default_init_options())
        .expect("initialize excluded include workspace");
    client
        .open(&top, &open_top)
        .expect("open excluded include top");
    client
        .open(&include, &open_include)
        .expect("open unsaved excluded include buffer");

    let symbols = wait_for_workspace_symbols(&mut client, "ExcludedSvhTop", |result| {
        let symbol_names = names(result);
        symbol_names
            .iter()
            .any(|name| name == "UnsavedExcludedSvhTop")
            && symbol_names.iter().all(|name| name != "DiskExcludedSvhTop")
    });
    assert_no_shadow_uris(&symbols);
    client.shutdown();
}

#[test]
fn lsp_stdio_loads_per_root_lint_configuration() {
    let fixture = FixtureTree::new();
    let root_a = fixture.root("root-a");
    let root_b = fixture.root("root-b");
    let path_a = root_a.join("lint").join("per_root.sv");
    let path_b = root_b.join("lint").join("per_root.sv");
    let uri_a = file_uri(&path_a);
    let uri_b = file_uri(&path_b);
    let mut client = LspProcess::spawn(&fixture.root);
    client
        .initialize(
            &[("root-a", &root_a), ("root-b", &root_b)],
            default_init_options(),
        )
        .expect("initialize lint workspace");

    client
        .open(
            &path_a,
            &fs::read_to_string(&path_a).expect("read root A lint source"),
        )
        .expect("open root A lint source");
    client
        .open(
            &path_b,
            &fs::read_to_string(&path_b).expect("read root B lint source"),
        )
        .expect("open root B lint source");

    // root-a/llg.toml disables unused-signal, while root-b's promotes it to
    // an error.  The two open documents observe their own root configuration.
    let root_a_diagnostics = wait_for_diagnostics(&mut client, &uri_a, |params| {
        !has_lint_rule(params, "unused-signal")
    });
    assert!(!has_lint_rule(&root_a_diagnostics, "unused-signal"));
    let root_b_diagnostics = wait_for_diagnostics(&mut client, &uri_b, |params| {
        lint_severity(params, "unused-signal") == Some(1)
    });
    assert_eq!(lint_severity(&root_b_diagnostics, "unused-signal"), Some(1));
    assert_no_shadow_uris(&root_b_diagnostics);
    client.shutdown();
}

#[test]
fn lsp_stdio_lints_standalone_files_without_a_workspace_folder() {
    let cwd_fixture = FixtureTree::new();
    let source_fixture = FixtureTree::new();
    let directory_a = source_fixture.base().join("standalone-a");
    let directory_b = source_fixture.base().join("standalone-b");
    fs::create_dir_all(&directory_a).expect("create first standalone directory");
    fs::create_dir_all(&directory_b).expect("create second standalone directory");
    let path_a = directory_a.join("bad.sv");
    let path_b = directory_b.join("lint.sv");
    let uri_a = file_uri(&path_a);
    let uri_b = file_uri(&path_b);
    let broken = "// llg-lsp-fixture: standalone-a/bad.sv\nmodule StandaloneBroken;\n";
    let lint_a = "// llg-lsp-fixture: standalone-a/bad.sv\nmodule StandaloneLintA;\n  logic unused_a;\nendmodule\n";
    let lint_b = "// llg-lsp-fixture: standalone-b/lint.sv\nmodule StandaloneLintB;\n  logic unused_b;\nendmodule\n";
    let recovered =
        "// llg-lsp-fixture: standalone-a/bad.sv\nmodule StandaloneRecovered; endmodule\n";

    let mut client = LspProcess::spawn(cwd_fixture.base());
    client
        .initialize(&[], default_init_options())
        .expect("initialize without workspace folders");

    client
        .open(&path_a, broken)
        .expect("open standalone syntax-error source");
    let syntax = wait_for_diagnostics(&mut client, &uri_a, has_severity_1);
    assert!(
        has_severity_1(&syntax),
        "standalone syntax error was not published"
    );

    client
        .change(&path_a, 2, lint_a)
        .expect("repair standalone syntax and introduce lint finding");
    let linted_a = wait_for_diagnostics(&mut client, &uri_a, |params| {
        !has_severity_1(params) && has_lint_rule(params, "unused-signal")
    });
    assert!(has_lint_rule(&linted_a, "unused-signal"));

    client
        .open(&path_b, lint_b)
        .expect("open source in second standalone directory");
    let linted_b = wait_for_diagnostics(&mut client, &uri_b, |params| {
        has_lint_rule(params, "unused-signal")
    });
    assert!(has_lint_rule(&linted_b, "unused-signal"));

    client
        .change(&path_a, 3, recovered)
        .expect("remove standalone lint finding");
    let recovered_diagnostics = wait_for_diagnostics(&mut client, &uri_a, |params| {
        !has_severity_1(params) && !has_lint_rule(params, "unused-signal")
    });
    assert!(!has_severity_1(&recovered_diagnostics));
    assert!(!has_lint_rule(&recovered_diagnostics, "unused-signal"));
    client.shutdown();
}

#[test]
fn lsp_stdio_root_config_reload_keeps_discovery_filters() {
    // Editing the root `llg.toml` (lint-only change) must not reset source
    // discovery filters.
    let fixture = FixtureTree::new();
    let root_a = fixture.root("root-a");
    let path = root_a.join("discovery").join("lint_settings.sv");
    let text = format!(
        "{SOURCE_HEADER} root-a/discovery/lint_settings.sv\nmodule LintSettingsAllowed;\n  logic unused_lint_settings;\nendmodule\n"
    );
    fs::write(&path, &text).expect("write lint settings source");
    let uri = file_uri(&path);
    let mut client = LspProcess::spawn(&fixture.root);
    client
        .initialize(&[("root-a", &root_a)], default_init_options())
        .expect("initialize restrictive workspace");
    client
        .open(&path, &text)
        .expect("open lint settings source");
    let initial = wait_for_diagnostics(&mut client, &uri, |params| {
        !has_lint_rule(params, "unused-signal")
    });
    assert!(!has_lint_rule(&initial, "unused-signal"));

    // Flip unused-signal on (as a warning) in the config; discovery filters
    // (which exclude `**/excluded/**`) must be retained.
    let config_path = root_a.join(CONFIG_FILE);
    fs::write(
        &config_path,
        "schema_version = 1\n\
         [sources]\n\
         directories = [\".\"]\n\
         include = [\"**/*.v\", \"**/*.sv\"]\n\
         exclude = [\"**/excluded/**\"]\n\
         [lint]\n\
         enabled = true\n\
         [lint.rules.unused-signal]\n\
         enabled = true\n\
         severity = \"warning\"\n",
    )
    .expect("update root config");
    client
        .send_watch_event(&config_path, 2)
        .expect("send config watch event");
    let updated = wait_for_diagnostics(&mut client, &uri, |params| {
        lint_severity(params, "unused-signal") == Some(2)
    });
    assert_eq!(lint_severity(&updated, "unused-signal"), Some(2));

    let excluded = client
        .request(
            "workspace/symbol",
            json!({ "query": "ExcludedShouldNotAppear" }),
        )
        .expect("query excluded symbol");
    assert!(
        names(&excluded)
            .iter()
            .all(|name| name != "ExcludedShouldNotAppear"),
        "config reload reset discovery filters: {excluded}"
    );
    client.shutdown();
}

#[test]
fn lsp_stdio_root_config_watch_ignores_restrictive_source_include() {
    // The effective `llg.toml` is watched even when the root's discovery
    // filters are restrictive (the config controls discovery and must remain
    // observable).
    let fixture = FixtureTree::new();
    let root_a = fixture.root("root-a");
    let source_dir = root_a.join("src");
    fs::create_dir_all(&source_dir).expect("create restrictive include directory");
    let source = source_dir.join("lint_watch.sv");
    let source_text = format!(
        "{SOURCE_HEADER} root-a/src/lint_watch.sv\nmodule RootLintWatch;\n  logic unused_root_lint_watch;\nendmodule\n"
    );
    fs::write(&source, &source_text).expect("write root lint watch source");
    let uri = file_uri(&source);
    // Restrictive config: only `src/**` is discoverable, `unused-signal` off.
    let config_path = root_a.join(CONFIG_FILE);
    fs::write(
        &config_path,
        "schema_version = 1\n\
         [sources]\n\
         directories = [\".\"]\n\
         include = [\"src/**\"]\n\
         exclude = [\"**/excluded/**\"]\n\
         [lint]\n\
         enabled = true\n\
         [lint.rules.unused-signal]\n\
         enabled = false\n",
    )
    .expect("write restrictive root config");
    let mut client = LspProcess::spawn(&fixture.root);
    client
        .initialize(&[("root-a", &root_a)], default_init_options())
        .expect("initialize restrictive lint workspace");
    client
        .open(&source, &source_text)
        .expect("open root lint watch source");
    let initial = wait_for_diagnostics(&mut client, &uri, |params| {
        !has_lint_rule(params, "unused-signal")
    });
    assert!(!has_lint_rule(&initial, "unused-signal"));

    fs::write(
        &config_path,
        "schema_version = 1\n\
         [sources]\n\
         directories = [\".\"]\n\
         include = [\"src/**\"]\n\
         exclude = [\"**/excluded/**\"]\n\
         [lint]\n\
         enabled = true\n\
         [lint.rules.unused-signal]\n\
         enabled = true\n\
         severity = \"warning\"\n",
    )
    .expect("change root lint config");
    client
        .send_watch_event(&config_path, 2)
        .expect("send root config watch event");
    let updated = wait_for_diagnostics(&mut client, &uri, |params| {
        lint_severity(params, "unused-signal") == Some(2)
    });
    assert_eq!(lint_severity(&updated, "unused-signal"), Some(2));
    client.shutdown();
}

#[test]
fn lsp_stdio_init_with_default_and_overridden_config_files() {
    let fixture = FixtureTree::new();
    let root_a = fixture.root("root-a");
    let root_b = fixture.root("root-b");

    // Default init (no overrides): both roots load `<root>/llg.toml`.
    let mut client = LspProcess::spawn(&fixture.root);
    client
        .initialize(
            &[("root-a", &root_a), ("root-b", &root_b)],
            default_init_options(),
        )
        .expect("initialize with default config files");
    // root-a disables unused-signal; root-b promotes it to an error.
    let path_a = root_a.join("lint").join("per_root.sv");
    let path_b = root_b.join("lint").join("per_root.sv");
    let uri_a = file_uri(&path_a);
    let uri_b = file_uri(&path_b);
    client
        .open(&path_a, &fs::read_to_string(&path_a).expect("read a"))
        .expect("open a");
    client
        .open(&path_b, &fs::read_to_string(&path_b).expect("read b"))
        .expect("open b");
    let diag_a = wait_for_diagnostics(&mut client, &uri_a, |p| !has_lint_rule(p, "unused-signal"));
    assert!(!has_lint_rule(&diag_a, "unused-signal"));
    let diag_b = wait_for_diagnostics(&mut client, &uri_b, |p| {
        lint_severity(p, "unused-signal") == Some(1)
    });
    assert_eq!(lint_severity(&diag_b, "unused-signal"), Some(1));
    client.shutdown();

    // Overridden init: point root-a at a custom config (in root-a) that
    // enables unused-signal as an error.
    let custom_config = root_a.join("custom.toml");
    fs::write(
        &custom_config,
        "schema_version = 1\n\
         [sources]\n\
         directories = [\".\"]\n\
         include = [\"**/*.v\", \"**/*.sv\"]\n\
         exclude = [\"**/excluded/**\"]\n\
         [lint]\n\
         enabled = true\n\
         [lint.rules.unused-signal]\n\
         enabled = true\n\
         severity = \"error\"\n",
    )
    .expect("write custom config");
    let mut client = LspProcess::spawn(&fixture.root);
    let options = init_options_with_config_files(&[(
        file_uri(&root_a).as_str(),
        custom_config.to_str().expect("path"),
    )]);
    client
        .initialize(&[("root-a", &root_a)], options)
        .expect("initialize with overridden config file");
    client
        .open(&path_a, &fs::read_to_string(&path_a).expect("read a"))
        .expect("open a");
    let overridden = wait_for_diagnostics(&mut client, &uri_a, |p| {
        lint_severity(p, "unused-signal") == Some(1)
    });
    assert_eq!(
        lint_severity(&overridden, "unused-signal"),
        Some(1),
        "config override must load the custom lint policy for root-a"
    );
    client.shutdown();
}

#[test]
fn lsp_stdio_config_reload_without_restart() {
    let fixture = FixtureTree::new();
    let root_a = fixture.root("root-a");
    let path = root_a.join("lint").join("per_root.sv");
    let uri = file_uri(&path);
    let mut client = LspProcess::spawn(&fixture.root);
    client
        .initialize(&[("root-a", &root_a)], default_init_options())
        .expect("initialize config reload workspace");
    client
        .open(&path, &fs::read_to_string(&path).expect("read lint source"))
        .expect("open lint source");
    let initial = wait_for_diagnostics(&mut client, &uri, |p| !has_lint_rule(p, "unused-signal"));
    assert!(!has_lint_rule(&initial, "unused-signal"));

    // Reload with unused-signal as an error: no restart required.
    let config_path = root_a.join(CONFIG_FILE);
    fs::write(
        &config_path,
        "schema_version = 1\n\
         [sources]\n\
         directories = [\".\"]\n\
         include = [\"**/*.v\", \"**/*.sv\"]\n\
         exclude = [\"**/excluded/**\"]\n\
         [lint]\n\
         enabled = true\n\
         [lint.rules.unused-signal]\n\
         enabled = true\n\
         severity = \"error\"\n",
    )
    .expect("write reloaded config");
    client
        .send_watch_event(&config_path, 2)
        .expect("send config reload event");
    let reloaded = wait_for_diagnostics(&mut client, &uri, |p| {
        lint_severity(p, "unused-signal") == Some(1)
    });
    assert_eq!(lint_severity(&reloaded, "unused-signal"), Some(1));

    // A malformed reload must retain the last-valid config and publish a
    // diagnostic against the TOML URI.
    fs::write(&config_path, "schema_version = 1\n[sources\n").expect("write malformed config");
    client
        .send_watch_event(&config_path, 2)
        .expect("send malformed config event");
    let config_uri = file_uri(&config_path);
    wait_for_diagnostics(&mut client, &config_uri, |p| {
        p.get("diagnostics")
            .and_then(Value::as_array)
            .is_some_and(|d| {
                d.iter().any(|diag| {
                    diag.get("source").and_then(Value::as_str) == Some("llg")
                        && diag.get("severity").and_then(Value::as_u64) == Some(1)
                })
            })
    });
    // The open file still gets its retained policy (unused-signal as error).
    // Identical payloads are not re-sent after every commit, so nudge the
    // buffer with an extra finding to force a fresh publication that proves
    // the malformed reload retained the last-valid lint policy.
    let retained_text = format!(
        "{}\nmodule RetainedProbe; endmodule\n",
        fs::read_to_string(&path).expect("read lint source")
    );
    client
        .change(&path, 2, &retained_text)
        .expect("nudge open buffer");
    let retained = wait_for_diagnostics(&mut client, &uri, |p| {
        lint_severity(p, "unused-signal") == Some(1)
    });
    assert_eq!(
        lint_severity(&retained, "unused-signal"),
        Some(1),
        "malformed reload must retain the last-valid lint policy"
    );
    client.shutdown();
}

#[test]
fn lsp_stdio_arbitrary_extension_include_dep_changes() {
    // A resolved include dependency of arbitrary extension is watched: editing
    // it re-analyzes and the change reaches the published output (the `.mem`
    // file defines a macro consumed by `top.sv`, flipping its lint findings).
    let fixture = FixtureTree::new();
    let root_a = fixture.root("root-a");
    let dir = root_a.join("dyninc");
    fs::create_dir_all(&dir).expect("create include directory");
    let top = dir.join("top.sv");
    let header = dir.join("config.mem");
    let top_text = concat!(
        "// llg-lsp-fixture: root-a/dyninc/top.sv\n",
        "`include \"config.mem\"\n",
        "module DynIncTop;\n",
        "  logic used_dyn_inc;\n",
        "  logic unused_dyn_inc;\n",
        "  assign used_dyn_inc = 1'b1;\n",
        "`ifdef DYN_EXTRA_SIGNAL\n",
        "  logic unused_dyn_extra;\n",
        "`endif\n",
        "endmodule\n",
    );
    fs::write(&top, top_text).expect("write top");
    fs::write(&header, "// llg-lsp-fixture: root-a/dyninc/config.mem\n").expect("write header");

    // Enable unused-signal so the macro flip is observable in diagnostics.
    fs::write(
        root_a.join(CONFIG_FILE),
        "schema_version = 1\n\
         [sources]\n\
         directories = [\".\"]\n\
         include = [\"**/*.v\", \"**/*.sv\"]\n\
         exclude = [\"**/excluded/**\"]\n\
         [lint]\n\
         enabled = true\n\
         [lint.rules.unused-signal]\n\
         enabled = true\n\
         severity = \"warning\"\n",
    )
    .expect("write lint-enabled config");

    let mut client = LspProcess::spawn(&fixture.root);
    client
        .initialize(&[("root-a", &root_a)], default_init_options())
        .expect("initialize include-dep workspace");
    let top_uri = file_uri(&top);
    client
        .open(&top, &fs::read_to_string(&top).expect("read top"))
        .expect("open top");

    // Baseline: only the unconditional unused signal is reported.
    wait_for_diagnostics(&mut client, &top_uri, |params| {
        params
            .get("diagnostics")
            .and_then(Value::as_array)
            .is_some_and(|diagnostics| {
                diagnostics.iter().any(|diagnostic| {
                    diagnostic
                        .get("message")
                        .and_then(Value::as_str)
                        .is_some_and(|message| message.contains("unused_dyn_inc"))
                }) && diagnostics.iter().all(|diagnostic| {
                    diagnostic
                        .get("message")
                        .and_then(Value::as_str)
                        .is_none_or(|message| !message.contains("unused_dyn_extra"))
                })
            })
    });

    // Change the `.mem` include so it defines a macro consumed by top; the
    // resolved include dependency is watched, so this must flip the output.
    fs::write(
        &header,
        "// llg-lsp-fixture: root-a/dyninc/config.mem\n`define DYN_EXTRA_SIGNAL\n",
    )
    .expect("change include dependency");
    client
        .send_watch_event(&header, 2)
        .expect("send include-dep watch event");
    let flipped = wait_for_diagnostics(&mut client, &top_uri, |params| {
        params
            .get("diagnostics")
            .and_then(Value::as_array)
            .is_some_and(|diagnostics| {
                diagnostics.iter().any(|diagnostic| {
                    diagnostic
                        .get("message")
                        .and_then(Value::as_str)
                        .is_some_and(|message| message.contains("unused_dyn_extra"))
                })
            })
    });
    assert!(
        flipped
            .get("diagnostics")
            .and_then(Value::as_array)
            .is_some_and(|diagnostics| diagnostics.iter().any(|diagnostic| {
                diagnostic
                    .get("message")
                    .and_then(Value::as_str)
                    .is_some_and(|message| message.contains("unused_dyn_extra"))
            })),
        "editing the arbitrary-extension dep did not affect diagnostics: {flipped}"
    );

    let symbols = wait_for_workspace_symbols(&mut client, "DynIncTop", |result| {
        names(result).iter().any(|name| name == "DynIncTop")
    });
    assert_no_shadow_uris(&symbols);
    client.shutdown();
}

#[test]
fn lsp_stdio_dep_change_refreshes_every_dependent_root() {
    // A header living under root-b's tree is included by BOTH roots (allowed
    // because it sits under a configured include directory of each root).
    // Editing it must refresh BOTH roots' published diagnostics even though
    // the header classifies as an arbitrary-extension (`Other`) file: the
    // header defines a macro that each top consumes, so the flip is visible
    // in every dependent root's own output.
    let fixture = FixtureTree::new();
    let ext = fixture.root("ext-shared");
    fs::create_dir_all(&ext).expect("create external include dir");
    let header = ext.join("cross_hdr.svh");
    let header_text =
        "// llg-lsp-fixture: ext-shared/cross_hdr.svh\n// shared cross-root macro header\n";
    fs::write(&header, header_text).expect("write cross-root header");

    let mut roots = Vec::new();
    for name in ["cross-a", "cross-b"] {
        let root = fixture.root(name);
        let src = root.join("src");
        fs::create_dir_all(&src).expect("create source dir");
        // The external dir is a configured include directory of both roots,
        // so the include target is authorized for either of them.
        fs::write(
            root.join(CONFIG_FILE),
            "schema_version = 1\n\
                 [sources]\n\
                 directories = [\".\"]\n\
                 include = [\"**/*.v\", \"**/*.sv\"]\n\
                 [compile]\n\
                 include_dirs = [\"../ext-shared\"]\n\
                 [lint]\n\
                 enabled = true\n",
        )
        .expect("write cross-root config");
        let top = src.join("top.sv");
        let stem = name.replace('-', "_");
        let top_text = format!(
            "{SOURCE_HEADER} {name}/src/top.sv\n\
             `include \"../../ext-shared/cross_hdr.svh\"\n\
             module CrossUser_{stem};\n\
               logic used_{stem};\n\
               logic unused_base_{stem};\n\
               assign used_{stem} = 1'b1;\n\
             `ifdef CROSS_EXTRA_UNUSED\n\
               logic unused_extra_{stem};\n\
             `endif\n\
             endmodule\n"
        );
        fs::write(&top, &top_text).expect("write cross-root top");
        roots.push((name, root, top));
    }

    let mut client = LspProcess::spawn(&fixture.root);
    client
        .initialize(
            &[("cross-a", &roots[0].1), ("cross-b", &roots[1].1)],
            default_init_options(),
        )
        .expect("initialize cross-root workspace");

    // Baseline on both roots: only the unconditional unused signal shows up.
    for (_, _, top) in &roots {
        client
            .open(top, &fs::read_to_string(top).expect("read top"))
            .expect("open cross-root top");
        wait_for_diagnostics(&mut client, &file_uri(top), |params| {
            params
                .get("diagnostics")
                .and_then(Value::as_array)
                .is_some_and(|diagnostics| {
                    diagnostics.iter().any(|diagnostic| {
                        diagnostic
                            .get("message")
                            .and_then(Value::as_str)
                            .is_some_and(|message| message.contains("unused_base_"))
                    }) && diagnostics.iter().all(|diagnostic| {
                        diagnostic
                            .get("message")
                            .and_then(Value::as_str)
                            .is_none_or(|message| !message.contains("unused_extra_"))
                    })
                })
        });
    }

    // Edit the shared header; the dep is tracked by both roots, so the watch
    // event must refresh BOTH roots' diagnostics.
    fs::write(
        &header,
        format!("{header_text}`define CROSS_EXTRA_UNUSED\n"),
    )
    .expect("edit cross-root header");
    client
        .send_watch_event(&header, 2)
        .expect("send cross-root dep watch event");
    for (name, _, top) in &roots {
        let extra = format!("unused_extra_{}", name.replace('-', "_"));
        let flipped = wait_for_diagnostics(&mut client, &file_uri(top), |params| {
            params
                .get("diagnostics")
                .and_then(Value::as_array)
                .is_some_and(|diagnostics| {
                    diagnostics.iter().any(|diagnostic| {
                        diagnostic
                            .get("message")
                            .and_then(Value::as_str)
                            .is_some_and(|message| message.contains(&extra))
                    })
                })
        });
        assert!(
            flipped
                .get("diagnostics")
                .and_then(Value::as_array)
                .is_some_and(|diagnostics| diagnostics.iter().any(|diagnostic| {
                    diagnostic
                        .get("message")
                        .and_then(Value::as_str)
                        .is_some_and(|message| message.contains(extra.as_str()))
                })),
            "dependent root was not refreshed by the dep change: {flipped}"
        );
    }
    client.shutdown();
}

/// Diagnostics are published project-wide: a compilation unit that is NEVER
/// opened still receives its findings, and fixing it on disk refreshes its
/// publication through a watched-file event — all while only the other file
/// was ever opened.
#[test]
fn lsp_stdio_publishes_and_clears_diagnostics_for_never_opened_files() {
    let fixture = FixtureTree::new();
    let root = fixture.root("project-wide");
    let good = root.join("good.sv");
    let broken = root.join("broken.sv");
    // The fix removes the broken assignment; the file stays closed.  The
    // remaining diagnostics may still carry Surelog's own warnings (e.g. the
    // missing-timescale notice), so the refresh is asserted as "the syntax
    // error is gone", not "the list is empty".
    let original = fs::read_to_string(&broken).expect("read broken source");
    assert!(original.contains("assign broken_signal = ;"));
    let fixed = original.replace("  assign broken_signal = ;\n", "");
    assert_ne!(fixed, original);

    let mut client = LspProcess::spawn(fixture.base());
    client
        .initialize(&[("project-wide", &root)], default_init_options())
        .expect("initialize project-wide workspace");

    // Only file A (good.sv) is ever opened; B (broken.sv) stays closed.
    let good_text = fs::read_to_string(&good).expect("read good source");
    client.open(&good, &good_text).expect("open good.sv");

    // File B was never opened, yet its error reaches the client.
    let broken_uri = file_uri(&broken);
    let error_params = wait_for_diagnostics(&mut client, &broken_uri, has_error_or_warning);
    assert!(
        has_severity_1(&error_params),
        "never-opened file must publish its syntax error"
    );

    // The opened file keeps receiving its own publications alongside.
    wait_for_diagnostics(&mut client, &file_uri(&good), |_params| true);

    // Fixing B on disk + watched-file event refreshes B's diagnostics
    // without ever opening it.
    fs::write(&broken, fixed).expect("fix broken source on disk");
    client
        .send_watch_event(&broken, 2)
        .expect("watched change for fixed source");
    let refreshed = wait_for_diagnostics(&mut client, &broken_uri, |params| {
        !params
            .get("diagnostics")
            .and_then(Value::as_array)
            .is_some_and(|diagnostics| {
                diagnostics.iter().any(|diagnostic| {
                    diagnostic
                        .get("message")
                        .and_then(Value::as_str)
                        .is_some_and(|message| message.contains("Syntax error"))
                })
            })
    });
    assert!(
        !has_severity_1(&refreshed),
        "fixed never-opened file must lose its error: {refreshed}"
    );
    client.shutdown();
}

/// A project whose sources contain a Surelog ERROR still serves navigation
/// features from the partial analysis: Surelog elaborates the surviving set
/// (the error here is a failed include, which does not abort the UHDM
/// stage), so documentSymbol returns modules and hover resolves the
/// declaration even though the whole-project analysis never reaches strict
/// validity.  Diagnostics keep publishing regardless.
#[test]
fn lsp_stdio_error_project_still_serves_features() {
    let fixture = FixtureTree::new();
    let root = fixture.root("err-proj");
    let src = root.join("src");
    fs::create_dir_all(&src).expect("create err-proj root");
    fs::write(
        root.join(CONFIG_FILE),
        "schema_version = 1\n\
         [sources]\n\
         directories = [\".\"]\n\
         include = [\"**/*.v\", \"**/*.sv\"]\n",
    )
    .expect("write err-proj config");
    let clean = src.join("clean_mod.sv");
    // The task's named clean module: `module clean_mod(...)`.
    let clean_text = concat!(
        "// llg-lsp-fixture: err-proj/src/clean_mod.sv\n",
        "module clean_mod (\n",
        "  input logic clk\n",
        ");\n",
        "  logic value;\n",
        "endmodule\n"
    );
    fs::write(&clean, clean_text).expect("write clean source");
    // Guaranteed Surelog ERROR (Severity::Error, no syntax cascade): the
    // include target does not exist anywhere in the configured dirs.
    let broken = src.join("broken_include.sv");
    fs::write(
        &broken,
        concat!(
            "// llg-lsp-fixture: err-proj/src/broken_include.sv\n",
            "`include \"missing_defs.svh\"\n",
            "module broken_inc;\n",
            "  logic bi_signal;\n",
            "endmodule\n"
        ),
    )
    .expect("write include-broken source");

    let mut client = LspProcess::spawn(fixture.base());
    client
        .initialize(&[("err-proj", &root)], default_init_options())
        .expect("initialize err-proj workspace");

    // Premise: the project really is tainted — the broken unit publishes an
    // error while it is NEVER opened.
    let broken_uri = file_uri(&broken);
    wait_for_diagnostics(&mut client, &broken_uri, has_severity_1);

    // didOpen the clean file; features must be served for it despite the
    // project-wide error.
    client.open(&clean, clean_text).expect("open clean source");
    let clean_uri = file_uri(&clean);
    let symbols = wait_for_document_symbols(&mut client, &clean_uri, |result| {
        names(result).iter().any(|name| name == "clean_mod")
    });
    assert_has_name(&symbols, "clean_mod");
    assert_no_shadow_uris(&symbols);
    let hover = client
        .request(
            "textDocument/hover",
            json!({
                "textDocument": { "uri": clean_uri },
                "position": position_at(clean_text, "module clean_mod", "module ".len())
            }),
        )
        .expect("hover on clean module declaration");
    assert!(
        !hover.is_null(),
        "hover on clean_mod must be served from the error project: {hover}"
    );
    assert!(hover.get("contents").is_some(), "hover shape: {hover}");
    client.shutdown();
}

/// A project with a SYNTAX error makes Surelog skip its whole compile/UHDM
/// stage, but the parse tree survives: declaration-level features must serve
/// (parse-tree fallback) instead of leaving the root feature-less until the
/// file is fixed.  The opened clean file gets its module document symbol and
/// hover, and the workspace index finds declarations even inside the
/// unterminated broken unit; instance-level data stays unavailable.
#[test]
fn lsp_stdio_syntax_error_still_serves_declarations() {
    let fixture = FixtureTree::new();
    let root = fixture.root("decl-fallback");
    let src = root.join("src");
    fs::create_dir_all(&src).expect("create decl-fallback root");
    fs::write(
        root.join(CONFIG_FILE),
        "schema_version = 1\n\
         [sources]\n\
         directories = [\".\"]\n\
         include = [\"**/*.v\", \"**/*.sv\"]\n",
    )
    .expect("write decl-fallback config");
    let clean = src.join("clean_module.sv");
    let clean_text = concat!(
        "// llg-lsp-fixture: decl-fallback/src/clean_module.sv\n",
        "module clean_mod (\n",
        "  input logic clk\n",
        ");\n",
        "  logic value;\n",
        "endmodule\n"
    );
    fs::write(&clean, clean_text).expect("write clean source");
    // Unterminated module: a guaranteed Severity::Syntax error that skips
    // Surelog's compile/elaborate/UHDM stages entirely.
    let broken = src.join("broken.sv");
    fs::write(
        &broken,
        concat!(
            "// llg-lsp-fixture: decl-fallback/src/broken.sv\n",
            "module broken_unterminated;\n",
            "  logic bi_signal;\n"
        ),
    )
    .expect("write unterminated source");

    let mut client = LspProcess::spawn(fixture.base());
    client
        .initialize(&[("decl-fallback", &root)], default_init_options())
        .expect("initialize decl-fallback workspace");

    // Premise: the project really is syntax-broken — the unterminated module
    // publishes an error while it is NEVER opened.
    let broken_uri = file_uri(&broken);
    wait_for_diagnostics(&mut client, &broken_uri, has_severity_1);

    // didOpen the clean file: document symbols must include the declared
    // module despite the project-wide syntax failure.
    client.open(&clean, clean_text).expect("open clean source");
    let clean_uri = file_uri(&clean);
    let symbols = wait_for_document_symbols(&mut client, &clean_uri, |result| {
        names(result).iter().any(|name| name == "clean_mod")
    });
    assert_has_name(&symbols, "clean_mod");
    assert_no_shadow_uris(&symbols);

    // The workspace index finds the clean module AND the broken unit's
    // declaration (Surelog's parse-error recovery keeps its header).
    let workspace = wait_for_workspace_symbols(&mut client, "clean_mod", |result| {
        names(result).iter().any(|name| name == "clean_mod")
    });
    assert_has_name(&workspace, "clean_mod");
    assert_no_shadow_uris(&workspace);
    let broken_symbols = wait_for_workspace_symbols(&mut client, "broken_unterminated", |result| {
        names(result)
            .iter()
            .any(|name| name == "broken_unterminated")
    });
    assert_has_name(&broken_symbols, "broken_unterminated");

    // Hover on the clean module declaration is served.
    let hover = client
        .request(
            "textDocument/hover",
            json!({
                "textDocument": { "uri": clean_uri },
                "position": position_at(clean_text, "module clean_mod", "module ".len())
            }),
        )
        .expect("hover on clean_mod under syntax failure");
    assert!(
        !hover.is_null(),
        "hover on clean_mod must be served by the parse-tree fallback: {hover}"
    );
    assert!(hover.get("contents").is_some(), "hover shape: {hover}");
    client.shutdown();
}

/// A Fatal analysis (include-isolation preflight failure discovered by the
/// initial scan) is feature-less: no snapshot ever existed, so
/// documentSymbol answers null/empty while the preflight diagnostic still
/// arrives.
#[test]
fn lsp_stdio_fatal_analysis_stays_featureless() {
    let fixture = FixtureTree::new();
    let root = fixture.root("fatal-root");
    let src = root.join("src");
    fs::create_dir_all(&src).expect("create fatal fixture root");
    fs::write(
        root.join(CONFIG_FILE),
        "schema_version = 1\n\
         [sources]\n\
         directories = [\".\"]\n\
         include = [\"**/*.v\", \"**/*.sv\"]\n",
    )
    .expect("write fatal root config");
    let top = src.join("top.sv");
    fs::write(
        &top,
        concat!(
            "// llg-lsp-fixture: fatal-root/src/top.sv\n",
            "`include \"../../outside_fatal.svh\"\n",
            "module FatalTop; endmodule\n"
        ),
    )
    .expect("write escaping source");
    let top_uri = file_uri(&top);

    let mut client = LspProcess::spawn(fixture.base());
    client
        .initialize(&[("fatal-root", &root)], default_init_options())
        .expect("initialize fatal workspace");

    // The preflight failure is published against the compiled file even
    // though it was never opened.
    let fatal_params = wait_for_diagnostics(&mut client, &top_uri, |params| {
        params
            .get("diagnostics")
            .and_then(Value::as_array)
            .is_some_and(|diagnostics| {
                diagnostics.iter().any(|diagnostic| {
                    diagnostic
                        .get("message")
                        .and_then(Value::as_str)
                        .is_some_and(|message| {
                            message.contains("escapes configured source/include directories")
                        })
                })
            })
    });
    assert!(
        has_severity_1(&fatal_params),
        "preflight escape must publish an error: {fatal_params}"
    );

    // No servable snapshot ever existed: features stay empty/null.
    let symbols = wait_for_document_symbols(&mut client, &top_uri, Value::is_null);
    assert!(
        symbols.is_null(),
        "documentSymbol must stay feature-less on a fatal-only root: {symbols}"
    );
    let hover = client
        .request(
            "textDocument/hover",
            json!({
                "textDocument": { "uri": top_uri },
                "position": { "line": 2, "character": 10 }
            }),
        )
        .expect("hover request during fatal state");
    assert!(
        hover.is_null(),
        "hover must stay feature-less on a fatal-only root: {hover}"
    );
    client.shutdown();
}

/// Fixing the broken file through a watched-file event upgrades the
/// analysis.  While the syntax error stands Surelog produces no UHDM at all,
/// so only DECLARATION-LEVEL data serves (the module name comes from the
/// parse-tree fallback); after the fix the analysis becomes strictly valid —
/// the error publication disappears and the recovered module keeps its
/// workspace-index entry with full elaborated data.
#[test]
fn lsp_stdio_fixing_broken_file_upgrades_analysis_to_valid() {
    let fixture = FixtureTree::new();
    let root = fixture.root("project-wide");
    let broken = root.join("broken.sv");
    let original = fs::read_to_string(&broken).expect("read broken source");
    let fixed = original.replace("  assign broken_signal = ;\n", "");
    assert_ne!(fixed, original, "fixture must contain the broken assign");

    let mut client = LspProcess::spawn(fixture.base());
    client
        .initialize(&[("project-wide", &root)], default_init_options())
        .expect("initialize project-wide workspace");
    let broken_uri = file_uri(&broken);
    wait_for_diagnostics(&mut client, &broken_uri, has_severity_1);

    // While the parse error stands the project still serves declaration-level
    // data: the parse-tree fallback surfaces the module name.
    let before = wait_for_workspace_symbols(&mut client, "ProjectWideBroken", |result| {
        names(result).iter().any(|name| name == "ProjectWideBroken")
    });
    assert_has_name(&before, "ProjectWideBroken");

    // Fix the broken file ON DISK and notify through the watched-file event.
    fs::write(&broken, &fixed).expect("fix broken source on disk");
    client
        .send_watch_event(&broken, 2)
        .expect("watched change for fixed source");
    let refreshed = wait_for_diagnostics(&mut client, &broken_uri, |params| {
        !params
            .get("diagnostics")
            .and_then(Value::as_array)
            .is_some_and(|diagnostics| {
                diagnostics.iter().any(|diagnostic| {
                    diagnostic
                        .get("message")
                        .and_then(Value::as_str)
                        .is_some_and(|message| message.contains("Syntax error"))
                })
            })
    });
    assert!(
        !has_severity_1(&refreshed),
        "fixed file must lose its error: {refreshed}"
    );

    // The re-analysis is valid now: the recovered module joins the index.
    let symbols = wait_for_workspace_symbols(&mut client, "ProjectWideBroken", |result| {
        names(result).iter().any(|name| name == "ProjectWideBroken")
    });
    assert_has_name(&symbols, "ProjectWideBroken");
    client.shutdown();
}

#[test]
fn lsp_stdio_shared_external_file_aggregates_labeled_findings() {
    // Two roots share one external include directory with different compile
    // defines and lint severities.  The shared file is analyzed under both
    // configurations; its aggregated publication shows identical findings
    // exactly once and conflicting findings labeled per root.
    let fixture = FixtureTree::new();
    let ext = fixture.root("agg-ext");
    fs::create_dir_all(&ext).expect("create shared external dir");
    let header = ext.join("agg_hdr.svh");
    // `AGG_FLAG_NAME` is defined differently per root, so the flagged signal
    // declaration occupies the SAME physical line under both configurations
    // while its name (and therefore the lint message) differs.
    let header_text = concat!(
        "// llg-lsp-fixture: agg-ext/agg_hdr.svh\n",
        "`ifndef AGG_HDR_SVH\n",
        "`define AGG_HDR_SVH\n",
        "module AggShared;\n",
        "  logic [3:0] agg_narrow;\n",
        "  logic [7:0] agg_wide;\n",
        "  assign agg_wide = agg_narrow;\n",
        "  logic unused_shared_signal;\n",
        "  logic `AGG_FLAG_NAME ;\n",
        "endmodule\n",
        "`endif\n",
    );
    fs::write(&header, header_text).expect("write shared header");

    let mut roots = Vec::new();
    for (name, define) in [
        ("agg-a", "AGG_FLAG_NAME=agg_flag_a"),
        ("agg-b", "AGG_FLAG_NAME=agg_flag_b"),
    ] {
        let root = fixture.root(name);
        let src = root.join("src");
        fs::create_dir_all(&src).expect("create source dir");
        // agg-a promotes width-mismatch to a warning; agg-b keeps the Info
        // default.  Different defines rename the flag signal declared at the
        // same location in the shared header.
        let width_override = if name == "agg-a" {
            "[lint.rules.width-mismatch]\nseverity = \"warning\"\n"
        } else {
            ""
        };
        fs::write(
            root.join(CONFIG_FILE),
            format!(
                "schema_version = 1\n\
                 [sources]\n\
                 directories = [\".\"]\n\
                 include = [\"**/*.v\", \"**/*.sv\"]\n\
                 [compile]\n\
                 include_dirs = [\"../agg-ext\"]\n\
                 defines = [\"{define}\"]\n\
                 [lint]\n\
                 enabled = true\n\
                 {width_override}"
            ),
        )
        .expect("write aggregation config");
        let top = src.join("top.sv");
        let stem = name.replace('-', "_");
        let top_text = format!(
            "{SOURCE_HEADER} {name}/src/top.sv\n\
             `include \"../../agg-ext/agg_hdr.svh\"\n\
             module AggUser_{stem};\n\
             endmodule\n"
        );
        fs::write(&top, &top_text).expect("write aggregation top");
        roots.push((name, root, top));
    }

    let mut client = LspProcess::spawn(&fixture.root);
    client
        .initialize(
            &[("agg-a", &roots[0].1), ("agg-b", &roots[1].1)],
            default_init_options(),
        )
        .expect("initialize aggregation workspace");
    for (_, _, top) in &roots {
        client
            .open(top, &fs::read_to_string(top).expect("read top"))
            .expect("open aggregation top");
    }
    // Wait until both roots resolved the shared header as a dependency and
    // committed a valid analysis.
    for (_, _, top) in &roots {
        wait_for_diagnostics(&mut client, &file_uri(top), has_no_severity_1);
    }
    wait_for_workspace_symbols(&mut client, "AggUser_", |result| {
        let symbol_names = names(result);
        symbol_names.iter().any(|name| name == "AggUser_agg_a")
            && symbol_names.iter().any(|name| name == "AggUser_agg_b")
    });

    // The union for the never-opened shared file is published by the
    // aggregation slice once both roots have committed.  Ownership ties
    // resolve deterministically to the lowest normalized root path (agg-a),
    // so its copies stay unlabeled while agg-b's differing copies carry
    // [agg-b].
    let header_uri = file_uri(&header);
    let aggregated = client
        .wait_for_notification_where("textDocument/publishDiagnostics", |params| {
            params.get("uri").and_then(Value::as_str) == Some(header_uri.as_str())
                && params
                    .get("diagnostics")
                    .and_then(Value::as_array)
                    .is_some_and(|diagnostics| {
                        !diagnostics.is_empty()
                            && diagnostics.iter().any(|diagnostic| {
                                diagnostic
                                    .get("message")
                                    .and_then(Value::as_str)
                                    .is_some_and(|message| message.contains("[agg-b]"))
                            })
                    })
        })
        .expect("aggregated publication for the shared file");
    let aggregated = aggregated.get("params").cloned().unwrap_or(Value::Null);

    let diagnostics = aggregated
        .get("diagnostics")
        .and_then(Value::as_array)
        .expect("aggregated diagnostics array");
    let messages: Vec<&str> = diagnostics
        .iter()
        .filter_map(|diagnostic| diagnostic.get("message").and_then(Value::as_str))
        .collect();

    // Identical finding from both roots appears exactly once, unlabeled.
    let unused_plain = messages
        .iter()
        .filter(|message| message.contains("unused_shared_signal") && !message.contains('['))
        .count();
    assert_eq!(
        unused_plain, 1,
        "identical findings must appear once without a label: {messages:?}"
    );

    // The same-location finding differs per root (define-renamed signal):
    // the owner copy stays unlabeled, the non-owner copy carries [agg-b].
    assert!(
        messages
            .iter()
            .any(|message| message.contains("agg_flag_a") && !message.contains('[')),
        "owner copy must stay unlabeled: {messages:?}"
    );
    assert!(
        messages
            .iter()
            .any(|message| message.contains("agg_flag_b") && message.ends_with("[agg-b]")),
        "non-owner copy must carry the documented [root-name] label: {messages:?}"
    );

    // The width-mismatch pair fires identically in both roots except for the
    // per-root severity override: both copies survive at the same location.
    let width_findings = diagnostics
        .iter()
        .filter(|diagnostic| {
            diagnostic.get("code").and_then(Value::as_str) == Some("width-mismatch")
        })
        .count();
    assert_eq!(
        width_findings, 2,
        "differing-severity findings must both survive with labels: {messages:?}"
    );
    client.shutdown();
}

#[test]
fn lsp_stdio_semantic_tokens_use_current_open_buffer_and_cached_unopened_snapshot() {
    // Arrange
    let fixture = FixtureTree::new();
    let root = fixture.root("semantic-current");
    fs::create_dir_all(&root).expect("create semantic-token workspace");
    fs::write(
        root.join(CONFIG_FILE),
        "schema_version = 1\n\n[sources]\ndirectories = [\".\"]\ninclude = [\"**/*.v\", \"**/*.sv\"]\n\n[compile]\ndefines = [\"SEMANTIC_ONLY=1\"]\n",
    )
    .expect("write semantic-token config");
    let opened = root.join("opened.sv");
    let unopened = root.join("unopened.sv");
    let other = root.join("other_unit.sv");
    let include = root.join("not_consumed.svh");
    let opened_disk = format!(
        "{SOURCE_HEADER} semantic-current/opened.sv\nmodule DiskOpened;\n  logic disk_signal;\nendmodule\n"
    );
    let unopened_snapshot = format!(
        "{SOURCE_HEADER} semantic-current/unopened.sv\nmodule CachedUnopened;\n  logic cached_signal;\nendmodule\n"
    );
    fs::write(&opened, &opened_disk).expect("write opened disk snapshot");
    fs::write(&unopened, &unopened_snapshot).expect("write unopened snapshot");
    fs::write(
        &other,
        format!(
            "{SOURCE_HEADER} semantic-current/other_unit.sv\nmodule UnrelatedProjectUnit; endmodule\n"
        ),
    )
    .expect("write unrelated project unit");
    fs::write(
        &include,
        format!(
            "{SOURCE_HEADER} semantic-current/not_consumed.svh\nmodule IncludedUnit; endmodule\n"
        ),
    )
    .expect("write include sentinel");
    let mut client = LspProcess::spawn(fixture.base());
    client
        .initialize(&[("semantic-current", &root)], default_init_options())
        .expect("initialize semantic-token workspace");

    // Change the closed file without a watched-file notification.  Its LSP
    // result must continue to come from the initialized project snapshot.
    let unopened_new_disk = format!(
        "{SOURCE_HEADER} semantic-current/unopened.sv\n\n\n\n\n\n\n\nmodule NewDiskOnly; endmodule\n"
    );
    fs::write(&unopened, unopened_new_disk).expect("replace unopened disk text");

    // Act
    let unopened_tokens = client
        .request(
            "textDocument/semanticTokens/full",
            json!({ "textDocument": { "uri": file_uri(&unopened) } }),
        )
        .expect("request unopened semantic tokens");
    let opened_buffer = format!(
        "{SOURCE_HEADER} semantic-current/opened.sv\n`include \"not_consumed.svh\"\n\n\n\n\nmodule UnsavedOpened;\n  logic unsaved_signal;\nendmodule\n"
    );
    client
        .open(&opened, &opened_buffer)
        .expect("open unsaved semantic-token buffer");
    let opened_tokens = client
        .request(
            "textDocument/semanticTokens/full",
            json!({ "textDocument": { "uri": file_uri(&opened) } }),
        )
        .expect("request opened semantic tokens");

    // Assert
    let unopened_positions = semantic_token_positions(&unopened_tokens);
    assert!(
        !unopened_positions.is_empty(),
        "cached unopened semantic tokens must be non-empty"
    );
    assert!(
        unopened_positions.contains(&(1, 0)),
        "unopened file did not retain the cached module position: {unopened_positions:?}"
    );
    assert!(
        unopened_positions.iter().all(|(line, _)| *line < 8),
        "unopened file was reparsed from changed disk text: {unopened_positions:?}"
    );

    let opened_positions = semantic_token_positions(&opened_tokens);
    assert!(
        !opened_positions.is_empty(),
        "opened semantic tokens must be non-empty"
    );
    assert!(
        opened_positions.contains(&(6, 0)),
        "opened file did not use the current in-memory module position: {opened_positions:?}"
    );

    // A syntax-broken current buffer must produce no semantic tokens.  In
    // particular, it must neither expose a partial parse-only stream nor
    // fall back to the last valid project snapshot.
    let syntax_buffer =
        format!("{SOURCE_HEADER} semantic-current/opened.sv\n\n\n\n\nmodule SyntaxOnly;\n");
    client
        .change(&opened, 2, &syntax_buffer)
        .expect("change opened buffer to syntax-broken source");
    let syntax_tokens = client
        .request(
            "textDocument/semanticTokens/full",
            json!({ "textDocument": { "uri": file_uri(&opened) } }),
        )
        .expect("request syntax-broken opened semantic tokens");
    assert!(
        semantic_token_positions(&syntax_tokens).is_empty(),
        "syntax-broken current buffer returned semantic tokens: {syntax_tokens}"
    );

    // A later complete revision must not be pinned to the cached empty result
    // from the broken text: the text hash changes and valid unsaved
    // highlighting becomes available again immediately.
    let recovered_buffer = format!(
        "{SOURCE_HEADER} semantic-current/opened.sv\n\nmodule RecoveredOpened;\n  logic recovered_signal;\nendmodule\n"
    );
    client
        .change(&opened, 3, &recovered_buffer)
        .expect("repair opened semantic-token buffer");
    let recovered_tokens = client
        .request(
            "textDocument/semanticTokens/full",
            json!({ "textDocument": { "uri": file_uri(&opened) } }),
        )
        .expect("request repaired opened semantic tokens");
    assert!(
        !semantic_token_positions(&recovered_tokens).is_empty(),
        "repaired current buffer did not recover semantic tokens: {recovered_tokens}"
    );

    // Act: replace the token-bearing open buffer with whitespace only.
    client
        .change(&opened, 4, " \n\t\n")
        .expect("change opened buffer to whitespace");
    let empty_opened_tokens = client
        .request(
            "textDocument/semanticTokens/full",
            json!({ "textDocument": { "uri": file_uri(&opened) } }),
        )
        .expect("request whitespace-only opened semantic tokens");

    // Assert: a successful empty parse is authoritative, not a reason to
    // return the token-bearing cached project snapshot.
    assert!(
        semantic_token_positions(&empty_opened_tokens).is_empty(),
        "whitespace-only current buffer returned stale cached semantic tokens: {empty_opened_tokens}"
    );
    client.shutdown();
}

/// Connection LABELS are visually distinguishable from the SIGNALS connected
/// to them: every named port `.label` and parameter-override `#.LABEL` token
/// carries the custom `connectionLabel` modifier while the connected
/// actual/RHS identifiers stay plain.  Asserted in BOTH serving paths:
///
/// * the cached PROJECT path (unopened document, full analysis with UHDM),
/// * the ISOLATED open-buffer path (`-parseonly` over the staged buffer —
///   no project model, so the marking must be purely syntactic).
#[test]
fn lsp_stdio_semantic_tokens_mark_connection_labels_in_both_serving_paths() {
    let fixture = FixtureTree::new();
    let root_a = fixture.root("root-a");
    let labels_path = root_a.join("bindings").join("labels").join("top.sv");
    let labels_text = fs::read_to_string(&labels_path).expect("read label_top fixture");
    let labels_uri = file_uri(&labels_path);

    let mut client = LspProcess::spawn(&fixture.root);
    let initialize = client
        .initialize(&[("root-a", &root_a)], default_init_options())
        .expect("initialize connection-label workspace");
    let legend = initialize
        .get("capabilities")
        .and_then(|capabilities| capabilities.get("semanticTokensProvider"))
        .and_then(|provider| provider.get("legend"))
        .cloned()
        .expect("semanticTokensProvider.legend");
    let legend_types = legend_names(&legend, "tokenTypes");
    let legend_modifiers = legend_names(&legend, "tokenModifiers");
    assert!(
        legend_modifiers
            .iter()
            .any(|name| name == "connectionLabel"),
        "legend must advertise the connectionLabel modifier: {legend_modifiers:?}"
    );

    wait_for_diagnostics(&mut client, &labels_uri, has_no_severity_1);

    // ── Path 1: unopened document → cached project-index tokens ──────────
    let cached = client
        .request(
            "textDocument/semanticTokens/full",
            json!({ "textDocument": { "uri": labels_uri } }),
        )
        .expect("cached semantic tokens request");
    let rows = semantic_token_rows(&cached, &legend_types, &legend_modifiers);
    assert!(!rows.is_empty(), "cached token stream must be non-empty");

    // `label_child u_child(.clk(wa));` — the label token starts at the
    // identifier AFTER the dot, the actual at the identifier after the paren.
    let (clk_line, clk_dot_col) = position_of(&labels_text, ".clk", 0);
    let clk_row = row_at(&rows, clk_line, clk_dot_col + 1);
    assert_eq!(clk_row.token_type, "function", "clk row: {clk_row:?}");
    assert!(
        clk_row
            .modifiers
            .iter()
            .any(|name| name == "connectionLabel"),
        ".clk label must carry connectionLabel: {clk_row:?}"
    );
    let (wa_line, wa_paren_col) = position_of(&labels_text, "(wa)", 0);
    let wa_row = row_at(&rows, wa_line, wa_paren_col + 1);
    assert_eq!(wa_row.token_type, "variable", "wa row: {wa_row:?}");
    assert!(
        !wa_row
            .modifiers
            .iter()
            .any(|name| name == "connectionLabel"),
        "connected signal must NOT carry connectionLabel: {wa_row:?}"
    );

    // ── Path 2: open buffer → isolated parse-only tokens ─────────────────
    // The buffer exercises single-line AND multi-line instantiations with
    // named PORT connections and named PARAMETER overrides.
    let iso_path = root_a.join("iso_labels.sv");
    fs::write(&iso_path, format!("{SOURCE_HEADER} placeholder\n")).expect("seed iso file");
    let iso_uri = file_uri(&iso_path);
    let iso_buffer = format!(
        "{SOURCE_HEADER} root-a/iso_labels.sv\n\
module iso_top;\n  logic wa;\n  logic [7:0] tq;\n\n\
  p_pchild #(.W(4), .D(wa)) u_iso (.clk(wa), .q(tq));\n\n\
  p_pchild #(\n    .W(8),\n    .D(1)\n  ) u_ml (\n    .clk(wa),\n    .q(tq)\n  );\n\
endmodule\n"
    );
    client
        .open(&iso_path, &iso_buffer)
        .expect("open iso buffer");
    let isolated = client
        .request(
            "textDocument/semanticTokens/full",
            json!({ "textDocument": { "uri": iso_uri } }),
        )
        .expect("isolated semantic tokens request");
    let iso_rows = semantic_token_rows(&isolated, &legend_types, &legend_modifiers);
    assert!(
        !iso_rows.is_empty(),
        "isolated token stream must be non-empty"
    );

    let expect_label = |needle: &str, occurrence: usize, want_type: &str| -> Vec<String> {
        let (line, dot_col) = position_of(&iso_buffer, needle, occurrence);
        let row = row_at(&iso_rows, line, dot_col + 1);
        assert_eq!(row.token_type, want_type, "{needle} row: {row:?}");
        assert!(
            row.modifiers.iter().any(|name| name == "connectionLabel"),
            "{needle} label must carry connectionLabel: {row:?}"
        );
        row.modifiers.clone()
    };
    let expect_plain = |text: &str, needle: &str, occurrence: usize, offset: usize| {
        let (line, col) = position_of(text, needle, occurrence);
        let row = row_at(&iso_rows, line, col + offset as u64);
        assert_eq!(row.token_type, "variable", "{needle} row: {row:?}");
        assert!(
            !row.modifiers.iter().any(|name| name == "connectionLabel"),
            "connected signal at {needle}+{offset} must NOT carry connectionLabel: {row:?}"
        );
    };

    // Single-line instantiation: param override labels …
    let w_mods = expect_label(".W", 0, "property");
    assert!(
        w_mods.iter().any(|name| name == "readonly"),
        "override label keeps its readonly base modifier: {w_mods:?}"
    );
    expect_label(".D", 0, "property");
    // … port labels …
    expect_label(".clk", 0, "function");
    expect_label(".q", 0, "function");
    // … and the connected signals stay plain.
    expect_plain(&iso_buffer, ".D(wa)", 0, 3);
    expect_plain(&iso_buffer, ".clk(wa)", 0, 5);
    expect_plain(&iso_buffer, ".q(tq)", 0, 3);

    // Multi-line instantiation: identical marking on continuation lines.
    expect_label(".W", 1, "property");
    expect_label(".D", 1, "property");
    expect_label(".clk", 1, "function");
    expect_label(".q", 1, "function");
    expect_plain(&iso_buffer, ".clk(wa)", 1, 5);
    expect_plain(&iso_buffer, ".q(tq)", 1, 3);

    client.shutdown();
}

#[test]
fn lsp_stdio_read_only_shadow_and_clean_shutdown() {
    // The server stages unsaved buffers under a private per-process temp
    // shadow and must never create a shadow tree inside the project.
    let fixture = FixtureTree::new();
    let root_a = fixture.root("root-a");
    let path = root_a.join("navigation").join("snapshot.sv");
    let valid = fs::read_to_string(&path).expect("read snapshot fixture");
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

    // The server was launched with its CWD inside this fixture tree; Surelog
    // side-effects (slpp_all/, logs) must never appear anywhere inside it.
    fn assert_no_surelog_artifacts(dir: &Path) {
        for entry in fs::read_dir(dir).expect("read fixture tree") {
            let entry = entry.expect("fixture entry");
            let name = entry.file_name();
            assert!(
                name != std::ffi::OsStr::new("slpp_all")
                    && name != std::ffi::OsStr::new("surelog.log")
                    && name != std::ffi::OsStr::new("uhdm.log")
                    && name != std::ffi::OsStr::new("surelog.uhdm.log"),
                "Surelog artifact leaked into the project tree: {}",
                entry.path().display()
            );
            if entry.file_type().expect("entry type").is_dir() {
                assert_no_surelog_artifacts(&entry.path());
            }
        }
    }
    assert_no_surelog_artifacts(fixture.base());
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

/// An override config reached through `initializationOptions.llg.configFiles`
/// may carry ANY basename; editing it must reload that root without a restart
/// exactly like `<root>/llg.toml` does (review B).
#[test]
fn lsp_stdio_override_config_watch_reloads_without_restart() {
    let fixture = FixtureTree::new();
    let root_a = fixture.root("root-a");
    let custom_config = root_a.join("custom.toml");
    let config_with_rule = |severity: &str| {
        format!(
            "schema_version = 1\n\
             [sources]\n\
             directories = [\".\"]\n\
             include = [\"**/*.v\", \"**/*.sv\"]\n\
             exclude = [\"**/excluded/**\"]\n\
             [lint]\n\
             enabled = true\n\
             [lint.rules.unused-signal]\n\
             enabled = true\n\
             severity = \"{severity}\"\n"
        )
    };
    fs::write(&custom_config, config_with_rule("error")).expect("write override config");

    let mut client = LspProcess::spawn(&fixture.root);
    let options = init_options_with_config_files(&[(
        file_uri(&root_a).as_str(),
        custom_config.to_str().expect("config path"),
    )]);
    client
        .initialize(&[("root-a", &root_a)], options)
        .expect("initialize override workspace");

    let path = root_a.join("lint").join("per_root.sv");
    let uri = file_uri(&path);
    client
        .open(&path, &fs::read_to_string(&path).expect("read lint source"))
        .expect("open lint source");
    let baseline = wait_for_diagnostics(&mut client, &uri, |params| {
        lint_severity(params, "unused-signal") == Some(1)
    });
    assert_eq!(
        lint_severity(&baseline, "unused-signal"),
        Some(1),
        "override config policy must be active initially: {baseline}"
    );

    // Modify the OVERRIDE config (not llg.toml): unused-signal demoted to a
    // warning.  The watch event classifies as `Other` by basename and must
    // still be routed to this root via its effective config path.
    fs::write(&custom_config, config_with_rule("warning")).expect("update override config");
    client
        .send_watch_event(&custom_config, 2)
        .expect("send override-config watch event");
    let updated = wait_for_diagnostics(&mut client, &uri, |params| {
        lint_severity(params, "unused-signal") == Some(2)
    });
    assert_eq!(
        lint_severity(&updated, "unused-signal"),
        Some(2),
        "editing the override config must refresh diagnostics without restart: {updated}"
    );
    client.shutdown();
}

/// Hover over a declaration returns markup content, and completion returns a
/// valid non-empty response shape (review G: these requests had no stdio
/// coverage).
#[test]
fn lsp_stdio_hover_and_completion_return_valid_shapes() {
    let fixture = FixtureTree::new();
    let root_a = fixture.root("root-a");
    let path = root_a.join("navigation").join("snapshot.sv");
    let valid = fs::read_to_string(&path).expect("read snapshot fixture");
    let uri = file_uri(&path);
    let mut client = LspProcess::spawn(&fixture.root);
    client
        .initialize(&[("root-a", &root_a)], default_init_options())
        .expect("initialize hover workspace");
    client.open(&path, &valid).expect("open snapshot source");
    wait_for_diagnostics(&mut client, &uri, has_no_severity_1);

    // Hover over the module name of the module DECLARATION.
    let hover = client
        .request(
            "textDocument/hover",
            json!({
                "textDocument": { "uri": uri },
                "position": position_at(&valid, "module SnapshotTop", 9)
            }),
        )
        .expect("hover request");
    let contents = hover
        .as_object()
        .and_then(|hover| hover.get("contents"))
        .expect("hover result must carry contents");
    let value = match contents.get("value").and_then(Value::as_str) {
        Some(markup) => markup.to_owned(),
        None => contents.to_string(),
    };
    assert!(
        value.contains("SnapshotTop"),
        "hover over the module name must mention it: {value:?}"
    );
    assert_no_shadow_uris(&hover);

    // Completion anywhere in the indexed file yields a well-formed,
    // non-empty item list (array form or {items:[...]}).
    let completion = client
        .request(
            "textDocument/completion",
            json!({
                "textDocument": { "uri": uri },
                "position": position_at(&valid, "endmodule", 0)
            }),
        )
        .expect("completion request");
    let items = match &completion {
        Value::Array(items) => items.clone(),
        Value::Object(object) => object
            .get("items")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default(),
        other => panic!("completion returned an invalid shape: {other}"),
    };
    assert!(
        !items.is_empty(),
        "completion in an indexed file must return items: {completion}"
    );
    assert!(
        items.iter().all(|item| item.get("label").is_some()),
        "every completion item needs a label: {items:?}"
    );
    assert_no_shadow_uris(&completion);
    client.shutdown();
}

/// Watchers are re-registered with the SAME registration id once a later
/// valid commit resolves new include dependencies (digest change), so the
/// exact dep path becomes watched (review G).
#[test]
fn lsp_stdio_watchers_reregister_after_dep_resolution_changes_digest() {
    let fixture = FixtureTree::new();
    let root_a = fixture.root("root-a");
    let dir = root_a.join("dynreg");
    fs::create_dir_all(&dir).expect("create dynamic registration directory");
    let top = dir.join("top.sv");
    let initial_text = format!(
        "{SOURCE_HEADER} root-a/dynreg/top.sv\nmodule DynRegTop;\n  logic used_dyn_reg;\nendmodule\n"
    );
    fs::write(&top, &initial_text).expect("write reregistration top");
    let dep = dir.join("dep.inc");
    fs::write(
        &dep,
        format!("{SOURCE_HEADER} root-a/dynreg/dep.inc\n`define DYN_REG_EXTRA\n"),
    )
    .expect("write reregistration include dep");

    let mut client = LspProcess::spawn(&fixture.root);
    client
        .initialize(&[("root-a", &root_a)], default_init_options())
        .expect("initialize watcher-reregistration workspace");

    let is_watch_registration = |message: &Value| {
        message
            .get("params")
            .and_then(|params| params.get("registrations"))
            .and_then(Value::as_array)
            .is_some_and(|registrations| {
                registrations.iter().any(|registration| {
                    registration.get("method").and_then(Value::as_str)
                        == Some("workspace/didChangeWatchedFiles")
                        && registration.get("id").and_then(Value::as_str)
                            == Some("llg-watched-files")
                })
            })
    };
    let watchers_of = |message: &Value| -> Vec<String> {
        message
            .get("params")
            .and_then(|params| params.get("registrations"))
            .and_then(Value::as_array)
            .and_then(|registrations| {
                registrations.iter().find_map(|registration| {
                    registration
                        .get("registerOptions")
                        .and_then(|options| options.get("watchers"))
                        .and_then(Value::as_array)
                        .map(|watchers| {
                            watchers
                                .iter()
                                .filter_map(|watcher| {
                                    watcher.get("globPattern").and_then(Value::as_str)
                                })
                                .map(str::to_owned)
                                .collect::<Vec<_>>()
                        })
                })
            })
            .unwrap_or_default()
    };

    // First registration: covers the effective config and *.v/*.sv globs but
    // not the not-yet-resolved include dependency.
    let first = client
        .wait_for_server_request_where(|message| is_watch_registration(message))
        .expect("first watched-file registration");
    let first_watchers = watchers_of(&first);
    assert!(
        !first_watchers
            .iter()
            .any(|pattern| pattern.ends_with("dep.inc")),
        "precondition: dep must be unwatched before resolution: {first_watchers:?}"
    );

    // A later valid commit resolves the include dependency; the digest
    // changes, forcing a SECOND registration that reuses the same id and now
    // watches the exact dep path.
    let resolved_text = format!(
        "{SOURCE_HEADER} root-a/dynreg/top.sv\n\
         `include \"dep.inc\"\n\
         module DynRegTop;\n\
           logic used_dyn_reg;\n\
         `ifdef DYN_REG_EXTRA\n\
           logic unused_dyn_reg_extra;\n\
         `endif\n\
         endmodule\n"
    );
    client
        .change(&top, 2, &resolved_text)
        .expect("resolve include dependency via didChange");
    let deadline = Instant::now() + REQUEST_TIMEOUT;
    loop {
        let count = client
            .all_server_requests
            .iter()
            .filter(|message| is_watch_registration(message))
            .count();
        if count >= 2 {
            break;
        }
        let message = client
            .receive_until(deadline)
            .expect("second watched-file registration never arrived after dep resolution");
        client.route_unsolicited(message).expect("route message");
    }
    let registrations: Vec<Value> = client
        .all_server_requests
        .iter()
        .filter(|message| is_watch_registration(message))
        .cloned()
        .collect();
    let second = &registrations[1];
    assert_eq!(
        second
            .get("params")
            .and_then(|params| params.get("registrations"))
            .and_then(Value::as_array)
            .and_then(|regs| regs.first())
            .and_then(|registration| registration.get("id"))
            .and_then(Value::as_str),
        first
            .get("params")
            .and_then(|params| params.get("registrations"))
            .and_then(Value::as_array)
            .and_then(|regs| regs.first())
            .and_then(|registration| registration.get("id"))
            .and_then(Value::as_str),
        "re-registration must reuse the SAME registration id"
    );
    let second_watchers = watchers_of(second);
    assert!(
        second_watchers
            .iter()
            .any(|pattern| pattern.ends_with("dep.inc")),
        "re-registered watchers must cover the newly resolved dep: {second_watchers:?}"
    );
    assert_ne!(
        first_watchers, second_watchers,
        "the watcher set must have changed to trigger re-registration"
    );
    client.shutdown();
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

/// Goto-definition is binding-precise: two modules in distinct files each
/// declare `logic clk;`, and tb instantiates both with plain signal uses of
/// each instance-scope net.  A request at every use returns exactly ONE
/// location equal to that module's declaration file+line, and a request on a
/// declaration resolves to itself.
#[test]
fn lsp_stdio_goto_definition_is_binding_precise() {
    let fixture = FixtureTree::new();
    let root_a = fixture.root("root-a");
    let nets_dir = root_a.join("bindings").join("nets");
    let ma_path = nets_dir.join("m_a.sv");
    let mb_path = nets_dir.join("m_b.sv");
    let tb_path = nets_dir.join("tb.sv");
    let ma_text = fs::read_to_string(&ma_path).expect("read m_a fixture");
    let mb_text = fs::read_to_string(&mb_path).expect("read m_b fixture");
    let tb_text = fs::read_to_string(&tb_path).expect("read tb fixture");

    let mut client = LspProcess::spawn(&fixture.root);
    client
        .initialize(&[("root-a", &root_a)], default_init_options())
        .expect("initialize binding-precision workspace");
    // Opening one file triggers the root-wide compile; a clean diagnostic
    // publication for it proves the analysis carries feature data.
    client.open(&tb_path, &tb_text).expect("open tb source");
    wait_for_diagnostics(&mut client, &file_uri(&tb_path), has_no_severity_1);

    // The declaration positions of `logic clk;` in both module files.
    let ma_decl_start = position_at(&ma_text, "logic clk", 6);
    let mb_decl_start = position_at(&mb_text, "logic clk", 6);

    // Definition at the `clk` USE inside m_a (`assign clk = ...`) → exactly
    // one location: m_a's own `logic clk` declaration.
    let response = client
        .request(
            "textDocument/definition",
            json!({
                "textDocument": { "uri": file_uri(&ma_path) },
                "position": position_at(&ma_text, "assign clk", 7)
            }),
        )
        .expect("definition request at m_a clk use");
    let (uri, start) = single_location(&response, "definition at m_a use");
    assert_eq!(uri, file_uri(&ma_path), "m_a use → m_a decl");
    assert_eq!(start, ma_decl_start, "start: {start} vs {ma_decl_start}");
    assert_no_shadow_uris(&response);

    // Same for m_b: its use must land on ITS OWN declaration, not m_a's.
    let response = client
        .request(
            "textDocument/definition",
            json!({
                "textDocument": { "uri": file_uri(&mb_path) },
                "position": position_at(&mb_text, "assign clk", 7)
            }),
        )
        .expect("definition request at m_b clk use");
    let (uri, start) = single_location(&response, "definition at m_b use");
    assert_eq!(uri, file_uri(&mb_path), "m_b use → m_b decl");
    assert_eq!(start, mb_decl_start, "start: {start} vs {mb_decl_start}");
    assert_no_shadow_uris(&response);

    // Definition ON the declaration itself → itself.
    let response = client
        .request(
            "textDocument/definition",
            json!({
                "textDocument": { "uri": file_uri(&ma_path) },
                "position": ma_decl_start
            }),
        )
        .expect("definition request on m_a decl");
    let (uri, start) = single_location(&response, "definition on m_a decl");
    assert_eq!(uri, file_uri(&ma_path));
    assert_eq!(start, ma_decl_start);
    assert_no_shadow_uris(&response);

    // Plain signal uses of each instance-scope connection net in tb resolve
    // to tb's own declarations.
    for (net, use_needle, decl_needle) in [("wa", "wa =", "logic wa"), ("wb", "wb =", "logic wb")] {
        let expected = position_at(&tb_text, decl_needle, 6);
        let response = client
            .request(
                "textDocument/definition",
                json!({
                    "textDocument": { "uri": file_uri(&tb_path) },
                    "position": position_at(&tb_text, use_needle, 0)
                }),
            )
            .unwrap_or_else(|error| panic!("definition request at {net} use: {error}"));
        let (uri, start) = single_location(&response, &format!("definition at {net} use"));
        assert_eq!(uri, file_uri(&tb_path), "{net} use → tb decl");
        assert_eq!(start, expected, "{net}: {start} vs {expected}");
        assert_no_shadow_uris(&response);
    }

    // Cursor normalization: repeating the m_a `clk`-use definition request at
    // the LAST character column of the same identifier (col + name_len - 1)
    // must return the identical single Location — ref bindings are keyed at
    // the token-start column, which a mid-identifier cursor now reuses.
    let clk_use_col = 7;
    let clk_name_len = "clk".chars().count();
    let response = client
        .request(
            "textDocument/definition",
            json!({
                "textDocument": { "uri": file_uri(&ma_path) },
                "position": position_at(&ma_text, "assign clk", clk_use_col + clk_name_len - 1)
            }),
        )
        .expect("definition request at m_a clk use (last character)");
    let (uri, start) = single_location(&response, "definition at m_a use (last character)");
    assert_eq!(uri, file_uri(&ma_path), "m_a last-char click → m_a decl");
    assert_eq!(start, ma_decl_start);
    assert_no_shadow_uris(&response);
    client.shutdown();
}

/// A module type and an instance identifier occupy different namespaces for
/// navigation.  In the original `foo.v` regression, `Bar Bar(...)` caused a
/// later `Bar u_bar(...)` type reference to jump to the first instance name
/// instead of the `module Bar` declaration.
#[test]
fn lsp_stdio_goto_definition_module_type_ignores_same_named_instance() {
    // Arrange
    let fixture = FixtureTree::new();
    let root_a = fixture.root("root-a");
    let source_path = root_a.join("navigation").join("foo.v");
    let source_text = fs::read_to_string(&source_path).expect("read foo.v regression fixture");
    let source_uri = file_uri(&source_path);
    let expected_module = position_at(&source_text, "module Bar", "module ".len());

    let mut client = LspProcess::spawn(fixture.base());
    client
        .initialize(&[("root-a", &root_a)], default_init_options())
        .expect("initialize module-instance collision workspace");
    client
        .open(&source_path, &source_text)
        .expect("open foo.v regression fixture");
    wait_for_diagnostics(&mut client, &source_uri, has_no_severity_1);

    // Act + Assert: both module-type occurrences resolve to the module
    // declaration, including the occurrence whose adjacent instance name is
    // also `Bar`.
    for (needle, offset) in [("Bar Bar(", 0), ("Bar u_bar(", 0)] {
        let response = client
            .request(
                "textDocument/definition",
                json!({
                    "textDocument": { "uri": source_uri },
                    "position": position_at(&source_text, needle, offset)
                }),
            )
            .unwrap_or_else(|error| panic!("definition request at {needle}: {error}"));
        let (uri, start) = single_location(&response, "definition at module type");
        assert_eq!(uri, source_uri, "{needle} must resolve within foo.v");
        assert_eq!(
            start, expected_module,
            "{needle} must resolve to module Bar"
        );
        assert_no_shadow_uris(&response);
    }

    // Control: navigation on the same-named instance identifier still uses
    // the established instance-to-module definition behavior.
    let response = client
        .request(
            "textDocument/definition",
            json!({
                "textDocument": { "uri": source_uri },
                "position": position_at(&source_text, "Bar Bar(", "Bar ".len())
            }),
        )
        .expect("definition request at same-named instance identifier");
    let (uri, start) = single_location(&response, "definition at instance name");
    assert_eq!(uri, source_uri);
    assert_eq!(start, expected_module);
    assert_no_shadow_uris(&response);
    client.shutdown();
}

/// Goto-definition at a named port-connection resolves each SIDE of the
/// connection to its own declaration: the `.clk` LABEL reaches the CHILD
/// module's PORT declaration while the connected signal (the ACTUAL) stays
/// on its OWN declaration in the instantiating (parent) scope — including
/// through a multi-line instantiation.
#[test]
fn lsp_stdio_goto_definition_port_label() {
    let fixture = FixtureTree::new();
    let root_a = fixture.root("root-a");
    let root_b = fixture.root("root-b");
    let top_path = root_a.join("bindings").join("labels").join("top.sv");
    let child_path = root_a.join("bindings").join("labels").join("child.sv");
    let top_text = fs::read_to_string(&top_path).expect("read label_top fixture");
    let child_text = fs::read_to_string(&child_path).expect("read label_child fixture");

    let mut client = LspProcess::spawn(&fixture.root);
    client
        .initialize(
            &[("root-a", &root_a), ("root-b", &root_b)],
            default_init_options(),
        )
        .expect("initialize port-label workspace");
    client.open(&top_path, &top_text).expect("open label_top");
    wait_for_diagnostics(&mut client, &file_uri(&top_path), has_no_severity_1);

    // Request at the `.clk` label column (the identifier after the dot).
    // Expected: the child port declaration line in child.sv.
    let expected_port = position_at(&child_text, "input logic clk", 12);
    let response = client
        .request(
            "textDocument/definition",
            json!({
                "textDocument": { "uri": file_uri(&top_path) },
                "position": position_at(&top_text, ".clk", 1)
            }),
        )
        .expect("definition request at .clk label");
    let (uri, start) = single_location(&response, "definition at .clk label");
    assert_eq!(
        uri,
        file_uri(&child_path),
        "label must resolve into the CHILD module file"
    );
    assert_eq!(start, expected_port, "start: {start} vs {expected_port}");
    assert_no_shadow_uris(&response);

    // Request at the connected signal (`wa` inside `.clk(wa)`): the ACTUAL
    // resolves to its OWN declaration in the instantiating (parent) scope —
    // `logic wa` in this very file — never into the child module.
    let parent_wa = position_at(&top_text, "logic wa", 6);
    let response = client
        .request(
            "textDocument/definition",
            json!({
                "textDocument": { "uri": file_uri(&top_path) },
                "position": position_at(&top_text, "(wa)", 1)
            }),
        )
        .expect("definition request at connection actual");
    let (uri, start) = single_location(&response, "definition at connection actual");
    assert_eq!(
        uri,
        file_uri(&top_path),
        "the connected signal must resolve to its PARENT-scope declaration"
    );
    assert_eq!(start, parent_wa, "start: {start} vs {parent_wa}");
    assert_no_shadow_uris(&response);

    // Multi-line variant via the existing root-b ports fixture:
    // `.clk\n      (clk)` under `port_child u_child (`.
    let ml_top_path = root_b.join("ports").join("top.sv");
    let ml_child_path = root_b.join("ports").join("child.sv");
    let ml_top_text = fs::read_to_string(&ml_top_path).expect("read multiline top fixture");
    let ml_child_text = fs::read_to_string(&ml_child_path).expect("read multiline child fixture");
    let ml_expected_port = position_at(&ml_child_text, "input logic clk", 12);
    let response = client
        .request(
            "textDocument/definition",
            json!({
                "textDocument": { "uri": file_uri(&ml_top_path) },
                "position": position_at(&ml_top_text, ".clk", 1)
            }),
        )
        .expect("definition request at multi-line .clk label");
    let (uri, start) = single_location(&response, "definition at multi-line .clk label");
    assert_eq!(uri, file_uri(&ml_child_path));
    assert_eq!(
        start, ml_expected_port,
        "start: {start} vs {ml_expected_port}"
    );
    assert_no_shadow_uris(&response);

    // Multi-line ACTUAL (`clk` inside the continuation-line `(clk)`): the
    // ACTUAL stays on port_top's OWN `logic clk` declaration — same file,
    // never the child module.
    let ml_parent_clk = position_at(&ml_top_text, "logic clk", 6);
    let response = client
        .request(
            "textDocument/definition",
            json!({
                "textDocument": { "uri": file_uri(&ml_top_path) },
                "position": position_at(&ml_top_text, "(clk)", 1)
            }),
        )
        .expect("definition request at multi-line connection actual");
    let (uri, start) = single_location(&response, "definition at multi-line connection actual");
    assert_eq!(
        uri,
        file_uri(&ml_top_path),
        "multi-line actual must resolve to its PARENT-scope declaration"
    );
    assert_eq!(start, ml_parent_clk, "start: {start} vs {ml_parent_clk}");
    assert_no_shadow_uris(&response);
    client.shutdown();
}

/// Connection navigation survives a SYNTAX-BROKEN sibling: with `broken.v`
/// keeping the whole root at outcome=parse (Surelog skips UHDM entirely),
/// the parse-tree fallback still binds both sides of a named port connection
/// — the `.clk` LABEL to the child module's port declaration and the
/// ACTUAL to its own declaration in the instantiating scope — visible in
/// `llg/dumpTokens` `bind=` rows (`via=label` / `via=connection`) and
/// served by `textDocument/definition`.
#[test]
fn lsp_stdio_parse_fallback_binds_port_connections_to_child_ports() {
    let base = std::env::temp_dir().join(format!("llg-lsp-stdio-fb-{}", std::process::id()));
    let ws = base.join("fallback-ws");
    fs::create_dir_all(&ws).expect("create fallback workspace");
    let child_path = ws.join("fb_child.sv");
    let tb_path = ws.join("tb_fb.sv");
    let broken_path = ws.join("broken.v");
    fs::write(
        &child_path,
        "module fb_child(input logic clk, output logic q);\n  assign q = clk;\nendmodule\n",
    )
    .expect("write fb_child");
    let tb_text = "module tb_fb;\n  logic wa;\n  logic t_q;\n\n  fb_child u_fb(.clk(wa), .q(t_q));\nendmodule\n";
    fs::write(&tb_path, tb_text).expect("write tb_fb");
    // Unterminated module on purpose: Severity::Syntax ⇒ no UHDM.
    fs::write(&broken_path, "module broken(\n   input clk\n").expect("write broken.v");
    fs::write(
        ws.join(CONFIG_FILE),
        "schema_version = 1\n\n[sources]\ndirectories = [\".\"]\ninclude = [\"**/*.v\", \"**/*.sv\"]\n",
    )
    .expect("write llg.toml");

    struct Cleanup(PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(base.clone());

    let mut client = LspProcess::spawn(&base);
    client
        .initialize(&[("fallback-ws", &ws)], default_init_options())
        .expect("initialize fallback workspace");
    client.open(&tb_path, tb_text).expect("open tb_fb");

    // Poll the dump until the parse-fallback analysis carries the connection
    // bindings (the ~300 ms debounce plus the compile delay mean the first
    // answer can predate them).
    let tb_uri = file_uri(&tb_path);
    let deadline = Instant::now() + POLL_TIMEOUT;
    let mut interval = POLL_INTERVAL;
    let lines: Vec<String> = loop {
        match client.request_with_timeout(
            "llg/dumpTokens",
            json!({ "uri": tb_uri }),
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
                let want_clk = "bind=fb_child.sv:0:28[clk,port]";
                let want_q = "bind=fb_child.sv:0:46[q,port]";
                let ready = lines.iter().any(|l| l.contains(want_clk))
                    && lines.iter().any(|l| l.contains(want_q));
                if ready {
                    break lines;
                }
            }
            Err(error) if error.starts_with("timed out") => {}
            Err(error) => panic!("llg/dumpTokens failed: {error}"),
        }
        assert!(
            Instant::now() < deadline,
            "parse-fallback connection bindings never appeared in the dump"
        );
        thread::sleep(interval);
        interval = interval.saturating_mul(2).min(Duration::from_secs(1));
    };

    // Dump oracle: the label row points INTO the child module, the actual
    // rows stay on tb_fb's OWN declarations — each with its provenance tag.
    let row = |prefix: &str| -> String {
        lines
            .iter()
            .find(|l| l.starts_with(prefix))
            .unwrap_or_else(|| panic!("no dump row at {prefix}: {lines:?}"))
            .clone()
    };
    let label_row = row("tb_fb.sv:4:17");
    let actual_row = row("tb_fb.sv:4:21");
    assert!(
        label_row.contains("REF")
            && label_row.contains("via=label")
            && label_row.contains("bind=fb_child.sv:0:28[clk,port]"),
        "label row must bind to the child port: {label_row}"
    );
    assert!(
        actual_row.contains("REF")
            && actual_row.contains("via=connection")
            && actual_row.contains("bind=tb_fb.sv:1:8[wa,var]"),
        "actual row must bind to its parent-scope declaration: {actual_row}"
    );
    let q_actual_row = row("tb_fb.sv:4:29");
    assert!(
        q_actual_row.contains("via=connection")
            && q_actual_row.contains("bind=tb_fb.sv:2:8[t_q,var]"),
        "t_q row must bind to its parent-scope declaration: {q_actual_row}"
    );

    // Wire oracle: goto-definition at the label lands on the child port,
    // at the actual on the same-file parent-scope declaration.
    let expected_clk = position_at(
        &fs::read_to_string(&child_path).expect("reread fb_child"),
        "input logic clk",
        12,
    );
    let expected_wa = position_at(tb_text, "logic wa", 6);
    for (what, position, want_uri, want_start) in [
        (
            "label",
            position_at(tb_text, ".clk", 1),
            file_uri(&child_path),
            expected_clk.clone(),
        ),
        (
            "actual",
            position_at(tb_text, "(wa)", 1),
            file_uri(&tb_path),
            expected_wa,
        ),
    ] {
        let response = client
            .request(
                "textDocument/definition",
                json!({
                    "textDocument": { "uri": tb_uri },
                    "position": position
                }),
            )
            .unwrap_or_else(|error| panic!("definition request at {what}: {error}"));
        let (uri, start) = single_location(&response, &format!("parse-mode definition at {what}"));
        assert_eq!(uri, want_uri, "{what}: wrong target file");
        assert_eq!(start, want_start, "{what}: {start} vs {want_start}");
        assert_no_shadow_uris(&response);
    }

    // The broken sibling keeps the root at outcome=parse: the trailing dump
    // summary proves this test really exercised the fallback path.
    let summary = lines.last().cloned().unwrap_or_default();
    assert!(
        summary.contains("outcome=parse"),
        "expected the parse-fallback outcome in the summary: {summary}"
    );
    client.shutdown();
}

/// Goto-definition at a named PARAMETER override resolves each SIDE of the
/// override to its own declaration: the `.W` LABEL reaches the CHILD module's
/// PARAMETER declaration while the override RHS (the `W` inside `.D(W)`)
/// stays on its OWN declaration in the instantiating (parent) scope — even
/// though that scope holds a same-named `localparam W` decoy that name-based
/// resolution would wrongly pick for the label.  Single- and multi-line
/// instantiations.
#[test]
fn lsp_stdio_goto_definition_param_label() {
    let fixture = FixtureTree::new();
    let root_a = fixture.root("root-a");
    let root_b = fixture.root("root-b");
    let top_path = root_a.join("bindings").join("params").join("top.sv");
    let child_path = root_a.join("bindings").join("params").join("child.sv");
    let top_text = fs::read_to_string(&top_path).expect("read param_top fixture");
    let child_text = fs::read_to_string(&child_path).expect("read param_child fixture");

    let mut client = LspProcess::spawn(&fixture.root);
    client
        .initialize(
            &[("root-a", &root_a), ("root-b", &root_b)],
            default_init_options(),
        )
        .expect("initialize param-label workspace");
    client.open(&top_path, &top_text).expect("open param top");
    wait_for_diagnostics(&mut client, &file_uri(&top_path), has_no_severity_1);

    // Request at the `.W` label identifier.  Expected: the child parameter
    // declaration line in child.sv — never the same-named localparam decoy.
    let expected_w = position_at(&child_text, "parameter int W", 14);
    let response = client
        .request(
            "textDocument/definition",
            json!({
                "textDocument": { "uri": file_uri(&top_path) },
                "position": position_at(&top_text, ".W", 1)
            }),
        )
        .expect("definition request at .W label");
    let (uri, start) = single_location(&response, "definition at .W label");
    assert_eq!(
        uri,
        file_uri(&child_path),
        "override label must resolve into the CHILD module file"
    );
    assert_eq!(start, expected_w, "start: {start} vs {expected_w}");
    assert_no_shadow_uris(&response);

    // The `.D` label likewise reaches the child's D parameter.
    let expected_d = position_at(&child_text, "parameter int D", 14);
    let response = client
        .request(
            "textDocument/definition",
            json!({
                "textDocument": { "uri": file_uri(&top_path) },
                "position": position_at(&top_text, ".D(", 1)
            }),
        )
        .expect("definition request at .D label");
    let (uri, start) = single_location(&response, "definition at .D label");
    assert_eq!(uri, file_uri(&child_path));
    assert_eq!(start, expected_d, "start: {start} vs {expected_d}");
    assert_no_shadow_uris(&response);

    // The override RHS `W` inside `.D(W)` resolves to the DECOY localparam
    // in the instantiating scope — the opposite direction of the label.
    let expected_decoy = position_at(&top_text, "localparam int W", 15);
    let response = client
        .request(
            "textDocument/definition",
            json!({
                "textDocument": { "uri": file_uri(&top_path) },
                "position": position_at(&top_text, ".D(W)", 3)
            }),
        )
        .expect("definition request at override RHS");
    let (uri, start) = single_location(&response, "definition at override RHS");
    assert_eq!(
        uri,
        file_uri(&top_path),
        "the override RHS must resolve to its PARENT-scope declaration"
    );
    assert_eq!(start, expected_decoy, "start: {start} vs {expected_decoy}");
    assert_no_shadow_uris(&response);

    // Multi-line variant via root-b: the `.W` label sits on a continuation
    // line below `ml_pchild #(` and still reaches the child parameter.
    let ml_top_path = root_b.join("params").join("top.sv");
    let ml_child_path = root_b.join("params").join("child.sv");
    let ml_top_text = fs::read_to_string(&ml_top_path).expect("read multiline param top fixture");
    let ml_child_text =
        fs::read_to_string(&ml_child_path).expect("read multiline param child fixture");
    client
        .open(&ml_top_path, &ml_top_text)
        .expect("open multiline param top");
    wait_for_diagnostics(&mut client, &file_uri(&ml_top_path), has_no_severity_1);

    let ml_expected_w = position_at(&ml_child_text, "parameter int W", 14);
    let response = client
        .request(
            "textDocument/definition",
            json!({
                "textDocument": { "uri": file_uri(&ml_top_path) },
                "position": position_at(&ml_top_text, ".W", 1)
            }),
        )
        .expect("definition request at multi-line .W label");
    let (uri, start) = single_location(&response, "definition at multi-line .W label");
    assert_eq!(uri, file_uri(&ml_child_path));
    assert_eq!(start, ml_expected_w, "start: {start} vs {ml_expected_w}");
    assert_no_shadow_uris(&response);

    // Multi-line RHS `W` stays on the instantiating scope's localparam.
    let ml_expected_decoy = position_at(&ml_top_text, "localparam int W", 15);
    let response = client
        .request(
            "textDocument/definition",
            json!({
                "textDocument": { "uri": file_uri(&ml_top_path) },
                "position": position_at(&ml_top_text, "(W)", 1)
            }),
        )
        .expect("definition request at multi-line override RHS");
    let (uri, start) = single_location(&response, "definition at multi-line override RHS");
    assert_eq!(
        uri,
        file_uri(&ml_top_path),
        "multi-line override RHS must resolve to its PARENT-scope declaration"
    );
    assert_eq!(
        start, ml_expected_decoy,
        "start: {start} vs {ml_expected_decoy}"
    );
    assert_no_shadow_uris(&response);
    client.shutdown();
}

/// Parameter-override navigation survives a SYNTAX-BROKEN sibling: with
/// `broken.v` keeping the whole root at outcome=parse (Surelog skips UHDM
/// entirely), the parse-tree fallback still binds both sides of a named
/// parameter override — the `.PW` LABEL to the child module's parameter
/// declaration and the RHS reference to its own declaration in the
/// instantiating scope — visible in `llg/dumpTokens` `bind=` rows
/// (`via=label` / `via=connection`) and served by
/// `textDocument/definition`.
#[test]
fn lsp_stdio_parse_fallback_binds_param_overrides_to_child_params() {
    let base = std::env::temp_dir().join(format!("llg-lsp-stdio-pfb-{}", std::process::id()));
    let ws = base.join("fallback-ws");
    fs::create_dir_all(&ws).expect("create fallback workspace");
    let child_path = ws.join("fb_pchild.sv");
    let tb_path = ws.join("tb_fb2.sv");
    let broken_path = ws.join("broken.v");
    let child_text = concat!(
        "module fb_pchild #(\n",
        "  parameter int PW = 8,\n",
        "  parameter int PD = 3\n",
        ") (\n",
        "  input logic clk,\n",
        "  output logic [7:0] q\n",
        ");\n",
        "  assign q = '0;\n",
        "endmodule\n",
    );
    fs::write(&child_path, child_text).expect("write fb_pchild");
    let tb_text = concat!(
        "module tb_fb2;\n",
        "  localparam int PW = 1;\n",
        "  logic wa;\n",
        "  logic [7:0] t_q;\n",
        "\n",
        "  fb_pchild #(\n",
        "    .PW(4),\n",
        "    .PD(wa)\n",
        "  ) u_fb (\n",
        "    .clk(wa),\n",
        "    .q(t_q)\n",
        "  );\n",
        "endmodule\n",
    );
    fs::write(&tb_path, tb_text).expect("write tb_fb2");
    // Unterminated module on purpose: Severity::Syntax ⇒ no UHDM.
    fs::write(&broken_path, "module broken(\n   input clk\n").expect("write broken.v");
    fs::write(
        ws.join(CONFIG_FILE),
        "schema_version = 1\n\n[sources]\ndirectories = [\".\"]\ninclude = [\"**/*.v\", \"**/*.sv\"]\n",
    )
    .expect("write llg.toml");

    struct Cleanup(PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(base.clone());

    let mut client = LspProcess::spawn(&base);
    client
        .initialize(&[("fallback-ws", &ws)], default_init_options())
        .expect("initialize param fallback workspace");
    client.open(&tb_path, tb_text).expect("open tb_fb2");

    // Expected bind targets, computed from the fixture texts.
    let pw_decl = position_at(child_text, "parameter int PW", 14);
    let pd_decl = position_at(child_text, "parameter int PD", 14);
    let wa_decl = position_at(tb_text, "logic wa", 6);
    let bind_field = |target: &Value, name: &str, kind: &str| {
        format!(
            "bind=fb_pchild.sv:{}:{}[{name},{kind}]",
            target["line"].as_u64().unwrap_or(u64::MAX),
            target["character"].as_u64().unwrap_or(u64::MAX)
        )
    };
    let want_pw = bind_field(&pw_decl, "PW", "parameter");
    let want_pd = bind_field(&pd_decl, "PD", "parameter");
    let rhs_pos = position_at(tb_text, "(wa)", 1);

    // Poll the dump until the parse-fallback analysis carries the override
    // bindings (debounce plus compile delay mean the first answer can
    // predate them).
    let tb_uri = file_uri(&tb_path);
    let deadline = Instant::now() + POLL_TIMEOUT;
    let mut interval = POLL_INTERVAL;
    let lines: Vec<String> = loop {
        match client.request_with_timeout(
            "llg/dumpTokens",
            json!({ "uri": tb_uri }),
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
                let ready = lines.iter().any(|l| l.contains(&want_pw))
                    && lines.iter().any(|l| l.contains(&want_pd));
                if ready {
                    break lines;
                }
            }
            Err(error) if error.starts_with("timed out") => {}
            Err(error) => panic!("llg/dumpTokens failed: {error}"),
        }
        assert!(
            Instant::now() < deadline,
            "parse-fallback override bindings never appeared in the dump"
        );
        thread::sleep(interval);
        interval = interval.saturating_mul(2).min(Duration::from_secs(1));
    };

    // Dump oracle: the label rows point INTO the child module (parameters,
    // via=label), the RHS row stays on tb_fb2's OWN declaration.
    let row = |prefix: &str| -> String {
        lines
            .iter()
            .find(|l| l.starts_with(prefix))
            .unwrap_or_else(|| panic!("no dump row at {prefix}: {lines:?}"))
            .clone()
    };
    let pw_label_row = row("tb_fb2.sv:6:5");
    assert!(
        pw_label_row.contains("REF")
            && pw_label_row.contains("via=label")
            && pw_label_row.contains(&want_pw),
        ".PW label row must bind to the child parameter: {pw_label_row}"
    );
    let pd_label_row = row("tb_fb2.sv:7:5");
    assert!(
        pd_label_row.contains("via=label") && pd_label_row.contains(&want_pd),
        ".PD label row must bind to the child parameter: {pd_label_row}"
    );
    let rhs_row = row(&format!(
        "tb_fb2.sv:{}:{}",
        rhs_pos["line"].as_u64().unwrap(),
        rhs_pos["character"].as_u64().unwrap()
    ));
    assert!(
        rhs_row.contains("via=connection")
            && rhs_row.contains(&format!(
                "bind=tb_fb2.sv:{}:{}[wa,var]",
                wa_decl["line"].as_u64().unwrap(),
                wa_decl["character"].as_u64().unwrap()
            )),
        "RHS row must bind to its parent-scope declaration: {rhs_row}"
    );

    // Wire oracle: goto-definition at the label lands on the child
    // parameter, at the RHS on the same-file parent-scope declaration.
    let expected_pw_start = json!({
        "line": pw_decl["line"],
        "character": pw_decl["character"]
    });
    for (what, position, want_uri, want_start) in [
        (
            ".PW label",
            position_at(tb_text, ".PW", 1),
            file_uri(&child_path),
            expected_pw_start.clone(),
        ),
        (
            "RHS wa",
            position_at(tb_text, "(wa)", 1),
            file_uri(&tb_path),
            wa_decl.clone(),
        ),
    ] {
        let response = client
            .request(
                "textDocument/definition",
                json!({
                    "textDocument": { "uri": tb_uri },
                    "position": position
                }),
            )
            .unwrap_or_else(|error| panic!("definition request at {what}: {error}"));
        let (uri, start) = single_location(&response, &format!("parse-mode definition at {what}"));
        assert_eq!(uri, want_uri, "{what}: wrong target file");
        assert_eq!(start, want_start, "{what}: {start} vs {want_start}");
        assert_no_shadow_uris(&response);
    }

    // The broken sibling keeps the root at outcome=parse: the trailing dump
    // summary proves this test really exercised the fallback path.
    let summary = lines.last().cloned().unwrap_or_default();
    assert!(
        summary.contains("outcome=parse"),
        "expected the parse-fallback outcome in the summary: {summary}"
    );
    client.shutdown();
}

/// The custom `llg/dumpTokens` request serves the CLI dump rows for one open
/// document (workspace-relative positions, `bind=` fields) plus the trailing
/// `# analysis:` summary line.
#[test]
fn lsp_stdio_serves_custom_dump_tokens_request() {
    let fixture = FixtureTree::new();
    let root_a = fixture.root("root-a");
    let target_path = root_a.join("bindings").join("nets").join("m_a.sv");
    let target_uri = file_uri(&target_path);
    let target_text = fs::read_to_string(&target_path).expect("read m_a fixture");

    let mut client = LspProcess::spawn(&fixture.root);
    client
        .initialize(&[("root-a", &root_a)], default_init_options())
        .expect("initialize dump-tokens workspace");
    client.open(&target_path, &target_text).expect("open m_a");
    wait_for_diagnostics(&mut client, &target_uri, has_no_severity_1);

    let result = client
        .request("llg/dumpTokens", json!({ "uri": target_uri }))
        .expect("llg/dumpTokens request");
    let lines: Vec<String> = result
        .get("lines")
        .and_then(Value::as_array)
        .expect("dumpTokens result.lines array")
        .iter()
        .map(|line| line.as_str().expect("dumpTokens line string").to_owned())
        .collect();
    assert!(!lines.is_empty(), "dumpTokens returned no lines: {result}");
    assert!(
        lines
            .iter()
            .any(|line| line.starts_with("bindings/nets/m_a.sv:")),
        "dumpTokens rows must use workspace-relative file positions (no shadow paths): {lines:?}"
    );
    assert!(
        lines.iter().any(|line| line.contains("bind=")),
        "dumpTokens rows must carry the bind= field: {lines:?}"
    );
    assert!(
        lines
            .last()
            .is_some_and(|line| line.starts_with("# analysis:")),
        "dumpTokens response must end with the # analysis summary line: {lines:?}"
    );

    // A document owned by NO workspace root answers with an error line
    // instead of failing the request.  The path never has to exist; it only
    // must stay outside every root, and the pid suffix keeps concurrent test
    // processes from sharing one name.
    let outside = std::env::temp_dir().join(format!(
        "llg-dump-tokens-outside-root-{}.sv",
        std::process::id()
    ));
    let missing = client
        .request("llg/dumpTokens", json!({ "uri": file_uri(&outside) }))
        .expect("llg/dumpTokens request for unowned document");
    let missing_lines = missing.get("lines").and_then(Value::as_array);
    assert!(
        missing_lines.is_some_and(|lines| lines.len() == 1
            && lines[0]
                .as_str()
                .unwrap_or_default()
                .starts_with("# error:")),
        "unknown document must yield a single # error line: {missing}"
    );
    client.shutdown();
}

/// The module explorer is an end-to-end read of the committed analysis.  The
/// configured top deliberately leaves `unrelated` out of UHDM while the
/// source graph still sees every definition and source edge.  This catches
/// both the top-child-leaf root regression and the declaration-only contents
/// fallback over the actual JSON-RPC boundary.
#[test]
fn lsp_stdio_module_explorer_uses_source_graph_and_declaration_fallback() {
    let fixture = FixtureTree::module_explorer();
    let ws = fixture.root("workspace");

    let mut client = LspProcess::spawn(&ws);
    client
        .initialize(&[("module-explorer", &ws)], default_init_options())
        .expect("initialize module-explorer workspace");
    let snapshot = client
        .request("llg/moduleExplorer", json!({}))
        .expect("module explorer request");
    assert_no_shadow_uris(&snapshot);

    let roots = snapshot
        .get("roots")
        .and_then(Value::as_array)
        .expect("module explorer roots array");
    let root_names = roots
        .iter()
        .filter_map(|root| root.get("moduleType").and_then(Value::as_str))
        .collect::<Vec<_>>();
    assert!(
        root_names.contains(&"top"),
        "configured top must be a root: {snapshot}"
    );
    assert!(
        root_names.contains(&"unrelated"),
        "uninstantiated source definition must remain a root: {snapshot}"
    );
    assert!(
        !root_names.contains(&"child"),
        "instantiated child must not be a root: {snapshot}"
    );
    assert!(
        !root_names.contains(&"leaf"),
        "transitive leaf must not be a root: {snapshot}"
    );

    let top_root = roots
        .iter()
        .find(|root| root.get("moduleType").and_then(Value::as_str) == Some("top"))
        .expect("top root occurrence");
    let child = top_root
        .get("children")
        .and_then(Value::as_array)
        .and_then(|children| {
            children
                .iter()
                .find(|child| child.get("instanceName").and_then(Value::as_str) == Some("u_child"))
        })
        .expect("top.u_child nested occurrence");
    assert_eq!(
        child.get("moduleType").and_then(Value::as_str),
        Some("child")
    );
    assert_eq!(
        child.get("contentSource").and_then(Value::as_str),
        Some("elaborated")
    );
    assert_eq!(
        child
            .get("children")
            .and_then(Value::as_array)
            .and_then(|children| children.first())
            .and_then(|leaf| leaf.get("moduleType"))
            .and_then(Value::as_str),
        Some("leaf")
    );

    let child_port = child
        .get("ports")
        .and_then(Value::as_array)
        .and_then(|ports| {
            ports
                .iter()
                .find(|port| port.get("name").and_then(Value::as_str) == Some("clk"))
        })
        .expect("elaborated child port");
    assert!(
        child_port
            .get("location")
            .and_then(|location| location.get("uri"))
            .and_then(Value::as_str)
            .is_some_and(|uri| uri.ends_with("/workspace/child.sv")),
        "child port location must point at its declaration: {child_port}"
    );
    assert_eq!(
        child_port
            .get("location")
            .and_then(|location| location.get("range")),
        Some(&json!({
            "startLine": 1,
            "startCharacter": 52,
            "endLine": 1,
            "endCharacter": 55
        })),
        "child port location must cover the clk identifier: {child_port}"
    );
    let child_param = child
        .get("params")
        .and_then(Value::as_array)
        .and_then(|params| {
            params
                .iter()
                .find(|param| param.get("name").and_then(Value::as_str) == Some("WIDTH"))
        })
        .expect("elaborated child parameter");
    assert_eq!(
        child_param.get("value").and_then(Value::as_str),
        Some("32'sd8")
    );
    assert!(
        child_param
            .get("location")
            .and_then(|location| location.get("uri"))
            .and_then(Value::as_str)
            .is_some_and(|uri| uri.ends_with("/workspace/child.sv")),
        "child parameter location must point at its declaration: {child_param}"
    );
    assert_eq!(
        child_param
            .get("location")
            .and_then(|location| location.get("range")),
        Some(&json!({
            "startLine": 1,
            "startCharacter": 29,
            "endLine": 1,
            "endCharacter": 34
        })),
        "child parameter location must cover the WIDTH identifier: {child_param}"
    );
    let child_signal = child
        .get("signals")
        .and_then(Value::as_array)
        .and_then(|signals| {
            signals
                .iter()
                .find(|signal| signal.get("name").and_then(Value::as_str) == Some("payload"))
        })
        .expect("elaborated child signal");
    assert_eq!(
        child_signal
            .get("type")
            .and_then(|ty| ty.get("displayType"))
            .and_then(Value::as_str),
        Some("logic [7:0]"),
        "child signal width must use the exact elaborated WIDTH value: {child_signal}"
    );
    assert_eq!(
        child_signal
            .get("type")
            .and_then(|ty| ty.get("width"))
            .and_then(Value::as_u64),
        Some(8)
    );
    assert!(
        child_signal
            .get("location")
            .and_then(|location| location.get("uri"))
            .and_then(Value::as_str)
            .is_some_and(|uri| uri.ends_with("/workspace/child.sv")),
        "child signal location must point at its declaration: {child_signal}"
    );
    assert_eq!(
        child_signal
            .get("location")
            .and_then(|location| location.get("range")),
        Some(&json!({
            "startLine": 4,
            "startCharacter": 4,
            "endLine": 4,
            "endCharacter": 11
        })),
        "child signal location must cover the payload identifier: {child_signal}"
    );
    let child_memory = child
        .get("signals")
        .and_then(Value::as_array)
        .and_then(|signals| {
            signals
                .iter()
                .find(|signal| signal.get("name").and_then(Value::as_str) == Some("memory"))
        })
        .expect("elaborated packed-plus-unpacked child signal");
    assert_eq!(
        child_memory
            .get("type")
            .and_then(|ty| ty.get("displayType"))
            .and_then(Value::as_str),
        Some("logic [7:0] [0:1]"),
        "packed width must come from elaboration while the unpacked range stays source-backed: {child_memory}"
    );
    assert_eq!(
        child_memory
            .get("type")
            .and_then(|ty| ty.get("width"))
            .and_then(Value::as_u64),
        Some(8)
    );
    assert_eq!(
        child_memory.get("kind").and_then(Value::as_str),
        Some("array"),
        "the concrete elaborated array kind must remain intact: {child_memory}"
    );
    let leaf = child
        .get("children")
        .and_then(Value::as_array)
        .and_then(|children| children.first())
        .expect("nested leaf occurrence");
    let leaf_signal = leaf
        .get("signals")
        .and_then(Value::as_array)
        .and_then(|signals| signals.first())
        .expect("nested leaf signal");
    assert_eq!(
        leaf_signal
            .get("type")
            .and_then(|ty| ty.get("displayType"))
            .and_then(Value::as_str),
        Some("logic [5:0]"),
        "nested signal width must use the nested instance parameter: {leaf_signal}"
    );
    assert!(
        leaf_signal
            .get("location")
            .and_then(|location| location.get("uri"))
            .and_then(Value::as_str)
            .is_some_and(|uri| uri.ends_with("/workspace/leaf.sv")),
        "nested signal location must point at its declaration: {leaf_signal}"
    );

    let top_signal = top_root
        .get("signals")
        .and_then(Value::as_array)
        .and_then(|signals| signals.first())
        .expect("top internal signal");
    assert!(
        top_signal
            .get("location")
            .and_then(|location| location.get("uri"))
            .and_then(Value::as_str)
            .is_some_and(|uri| uri.ends_with("/workspace/top.sv")),
        "top signal location must point at its declaration: {top_signal}"
    );

    let unrelated = roots
        .iter()
        .find(|root| root.get("moduleType").and_then(Value::as_str) == Some("unrelated"))
        .expect("unrelated declaration-only root");
    assert_eq!(
        unrelated.get("contentSource").and_then(Value::as_str),
        Some("declaration")
    );
    let unrelated_port = unrelated
        .get("ports")
        .and_then(Value::as_array)
        .and_then(|ports| ports.first())
        .expect("unrelated formal port");
    assert_eq!(
        unrelated_port.get("detail").and_then(Value::as_str),
        Some("input logic pin"),
        "inline declaration details must be scoped to the selected identifier: {unrelated}"
    );
    assert_eq!(
        unrelated
            .get("signals")
            .and_then(Value::as_array)
            .map(|signals| signals
                .iter()
                .filter_map(|signal| signal.get("name").and_then(Value::as_str))
                .collect::<Vec<_>>()),
        Some(vec!["internal_bus"])
    );
    let unrelated_signal = unrelated
        .get("signals")
        .and_then(Value::as_array)
        .and_then(|signals| signals.first())
        .expect("unrelated internal signal");
    assert_eq!(
        unrelated_signal
            .get("type")
            .and_then(|ty| ty.get("kind"))
            .and_then(Value::as_str),
        Some("logic"),
        "declaration fallback must retain the source element type: {unrelated}"
    );
    assert_eq!(
        unrelated_signal
            .get("type")
            .and_then(|ty| ty.get("width"))
            .and_then(Value::as_u64),
        Some(8),
        "declaration fallback must retain literal packed width: {unrelated}"
    );
    assert_eq!(
        unrelated_signal
            .get("type")
            .and_then(|ty| ty.get("displayType"))
            .and_then(Value::as_str),
        Some("logic [7:0]"),
        "declaration fallback must retain a sanitized display type: {unrelated}"
    );
    assert!(
        unrelated_signal
            .get("location")
            .and_then(|location| location.get("uri"))
            .and_then(Value::as_str)
            .is_some_and(|uri| uri.ends_with("/workspace/unrelated.sv")),
        "declaration fallback signal location must point at its declaration: {unrelated}"
    );
    assert!(
        unrelated_port
            .get("location")
            .and_then(|location| location.get("uri"))
            .and_then(Value::as_str)
            .is_some_and(|uri| uri.ends_with("/workspace/unrelated.sv")),
        "declaration fallback port location must point at its declaration: {unrelated}"
    );
    assert!(
        unrelated
            .get("signals")
            .and_then(Value::as_array)
            .is_some_and(|signals| {
                signals.iter().all(|signal| {
                    !matches!(
                        signal.get("name").and_then(Value::as_str),
                        Some("function_local") | Some("task_local")
                    )
                })
            }),
        "function/task locals must not leak into module signals: {unrelated}"
    );
    assert!(
        unrelated
            .get("signals")
            .and_then(Value::as_array)
            .is_some_and(|signals| {
                signals
                    .iter()
                    .all(|signal| signal.get("name").and_then(Value::as_str) != Some("pin"))
            }),
        "formal ports must not be repeated as signals: {unrelated}"
    );
    assert!(
        snapshot
            .get("modules")
            .and_then(Value::as_array)
            .is_some_and(|modules| modules.iter().any(|module| {
                module.get("name").and_then(Value::as_str) == Some("unrelated")
                    && module.get("contentSource").and_then(Value::as_str) == Some("declaration")
            })),
        "module definition fallback must be present: {snapshot}"
    );

    client.shutdown();
}

// ── Built-in lint rules: type / careless-mistake checks ─────────────────────
//
// End-to-end acceptance for the rules added alongside `unused-signal`:
// `implicit-net`, `case-default-missing` and `comparison-width-mismatch`.
// The fixture root is the brand-new `lint-rules/` directory (not one of the
// pinned roots A/B/C); its single source fires exactly those three rules and
// no other.

#[test]
fn lsp_stdio_publishes_new_lint_rules_and_honors_config() {
    let fixture = FixtureTree::new();
    let root = fixture.root("lint-rules");
    let path = root.join("src").join("careless.sv");
    let uri = file_uri(&path);

    let mut client = LspProcess::spawn(&fixture.root);
    client
        .initialize(&[("lint-rules", &root)], default_init_options())
        .expect("initialize lint-rules workspace");
    client
        .open(
            &path,
            &fs::read_to_string(&path).expect("read careless fixture"),
        )
        .expect("open careless fixture");

    // All three new rules publish as llg-lint diagnostics with their default
    // warning severity (LSP severity 2).
    let diagnostics = wait_for_diagnostics(&mut client, &uri, |params| {
        has_lint_rule(params, "implicit-net")
            && has_lint_rule(params, "case-default-missing")
            && has_lint_rule(params, "comparison-width-mismatch")
    });
    assert_eq!(lint_severity(&diagnostics, "implicit-net"), Some(2));
    assert_eq!(lint_severity(&diagnostics, "case-default-missing"), Some(2));
    assert_eq!(
        lint_severity(&diagnostics, "comparison-width-mismatch"),
        Some(2)
    );
    assert_no_shadow_uris(&diagnostics);

    // Disabling one rule through the watched root `llg.toml` removes exactly
    // that rule's findings; the other two keep publishing.
    let config_path = root.join(CONFIG_FILE);
    fs::write(
        &config_path,
        "schema_version = 1\n\
         [sources]\n\
         directories = [\".\"]\n\
         include = [\"**/*.v\", \"**/*.sv\"]\n\
         [lint]\n\
         enabled = true\n\
         [lint.rules.case-default-missing]\n\
         enabled = false\n",
    )
    .expect("update lint-rules config");
    client
        .send_watch_event(&config_path, 2)
        .expect("send config watch event");
    let updated = wait_for_diagnostics(&mut client, &uri, |params| {
        !has_lint_rule(params, "case-default-missing")
            && has_lint_rule(params, "implicit-net")
            && has_lint_rule(params, "comparison-width-mismatch")
    });
    assert!(!has_lint_rule(&updated, "case-default-missing"));
    assert_eq!(lint_severity(&updated, "implicit-net"), Some(2));
    assert_eq!(
        lint_severity(&updated, "comparison-width-mismatch"),
        Some(2)
    );
    client.shutdown();
}

#[test]
fn lsp_stdio_publishes_expanded_careless_mistake_rules_and_honors_config() {
    let fixture = FixtureTree::new();
    let root = fixture.root("lint-rules");
    let path = root.join("src").join("careless_more.sv");
    let uri = file_uri(&path);
    let rule_ids = [
        "undriven-signal",
        "incomplete-sensitivity-list",
        "out-of-range-select",
        "xz-logical-equality",
        "duplicate-case-item",
    ];

    let mut client = LspProcess::spawn(&fixture.root);
    client
        .initialize(&[("lint-rules", &root)], default_init_options())
        .expect("initialize expanded lint-rules workspace");
    client
        .open(
            &path,
            &fs::read_to_string(&path).expect("read expanded careless fixture"),
        )
        .expect("open expanded careless fixture");

    let diagnostics = wait_for_diagnostics(&mut client, &uri, |params| {
        rule_ids.iter().all(|rule| has_lint_rule(params, rule))
    });
    for rule in rule_ids {
        assert_eq!(
            lint_severity(&diagnostics, rule),
            Some(2),
            "{rule} should publish as a warning: {diagnostics:?}"
        );
    }
    assert_no_shadow_uris(&diagnostics);

    let config_path = root.join(CONFIG_FILE);
    fs::write(
        &config_path,
        "schema_version = 1\n\
         [sources]\n\
         directories = [\".\"]\n\
         include = [\"**/*.v\", \"**/*.sv\"]\n\
         [lint]\n\
         enabled = true\n\
         [lint.rules.out-of-range-select]\n\
         enabled = false\n",
    )
    .expect("disable out-of-range-select");
    client
        .send_watch_event(&config_path, 2)
        .expect("send expanded lint config watch event");
    let updated = wait_for_diagnostics(&mut client, &uri, |params| {
        !has_lint_rule(params, "out-of-range-select")
            && rule_ids
                .iter()
                .filter(|rule| **rule != "out-of-range-select")
                .all(|rule| has_lint_rule(params, rule))
    });
    assert!(!has_lint_rule(&updated, "out-of-range-select"));
    for rule in rule_ids
        .iter()
        .filter(|rule| **rule != "out-of-range-select")
    {
        assert_eq!(lint_severity(&updated, rule), Some(2));
    }
    client.shutdown();
}

#[test]
fn lsp_stdio_publishes_control_lint_batch_and_honors_config() {
    let fixture = FixtureTree::new();
    let root = fixture.root("lint-rules");
    let path = root.join("src").join("careless_control.sv");
    let uri = file_uri(&path);
    let rule_ids = [
        "empty-implicit-sensitivity",
        "assignment-in-condition",
        "casex-statement",
    ];

    let mut client = LspProcess::spawn(&fixture.root);
    client
        .initialize(&[("lint-rules", &root)], default_init_options())
        .expect("initialize control lint workspace");
    client
        .open(
            &path,
            &fs::read_to_string(&path).expect("read control lint fixture"),
        )
        .expect("open control lint fixture");

    let diagnostics = wait_for_diagnostics(&mut client, &uri, |params| {
        rule_ids.iter().all(|rule| has_lint_rule(params, rule))
    });
    assert_eq!(
        diagnostics.get("uri").and_then(Value::as_str),
        Some(uri.as_str()),
        "diagnostics must retain the real workspace URI: {diagnostics:?}"
    );
    for rule in rule_ids {
        assert_eq!(
            lint_severity(&diagnostics, rule),
            Some(2),
            "{rule} should publish as a warning: {diagnostics:?}"
        );
    }
    assert_no_shadow_uris(&diagnostics);

    let config_path = root.join(CONFIG_FILE);
    fs::write(
        &config_path,
        "schema_version = 1\n\
         [sources]\n\
         directories = [\".\"]\n\
         include = [\"**/*.v\", \"**/*.sv\"]\n\
         [lint]\n\
         enabled = true\n\
         [lint.rules.empty-implicit-sensitivity]\n\
         enabled = false\n\
         [lint.rules.casex-statement]\n\
         severity = \"error\"\n",
    )
    .expect("update control lint config");
    client
        .send_watch_event(&config_path, 2)
        .expect("send control lint config watch event");
    let updated = wait_for_diagnostics(&mut client, &uri, |params| {
        !has_lint_rule(params, "empty-implicit-sensitivity")
            && has_lint_rule(params, "assignment-in-condition")
            && lint_severity(params, "casex-statement") == Some(1)
    });
    assert!(!has_lint_rule(&updated, "empty-implicit-sensitivity"));
    assert_eq!(lint_severity(&updated, "assignment-in-condition"), Some(2));
    assert_eq!(lint_severity(&updated, "casex-statement"), Some(1));
    assert_no_shadow_uris(&updated);
    client.shutdown();
}

// ── textDocument/prepareRename + textDocument/rename ────────────────────────
//
// Rename reuses the find-references machinery, so the acceptance surface is:
// the edit set equals the reference set (declaration included), each edit
// replaces ONLY the identifier span, and non-renamable positions (keywords,
// instance names) answer null instead of an edit.  These tests use
// self-contained generated workspaces so the shared fixture tree stays
// untouched.

/// Owns a generated rename-workspace base directory, removed on drop.
struct TempDirCleanup(PathBuf);

impl Drop for TempDirCleanup {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Create a single-root workspace under a fresh temp base with the given
/// source files; returns (cleanup guard, workspace dir).
fn rename_workspace(dir_name: &str, files: &[(&str, &str)]) -> (TempDirCleanup, PathBuf) {
    let base =
        std::env::temp_dir().join(format!("llg-lsp-rename-{}-{dir_name}", std::process::id()));
    let ws = base.join("ws");
    fs::create_dir_all(&ws).expect("create rename workspace");
    fs::write(
        ws.join(CONFIG_FILE),
        "schema_version = 1\n\
         [sources]\n\
         directories = [\".\"]\n\
         include = [\"**/*.v\", \"**/*.sv\"]\n\
         [lint]\n\
         enabled = false\n",
    )
    .expect("write rename workspace llg.toml");
    for (file, text) in files {
        fs::write(ws.join(file), text).expect("write rename workspace source");
    }
    (TempDirCleanup(base), ws)
}

/// The `newText`-annotated edit list one URI contributes to a WorkspaceEdit.
fn edits_for(result: &Value, uri: &str) -> Vec<(Value, Value, String)> {
    let edits = result
        .get("changes")
        .and_then(Value::as_object)
        .unwrap_or_else(|| panic!("workspace edit must carry changes: {result}"))
        .get(uri)
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("no edits for {uri}: {result}"));
    edits
        .iter()
        .map(|edit| {
            let range = edit.get("range").cloned().expect("edit range");
            let start = range.get("start").cloned().expect("range start");
            let end = range.get("end").cloned().expect("range end");
            let new_text = edit
                .get("newText")
                .and_then(Value::as_str)
                .expect("edit newText")
                .to_owned();
            (start, end, new_text)
        })
        .collect()
}

fn start_key(start: &Value) -> (u64, u64) {
    (
        start.get("line").and_then(Value::as_u64).expect("line"),
        start
            .get("character")
            .and_then(Value::as_u64)
            .expect("character"),
    )
}

/// Scenario (1): renaming a net referenced across two files produces edits in
/// BOTH uris — the child module's port declaration + its in-module use in one
/// file, and the `.clk` named-connection label in the instantiating file.
#[test]
fn lsp_stdio_rename_net_across_files() {
    let child_text = "module ren_child(input logic clk, output logic q);\n\
                      \x20 assign q = clk;\n\
                      endmodule\n";
    let top_text = "module ren_top;\n\
                    \x20 logic wa;\n\
                    \x20 logic t_q;\n\n\
                    \x20 ren_child u0(.clk(wa), .q(t_q));\n\n\
                    \x20 always #5 wa = ~wa;\n\
                    endmodule\n";
    let (_cleanup, ws) = rename_workspace(
        "cross-file",
        &[("ren_child.sv", child_text), ("ren_top.sv", top_text)],
    );
    let child_path = ws.join("ren_child.sv");
    let top_path = ws.join("ren_top.sv");
    let child_uri = file_uri(&child_path);
    let top_uri = file_uri(&top_path);

    let mut client = LspProcess::spawn(&ws);
    client
        .initialize(&[("rename-ws", &ws)], default_init_options())
        .expect("initialize rename workspace");
    client.open(&top_path, top_text).expect("open ren_top");
    wait_for_diagnostics(&mut client, &top_uri, has_no_severity_1);

    // prepareRename at the in-module use of `clk` (`assign q = clk;`).
    let clk_use = position_at(child_text, "= clk", 2);
    let prepared = client
        .request(
            "textDocument/prepareRename",
            json!({
                "textDocument": { "uri": child_uri },
                "position": position_at(child_text, "= clk", 4)
            }),
        )
        .expect("prepareRename at clk use");
    assert_eq!(
        prepared.get("placeholder").and_then(Value::as_str),
        Some("clk"),
        "prepareRename must offer the current name: {prepared}"
    );
    assert_eq!(
        prepared.get("range").and_then(|range| range.get("start")),
        Some(&clk_use),
        "range start must be the identifier start: {prepared}"
    );

    // The rename itself: declaration + use in ren_child.sv, label in ren_top.sv.
    let new_name = "clk_in";
    let result = client
        .request(
            "textDocument/rename",
            json!({
                "textDocument": { "uri": child_uri },
                "position": position_at(child_text, "= clk", 4),
                "newName": new_name
            }),
        )
        .expect("rename request for clk");
    assert_no_shadow_uris(&result);
    let changes = result
        .get("changes")
        .and_then(Value::as_object)
        .expect("workspace edit changes");
    let uris: Vec<&String> = changes.keys().collect();
    assert_eq!(
        uris.len(),
        2,
        "edits must span exactly both files: {changes:?}"
    );

    // ren_child.sv: port declaration + in-module use, 3 chars each.
    let child_edits = edits_for(&result, &child_uri);
    let expected_decl = position_at(child_text, "input logic clk", 12);
    assert_eq!(child_edits.len(), 2, "child edits: {child_edits:?}");
    assert_eq!(start_key(&child_edits[0].0), start_key(&expected_decl));
    assert_eq!(start_key(&child_edits[1].0), start_key(&clk_use));
    for (start, end, text) in &child_edits {
        assert_eq!(text, new_name);
        assert_eq!(
            end.get("character").and_then(Value::as_u64),
            Some(start.get("character").and_then(Value::as_u64).expect("c") + 3),
            "only the `clk` identifier may be replaced: {start}..{end}"
        );
    }

    // ren_top.sv: exactly the `.clk` connection-label occurrence.
    let top_edits = edits_for(&result, &top_uri);
    let expected_label = position_at(top_text, ".clk", 1);
    assert_eq!(top_edits.len(), 1, "top edits: {top_edits:?}");
    assert_eq!(start_key(&top_edits[0].0), start_key(&expected_label));
    assert_eq!(top_edits[0].2, new_name);
    client.shutdown();
}

/// Scenario (2): prepareRename answers a range+placeholder on a parameter and
/// NULL on a keyword.
#[test]
fn lsp_stdio_prepare_rename_parameter_and_keyword() {
    let text = "module param_mod #(parameter W = 8)(\n\
                \x20 input logic [W-1:0] d,\n\
                \x20 output logic [7:0] q\n\
                );\n\
                \x20 assign q = d;\n\
                endmodule\n\
                \n\
                module tb_params;\n\
                \x20 logic [7:0] d;\n\
                \x20 logic [7:0] q;\n\
                \x20 param_mod #(.W(8)) u0(.d(d), .q(q));\n\
                endmodule\n";
    let (_cleanup, ws) = rename_workspace("param", &[("param_mod.sv", text)]);
    let path = ws.join("param_mod.sv");
    let uri = file_uri(&path);

    let mut client = LspProcess::spawn(&ws);
    client
        .initialize(&[("rename-ws", &ws)], default_init_options())
        .expect("initialize parameter rename workspace");
    client.open(&path, text).expect("open param_mod");
    wait_for_diagnostics(&mut client, &uri, has_no_severity_1);

    // Parameter declaration: placeholder W, single-character range.
    let w_decl = position_at(text, "parameter W = 8", 10);
    let prepared = client
        .request(
            "textDocument/prepareRename",
            json!({
                "textDocument": { "uri": uri },
                "position": w_decl
            }),
        )
        .expect("prepareRename on parameter");
    assert_eq!(
        prepared.get("placeholder").and_then(Value::as_str),
        Some("W")
    );
    assert_eq!(
        prepared.get("range").and_then(|range| range.get("start")),
        Some(&w_decl)
    );

    // Keyword position (`module`): not renamable → null.
    let keyword = client
        .request(
            "textDocument/prepareRename",
            json!({
                "textDocument": { "uri": uri },
                "position": position_at(text, "module param_mod", 2)
            }),
        )
        .expect("prepareRename on keyword");
    assert!(
        keyword.is_null(),
        "a keyword must not be renamable: {keyword}"
    );
    client.shutdown();
}

/// Scenario (3): an illegal new name is rejected with an invalidParams error;
/// nothing renames silently.
#[test]
fn lsp_stdio_rename_rejects_invalid_names() {
    let text = "module inv_mod;\n\
                \x20 logic sig1;\n\
                \x20 assign sig1 = 1'b0;\n\
                endmodule\n";
    let (_cleanup, ws) = rename_workspace("invalid-name", &[("inv_mod.sv", text)]);
    let path = ws.join("inv_mod.sv");
    let uri = file_uri(&path);

    let mut client = LspProcess::spawn(&ws);
    client
        .initialize(&[("rename-ws", &ws)], default_init_options())
        .expect("initialize invalid-name workspace");
    client.open(&path, text).expect("open inv_mod");
    wait_for_diagnostics(&mut client, &uri, has_no_severity_1);

    let sig_decl = position_at(text, "logic sig1", 6);
    for bad in ["1abc", "has space", "a-b", "module"] {
        let error = client
            .request(
                "textDocument/rename",
                json!({
                    "textDocument": { "uri": uri },
                    "position": sig_decl,
                    "newName": bad
                }),
            )
            .err()
            .unwrap_or_else(|| panic!("`{bad}` must be rejected"));
        assert!(
            error.contains("-32602") && error.contains(bad),
            "invalid name must yield invalid_params naming the offender: {error}"
        );
    }

    // A valid rename still works afterwards.
    let ok = client
        .request(
            "textDocument/rename",
            json!({
                "textDocument": { "uri": uri },
                "position": sig_decl,
                "newName": "sig2"
            }),
        )
        .expect("valid rename after rejections");
    let edits = edits_for(&ok, &uri);
    assert_eq!(edits.len(), 2, "decl + use: {edits:?}");
    client.shutdown();
}

/// Scenario (4): prefix-collision safety — renaming `data` never touches
/// `data_out`, even though every occurrence shares a prefix and lives in the
/// same module.
#[test]
fn lsp_stdio_rename_is_prefix_collision_safe() {
    let text = "module data_mod;\n\
                \x20 logic data;\n\
                \x20 logic data_out;\n\n\
                \x20 assign data_out = ~data;\n\
                endmodule\n";
    let (_cleanup, ws) = rename_workspace("prefix", &[("data_mod.sv", text)]);
    let path = ws.join("data_mod.sv");
    let uri = file_uri(&path);

    let mut client = LspProcess::spawn(&ws);
    client
        .initialize(&[("rename-ws", &ws)], default_init_options())
        .expect("initialize prefix workspace");
    client.open(&path, text).expect("open data_mod");
    wait_for_diagnostics(&mut client, &uri, has_no_severity_1);

    let data_decl = position_at(text, "logic data;", 6);
    let data_use = position_at(text, "~data;", 1);
    let result = client
        .request(
            "textDocument/rename",
            json!({
                "textDocument": { "uri": uri },
                "position": data_decl,
                "newName": "din"
            }),
        )
        .expect("rename request for data");
    let edits = edits_for(&result, &uri);
    assert_eq!(
        edits.len(),
        2,
        "exactly the two `data` occurrences may be edited: {edits:?}"
    );
    assert_eq!(start_key(&edits[0].0), start_key(&data_decl));
    assert_eq!(start_key(&edits[1].0), start_key(&data_use));
    for (start, end, new_text) in &edits {
        assert_eq!(new_text, "din");
        assert_eq!(
            end.get("character").and_then(Value::as_u64),
            Some(start.get("character").and_then(Value::as_u64).expect("c") + 4),
            "only the 4-char `data` identifier may be replaced: {start}..{end}"
        );
    }

    // The same request from the `data_out` side stays on its own family.
    let data_out_decl = position_at(text, "logic data_out;", 6);
    let prepared = client
        .request(
            "textDocument/prepareRename",
            json!({
                "textDocument": { "uri": uri },
                "position": data_out_decl
            }),
        )
        .expect("prepareRename on data_out");
    assert_eq!(
        prepared.get("placeholder").and_then(Value::as_str),
        Some("data_out"),
        "{prepared}"
    );
    client.shutdown();
}

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

fn memo_cache_stats(client: &mut LspProcess, uri: &str) -> (u64, u64) {
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

// ── Parameter hover: elaborated values from the committed analysis ───────────
//
// Contract:
// * hovering a parameter/localparam DECLARATION — or any binding-precise
//   REFERENCE to it, including uses inside the instance body — appends a
//   short `value = <const>` line rendered ONLY from the committed analysis
//   model (`InstanceModel.params` / gen-scope / package parameters).  No
//   parsing or elaboration happens in the hover request path; unresolved
//   values omit the line silently.
// * `[compile.param_overrides]` (Surelog `-P`) values show up wherever the
//   overridden value is committed — declaration and in-instance use sites
//   alike.
// * identical repeat hovers return byte-identical responses served by the
//   request cache: the `# request-cache:` hits counter grows while misses
//   stay put (no recompute).

/// Single-root generated workspace with a `-PWIDTH=8` override applied to its
/// top module (modeled on the config_effect generate-branch scenario).
const POV_TOP_SV: &str = "\
module pov_top #(parameter int WIDTH = 4)();
  generate
    if (WIDTH >= 8) begin : g_wide
      localparam int BRANCH = 8;
    end else begin : g_narrow
      localparam int BRANCH = 4;
    end
  endgenerate
  localparam int PLAIN = 7;
  localparam int DEPTH = WIDTH * 2;
endmodule
";

const POV_TOP_TOML: &str = "\
schema_version = 1

[sources]
directories = [\".\"]
include = [\"**/*.v\", \"**/*.sv\"]

[lint]
enabled = false

[compile]
top = \"pov_top\"

[compile.param_overrides]
WIDTH = 8
";

fn param_hover_workspace(tag: &str) -> (TempDirCleanup, PathBuf, PathBuf) {
    let base =
        std::env::temp_dir().join(format!("llg-lsp-param-hover-{}-{tag}", std::process::id()));
    let ws = base.join("ws");
    fs::create_dir_all(&ws).expect("create param-hover workspace");
    fs::write(ws.join(CONFIG_FILE), POV_TOP_TOML).expect("write llg.toml");
    let path = ws.join("pov_top.sv");
    fs::write(&path, POV_TOP_SV).expect("write pov_top.sv");
    (TempDirCleanup(base), ws, path)
}

/// Markup payload of a hover request at `pos` (empty string for a null hover).
fn param_hover_markup(client: &mut LspProcess, uri: &str, pos: &Value) -> String {
    let hover = client
        .request(
            "textDocument/hover",
            json!({ "textDocument": { "uri": uri }, "position": pos }),
        )
        .expect("hover request");
    hover
        .get("contents")
        .and_then(|contents| contents.get("value"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

/// The `value = …` line of a hover markup, if present.
fn value_line(markup: &str) -> Option<&str> {
    markup
        .lines()
        .find(|line| line.starts_with("value = "))
        .map(|line| line.trim_start_matches("value = "))
}

/// Numeric payload of an elaborated literal (`32'sd8`, `64'd8`, `7` → 8).
fn literal_tail(rendered: &str) -> u64 {
    let digits = match rendered.split_once('\'') {
        Some((_, rest)) => rest.trim_start_matches(['s', 'd']),
        None => rendered,
    };
    digits
        .parse()
        .unwrap_or_else(|error| panic!("elaborated value {rendered:?} must end in digits: {error}"))
}

/// (a)+(b): simple and expression-valued localparams plus the overridden top
/// parameter show their elaborated values at declaration AND in-instance
/// reference sites; the surviving generate branch reports its own constant.
#[test]
fn lsp_stdio_param_hover_shows_elaborated_and_overridden_values() {
    let (_guard, ws, path) = param_hover_workspace("values");
    let uri = file_uri(&path);

    let mut client = LspProcess::spawn(&ws);
    client
        .initialize(&[("pov-ws", &ws)], default_init_options())
        .expect("initialize param-hover workspace");
    client.open(&path, POV_TOP_SV).expect("open pov_top.sv");
    wait_for_diagnostics(&mut client, &uri, has_no_severity_1);

    // (a) A plain localparam shows its value on its own short line.
    let plain_decl = position_at(POV_TOP_SV, "localparam int PLAIN", 15);
    let markup = param_hover_markup(&mut client, &uri, &plain_decl);
    assert!(
        markup.contains("localparam PLAIN"),
        "declaration text must stay: {markup:?}"
    );
    assert_eq!(
        value_line(&markup).map(literal_tail),
        Some(7),
        "simple localparam must show its elaborated value: {markup:?}"
    );

    // The expression-valued DEPTH (= WIDTH * 2 with WIDTH overridden to 8)
    // shows the EVALUATED constant from the committed model.
    let depth_decl = position_at(POV_TOP_SV, "localparam int DEPTH", 15);
    let markup = param_hover_markup(&mut client, &uri, &depth_decl);
    assert_eq!(
        value_line(&markup).map(literal_tail),
        Some(16),
        "expression-valued localparam must show the evaluated constant: {markup:?}"
    );

    // (b) The -P-overridden top parameter shows 8 (not the source default 4).
    let width_decl = position_at(POV_TOP_SV, "parameter int WIDTH", 14);
    let markup = param_hover_markup(&mut client, &uri, &width_decl);
    assert!(
        markup.contains("parameter WIDTH"),
        "declaration text must stay: {markup:?}"
    );
    assert_eq!(
        value_line(&markup).map(literal_tail),
        Some(8),
        "the overridden value must win over the source default: {markup:?}"
    );

    // In-instance reference site: the `WIDTH` use inside DEPTH's initializer
    // binds through ref_bindings and still shows the overridden value.
    let width_ref = position_at(POV_TOP_SV, "= WIDTH * 2", 2);
    let markup = param_hover_markup(&mut client, &uri, &width_ref);
    assert_eq!(
        value_line(&markup).map(literal_tail),
        Some(8),
        "in-instance reference must show the overridden value: {markup:?}"
    );

    // Generate-scope parameter of the surviving branch (g_wide under the
    // override) resolves through the committed gen-scope model.
    let branch_decl = position_at(POV_TOP_SV, "localparam int BRANCH", 15);
    let markup = param_hover_markup(&mut client, &uri, &branch_decl);
    assert_eq!(
        value_line(&markup).map(literal_tail),
        Some(8),
        "branch-local parameter must show its elaborated constant: {markup:?}"
    );

    client.shutdown();
}

/// (c): two identical hover requests return byte-identical responses AND are
/// served by the request memoization cache (hits counter grows, misses do
/// not) — proving no recomputation happens between repeats.
#[test]
fn lsp_stdio_param_hover_repeats_are_memoized() {
    let (_guard, ws, path) = param_hover_workspace("memo");
    let uri = file_uri(&path);

    let mut client = LspProcess::spawn(&ws);
    client
        .initialize(&[("pov-ws", &ws)], default_init_options())
        .expect("initialize param-hover workspace");
    client.open(&path, POV_TOP_SV).expect("open pov_top.sv");
    wait_for_diagnostics(&mut client, &uri, has_no_severity_1);

    let width_decl = position_at(POV_TOP_SV, "parameter int WIDTH", 14);
    // Warm the dumpTokens stats path itself so its own counters cannot
    // confound the miss comparison below.
    let _ = memo_cache_stats(&mut client, &uri);

    let cold = param_hover_markup(&mut client, &uri, &width_decl);
    let warm = param_hover_markup(&mut client, &uri, &width_decl);
    assert_eq!(cold, warm, "repeat hover markup must be byte-identical");

    let (hits_before, misses_before) = memo_cache_stats(&mut client, &uri);
    let warm_again = param_hover_markup(&mut client, &uri, &width_decl);
    let (hits_after, misses_after) = memo_cache_stats(&mut client, &uri);
    assert_eq!(warm_again, cold, "third response drifted");
    assert!(
        misses_after == misses_before,
        "the repeat hover recomputed (misses {} -> {})",
        misses_before,
        misses_after
    );
    assert!(
        hits_after > hits_before,
        "the repeat hover was not served from the request cache"
    );
    assert!(
        value_line(&cold).map(literal_tail) == Some(8),
        "memoized payload keeps the elaborated value line: {cold:?}"
    );

    client.shutdown();
}

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

// ── llg/configChanged notification ─────────────────────────────────────────
//
// An effective llg.toml reload (e.g. a `[compile] defines` edit) must PUSH a
// `llg/configChanged` notification to the client: inactive-range dimming is
// pulled by the client per open buffer, so without the push it would stay
// stale indefinitely after editing defines (diagnostics republish themselves,
// they cannot carry the signal).  Client-side watcher behavior cannot be
// exercised over stdio; this pins the server half of the contract — emission
// on every state-changing reload, empty params, fresh data behind the signal,
// and NO emission when a reload parses to an identical config.

#[test]
fn lsp_stdio_config_hot_reload_pushes_config_changed_notification() {
    const SOURCE: &str = "\
module m;
`ifdef FEATURE
  logic on;
`else
  logic off;
`endif
endmodule
";
    const CONFIG_DEFINES_FEATURE: &str = "schema_version = 1\n\
         [sources]\n\
         directories = [\".\"]\n\
         include = [\"**/*.v\", \"**/*.sv\"]\n\
         [compile]\n\
         defines = [\"FEATURE\"]\n\
         [lint]\n\
         enabled = false\n";
    let (guard, ws) = rename_workspace("config-changed", &[("top.sv", SOURCE)]);
    let path = ws.join("top.sv");
    let uri = file_uri(&path);
    let config_path = ws.join(CONFIG_FILE);

    let mut client = LspProcess::spawn(&ws);
    client
        .initialize(&[("config-changed-ws", &ws)], default_init_options())
        .expect("initialize config-changed workspace");
    client.open(&path, SOURCE).expect("open source");
    wait_for_diagnostics(&mut client, &uri, has_no_severity_1);

    // Hot-reload `[compile] defines` through the watched llg.toml.
    fs::write(&config_path, CONFIG_DEFINES_FEATURE).expect("write config enabling FEATURE");
    client
        .send_watch_event(&config_path, 2)
        .expect("send config watch event");

    // The server must PUSH the reload signal; the params object is empty by
    // contract (the client refetches everything it decorates).
    let notified = client
        .wait_for_notification_where("llg/configChanged", |params| {
            params.as_object().is_some_and(|object| object.is_empty())
        })
        .expect("llg/configChanged notification after defines hot reload");

    // The data behind the signal is already committed: the SAME open buffer
    // answers with the complementary ranges, no didChange/didSave involved.
    let deadline = Instant::now() + POLL_TIMEOUT;
    loop {
        if Instant::now() > deadline {
            panic!("inactive ranges never flipped after the configChanged push: {notified}");
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

    // A reload that parses to an IDENTICAL config changes no state and must
    // NOT re-notify.  Bounded absence window: any wrongful notification is
    // sent synchronously with the reload handling, well inside the window.
    let notifications_before = client.notifications.len();
    fs::write(&config_path, CONFIG_DEFINES_FEATURE).expect("rewrite identical config");
    client
        .send_watch_event(&config_path, 2)
        .expect("send identical-config watch event");
    let quiet_deadline = Instant::now() + Duration::from_secs(3);
    while let Ok(message) = client.receive_until(quiet_deadline) {
        client
            .route_unsolicited(message)
            .expect("route messages while waiting out the identical reload");
    }
    assert!(
        !client.notifications[notifications_before..]
            .iter()
            .any(|message| message.get("method").and_then(Value::as_str)
                == Some("llg/configChanged")),
        "identical reload must not re-notify: {:?}",
        &client.notifications[notifications_before..]
    );

    client.shutdown();
    drop(guard);
}

// ── Macro-usage hover: resolved values from committed data ───────────────────
//
// Contract:
// * hovering a macro USAGE (`` `NAME ``) shows its RESOLVED VALUE, rendered
//   like the parameter-hover house style (`macro WIDTH = 8` inside a
//   SystemVerilog fence).  The value comes ONLY from committed data: the
//   root's `[compile] defines` (authoritative base table for every file)
//   plus in-source `` `define ``/`` `undef `` directives resolved per file,
//   positionally (last definition wins; conditionals honored against the
//   evolving table).  Nothing is parsed or elaborated by the request.
// * hovering an UNDEFINED macro states that it is not defined under the
//   current configuration and names the checked config file — never a wrong
//   value.
// * identical repeat hovers are byte-identical AND served from the request
//   memoization cache (`# request-cache:` hits grow, misses stay flat).

const MACRO_TOP_TOML: &str = "\
schema_version = 1

[sources]
directories = [\".\"]
include = [\"**/*.v\", \"**/*.sv\"]

[lint]
enabled = false

[compile]
defines = [\"DEPTH=16\"]
";

const MACRO_TOP_SV: &str = "\
`define WIDTH_LOCAL 8
module macro_top;
  localparam int W = `WIDTH_LOCAL;
  localparam int D = `DEPTH;
endmodule
module macro_undef_user;
  localparam int U = `NOWHERE;
endmodule
";

fn macro_hover_workspace(tag: &str) -> (TempDirCleanup, PathBuf, PathBuf) {
    let base =
        std::env::temp_dir().join(format!("llg-lsp-macro-hover-{}-{tag}", std::process::id()));
    let ws = base.join("ws");
    fs::create_dir_all(&ws).expect("create macro-hover workspace");
    fs::write(ws.join(CONFIG_FILE), MACRO_TOP_TOML).expect("write llg.toml");
    let path = ws.join("macro_top.sv");
    fs::write(&path, MACRO_TOP_SV).expect("write macro_top.sv");
    (TempDirCleanup(base), ws, path)
}

fn macro_hover_markup(client: &mut LspProcess, uri: &str, pos: &Value) -> String {
    let hover = client
        .request(
            "textDocument/hover",
            json!({ "textDocument": { "uri": uri }, "position": pos }),
        )
        .expect("hover request");
    hover
        .get("contents")
        .and_then(|contents| contents.get("value"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

/// (a)+(b)+(c): in-source macro shows its value, a `[compile] defines` macro
/// shows the configured value, and an undefined macro shows the not-defined
/// message naming llg.toml.
#[test]
fn lsp_stdio_macro_hover_shows_resolved_config_and_undefined_values() {
    let (_guard, ws, path) = macro_hover_workspace("values");
    let uri = file_uri(&path);

    let mut client = LspProcess::spawn(&ws);
    client
        .initialize(&[("macro-ws", &ws)], default_init_options())
        .expect("initialize macro-hover workspace");
    client.open(&path, MACRO_TOP_SV).expect("open macro_top.sv");
    // The undefined `NOWHERE makes Surelog report an error; the analysis is
    // still feature-servable, which is exactly what this wait observes.
    wait_for_diagnostics(&mut client, &uri, has_severity_1);

    // (a) In-source define: the usage resolves to its defining value.
    let local_use = position_at(MACRO_TOP_SV, "`WIDTH_LOCAL;", 3);
    let markup = macro_hover_markup(&mut client, &uri, &local_use);
    assert!(
        markup.contains("```systemverilog"),
        "defined macros render in the standard code fence: {markup:?}"
    );
    assert!(
        markup.contains("macro WIDTH_LOCAL = 8"),
        "the in-source value must show: {markup:?}"
    );
    assert!(
        markup.contains(&format!("defined at {}", path.display())),
        "source-origin definitions name their site: {markup:?}"
    );

    // (b) Config define: the usage shows the `[compile] defines` value.
    let config_use = position_at(MACRO_TOP_SV, "`DEPTH;", 2);
    let markup = macro_hover_markup(&mut client, &uri, &config_use);
    assert!(
        markup.contains("macro DEPTH = 16"),
        "the config-supplied value must show: {markup:?}"
    );

    // (c) Undefined macro: explicit not-defined message naming the config.
    let undefined_use = position_at(MACRO_TOP_SV, "`NOWHERE;", 3);
    let markup = macro_hover_markup(&mut client, &uri, &undefined_use);
    assert!(
        markup.contains("`NOWHERE` is not defined under the current configuration"),
        "undefined macros must say so: {markup:?}"
    );
    assert!(
        markup.contains("[compile] defines"),
        "the message must point at the checked configuration surface: {markup:?}"
    );
    assert!(
        !markup.contains("```systemverilog"),
        "a status message must not masquerade as code: {markup:?}"
    );

    client.shutdown();
}

/// (d): two identical hovers are byte-identical AND served from the request
/// cache — hits grow, misses stay flat (no recomputation between repeats),
/// proving warm repeats cost microseconds rather than a rescan or reparse.
#[test]
fn lsp_stdio_macro_hover_repeats_are_memoized() {
    let (_guard, ws, path) = macro_hover_workspace("memo");
    let uri = file_uri(&path);

    let mut client = LspProcess::spawn(&ws);
    client
        .initialize(&[("macro-ws", &ws)], default_init_options())
        .expect("initialize macro-hover workspace");
    client.open(&path, MACRO_TOP_SV).expect("open macro_top.sv");
    wait_for_diagnostics(&mut client, &uri, has_severity_1);

    let local_use = position_at(MACRO_TOP_SV, "`WIDTH_LOCAL;", 3);
    // Warm the dumpTokens stats path itself so its own counters cannot
    // confound the miss comparison below.
    let _ = memo_cache_stats(&mut client, &uri);

    let cold = macro_hover_markup(&mut client, &uri, &local_use);
    assert!(
        cold.contains("macro WIDTH_LOCAL = 8"),
        "cold payload carries the resolved value: {cold:?}"
    );
    let warm = macro_hover_markup(&mut client, &uri, &local_use);
    assert_eq!(warm, cold, "repeat hover markup must be byte-identical");

    let (hits_before, misses_before) = memo_cache_stats(&mut client, &uri);
    let warm_again = macro_hover_markup(&mut client, &uri, &local_use);
    let (hits_after, misses_after) = memo_cache_stats(&mut client, &uri);
    assert_eq!(warm_again, cold, "third response drifted");
    assert!(
        misses_after == misses_before,
        "the repeat hover recomputed (misses {} -> {})",
        misses_before,
        misses_after
    );
    assert!(
        hits_after > hits_before,
        "the repeat hover was not served from the request cache"
    );

    client.shutdown();
}
