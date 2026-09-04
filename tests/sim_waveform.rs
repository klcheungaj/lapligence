//! End-to-end VCD/FST waveform tests.
//!
//! These cases exercise the complete Surelog -> IR -> generated C -> CMake
//! pipeline.  The runtime's lower-level self-test separately validates queue
//! wraparound and reopens FST output with GTKWave's official reader.

use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use llg::core::compile;
use llg::sim;

static SURELOG_LOCK: Mutex<()> = Mutex::new(());
static NEXT_DIR: AtomicU64 = AtomicU64::new(0);

fn fresh_dir(tag: &str) -> std::path::PathBuf {
    let sequence = NEXT_DIR.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "llg_sim_waveform_{tag}_{}_{}",
        std::process::id(),
        sequence
    ));
    std::fs::create_dir_all(&dir).expect("create waveform test directory");
    dir
}

fn run_waveform(sv: &str, tag: &str) -> Result<(std::path::PathBuf, String, Vec<String>), String> {
    let dir = fresh_dir(tag);
    let src = dir.join("tb.sv");
    std::fs::write(&src, sv).map_err(|error| format!("write source: {error}"))?;

    let original_dir = std::env::current_dir().map_err(|error| format!("current dir: {error}"))?;
    std::env::set_current_dir(&dir).map_err(|error| format!("chdir: {error}"))?;
    let result = (|| {
        let output = compile::compile_checked(&compile::CompileOpts {
            files: vec![src.to_string_lossy().into_owned()],
            top: Some("tb".to_string()),
            ..Default::default()
        })
        .map_err(|error| format!("compile: {error}"))?;
        let design = output.uhdm_design().ok_or("no UHDM design")?;
        let generated =
            sim::codegen::generate(design).map_err(|error| format!("codegen: {error}"))?;
        if !generated.model_c.contains("#define LLG_WAVEFORM 1") {
            return Err("waveform controls did not enable generated runtime support".to_string());
        }
        let exe = sim::build::build_model_cmake(&dir, &[("model.c", generated.model_c.as_str())])
            .map_err(|error| format!("cmake: {error}"))?;
        let process = Command::new(&exe)
            .current_dir(&dir)
            .output()
            .map_err(|error| format!("run {}: {error}", exe.display()))?;
        if !process.status.success() {
            return Err(format!(
                "simulator exited with {:?}: {}",
                process.status,
                String::from_utf8_lossy(&process.stderr)
            ));
        }
        Ok((
            dir.clone(),
            String::from_utf8_lossy(&process.stderr).into_owned(),
            generated.warnings,
        ))
    })();
    std::env::set_current_dir(original_dir).map_err(|error| format!("restore cwd: {error}"))?;
    result
}

#[test]
fn vcd_records_controls_four_state_real_and_final_changes() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = SURELOG_LOCK.lock().unwrap();
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
        #1 $finish;
    end

    final begin
        value = 4'b0011;
    end
endmodule
"#;

    let (dir, stderr, warnings) =
        run_waveform(sv, "vcd_controls").expect("VCD simulation should run");
    assert!(stderr.is_empty(), "unexpected simulator stderr: {stderr}");
    assert_eq!(
        warnings
            .iter()
            .filter(|warning| warning.contains("$dumpvars depth/scope filtering"))
            .count(),
        1,
        "a model should report the conservative dump selection exactly once: {warnings:?}"
    );
    let vcd = std::fs::read_to_string(dir.join("trace.vcd")).expect("read generated VCD");

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

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn fst_is_written_by_the_generated_model() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = SURELOG_LOCK.lock().unwrap();
    let sv = r#"`timescale 10ps/1ps
module tb;
    reg [7:0] value;
    initial begin
        $dumpfile("trace.fst");
        value = 8'h00;
        $dumpvars;
        #1 value = 8'ha5;
        #1 value = 8'h5a;
        $dumpflush;
        #1 $finish;
    end
endmodule
"#;

    let (dir, stderr, warnings) = run_waveform(sv, "fst").expect("FST simulation should run");
    assert!(stderr.is_empty(), "unexpected simulator stderr: {stderr}");
    assert!(
        warnings.is_empty(),
        "unexpected codegen warnings: {warnings:?}"
    );
    let metadata = std::fs::metadata(dir.join("trace.fst")).expect("generated FST metadata");
    assert!(
        metadata.len() > 64,
        "generated FST should contain hierarchy and values"
    );

    validate_generated_fst(&dir).expect("official FST reader should validate generated contents");

    let _ = std::fs::remove_dir_all(dir);
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
    let compile = Command::new(&compiler)
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
        .arg(&executable)
        .output()
        .map_err(|error| format!("run FST reader compiler `{compiler}`: {error}"))?;
    if !compile.status.success() {
        return Err(format!(
            "compile FST reader probe: {}",
            String::from_utf8_lossy(&compile.stderr)
        ));
    }
    let output = Command::new(&executable)
        .current_dir(dir)
        .output()
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
