//! The path operand of `` `include `` over the wire (ST-23): a `string`
//! token with no modifiers, for both `"file"` and `<file>`, in the project
//! stream of an unopened file and in the open-buffer stream.

use super::*;

const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/semantic_tokens/include_path"
);

/// The fixture project copied under a fresh temp base; returns the cleanup
/// guard and the workspace root.
pub(super) fn include_path_root(tag: &str) -> (TempDirCleanup, PathBuf) {
    let base =
        std::env::temp_dir().join(format!("llg-lsp-include-path-{}-{tag}", std::process::id()));
    let root = base.join("project");
    copy_tree(Path::new(FIXTURE), &root).expect("copy include path fixture");
    (TempDirCleanup(base), root)
}

/// One line per token in the form `line column "text" type modifiers`, as
/// the reporter's script prints it (the modifier list is comma-joined, and
/// empty after a trailing space). The fixture is ASCII, so columns index
/// bytes.
fn token_listing(
    result: &Value,
    source: &str,
    types: &[String],
    modifiers: &[String],
) -> Vec<String> {
    let data = result
        .get("data")
        .and_then(Value::as_array)
        .expect("semantic token result data");
    let lines: Vec<&str> = source.split('\n').collect();
    let (mut line, mut character) = (0u64, 0u64);
    let mut out = Vec::new();
    for token in data.as_chunks::<5>().0 {
        let field = |index: usize| token[index].as_u64().expect("semantic token field");
        if field(0) != 0 {
            line += field(0);
            character = 0;
        }
        character += field(1);
        let start = character as usize;
        let text = &lines[line as usize][start..start + field(2) as usize];
        let names: Vec<&str> = modifiers
            .iter()
            .enumerate()
            .filter(|(bit, _)| field(4) & (1 << bit) != 0)
            .map(|(_, name)| name.as_str())
            .collect();
        out.push(format!(
            "{line} {character} {} {} {}",
            serde_json::to_string(text).expect("quote token text"),
            types[field(3) as usize],
            names.join(",")
        ));
    }
    out
}

/// The whole token stream of `t.sv` equals the reporter's expected stream,
/// byte for byte, in both serving paths: `` `include `` stays `keyword` +
/// `controlFlow`, each path operand is `string` with no modifiers, and the
/// rest of the file is unchanged.
#[test]
fn include_path_operands_are_strings_in_the_whole_token_stream() {
    let (_cleanup, root) = include_path_root("stream");
    let source = fs::read_to_string(root.join("t.sv")).expect("read t.sv");
    let expected: Vec<String> = fs::read_to_string(root.join("expected-tokens.txt"))
        .expect("read expected tokens")
        .lines()
        .map(str::to_owned)
        .collect();
    let path = root.join("t.sv");
    let uri = file_uri(&path);
    let mut client = LspProcess::spawn(&root);
    let initialize = client
        .initialize(&[("project", &root)], default_init_options())
        .unwrap();
    let legend = &initialize["capabilities"]["semanticTokensProvider"]["legend"];
    let types = legend_names(legend, "tokenTypes");
    let modifiers = legend_names(legend, "tokenModifiers");
    wait_for_diagnostics(&mut client, &uri, |_| true);
    for opened in [false, true] {
        if opened {
            client.open(&path, &source).expect("open t.sv");
        }
        let result = client
            .request(
                "textDocument/semanticTokens/full",
                json!({ "textDocument": { "uri": uri } }),
            )
            .expect("t.sv semantic token response");
        let label = if opened { "open buffer" } else { "project" };
        assert_eq!(
            token_listing(&result, &source, &types, &modifiers),
            expected,
            "ST-23 {label}: token stream of t.sv"
        );
    }
    client.shutdown();
}
