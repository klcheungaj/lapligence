use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use llg::core::compile::{self, CompileOpts};
use llg::core::db::Db;
use llg::sim;

use crate::c_compiler;
use crate::generated_c_lint;

const SHARDS: usize = 6;

fn compiler_available(compiler: &str) -> bool {
    Command::new(compiler)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

fn syntax_check(compiler: &str, model: &str, fixture: &Path, mode: &str) {
    use std::io::Write;

    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut command = Command::new(compiler);
    command.args([
        "-x",
        "c",
        "-std=c11",
        "-O2",
        "-Wall",
        "-Wno-unused-function",
        "-Wno-misleading-indentation",
    ]);
    if c_compiler::is_gnu_gcc(compiler) {
        command.arg("-Werror=jump-misses-init");
    }
    if mode.contains("compact") {
        command.arg("-DLLG_SV4_USE_GMP=1");
        command.arg(if mode.contains("gmp") {
            "-DLLG_SV4_GMP_KERNELS=1"
        } else {
            "-DLLG_SV4_GMP_KERNELS=0"
        });
    }
    if mode.contains("debug") {
        command.arg("-DLLG_CO_DEBUG");
    }
    let mut child = command
        .args(["-fsyntax-only", "-"])
        .arg(format!("-I{}", root.join("src/sim/rt").display()))
        .arg(format!(
            "-I{}",
            root.join("vendor/slang/external/ieee1800").display()
        ))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|error| {
            panic!(
                "start {compiler} for {} ({mode}): {error}",
                fixture.display()
            )
        });
    child
        .stdin
        .as_mut()
        .expect("compiler stdin")
        .write_all(model.as_bytes())
        .unwrap_or_else(|error| {
            panic!(
                "write {} ({mode}) to {compiler}: {error}",
                fixture.display()
            )
        });
    let output = child.wait_with_output().unwrap_or_else(|error| {
        panic!(
            "wait for {compiler} on {} ({mode}): {error}",
            fixture.display()
        )
    });
    assert!(
        output.status.success() && output.stderr.is_empty(),
        "{} ({mode}) failed or warned in {compiler} syntax check:\n{}",
        fixture.display(),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn fixture_paths(root: &Path) -> Vec<PathBuf> {
    fn visit(path: &Path, paths: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(path) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                visit(&path, paths);
            } else if matches!(
                path.extension().and_then(|ext| ext.to_str()),
                Some("sv" | "v")
            ) {
                paths.push(path);
            }
        }
    }

    let mut paths = Vec::new();
    visit(root, &mut paths);
    paths.sort();
    paths
}

fn lint_fixture_shard(shard: usize) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sim");
    let compilers = ["gcc", "clang"]
        .into_iter()
        .filter(|compiler| compiler_available(compiler))
        .collect::<Vec<_>>();
    let mut generated = 0usize;
    for (index, path) in fixture_paths(&root).into_iter().enumerate() {
        if index % SHARDS != shard {
            continue;
        }
        let Ok(compiled) = compile::compile(&CompileOpts {
            files: vec![path.to_string_lossy().into_owned()],
            ..Default::default()
        }) else {
            continue;
        };
        if !compiled.ok() {
            continue;
        }
        let Ok(database) = Db::from_slang(&compiled.snapshot) else {
            continue;
        };
        for (mode, options) in [
            ("default", sim::opt::OptConfig::default()),
            ("no-opt", sim::opt::OptConfig::none()),
        ] {
            let Ok(model) = sim::codegen::generate_from_db_with_opts(&database, &options) else {
                continue;
            };
            generated += 1;
            if let Err(errors) = generated_c_lint::lint_generated_coroutine_c(&model.model_c) {
                panic!(
                    "{} ({mode}) failed generated coroutine C lint:\n{}",
                    path.display(),
                    errors.join("\n")
                );
            }
            if let Err(errors) = generated_c_lint::lint_standard_identifiers(&model.model_c) {
                panic!(
                    "{} ({mode}) emitted a non-standard C identifier:\n{}",
                    path.display(),
                    errors.join("\n")
                );
            }
            for compiler in &compilers {
                syntax_check(compiler, &model.model_c, &path, mode);
            }
        }
    }
    assert!(generated > 0, "fixture shard {shard} generated no C models");
}

fn run_shard(shard: usize) {
    std::thread::Builder::new()
        .name(format!("generated-c-lint-{shard}"))
        .stack_size(32 * 1024 * 1024)
        .spawn(move || lint_fixture_shard(shard))
        .expect("spawn generated-C lint worker")
        .join()
        .expect("generated-C lint worker panicked");
}

#[test]
fn fixture_sweep_0() {
    run_shard(0);
}

#[test]
fn fixture_sweep_1() {
    run_shard(1);
}

