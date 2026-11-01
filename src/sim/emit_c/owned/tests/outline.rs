//! Statement runs of long bodies outlined into helper functions.
use super::*;
use crate::sim::emit_c::model::render_with_sharing_threshold;
use crate::sim::ir::{LLG_OUTLINE_MIN_RUN_WEIGHT, LLG_OUTLINE_RUN_WEIGHT};

/// Weight of [`increment`]: the assignment, its lvalue and three expression
/// nodes.
const INCREMENT_WEIGHT: usize = 5;

fn signal(name: &str) -> IrSignal {
    IrSignal::new(
        name.to_owned(),
        None,
        IrType::Packed {
            width: 32,
            signed: false,
            two_state: false,
        },
        None,
    )
    .unwrap()
}

/// `G_a = G_a + step;`
fn increment(step: u64) -> IrStmt {
    IrStmt::Assign {
        lhs: IrLhs::Whole(0),
        rhs: IrExpr::new(
            IrExprKind::Bin {
                op: IrBinOp::Add,
                a: Box::new(IrExpr::new(IrExprKind::SigRead(0), 32, false, None)),
                b: Box::new(number(step, 32)),
            },
            32,
            false,
            None,
        ),
        nba: false,
    }
}

/// `G_a = G_a + x;` for the local `x`.
fn add_local() -> IrStmt {
    IrStmt::Assign {
        lhs: IrLhs::Whole(0),
        rhs: IrExpr::new(
            IrExprKind::Bin {
                op: IrBinOp::Add,
                a: Box::new(IrExpr::new(IrExprKind::SigRead(0), 32, false, None)),
                b: Box::new(IrExpr::new(
                    IrExprKind::LocalRead("x".to_owned()),
                    32,
                    false,
                    None,
                )),
            },
            32,
            false,
            None,
        ),
        nba: false,
    }
}

fn increments(count: usize, first: u64) -> Vec<IrStmt> {
    (0..count as u64)
        .map(|step| increment(first + step))
        .collect()
}

/// One `initial` per instance, all from one source location.
fn model_with(instances: usize, body: Vec<IrStmt>) -> ExecutionModel {
    let mut model = IrModel::new("outline".to_owned(), 1).unwrap();
    model.signals.push(signal("G_a"));
    for index in 0..instances {
        let mut process = IrProcess::new(
            format!("p_run{index}"),
            format!("tb.u{index}"),
            IrShape::RunOnce,
            Vec::new(),
            body.clone(),
        );
        process.origin = crate::sim::semantic::Origin::Synthetic {
            reason: "one source process".to_owned(),
        };
        model.spawns.push(process.c_name.clone());
        model.processes.push(process);
    }
    ExecutionModel::lower(model).unwrap()
}

fn render(instances: usize, body: Vec<IrStmt>) -> String {
    render_with_sharing_threshold(&model_with(instances, body), 4).unwrap()
}

/// Text of the helper `name`, from its signature to its closing brace.
fn helper<'s>(source: &'s str, name: &str) -> &'s str {
    let start = source
        .find(&format!("static int {name}(sv4_t* _llg_t) {{\n"))
        .unwrap_or_else(|| panic!("missing helper {name}"));
    let end = source[start..].find("\n}\n").unwrap() + start;
    &source[start..end]
}

/// The body function of process `name`.
fn body<'s>(source: &'s str, name: &str) -> &'s str {
    let start = source
        .find(&format!(
            "static llg_co_status_t {name}(llg_co_frame_t* co, llg_co_chain_t* ch) {{\n"
        ))
        .unwrap();
    let end = source[start..].find("\n}\n").unwrap() + start;
    &source[start..end]
}

#[test]
fn bodies_up_to_one_run_are_emitted_unchanged() {
    let count = LLG_OUTLINE_RUN_WEIGHT / INCREMENT_WEIGHT;
    let source = render(1, increments(count, 1));
    assert!(!source.contains("_llg_run_"));
    assert_eq!(body(&source, "p_run0").matches("sv4_add").count(), count);
}

