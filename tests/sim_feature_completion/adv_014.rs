//! ADV-014: global clocking future sampled value functions (IEEE 1800-2009
//! 16.9.4): `$future_gclk`, `$rising_gclk`, `$falling_gclk`, `$steady_gclk`
//! and `$changing_gclk`. Hand-derived fixtures compare their action lines as
//! a set (actions of different assertions in one time step have no defined
//! order); every positive case runs in both optimizer modes on the legacy,
//! compact/portable and compact/GMP value backends.
use super::sim_cli;
use std::process::Output;

const SUITE: &str = "feature_completion/adv_014";

macro_rules! fixture {
    ($name:literal) => {
        include_str!(concat!(
            "../fixtures/sim/feature_completion/adv_014/",
            $name
        ))
    };
}

fn sorted_lines(text: &str) -> Vec<String> {
    let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
    lines.sort();
    lines
}

/// The run succeeds and its action lines are exactly `expected`, in any
/// order.
fn assert_line_set(label: &str, output: &Output, expected: &str) {
    assert!(
        output.status.success(),
        "{label}: {}",
        sim_cli::observed_output(output)
    );
    let stdout = llg::ffi::platform::native_text_to_lf(output.stdout.clone());
    let actual = sorted_lines(&String::from_utf8_lossy(&stdout));
    let expected = sorted_lines(expected);
    assert_eq!(actual, expected, "{label}");
}

/// A01: the five functions on the global clock, `$global_clock`, the
/// opposite edge and a faster clock, with X bits; each action runs at the
/// next global clock tick, and ticks at or after the last one never report.
#[test]
fn future_values_and_reporting_delay() {
    sim_cli::run_case_checked_matrix(SUITE, "values", &[], &|label, output| {
        assert_line_set(label, output, fixture!("values.out"))
    });
}

/// A01: multi-tick attempts report at the global clock tick after their
/// last tick (vacuous successes and covers too), across a second clock and
/// across gated-off global clock edges.
#[test]
fn attempts_report_after_their_last_tick() {
    sim_cli::run_case_checked_matrix(SUITE, "intervals", &[], &|label, output| {
        assert_line_set(label, output, fixture!("intervals.out"))
    });
}

/// A01: inside if-else, a synchronous abort, an initial-procedure assertion,
/// a procedural assertion and a property local input formal.
#[test]
fn future_functions_compose_with_property_forms() {
    sim_cli::run_case_checked_matrix(SUITE, "composition", &[], &|label, output| {
        assert_line_set(label, output, fixture!("composition.out"))
    });
}

/// A01: ticks after the last global clock tick are never evaluated; pending
/// attempts settle by the ordinary end-of-simulation rule (A14-D3).
#[test]
fn end_of_simulation_before_the_next_global_tick() {
    sim_cli::run_case_checked_matrix(SUITE, "end", &[], &|label, output| {
        assert_line_set(label, output, fixture!("end.out"))
    });
}

/// A02: $assertkill and disable iff act only within the attempt interval;
/// $assertoff stops new attempts; action controls apply when the delayed
/// action runs (A14-D2, A14-D4).
#[test]
fn controls_during_pending_evaluations() {
    sim_cli::run_case_checked_matrix(SUITE, "controls", &[], &|label, output| {
        assert_line_set(label, output, fixture!("controls.out"))
    });
}

/// A03: pending evaluations are reused in the steady state and count
/// against LLG_SEQUENCE_THREAD_LIMIT while the global clock stops.
#[test]
fn pending_evaluations_are_bounded() {
    sim_cli::run_case_checked_matrix(SUITE, "budget", &[], &|label, output| {
        assert_line_set(label, output, "done passes=2045\n")
    });
    for optimized in [false, true] {
        let limited = sim_cli::invoke_with_env(
            SUITE,
            "budget",
            optimized,
            &[],
            &[("LLG_SEQUENCE_THREAD_LIMIT", "50")],
            &[],
        );
        let stderr = String::from_utf8_lossy(&limited.stderr);
        assert!(!limited.status.success(), "optimized={optimized}: {stderr}");
        assert!(
            stderr.contains(
                "llg: sequence thread budget exhausted: more than 50 live sequence threads at \
                 time 2091 (concurrent assertion bud at "
            ),
            "optimized={optimized}: {stderr}"
        );
        assert!(
            !String::from_utf8_lossy(&limited.stdout).contains("done"),
            "optimized={optimized}: the run must stop at the budget"
        );
    }
}

/// A03: the illegal contexts of 16.9.4 stay located frontend diagnostics,
/// with the sampled value argument restrictions (16.9.3, 16.6.1).
#[test]
fn illegal_contexts_are_rejected() {
    sim_cli::reject_case(
        SUITE,
        "neg_nested",
        "neg_nested.sv:7:57 global clocking future sampled value functions may not be nested",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_match_item",
        "neg_match_item.sv:12:45 cannot call '$future_gclk' in an assertion that has sequence \
         match items",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_procedural",
        "neg_procedural.sv:8:40 global clocking future sampled value functions may be invoked \
         only in sequence and property expressions",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_action",
        "neg_action.sv:7:62 global clocking future sampled value functions may be invoked only \
         in sequence and property expressions",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_no_global",
        "neg_no_global.sv:5:39 no global clocking is available in this scope",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_real",
        "$future_gclk of a real expression is illegal in `tb`: sampled-value functions exclude \
         noninteger (shortreal, real, realtime) operands (SV 16.6.1)",
    );
}

/// A03: forms llg does not defer are located limits, not silent results.
#[test]
fn unsupported_deferred_forms_are_located() {
    sim_cli::reject_case(
        SUITE,
        "neg_expect",
        "neg_expect.sv:11:5 is not supported (IEEE 1800-2009 16.9.4)",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_async_abort",
        "neg_async_abort.sv:10:3 is not supported (IEEE 1800-2009 16.9.4)",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_endpoint",
        "neg_endpoint.sv:14:11 is not supported (IEEE 1800-2009 16.9.4)",
    );
}
