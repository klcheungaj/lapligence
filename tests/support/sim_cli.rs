//! File-based simulator acceptance tests through the public executable.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::Duration;

use super::sim_harness;

fn fixture_path(suite: &str, fixture: &str) -> PathBuf {
    let name = if matches!(
        Path::new(fixture).extension().and_then(|ext| ext.to_str()),
        Some("v" | "sv")
    ) {
        fixture.to_owned()
    } else {
        format!("{fixture}.sv")
    };
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim")
        .join(suite)
        .join(name);
    assert!(source.is_file(), "missing fixture: {}", source.display());
    source
}

/// Execute a checked fixture after destroying the frontend snapshot and owned Db.
/// Generation validates semantic and execution IR before owned whole-model emission.
/// This supplements, rather than replaces, public CLI acceptance of the same fixture.
pub(crate) fn run_case_after_db_drop(suite: &str, fixture: &str, expected: &str) {
    let source = fixture_path(suite, fixture);
    run_compile_opts_after_db_drop(
        suite,
        fixture,
        llg::core::compile::CompileOpts {
            files: vec![source.to_string_lossy().into_owned()],
            top: Some("tb".to_owned()),
            ..Default::default()
        },
        expected,
        "",
    );
}

/// [`run_case_after_db_drop`] for a fixture compiled with explicit options,
/// such as library maps and a configuration top. `opts.files` must name the
/// fixture itself so its owned source text can be checked after the drop.
pub(crate) fn run_compile_opts_after_db_drop(
    suite: &str,
    fixture: &str,
    opts: llg::core::compile::CompileOpts,
    expected: &str,
    expected_stderr: &str,
) {
    use llg::core::{compile, db::Db};
    use llg::sim::{build, codegen, opt::OptConfig};

    assert!(
        build::cmake_available(),
        "owned execution tests require CMake"
    );
    let source = fixture_path(suite, fixture);
    let source_text = std::fs::read_to_string(&source).expect("read checked fixture");
    let value_config = llg::sim::value_backend::ValueConfig::from_env().expect("value selection");
    let models = sim_harness::with_frontend_temp_cwd("owned-feature", |_| {
        let compiled =
            compile::compile_checked(&opts).map_err(|error| format!("compile: {error}"))?;
        let database =
            Db::from_slang(&compiled.snapshot).map_err(|error| format!("database: {error}"))?;
        drop(compiled);
        let captured_path = database
            .nodes()
            .iter()
            .filter(|node| node.line > 0)
            .filter_map(|node| node.file.as_deref())
            .find(|file| Path::new(file).file_name() == source.file_name())
            .expect("owned source location survives snapshot destruction");
        assert_eq!(
            database.source_text(captured_path),
            Some(source_text.as_str())
        );
        let mut models = Vec::new();
        for optimized in [false, true] {
            let options = if optimized {
                OptConfig::default()
            } else {
                OptConfig::none()
            };
            let model = codegen::generate_from_db_with_codegen_options(
                &database,
                &codegen::CodegenOptions {
                    optimization: options,
                    value_config,
                    ..Default::default()
                },
            )
            .map_err(|error| format!("codegen: {error}"))?;
            assert!(
                model.warnings.is_empty(),
                "unexpected warnings: {:?}",
                model.warnings
            );
            models.push((optimized, model));
        }
        drop(database);
        Ok(models)
    })
    .expect("checked compilation and validated owned emission");
    for (optimized, model) in models {
        for level in [build::ModelOptLevel::O0, build::ModelOptLevel::O3] {
            let directory =
                sim_harness::TempDir::new("owned-feature-model").expect("model directory");
            let options = build::CmakeBuildOpts {
                model_opt_level: level,
                value_config,
                ..Default::default()
            };
            let executable =
                build::build_model_cmake_with_opts(directory.path(), &model.sources(), &options)
                    .expect("build owned generated model");
            let output =
                sim_harness::run_executable_output(&executable).expect("execute owned model");
            let label =
                format!("{suite}/{fixture}, Db dropped, optimized={optimized}, native={level:?}");
            assert_case_output(output, &label, expected, expected_stderr, &[]);
        }
    }
}

