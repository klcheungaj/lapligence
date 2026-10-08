//! SIM-018: reclamation of unreachable class-object graphs, including cycles,
//! at scheduler safe points. Outputs are derived by hand in the fixture readme;
//! collector statistics are the live-memory oracle.
use super::{sim_cli, sim_harness};

const SUITE: &str = "feature_completion/sim_018";

/// Collector settings a caller's environment could leak into a test that
/// fixes its own policy.
const COLLECTOR_ENV: [&str; 6] = [
    "LLG_GC",
    "LLG_GC_THRESHOLD",
    "LLG_GC_GROWTH_PERCENT",
    "LLG_GC_STRESS",
    "LLG_GC_VERIFY",
    "LLG_GC_STATS",
];

/// Collect at every safe point and keep unreachable objects poisoned instead
/// of freed, so any access through a missed root is a located failure.
const STRESS: [(&str, &str); 2] = [("LLG_GC_STRESS", "1"), ("LLG_GC_VERIFY", "1")];

/// Run `fixture` on the legacy backend with `envs` and statistics enabled,
/// returning stdout and the single `llg: gc:` statistics line.
fn run_with_stats(fixture: &str, optimized: bool, envs: &[(&str, &str)]) -> (String, String) {
    let mut controls = envs.to_vec();
    controls.extend([
        ("LLG_GC_STATS", "1"),
        ("LLG_DEV_VALUE_BACKEND", "legacy"),
        ("LLG_DEV_COMPACT_KERNELS", "portable"),
    ]);
    let removed = COLLECTOR_ENV
        .iter()
        .copied()
        .filter(|name| !envs.iter().any(|(set, _)| set == name))
        .filter(|name| *name != "LLG_GC_STATS")
        .collect::<Vec<_>>();
    let output = sim_cli::invoke_with_env(SUITE, fixture, optimized, &[], &controls, &removed);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{fixture}: {stderr}");
    let stats = stderr
        .lines()
        .filter(|line| line.starts_with("llg: gc: "))
        .collect::<Vec<_>>();
    assert_eq!(stats.len(), 1, "{fixture}: {stderr}");
    (
        String::from_utf8_lossy(&output.stdout).into_owned(),
        stats[0].to_owned(),
    )
}

#[test]
fn unreachable_cycles_reach_a_bounded_plateau() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_018/cycle_plateau.out");
    sim_cli::run_case_backend_parity(SUITE, "cycle_plateau", expected, &[], &[]);
    for optimized in [false, true] {
        // Threshold 100: a collection after every 50 iterations leaves the
        // three reachable objects; the peak is those plus 100 new ones.
        let (stdout, stats) =
            run_with_stats("cycle_plateau", optimized, &[("LLG_GC_THRESHOLD", "100")]);
        assert_eq!(stdout, expected);
        assert_eq!(
            stats,
            "llg: gc: collections=400 failed=0 allocated=40001 freed=39998 condemned=0 live=3 peak=103 pinned=0"
        );
        // Default policy (4096, growth 100%): nine collections, bounded peak.
        let (_, stats) = run_with_stats("cycle_plateau", optimized, &[]);
        assert_eq!(
            stats,
            "llg: gc: collections=9 failed=0 allocated=40001 freed=36862 condemned=0 live=3139 peak=4099 pinned=0"
        );
        // Without collection every object stays until model close.
        let (_, stats) = run_with_stats("cycle_plateau", optimized, &[("LLG_GC", "0")]);
        assert_eq!(
            stats,
            "llg: gc: collections=0 failed=0 allocated=40001 freed=0 condemned=0 live=40001 peak=40001 pinned=0"
        );
    }
}

#[test]
fn cycles_reachable_from_suspended_children_survive() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_018/suspended_child.out");
    // Default policy, then the stress/verify lane.
    sim_cli::run_case_backend_parity(SUITE, "suspended_child", expected, &[], &[]);
    sim_cli::run_case_backend_parity(SUITE, "suspended_child", expected, &[], &STRESS);
}

#[test]
fn mailbox_messages_and_pending_deliveries_keep_cycles() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_018/mailbox_only.out");
    // Default policy, then the stress/verify lane.
    sim_cli::run_case_backend_parity(SUITE, "mailbox_only", expected, &[], &[]);
    sim_cli::run_case_backend_parity(SUITE, "mailbox_only", expected, &[], &STRESS);
}

#[test]
fn queued_nonblocking_updates_keep_cycles() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_018/queued_action.out");
    // Default policy, then the stress/verify lane.
    sim_cli::run_case_backend_parity(SUITE, "queued_action", expected, &[], &[]);
    sim_cli::run_case_backend_parity(SUITE, "queued_action", expected, &[], &STRESS);
}

#[test]
fn running_methods_pin_their_receivers() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_018/receiver_pin.out");
    // Default policy, then the stress/verify lane.
    sim_cli::run_case_backend_parity(SUITE, "receiver_pin", expected, &[], &[]);
    sim_cli::run_case_backend_parity(SUITE, "receiver_pin", expected, &[], &STRESS);
}

