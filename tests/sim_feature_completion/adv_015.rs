//! ADV-015: checker execution and checker bind (IEEE 1800-2009 Clause 17,
//! 23.11). Hand-derived fixtures compare their output lines as a set
//! (actions of different assertions in one time step and final procedures
//! of different instances have no defined order); every positive case runs
//! in both optimizer modes on the legacy, compact/portable and compact/GMP
//! value backends.
use super::sim_cli;
use std::process::Output;

const SUITE: &str = "feature_completion/adv_015";

macro_rules! fixture {
    ($name:literal) => {
        include_str!(concat!(
            "../fixtures/sim/feature_completion/adv_015/",
            $name
        ))
    };
}

fn sorted_lines(text: &str) -> Vec<String> {
    let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
    lines.sort();
    lines
}

/// The run succeeds and its output lines are exactly `expected`, in any
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

/// A01: checker procedures and action blocks run in Reactive, read formals
/// as sampled values of their actuals and see checker variables before
/// their Re-NBA update.
#[test]
fn procedures_sample_formals_and_update_in_re_nba() {
    sim_cli::run_case_checked_matrix(SUITE, "sampling", &[], &|label, output| {
        assert_line_set(label, output, fixture!("sampling.out"))
    });
}

/// A01: `$inferred_clock`/`$inferred_disable` defaults, a nested checker
/// with an inferred clock, checker let/function/sequence declarations,
/// covers, a deferred assertion and final procedures.
#[test]
fn inferred_context_and_nested_checkers() {
    sim_cli::run_case_checked_matrix(SUITE, "nested", &[], &|label, output| {
        assert_line_set(label, output, fixture!("nested.out"))
    });
}

/// A01: a checker bound into an RTL module observes its interface port,
/// drives checker variables, samples a covergroup and instantiates a
/// subchecker with an inferred clock.
#[test]
fn bound_checker_observes_rtl_interface() {
    sim_cli::run_case_checked_matrix(SUITE, "bind", &[], &|label, output| {
        assert_line_set(label, output, fixture!("bind.out"))
    });
}

/// A01 boundary: disjoint bits assigned by two procedures, a named block
/// with assertions, two instances with independent state and an unused
/// checker declaration.
#[test]
fn disjoint_assignments_and_independent_instances() {
    sim_cli::run_case_checked_matrix(SUITE, "boundary", &[], &|label, output| {
        assert_line_set(label, output, fixture!("boundary.out"))
    });
}

/// A01: a checker formal passed to a let declared inside or outside the
/// checker reads the sampled value of its actual, through ordered, named
/// and default arguments, nested lets and repeated calls.
#[test]
fn let_arguments_read_sampled_formals() {
    sim_cli::run_case_checked_matrix(SUITE, "let_formals", &[], &|label, output| {
        assert_line_set(label, output, fixture!("let_formals.out"))
    });
}

/// A03: the 17.5 and 17.7.1 restrictions on checker procedures are located
/// errors.
#[test]
fn illegal_checker_procedures_are_rejected() {
    let procedure = "is not allowed in a checker always procedure: it may contain only event \
                     controls, nonblocking assignments to checker variables and assertions \
                     (IEEE 1800-2009 17.5)";
    sim_cli::reject_case(
        SUITE,
        "neg_blocking",
        &format!(
            "neg_blocking.sv:5:17: a blocking assignment (17.7.1 allows only nonblocking \
             assignments) {procedure}"
        ),
    );
    sim_cli::reject_case(
        SUITE,
        "neg_if",
        &format!("neg_if.sv:5:17: an if statement {procedure}"),
    );
    sim_cli::reject_case(
        SUITE,
        "neg_delay",
        &format!("neg_delay.sv:6:5: a delay control (only event controls are allowed) {procedure}"),
    );
    sim_cli::reject_case(
        SUITE,
        "neg_immediate",
        &format!(
            "neg_immediate.sv:5:5: a simple immediate assertion (only deferred and concurrent \
             assertions are allowed) {procedure}"
        ),
    );
    sim_cli::reject_case(
        SUITE,
        "neg_sar",
        "neg_sar.sv:6:17: checker variable `b` is already assigned at ",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_sar",
        "neg_sar.sv:5:17; a bit of a checker variable may be assigned only once (Single \
         Assignment Rule, IEEE 1800-2009 17.7.1)",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_select",
        "neg_select.sv:5:17: the target of a checker variable assignment must be its longest \
         static prefix; its select must be a constant expression (IEEE 1800-2009 17.7.1, \
         11.5.3)",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_foreign",
        "neg_foreign.sv:7:19: a checker procedure may assign only variables of its own \
         checker, not `m` (IEEE 1800-2009 17.7.1)",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_initial_nba",
        "neg_initial_nba.sv:5:11 statement is not allowed in 'initial' procedure inside a \
         checker",
    );
}

/// A03: checker forms of the later revision are edition errors under the
/// IEEE 1800-2009 profile.
#[test]
fn later_revision_checker_forms_are_edition_errors() {
    sim_cli::reject_case(
        SUITE,
        "neg_always_ff",
        "neg_always_ff.sv:5:3 `checker always_ff procedure` is not available in IEEE 2009",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_output",
        "neg_output.sv:3:41 `checker output formal` is not available in IEEE 2009",
    );
}

/// A02: free variables start random or initialized, active ones are solved
/// at their assume set clock events (or every time step) and read with
/// their current values, constant and assigned ones are not redrawn.
#[test]
fn free_variables_are_solved_before_observed() {
    sim_cli::run_case_checked_matrix(SUITE, "free_vars", &[], &|label, output| {
        assert_line_set(label, output, fixture!("free_vars.out"))
    });
}

/// A02: an assume set without a solution is an unsuccessful attempt whose
/// assumption fails when checked; another instance's set is still solved.
#[test]
fn unsuccessful_solution_attempts_fail_their_assumptions() {
    sim_cli::run_case_checked_matrix(SUITE, "free_unsat", &[], &|label, output| {
        assert_line_set(label, output, fixture!("free_unsat.out"))
    });
}

/// A02/A03: free variable forms llg does not randomize are located limits.
#[test]
fn unsupported_free_variable_forms_are_located() {
    sim_cli::reject_case(
        SUITE,
        "neg_free_history",
        "neg_free_history.sv:6:35: $rose of active free checker variable `f` is not supported: \
         llg reads active free variables with their current value (IEEE 1800-2009 17.7.2) and \
         keeps no sampled history of them",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_free_unpacked",
        "neg_free_unpacked.sv:4:12: free checker variable `f` of an unpacked type is not \
         supported: llg randomizes packed integral free variables (IEEE 1800-2009 17.7.2)",
    );
}

/// A03: procedural checker instances are located limits, not silent
/// results.
#[test]
fn unsupported_checker_forms_are_located() {
    sim_cli::reject_case(
        SUITE,
        "neg_procedural",
        "checker instance in procedural code (IEEE 1800-2009 17.4); instantiate the checker \
         outside the procedure` at ",
    );
    sim_cli::reject_case(SUITE, "neg_procedural", "neg_procedural.sv:10:5");
}
