//! File-based HDL→owned DB→IR→C11→execution conformance tests. Expectations
//! are independent of llg's value implementation; both optimizer modes must
//! match, including bits above the first machine word.

#[path = "support/sim.rs"]
mod sim_harness;

use llg::core::{compile, db::Db};
use llg::sim::{self, opt::OptConfig};
use std::path::Path;

fn run_fixture(file: &str, width: usize, expected: &str) {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/data_types")
        .join(file);
    sim_harness::with_surelog_temp_cwd("data-types", |dir| {
        let source = dir.join(file);
        std::fs::copy(&fixture, &source).map_err(|error| format!("copy fixture: {error}"))?;
        let compiled = compile::compile_checked(&compile::CompileOpts {
            files: vec![source.to_string_lossy().into_owned()],
            top: Some("tb".to_owned()),
            param_overrides: vec![format!("-PWIDTH={width}")],
            ..Default::default()
        })
        .map_err(|error| format!("{file}, width {width}: compile: {error}"))?;
        let db = Db::build_with_source_files(
            compiled.uhdm_design().ok_or("no UHDM design")?,
            &compiled.frontend_source_files(),
        )
        .map_err(|error| error.to_string())?;
        // Exercise both variants even when one fails, so a conformance gap
        // records whether optimization affects the result.
        let mut failures = Vec::new();
        for (variant, options) in [
            ("unoptimized", OptConfig::none()),
            ("optimized", OptConfig::default()),
        ] {
            let actual = (|| {
                let model = sim::codegen::generate_from_db_with_opts(&db, &options)
                    .map_err(|error| format!("lowering: {error}"))?;
                let executable = sim::build::build_model_cmake(
                    &dir.join(variant),
                    &[("model.c", model.model_c.as_str())],
                )
                .map_err(|error| format!("C model build: {error}"))?;
                sim_harness::run_executable(&executable)
            })();
            match actual {
                Ok(actual) if actual == expected => {}
                Ok(actual) => failures.push(format!("{variant}: {}", mismatch(expected, &actual))),
                Err(error) => failures.push(format!("{variant}: {error}")),
            }
        }
        if failures.is_empty() {
            Ok(())
        } else {
            Err(format!("{file}, width {width}:\n{}", failures.join("\n")))
        }
    })
    .expect("datatype end-to-end conformance");
}

fn mismatch(expected: &str, actual: &str) -> String {
    let offset = expected
        .bytes()
        .zip(actual.bytes())
        .position(|(a, b)| a != b)
        .unwrap_or(expected.len().min(actual.len()));
    let line = expected.as_bytes()[..offset]
        .iter()
        .filter(|byte| **byte == b'\n')
        .count()
        + 1;
    let expected_line = expected.lines().nth(line - 1).unwrap_or("<end of output>");
    let actual_line = actual.lines().nth(line - 1).unwrap_or("<end of output>");
    format!("output mismatch at byte {offset}, line {line}: expected {expected_line:?}, got {actual_line:?}")
}

// Literal IEEE tables, with states ordered 0/1/X/Z. Do not call the simulator's
// scalar/vector helpers here: that would share defects with the tested path.
const AND: [&str; 4] = ["0000", "01xx", "0xxx", "0xxx"];
const OR: [&str; 4] = ["01xx", "1111", "x1xx", "x1xx"];
const XOR: [&str; 4] = ["01xx", "10xx", "xxxx", "xxxx"];
const XNOR: [&str; 4] = ["10xx", "01xx", "xxxx", "xxxx"];
const MUX: [&str; 4] = ["0xxx", "x1xx", "xxxx", "xxxz"];
const EQ: [&str; 4] = ["10xx", "01xx", "xxxx", "xxxx"];

fn table_bit(table: &[&str; 4], left: usize, right: usize) -> char {
    char::from(table[left].as_bytes()[right])
}

fn truth_output(width: usize) -> String {
    let mut expected = String::new();
    for left in 0..4 {
        for right in 0..4 {
            let vector = |table| table_bit(table, left, right).to_string().repeat(width);
            let not = ["1", "0", "x", "x"][left].repeat(width);
            expected.push_str(&format!(
                "pair={left}{right}\nand={}\nor={}\nxor={}\nxnor={}\nnot={not}\nmux={}\nmuxz={}\neq={} case={}\n",
                vector(&AND), vector(&OR), vector(&XOR), vector(&XNOR), vector(&MUX), vector(&MUX),
                table_bit(&EQ, left, right), u8::from(left == right),
            ));
            let reduced_xor = match left {
                0 => '0',
                1 if width.is_multiple_of(2) => '0',
                1 => '1',
                _ => 'x',
            };
            let reduced_xnor = match reduced_xor {
                '0' => '1',
                '1' => '0',
                _ => 'x',
            };
            expected.push_str(&format!(
                "land={} lor={} lnot={} nand={} nor={} rxor={reduced_xor} rxnor={reduced_xnor}\n",
                table_bit(&AND, left, right),
                table_bit(&OR, left, right),
                ["1", "0", "x", "x"][left],
                ["1", "0", "x", "x"][left],
                ["1", "0", "x", "x"][left],
            ));
        }
    }
    // Hand-derived byte oracles for 10xz01zx versus 0110xz10.
    expected.push_str(&format!(
        "mixed-and={}\nmixed-or={}\nmixed-xor={}\nmixed-mux={}\n",
        "0".repeat(width % 8) + &"00x00xx0".repeat(width / 8),
        "0".repeat(width % 8) + &"111xx11x".repeat(width / 8),
        "0".repeat(width % 8) + &"11xxxxxx".repeat(width / 8),
        "0".repeat(width % 8) + &"xxxxxxxx".repeat(width / 8),
    ));
    expected.push_str("high eq=0 case=0 gt=1 or=1 and=0 xor=1\n");
    expected.push_str("highx eq=x case=0 or=x and=0 xor=x\n");
    expected.push_str("known-dominance or=1\n");
    expected.push_str(&format!(
        "highz or=1 and=x xor=x not=x{}\n",
        "0".repeat(width - 1)
    ));
    expected
}

