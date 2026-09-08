#![cfg(feature = "slang")]

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use llg::core::compile::slang::{
    self, CompileError, CompileOptions, CompileRequest, ConstantValue, DiagnosticProvider,
    DiagnosticSeverity, SlangErrorKind, Source,
};

fn request<'a>(sources: &'a [Source<'a>], options: &'a CompileOptions) -> CompileRequest<'a> {
    CompileRequest { sources, options }
}

fn integer_value(snapshot: &slang::Snapshot, constant_id: u64) -> Option<u64> {
    let constant = snapshot.constants.get(usize::try_from(constant_id).ok()?)?;
    match &constant.value {
        ConstantValue::Integer {
            value_words,
            unknown_words,
            ..
        } if unknown_words.iter().all(|word| *word == 0) => value_words.first().copied(),
        _ => None,
    }
}

#[test]
fn captures_owned_hierarchy_and_resolved_parameter_values() {
    let source = Source::compilation_unit(
        "hierarchy.sv",
        r#"
module child #(parameter int WIDTH = 1);
endmodule

module top #(parameter int BASE = 3);
    child #(.WIDTH(BASE + 2)) u_child();
endmodule
"#,
    );
    let sources = [source];
    let options = CompileOptions {
        top_modules: vec!["top".into()],
        parameter_overrides: vec![slang::ParameterOverride {
            name: "BASE".into(),
            value: "7".into(),
        }],
        ..CompileOptions::default()
    };

    let snapshot = slang::compile_checked(&request(&sources, &options))
        .expect("valid hierarchy should compile");
    assert!(!snapshot.has_errors());
    assert!(snapshot.analysis_ran());

    let top = snapshot
        .instances
        .iter()
        .find(|instance| instance.name == "top")
        .expect("top instance");
    let child = snapshot
        .instances
        .iter()
        .find(|instance| instance.name == "u_child")
        .expect("child instance");
    assert_eq!(child.parent_id, Some(top.id));
    assert_eq!(child.definition_name, "child");

    let width = snapshot
        .parameters
        .iter()
        .find(|parameter| parameter.owner_instance_id == child.id && parameter.name == "WIDTH")
        .expect("resolved child WIDTH parameter");
    assert_eq!(
        integer_value(
            &snapshot,
            width.constant_id.expect("value parameter has a constant")
        ),
        Some(9)
    );

    // Native snapshot storage has already been destroyed by the FFI facade.
    assert_eq!(snapshot.files[0].name, "hierarchy.sv");
    assert!(snapshot
        .types
        .iter()
        .any(|ty| ty.is_signed && ty.bit_width == 32));
}

#[test]
fn syntax_diagnostics_remain_owned_and_checked_compile_rejects_them() {
    let sources = [Source::compilation_unit(
        "broken.sv",
        "module broken(input logic a; endmodule\n",
    )];
    let options = CompileOptions::default();

    let snapshot = slang::compile(&request(&sources, &options))
        .expect("HDL errors belong in a successful snapshot");
    assert!(snapshot.has_errors());
    assert!(!snapshot.diagnostics.is_empty());
    assert!(snapshot.diagnostics.iter().any(|diagnostic| matches!(
        diagnostic.severity,
        DiagnosticSeverity::Error | DiagnosticSeverity::Fatal
    )));
    assert!(snapshot.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .primary
            .is_some_and(|range| range.file_id == snapshot.files[0].id)
    }));

    match slang::compile_checked(&request(&sources, &options)) {
        Err(CompileError::Diagnostics(diagnostics)) => assert!(!diagnostics.is_empty()),
        other => panic!("expected owned diagnostics, got {other:?}"),
    }
}

#[test]
fn invalid_requests_are_startup_failures() {
    let options = CompileOptions::default();
    match slang::compile_checked(&request(&[], &options)) {
        Err(CompileError::Startup(error)) => {
            assert_eq!(error.kind(), SlangErrorKind::InvalidArgument);
            assert!(error.message().contains("source"));
        }
        other => panic!("expected startup failure, got {other:?}"),
    }
}

