//! Characterization of Q02 unresolved-oracle behavior; goldens are not conformance oracles.

use std::path::Path;
use std::process::Command;
use std::time::Duration;

use crate::sim_harness;

fn characterize(stem: &str, extension: &str) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sim/undefined_behavior");
    let source = format!("{stem}.{extension}");
    assert!(root.join(&source).is_file(), "missing fixture {source}");
    let data: &[&str] = match stem {
        "q02_short_hex" | "q02_narrow_signed" | "q02_types" | "q02_views" => &["q02_hex.mem"],
        "q02_short_binary" | "q02_binary_narrow" | "q02_binary_types" => &["q02_binary.mem"],
        "q02_enum_numeric" => &["q02_enum.mem"],
        "q02_enum_tokens" => &[
            "q02_hex_x.mem",
            "q02_hex_z.mem",
            "q02_hex_1x.mem",
            "q02_hex_x1.mem",
            "q02_hex_zX.mem",
            "q02_bin_x.mem",
            "q02_bin_z.mem",
            "q02_bin_1x.mem",
            "q02_bin_x1.mem",
            "q02_bin_zX.mem",
        ],
        "q02_malformed" => &["q02_malformed.mem"],
        "q02_bad_address" => &["q02_bad_address.mem"],
        "q02_short_file" => &["q02_short_file.mem"],
        "q02_long_file" => &["q02_long_file.mem"],
        "q02_wakeup" => &["q02_wakeup.mem"],
        _ => &[],
    };
    let editions: &[&str] = if extension == "v" {
        &["2001", "2009"]
    } else {
        &["2009"]
    };
    for &edition in editions {
        for optimized in [true, false] {
            let mode = if optimized { "opt" } else { "no-opt" };
            let label = format!("{stem}, edition={edition}, mode={mode}");
            let directory = sim_harness::TempDir::new(stem)
                .unwrap_or_else(|error| panic!("{label}: test directory: {error}"));
            for file in std::iter::once(source.as_str()).chain(data.iter().copied()) {
                std::fs::copy(root.join(file), directory.path().join(file))
                    .unwrap_or_else(|error| panic!("{label}: copy {file}: {error}"));
            }
            let mut command = Command::new(env!("CARGO_BIN_EXE_llg"));
            command
                .current_dir(directory.path())
                .args(["--top", "tb", "--edition", edition]);
            if !optimized {
                command.arg("--no-opt");
            }
            command.arg(&source);
            let output = sim_harness::run_command(&mut command, Duration::from_secs(180))
                .unwrap_or_else(|error| panic!("{label}: {error}"));
            let prefix = format!("{stem}.{edition}.{mode}.llg");
            let expected_stdout = std::fs::read(root.join(format!("{prefix}.out")))
                .unwrap_or_else(|error| panic!("{label}: stdout golden: {error}"));
            let expected_stderr = std::fs::read(root.join(format!("{prefix}.err")))
                .unwrap_or_else(|error| panic!("{label}: stderr golden: {error}"));
            let expected_status = std::fs::read_to_string(root.join(format!("{prefix}.status")))
                .unwrap_or_else(|error| panic!("{label}: status golden: {error}"));
            let expected_status: i32 = expected_status
                .trim()
                .parse()
                .unwrap_or_else(|error| panic!("{label}: invalid status golden: {error}"));
            assert_eq!(output.stdout, expected_stdout, "{label}: stdout");
            assert_eq!(output.stderr, expected_stderr, "{label}: stderr");
            assert_eq!(
                output.status.code(),
                Some(expected_status),
                "{label}: status"
            );
        }
    }
}

macro_rules! fixture {
    ($name:ident, $extension:literal) => {
        #[test]
        fn $name() {
            characterize(stringify!($name), $extension);
        }
    };
}

fixture!(q02_short_hex, "v");
fixture!(q02_short_binary, "v");
fixture!(q02_binary_narrow, "v");
fixture!(q02_binary_types, "sv");
fixture!(q02_narrow_signed, "v");
fixture!(q02_types, "sv");
fixture!(q02_enum_numeric, "sv");
fixture!(q02_enum_tokens, "sv");
fixture!(q02_views, "sv");
fixture!(q02_malformed, "v");
fixture!(q02_bad_address, "v");
fixture!(q02_short_file, "v");
fixture!(q02_long_file, "v");
fixture!(q02_wakeup, "v");
