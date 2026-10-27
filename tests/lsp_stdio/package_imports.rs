//! Package items used through imports and scope qualifiers, in both serving
//! paths.
//
// Contract:
// * a package item used through a wildcard import at compilation-unit
//   scope, in a module header or in a module body, through an explicit
//   import item or through `pkg::name` has its own kind's token (parameter
//   and localparam `variable.readonly`, typedef `type`, enum item
//   `enumMember.readonly`, function `function`), and definition, hover and
//   references reach its declaration in the package, also when the package
//   is in another project file or exports an item of a third package.
// * the names of an import item are references, never declarations: the
//   package is `namespace` and the member has the kind of what it imports.
// * the project stream (an unopened file) and the isolated open-buffer parse
//   agree token for token; an unsaved edit resolves package items through
//   the package units of the committed analysis.

use super::*;

const PKG_TOML: &str = "\
schema_version = 1

[sources]
directories = [\".\"]
include = [\"**/*.v\", \"**/*.sv\"]

[lint]
enabled = false
";

const BASE_PKG: &str = "\
package base_pkg;
  localparam int B_V = 1;
endpackage
";

const CFG_PKG: &str = "\
package cfg_pkg;
  import base_pkg::*;
  export base_pkg::B_V;
  parameter int P_W = 8;
  localparam int L_D = 4;
  typedef logic [P_W-1:0] word_t;
  typedef enum logic [1:0] { ST_IDLE, ST_RUN } state_t;
  typedef struct packed { logic a; logic [3:0] b; } rec_t;
  function automatic int f_inc(int v);
    return v + B_V;
  endfunction
endpackage
";

const CU_USER: &str = "\
import cfg_pkg::*;
module cu_user #(parameter int W = P_W) (input word_t a, output logic [L_D-1:0] y);
  state_t st;
  rec_t r;
  assign y = f_inc(L_D) + W + a + r.b;
  initial st = ST_RUN;
endmodule
";

const USERS: &str = "\
module hdr_user import cfg_pkg::*; #(parameter int W = P_W) (input word_t a, output logic [L_D-1:0] y);
  state_t st;
  assign y = f_inc(L_D) + W + a;
  initial st = ST_IDLE;
endmodule
module body_user (output logic [7:0] y);
  import cfg_pkg::*;
  localparam int B = P_W + L_D + B_V;
  word_t w;
  rec_t r;
  assign y = w + B + f_inc(1) + r.b;
endmodule
module explicit_user (output logic [7:0] y);
  import cfg_pkg::P_W;
  import cfg_pkg::word_t;
  import cfg_pkg::ST_RUN;
  import cfg_pkg::f_inc;
  word_t w;
  assign y = w + P_W + f_inc(ST_RUN);
endmodule
module qual_user (input cfg_pkg::word_t a, output logic [cfg_pkg::L_D-1:0] y);
  cfg_pkg::state_t st;
  assign y = cfg_pkg::f_inc(cfg_pkg::P_W) + a;
  initial st = cfg_pkg::ST_IDLE;
endmodule
";

const SAME_FILE: &str = "\
package same_pkg;
  parameter int S_W = 3;
  localparam int S_L = 2;
  typedef logic [S_W-1:0] s_t;
endpackage
module same_user import same_pkg::*; #(parameter int W = S_W) (input s_t a, output logic [S_L-1:0] y);
  assign y = a + W + same_pkg::S_L;
endmodule
";

struct PkgWorkspace {
    _guard: TempDirCleanup,
    ws: PathBuf,
    base: PathBuf,
    cfg: PathBuf,
    cu: PathBuf,
    users: PathBuf,
    same: PathBuf,
}

fn pkg_workspace(tag: &str) -> PkgWorkspace {
    let base_dir =
        std::env::temp_dir().join(format!("llg-lsp-pkg-import-{}-{tag}", std::process::id()));
    let ws = base_dir.join("ws");
    fs::create_dir_all(&ws).expect("create package workspace");
    fs::write(ws.join(CONFIG_FILE), PKG_TOML).expect("write llg.toml");
    let write = |name: &str, text: &str| {
        let path = ws.join(name);
        fs::write(&path, text).unwrap_or_else(|error| panic!("write {name}: {error}"));
        path
    };
    PkgWorkspace {
        base: write("base_pkg.sv", BASE_PKG),
        cfg: write("cfg_pkg.sv", CFG_PKG),
        cu: write("cu_user.sv", CU_USER),
        users: write("users.sv", USERS),
        same: write("same.sv", SAME_FILE),
        _guard: TempDirCleanup(base_dir),
        ws,
    }
}

/// Where a use must resolve: a declaration needle (plus column offset) in
/// one of the workspace files.
#[derive(Clone, Copy)]
enum Target {
    Cfg(&'static str, u64),
    Base(&'static str, u64),
    Same(&'static str, u64),
}

/// One use: needle, occurrence, column offset, token type, modifiers and
/// declaration.
type Use = (
    &'static str,
    usize,
    u64,
    &'static str,
    &'static [&'static str],
    Target,
);

const RO: &[&str] = &["readonly"];
const NONE: &[&str] = &[];

const PKG: Target = Target::Cfg("package cfg_pkg", 8);
const P_W: Target = Target::Cfg("P_W = 8", 0);
const L_D: Target = Target::Cfg("L_D = 4", 0);
const WORD: Target = Target::Cfg("word_t;", 0);
const STATE: Target = Target::Cfg("state_t;", 0);
const REC: Target = Target::Cfg("rec_t;", 0);
const F_INC: Target = Target::Cfg("f_inc(int", 0);
const IDLE: Target = Target::Cfg("ST_IDLE,", 0);
const RUN: Target = Target::Cfg("ST_RUN }", 0);

const CU_USES: &[Use] = &[
    ("cfg_pkg::*", 0, 0, "namespace", NONE, PKG),
    ("P_W)", 0, 0, "variable", RO, P_W),
    ("word_t a", 0, 0, "type", NONE, WORD),
    ("L_D-1", 0, 0, "variable", RO, L_D),
    ("state_t st", 0, 0, "type", NONE, STATE),
    ("rec_t r", 0, 0, "type", NONE, REC),
    ("f_inc(L_D)", 0, 0, "function", NONE, F_INC),
    ("f_inc(L_D)", 0, 6, "variable", RO, L_D),
    ("ST_RUN;", 0, 0, "enumMember", RO, RUN),
];

const USERS_USES: &[Use] = &[
    // Header import, used in the parameter and port lists and the body.
    ("cfg_pkg::*", 0, 0, "namespace", NONE, PKG),
    ("P_W)", 0, 0, "variable", RO, P_W),
    ("word_t a", 0, 0, "type", NONE, WORD),
    ("L_D-1", 0, 0, "variable", RO, L_D),
    ("state_t st", 0, 0, "type", NONE, STATE),
    ("f_inc(L_D)", 0, 0, "function", NONE, F_INC),
    ("f_inc(L_D)", 0, 6, "variable", RO, L_D),
    ("ST_IDLE;", 0, 0, "enumMember", RO, IDLE),
    // Body import, also of an item cfg_pkg exports from base_pkg.
    ("cfg_pkg::*", 1, 0, "namespace", NONE, PKG),
    ("P_W + L_D", 0, 0, "variable", RO, P_W),
    ("L_D + B_V", 0, 0, "variable", RO, L_D),
    ("B_V;", 0, 0, "variable", RO, Target::Base("B_V = 1", 0)),
    ("word_t w", 0, 0, "type", NONE, WORD),
    ("rec_t r", 0, 0, "type", NONE, REC),
    ("f_inc(1)", 0, 0, "function", NONE, F_INC),
    // Explicit import items and their uses.
    ("cfg_pkg::P_W;", 0, 0, "namespace", NONE, PKG),
    ("cfg_pkg::P_W;", 0, 9, "variable", RO, P_W),
    ("cfg_pkg::word_t;", 0, 9, "type", NONE, WORD),
    ("cfg_pkg::ST_RUN;", 0, 9, "enumMember", RO, RUN),
    ("cfg_pkg::f_inc;", 0, 0, "namespace", NONE, PKG),
    ("cfg_pkg::f_inc;", 0, 9, "function", NONE, F_INC),
    ("word_t w", 1, 0, "type", NONE, WORD),
    ("P_W + f_inc", 0, 0, "variable", RO, P_W),
    ("f_inc(ST_RUN)", 0, 0, "function", NONE, F_INC),
    ("f_inc(ST_RUN)", 0, 6, "enumMember", RO, RUN),
    // Scope-qualified uses in ports and the body.
    ("cfg_pkg::word_t a", 0, 0, "namespace", NONE, PKG),
    ("cfg_pkg::word_t a", 0, 9, "type", NONE, WORD),
    ("cfg_pkg::L_D-1", 0, 9, "variable", RO, L_D),
    ("cfg_pkg::state_t", 0, 9, "type", NONE, STATE),
    ("cfg_pkg::f_inc(", 0, 9, "function", NONE, F_INC),
    ("cfg_pkg::P_W)", 0, 0, "namespace", NONE, PKG),
    ("cfg_pkg::P_W)", 0, 9, "variable", RO, P_W),
    ("cfg_pkg::ST_IDLE", 0, 9, "enumMember", RO, IDLE),
];

const SAME_USES: &[Use] = &[
    (
        "same_pkg::*",
        0,
        0,
        "namespace",
        NONE,
        Target::Same("package same_pkg", 8),
    ),
    ("S_W)", 0, 0, "variable", RO, Target::Same("S_W = 3", 0)),
    ("s_t a", 0, 0, "type", NONE, Target::Same("s_t;", 0)),
    ("S_L-1", 0, 0, "variable", RO, Target::Same("S_L = 2", 0)),
    (
        "same_pkg::S_L",
        0,
        10,
        "variable",
        RO,
        Target::Same("S_L = 2", 0),
    ),
];

fn rows_for(
    client: &mut LspProcess,
    path: &Path,
    types: &[String],
    modifiers: &[String],
) -> Vec<SemanticRow> {
    let result = client
        .request(
            "textDocument/semanticTokens/full",
            json!({ "textDocument": { "uri": file_uri(path) } }),
        )
        .expect("semantic token response");
    semantic_token_rows(&result, types, modifiers)
}

fn row_tuples(rows: &[SemanticRow]) -> Vec<(u64, u64, String, Vec<String>)> {
    rows.iter()
        .map(|row| {
            (
                row.line,
                row.character,
                row.token_type.clone(),
                row.modifiers.clone(),
            )
        })
        .collect()
}

fn lsp_position(source: &str, needle: &str, occurrence: usize, offset: u64) -> Value {
    let (line, character) = position_of(source, needle, occurrence);
    json!({ "line": line, "character": character + offset })
}

/// Tokens, definition, hover and references of every use in `uses`.
fn check_uses(
    client: &mut LspProcess,
    workspace: &PkgWorkspace,
    path: &Path,
    source: &str,
    uses: &[Use],
    rows: &[SemanticRow],
    opened: bool,
) {
    let uri = file_uri(path);
    for &(needle, occurrence, offset, token_type, token_modifiers, target) in uses {
        let what = format!("opened={opened} {needle}#{occurrence}+{offset}");
        let (line, character) = position_of(source, needle, occurrence);
        let row = row_at(rows, line, character + offset);
        assert_eq!(row.token_type, token_type, "{what}: {row:?}");
        assert_eq!(row.modifiers, token_modifiers, "{what}: {row:?}");

        let (target_path, target_source, target_needle, target_offset) = match target {
            Target::Cfg(needle, offset) => (&workspace.cfg, CFG_PKG, needle, offset),
            Target::Base(needle, offset) => (&workspace.base, BASE_PKG, needle, offset),
            Target::Same(needle, offset) => (&workspace.same, SAME_FILE, needle, offset),
        };
        let target_uri = file_uri(target_path);
        let target_start = lsp_position(target_source, target_needle, 0, target_offset);
        let position = lsp_position(source, needle, occurrence, offset);
        let definition = client
            .request(
                "textDocument/definition",
                json!({ "textDocument": { "uri": uri }, "position": position }),
            )
            .expect("definition response");
        let (definition_uri, definition_start) = single_location(&definition, &what);
        assert_eq!(
            (definition_uri.as_str(), &definition_start),
            (target_uri.as_str(), &target_start),
            "{what}: definition"
        );

        let hover = client
            .request(
                "textDocument/hover",
                json!({ "textDocument": { "uri": uri }, "position": position }),
            )
            .expect("hover response");
        let name = target_needle[target_offset as usize..]
            .split(|c: char| !(c.is_alphanumeric() || c == '_'))
            .next()
            .unwrap();
        assert!(
            hover.to_string().contains(name),
            "{what}: hover must name {name}: {hover}"
        );

        let references = client
            .request(
                "textDocument/references",
                json!({
                    "textDocument": { "uri": uri },
                    "position": position,
                    "context": { "includeDeclaration": true }
                }),
            )
            .expect("references response");
        let has = |uri: &str, start: &Value| {
            references.as_array().is_some_and(|locations| {
                locations.iter().any(|location| {
                    location["uri"].as_str() == Some(uri) && &location["range"]["start"] == start
                })
            })
        };
        assert!(
            has(&target_uri, &target_start),
            "{what}: references miss the declaration: {references}"
        );
        assert!(
            has(&uri, &position),
            "{what}: references miss the use: {references}"
        );
    }
}

/// Every import form, the explicit import item names and scope-qualified
/// uses resolve to the package declarations through tokens, definition,
/// hover and references; the project stream and the isolated open-buffer
/// parse agree token for token.
#[test]
fn lsp_stdio_package_items_resolve_in_both_serving_paths() {
    let workspace = pkg_workspace("items");
    let mut client = LspProcess::spawn(&workspace.ws);
    let initialize = client
        .initialize(&[("pkg-items", &workspace.ws)], default_init_options())
        .expect("initialize package workspace");
    let legend = &initialize["capabilities"]["semanticTokensProvider"]["legend"];
    let types = legend_names(legend, "tokenTypes");
    let modifiers = legend_names(legend, "tokenModifiers");
    for path in [&workspace.cu, &workspace.users, &workspace.same] {
        wait_for_diagnostics(&mut client, &file_uri(path), has_no_severity_1);
    }

    let files: [(&PathBuf, &str, &[Use]); 3] = [
        (&workspace.cu, CU_USER, CU_USES),
        (&workspace.users, USERS, USERS_USES),
        (&workspace.same, SAME_FILE, SAME_USES),
    ];
    for (path, source, uses) in files {
        let project = rows_for(&mut client, path, &types, &modifiers);
        check_uses(&mut client, &workspace, path, source, uses, &project, false);
        client.open(path, source).expect("open package user");
        let isolated = rows_for(&mut client, path, &types, &modifiers);
        check_uses(&mut client, &workspace, path, source, uses, &isolated, true);
        assert_eq!(
            row_tuples(&project),
            row_tuples(&isolated),
            "{}: project and open-buffer streams differ",
            path.display()
        );
    }

    // Import item names are references, not symbols of the importing module.
    let symbols = client
        .request(
            "textDocument/documentSymbol",
            json!({ "textDocument": { "uri": file_uri(&workspace.users) } }),
        )
        .expect("document symbols");
    let names = names(&symbols);
    for imported in ["P_W", "word_t", "ST_RUN", "f_inc", "cfg_pkg"] {
        assert!(
            !names.iter().any(|name| name == imported),
            "{imported} listed as a users.sv symbol: {names:?}"
        );
    }
    client.shutdown();
}

/// An unsaved edit that adds imports and qualified uses, also of a package
/// the buffer did not name before, resolves through the package units of
/// the committed analysis.
#[test]
fn lsp_stdio_unsaved_package_uses_resolve_through_the_committed_package_units() {
    let workspace = pkg_workspace("unsaved");
    let mut client = LspProcess::spawn(&workspace.ws);
    let initialize = client
        .initialize(&[("pkg-unsaved", &workspace.ws)], default_init_options())
        .expect("initialize package workspace");
    let legend = &initialize["capabilities"]["semanticTokensProvider"]["legend"];
    let types = legend_names(legend, "tokenTypes");
    let modifiers = legend_names(legend, "tokenModifiers");
    wait_for_diagnostics(&mut client, &file_uri(&workspace.same), has_no_severity_1);
    client
        .open(&workspace.same, SAME_FILE)
        .expect("open same.sv");
    let edited = format!(
        "{SAME_FILE}module late_user;\n  import cfg_pkg::L_D;\n  localparam int LU = L_D + base_pkg::B_V + cfg_pkg::P_W;\nendmodule\n"
    );
    client
        .change(&workspace.same, 2, &edited)
        .expect("edit same.sv");
    let rows = rows_for(&mut client, &workspace.same, &types, &modifiers);
    for (needle, offset, token_type, token_modifiers) in [
        ("cfg_pkg::L_D", 0, "namespace", NONE),
        ("cfg_pkg::L_D", 9, "variable", RO),
        ("L_D + base_pkg", 0, "variable", RO),
        ("base_pkg::B_V", 0, "namespace", NONE),
        ("base_pkg::B_V", 10, "variable", RO),
        ("cfg_pkg::P_W", 9, "variable", RO),
        ("S_W)", 0, "variable", RO),
    ] {
        let (line, character) = position_of(&edited, needle, 0);
        let row = row_at(&rows, line, character + offset);
        assert_eq!(row.token_type, token_type, "{needle}+{offset}: {row:?}");
        assert_eq!(row.modifiers, token_modifiers, "{needle}+{offset}: {row:?}");
    }
    client.shutdown();
}
