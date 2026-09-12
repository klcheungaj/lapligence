//! End-to-end VCD/FST waveform tests.
//!
//! These cases exercise the complete Slang -> semantic IR -> execution IR -> CMake
//! pipeline.  The runtime's lower-level self-test separately validates queue
//! wraparound and reopens FST output with GTKWave's official reader.

use std::process::Command;
#[path = "support/sim.rs"]
mod sim_harness;
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;

use llg::core::compile;
use llg::sim;

static CWD_LOCK: Mutex<()> = Mutex::new(());
fn run_waveform(
    sv: &str,
    tag: &str,
) -> Result<(sim_harness::TempDir, String, Vec<String>), String> {
    run_waveform_with_opts(sv, tag, &sim::opt::OptConfig::default())
}

fn run_waveform_with_opts(
    sv: &str,
    tag: &str,
    opts: &sim::opt::OptConfig,
) -> Result<(sim_harness::TempDir, String, Vec<String>), String> {
    let dir = sim_harness::TempDir::new(tag)?;
    let src = dir.path().join("tb.sv");
    std::fs::write(&src, sv).map_err(|error| format!("write source: {error}"))?;

    let (stderr, warnings) = sim_harness::with_cwd(dir.path(), || {
        let output = compile::compile_checked(&compile::CompileOpts {
            files: vec![src.to_string_lossy().into_owned()],
            top: Some("tb".to_string()),
            ..Default::default()
        })
        .map_err(|error| format!("compile: {error}"))?;
        let db = llg::core::db::Db::from_slang(&output.snapshot)
            .map_err(|error| format!("db: {error}"))?;
        let generated = sim::codegen::generate_with_opts(&db, opts)
            .map_err(|error| format!("codegen: {error}"))?;
        if !generated.model_c.contains("#define LLG_WAVEFORM 1") {
            return Err("waveform controls did not enable generated runtime support".to_string());
        }
        let exe =
            sim::build::build_model_cmake(dir.path(), &[("model.c", generated.model_c.as_str())])
                .map_err(|error| format!("cmake: {error}"))?;
        let process = sim_harness::run_command(
            Command::new(&exe).current_dir(dir.path()),
            Duration::from_secs(60),
        )?;
        if !process.status.success() {
            return Err(format!(
                "simulator exited with {:?}: {}",
                process.status,
                String::from_utf8_lossy(&process.stderr)
            ));
        }
        Ok((
            String::from_utf8_lossy(&process.stderr).into_owned(),
            generated.warnings,
        ))
    })?;
    Ok((dir, stderr, warnings))
}

fn run_checked_in_fixture(
    fixture: &str,
    optimized: bool,
) -> Result<(sim_harness::TempDir, std::process::Output), String> {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/waveform")
        .join(format!("{fixture}.sv"));
    if !source.is_file() {
        return Err(format!("missing waveform fixture {}", source.display()));
    }
    let dir = sim_harness::TempDir::new(&format!("waveform-{fixture}"))?;
    let mut command = Command::new(env!("CARGO_BIN_EXE_llg"));
    command.current_dir(dir.path()).args(["--top", "tb"]);
    if !optimized {
        command.arg("--no-opt");
    }
    command.arg(source);
    let output = sim_harness::run_command(&mut command, Duration::from_secs(180))
        .map_err(|error| format!("run {fixture}: {error}"))?;
    Ok((dir, output))
}

fn read_fixture_vcd(fixture: &str, optimized: bool) -> (sim_harness::TempDir, String) {
    let (dir, output) = run_checked_in_fixture(fixture, optimized)
        .unwrap_or_else(|error| panic!("{fixture}, optimized={optimized}: {error}"));
    assert!(
        output.status.success(),
        "{fixture}, optimized={optimized}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stdout.is_empty(),
        "{fixture}, optimized={optimized} wrote stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    let diagnostics = String::from_utf8_lossy(&output.stderr);
    let runtime_diagnostics = diagnostics
        .lines()
        .filter(|line| !line.starts_with("Warning: "))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        runtime_diagnostics.is_empty(),
        "{fixture}, optimized={optimized} wrote runtime diagnostics: {runtime_diagnostics}"
    );
    let path = dir.path().join("trace.vcd");
    let vcd = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    (dir, vcd)
}

