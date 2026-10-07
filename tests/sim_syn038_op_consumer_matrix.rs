//! SYN-038 expression consumers across call arguments, event controls,
//! function returns, port actuals, declaration initializers, and dimensions.

use crate::sim_harness;

use std::path::Path;
use std::process::Command;
use std::time::Duration;

const FIXTURE_SOURCE: &str = include_str!("fixtures/sim/syn038_pairwise/op_consumer_matrix.sv");
const EXPECTED_STDOUT: &[u8] = b"calls=18,0,18,18 events=1,1,1 widths=5,7,2\n";
const EXPECTED_STDERR: &[u8] = b"";

const FOCAL_SOURCE_ANCHORS: &[&str] = &[
    "consume(select ? a : b)",
    "consume_bit(a == b)",
    "consume(two_byte_t'(a))",
    "consume('{a[7],a[6],a[5],a[4],a[3],a[2],a[1],a[0]})",
    "return x == y;",
    "always @(select ? a[0] : b[0])",
    "always @(two_byte_t'(a))",
    "always @(byte_t'{a[7],a[6],a[5],a[4],a[3],a[2],a[1],a[0]})",
    ".value(two_byte_t'(a))",
    "typedef logic [(CONST_SELECT ? 5 : 7)-1:0] conditional_width_t;",
    "typedef logic [((CONST_A == CONST_B) ? 5 : 7)-1:0] equality_width_t;",
    "typedef logic [(int'(CONST_A[3:0]) - 1):0] cast_width_t;",
    "byte_t conditional_decl = CONST_SELECT ? CONST_A : CONST_B;",
];

#[test]
fn expression_consumers_keep_distinct_contexts_in_both_cli_modes() {
    assert!(
        FIXTURE_SOURCE.starts_with(
            "// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/op_consumer_matrix.sv\n"
        ),
        "fixture source header changed"
    );
    for anchor in FOCAL_SOURCE_ANCHORS {
        assert!(
            FIXTURE_SOURCE.contains(anchor),
            "fixture lost source anchor: {anchor}"
        );
    }
    for assertion in [
        "if (conditional_decl != 8'h12)",
        "if (observed != 8'h12)",
        "if (conditional_call != 18 || equality_call != 0 || cast_call != 18 || pattern_call != 18)",
        "if (equal_return(a,b) != 0)",
        "if (event_conditional < 1 || event_cast < 1 || event_pattern < 1)",
        "if ($bits(conditional_width_t) != 5 || $bits(equality_width_t) != 7 || $bits(cast_width_t) != 2)",
    ] {
        assert!(FIXTURE_SOURCE.contains(assertion), "fixture lost result assertion: {assertion}");
    }

    assert!(
        llg::sim::build::cmake_available(),
        "CLI tests require CMake"
    );
    let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/syn038_pairwise/op_consumer_matrix.sv");

    for optimized in [false, true] {
        let directory =
            sim_harness::TempDir::new("syn038-op-consumer-matrix").expect("CLI test directory");
        let mut command = Command::new(env!("CARGO_BIN_EXE_llg"));
        command.current_dir(directory.path()).args(["--top", "tb"]);
        if !optimized {
            command.arg("--no-opt");
        }
        command.args(["--edition", "sv2009"]).arg(&fixture_path);
        let output = sim_harness::run_command(&mut command, Duration::from_secs(180))
            .unwrap_or_else(|error| {
                panic!("syn038_pairwise/op_consumer_matrix, optimized={optimized}: {error}")
            });
        let label = format!("syn038_pairwise/op_consumer_matrix, optimized={optimized}");
        assert_eq!(
            output.status.code(),
            Some(0),
            "{label}: stderr={} stdout={}",
            String::from_utf8_lossy(&output.stderr),
            String::from_utf8_lossy(&output.stdout)
        );
        assert_eq!(output.stdout.as_slice(), EXPECTED_STDOUT, "{label}");
        assert_eq!(
            crate::sim_harness::strip_lint_reports(&output.stderr).as_bytes(),
            EXPECTED_STDERR,
            "{label}"
        );
    }
}
