//! Joined task forks observe persistent storage and share automatic outputs.

use crate::sim_cli;
use crate::sim_harness;

use std::path::Path;

const STATIC_SOURCE: &str =
    include_str!("fixtures/sim/syn038_pairwise/nested_task_fork_static_event.sv");
const FORMAL_SOURCE: &str =
    include_str!("fixtures/sim/syn038_pairwise/nested_task_fork_formal_event.sv");
const REAL_SOURCE: &str =
    include_str!("fixtures/sim/syn038_pairwise/nested_task_fork_real_join.sv");

#[test]
fn joined_task_forks_share_persistent_events_and_automatic_outputs() {
    assert!(STATIC_SOURCE.contains("static bit static_source;"));
    assert!(STATIC_SOURCE.contains("#1 static_source = 1'b1;"));
    assert!(STATIC_SOURCE.contains("@(tb.sample_event_source.static_source);"));
    assert!(STATIC_SOURCE.contains("seen = 1'b1;"));
    assert!(STATIC_SOURCE.contains("if (!observed) $fatal"));
    assert!(FORMAL_SOURCE
        .contains("task sample_formal_event(input bit formal_source, output bit observed);"));
    assert!(FORMAL_SOURCE.contains("tb.sample_formal_event.formal_source = 1'b1;"));
    assert!(FORMAL_SOURCE.contains("@(tb.sample_formal_event.formal_source);"));
    assert!(FORMAL_SOURCE.contains("observed = 1'b1;"));
    assert!(FORMAL_SOURCE.contains("if (!formal_seen) $fatal"));
    assert!(REAL_SOURCE.contains("begin #1 shared = 2.5; end"));
    assert!(REAL_SOURCE.contains("begin #2 seen = shared; end"));
    assert!(REAL_SOURCE.contains("if (observed != 2.5) $fatal"));

    assert!(
        llg::sim::build::cmake_available(),
        "CLI tests require CMake"
    );
    let source_root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sim/syn038_pairwise");
    for optimized in [false, true] {
        for (stem, stdout, warning_lines) in [
            (
                "nested_task_fork_static_event",
                "static-event=1\n",
                &[(14, 19)][..],
            ),
            (
                "nested_task_fork_formal_event",
                "formal-hier-event=1\n",
                &[(11, 17), (14, 19)][..],
            ),
            ("nested_task_fork_real_join", "real=2.5\n", &[][..]),
        ] {
            let fixture = source_root.join(format!("{stem}.sv"));
            let output = sim_cli::invoke_with_env(
                "syn038_pairwise",
                stem,
                optimized,
                &["--edition", "sv2009"],
                &[],
                &[],
            );
            assert!(output.status.success(), "{stem}: {:?}", output.stderr);
            assert_eq!(
                output.stdout,
                stdout.as_bytes(),
                "{stem}, optimized={optimized}"
            );
            let warnings = warning_lines
                .iter()
                .map(|(line, column)| {
                    format!(
                        "Warning: {}:{line}:{column} 'tb' is an upward hierarchical name reference\n",
                        sim_harness::source_display(&fixture)
                    )
                })
                .collect::<String>();
            assert_eq!(
                crate::sim_harness::strip_lint_reports(&output.stderr),
                warnings,
                "{stem}, optimized={optimized}"
            );
        }
    }
}
