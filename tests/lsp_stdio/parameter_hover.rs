//! Parameter hover.

use super::*;

// ── Parameter hover: elaborated values from the committed analysis ───────────
//
// Contract:
// * hovering a parameter/localparam DECLARATION — or any binding-precise
//   REFERENCE to it, including uses inside the instance body — appends a
//   short `value = <const>` line rendered ONLY from the committed analysis
//   model (`InstanceModel.params` / gen-scope / package parameters).  No
//   parsing or elaboration happens in the hover request path; unresolved
//   values omit the line silently.
// * `[compile.param_overrides]` values show up wherever the
//   overridden value is committed — declaration and in-instance use sites
//   alike.
// * identical repeat hovers return byte-identical responses served by the
//   request cache: the `# request-cache:` hits counter grows while misses
//   stay put (no recompute).

/// Single-root generated workspace with a `WIDTH=8` override applied to its
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
pub(super) fn param_hover_markup(client: &mut LspProcess, uri: &str, pos: &Value) -> String {
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
pub(super) fn value_line(markup: &str) -> Option<&str> {
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