macro_rules! truth_case {
    ($name:ident, $width:expr) => {
        #[test]
        fn $name() {
            run_fixture("four_state_truth.v", $width, &truth_output($width));
        }
    };
}

truth_case!(four_state_truth_tables_128_bits, 128);
truth_case!(four_state_truth_tables_65_bits, 65);
truth_case!(four_state_truth_tables_129_bits, 129);
truth_case!(four_state_truth_tables_512_bits, 512);
truth_case!(four_state_truth_tables_513_bits, 513);
truth_case!(four_state_truth_tables_1023_bits, 1023);
truth_case!(four_state_truth_tables_1024_bits, 1024);

#[test]
fn four_state_truth_tables_2048_bits() {
    run_fixture("four_state_truth.v", 2048, &truth_output(2048));
}

macro_rules! marker_case {
    ($(#[$attribute:meta])* $name:ident, $file:literal, $label:literal, $width:expr) => {
        #[test]
        $(#[$attribute])*
        fn $name() {
            run_fixture($file, $width, &format!("PASS {} WIDTH={}\n", $label, $width));
        }
    };
}

marker_case!(arithmetic_128_bits, "wide_arithmetic.v", "arithmetic", 128);
marker_case!(arithmetic_512_bits, "wide_arithmetic.v", "arithmetic", 512);
marker_case!(
    arithmetic_1024_bits,
    "wide_arithmetic.v",
    "arithmetic",
    1024
);
marker_case!(
    arithmetic_2048_bits,
    "wide_arithmetic.v",
    "arithmetic",
    2048
);
marker_case!(
    arithmetic_4096_bits,
    "wide_arithmetic.v",
    "arithmetic",
    4096
);

marker_case!(four_to_two_state_128_bits, "two_state.sv", "two_state", 128);
marker_case!(four_to_two_state_512_bits, "two_state.sv", "two_state", 512);
marker_case!(
    four_to_two_state_1024_bits,
    "two_state.sv",
    "two_state",
    1024
);
marker_case!(
    four_to_two_state_atoms,
    "two_state_atoms.sv",
    "two_state_atoms",
    128
);
marker_case!(
    four_to_two_state_casts_128_bits,
    "two_state_casts.sv",
    "two_state_casts",
    128
);
marker_case!(
    four_to_two_state_casts_512_bits,
    "two_state_casts.sv",
    "two_state_casts",
    512
);
marker_case!(
    four_to_two_state_casts_1024_bits,
    "two_state_casts.sv",
    "two_state_casts",
    1024
);
marker_case!(
    four_to_two_state_atom_casts,
    "two_state_atom_casts.sv",
    "two_state_atom_casts",
    128
);
marker_case!(
    four_to_two_state_scalar_cast,
    "two_state_scalar_cast.sv",
    "two_state_scalar_cast",
    128
);

marker_case!(casts_128_bits, "casts.sv", "casts", 128);
marker_case!(casts_512_bits, "casts.sv", "casts", 512);
marker_case!(casts_1024_bits, "casts.sv", "casts", 1024);
marker_case!(
    sized_casts_128_bits,
    "casts_conformance.sv",
    "casts-conformance",
    128
);
marker_case!(
    sized_casts_512_bits,
    "casts_conformance.sv",
    "casts-conformance",
    512
);
marker_case!(
    sized_casts_1024_bits,
    "casts_conformance.sv",
    "casts-conformance",
    1024
);

marker_case!(division_128_bits, "wide_division.v", "division", 128);
marker_case!(division_512_bits, "wide_division.v", "division", 512);
marker_case!(division_1024_bits, "wide_division.v", "division", 1024);
marker_case!(modulo_128_bits, "wide_modulo.v", "modulo", 128);
marker_case!(modulo_512_bits, "wide_modulo.v", "modulo", 512);
marker_case!(modulo_1024_bits, "wide_modulo.v", "modulo", 1024);
marker_case!(power_128_bits, "wide_power.v", "power", 128);
marker_case!(power_512_bits, "wide_power.v", "power", 512);
marker_case!(power_1024_bits, "wide_power.v", "power", 1024);

macro_rules! equality_case {
    ($name:ident, $width:expr) => {
        #[test]
        fn $name() {
            run_fixture(
                "equality_unknown.v",
                $width,
                concat!(
                    "high-x eq=0 ne=1 case=0 ncase=1\n",
                    "low-x eq=0 ne=1 case=0 ncase=1\n",
                    "low-z eq=0 ne=1 case=0 ncase=1\n",
                ),
            );
        }
    };
}

equality_case!(known_mismatch_dominates_unknown_equality_128_bits, 128);
equality_case!(known_mismatch_dominates_unknown_equality_512_bits, 512);
equality_case!(known_mismatch_dominates_unknown_equality_1024_bits, 1024);