/// Resolve companion inputs (library maps, library sources, configurations)
/// named relative to the suite directory. Each must be a checked-in file. An
/// argument equal to an input name, or `library=name`, becomes its absolute
/// path, so maps keep their own directory as the relative base.
fn resolve_input_args(suite: &str, inputs: &[&str], args: &[&str]) -> Vec<String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim")
        .join(suite);
    let absolute = |name: &str| {
        let path = root.join(name);
        assert!(path.is_file(), "missing fixture input: {}", path.display());
        path.to_string_lossy().into_owned()
    };
    for input in inputs {
        absolute(input);
    }
    args.iter()
        .map(|arg| {
            if inputs.contains(arg) {
                return absolute(arg);
            }
            match arg.split_once('=') {
                Some((library, name)) if inputs.contains(&name) => {
                    format!("{library}={}", absolute(name))
                }
                _ => (*arg).to_owned(),
            }
        })
        .collect()
}

/// Run a fixture whose command also names checked-in companion inputs, in
/// both optimizer modes with explicit child environment controls (for
/// example the value backend). See [`resolve_input_args`].
pub(crate) fn run_case_with_inputs(
    suite: &str,
    fixture: &str,
    inputs: &[&str],
    expected: &str,
    expected_stderr: &str,
    args: &[&str],
    envs: &[(&str, &str)],
) {
    assert!(
        llg::sim::build::cmake_available(),
        "CLI tests require CMake"
    );
    let args = resolve_input_args(suite, inputs, args);
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    for optimized in [false, true] {
        let output = invoke_with_env(suite, fixture, optimized, &args, envs, &[]);
        let label = format!("{suite}/{fixture}, optimized={optimized}, env={envs:?}");
        assert_case_output(output, &label, expected, expected_stderr, &[]);
    }
}

/// Reject a fixture whose command names checked-in companion inputs.
pub(crate) fn reject_case_with_inputs(
    suite: &str,
    fixture: &str,
    inputs: &[&str],
    diagnostic: &str,
    args: &[&str],
) {
    let args = resolve_input_args(suite, inputs, args);
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    reject_case_with_args(suite, fixture, diagnostic, &args);
}

fn invoke_with_args(suite: &str, fixture: &str, optimized: bool, args: &[&str]) -> Output {
    invoke_with_env(suite, fixture, optimized, args, &[], &[])
}

/// Invoke a fixture after source files which must precede it in a merged
/// compilation unit. The paths are still checked-in fixture stems, so the
/// public CLI observes the same admission and file ordering as a user command.
pub(crate) fn invoke_with_source_prefix(
    suite: &str,
    fixture: &str,
    prefix: &[&str],
    optimized: bool,
    args: &[&str],
) -> Output {
    invoke_with_source_prefix_and_runtime_args(suite, fixture, prefix, optimized, args, &[])
}

fn invoke_with_source_prefix_and_runtime_args(
    suite: &str,
    fixture: &str,
    prefix: &[&str],
    optimized: bool,
    args: &[&str],
    runtime_args: &[&str],
) -> Output {
    let source = fixture_path(suite, fixture);
    let prefix_paths: Vec<_> = prefix
        .iter()
        .map(|stem| fixture_path(suite, stem))
        .collect();
    let directory = sim_harness::TempDir::new(fixture).expect("CLI test directory");
    let label = format!("{suite}/{fixture}, optimized={optimized}");
    run_fixture_command(&source, &label, |lint_args| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_llg"));
        command.current_dir(directory.path()).args(["--top", "tb"]);
        if !optimized {
            command.arg("--no-opt");
        }
        command.args(args).args(lint_args);
        for path in &prefix_paths {
            command.arg(path);
        }
        command.arg(&source);
        if !runtime_args.is_empty() {
            command.arg("--").args(runtime_args);
        }
        command
    })
}

