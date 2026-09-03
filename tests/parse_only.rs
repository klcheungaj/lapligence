//! Integration coverage for the single-source Surelog parse-only API.
//!
//! Surelog keeps process-global state and `std::env::set_current_dir` is
//! process-global too, so both tests run with the CWD pointed at a fresh
//! temp dir and are serialized through a mutex.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use llg::core::compile;

/// Serializes the two `#[test]`s: Surelog uses process-global C++ singletons
/// (its `FileSystem` freezes the first session's working directory for the
/// whole process) and the CWD is process-global (`CwdGuard` chdirs), so the
/// tests must not run concurrently in one process.  The lock is acquired
/// before the shared temp dir is (re)created and released only after it is
/// dropped, so Surelog's frozen working-directory path is never deleted while
/// another test's session is alive.
static SURELOG_LOCK: Mutex<()> = Mutex::new(());

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new() -> Self {
        // One stable path per process (matching the other Surelog test
        // binaries): Surelog's FileSystem singleton freezes the first
        // session's working directory for the whole process, so every test
        // must chdir into the same directory and it must be recreated at the
        // same path.  Cleanup happens under `SURELOG_LOCK`.
        let path = std::env::temp_dir().join(format!("llg-parse-only-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create parse-only temp directory");
        Self { path }
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

struct CwdGuard(PathBuf);

impl CwdGuard {
    fn enter(path: &Path) -> Self {
        let previous = std::env::current_dir().expect("read current directory");
        std::env::set_current_dir(path).expect("enter parse-only working directory");
        Self(previous)
    }
}

impl Drop for CwdGuard {
    fn drop(&mut self) {
        std::env::set_current_dir(&self.0).expect("restore current directory");
    }
}

#[test]
fn parse_only_collects_tokens_from_the_requested_source_without_consuming_its_include() {
    // Arrange
    // Hold the lock for the whole test — shared temp-dir (re)creation,
    // chdir, parse and cleanup — so Surelog's frozen working-directory path
    // is never deleted while this session is alive (drop order: `_guard`
    // precedes `_cwd` and `temp`, so it outlives both on unwind).
    let _guard = SURELOG_LOCK.lock().unwrap();
    let temp = TempDir::new();
    let project = temp.path.join("project");
    let work = temp.path.join("work");
    fs::create_dir_all(&project).expect("create parse-only project directory");
    fs::create_dir_all(&work).expect("create parse-only working directory");
    let source = project.join("opened.sv");
    let included = project.join("not_consumed.svh");
    let source_text = "// llg-test-fixture: tests/parse_only.rs/opened.sv\r\n\
`include \"not_consumed.svh\"\r\n\
\r\n\
module ParsedInMemory #(parameter int WIDTH = `INCLUDED_WIDTH);\r\n\
  logic local_signal;\r\n\
endmodule\r\n";
    let included_text = "// llg-test-fixture: tests/parse_only.rs/not_consumed.svh\n\
`define INCLUDED_WIDTH 8\n\
`include \"parse_only_missing_include.svh\"\n\
module IncludedMustNotBeParsed;\n\
  logic included_signal;\n\
endmodule\n";
    fs::write(&source, source_text).expect("write parse-only source");
    fs::write(&included, included_text).expect("write include sentinel");
    let _cwd = CwdGuard::enter(&work);

    // Act
    let parsed = compile::parse_only(
        source.to_str().expect("UTF-8 parse-only source path"),
        &["-DSEMANTIC_ONLY=1".to_owned()],
    )
    .expect("parse one source");

    // Assert
    assert_eq!(
        parsed.tokens.len(),
        1,
        "parse-only must create FileContent for only the requested source: {:?}",
        parsed
            .tokens
            .iter()
            .map(|tokens| &tokens.path)
            .collect::<Vec<_>>()
    );
    let tokens = &parsed.tokens[0];
    assert_eq!(Path::new(&tokens.path), source);
    assert!(
        tokens.nodes.iter().any(|node| {
            node.name.as_deref() == Some("ParsedInMemory") && node.line == 4 && node.col == 8
        }),
        "requested module token did not preserve its source position: {:?}",
        tokens.nodes
    );
    assert!(
        tokens.nodes.iter().any(|node| {
            node.name.as_deref() == Some("module") && node.line == 4 && node.col == 1
        }),
        "literal module keyword was not recovered from the opened source: {:?}",
        tokens.nodes
    );
    assert!(
        tokens.nodes.iter().all(|node| {
            !matches!(
                node.name.as_deref(),
                Some("IncludedMustNotBeParsed" | "included_signal")
            )
        }),
        "included-file tokens leaked into the parse-only result"
    );
    assert!(
        parsed.diagnostics.iter().all(|diagnostic| {
            diagnostic.file.as_deref() != included.to_str()
                && !diagnostic.message.contains("not_consumed.svh")
                && !diagnostic
                    .message
                    .contains("parse_only_missing_include.svh")
        }),
        "parse-only diagnostics show that the include was consumed: {:?}",
        parsed.diagnostics
    );
    assert_eq!(fs::read_to_string(&source).unwrap(), source_text);
    assert_eq!(fs::read_to_string(&included).unwrap(), included_text);
    assert!(
        !project.join("slpp_all").exists(),
        "Surelog artifacts must stay out of the source tree"
    );
}

/// The isolated open-buffer path must mark connection LABELS structurally:
/// `.label` identifiers carry `TOKEN_PORT_CONN_LABEL` and `#(.LABEL)`
/// overrides carry `TOKEN_PARAM_CONN_LABEL` (which `semantic_tokens::encode`
/// renders with the `connectionLabel` modifier), while the connected
/// actual/RHS identifiers never do.  The child module is intentionally absent
/// — classification is purely syntactic, exactly like the LSP's request-local
/// staged-buffer serving path.
#[test]
fn parse_only_classifies_connection_labels_and_leaves_actuals_plain() {
    use llg::ffi::vpi;

    // Arrange: single-line AND multi-line instantiations with named
    // parameter overrides and named port connections.
    //
    // 1-based identifier columns on the asserted lines:
    //   line 6:  `  child #(.W(4), .D(wa)) u_iso (.clk(wa), .q(tq));`
    //            W=12 D=19 wa=21          clk=34 wa=38 q=44 tq=46
    //   line 9+: `    .W(8),` / `    .clk(wa),` … → labels col 6.
    let _guard = SURELOG_LOCK.lock().unwrap();
    let temp = TempDir::new();
    let work = temp.path.join("work");
    fs::create_dir_all(&work).expect("create parse-only working directory");
    let source = temp.path.join("labels.sv");
    let source_text = concat!(
        "// llg-test-fixture: tests/parse_only.rs/labels.sv\n",
        "module labels;\n",
        "  logic wa;\n",
        "  logic [7:0] tq;\n",
        "\n",
        "  child #(.W(4), .D(wa)) u_iso (.clk(wa), .q(tq));\n",
        "\n",
        "  child #(\n",
        "    .W(8),\n",
        "    .D(1)\n",
        "  ) u_ml (\n",
        "    .clk(wa),\n",
        "    .q(tq)\n",
        "  );\n",
        "endmodule\n",
    );
    fs::write(&source, source_text).expect("write parse-only label source");
    let _cwd = CwdGuard::enter(&work);

    // Act
    let parsed = compile::parse_only(
        source.to_str().expect("UTF-8 parse-only label source path"),
        &[],
    )
    .expect("parse one label source");
    assert_eq!(parsed.tokens.len(), 1, "only the requested file parses");
    let tokens = &parsed.tokens[0];

    // Synthetic type of the identifier token at 1-based `(line, col)`.
    let type_at = |line: u32, col: u32| -> i32 {
        tokens
            .nodes
            .iter()
            .find(|node| node.line == line && node.col == col)
            .unwrap_or_else(|| panic!("no token at {line}:{col}: {:?}", tokens.nodes))
            .vpi_type
    };

    // Assert — single-line instantiation (line 6).
    // Override labels carry TOKEN_PARAM_CONN_LABEL …
    assert_eq!(
        type_at(6, 12),
        vpi::TOKEN_PARAM_CONN_LABEL,
        ".W override label must be classified as a param connection label"
    );
    assert_eq!(
        type_at(6, 19),
        vpi::TOKEN_PARAM_CONN_LABEL,
        ".D override label must be classified as a param connection label"
    );
    // … its RHS stays a plain signal reference …
    assert_ne!(
        type_at(6, 21),
        vpi::TOKEN_PARAM_CONN_LABEL,
        ".D RHS must not be a connection label"
    );
    // … port labels carry TOKEN_PORT_CONN_LABEL …
    assert_eq!(
        type_at(6, 34),
        vpi::TOKEN_PORT_CONN_LABEL,
        ".clk label must be classified as a port connection label"
    );
    assert_eq!(
        type_at(6, 44),
        vpi::TOKEN_PORT_CONN_LABEL,
        ".q label must be classified as a port connection label"
    );
    // … while the connected signals stay plain references.
    assert_ne!(
        type_at(6, 38),
        vpi::TOKEN_PORT_CONN_LABEL,
        ".clk actual must not be a connection label"
    );
    assert_ne!(
        type_at(6, 46),
        vpi::TOKEN_PORT_CONN_LABEL,
        ".q actual must not be a connection label"
    );

    // Assert — multi-line instantiation: identical marking on the
    // continuation lines.
    assert_eq!(
        type_at(9, 6),
        vpi::TOKEN_PARAM_CONN_LABEL,
        "multi-line .W label must be a param connection label"
    );
    assert_eq!(
        type_at(10, 6),
        vpi::TOKEN_PARAM_CONN_LABEL,
        "multi-line .D label must be a param connection label"
    );
    assert_eq!(
        type_at(12, 6),
        vpi::TOKEN_PORT_CONN_LABEL,
        "multi-line .clk label must be a port connection label"
    );
    assert_ne!(
        type_at(12, 10),
        vpi::TOKEN_PORT_CONN_LABEL,
        "multi-line .clk actual must not be a connection label"
    );
    assert_eq!(
        type_at(13, 6),
        vpi::TOKEN_PORT_CONN_LABEL,
        "multi-line .q label must be a port connection label"
    );
    assert_ne!(
        type_at(13, 8),
        vpi::TOKEN_PORT_CONN_LABEL,
        "multi-line .q actual must not be a connection label"
    );
}
