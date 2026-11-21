//! KI illegal port writes: assignments the instance tree does not show but
//! IEEE 1800-2009 23.3.3.2 and 6.5 forbid are located compile errors.
//! A write through a virtual-interface handle to a variable declared as an
//! input port of the handle's interface type is rejected where it is
//! written (decision A21-D7), and a write in a subroutine a process calls (a
//! class method, or what it calls) to a variable that an output port or a
//! continuous assignment drives is rejected at the write (decision A21-D8).
//! Every expected value is derived by hand in the fixture readme. Positive
//! cases run in both optimizer modes and on every value backend.
use super::sim_cli;

const SUITE: &str = "feature_completion/ki_illegal_port_writes";

macro_rules! fixture {
    ($name:literal) => {
        include_str!(concat!(
            "../fixtures/sim/feature_completion/ki_illegal_port_writes/",
            $name
        ))
    };
}

const INPUT_WRITE: &str = "semantic error: `v.a` writes `a`, an input port variable of interface \
                           `ifc`, through a virtual interface; assignments to variables declared \
                           as input ports are illegal (IEEE 1800-2009 23.3.3.2)";

/// Blocking and nonblocking writes, an output actual and a concatenation
/// member through a class's handle, a write in a method nothing calls and a
/// module's handle to an instance whose input port is unconnected: each is
/// one located error at the written member.
#[test]
fn virtual_interface_writes_to_input_port_variables_are_errors() {
    sim_cli::reject_case_with_id_exact(
        SUITE,
        "vif_blocking",
        "CODEGEN-002",
        Some("9:8"),
        INPUT_WRITE,
    );
    sim_cli::reject_case_with_id_exact(
        SUITE,
        "vif_nonblocking",
        "CODEGEN-002",
        Some("9:8"),
        INPUT_WRITE,
    );
    sim_cli::reject_case_with_id_exact(
        SUITE,
        "vif_output_actual",
        "CODEGEN-002",
        Some("10:12"),
        INPUT_WRITE,
    );
    sim_cli::reject_case_with_id_exact(
        SUITE,
        "vif_concatenation",
        "CODEGEN-002",
        Some("9:14"),
        INPUT_WRITE,
    );
    sim_cli::reject_case_with_id_exact(
        SUITE,
        "vif_never_runs",
        "CODEGEN-002",
        Some("8:5"),
        INPUT_WRITE,
    );
    sim_cli::reject_case_with_id_exact(
        SUITE,
        "vif_module_handle",
        "CODEGEN-002",
        Some("11:8"),
        INPUT_WRITE,
    );
}

/// The nearest legal form: the handle writes a member that is no port and
/// reads the input port variable.
#[test]
fn virtual_interface_reads_of_input_port_variables_run() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "vif_member_write",
        fixture!("vif_member_write.out"),
        &[],
        &[],
    );
}

/// A class task, a class function called in an expression, a class task
/// reached through another, one reached from a module task, a
/// compilation-unit class writing the hierarchical name, a bit select, an
/// output actual of another class task, and a class task writing a
/// continuously assigned variable: one located error at the write.
#[test]
fn called_subroutine_writes_to_continuously_driven_variables_are_errors() {
    let method = |name: &str| format!("procedural assignment in class method `{name}`");
    sim_cli::reject_case_with_id(
        SUITE,
        "method_function",
        "CODEGEN-002",
        Some("12:7"),
        &method("D::f"),
    );
    sim_cli::reject_case_with_id(
        SUITE,
        "method_nested",
        "CODEGEN-002",
        Some("12:7"),
        &method("D::inner"),
    );
    sim_cli::reject_case_with_id(
        SUITE,
        "method_from_task",
        "CODEGEN-002",
        Some("12:11"),
        &method("D::run"),
    );
    sim_cli::reject_case_with_id(
        SUITE,
        "method_hierarchical",
        "CODEGEN-002",
        Some("9:9"),
        &method("D::run"),
    );
    sim_cli::reject_case_with_id(
        SUITE,
        "method_select",
        "CODEGEN-002",
        Some("13:11"),
        "variable storage `tb.s[0]` has both",
    );
    sim_cli::reject_case_with_id(
        SUITE,
        "method_output_actual",
        "CODEGEN-002",
        Some("16:15"),
        &method("D::run"),
    );
    sim_cli::reject_case_with_id(
        SUITE,
        "method_continuous",
        "CODEGEN-002",
        Some("10:11"),
        "a continuous assignment by `tb.continuous` at ",
    );
    sim_cli::reject_case_with_id(SUITE, "method_task", "CODEGEN-002", Some("12:11"), "semantic error: variable storage `tb.s` has both a continuous assignment by `tb.d.port` at ",
    );
    sim_cli::reject_case_with_id(
        SUITE,
        "method_task",
        "CODEGEN-002",
        Some("12:11"),
        ":3:42 and a procedural assignment in class method `D::run`",
    );
}

/// Only subroutines a process reaches through statically known callees are
/// checked (A21-D8): a class task nothing calls, and an override called
/// through a base-class handle, still run.
#[test]
fn uncalled_and_overriding_methods_are_not_checked() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "method_uncalled",
        fixture!("method_uncalled.out"),
        &[],
        &[],
    );
    sim_cli::run_case_backend_parity(
        SUITE,
        "method_virtual_base",
        fixture!("method_virtual_base.out"),
        &[],
        &[],
    );
}