/// Invoke one checked-in fixture after copying its independent memory-file
/// inputs into the child working directory. Memory tasks resolve relative
/// paths from that directory, so the files must be installed after creating
/// the isolated directory and before launching the public CLI.
fn invoke_with_files(
    suite: &str,
    fixture: &str,
    optimized: bool,
    args: &[&str],
    files: &[(&str, &str)],
) -> Output {
    let source = fixture_path(suite, fixture);
    let directory = sim_harness::TempDir::new(fixture).expect("CLI test directory");
    for (name, contents) in files {
        std::fs::write(directory.path().join(name), contents)
            .unwrap_or_else(|error| panic!("{suite}/{fixture}: write {name}: {error}"));
    }
    let label = format!("{suite}/{fixture}, optimized={optimized}");
    run_fixture_command(&source, &label, |lint_args| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_llg"));
        command.current_dir(directory.path()).args(["--top", "tb"]);
        if !optimized {
            command.arg("--no-opt");
        }
        command.args(args).args(lint_args).arg(&source);
        command
    })
}

/// Invoke one checked-in simulator fixture with explicit child-process
/// environment controls. `remove_env` is applied after `envs`, so tests can
/// guarantee that a host configuration variable is absent even when the test
/// runner inherited it.
pub(crate) fn invoke_with_env(
    suite: &str,
    fixture: &str,
    optimized: bool,
    args: &[&str],
    envs: &[(&str, &str)],
    remove_env: &[&str],
) -> Output {
    let source = fixture_path(suite, fixture);
    let directory = sim_harness::TempDir::new(fixture).expect("CLI test directory");
    let label = format!("{suite}/{fixture}, optimized={optimized}");
    run_fixture_command(&source, &label, |lint_args| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_llg"));
        command.current_dir(directory.path()).args(["--top", "tb"]);
        if !optimized {
            command.arg("--no-opt");
        }
        command.args(args).args(lint_args);
        command.arg(&source);
        command.envs(envs.iter().copied());
        for variable in remove_env {
            command.env_remove(variable);
        }
        command
    })
}

/// [`invoke_with_env`] after writing checked-in input `files` (name,
/// contents) into the child's working directory, where relative `$fopen`
/// paths resolve.
fn invoke_with_env_and_files(
    suite: &str,
    fixture: &str,
    optimized: bool,
    args: &[&str],
    envs: &[(&str, &str)],
    remove_env: &[&str],
    files: &[(&str, &str)],
) -> Output {
    let source = fixture_path(suite, fixture);
    let directory = sim_harness::TempDir::new(fixture).expect("CLI test directory");
    for (name, contents) in files {
        std::fs::write(directory.path().join(name), contents)
            .unwrap_or_else(|error| panic!("{suite}/{fixture}: write {name}: {error}"));
    }
    let label = format!("{suite}/{fixture}, optimized={optimized}");
    run_fixture_command(&source, &label, |lint_args| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_llg"));
        command.current_dir(directory.path()).args(["--top", "tb"]);
        if !optimized {
            command.arg("--no-opt");
        }
        command.args(args).args(lint_args);
        command.arg(&source);
        command.envs(envs.iter().copied());
        for variable in remove_env {
            command.env_remove(variable);
        }
        command
    })
}

fn invoke(suite: &str, fixture: &str, optimized: bool) -> Output {
    invoke_with_args(suite, fixture, optimized, &[])
}

fn invoke_with_runtime_args(
    suite: &str,
    fixture: &str,
    optimized: bool,
    args: &[&str],
    runtime_args: &[&str],
) -> Output {
    let source = fixture_path(suite, fixture);
    let directory = sim_harness::TempDir::new(fixture).expect("CLI test directory");
    let label = format!("{suite}/{fixture}, optimized={optimized}");
    run_fixture_command(&source, &label, |lint_args| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_llg"));
        command.current_dir(directory.path()).args(["--top", "tb"]);
        if !optimized {
            command.arg("--no-opt");
        }
        command
            .args(args)
            .args(lint_args)
            .arg(&source)
            .arg("--")
            .args(runtime_args);
        command
    })
}

