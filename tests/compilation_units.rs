//! Regression coverage for explicit merged and separate compilation-unit models.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use llg::core::compile::{self, CompilationUnitMode, CompileOpts, OwnedSource};
use llg::ffi::slang::{LanguageEdition, Limits};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn compile_sources(
    sources: Vec<OwnedSource>,
    edition: LanguageEdition,
    mode: CompilationUnitMode,
) -> compile::CompileOut {
    compile::compile_sources(
        &sources,
        &CompileOpts {
            edition,
            compilation_unit_mode: mode,
            ..CompileOpts::default()
        },
    )
    .expect("admitted source buffers should reach Slang")
}

#[test]
fn macros_cross_files_only_in_explicit_merged_mode_for_both_editions() {
    for edition in [
        LanguageEdition::Verilog2001,
        LanguageEdition::SystemVerilog2009,
    ] {
        let sources = vec![
            OwnedSource::compilation_unit("a.sv", "`define SHARED_WIDTH 2\nmodule a; endmodule\n"),
            OwnedSource::compilation_unit(
                "b.sv",
                "module b; wire [`SHARED_WIDTH-1:0] data; endmodule\n",
            ),
        ];
        let merged = compile_sources(sources.clone(), edition, CompilationUnitMode::Merged);
        assert!(
            merged.ok(),
            "merged {edition:?} diagnostics: {:?}",
            merged.diagnostics
        );
        assert_eq!(
            merged.snapshot.compilation_unit_mode,
            CompilationUnitMode::Merged
        );

        let separate = compile_sources(sources, edition, CompilationUnitMode::Separate);
        assert!(
            !separate.ok(),
            "separate {edition:?} unexpectedly saw file A's macro"
        );
        assert_eq!(
            separate.snapshot.compilation_unit_mode,
            CompilationUnitMode::Separate
        );
        assert!(separate
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.file.as_deref() == Some("b.sv")));
    }
}

#[test]
fn merged_mode_preserves_unit_scope_and_declarations_spanning_buffers() {
    let sources = vec![
        OwnedSource::compilation_unit(
            "unit_a.sv",
            "int shared_unit_value = 7;\nmodule a; endmodule\n",
        ),
        OwnedSource::compilation_unit(
            "unit_b.sv",
            "module b; initial if (shared_unit_value !== 7) $error; endmodule\n",
        ),
    ];
    let merged = compile_sources(
        sources.clone(),
        LanguageEdition::SystemVerilog2009,
        CompilationUnitMode::Merged,
    );
    assert!(
        merged.ok(),
        "merged $unit diagnostics: {:?}",
        merged.diagnostics
    );

    let separate = compile_sources(
        sources,
        LanguageEdition::SystemVerilog2009,
        CompilationUnitMode::Separate,
    );
    assert!(
        !separate.ok(),
        "separate mode unexpectedly shared $unit scope"
    );
    assert!(separate
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.file.as_deref() == Some("unit_b.sv")));

    let split_declaration = vec![
        OwnedSource::compilation_unit("split_a.sv", "module split;\n"),
        OwnedSource::compilation_unit("split_b.sv", "endmodule\n"),
    ];
    let merged = compile_sources(
        split_declaration.clone(),
        LanguageEdition::SystemVerilog2009,
        CompilationUnitMode::Merged,
    );
    assert!(
        merged.ok(),
        "merged declaration diagnostics: {:?}",
        merged.diagnostics
    );
    let separate = compile_sources(
        split_declaration,
        LanguageEdition::SystemVerilog2009,
        CompilationUnitMode::Separate,
    );
    assert!(
        !separate.ok(),
        "separate mode accepted a declaration split across units"
    );
}

