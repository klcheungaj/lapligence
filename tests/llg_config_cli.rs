//! `llg.toml` support in the `llg` driver, through the public executable.
//!
//! Each test copies the checked-in `tests/fixtures/config_cli` project into an
//! isolated directory, writes a config file there and runs `llg` from it.

#[path = "support/sim.rs"]
mod sim_harness;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::Duration;

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/config_cli")
}

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("create fixture directory");
    for entry in std::fs::read_dir(from).expect("read fixture directory") {
        let entry = entry.expect("fixture entry");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("fixture type").is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).expect("copy fixture file");
        }
    }
}

/// An isolated project directory holding a copy of the fixture design.
struct Project {
    dir: sim_harness::TempDir,
}

impl Project {
    fn new() -> Self {
        let dir = sim_harness::TempDir::new("config-cli").expect("project directory");
        copy_tree(&fixture_root(), dir.path());
        Self { dir }
    }

    fn path(&self) -> &Path {
        self.dir.path()
    }

    fn write(&self, name: &str, text: &str) {
        let path = self.path().join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create parent");
        }
        std::fs::write(path, text).expect("write file");
    }

    fn run(&self, args: &[&str]) -> Output {
        self.run_in(self.path(), args)
    }

    fn run_in(&self, cwd: &Path, args: &[&str]) -> Output {
        assert!(
            llg::sim::build::cmake_available(),
            "CLI tests require CMake"
        );
        let mut command = Command::new(env!("CARGO_BIN_EXE_llg"));
        command.current_dir(cwd).args(args);
        sim_harness::run_command(&mut command, Duration::from_secs(180)).expect("run llg")
    }
}

/// Project config: sources come from `rtl/`, headers from `inc_a/`.
const BASE_CONFIG: &str = r#"schema_version = 1

[sources]
directories = ["rtl"]

[compile]
top = "tb"
include_dirs = ["inc_a"]
defines = ["FAST"]

[compile.param_overrides]
DEPTH = 8
"#;

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn assert_prints(output: &Output, expected: &str) {
    assert!(output.status.success(), "{}", stderr(output));
    assert_eq!(stdout(output), expected, "{}", stderr(output));
}

#[test]
fn llg_toml_in_the_current_directory_drives_a_run_without_arguments() {
    let project = Project::new();
    project.write("llg.toml", BASE_CONFIG);
    // top, include dir, define, parameter override and source directory all
    // come from the file.
    assert_prints(&project.run(&[]), "mode=fast depth=8 inc=1\n");
}

#[test]
fn command_line_values_override_every_kind_of_config_value() {
    let project = Project::new();
    project.write("llg.toml", BASE_CONFIG);
    // Command-line lists replace the config lists: the config `FAST` define,
    // `inc_a` include directory and DEPTH override do not survive, and the
    // named file replaces the discovered sources.
    let output = project.run(&[
        "-D",
        "UNRELATED=1",
        "-I",
        "inc_b",
        "--param-override",
        "DEPTH=16",
        "rtl/tb.sv",
    ]);
    assert_prints(&output, "mode=slow depth=16 inc=2\n");
    // A scalar replaces the config value.
    assert_prints(&project.run(&["--top", "other_tb"]), "other\n");
}

#[test]
fn explicit_config_resolves_paths_from_its_own_directory() {
    let project = Project::new();
    // The default llg.toml must be ignored when --config is given.
    project.write(
        "llg.toml",
        "schema_version = 1\n[compile]\ntop = \"other_tb\"\n",
    );
    project.write(
        "conf/custom.toml",
        r#"schema_version = 1
[sources]
files = ["../rtl/tb.sv"]
[compile]
top = "tb"
include_dirs = ["../inc_b"]
"#,
    );
    let other = project.path().join("elsewhere");
    std::fs::create_dir_all(&other).expect("create other directory");
    let config = project.path().join("conf/custom.toml");
    let output = project.run_in(&other, &["--config", config.to_str().unwrap()]);
    assert_prints(&output, "mode=slow depth=4 inc=2\n");
}