fn assert_case_output(
    output: Output,
    label: &str,
    expected: &str,
    expected_stderr: &str,
    expected_warnings: &[&str],
) {
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{label}: {stderr}");
    let actual = String::from_utf8_lossy(&output.stdout);
    if actual != expected {
        let line = actual
            .lines()
            .zip(expected.lines())
            .position(|(actual, expected)| actual != expected)
            .unwrap_or_else(|| actual.lines().count().min(expected.lines().count()));
        panic!(
            "{label}: line {}: expected {:?}, got {:?} ({} versus {} lines)\n{stderr}",
            line + 1,
            expected.lines().nth(line),
            actual.lines().nth(line),
            expected.lines().count(),
            actual.lines().count()
        );
    }
    let mut warnings = Vec::new();
    let mut runtime_stderr = String::new();
    for line in stderr.lines() {
        if let Some(warning) = line.strip_prefix("llg: warning: ") {
            warnings.push(warning);
        } else if !crate::sim_harness::is_compile_report_line(line) {
            // Legal conformance probes intentionally provoke frontend
            // width/sign/range warnings; lowering warnings remain exact.
            runtime_stderr.push_str(line);
            runtime_stderr.push('\n');
        }
    }
    warnings.sort_unstable();
    let mut expected_warnings = expected_warnings.to_vec();
    expected_warnings.sort_unstable();
    assert_eq!(warnings, expected_warnings, "{label}");
    assert_eq!(runtime_stderr, expected_stderr, "{label}");
}

pub(crate) fn run_case_backend_parity(
    suite: &str,
    fixture: &str,
    expected: &str,
    args: &[&str],
    envs: &[(&str, &str)],
) {
    run_case_backend_parity_with_files(suite, fixture, expected, args, envs, &[]);
}

/// [`run_case_backend_parity`] with checked-in input `files` (name, contents)
/// written into every run's working directory.
pub(crate) fn run_case_backend_parity_with_files(
    suite: &str,
    fixture: &str,
    expected: &str,
    args: &[&str],
    envs: &[(&str, &str)],
    files: &[(&str, &str)],
) {
    let gmp = sim_harness::test_gmp_root();
    for optimized in [false, true] {
        let mut controls = envs.to_vec();
        controls.extend([
            ("LLG_DEV_VALUE_BACKEND", "legacy"),
            ("LLG_DEV_COMPACT_KERNELS", "portable"),
        ]);
        let legacy = invoke_with_env_and_files(
            suite,
            fixture,
            optimized,
            args,
            &controls,
            &["GMP_ROOT"],
            files,
        );
        let stdout = legacy.stdout.clone();
        let stderr = legacy.stderr.clone();
        assert!(
            legacy.status.success(),
            "{suite}/{fixture}, legacy: {}",
            String::from_utf8_lossy(&stderr)
        );
        assert_eq!(
            stdout,
            expected.as_bytes(),
            "{suite}/{fixture}, legacy, optimized={optimized}: independent output mismatch"
        );
        for kernel in ["portable", "gmp"] {
            let mut controls = envs.to_vec();
            controls.extend([
                ("LLG_DEV_VALUE_BACKEND", "compact"),
                ("LLG_DEV_COMPACT_KERNELS", kernel),
                ("GMP_ROOT", gmp.as_str()),
            ]);
            let compact =
                invoke_with_env_and_files(suite, fixture, optimized, args, &controls, &[], files);
            let label = format!("{suite}/{fixture}, compact/{kernel}, optimized={optimized}");
            assert!(
                compact.status.success(),
                "{label}: {}",
                String::from_utf8_lossy(&compact.stderr)
            );
            assert_eq!(compact.stdout, stdout, "{label}: legacy stdout mismatch");
            assert_eq!(
                compact.stdout,
                expected.as_bytes(),
                "{label}: independent output mismatch"
            );
            assert_eq!(compact.stderr, stderr, "{label}: legacy stderr mismatch");
        }
    }
}

/// Run one fixture through the public CLI in both optimizer modes on legacy,
/// compact/portable and compact/GMP values, and
/// hand each labeled output to `check`. For outputs that are not a single
/// exact string: permitted race outcomes, partial orders, or runtime failures
/// after some output.
pub(crate) fn run_case_checked_matrix(
    suite: &str,
    fixture: &str,
    args: &[&str],
    check: &dyn Fn(&str, &Output),
) {
    assert!(
        llg::sim::build::cmake_available(),
        "CLI tests require CMake"
    );
    let gmp = sim_harness::test_gmp_root();
    for optimized in [false, true] {
        for (backend, kernel) in [
            ("legacy", "portable"),
            ("compact", "portable"),
            ("compact", "gmp"),
        ] {
            let controls = [
                ("LLG_DEV_VALUE_BACKEND", backend),
                ("LLG_DEV_COMPACT_KERNELS", kernel),
                ("GMP_ROOT", gmp.as_str()),
            ];
            let output = invoke_with_env(suite, fixture, optimized, args, &controls, &[]);
            let label = format!("{suite}/{fixture}, {backend}/{kernel}, optimized={optimized}");
            check(&label, &output);
        }
    }
}