#[test]
fn includes_keep_parent_ownership_and_packages_keep_diagnostic_files() {
    let sources = vec![
        OwnedSource::compilation_unit(
            "src/parent.sv",
            "module parent; `include \"header.svh\" endmodule\n",
        ),
        OwnedSource::include("src/header.svh", "wire included;\n"),
    ];
    for mode in [CompilationUnitMode::Separate, CompilationUnitMode::Merged] {
        let out = compile_sources(sources.clone(), LanguageEdition::SystemVerilog2009, mode);
        assert!(
            out.ok(),
            "include mode {mode:?} diagnostics: {:?}",
            out.diagnostics
        );
        assert_eq!(out.snapshot.files.len(), 2);
        assert_eq!(out.snapshot.files[0].name, "src/parent.sv");
        assert_eq!(out.snapshot.files[1].name, "src/header.svh");
    }

    let package_sources = vec![
        OwnedSource::compilation_unit("pkg.sv", "package p; typedef int value_t; endpackage\n"),
        OwnedSource::compilation_unit(
            "use.sv",
            "module consumer; import p::*; value_t value; endmodule\n",
        ),
    ];
    for mode in [CompilationUnitMode::Separate, CompilationUnitMode::Merged] {
        let out = compile_sources(
            package_sources.clone(),
            LanguageEdition::SystemVerilog2009,
            mode,
        );
        assert!(
            out.ok(),
            "package mode {mode:?} diagnostics: {:?}",
            out.diagnostics
        );
    }

    let missing_package = vec![OwnedSource::compilation_unit(
        "package_use.sv",
        "module consumer; import missing::*; endmodule\n",
    )];
    let mut diagnostic_files = Vec::new();
    for mode in [CompilationUnitMode::Separate, CompilationUnitMode::Merged] {
        let out = compile_sources(
            missing_package.clone(),
            LanguageEdition::SystemVerilog2009,
            mode,
        );
        let diagnostic = out
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.file.is_some())
            .expect("missing package diagnostic");
        diagnostic_files.push(diagnostic.file.clone());
    }
    assert_eq!(diagnostic_files, vec![Some("package_use.sv".into()); 2]);
}

#[test]
fn macro_expanded_in_memory_includes_use_only_admitted_buffers() {
    let admitted = vec![
        OwnedSource::compilation_unit(
            "/virtual/project/top.sv",
            "`define HEADER \"header.svh\"\n`include `HEADER\nmodule top; endmodule\n",
        ),
        OwnedSource::include("/virtual/project/header.svh", "`define HEADER_VALUE 7\n"),
    ];
    for mode in [CompilationUnitMode::Separate, CompilationUnitMode::Merged] {
        let out = compile_sources(admitted.clone(), LanguageEdition::SystemVerilog2009, mode);
        assert!(
            out.ok(),
            "admitted macro include {mode:?}: {:?}",
            out.diagnostics
        );
    }

    let unauthorized = vec![OwnedSource::compilation_unit(
        "/virtual/project/unauthorized.sv",
        "`define HEADER \"/etc/passwd\"\n`include `HEADER\nmodule unauthorized; endmodule\n",
    )];
    let out = compile_sources(
        unauthorized,
        LanguageEdition::SystemVerilog2009,
        CompilationUnitMode::Merged,
    );
    assert!(!out.ok());
    assert_eq!(out.snapshot.files.len(), 1);
    assert!(!out
        .snapshot
        .files
        .iter()
        .any(|file| file.name == "/etc/passwd"));
}

fn p52_temp_dir() -> PathBuf {
    let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "llg-p52-admission-{}-{sequence}",
        std::process::id()
    ))
}

fn write_p52_fixture(root: &Path) {
    fs::create_dir_all(root.join("include")).expect("create P52 include root");
    fs::write(
        root.join("include/bootstrap.svh"),
        "`define HEADER_NAME header.svh\n",
    )
    .expect("write macro bootstrap header");
    fs::write(
        root.join("include/header.svh"),
        "`define INCLUDED_VALUE 37\n",
    )
    .expect("write macro include header");
    fs::write(
        root.join("a.sv"),
        r#"`include "bootstrap.svh"
`define QUOTE(value) `"value`"
`define SHARED_HEADER `QUOTE(`HEADER_NAME)
`include `SHARED_HEADER
module a; wire [`INCLUDED_VALUE-1:0] data; endmodule
"#,
    )
    .expect("write first P52 source");
    fs::write(
        root.join("b.sv"),
        "`include `SHARED_HEADER\nmodule b; endmodule\n",
    )
    .expect("write second P52 source");
}

fn compile_p52_files(
    root: &Path,
    files: &[&str],
    edition: LanguageEdition,
    mode: CompilationUnitMode,
    defines: &[&str],
) -> Result<compile::CompileOut, compile::StartupError> {
    compile::compile(&CompileOpts {
        files: files
            .iter()
            .map(|file| root.join(file).to_string_lossy().into())
            .collect(),
        include_dirs: vec![root.join("include").to_string_lossy().into()],
        edition,
        compilation_unit_mode: mode,
        defines: defines.iter().map(|define| (*define).to_owned()).collect(),
        ..CompileOpts::default()
    })
}

