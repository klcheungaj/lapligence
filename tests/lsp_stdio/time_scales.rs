//! Mixed time scales: the ELAB-006 error `llg` stops on, over the wire.

use super::*;

const SUB: &str = concat!(
    "// llg-lsp-fixture: generated/time-scales/sub.sv\n",
    "module sub;\n",
    "endmodule\n",
);

const TIMED_TB: &str = concat!(
    "// llg-lsp-fixture: generated/time-scales/tb.sv\n",
    "`timescale 1ns/1ps\n",
    "module tb;\n",
    "  sub u();\n",
    "endmodule\n",
);

const UNTIMED_TB: &str = concat!(
    "// llg-lsp-fixture: generated/time-scales/tb.sv\n",
    "module tb;\n",
    "  sub u();\n",
    "endmodule\n",
);

const CONFIG: &str = "schema_version = 1\n[sources]\ndirectories = [\".\"]\n";

fn time_scale_workspace(label: &str, tb: &str) -> (TempDirCleanup, PathBuf) {
    let id = NEXT_FIXTURE_ID.fetch_add(1, Ordering::Relaxed);
    let base = std::env::temp_dir().join(format!(
        "llg-lsp-time-scales-{label}-{}-{id}",
        std::process::id()
    ));
    let root = base.join("workspace");
    fs::create_dir_all(&root).expect("create time-scale workspace");
    fs::write(root.join(CONFIG_FILE), CONFIG).expect("write time-scale config");
    fs::write(root.join("sub.sv"), SUB).expect("write sub.sv");
    fs::write(root.join("tb.sv"), tb).expect("write tb.sv");
    (TempDirCleanup(base), root)
}

fn diagnostics_with_code<'a>(params: &'a Value, code: &str) -> Vec<&'a Value> {
    params
        .get("diagnostics")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|diagnostic| diagnostic.get("code").and_then(Value::as_str) == Some(code))
        .collect()
}

/// A design in which only `tb` has a time scale publishes one ELAB-006 error
/// on `sub`'s name with `tb`'s name as related information; giving
/// `compile.timescale` in the watched `llg.toml` clears it, and with it the
/// `missing-timescale` warning.
#[test]
fn lsp_stdio_mixed_time_scales_are_an_error_until_compile_timescale_is_set() {
    let (_cleanup, root) = time_scale_workspace("mixed", TIMED_TB);
    let sub_path = root.join("sub.sv");
    let tb_path = root.join("tb.sv");
    let (sub_uri, tb_uri) = (file_uri(&sub_path), file_uri(&tb_path));

    let mut client = LspProcess::spawn(&root);
    client
        .initialize(&[("time-scales", &root)], default_init_options())
        .expect("initialize time-scale workspace");
    client.open(&sub_path, SUB).expect("open sub.sv");
    let published = wait_for_diagnostics(&mut client, &sub_uri, |params| {
        !diagnostics_with_code(params, "ELAB-006").is_empty()
    });
    assert_no_shadow_uris(&published);
    let errors = diagnostics_with_code(&published, "ELAB-006");
    assert_eq!(errors.len(), 1, "{published}");
    let error = errors[0];
    assert_eq!(error.get("severity").and_then(Value::as_u64), Some(1));
    assert_eq!(error.get("source").and_then(Value::as_str), Some("llg"));
    assert_eq!(
        error.pointer("/range/start"),
        Some(&json!({ "line": 1, "character": 7 }))
    );
    assert_eq!(
        error.get("message").and_then(Value::as_str),
        Some(
            "module `sub` has no `timescale, timeunit or timeprecision, but module `tb` has a \
             time scale; a design in which only some elements have one needs a default time \
             scale (`compile.timescale` in llg.toml) (IEEE 1800-2009 3.14.2.3)"
        )
    );
    let related = error
        .get("relatedInformation")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("ELAB-006 without related information: {error}"));
    assert_eq!(related.len(), 1, "{error}");
    assert_eq!(
        related[0].get("message").and_then(Value::as_str),
        Some("module `tb` has a time scale")
    );
    assert_eq!(
        related[0].pointer("/location/uri").and_then(Value::as_str),
        Some(tb_uri.as_str())
    );
    assert_eq!(
        related[0].pointer("/location/range/start"),
        Some(&json!({ "line": 2, "character": 7 }))
    );

    let config_path = root.join(CONFIG_FILE);
    fs::write(
        &config_path,
        format!("{CONFIG}[compile]\ntimescale = \"1ns/1ps\"\n"),
    )
    .expect("write compile.timescale");
    client
        .send_watch_event(&config_path, 2)
        .expect("send config watch event");
    let cleared = wait_for_diagnostics(&mut client, &sub_uri, |params| {
        diagnostics_with_code(params, "ELAB-006").is_empty()
    });
    assert!(
        !has_lint_rule(&cleared, "missing-timescale"),
        "an explicit default is not reported: {cleared}"
    );
    client.shutdown();
}

/// A design without any time scale is legal: `sub`, the first element taking
/// the default, gets the `missing-timescale` warning and nothing is an error.
#[test]
fn lsp_stdio_design_without_time_scales_warns_only() {
    let (_cleanup, root) = time_scale_workspace("none", UNTIMED_TB);
    let sub_path = root.join("sub.sv");
    let sub_uri = file_uri(&sub_path);

    let mut client = LspProcess::spawn(&root);
    client
        .initialize(&[("time-scales", &root)], default_init_options())
        .expect("initialize time-scale workspace");
    client.open(&sub_path, SUB).expect("open sub.sv");
    let published = wait_for_diagnostics(&mut client, &sub_uri, |params| {
        has_lint_rule(params, "missing-timescale")
    });
    assert_eq!(lint_severity(&published, "missing-timescale"), Some(2));
    assert!(
        diagnostics_with_code(&published, "ELAB-006").is_empty(),
        "{published}"
    );
    assert!(has_no_severity_1(&published), "{published}");
    client.shutdown();
}