pub(crate) fn run_case(
    suite: &str,
    fixture: &str,
    expected: &str,
    expected_stderr: &str,
    expected_warnings: &[&str],
) {
    run_case_with_args(
        suite,
        fixture,
        expected,
        expected_stderr,
        expected_warnings,
        &[],
    );
}

pub(crate) fn run_case_with_runtime_args(
    suite: &str,
    fixture: &str,
    expected: &str,
    expected_stderr: &str,
    runtime_args: &[&str],
) {
    run_case_with_cli_and_runtime_args(
        suite,
        fixture,
        expected,
        expected_stderr,
        &[],
        runtime_args,
    );
}

pub(crate) fn run_case_with_cli_and_runtime_args(
    suite: &str,
    fixture: &str,
    expected: &str,
    expected_stderr: &str,
    args: &[&str],
    runtime_args: &[&str],
) {
    assert!(
        llg::sim::build::cmake_available(),
        "CLI tests require CMake"
    );
    for optimized in [false, true] {
        let output = invoke_with_runtime_args(suite, fixture, optimized, args, runtime_args);
        let label = format!("{suite}/{fixture}, optimized={optimized}");
        assert_case_output(output, &label, expected, expected_stderr, &[]);
    }
}

pub(crate) fn run_case_with_args(
    suite: &str,
    fixture: &str,
    expected: &str,
    expected_stderr: &str,
    expected_warnings: &[&str],
    args: &[&str],
) {
    assert!(
        llg::sim::build::cmake_available(),
        "CLI tests require CMake"
    );
    for optimized in [false, true] {
        let output = invoke_with_args(suite, fixture, optimized, args);
        let label = format!("{suite}/{fixture}, optimized={optimized}");
        assert_case_output(output, &label, expected, expected_stderr, expected_warnings);
    }
}

pub(crate) fn run_case_with_files(
    suite: &str,
    fixture: &str,
    expected: &str,
    expected_stderr: &str,
    expected_warnings: &[&str],
    args: &[&str],
    files: &[(&str, &str)],
) {
    assert!(
        llg::sim::build::cmake_available(),
        "CLI tests require CMake"
    );
    for optimized in [false, true] {
        let output = invoke_with_files(suite, fixture, optimized, args, files);
        let label = format!("{suite}/{fixture}, optimized={optimized}");
        assert_case_output(output, &label, expected, expected_stderr, expected_warnings);
    }
}

pub(crate) fn run_case_with_source_prefix(
    suite: &str,
    fixture: &str,
    prefix: &[&str],
    expected: &str,
    expected_stderr: &str,
    expected_warnings: &[&str],
    args: &[&str],
) {
    assert!(
        llg::sim::build::cmake_available(),
        "CLI tests require CMake"
    );
    for optimized in [false, true] {
        let output = invoke_with_source_prefix(suite, fixture, prefix, optimized, args);
        let label = format!("{suite}/{fixture}, optimized={optimized}");
        assert_case_output(output, &label, expected, expected_stderr, expected_warnings);
    }
}

pub(crate) fn run_case_with_source_prefix_and_runtime_args(
    suite: &str,
    fixture: &str,
    prefix: &[&str],
    expected: &str,
    expected_stderr: &str,
    args: &[&str],
    runtime_args: &[&str],
) {
    assert!(
        llg::sim::build::cmake_available(),
        "CLI tests require CMake"
    );
    for optimized in [false, true] {
        let output = invoke_with_source_prefix_and_runtime_args(
            suite,
            fixture,
            prefix,
            optimized,
            args,
            runtime_args,
        );
        let label = format!("{suite}/{fixture}, optimized={optimized}");
        assert_case_output(output, &label, expected, expected_stderr, &[]);
    }
}

