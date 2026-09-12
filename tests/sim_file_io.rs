//! End-to-end file descriptor, multichannel output, and file-control coverage.

#[path = "support/sim.rs"]
mod sim_harness;

use std::fs;

use llg::core::{compile, db::Db};
use llg::sim::{self, opt::OptConfig};

#[test]
fn file_output_and_controls_match_in_both_optimizer_modes() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    sim_harness::with_frontend_temp_cwd("file-output", |dir| {
        let source = dir.join("file_output.sv");
        let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/sim/file_io/file_output.sv");
        fs::copy(&fixture, &source).map_err(|error| error.to_string())?;
        let compiled = compile::compile_checked(&compile::CompileOpts {
            files: vec![source.to_string_lossy().into_owned()],
            top: Some("tb".to_owned()),
            ..Default::default()
        })
        .map_err(|error| error.to_string())?;
        let db = Db::from_slang(&compiled.snapshot).map_err(|error| error.to_string())?;
        let expected_stdout = "fd=4\n fanout=9\ntell=27\nseek=0\nrewound=0\nerror=0 message= eof=0\nclosed=1 message=invalid or closed file descriptor\n";
        let expected_file = "line=7\ntail=ab fanout=9\nab\n";
        for (name, options) in [("optimized", OptConfig::default()), ("unoptimized", OptConfig::none())] {
            let output_dir = dir.join(name);
            let model = sim::codegen::generate_from_db_with_opts(&db, &options)
                .map_err(|error| error.to_string())?;
            let executable = sim::build::build_model_cmake(
                &output_dir,
                &[("model.c", model.model_c.as_str())],
            )
            .map_err(|error| error.to_string())?;
            let output = sim_harness::with_cwd(&output_dir, || {
                sim_harness::run_executable(&executable)
            })?;
            assert_eq!(output, expected_stdout, "{name} stdout");
            assert_eq!(
                fs::read_to_string(output_dir.join("file_output.txt"))
                    .map_err(|error| error.to_string())?,
                expected_file,
                "{name} file output"
            );
            assert_eq!(
                fs::read_to_string(output_dir.join("final_output.txt"))
                    .map_err(|error| error.to_string())?,
                "final\n",
                "{name} final file output"
            );
        }
        Ok(())
    })
    .expect("file output simulation");
}

#[test]
fn file_input_operations_match_in_both_optimizer_modes() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    sim_harness::with_frontend_temp_cwd("file-input", |dir| {
        let source = dir.join("file_input.sv");
        let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/sim/file_io/file_input.sv");
        fs::copy(&fixture, &source).map_err(|error| error.to_string())?;
        let compiled = compile::compile_checked(&compile::CompileOpts {
            files: vec![source.to_string_lossy().into_owned()],
            top: Some("tb".to_owned()),
            ..Default::default()
        })
        .map_err(|error| error.to_string())?;
        let db = Db::from_slang(&compiled.snapshot).map_err(|error| error.to_string())?;
        let expected_stdout = "packed_line=0041206c696e650a bytes=7\n".to_owned()
            + "scan=3 decimal=123 hexadecimal=001x word=word\n"
            + "sscanf=2 decimal=42 hexadecimal=00xz\n"
            + "selected=aa,bb bytes=2\n"
            + "wide=123456 bytes=3\n"
            + "descending=xx,12,34,xx bytes=2\n"
            + "ascending=12,34,56,78 bytes=4\n";
        for (name, options) in [
            ("optimized", OptConfig::default()),
            ("unoptimized", OptConfig::none()),
        ] {
            let output_dir = dir.join(name);
            let model = sim::codegen::generate_from_db_with_opts(&db, &options)
                .map_err(|error| error.to_string())?;
            let executable =
                sim::build::build_model_cmake(&output_dir, &[("model.c", model.model_c.as_str())])
                    .map_err(|error| error.to_string())?;
            fs::write(
                output_dir.join("file_input.txt"),
                b"A line\n123 1x skip word\n",
            )
            .map_err(|error| error.to_string())?;
            fs::write(
                output_dir.join("file_input.bin"),
                [0x12_u8, 0x34, 0x56, 0x78, 0x9a, 0xbc],
            )
            .map_err(|error| error.to_string())?;
            let output =
                sim_harness::with_cwd(&output_dir, || sim_harness::run_executable(&executable))?;
            assert_eq!(output, expected_stdout, "{name} stdout");
        }
        Ok(())
    })
    .expect("file input simulation");
}

#[test]
fn deferred_file_output_keeps_owned_args_until_postponed_region() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    sim_harness::with_frontend_temp_cwd("deferred-file-output", |dir| {
        let source = dir.join("deferred_file_output.sv");
        let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/sim/file_io/deferred_file_output.sv");
        fs::copy(&fixture, &source).map_err(|error| error.to_string())?;
        let compiled = compile::compile_checked(&compile::CompileOpts {
            files: vec![source.to_string_lossy().into_owned()],
            top: Some("tb".to_owned()),
            ..Default::default()
        })
        .map_err(|error| error.to_string())?;
        let db = Db::from_slang(&compiled.snapshot).map_err(|error| error.to_string())?;
        let expected_file = "strobe=0010\nmonitor=2\n";
        for (name, options) in [
            ("optimized", OptConfig::default()),
            ("unoptimized", OptConfig::none()),
        ] {
            let output_dir = dir.join(name);
            let model = sim::codegen::generate_from_db_with_opts(&db, &options)
                .map_err(|error| error.to_string())?;
            let executable =
                sim::build::build_model_cmake(&output_dir, &[("model.c", model.model_c.as_str())])
                    .map_err(|error| error.to_string())?;
            let output =
                sim_harness::with_cwd(&output_dir, || sim_harness::run_executable(&executable))?;
            assert!(output.is_empty(), "{name} stdout: {output:?}");
            assert_eq!(
                fs::read_to_string(output_dir.join("deferred_file.txt"))
                    .map_err(|error| error.to_string())?,
                expected_file,
                "{name} file output"
            );
        }
        Ok(())
    })
    .expect("deferred file output simulation");
}