#[test]
fn fixture_sweep_2() {
    run_shard(2);
}

#[test]
fn fixture_sweep_3() {
    run_shard(3);
}

#[test]
fn fixture_sweep_4() {
    run_shard(4);
}

#[test]
fn fixture_sweep_5() {
    run_shard(5);
}

#[test]
fn debug_frame_overlay_fixtures() {
    std::thread::Builder::new()
        .name("debug-frame-overlays".to_owned())
        .stack_size(32 * 1024 * 1024)
        .spawn(|| {
            let root = Path::new(env!("CARGO_MANIFEST_DIR"));
            for fixture in [
                "nested_control_flow",
                "nested_timing_calls",
                "copyback_once_cancel",
                "deep_cancellation",
            ] {
                let path = root
                    .join("tests/fixtures/sim/coroutine_semantics")
                    .join(format!("{fixture}.sv"));
                let compiled = compile::compile_checked(&CompileOpts {
                    files: vec![path.to_string_lossy().into_owned()],
                    ..Default::default()
                })
                .expect("compile overlay fixture");
                let database = Db::from_slang(&compiled.snapshot).unwrap();
                for options in [sim::opt::OptConfig::default(), sim::opt::OptConfig::none()] {
                    let model =
                        sim::codegen::generate_from_db_with_opts(&database, &options).unwrap();
                    generated_c_lint::lint_generated_coroutine_c(&model.model_c).unwrap();
                    for compiler in ["gcc", "clang"] {
                        assert!(compiler_available(compiler), "missing {compiler}");
                        syntax_check(compiler, &model.model_c, &path, "release");
                        syntax_check(compiler, &model.model_c, &path, "debug");
                    }
                }
            }
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn compact_selected_frame_lint() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for path in [
        "tests/fixtures/sim/value_backends/implemented.sv",
        "tests/fixtures/sim/instance_sharing/identities.sv",
        "tests/fixtures/sim/net_resolution/mixed_biased_structural.sv",
        "tests/fixtures/sim/net_partition/ranges.sv",
        "tests/fixtures/sim/net_partition/runtime.sv",
    ] {
        let fixture = root.join(path);
        let compiled = compile::compile_checked(&CompileOpts {
            files: vec![fixture.to_string_lossy().into_owned()],
            top: Some("tb".into()),
            ..Default::default()
        })
        .unwrap();
        let database = Db::from_slang(&compiled.snapshot).unwrap();
        for (kernel, mode) in [
            (
                sim::value_backend::CompactKernel::Portable,
                "compact-portable",
            ),
            (sim::value_backend::CompactKernel::Gmp, "compact-gmp"),
        ] {
            for optimization in [sim::opt::OptConfig::default(), sim::opt::OptConfig::none()] {
                let model = sim::codegen::generate_from_db_with_codegen_options(
                    &database,
                    &sim::codegen::CodegenOptions {
                        value_config: sim::value_backend::ValueConfig {
                            backend: sim::value_backend::ValueBackend::Compact,
                            kernel,
                        },
                        optimization,
                        ..Default::default()
                    },
                )
                .unwrap();
                generated_c_lint::lint_generated_coroutine_c(&model.model_c).unwrap();
                for compiler in ["gcc", "clang"]
                    .into_iter()
                    .filter(|compiler| compiler_available(compiler))
                {
                    syntax_check(compiler, &model.model_c, &fixture, mode);
                }
            }
        }
    }
}

#[test]
fn electrical_net_partition_fixtures() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sim");
    let compilers = ["gcc", "clang"]
        .into_iter()
        .filter(|compiler| compiler_available(compiler))
        .collect::<Vec<_>>();
    assert!(
        !compilers.is_empty(),
        "electrical frame lint requires a C compiler"
    );
    for fixture in [
        "net_partition/ranges.sv",
        "net_partition/runtime.sv",
        "waveform/partitioned_nets.sv",
        "continuation_20_23/continuous_contexts.sv",
    ] {
        let path = root.join(fixture);
        let compiled = compile::compile_checked(&CompileOpts {
            files: vec![path.to_string_lossy().into_owned()],
            top: Some("tb".to_owned()),
            ..Default::default()
        })
        .unwrap();
        let database = Db::from_slang(&compiled.snapshot).unwrap();
        for (mode, options) in [
            ("default", sim::opt::OptConfig::default()),
            ("no-opt", sim::opt::OptConfig::none()),
        ] {
            let model = sim::codegen::generate_from_db_with_opts(&database, &options).unwrap();
            generated_c_lint::lint_generated_coroutine_c(&model.model_c).unwrap();
            generated_c_lint::lint_standard_identifiers(&model.model_c).unwrap();
            for compiler in &compilers {
                syntax_check(compiler, &model.model_c, &path, mode);
            }
        }
    }
}