pub(crate) fn reject_case(suite: &str, fixture: &str, diagnostic: &str) {
    reject_case_with_args(suite, fixture, diagnostic, &[]);
}

pub(crate) fn reject_case_with_args(suite: &str, fixture: &str, diagnostic: &str, args: &[&str]) {
    for optimized in [false, true] {
        let output = invoke_with_args(suite, fixture, optimized, args);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(1), "{fixture}: {stderr}");
        assert!(output.stdout.is_empty(), "{fixture}: {output:?}");
        assert!(stderr.contains(diagnostic), "{fixture}: {stderr}");
    }
}

pub(crate) fn reject_case_with_exact_stderr(
    suite: &str,
    fixture: &str,
    expected_stderr: &str,
    args: &[&str],
) {
    for optimized in [false, true] {
        let output = invoke_with_args(suite, fixture, optimized, args);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(1), "{fixture}: {stderr}");
        assert!(output.stdout.is_empty(), "{fixture}: {output:?}");
        assert_eq!(stderr, expected_stderr, "{fixture}: unexpected diagnostic");
    }
}

/// Reject a fixture with exactly the expected source-located
/// `error: <fixture path>:<suffix>` lines (the stable `unsupported:`
/// diagnostics of by-design rejections), in both optimizer modes. The run must
/// exit 1 with an empty stdout, report no generic codegen failure, and leave
/// its `--out-dir` empty: nothing reached C generation.
pub(crate) fn reject_case_with_error_lines(
    suite: &str,
    fixture: &str,
    suffixes: &[String],
    args: &[&str],
) {
    let source = sim_harness::source_display(&fixture_path(suite, fixture));
    let expected: Vec<String> = suffixes
        .iter()
        .map(|suffix| format!("error: {source}:{suffix}"))
        .collect();
    for optimized in [false, true] {
        let out_dir = sim_harness::TempDir::new("reject-out").expect("output directory");
        let out = out_dir.path().to_string_lossy().into_owned();
        let mut full_args: Vec<&str> = args.to_vec();
        full_args.extend(["--out-dir", out.as_str()]);
        let output = invoke_with_args(suite, fixture, optimized, &full_args);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let label = format!("{suite}/{fixture}, optimized={optimized}, args={args:?}");
        assert_eq!(output.status.code(), Some(1), "{label}: {stderr}");
        assert!(output.stdout.is_empty(), "{label}: ran: {output:?}");
        let errors: Vec<&str> = stderr
            .lines()
            .filter(|line| line.starts_with("error: "))
            .collect();
        assert_eq!(errors, expected, "{label}: {stderr}");
        assert!(
            !stderr.contains("codegen error") && !stderr.contains("panicked"),
            "{label}: generic failure: {stderr}"
        );
        assert!(
            std::fs::read_dir(out_dir.path()).unwrap().next().is_none(),
            "{label}: C generation started"
        );
    }
}

pub(crate) fn reject_case_with_runtime_args(
    suite: &str,
    fixture: &str,
    diagnostic: &str,
    runtime_args: &[&str],
) {
    for optimized in [false, true] {
        let output = invoke_with_runtime_args(suite, fixture, optimized, &[], runtime_args);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(1), "{fixture}: {stderr}");
        assert!(output.stdout.is_empty(), "{fixture}: {output:?}");
        assert!(stderr.contains(diagnostic), "{fixture}: {stderr}");
    }
}

/// The default-lint errors a checked-in fixture deliberately triggers, from
/// `tests/fixtures/sim/lint/expected_errors.tsv`, and the configuration that
/// disables their rules.
pub(crate) struct ExpectedLint {
    /// `rule line:col`, sorted.
    errors: Vec<String>,
    /// `allow_*.toml` disabling exactly the listed rules.
    config: String,
}

