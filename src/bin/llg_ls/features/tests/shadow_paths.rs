//! Shadow paths.

use super::*;

#[test]
fn shadow_path_round_trips_absolute_paths() {
    let base = process_shadow_base();
    for real in [
        "/repo/rtl/top.sv",
        "/tmp/proj/sub dir/top.sv",
        "/a/b/c/d.sv",
        "/workspaces/llg/src/bin/llg/features.rs",
    ] {
        let shadow = shadow_path(Path::new(real), &base);
        assert!(
            shadow.starts_with(&base),
            "shadow not under the tree: {shadow:?}"
        );
        assert_eq!(
            real_path(&shadow, &base),
            Some(PathBuf::from(real)),
            "round-trip failed for {real}"
        );
    }
}

#[test]
fn real_path_rejects_paths_outside_shadow_tree() {
    let base = process_shadow_base();
    assert_eq!(real_path(Path::new("/repo/rtl/top.sv"), &base), None);
    assert_eq!(real_path(Path::new("/other/x.sv"), &base), None);
    // The shadow tree root itself has no real path.
    assert_eq!(real_path(&base, &base), None);
}

/// Full compile of a design staged at its deterministic shadow path: the
/// analysis must be keyed by the shadow path (model, tokens, lint).
#[test]
fn analyze_full_pipeline_compiles_shadow_path() {
    let _guards = analysis_guards();
    let dir = std::env::temp_dir().join(format!("llg_llg_bin_shadow_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let orig_cwd = std::env::current_dir().expect("current dir");
    let _restore = TempDirGuard {
        dir: dir.clone(),
        orig: orig_cwd,
    };
    std::env::set_current_dir(&dir).expect("chdir to temp dir");

    // Stage an unsaved buffer: write the *shadow* copy only; the real file
    // exists on disk too (as in a workspace) but the compile must read the
    // shadow copy.
    let real = dir.join("rtl").join("top.sv");
    std::fs::create_dir_all(real.parent().expect("parent dir")).expect("create rtl dir");
    std::fs::write(&real, "module top; endmodule\n").expect("write real file");
    let shadow = shadow_path(&real, &dir);
    std::fs::create_dir_all(shadow.parent().expect("shadow parent")).expect("create shadow dir");
    std::fs::write(&shadow, "module top; logic unused_sig; endmodule\n")
        .expect("write shadow file");

    let shadow_str = shadow.to_string_lossy().into_owned();
    let opts = CompileOpts {
        files: vec![shadow_str.clone()],
        top: None,
        ..Default::default()
    };
    let a = analyze(&opts);
    assert!(
        !a.diagnostics.iter().any(|d| matches!(
            d.severity,
            Severity::Fatal | Severity::Syntax | Severity::Error
        )),
        "unexpected diagnostics: {:?}",
        a.diagnostics
    );
    assert!(
        a.model
            .modules
            .iter()
            .any(|m| m.file.as_deref() == Some(shadow_str.as_str())),
        "module files: {:?}",
        a.model
            .modules
            .iter()
            .map(|m| m.file.clone())
            .collect::<Vec<_>>()
    );
    assert!(
        a.tokens.iter().any(|ft| ft.path == shadow_str),
        "token files: {:?}",
        a.tokens
            .iter()
            .map(|ft| ft.path.clone())
            .collect::<Vec<_>>()
    );
    // The lint finding (unused signal) is keyed by the shadow path too.
    let map = lsp_diagnostics(&a);
    let diags = map
        .iter()
        .find(|(f, _)| *f == &shadow_str)
        .map(|(_, v)| v)
        .expect("diagnostics for the shadow path");
    assert!(
        diags.iter().any(|d| {
            d.source.as_deref() == Some("llg-lint") && d.message.contains("unused_sig")
        }),
        "unused-signal lint missing: {diags:?}"
    );
}
