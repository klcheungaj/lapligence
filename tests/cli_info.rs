//! Process-level CLI output and usage-error contracts.

use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

fn invoke(binary: &str, args: &[&str]) -> Output {
    let mut child = Command::new(binary)
        .args(args)
        .env("LLG_MEMORY_LIMIT_MB", "1")
        .env("LLG_LOG", "trace")
        .env_remove("LLG_LOG_FILE")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start CLI");
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if child.try_wait().expect("poll CLI").is_some() {
            return child.wait_with_output().expect("collect output");
        }
        if Instant::now() >= deadline {
            child.kill().expect("stop stalled CLI");
            let output = child.wait_with_output().expect("reap stalled CLI");
            panic!("CLI waited for input instead of exiting: {output:?}");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn check_information(binary: &str, name: &str, mode: &str) {
    for flag in ["--version", "-V"] {
        let output = invoke(binary, &[flag]);
        assert!(output.status.success(), "{output:?}");
        assert!(output.stderr.is_empty(), "{output:?}");
        assert_eq!(
            String::from_utf8(output.stdout).unwrap(),
            format!("{name} {}\n", env!("CARGO_PKG_VERSION"))
        );
    }
    for flag in ["--help", "-h"] {
        let output = invoke(binary, &[flag]);
        assert!(output.status.success(), "{output:?}");
        assert!(output.stderr.is_empty(), "{output:?}");
        let help = String::from_utf8(output.stdout).unwrap();
        for expected in [name, "Usage:", "--help", "--version", mode] {
            assert!(help.contains(expected), "missing {expected}: {help}");
        }
    }
}

#[test]
fn simulator_information_exits_without_compiling_or_installing_memory_limits() {
    check_information(env!("CARGO_BIN_EXE_llg"), "llg", "--gen-only");
    let output = invoke(env!("CARGO_BIN_EXE_llg"), &["--help"]);
    assert!(String::from_utf8_lossy(&output.stdout).contains("--no-opt"));
    assert!(String::from_utf8_lossy(&output.stdout).contains("--edition <v2001|sv2009>"));
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("--compilation-units <separate|merged>")
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("--delay-select <min|typ|max>"));
    assert!(String::from_utf8_lossy(&output.stdout).contains("--include-dir <path>"));
    assert!(String::from_utf8_lossy(&output.stdout).contains("--define <NAME[=VALUE]>"));
    assert!(String::from_utf8_lossy(&output.stdout).contains("--define-system-task <prototype>"));
    assert!(String::from_utf8_lossy(&output.stdout).contains("--libmap <file>"));
    assert!(String::from_utf8_lossy(&output.stdout).contains("--libfile"));
    assert!(String::from_utf8_lossy(&output.stdout).contains("--library-order"));
    assert!(String::from_utf8_lossy(&output.stdout).contains("--default-library"));
    assert!(String::from_utf8_lossy(&output.stdout).contains("--stop-policy <resume|exit>"));
    for option in [
        "--pulse-reject <0-100>",
        "--pulse-error <0-100>",
        "--pulse-style <onevent|ondetect>",
        "--show-cancelled",
        "--no-show-cancelled",
        "--no-timing-checks",
        "--timing-checks",
        "--no-notifiers",
        "--notifiers",
        "--no-timing-check-messages",
        "--timing-check-messages",
        "--neg-timing-checks",
        "--no-neg-timing-checks",
    ] {
        assert!(
            String::from_utf8_lossy(&output.stdout).contains(option),
            "{option}"
        );
    }
    assert!(String::from_utf8_lossy(&output.stdout).contains("--launcher"));
    for option in [
        "--out-dir <dir>",
        "--runtime-cache <dir>",
        "--cc <program>",
        "--cflags <flags>",
        "--cmake <program>",
        "--build-jobs <N>",
        "--model-opt-level <O0|O1|O2|O3|Os>",
        "--top <module>",
        "--lint-only",
        "--lint-json [<path>]",
        "-Werror",
        "--wave <file>",
        "--wave-depth <N>",
        "--config <file>",
    ] {
        assert!(
            String::from_utf8_lossy(&output.stdout).contains(option),
            "missing {option}"
        );
    }
}

#[cfg(feature = "lsp")]
#[test]
fn server_information_exits_without_serving_or_logging() {
    check_information(env!("CARGO_BIN_EXE_llg_ls"), "llg_ls", "--dump-tokens");
    check_information(
        env!("CARGO_BIN_EXE_llg_ls"),
        "llg_ls",
        "--staging-dir <DIR>",
    );
}

#[test]
fn simulator_missing_option_value_is_a_usage_error() {
    for (option, diagnostic) in [
        ("--generator", "requires a backend name"),
        ("--launcher", "requires a program name"),
        ("--libmap", "--libmap requires a file path"),
        ("--libfile", "--libfile requires [library=]file"),
        ("-v", "--libfile requires [library=]file"),
        ("--library-order", "--library-order requires a library name"),
        ("-L", "--library-order requires a library name"),
        (
            "--default-library",
            "--default-library requires a library name",
        ),
        ("--out-dir", "--out-dir requires a directory"),
        ("--runtime-cache", "--runtime-cache requires a directory"),
        ("--cc", "--cc requires a compiler program"),
        ("--cflags", "--cflags requires a flag string"),
        ("--cmake", "--cmake requires a program"),
        ("--build-jobs", "--build-jobs requires a positive integer"),
        ("--top", "--top requires a module name"),
        ("--wave", "--wave requires a file ending in .vcd or .fst"),
        (
            "--wave-depth",
            "--wave-depth requires a non-negative integer",
        ),
        (
            "--model-opt-level",
            "--model-opt-level requires O0, O1, O2, O3 or Os",
        ),
    ] {
        let output = invoke(env!("CARGO_BIN_EXE_llg"), &[option]);
        assert_eq!(output.status.code(), Some(2), "{output:?}");
        assert!(output.stdout.is_empty(), "{output:?}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(diagnostic),
            "{output:?}"
        );
    }
}

#[test]
fn simulator_model_opt_level_rejects_unknown_values() {
    for value in ["", "0", "-O2", "O4", "o3", "Oz"] {
        let output = invoke(env!("CARGO_BIN_EXE_llg"), &["--model-opt-level", value]);
        assert_eq!(output.status.code(), Some(2), "{output:?}");
        assert!(output.stdout.is_empty(), "{output:?}");
        assert!(
            String::from_utf8_lossy(&output.stderr)
                .contains("--model-opt-level requires O0, O1, O2, O3 or Os"),
            "{output:?}"
        );
    }
    for value in ["O0", "O1", "O2", "O3", "Os"] {
        let output = invoke(
            env!("CARGO_BIN_EXE_llg"),
            &["--model-opt-level", value, "--help"],
        );
        assert!(output.status.success(), "{output:?}");
        assert!(output.stderr.is_empty(), "{output:?}");
    }
}

#[test]
fn simulator_build_jobs_rejects_non_positive_and_non_numeric_values() {
    for value in ["0", "-1", "many", ""] {
        let output = invoke(env!("CARGO_BIN_EXE_llg"), &["--build-jobs", value]);
        assert_eq!(output.status.code(), Some(2), "{output:?}");
        assert!(output.stdout.is_empty(), "{output:?}");
        assert!(
            String::from_utf8_lossy(&output.stderr)
                .contains("--build-jobs requires a positive integer"),
            "{output:?}"
        );
    }
}

#[test]
fn simulator_edition_option_rejects_missing_and_unknown_values() {
    for (args, expected) in [
        (&["--edition"][..], "requires v2001 or sv2009"),
        (&["--edition", "2009"][..], "expected v2001 or sv2009"),
    ] {
        let output = invoke(env!("CARGO_BIN_EXE_llg"), args);
        assert_eq!(output.status.code(), Some(2), "{output:?}");
        assert!(output.stdout.is_empty(), "{output:?}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(expected),
            "{output:?}"
        );
    }
}

#[test]
fn simulator_stop_policy_rejects_missing_and_unknown_values() {
    for (args, expected) in [
        (&["--stop-policy"][..], "requires resume or exit"),
        (
            &["--stop-policy", "interactive"][..],
            "expected resume or exit",
        ),
    ] {
        let output = invoke(env!("CARGO_BIN_EXE_llg"), args);
        assert_eq!(output.status.code(), Some(2), "{output:?}");
        assert!(output.stdout.is_empty(), "{output:?}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(expected),
            "{output:?}"
        );
    }
}

#[test]
fn simulator_compilation_unit_mode_rejects_missing_and_unknown_values() {
    for (args, expected) in [
        (&["--compilation-units"][..], "requires separate or merged"),
        (
            &["--compilation-units", "grouped"][..],
            "expected separate or merged",
        ),
    ] {
        let output = invoke(env!("CARGO_BIN_EXE_llg"), args);
        assert_eq!(output.status.code(), Some(2), "{output:?}");
        assert!(output.stdout.is_empty(), "{output:?}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(expected),
            "{output:?}"
        );
    }
}

#[test]
fn simulator_delay_selection_rejects_missing_and_unknown_values() {
    for (args, expected) in [
        (
            &["--delay-select"][..],
            "--delay-select requires min, typ or max",
        ),
        (
            &["--delay-select", "typical"][..],
            "--delay-select: unsupported delay selection `typical` (expected min, typ or max)",
        ),
    ] {
        let output = invoke(env!("CARGO_BIN_EXE_llg"), args);
        assert_eq!(output.status.code(), Some(2), "{output:?}");
        assert!(output.stdout.is_empty(), "{output:?}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(expected),
            "{output:?}"
        );
    }
}

#[test]
fn simulator_include_and_define_options_require_values() {
    for (flag, expected) in [
        ("--include-dir", "--include-dir requires a path"),
        ("-I", "--include-dir requires a path"),
        ("--define", "--define requires NAME or NAME=VALUE"),
        ("-D", "--define requires NAME or NAME=VALUE"),
        (
            "--define-system-task",
            "--define-system-task requires a prototype",
        ),
    ] {
        let output = invoke(env!("CARGO_BIN_EXE_llg"), &[flag]);
        assert_eq!(output.status.code(), Some(2), "{output:?}");
        assert!(output.stdout.is_empty(), "{output:?}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(expected),
            "{output:?}"
        );
    }
}

#[cfg(feature = "lsp")]
#[test]
fn server_invalid_arguments_do_not_start_the_protocol() {
    for args in [
        vec!["--dump-tokens"],
        vec!["--staging-dir"],
        vec!["--stdio", "--stdio"],
        vec!["--stdio", "--dump-tokens", "."],
        vec!["--unknown"],
        vec!["--version", "extra"],
        vec!["--help", "extra"],
    ] {
        let output = invoke(env!("CARGO_BIN_EXE_llg_ls"), &args);
        assert_eq!(output.status.code(), Some(2), "{output:?}");
        assert!(output.stdout.is_empty(), "{output:?}");
        assert!(String::from_utf8_lossy(&output.stderr).contains("use --help"));
    }
}

#[test]
fn simulator_removed_options_are_usage_errors() {
    for option in [
        "--max-export-mib",
        "--lint",
        "--no-lint",
        "--lint-config",
        "-Wall",
    ] {
        let output = invoke(env!("CARGO_BIN_EXE_llg"), &[option, "4096", "design.sv"]);
        assert_eq!(output.status.code(), Some(2), "{output:?}");
        assert!(output.stdout.is_empty(), "{output:?}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(&format!("unknown option `{option}`")),
            "{output:?}"
        );
    }
}

#[test]
fn pulse_controls_reject_missing_and_invalid_values() {
    for (args, expected) in [
        (&["--pulse-reject"][..], "requires an integer from 0 to 100"),
        (
            &["--pulse-error", "101"][..],
            "--pulse-error: expected an integer from 0 to 100",
        ),
        (&["--pulse-style"][..], "requires onevent or ondetect"),
        (
            &["--pulse-style", "late"][..],
            "--pulse-style: expected onevent or ondetect",
        ),
    ] {
        let output = invoke(env!("CARGO_BIN_EXE_llg"), args);
        assert_eq!(output.status.code(), Some(2), "{output:?}");
        assert!(output.stdout.is_empty(), "{output:?}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(expected),
            "{output:?}"
        );
    }
}

/// Usage errors are catalog messages without a location, rendered through
/// the shared layout template.
#[test]
fn simulator_usage_errors_carry_a_catalog_id_without_location() {
    use llg::core::report::{self, catalog, Message, Severity};
    for (args, def, text) in [
        (
            &["--top"][..],
            &catalog::CLI_OPTION_VALUE,
            "--top requires a module name",
        ),
        (
            &["--no-such-option"][..],
            &catalog::CLI_UNKNOWN_OPTION,
            "unknown option `--no-such-option`",
        ),
    ] {
        let output = invoke(env!("CARGO_BIN_EXE_llg"), args);
        assert_eq!(output.status.code(), Some(2), "{output:?}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        let first = stderr.lines().next().unwrap_or_default();
        let message = report::parse(first).unwrap_or_else(|| panic!("unparsed: {stderr}"));
        assert_eq!(message.severity, Severity::Error, "{stderr}");
        assert_eq!(message.id, def.id(), "{stderr}");
        assert_eq!(message.location, None, "{stderr}");
        assert!(message.text.starts_with(text), "{stderr}");
        assert!(
            first.starts_with(Message::new(def, text).render().as_str()),
            "{stderr}"
        );
    }
}

/// A frontend error is a located `SLANG-<name>` message with its option
/// hint absent (errors have no -W option), followed by the SLANG phase
/// summary as the last line.
#[test]
fn simulator_frontend_errors_end_with_the_slang_summary() {
    use llg::core::report::{self, Severity};
    let directory =
        std::env::temp_dir().join(format!("llg-cli-slang-summary-{}", std::process::id()));
    std::fs::create_dir_all(&directory).expect("create temp dir");
    let source = directory.join("undeclared.sv");
    std::fs::write(&source, "module m;\n  initial x = 1;\nendmodule\n").expect("write source");
    let output = Command::new(env!("CARGO_BIN_EXE_llg"))
        .arg("--gen-only")
        .arg(&source)
        .current_dir(&directory)
        .output()
        .expect("run llg");
    let _ = std::fs::remove_dir_all(&directory);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    let messages: Vec<_> = stderr.lines().filter_map(report::parse).collect();
    assert_eq!(messages.len(), stderr.lines().count(), "{stderr}");
    let error = &messages[0];
    assert_eq!(error.severity, Severity::Error, "{stderr}");
    assert_eq!(error.id, "SLANG-UndeclaredIdentifier", "{stderr}");
    assert!(
        error
            .location
            .as_deref()
            .is_some_and(|l| l.ends_with("undeclared.sv:2:11")),
        "{stderr}"
    );
    let summary = messages.last().expect("summary");
    assert_eq!(summary.id, "SLANG-000", "{stderr}");
    assert_eq!(summary.severity, Severity::Info, "{stderr}");
    let counts = report::Counts {
        error: 1,
        ..Default::default()
    };
    assert_eq!(summary.text, counts.summary_text(), "{stderr}");
}