/// The manifest entry for `source`, if it is listed.
pub(crate) fn expected_lint(source: &Path) -> Option<&'static ExpectedLint> {
    static MANIFEST: std::sync::OnceLock<std::collections::HashMap<PathBuf, ExpectedLint>> =
        std::sync::OnceLock::new();
    let manifest = MANIFEST.get_or_init(|| {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sim");
        let text = std::fs::read_to_string(root.join("lint/expected_errors.tsv"))
            .expect("read lint/expected_errors.tsv");
        let mut errors: std::collections::BTreeMap<PathBuf, Vec<(String, String)>> =
            std::collections::BTreeMap::new();
        for line in text.lines().filter(|line| !line.starts_with('#') && !line.is_empty()) {
            let fields: Vec<&str> = line.split('\t').collect();
            let [fixture, rule, position] = fields[..] else {
                panic!("malformed lint/expected_errors.tsv line: {line:?}");
            };
            let path = root.join(fixture);
            assert!(path.is_file(), "lint manifest names a missing fixture: {fixture}");
            errors
                .entry(path)
                .or_default()
                .push((rule.to_owned(), position.to_owned()));
        }
        errors
            .into_iter()
            .map(|(path, errors)| {
                let has = |name: &str| errors.iter().any(|(rule, _)| rule == name);
                let config = match (has("mixed-assignments"), has("combinational-loop")) {
                    (true, true) => "allow_feedback_and_mixed.toml",
                    (true, false) => "allow_mixed_assignments.toml",
                    (false, true) => "allow_combinational_loop.toml",
                    (false, false) => panic!("lint manifest rules must be mixed-assignments or combinational-loop: {errors:?}"),
                };
                let mut listed: Vec<String> = errors
                    .iter()
                    .map(|(rule, position)| format!("{rule} {position}"))
                    .collect();
                listed.sort();
                let expected = ExpectedLint {
                    errors: listed,
                    config: root.join("lint").join(config).to_string_lossy().into_owned(),
                };
                (path, expected)
            })
            .collect()
    });
    manifest.get(source)
}

impl ExpectedLint {
    /// `--config` arguments that disable the listed rules.
    pub(crate) fn allow_args(&self) -> [&str; 2] {
        ["--config", self.config.as_str()]
    }

    /// Require `output` (a default-lint run of `source`) to have stopped
    /// before code generation with exactly the listed errors.
    pub(crate) fn assert_stopped(&self, source: &Path, output: &Output, label: &str) {
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            output.status.code(),
            Some(1),
            "{label}: default lint must stop the run: {stderr}"
        );
        assert!(
            output.stdout.is_empty(),
            "{label}: the model must not run: {stderr}"
        );
        let mut actual: Vec<String> = stderr
            .lines()
            .filter_map(|line| {
                let (location, finding) = line.split_once(": [ERROR] ")?;
                let rule = finding.split_once(": ")?.0;
                let mut parts = location.rsplitn(3, ':');
                let col = parts.next()?;
                let line_number = parts.next()?;
                let file = parts.next()?;
                assert_eq!(
                    Path::new(file).file_name(),
                    source.file_name(),
                    "{label}: lint error outside the fixture: {line}"
                );
                Some(format!("{rule} {line_number}:{col}"))
            })
            .collect();
        // Each instance of a module reports its findings; the manifest lists
        // distinct positions.
        actual.sort();
        actual.dedup();
        assert_eq!(
            actual, self.errors,
            "{label}: default lint errors: {stderr}"
        );
    }
}

/// Whether `output` reports a lint error finding.
fn has_lint_error(output: &Output) -> bool {
    String::from_utf8_lossy(&output.stderr)
        .lines()
        .any(|line| line.contains(": [ERROR] "))
}

/// Run the `llg` command `build(lint_args)` for the fixture `source`. A
/// fixture listed in `lint/expected_errors.tsv` is first run with the default
/// lint configuration, which must stop with exactly its listed errors; the
/// returned run then disables those rules with `--config`.
fn run_fixture_command(source: &Path, label: &str, build: impl Fn(&[&str]) -> Command) -> Output {
    let run = |mut command: Command| {
        sim_harness::run_command(&mut command, Duration::from_secs(180))
            .unwrap_or_else(|error| panic!("{label}: {error}"))
    };
    match expected_lint(source) {
        None => run(build(&[])),
        Some(expected) => {
            let output = run(build(&[]));
            // A frontend rejection (an edition without the fixture's syntax,
            // for instance) ends the run before lint; the caller checks it.
            if !output.status.success() && !has_lint_error(&output) {
                return output;
            }
            expected.assert_stopped(source, &output, label);
            run(build(&expected.allow_args()))
        }
    }
}