#[test]
fn macro_expanded_includes_follow_unit_mode_and_edition() {
    let root = p52_temp_dir();
    write_p52_fixture(&root);
    for edition in [
        LanguageEdition::Verilog2001,
        LanguageEdition::SystemVerilog2009,
    ] {
        let merged = compile_p52_files(
            &root,
            &["a.sv", "b.sv"],
            edition,
            CompilationUnitMode::Merged,
            &[],
        )
        .expect("merged macro-expanded include admission");
        assert!(
            merged.ok(),
            "merged {edition:?} diagnostics: {:?}",
            merged.diagnostics
        );
        assert!(merged
            .snapshot
            .files
            .iter()
            .any(|file| file.name.ends_with("include/header.svh")));

        let separate_root = compile_p52_files(
            &root,
            &["a.sv"],
            edition,
            CompilationUnitMode::Separate,
            &[],
        )
        .expect("separate root-local macro-expanded include admission");
        assert!(
            separate_root.ok(),
            "separate {edition:?} diagnostics: {:?}",
            separate_root.diagnostics
        );

        let separate = compile_p52_files(
            &root,
            &["a.sv", "b.sv"],
            edition,
            CompilationUnitMode::Separate,
            &[],
        )
        .expect("separate macro visibility should reach Slang diagnostics");
        assert!(
            !separate.ok(),
            "separate {edition:?} unexpectedly shared the filename macro",
        );
        assert!(separate.diagnostics.iter().any(|diagnostic| diagnostic
            .file
            .as_deref()
            .is_some_and(|file| file.ends_with("b.sv"))));
    }
    fs::remove_dir_all(root).expect("remove P52 fixture");
}

#[test]
fn conditional_macro_include_redefinition_and_unauthorized_paths_fail_closed() {
    let root = p52_temp_dir();
    fs::create_dir_all(root.join("include")).expect("create P52 conditional include root");
    fs::write(
        root.join("include/first.svh"),
        "`define SELECTED_VALUE 11\n",
    )
    .expect("write first conditional header");
    fs::write(
        root.join("include/second.svh"),
        "`define SELECTED_VALUE 22\n",
    )
    .expect("write second conditional header");
    fs::write(
        root.join("conditional.sv"),
        r#"`define SELECTED_HEADER "first.svh"
`ifdef USE_SECOND
`undef SELECTED_HEADER
`define SELECTED_HEADER "second.svh"
`endif
`include `SELECTED_HEADER
module conditional #(parameter int VALUE = `SELECTED_VALUE); endmodule
"#,
    )
    .expect("write conditional source");

    for mode in [CompilationUnitMode::Separate, CompilationUnitMode::Merged] {
        let selected = compile_p52_files(
            &root,
            &["conditional.sv"],
            LanguageEdition::SystemVerilog2009,
            mode,
            &["USE_SECOND"],
        )
        .expect("conditional macro include admission");
        assert!(
            selected.ok(),
            "selected header diagnostics: {:?}",
            selected.diagnostics
        );

        let first = compile_p52_files(
            &root,
            &["conditional.sv"],
            LanguageEdition::SystemVerilog2009,
            mode,
            &[],
        )
        .expect("conditional first header admission");
        assert!(
            first.ok(),
            "first header diagnostics: {:?}",
            first.diagnostics
        );
    }

    fs::write(
        root.join("missing.sv"),
        "`define MISSING_HEADER \"does-not-exist.svh\"\n`include `MISSING_HEADER\nmodule missing; endmodule\n",
    )
    .expect("write missing include source");
    let missing = compile_p52_files(
        &root,
        &["missing.sv"],
        LanguageEdition::SystemVerilog2009,
        CompilationUnitMode::Separate,
        &[],
    )
    .expect("missing include should remain a frontend diagnostic");
    assert!(!missing.ok());
    assert!(missing.snapshot.diagnostics.iter().any(|diagnostic| {
        diagnostic.name == "CouldNotOpenIncludeFile"
            && diagnostic.primary.is_some_and(|range| {
                missing
                    .snapshot
                    .files
                    .iter()
                    .any(|file| file.id == range.file_id && file.name.ends_with("missing.sv"))
            })
    }));

    fs::write(
        root.join("unauthorized.sv"),
        "`define UNAUTHORIZED_HEADER \"/etc/passwd\"\n`include `UNAUTHORIZED_HEADER\nmodule unauthorized; endmodule\n",
    )
    .expect("write unauthorized include source");
    let unauthorized = compile_p52_files(
        &root,
        &["unauthorized.sv"],
        LanguageEdition::SystemVerilog2009,
        CompilationUnitMode::Merged,
        &[],
    )
    .expect("unauthorized include should remain a frontend diagnostic");
    assert!(!unauthorized.ok());
    assert!(!unauthorized
        .snapshot
        .files
        .iter()
        .any(|file| file.name == "/etc/passwd"));
    fs::remove_dir_all(root).expect("remove P52 conditional fixture");
}