#[test]
fn resource_limits_accept_the_boundary_and_reject_the_next_record() {
    let top_only = [Source::compilation_unit(
        "top.sv",
        "module top; endmodule\n",
    )];
    let exact_options = CompileOptions {
        limits: slang::Limits {
            max_sources: 1,
            max_instances: 1,
            ..slang::Limits::default()
        },
        ..CompileOptions::default()
    };
    slang::compile_checked(&request(&top_only, &exact_options))
        .expect("one source and one instance meet the exact limits");

    let two_sources = [
        Source::compilation_unit("first.sv", "module first; endmodule\n"),
        Source::compilation_unit("second.sv", "module second; endmodule\n"),
    ];
    let error = slang::compile(&request(&two_sources, &exact_options))
        .expect_err("second source must cross max_sources");
    assert_eq!(error.kind(), SlangErrorKind::LimitExceeded);

    let nested = [Source::compilation_unit(
        "nested.sv",
        "module child; endmodule\nmodule top; child u_child(); endmodule\n",
    )];
    let options = CompileOptions {
        top_modules: vec!["top".into()],
        ..exact_options
    };
    let error = slang::compile(&request(&nested, &options))
        .expect_err("child instance must cross the native max_instances limit");
    assert_eq!(error.kind(), SlangErrorKind::LimitExceeded);
}

#[test]
fn warnings_do_not_turn_a_valid_snapshot_into_failure() {
    let sources = [Source::compilation_unit(
        "warning.sv",
        "module top; logic [3:0] value = 32'hffff_ffff; endmodule\n",
    )];
    let options = CompileOptions::default();
    let snapshot = slang::compile_checked(&request(&sources, &options))
        .expect("warnings should preserve frontend success");

    assert!(snapshot
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Warning));
    assert!(!snapshot.has_errors());
}

#[test]
fn analysis_diagnostics_include_unused_checks_and_quiet_controls() {
    let sources = [Source::compilation_unit(
        "analysis.sv",
        r#"
module top(input logic source_value, output logic sink_value);
    logic dead_value;
    logic observed_value;
    always_comb begin
        observed_value = source_value;
        sink_value = observed_value;
    end
endmodule
"#,
    )];
    let options = CompileOptions::default();
    let snapshot = slang::compile_checked(&request(&sources, &options))
        .expect("analysis warnings should preserve success");
    let analysis: Vec<_> = snapshot
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.provider == DiagnosticProvider::Analysis)
        .collect();
    assert!(analysis
        .iter()
        .any(|diagnostic| diagnostic.message.contains("dead_value")));
    assert!(!analysis
        .iter()
        .any(|diagnostic| diagnostic.message.contains("observed_value")));
}

#[test]
fn repeated_compiles_return_independent_owned_snapshots() {
    let options = CompileOptions::default();
    let snapshots: Vec<_> = (0..16)
        .map(|index| {
            let text = format!("module top #(parameter int INDEX = {index}); endmodule\n");
            let sources = [Source::compilation_unit("repeat.sv", &text)];
            slang::compile(&request(&sources, &options)).expect("repeated compile")
        })
        .collect();

    for (index, snapshot) in snapshots.iter().enumerate() {
        let parameter = snapshot
            .parameters
            .iter()
            .find(|parameter| parameter.name == "INDEX")
            .expect("INDEX parameter");
        assert_eq!(
            integer_value(snapshot, parameter.constant_id.expect("constant id")),
            Some(index as u64)
        );
        assert_eq!(snapshot.files[0].name, "repeat.sv");
    }
}

#[test]
fn concurrent_compiles_keep_project_state_isolated() {
    let workers: Vec<_> = (0..4_u64)
        .map(|index| {
            std::thread::spawn(move || {
                let name = format!("project_{index}.sv");
                let text = format!(
                    "module top #(parameter longint unsigned PROJECT = {index}); endmodule\n"
                );
                let sources = [Source::compilation_unit(&name, &text)];
                let options = CompileOptions::default();
                slang::compile_checked(&request(&sources, &options))
                    .expect("concurrent project compile")
            })
        })
        .collect();

    for (index, worker) in workers.into_iter().enumerate() {
        let snapshot = worker.join().expect("Slang worker did not panic");
        assert_eq!(snapshot.files[0].name, format!("project_{index}.sv"));
        let parameter = snapshot
            .parameters
            .iter()
            .find(|parameter| parameter.name == "PROJECT")
            .expect("PROJECT parameter");
        assert_eq!(
            integer_value(&snapshot, parameter.constant_id.expect("constant id")),
            Some(index as u64)
        );
    }
}

