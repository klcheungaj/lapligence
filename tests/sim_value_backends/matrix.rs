//! Witness of the FIN-002 value/optimizer matrix
//! (tests/readme.md#valueoptimizer-matrix): the value selection a matrix
//! cell names reaches the models the public CLI builds, proven by the link
//! guard symbol in the executable rather than by the environment alone (the
//! runtime exports it, so it is in the export table of a Windows executable).
//! Every model calls `llg_value_require_abi`, which the selected value header
//! renames to `llg_value_<identity>` and only the runtime archive of that
//! identity defines, so a model linked against another cell's runtime fails
//! to link instead of running on the wrong backend.
use llg::sim::value_backend::{CompactKernel, ValueConfig};
use std::collections::BTreeSet;
use std::path::Path;

use crate::{sim_cli, sim_harness};

const SUITE: &str = "value_backends";

/// The guard symbol prefix of `config` built with the GMP that
/// `GMP_ROOT=gmp_root` selects: the bundled subset when it is empty.
fn guard_prefix(config: ValueConfig, gmp_root: &str) -> String {
    let kernel = match config.kernel {
        CompactKernel::Portable => "portable",
        CompactKernel::Gmp if gmp_root.is_empty() => "gmp_bundled_",
        CompactKernel::Gmp => "gmp64_nail0_",
    };
    format!(
        "llg_value_v{}_b{}_k{}_{kernel}",
        config.backend.abi(),
        config.backend.selector(),
        config.kernel.selector()
    )
}

/// Every `llg_value_v*` symbol name in `bytes`.
fn guard_symbols(bytes: &[u8]) -> BTreeSet<String> {
    const MARK: &[u8] = b"llg_value_v";
    let mut symbols = BTreeSet::new();
    let mut start = 0;
    while let Some(offset) = bytes[start..]
        .windows(MARK.len())
        .position(|window| window == MARK)
    {
        let begin = start + offset;
        let end = bytes[begin..]
            .iter()
            .position(|byte| !(byte.is_ascii_alphanumeric() || *byte == b'_'))
            .map_or(bytes.len(), |length| begin + length);
        symbols.insert(String::from_utf8_lossy(&bytes[begin..end]).into_owned());
        start = end;
    }
    symbols
}

/// The model executables `llg` built under `out_dir/sim`: `sim` (`sim.exe`
/// on Windows) in the CMake tree's `bin` directory.
fn model_executables(out_dir: &Path) -> Vec<std::path::PathBuf> {
    let mut found = Vec::new();
    let mut pending = vec![out_dir.join("sim")];
    while let Some(directory) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if path.file_stem().is_some_and(|stem| stem == "sim")
                && path
                    .ancestors()
                    .any(|ancestor| ancestor.file_name().is_some_and(|name| name == "bin"))
            {
                found.push(path);
            }
        }
    }
    found
}

#[test]
fn matrix_cell_selection_reaches_the_built_model() {
    let cell = sim_harness::matrix_cell();
    let expected = cell.map_or(ValueConfig::COMPACT_GMP, |cell| {
        ValueConfig::parse(cell.backend, cell.kernel).expect("matrix lane")
    });
    assert_eq!(
        ValueConfig::from_env().expect("value selection"),
        expected,
        "the driver's selection"
    );
    let modes = sim_harness::optimizer_modes();
    match cell {
        Some(cell) => assert_eq!(modes, [cell.optimized]),
        None => assert_eq!(modes, [false, true]),
    }
    let gmp = sim_harness::test_gmp_root();
    let prefix = guard_prefix(expected, &gmp);
    for &optimized in modes {
        let (directory, output) = sim_cli::run_case_keeping_dir(
            SUITE,
            "implemented",
            optimized,
            &[],
            &[("GMP_ROOT", gmp.as_str())],
        );
        let label = format!("{expected:?}, optimized={optimized}");
        assert!(
            output.status.success(),
            "{label}: {}",
            sim_cli::observed_output(&output)
        );
        assert_eq!(String::from_utf8_lossy(&output.stdout), "product=323\n");
        let executables = model_executables(&directory.path().join("build"));
        assert_eq!(executables.len(), 1, "{label}: {executables:?}");
        let bytes = std::fs::read(&executables[0]).expect("read model executable");
        let guards = guard_symbols(&bytes);
        assert!(
            guards.len() == 1 && guards.iter().all(|guard| guard.starts_with(&prefix)),
            "{label}: guard symbols {guards:?}, expected one starting with {prefix}"
        );
    }
}

#[test]
fn matrix_cell_names_are_exact() {
    assert_eq!(
        sim_harness::parse_matrix_cell("legacy/portable/no-opt"),
        Ok(sim_harness::MatrixCell {
            backend: "legacy",
            kernel: "portable",
            optimized: false,
        })
    );
    assert_eq!(
        sim_harness::parse_matrix_cell("compact/gmp/normal"),
        Ok(sim_harness::MatrixCell {
            backend: "compact",
            kernel: "gmp",
            optimized: true,
        })
    );
    for invalid in [
        "",
        "legacy/gmp/normal",
        "compact/gmp",
        "compact/gmp/normal/extra",
        "Compact/gmp/normal",
        "compact/gmp/opt",
        "compact/portable/",
    ] {
        assert!(
            sim_harness::parse_matrix_cell(invalid).is_err(),
            "{invalid:?}"
        );
    }
}

#[test]
fn guard_symbols_are_read_whole() {
    let bytes = b"\0llg_value_v4_b0_k0_portable\0x llg_value_v5_b1_k1_gmp_bundled_0a1b.c\0";
    assert_eq!(
        guard_symbols(bytes),
        BTreeSet::from([
            "llg_value_v4_b0_k0_portable".to_owned(),
            "llg_value_v5_b1_k1_gmp_bundled_0a1b".to_owned(),
        ])
    );
}