#[test]
fn long_bodies_split_into_runs_of_at_most_the_run_weight() {
    let per_run = LLG_OUTLINE_RUN_WEIGHT / INCREMENT_WEIGHT;
    let count = per_run * 3 + 7;
    let source = render(1, increments(count, 1));
    let main = body(&source, "p_run0");
    let mut seen = 0;
    for index in 0..3 {
        let name = format!("p_run0_llg_run_{index}");
        let text = helper(&source, &name);
        assert_eq!(text.matches("sv4_add").count(), per_run, "{name}");
        // Statement order is kept: each run starts with the next step.
        assert!(text.contains(&format!("{}ULL", seen + 1)), "{name}");
        seen += per_run;
        assert!(main.contains(&format!(
            "if (LLG_CO_UNLIKELY({name}(_llg_t))) return LLG_CO_EXIT;"
        )));
        // Helpers are defined before the body that calls them.
        assert!(source.find(text).unwrap() < source.find(main).unwrap());
    }
    // The 7-statement tail is too light to leave the body.
    const { assert!(7 * INCREMENT_WEIGHT < LLG_OUTLINE_MIN_RUN_WEIGHT) };
    assert!(!source.contains("p_run0_llg_run_3"));
    assert_eq!(main.matches("sv4_add").count(), count - seen);
    assert!(main.contains(&format!("{}ULL", seen + 1)));
}

#[test]
fn suspensions_and_body_locals_never_enter_a_run() {
    let per_run = LLG_OUTLINE_RUN_WEIGHT / INCREMENT_WEIGHT;
    let mut statements = increments(per_run, 1);
    statements.push(IrStmt::Delay {
        ticks: IrDelay::Constant(1),
    });
    statements.extend(increments(per_run, 1000));
    statements.push(IrStmt::DeclLocal {
        name: "x".to_owned(),
        width: 32,
        signed: false,
        init: Some(Box::new(number(3, 32))),
        two_state: false,
    });
    statements.extend((0..per_run).map(|_| add_local()));
    // A block that declares its own local is self-contained.
    let mut own = vec![IrStmt::DeclLocal {
        name: "x".to_owned(),
        width: 32,
        signed: false,
        init: None,
        two_state: false,
    }];
    own.extend((0..per_run - 1).map(|_| add_local()));
    statements.push(IrStmt::Block(own));
    let source = render(1, statements);
    let main = body(&source, "p_run0");
    // The delay's resume point stays in the coroutine, between two runs.
    let first = main.find("p_run0_llg_run_0(_llg_t)").unwrap();
    let delay = main.find("LLG_CO_AWAIT").unwrap();
    let second = main.find("p_run0_llg_run_1(_llg_t)").unwrap();
    assert!(first < delay && delay < second);
    for index in 0..3 {
        assert!(!helper(&source, &format!("p_run0_llg_run_{index}")).contains("LLG_CO_"));
    }
    // Statements reading the body's `x` stay in the body; the block with its
    // own `x` leaves it.
    let inline_reads = main.matches("sv4_add").count();
    assert_eq!(inline_reads, per_run);
    assert_eq!(
        helper(&source, "p_run0_llg_run_2")
            .matches("sv4_add")
            .count(),
        per_run - 1
    );
    assert!(!source.contains("p_run0_llg_run_3"));
}

#[test]
fn jumps_out_of_a_statement_keep_it_inline() {
    let per_run = LLG_OUTLINE_RUN_WEIGHT / INCREMENT_WEIGHT;
    let mut inner = increments(per_run, 1);
    inner.push(IrStmt::If {
        cond: IrExpr::new(IrExprKind::SigRead(0), 32, false, None),
        then_: vec![IrStmt::Goto("done".to_owned())],
        els: None,
        check: IrUniquePriorityCheck::None,
    });
    inner.extend(increments(per_run, 1000));
    let statements = vec![
        IrStmt::Block(inner),
        IrStmt::Label("done".to_owned()),
        increment(7),
    ];
    let source = render(1, statements);
    let main = body(&source, "p_run0");
    // The conditional jump to `done` is emitted in the body, between the
    // two runs around it.
    let first = main.find("p_run0_llg_run_0(_llg_t)").unwrap();
    let jump = main.find("goto done;").unwrap();
    let second = main.find("p_run0_llg_run_1(_llg_t)").unwrap();
    assert!(first < jump && jump < second);
    assert!(!source.contains("p_run0_llg_run_2"));
}

#[test]
fn bodies_instance_sharing_may_merge_stay_whole() {
    let per_run = LLG_OUTLINE_RUN_WEIGHT / INCREMENT_WEIGHT;
    let statements = increments(per_run * 2, 1);
    // Four instances of one location form a sharing group: no helpers.
    let shared = render(4, statements.clone());
    assert!(shared.contains("llg_shared_body_0("));
    assert!(!shared.contains("_llg_run_"));
    // Three cannot share, so each outlines its own runs.
    let unshared = render(3, statements);
    assert!(!unshared.contains("llg_shared_body"));
    for index in 0..3 {
        assert!(unshared.contains(&format!("static int p_run{index}_llg_run_1(sv4_t* _llg_t)")));
    }
}