#[test]
fn systemverilog_string_parameters_preserve_non_utf8_bytes() {
    let sources = [Source::compilation_unit(
        "bytes.sv",
        r#"module top #(parameter string PAYLOAD = "\377\000A"); endmodule
"#,
    )];
    let options = CompileOptions::default();
    let snapshot = slang::compile_checked(&request(&sources, &options))
        .expect("byte-valued string parameter should compile");
    let parameter = snapshot
        .parameters
        .iter()
        .find(|parameter| parameter.name == "PAYLOAD")
        .expect("PAYLOAD parameter");
    let constant = snapshot
        .constants
        .get(
            usize::try_from(parameter.constant_id.expect("string constant"))
                .expect("constant id fits usize"),
        )
        .expect("constant record");
    // Slang applies the language's integral-to-string conversion and removes
    // zero bytes; the remaining 0xff must still cross the ABI losslessly.
    assert_eq!(constant.value, ConstantValue::String(vec![0xff, b'A']));
}

#[test]
fn constant_payloads_preserve_four_state_and_float_domains() {
    let sources = [Source::compilation_unit(
        "constants.sv",
        r#"
module top #(
    parameter logic [3:0] MASK = 4'b10xz,
    parameter real REAL_VALUE = 1.25,
    parameter shortreal SHORT_VALUE = 1.5
);
endmodule
"#,
    )];
    let options = CompileOptions::default();
    let snapshot = slang::compile_checked(&request(&sources, &options))
        .expect("typed constants should compile");
    let value = |name: &str| {
        let parameter = snapshot
            .parameters
            .iter()
            .find(|parameter| parameter.name == name)
            .unwrap_or_else(|| panic!("missing {name} parameter"));
        &snapshot.constants[usize::try_from(parameter.constant_id.expect("constant id"))
            .expect("constant id fits usize")]
        .value
    };

    assert_eq!(
        value("MASK"),
        &ConstantValue::Integer {
            is_signed: false,
            bit_width: 4,
            value_words: vec![0b1001],
            unknown_words: vec![0b0011],
        }
    );
    assert_eq!(value("REAL_VALUE"), &ConstantValue::Real(1.25));
    assert_eq!(value("SHORT_VALUE"), &ConstantValue::ShortReal(1.5));
}

#[test]
fn admitted_include_buffers_resolve_without_filesystem_reads() {
    let sources = [
        Source::compilation_unit(
            "/virtual/project/src/top.sv",
            "`include \"cached.svh\"\nmodule top #(parameter int VALUE = `CACHED_VALUE); endmodule\n",
        ),
        Source::include(
            "/virtual/project/include/cached.svh",
            "`define CACHED_VALUE 37\n",
        ),
    ];
    let options = CompileOptions {
        include_dirs: vec!["/virtual/project/include".into()],
        ..CompileOptions::default()
    };
    let snapshot = slang::compile_checked(&request(&sources, &options))
        .expect("admitted include should resolve from the source cache");
    let value = snapshot
        .parameters
        .iter()
        .find(|parameter| parameter.name == "VALUE")
        .and_then(|parameter| parameter.constant_id)
        .and_then(|id| integer_value(&snapshot, id));
    assert_eq!(value, Some(37));
}

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn unique_temp_dir() -> PathBuf {
    let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "llg-slang-admission-{}-{sequence}",
        std::process::id()
    ))
}

#[test]
fn existing_unadmitted_absolute_include_cannot_leak_into_elaboration() {
    let dir = unique_temp_dir();
    std::fs::create_dir(&dir).expect("create isolated test directory");
    let header = dir.join("external.svh");
    std::fs::write(&header, "`define EXTERNAL_SECRET 991\n").expect("write external header");
    let include_path = header.to_string_lossy().replace('\\', "/");
    let text = format!(
        "`include \"{include_path}\"\nmodule top #(parameter int LEAK = `EXTERNAL_SECRET); endmodule\n"
    );
    let sources = [Source::compilation_unit("top.sv", &text)];
    let options = CompileOptions::default();

    let snapshot = slang::compile(&request(&sources, &options))
        .expect("denied include should be represented by diagnostics");
    assert!(snapshot.has_errors());
    assert!(snapshot
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.name == "CouldNotOpenIncludeFile"));
    assert!(!snapshot.files.iter().any(|file| file.name == include_path));
    assert!(!snapshot.parameters.iter().any(|parameter| {
        parameter.name == "EXTERNAL_SECRET"
            || parameter
                .constant_id
                .and_then(|id| integer_value(&snapshot, id))
                == Some(991)
    }));

    std::fs::remove_file(&header).expect("remove external header");
    std::fs::remove_dir(&dir).expect("remove isolated test directory");
}
