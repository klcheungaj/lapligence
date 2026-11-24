//! A validated node costs no path text: a path is rendered only into the error
//! that reports it. A path is as long as its node is deep, so one rendered per
//! visit makes the validation of an operator chain quadratic in its length.

use super::*;

const CHAIN_TERMS: usize = 2_000;
const STATEMENT_DEPTH: usize = 200;

/// Room for the recursive checks of [`CHAIN_TERMS`] levels in an unoptimized
/// build.
const DEEP_STACK_BYTES: usize = 512 << 20;

/// The root of a path, counting how often the path is rendered.
#[derive(Default)]
struct CountedRoot(Cell<usize>);

impl fmt::Display for CountedRoot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.set(self.0.get() + 1);
        f.write_str("root")
    }
}

fn on_deep_stack(check: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(DEEP_STACK_BYTES)
        .spawn(check)
        .expect("test thread")
        .join()
        .expect("deep check");
}

/// `first + sig + sig + ...` with `terms` operators.
fn chain(first: IrExpr, terms: usize) -> IrExpr {
    (0..terms).fold(first, |a, _| {
        IrExpr::new(
            IrExprKind::Bin {
                op: IrBinOp::Add,
                a: Box::new(a),
                b: Box::new(signal_read(0)),
            },
            1,
            false,
            None,
        )
    })
}

fn signal_read(index: usize) -> IrExpr {
    IrExpr::new(IrExprKind::SigRead(index), 1, false, None)
}

fn if_statement(cond: IrExpr, then_: Vec<IrStmt>) -> IrStmt {
    IrStmt::If {
        cond,
        then_,
        els: None,
        check: IrUniquePriorityCheck::None,
    }
}

fn nested_ifs(condition: &IrExpr, depth: usize) -> Vec<IrStmt> {
    (0..depth).fold(Vec::new(), |body, _| {
        vec![if_statement(condition.clone(), body)]
    })
}

#[test]
fn a_valid_operator_chain_renders_no_path() {
    on_deep_stack(|| {
        let model = valid_model();
        let root = CountedRoot::default();
        Validator::new(&model)
            .validate_expr(
                &chain(signal_read(0), CHAIN_TERMS),
                &[],
                format_args!("{root}"),
            )
            .expect("valid chain");
        assert_eq!(root.0.get(), 0);
    });
}

#[test]
fn valid_nested_statements_render_no_path() {
    on_deep_stack(|| {
        let model = valid_model();
        let root = CountedRoot::default();
        Validator::new(&model)
            .validate_stmts(
                &nested_ifs(&chain(signal_read(0), 3), STATEMENT_DEPTH),
                &[],
                format_args!("{root}"),
            )
            .expect("valid statements");
        assert_eq!(root.0.get(), 0);
    });
}

#[test]
fn an_invalid_chain_renders_its_path_once() {
    on_deep_stack(|| {
        let model = valid_model();
        let root = CountedRoot::default();
        let error = Validator::new(&model)
            .validate_expr(
                &chain(signal_read(7), CHAIN_TERMS),
                &[],
                format_args!("{root}"),
            )
            .expect_err("the first operand reads no signal");
        assert_eq!(root.0.get(), 1);
        assert_eq!(error.path(), format!("root{}", ".a".repeat(CHAIN_TERMS)));
        assert_eq!(error.detail(), "signal index 7 is out of bounds");
    });
}

#[test]
fn an_invalid_nested_statement_renders_its_path_once() {
    on_deep_stack(|| {
        let model = valid_model();
        let root = CountedRoot::default();
        let mut body = nested_ifs(&signal_read(0), STATEMENT_DEPTH);
        let mut innermost = &mut body;
        for _ in 1..STATEMENT_DEPTH {
            let IrStmt::If { then_, .. } = &mut innermost[0] else {
                unreachable!("nested_ifs builds if statements");
            };
            innermost = then_;
        }
        innermost.push(if_statement(signal_read(7), Vec::new()));
        let error = Validator::new(&model)
            .validate_stmts(&body, &[], format_args!("{root}"))
            .expect_err("the innermost condition reads no signal");
        assert_eq!(root.0.get(), 1);
        assert_eq!(error.detail(), "signal index 7 is out of bounds");
        let expected = format!("root{}[1].cond", "[0].then".repeat(STATEMENT_DEPTH - 1));
        assert_eq!(error.path(), expected);
    });
}
