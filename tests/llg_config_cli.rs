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
        self.run_in_env(cwd, args, &[])
    }

    fn run_env(&self, args: &[&str], env: &[(&str, &str)]) -> Output {
        self.run_in_env(self.path(), args, env)
    }

    fn run_in_env(&self, cwd: &Path, args: &[&str], env: &[(&str, &str)]) -> Output {
        assert!(
            llg::sim::build::cmake_available(),
            "CLI tests require CMake"
        );
        let mut command = Command::new(env!("CARGO_BIN_EXE_llg"));
        command
            .current_dir(cwd)
            .args(args)
            .envs(env.iter().copied());
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
fn an_explicit_config_alone_drives_a_run() {
    let project = Project::new();
    project.write("llg.toml", BASE_CONFIG);
    // top, include dir, define, parameter override and source directory all
    // come from the file.
    assert_prints(
        &project.run(&["--config", "llg.toml"]),
        "mode=fast depth=8 inc=1\n",
    );
}

#[test]
fn llg_toml_in_the_current_directory_is_never_discovered() {
    let project = Project::new();
    project.write("llg.toml", BASE_CONFIG);
    // Without --config the file is ignored: no FAST define, no DEPTH override.
    let output = project.run(&["--top", "tb", "-I", "inc_a", "rtl/tb.sv"]);
    assert_prints(&output, "mode=slow depth=4 inc=1\n");
    // Not even a malformed file in the current directory is read.
    project.write("llg.toml", "schema_version = 99\nnot toml at all\n");
    let output = project.run(&["--top", "tb", "-I", "inc_a", "rtl/tb.sv"]);
    assert_prints(&output, "mode=slow depth=4 inc=1\n");
    // With nothing but the file present, no arguments still print usage.
    let output = project.run(&[]);
    assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
    assert!(
        stderr(&output).contains("usage: llg"),
        "{}",
        stderr(&output)
    );
    assert!(
        stderr(&output).contains("--config"),
        "usage names --config: {}",
        stderr(&output)
    );
}

/// The list-option design under test: command-line values append to the
/// config lists, `--clear <list>` discards the config values first.
const LISTS_ARGS: [&str; 4] = ["--config", "llg.toml", "--top", "lists_tb"];

#[test]
fn repeatable_options_append_to_the_config_lists() {
    let project = Project::new();
    project.write("llg.toml", BASE_CONFIG);
    // The config supplies FAST, DEPTH=8, include dir inc_a and the rtl sources;
    // the command line adds EXTRA, WIDTH=5, include dir inc_b and a source.
    let mut args = LISTS_ARGS.to_vec();
    args.extend([
        "-D",
        "EXTRA",
        "-G",
        "WIDTH=5",
        "-I",
        "inc_b",
        "extra/lists.sv",
    ]);
    let output = project.run(&args);
    assert!(output.status.success(), "{}", stderr(&output));
    let text = stdout(&output);
    assert!(
        text.contains("f=1 e=1 l=-1 depth=8 width=5 a=3 b=7\n"),
        "{text}"
    );
}

#[test]
fn clear_replaces_the_config_list_with_the_command_line_values() {
    let project = Project::new();
    project.write("llg.toml", BASE_CONFIG);
    // Defines and parameter overrides are replaced; the include directories
    // and sources still append.
    let mut args = LISTS_ARGS.to_vec();
    args.extend([
        "--clear",
        "defines",
        "--clear",
        "param-overrides",
        "-D",
        "EXTRA",
        "-I",
        "inc_b",
        "extra/lists.sv",
    ]);
    let output = project.run(&args);
    assert!(output.status.success(), "{}", stderr(&output));
    let text = stdout(&output);
    assert!(
        text.contains("f=0 e=1 l=-1 depth=1 width=1 a=3 b=7\n"),
        "{text}"
    );

    // Clearing include dirs drops inc_a: only_a.svh is no longer found.
    let mut args = LISTS_ARGS.to_vec();
    args.extend(["--clear", "include-dirs", "-I", "inc_b", "extra/lists.sv"]);
    let output = project.run(&args);
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    assert!(
        stderr(&output).contains("only_a.svh"),
        "{}",
        stderr(&output)
    );

    // Clearing sources drops the rtl directory: `tb` no longer exists.
    let output = project.run(&[
        "--config",
        "llg.toml",
        "--clear",
        "sources",
        "--top",
        "tb",
        "-I",
        "inc_a",
        "extra/lists.sv",
    ]);
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    assert!(stderr(&output).contains("tb"), "{}", stderr(&output));
    // ... and the same command with the remaining source works.
    let output = project.run(&[
        "--config",
        "llg.toml",
        "--clear",
        "sources,include-dirs",
        "--top",
        "lists_tb",
        "-I",
        "inc_a",
        "-I",
        "inc_b",
        "extra/lists.sv",
    ]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(
        stdout(&output).contains("f=1 e=0 l=-1 depth=8 width=1 a=3 b=7\n"),
        "{}",
        stdout(&output)
    );
}

#[test]
fn a_later_define_replaces_an_earlier_one_with_the_same_name() {
    let project = Project::new();
    project.write(
        "llg.toml",
        r#"schema_version = 1
[compile]
top = "lists_tb"
include_dirs = ["inc_a", "inc_b"]
defines = ["LEVEL=1"]
[sources]
files = ["extra/lists.sv"]
"#,
    );
    let output = project.run(&["--config", "llg.toml"]);
    assert!(
        stdout(&output).contains("l=1 "),
        "{}{}",
        stdout(&output),
        stderr(&output)
    );
    // The command-line value replaces the configured value for LEVEL.
    let output = project.run(&["--config", "llg.toml", "-D", "LEVEL=2"]);
    assert!(
        stdout(&output).contains("l=2 "),
        "{}{}",
        stdout(&output),
        stderr(&output)
    );
    // Repeating it on the command line: the last one wins.
    let output = project.run(&["--config", "llg.toml", "-D", "LEVEL=2", "-D", "LEVEL=3"]);
    assert!(
        stdout(&output).contains("l=3 "),
        "{}{}",
        stdout(&output),
        stderr(&output)
    );
}

#[test]
fn plusargs_append_to_the_config_plusargs_unless_cleared() {
    let project = Project::new();
    project.write(
        "llg.toml",
        r#"schema_version = 1
[compile]
top = "lists_tb"
include_dirs = ["inc_a", "inc_b"]
[sources]
files = ["extra/lists.sv"]
[simulator]
plusargs = ["+cfg"]
"#,
    );
    let output = project.run(&["--config", "llg.toml", "--", "+cli"]);
    assert!(
        stdout(&output).contains("pa_cfg=1 pa_cli=1\n"),
        "{}{}",
        stdout(&output),
        stderr(&output)
    );
    let output = project.run(&["--config", "llg.toml", "--clear", "plusargs", "--", "+cli"]);
    assert!(
        stdout(&output).contains("pa_cfg=0 pa_cli=1\n"),
        "{}{}",
        stdout(&output),
        stderr(&output)
    );
}

#[test]
fn clear_rejects_an_unknown_list() {
    let project = Project::new();
    let output = project.run(&["--clear", "nope", "rtl/tb.sv"]);
    assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
    assert!(stderr(&output).contains("nope"), "{}", stderr(&output));
    assert!(
        stderr(&output).contains("include-dirs"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn environment_overrides_the_config_and_the_command_line_overrides_both() {
    let project = Project::new();
    let missing = "llg-test-no-such-compiler";
    let args = ["--top", "tb", "-I", "inc_a", "rtl/tb.sv"];
    let with_config = |cc: &str| {
        project.write(
            "llg.toml",
            &format!("schema_version = 1\n[build]\ncc = \"{cc}\"\n"),
        );
    };

    // Config only: a valid compiler works, a missing one fails the build.
    with_config("cc");
    let mut command = vec!["--config", "llg.toml"];
    command.extend(args);
    let output = project.run_env(&command, &[("LLG_CC", "cc"), ("CC", "cc")]);
    assert_prints(&output, "mode=slow depth=4 inc=1\n");

    // $LLG_CC (missing) beats build.cc (valid): the build fails.
    let output = project.run_env(&command, &[("LLG_CC", missing)]);
    assert!(!output.status.success(), "{}", stdout(&output));
    assert!(stderr(&output).contains(missing), "{}", stderr(&output));

    // $LLG_CC (valid) beats build.cc (missing): the build works.
    with_config(missing);
    let output = project.run_env(&command, &[("LLG_CC", "cc")]);
    assert_prints(&output, "mode=slow depth=4 inc=1\n");

    // With only the config naming the missing compiler, the build fails.
    let output = project.run_env(&command, &[("LLG_CC", ""), ("CC", "")]);
    assert!(!output.status.success(), "{}", stdout(&output));
    assert!(stderr(&output).contains(missing), "{}", stderr(&output));

    // --cc beats both the environment (missing) and the config (missing).
    let mut with_cli = command.clone();
    with_cli.extend(["--cc", "cc"]);
    let output = project.run_env(&with_cli, &[("LLG_CC", missing)]);
    assert_prints(&output, "mode=slow depth=4 inc=1\n");
}

#[test]
fn command_line_values_override_every_kind_of_config_value() {
    let project = Project::new();
    project.write("llg.toml", BASE_CONFIG);
    // `--clear` drops the config lists, so the config `FAST` define, `inc_a`
    // include directory, DEPTH override and the rtl sources do not survive;
    // the named file is the only source.
    let output = project.run(&[
        "--config",
        "llg.toml",
        "--clear",
        "defines,include-dirs,param-overrides,sources",
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
    let output = project.run(&["--config", "llg.toml", "--top", "other_tb"]);
    assert_prints(&output, "other\n");
}

#[test]
fn explicit_config_resolves_paths_from_its_own_directory() {
    let project = Project::new();
    // The llg.toml in the current directory is not the file that applies.
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
fn a_missing_explicit_config_is_an_error() {
    let project = Project::new();
    let output = project.run(&["--config", "nope.toml", "rtl/tb.sv"]);
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    assert!(stderr(&output).contains("nope.toml"), "{}", stderr(&output));
    assert!(
        stderr(&output).contains("does not exist"),
        "{}",
        stderr(&output)
    );
    // Without --config no file is looked for: the command line suffices.
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
    let output = project.run(&["--config", "llg.toml", "rtl/tb.sv"]);
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    let message = stderr(&output);
    assert!(message.contains("llg.toml"), "{message}");
    assert!(message.contains("compile.topp"), "{message}");
    assert!(message.contains("line 3"), "{message}");

    project.write(
        "llg.toml",
        "schema_version = 1\n[build]\nmodel_opt_level = \"O9\"\n",
    );
    let output = project.run(&["--config", "llg.toml", "rtl/tb.sv"]);
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    assert!(
        stderr(&output).contains("build.model_opt_level"),
        "{}",
        stderr(&output)
    );

    project.write("llg.toml", "schema_version = 2\n");
    let output = project.run(&["--config", "llg.toml", "rtl/tb.sv"]);
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
    let output = project.run(&[
        "--config",
        "llg.toml",
        "--top",
        "tb",
        "-I",
        "inc_a",
        "rtl/tb.sv",
    ]);
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
    let output = project.run(&["--config", "llg.toml"]);
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
    let output = project.run(&[
        "--config",
        "llg.toml",
        "--no-gen-only",
        "--out-dir",
        "other_out",
    ]);
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
    let output = project.run(&[
        "--config",
        "llg.toml",
        "--top",
        "lint_top",
        "--gen-only",
        "lint.sv",
    ]);
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    assert!(
        stderr(&output).contains("unused-signal"),
        "{}",
        stderr(&output)
    );
    // --no-lint overrides the file's run = true.
    let output = project.run(&[
        "--config",
        "llg.toml",
        "--no-lint",
        "--top",
        "lint_top",
        "--gen-only",
        "lint.sv",
    ]);
    assert!(output.status.success(), "{}", stderr(&output));
}