fn vcd_declarations(vcd: &str) -> BTreeMap<String, (String, String, String)> {
    let mut scopes: Vec<&str> = Vec::new();
    let mut declarations = BTreeMap::new();
    for line in vcd.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        match fields.as_slice() {
            ["$scope", "module", name, "$end"] => scopes.push(name),
            ["$upscope", "$end"] => {
                scopes.pop().expect("VCD upscope without matching scope");
            }
            ["$var", kind, width, id, name, "$end"] => {
                let path = if scopes.is_empty() {
                    (*name).to_owned()
                } else {
                    format!("{}.{}", scopes.join("."), name)
                };
                declarations.insert(
                    path,
                    ((*kind).to_owned(), (*width).to_owned(), (*id).to_owned()),
                );
            }
            _ => {}
        }
    }
    assert!(scopes.is_empty(), "VCD hierarchy was not balanced");
    declarations
}

fn assert_vcd_names(vcd: &str, expected: &[&str]) -> BTreeMap<String, (String, String, String)> {
    let declarations = vcd_declarations(vcd);
    let actual: Vec<&str> = declarations.keys().map(String::as_str).collect();
    let mut expected = expected.to_vec();
    expected.sort_unstable();
    assert_eq!(actual, expected, "unexpected VCD catalog: {vcd}");
    declarations
}

#[test]
fn vcd_records_controls_four_state_real_and_final_changes() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = CWD_LOCK.lock().unwrap();
    let sv = r#"`timescale 1ns/1ps
module wave_child;
    reg \a.b ;
    initial \a.b = 1'b0;
endmodule

module tb;
    reg [3:0] value;
    real analog;
    reg \a@b ;
    wave_child u();

    initial begin
        $dumpfile("trace.vcd");
        value = 4'bxxxx;
        analog = 1.25;
        \a@b = 1'b1;
        $dumpvars(0, tb);
        $dumpvars(0, tb);
        $dumplimit(1000000);
        #1 value = 4'b10z1;
        analog = 2.5;
        #1 $dumpoff;
        value = 4'b0110;
        #1 $dumpon;
        value = 4'b1100;
        #1 $dumpall;
        $dumpflush;
        #1 $finish(0);
    end

    final begin
        value = 4'b0011;
    end
endmodule
"#;

    let (dir, stderr, warnings) =
        run_waveform(sv, "vcd_controls").expect("VCD simulation should run");
    assert!(stderr.is_empty(), "unexpected simulator stderr: {stderr}");
    assert!(
        warnings.is_empty(),
        "explicit dump selection should not emit a filtering warning: {warnings:?}"
    );
    let vcd = std::fs::read_to_string(dir.path().join("trace.vcd")).expect("read generated VCD");

    assert!(vcd.contains("$timescale 1ps $end"));
    assert!(vcd.contains("$date\n  reproducible build\n$end"));
    assert_eq!(vcd.matches("$scope module tb $end").count(), 1);
    assert!(vcd.contains(" analog $end"));
    assert!(vcd.contains(" value $end"));
    assert!(vcd.contains("a$2Eb $end"));
    assert!(vcd.contains("a$40b $end"));
    assert!(
        !vcd.contains("$scope module a $end"),
        "a dot inside an escaped identifier must not create a false scope"
    );
    assert!(vcd.contains("$dumpvars\n"));
    assert!(vcd.contains("$dumpoff\n"));
    assert!(vcd.contains("$dumpon\n"));
    assert!(vcd.contains("$dumpall\n"));
    assert!(vcd.contains("#1000\n"));
    assert!(
        vcd.contains("#5000\n"),
        "final change must retain the scheduler end time"
    );
    assert!(vcd.contains("b10z1 "));
    assert!(vcd.contains("b0011 "), "final-block value must be dumped");
    assert!(vcd.contains("r2.5 "));
}

#[test]
fn vcd_dumpvars_selection_uses_depth_names_and_declared_indices_in_both_modes() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = CWD_LOCK.lock().unwrap();
    let sv = r#"`timescale 1ns/1ps
module child;
    reg child_value;
    initial child_value = 1'b1;
endmodule

