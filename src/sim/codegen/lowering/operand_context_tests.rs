//! Operand types of context-determined operators, and the size of the IR of
//! long operator chains.
use super::ir_mutation_tests::lowered;
use super::*;
use crate::sim::opt::walk_model_exprs_mut;

/// Operators of a chain the frontend parses in a loop.
const CHAIN_TERMS: usize = 500;

/// Operators of a chain the frontend parses recursively.
const NESTED_TERMS: usize = 100;

/// Bound of the IR nodes per operator of a chain: the operator, its operands
/// other than the rest of the chain, and the conversions of those.
const NODES_PER_TERM: usize = 10;

/// Bound of the IR nodes of the statement around a chain.
const STATEMENT_NODES: usize = 16;

/// Room for lowering [`CHAIN_TERMS`] levels in an unoptimized build.
const DEEP_STACK_BYTES: usize = 512 << 20;

fn on_deep_stack(check: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(DEEP_STACK_BYTES)
        .spawn(check)
        .expect("test thread")
        .join()
        .expect("deep lowering");
}

fn assert_ir_grows_with_the_terms(chain: &str, terms: usize) {
    let mut model = lowered(&format!(
        "module tb; logic [7:0] a, b; logic [11:0] y; initial y = {chain}; endmodule\n"
    ));
    let mut nodes = 0;
    walk_model_exprs_mut(&mut model, &mut |_| nodes += 1);
    assert!(
        nodes > terms && nodes <= NODES_PER_TERM * terms + STATEMENT_NODES,
        "{nodes} IR nodes for {terms} operators"
    );
}

#[test]
fn component_operator_chains_lower_to_ir_of_their_length() {
    on_deep_stack(|| {
        assert_ir_grows_with_the_terms(
            &("a".to_owned() + &" + b".repeat(CHAIN_TERMS)),
            CHAIN_TERMS,
        );
        assert_ir_grows_with_the_terms(
            &("a".to_owned() + &" ^ b".repeat(CHAIN_TERMS)),
            CHAIN_TERMS,
        );
        assert_ir_grows_with_the_terms(&("a[0] ? b : ".repeat(NESTED_TERMS) + "a"), NESTED_TERMS);
    });
}

/// Each operand of a context-determined operator has the type of the operator:
/// a change of the common type along a chain reaches the operands below it.
#[test]
fn component_operands_take_the_type_of_their_context() {
    for value in [
        "n + n + w + w",
        "n + s + w + n + w",
        "w + (n + (n + s))",
        "s + s - n * s / w % n",
        "(s & s) | (n ^ s ^ w) ~^ s",
        "c ? s : c ? n + s : c ? w : s - s",
        "w + (c ? s + s : c ? n : s) + s",
        "(c ? s : s + s) + (c ? (s + s) : n) + w",
        "x + s + s + w",
    ] {
        let mut model = lowered(&format!(
            "module tb; logic c; logic [3:0] n; logic signed [5:0] s; logic [8:0] w;\n  \
             logic signed [15:0] x; logic [19:0] y;\n  initial y = {value};\nendmodule\n"
        ));
        let mut operators = 0;
        walk_model_exprs_mut(&mut model, &mut |expr| {
            let operands: [&IrExpr; 2] = match &expr.kind {
                IrExprKind::Bin { op, a, b } if is_context_binary(*op) => [a, b],
                IrExprKind::Mux { a, b, .. } => [a, b],
                _ => return,
            };
            operators += 1;
            for operand in operands {
                assert_eq!(
                    (operand.width, operand.signed),
                    (expr.width, expr.signed),
                    "`{value}`: {operand:?}"
                );
            }
        });
        assert!(operators >= 3, "`{value}` has {operators} operators");
    }
}
