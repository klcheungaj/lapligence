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
    assert!(String::from_utf8_lossy(&output.stdout).contains("--edition <2001|2009>"));
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("--compilation-units <separate|merged>")
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("--include-dir <path>"));
    assert!(String::from_utf8_lossy(&output.stdout).contains("--define <NAME[=VALUE]>"));
    assert!(String::from_utf8_lossy(&output.stdout).contains("--define-system-task <prototype>"));
    assert!(String::from_utf8_lossy(&output.stdout).contains("--stop-policy <resume|exit>"));
    assert!(String::from_utf8_lossy(&output.stdout).contains("--launcher"));
}

#[cfg(feature = "lsp")]
#[test]
fn server_information_exits_without_serving_or_logging() {
    check_information(env!("CARGO_BIN_EXE_llg_ls"), "llg_ls", "--dump-tokens");
}

#[test]
fn simulator_missing_option_value_is_a_usage_error() {
    for (option, diagnostic) in [
        ("--generator", "requires a backend name"),
        ("--launcher", "requires a program name"),
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
fn simulator_edition_option_rejects_missing_and_unknown_values() {
    for (args, expected) in [
        (&["--edition"][..], "requires 2001 or 2009"),
        (&["--edition", "2017"][..], "expected 2001 or 2009"),
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