module tb;
    reg [3:0] selected;
    reg [3:0] omitted;
    reg [7:0] memory [3:2];
    child u();

    initial begin
        $dumpfile("trace.vcd");
        selected = 4'h1;
        omitted = 4'h2;
        memory[3] = 8'ha3;
        memory[2] = 8'ha2;
        $dumpvars(1, tb);
        #1 selected = 4'hf;
        #1 $dumpflush;
        #1 $finish(0);
    end
endmodule
"#;

    for (optimized, opts) in [
        (true, sim::opt::OptConfig::default()),
        (false, sim::opt::OptConfig::none()),
    ] {
        let tag = if optimized {
            "vcd_dumpvars_depth_opt"
        } else {
            "vcd_dumpvars_depth_no_opt"
        };
        let (dir, stderr, warnings) =
            run_waveform_with_opts(sv, tag, &opts).expect("selected VCD simulation should run");
        assert!(stderr.is_empty(), "unexpected simulator stderr: {stderr}");
        assert!(
            warnings.is_empty(),
            "unexpected lowering warnings: {warnings:?}"
        );
        let vcd = std::fs::read_to_string(dir.path().join("trace.vcd")).expect("read selected VCD");
        assert!(
            vcd.contains(" selected $end"),
            "selected signal missing: {vcd}"
        );
        assert!(
            vcd.contains(" omitted $end"),
            "direct signal missing: {vcd}"
        );
        assert!(
            vcd.contains("memory$5B3$5D $end"),
            "declared index 3 missing: {vcd}"
        );
        assert!(
            vcd.contains("memory$5B2$5D $end"),
            "declared index 2 missing: {vcd}"
        );
        assert!(
            !vcd.contains("$scope module u $end") && !vcd.contains(" child_value $end"),
            "finite depth must exclude the child hierarchy: {vcd}"
        );
    }
}

#[test]
fn vcd_dumpvars_named_storage_excludes_unselected_catalog_entries() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = CWD_LOCK.lock().unwrap();
    let sv = r#"`timescale 1ns/1ps
module tb;
    reg [3:0] selected;
    reg [3:0] omitted;
    initial begin
        $dumpfile("trace.vcd");
        selected = 4'h1;
        omitted = 4'h2;
        $dumpvars(0, tb.selected);
        #1 selected = 4'he;
        #1 $dumpflush;
        #1 $finish(0);
    end
endmodule
"#;

    for (optimized, opts) in [
        (true, sim::opt::OptConfig::default()),
        (false, sim::opt::OptConfig::none()),
    ] {
        let tag = if optimized {
            "vcd_dumpvars_named_opt"
        } else {
            "vcd_dumpvars_named_no_opt"
        };
        let (dir, stderr, warnings) =
            run_waveform_with_opts(sv, tag, &opts).expect("named VCD simulation should run");
        assert!(stderr.is_empty(), "unexpected simulator stderr: {stderr}");
        assert!(
            warnings.is_empty(),
            "unexpected lowering warnings: {warnings:?}"
        );
        let vcd = std::fs::read_to_string(dir.path().join("trace.vcd")).expect("read named VCD");
        assert!(
            vcd.contains(" selected $end"),
            "named signal missing: {vcd}"
        );
        assert!(
            !vcd.contains(" omitted $end"),
            "unselected signal was dumped: {vcd}"
        );
    }
}

#[test]
fn checked_in_vcd_dumpvars_finite_depth_has_an_exact_catalog() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    for optimized in [true, false] {
        let (_dir, vcd) = read_fixture_vcd("depth", optimized);
        let declarations = assert_vcd_names(
            &vcd,
            &[
                "tb.memory$5B2$5D",
                "tb.memory$5B3$5D",
                "tb.omitted",
                "tb.selected",
            ],
        );
        assert_eq!(declarations["tb.selected"].0, "wire");
        assert_eq!(declarations["tb.selected"].1, "4");
        assert!(
            !vcd.contains("child_value $end") && !vcd.contains("leaf_value $end"),
            "finite depth leaked a descendant: {vcd}"
        );
        assert!(
            !vcd.contains("$scope module child $end"),
            "finite depth leaked a child scope: {vcd}"
        );
        assert!(
            vcd.contains("b10100011 "),
            "memory[3] initial value missing"
        );
        assert!(
            vcd.contains("b10100010 "),
            "memory[2] initial value missing"
        );
    }
}