#[test]
fn command_line_define_admits_a_macro_include_from_an_explicit_root() {
    let root = p52_temp_dir();
    fs::create_dir_all(root.join("include")).expect("create P52 command-line include root");
    fs::write(
        root.join("include/selected.svh"),
        "`define SELECTED_WIDTH 9\n",
    )
    .expect("write command-line selected header");
    fs::write(
        root.join("top.sv"),
        "`include `SELECTED_HEADER\nmodule top; wire [`SELECTED_WIDTH-1:0] data; endmodule\n",
    )
    .expect("write command-line source");

    for mode in [CompilationUnitMode::Separate, CompilationUnitMode::Merged] {
        let out = compile_p52_files(
            &root,
            &["top.sv"],
            LanguageEdition::SystemVerilog2009,
            mode,
            &[r#"SELECTED_HEADER="selected.svh""#],
        )
        .expect("command-line macro include admission");
        assert!(
            out.ok(),
            "command-line macro include {mode:?} diagnostics: {:?}",
            out.diagnostics
        );
        assert!(out
            .snapshot
            .files
            .iter()
            .any(|file| file.name.ends_with("include/selected.svh")));
    }
    fs::remove_dir_all(root).expect("remove P52 command-line fixture");
}

#[test]
fn include_cycles_are_admitted_once_and_left_to_frontend_diagnostics() {
    let root = p52_temp_dir();
    fs::create_dir_all(root.join("include")).expect("create P52 cycle include root");
    fs::write(root.join("include/a.svh"), "`include \"b.svh\"\n")
        .expect("write first cycle header");
    fs::write(root.join("include/b.svh"), "`include \"a.svh\"\n")
        .expect("write second cycle header");
    fs::write(
        root.join("top.sv"),
        "`include \"a.svh\"\nmodule top; endmodule\n",
    )
    .expect("write cycle source");

    let out = compile_p52_files(
        &root,
        &["top.sv"],
        LanguageEdition::SystemVerilog2009,
        CompilationUnitMode::Separate,
        &[],
    )
    .expect("include cycle should reach Slang for diagnostics");
    assert!(!out.ok(), "include cycle unexpectedly compiled cleanly");
    assert_eq!(out.snapshot.files.len(), 3);
    assert!(out
        .snapshot
        .files
        .iter()
        .any(|file| file.name.ends_with("include/a.svh")));
    assert!(out
        .snapshot
        .files
        .iter()
        .any(|file| file.name.ends_with("include/b.svh")));
    fs::remove_dir_all(root).expect("remove P52 cycle fixture");
}

#[test]
fn include_admission_enforces_the_shared_source_byte_budget() {
    let root = p52_temp_dir();
    fs::create_dir_all(root.join("include")).expect("create P52 limit include root");
    fs::write(root.join("include/header.svh"), "`define WIDTH 8\n").expect("write limited header");
    fs::write(
        root.join("top.sv"),
        "`include \"header.svh\"\nmodule top; wire [`WIDTH-1:0] data; endmodule\n",
    )
    .expect("write limited source");
    let source = fs::canonicalize(root.join("top.sv")).expect("canonical source path");
    let header = fs::canonicalize(root.join("include/header.svh")).expect("canonical header path");
    let required = source.to_string_lossy().len() as u64
        + fs::read_to_string(&source)
            .expect("read limited source")
            .len() as u64
        + header.to_string_lossy().len() as u64
        + fs::read_to_string(&header)
            .expect("read limited header")
            .len() as u64;
    let out = compile::compile(&CompileOpts {
        files: vec![source.to_string_lossy().into_owned()],
        include_dirs: vec![root.join("include").to_string_lossy().into_owned()],
        limits: Limits {
            max_source_bytes: required - 1,
            ..Limits::default()
        },
        ..CompileOpts::default()
    });
    let error = out.expect_err("include admission must enforce source bytes");
    assert_eq!(error.kind(), compile::StartupErrorKind::LimitExceeded);
    assert!(error.message().contains("source") || error.message().contains("include"));
    fs::remove_dir_all(root).expect("remove P52 limit fixture");
}
