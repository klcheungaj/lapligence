//! Macro hover.

use super::*;

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
    // The undefined `NOWHERE makes Slang report an error; the analysis is
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