#[test]
fn checked_in_vcd_dumpvars_unlimited_preserves_identities_and_types() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    for optimized in [true, false] {
        let (_dir, vcd) = read_fixture_vcd("unlimited", optimized);
        let declarations = assert_vcd_names(
            &vcd,
            &[
                "tb.a$24b",
                "tb.a$2Db",
                "tb.a$2Eb",
                "tb.a_b",
                "tb.analog",
                "tb.memory$5B2$5D",
                "tb.memory$5B3$5D",
                "tb.sampled",
                "tb.u.child_value",
                "tb.hier$2Edot.child_value",
            ],
        );
        assert_eq!(declarations["tb.analog"].0, "real");
        assert_eq!(declarations["tb.sampled"].0, "real");
        assert_eq!(declarations["tb.analog"].1, "64");
        assert_eq!(declarations["tb.sampled"].1, "64");
        assert!(vcd.contains("$scope module hier$2Edot $end"));
        assert_eq!(vcd.matches("$scope module hier$2Edot $end").count(), 1);
        assert!(vcd.contains("r2.5 "), "real value change missing: {vcd}");
        assert!(vcd.contains("b10100011 "), "array[3] value missing: {vcd}");
        assert!(vcd.contains("b10100010 "), "array[2] value missing: {vcd}");
    }
}

#[test]
fn checked_in_vcd_dumpvars_named_selection_is_lossless() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    for optimized in [true, false] {
        let (_dir, vcd) = read_fixture_vcd("named", optimized);
        let declarations = assert_vcd_names(
            &vcd,
            &["tb.memory$5B2$5D", "tb.memory$5B3$5D", "tb.selected"],
        );
        assert_eq!(declarations["tb.selected"].1, "4");
        assert_eq!(declarations["tb.memory$5B3$5D"].1, "8");
        assert!(
            !vcd.contains(" omitted $end"),
            "unselected scalar leaked: {vcd}"
        );
        assert!(
            vcd.contains("b10100011 "),
            "selected array element missing: {vcd}"
        );
    }
}

#[test]
fn checked_in_vcd_dumpvars_aliases_share_a_value_identity() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    for optimized in [true, false] {
        let (_dir, vcd) = read_fixture_vcd("aliases", optimized);
        let declarations = assert_vcd_names(&vcd, &["tb.child.value", "tb.value"]);
        assert_eq!(
            declarations["tb.child.value"].2, declarations["tb.value"].2,
            "reference aliases must use one VCD identifier: {vcd}"
        );
        assert!(vcd.contains("b0011 "), "initial alias value missing: {vcd}");
        assert!(vcd.contains("b1100 "), "child alias value missing: {vcd}");
    }
}

#[test]
fn checked_in_vcd_controls_are_identical_in_both_optimizer_modes() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    for optimized in [true, false] {
        let (_dir, vcd) = read_fixture_vcd("controls", optimized);
        assert!(vcd.contains("$timescale 1ps $end"));
        assert!(vcd.contains("$date\n  reproducible build\n$end"));
        assert_eq!(vcd.matches("$scope module tb $end").count(), 1);
        assert!(vcd.contains("$dumpvars\n"));
        assert!(vcd.contains("$dumpoff\n"));
        assert!(vcd.contains("$dumpon\n"));
        assert!(vcd.contains("$dumpall\n"));
        assert!(vcd.contains("#1000\n"));
        assert!(vcd.contains("#5000\n"));
        assert!(vcd.contains("b10z1 "));
        assert!(vcd.contains("b0011 "));
        assert!(vcd.contains("r2.5 "));
    }
}

