//! class-defects: class-method forms that gate-fixes-2 found rejected at
//! compile time. Named blocks inside class methods (declarations, `disable`
//! from inside the method and from another process, `%m`), and class
//! parameters read in method bodies, each specialization with its own
//! values. Every expected line is derived in the fixture readme from IEEE
//! 1800-2009. Positive cases run in both optimizer modes on the legacy,
//! compact/portable and compact/GMP value backends.
use super::sim_cli;

const SUITE: &str = "feature_completion/class_defects";

/// Item 1: named `begin`-`end` and `fork`-`join` blocks in static, virtual
/// and parameterized-class methods, with automatic and static block locals,
/// `disable` as `break` and `continue`, a hierarchical read of a static block
/// local, and `%m` naming the blocks (decision CD-D2).
#[test]
fn named_blocks_in_class_methods() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "named_blocks",
        include_str!("../fixtures/sim/feature_completion/class_defects/named_blocks.out"),
        &[],
        &[],
    );
}

/// Item 1: a `disable` from another process ends every active execution of
/// a class method's named block or of a class task, in every object; a
/// disable of an idle block has no effect (decision CD-D1).
#[test]
fn class_block_disable_from_another_process() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "named_block_disable",
        include_str!("../fixtures/sim/feature_completion/class_defects/named_block_disable.out"),
        &[],
        &[],
    );
}

/// Item 1 negatives: a `disable` of a function, and a hierarchical reference
/// to an automatic block variable of a method.
#[test]
fn class_block_misuse_is_rejected() {
    sim_cli::reject_case(
        SUITE,
        "neg_disable_function",
        "neg_disable_function.sv:11:13 disable statement must specify a task or block name",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_block_automatic_reference",
        "neg_block_automatic_reference.sv:11:17 cannot refer to automatic variable via \
         hierarchical reference",
    );
}