#[test]
fn missing_explicit_config_is_an_error_but_missing_default_is_not() {
    let project = Project::new();
    let output = project.run(&["--config", "nope.toml", "rtl/tb.sv"]);
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    assert!(stderr(&output).contains("nope.toml"), "{}", stderr(&output));
    assert!(
        stderr(&output).contains("does not exist"),
        "{}",
        stderr(&output)
    );
    // No llg.toml here: the run proceeds from the command line alone.
    let output = project.run(&["--top", "tb", "-I", "inc_a", "-D", "FAST", "rtl/tb.sv"]);
    assert_prints(&output, "mode=fast depth=4 inc=1\n");
}

#[test]
fn no_arguments_without_a_config_prints_usage() {
    let project = Project::new();
    let output = project.run(&[]);
    assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
    assert!(
        stderr(&output).contains("usage: llg"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn unknown_keys_and_invalid_values_name_the_file_and_key() {
    let project = Project::new();
    project.write("llg.toml", "schema_version = 1\n[compile]\ntopp = \"tb\"\n");
    let output = project.run(&["rtl/tb.sv"]);
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    let message = stderr(&output);
    assert!(message.contains("llg.toml"), "{message}");
    assert!(message.contains("compile.topp"), "{message}");
    assert!(message.contains("line 3"), "{message}");

    project.write(
        "llg.toml",
        "schema_version = 1\n[build]\nmodel_opt_level = \"O9\"\n",
    );
    let output = project.run(&["rtl/tb.sv"]);
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    assert!(
        stderr(&output).contains("build.model_opt_level"),
        "{}",
        stderr(&output)
    );

    project.write("llg.toml", "schema_version = 2\n");
    let output = project.run(&["rtl/tb.sv"]);
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    assert!(
        stderr(&output).contains("schema_version"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn language_server_only_keys_are_accepted_and_ignored() {
    let project = Project::new();
    project.write(
        "llg.toml",
        r#"schema_version = 1
[analysis]
max_file_bytes = 100
[lint]
enabled = false
[lint.rules.unused-signal]
severity = "error"
"#,
    );
    let output = project.run(&["--top", "tb", "-I", "inc_a", "rtl/tb.sv"]);
    assert_prints(&output, "mode=slow depth=4 inc=1\n");
}

#[test]
fn output_and_build_settings_from_the_config_apply_and_can_be_overridden() {
    let project = Project::new();
    project.write(
        "llg.toml",
        r#"schema_version = 1
[sources]
files = ["rtl/tb.sv"]
[compile]
top = "tb"
include_dirs = ["inc_a"]
[build]
gen_only = true
[output]
out_dir = "generated"
"#,
    );
    let output = project.run(&[]);
    assert!(output.status.success(), "{}", stderr(&output));
    let printed = PathBuf::from(stdout(&output).trim());
    assert!(
        printed.starts_with(project.path().join("generated").join("sim")),
        "model written below the configured out_dir: {printed:?}"
    );
    assert!(printed.join("CMakeLists.txt").is_file());
    assert!(
        !project.path().join("build").exists(),
        "the default output directory is unused"
    );

    // --no-gen-only runs the model; --out-dir replaces the configured one.
    let output = project.run(&["--no-gen-only", "--out-dir", "other_out"]);
    assert_prints(&output, "mode=slow depth=4 inc=1\n");
    assert!(project.path().join("other_out/sim").is_dir());
}

#[test]
fn config_lint_rules_apply_to_the_lint_gate() {
    let project = Project::new();
    project.write(
        "lint.sv",
        "module lint_top;\n  logic unused_sig;\n  initial $finish;\nendmodule\n",
    );
    project.write(
        "llg.toml",
        r#"schema_version = 1
[lint]
run = true
[lint.rules.unused-signal]
severity = "error"
"#,
    );
    let output = project.run(&["--top", "lint_top", "--gen-only", "lint.sv"]);
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    assert!(
        stderr(&output).contains("unused-signal"),
        "{}",
        stderr(&output)
    );
    // --no-lint overrides the file's run = true.
    let output = project.run(&["--no-lint", "--top", "lint_top", "--gen-only", "lint.sv"]);
    assert!(output.status.success(), "{}", stderr(&output));
}