#[test]
fn container_and_record_properties_form_collectable_cycles() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_018/containers_records.out");
    sim_cli::run_case_backend_parity(SUITE, "containers_records", expected, &[], &[]);
    sim_cli::run_case_backend_parity(SUITE, "containers_records", expected, &[], &STRESS);
    for optimized in [false, true] {
        // Eight objects stay reachable from module containers and the class
        // static property; every churned object (40 x 4) is reclaimed.
        let (stdout, stats) = run_with_stats("containers_records", optimized, &STRESS);
        assert_eq!(stdout, expected);
        assert!(
            stats.contains(" allocated=168 freed=0 condemned=160 live=8 "),
            "{stats}"
        );
    }
}

#[test]
fn deep_lists_are_marked_without_recursion() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_018/deep_chain.out");
    sim_cli::run_case_backend_parity(SUITE, "deep_chain", expected, &[], &[]);
    sim_cli::run_case_backend_parity(SUITE, "deep_chain", expected, &[], &STRESS);
    let (_, stats) = run_with_stats("deep_chain", true, &STRESS);
    assert!(
        stats.contains(" allocated=5020 freed=0 condemned=2519 live=2501 peak=5001 "),
        "{stats}"
    );
}

#[test]
fn collection_does_not_change_identity_or_random_streams() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_018/identity_random.out");
    sim_cli::run_case_checked_matrix(SUITE, "identity_random", &[], &|label, output| {
        assert!(output.status.success(), "{label}: {output:?}");
        let stdout = String::from_utf8_lossy(&output.stdout);
        let equalities = stdout
            .lines()
            .filter(|line| line.starts_with("eq "))
            .map(|line| format!("{line}\n"))
            .collect::<String>();
        assert_eq!(equalities, expected, "{label}");
    });
    let gmp = sim_harness::test_gmp_root();
    for optimized in [false, true] {
        for (backend, kernel) in [
            ("legacy", "portable"),
            ("compact", "portable"),
            ("compact", "gmp"),
        ] {
            let lane = [
                ("LLG_DEV_VALUE_BACKEND", backend),
                ("LLG_DEV_COMPACT_KERNELS", kernel),
                ("GMP_ROOT", gmp.as_str()),
            ];
            let run = |extra: &[(&str, &str)]| {
                let mut envs = lane.to_vec();
                envs.extend_from_slice(extra);
                let removed = COLLECTOR_ENV
                    .iter()
                    .copied()
                    .filter(|name| !extra.iter().any(|(set, _)| set == name))
                    .collect::<Vec<_>>();
                let output = sim_cli::invoke_with_env(
                    SUITE,
                    "identity_random",
                    optimized,
                    &[],
                    &envs,
                    &removed,
                );
                assert!(output.status.success(), "{output:?}");
                String::from_utf8_lossy(&output.stdout).into_owned()
            };
            let off = run(&[("LLG_GC", "0")]);
            let stressed = run(&[("LLG_GC_STRESS", "1")]);
            let label = format!("{backend}/{kernel}, optimized={optimized}");
            // Handle comparisons are derived by hand; the draws must equal
            // the run without any collection.
            let equalities = off
                .lines()
                .filter(|line| line.starts_with("eq "))
                .map(|line| format!("{line}\n"))
                .collect::<String>();
            assert_eq!(equalities, expected, "{label}");
            assert_eq!(off.lines().count(), 10, "{label}: {off}");
            assert_eq!(stressed, off, "{label}");
        }
    }
}

#[test]
fn invalid_collector_settings_are_configuration_errors() {
    for (name, value, message) in [
        (
            "LLG_GC_THRESHOLD",
            "0",
            "llg: invalid LLG_GC_THRESHOLD (must be a positive decimal uint64)",
        ),
        (
            "LLG_GC_GROWTH_PERCENT",
            "x",
            "llg: invalid LLG_GC_GROWTH_PERCENT (must be a positive decimal uint64)",
        ),
        ("LLG_GC", "2", "llg: invalid LLG_GC `2` (expected 0 or 1)"),
        (
            "LLG_GC_STRESS",
            "yes",
            "llg: invalid LLG_GC_STRESS `yes` (expected 0 or 1)",
        ),
    ] {
        let removed = COLLECTOR_ENV
            .iter()
            .copied()
            .filter(|variable| *variable != name)
            .collect::<Vec<_>>();
        let output = sim_cli::invoke_with_env(
            SUITE,
            "identity_random",
            true,
            &[],
            &[(name, value)],
            &removed,
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(1), "{name}: {stderr}");
        assert!(output.stdout.is_empty(), "{name}: {output:?}");
        assert!(
            stderr.lines().any(|line| line == message),
            "{name}: {stderr}"
        );
    }
}