#[test]
fn fst_is_written_by_the_generated_model() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = CWD_LOCK.lock().unwrap();

    for optimized in [true, false] {
        let (dir, output) = run_checked_in_fixture("fst", optimized)
            .unwrap_or_else(|error| panic!("fst, optimized={optimized}: {error}"));
        assert!(
            output.status.success(),
            "fst, optimized={optimized}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            output.stdout.is_empty(),
            "fst, optimized={optimized} wrote stdout: {}",
            String::from_utf8_lossy(&output.stdout),
        );
        let diagnostics = String::from_utf8_lossy(&output.stderr);
        let runtime_diagnostics = diagnostics
            .lines()
            .filter(|line| !line.starts_with("Warning: "))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            runtime_diagnostics.is_empty(),
            "fst, optimized={optimized} wrote runtime diagnostics: {runtime_diagnostics}"
        );
        let metadata =
            std::fs::metadata(dir.path().join("trace.fst")).expect("generated FST metadata");
        assert!(
            metadata.len() > 64,
            "generated FST should contain hierarchy and values"
        );

        for (name, source) in llg::sim::rt::waveform_sources() {
            std::fs::write(dir.path().join(name), source)
                .unwrap_or_else(|error| panic!("write FST reader source {name}: {error}"));
        }

        validate_generated_fst(dir.path())
            .expect("official FST reader should validate generated contents");
    }
}

#[cfg(not(windows))]
fn validate_generated_fst(dir: &std::path::Path) -> Result<(), String> {
    let probe = r#"#include "fstapi.h"
#include <stdint.h>
#include <stdio.h>
#include <string.h>

int main(void) {
    void* reader = fstReaderOpen("trace.fst");
    if (!reader) return 2;
    int saw_tb = 0;
    int saw_value = 0;
    fstHandle value_handle = 0;
    struct fstHier* item;
    while ((item = fstReaderIterateHier(reader)) != NULL) {
        if (item->htyp == FST_HT_SCOPE && strcmp(item->u.scope.name, "tb") == 0)
            saw_tb = 1;
        if (item->htyp == FST_HT_VAR && strcmp(item->u.var.name, "value") == 0) {
            saw_value = 1;
            value_handle = item->u.var.handle;
        }
    }
    char at_10[32];
    char at_20[32];
    if (!value_handle ||
        !fstReaderGetValueFromHandleAtTime(reader, 10, value_handle, at_10) ||
        !fstReaderGetValueFromHandleAtTime(reader, 20, value_handle, at_20)) {
        fstReaderClose(reader);
        return 3;
    }
    int valid = saw_tb && saw_value && fstReaderGetVarCount(reader) == 1 &&
                fstReaderGetEndTime(reader) == 30 &&
                strcmp(fstReaderGetDateString(reader), "reproducible build") == 0 &&
                strcmp(at_10, "10100101") == 0 &&
                strcmp(at_20, "01011010") == 0;
    fstReaderClose(reader);
    return valid ? 0 : 4;
}
"#;
    let probe_path = dir.join("fst_probe.c");
    std::fs::write(&probe_path, probe).map_err(|error| format!("write FST probe: {error}"))?;
    let compiler = std::env::var("LLG_CC")
        .or_else(|_| std::env::var("CC"))
        .unwrap_or_else(|_| "cc".to_string());
    let executable = dir.join("fst_probe");
    let compile = sim_harness::run_command(
        Command::new(&compiler)
            .current_dir(dir)
            .args([
                "-std=c11",
                "-DFST_CONFIG_INCLUDE=\"fst_config.h\"",
                "-I.",
                "fst_probe.c",
                "fstapi.c",
                "fastlz.c",
                "lz4.c",
                "-lz",
                "-lm",
                "-o",
            ])
            .arg(&executable),
        Duration::from_secs(60),
    )
    .map_err(|error| format!("run FST reader compiler `{compiler}`: {error}"))?;
    if !compile.status.success() {
        return Err(format!(
            "compile FST reader probe: {}",
            String::from_utf8_lossy(&compile.stderr)
        ));
    }
    let output = sim_harness::run_command(
        Command::new(&executable).current_dir(dir),
        Duration::from_secs(60),
    )
    .map_err(|error| format!("run FST reader probe: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "FST reader rejected hierarchy/timed values with {:?}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}

#[cfg(windows)]
fn validate_generated_fst(_dir: &std::path::Path) -> Result<(), String> {
    // The broader generated-model toolchain is not yet validated under native
    // Windows. The portable runtime self-test exercises the reader there once
    // that CI lane is enabled.
    Ok(())
}
