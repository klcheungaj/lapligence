//! Cancellation checks follow resume points, `disable`, calls that may
//! disable, and nested activation exits only (design §13.2 rule 1).
use super::*;

const CHECK: &str = "llg_activation_cancelled()";

fn render(model: IrModel) -> String {
    let execution = ExecutionModel::lower(model).expect("validate cancellation model");
    super::super::super::model::render(&execution).expect("emit cancellation model")
}

fn assign(value: u64) -> IrStmt {
    IrStmt::Assign {
        lhs: IrLhs::Whole(0),
        rhs: number(value, 65),
        nba: false,
    }
}

fn delay() -> IrStmt {
    IrStmt::Delay {
        ticks: IrDelay::Constant(1),
    }
}

fn scope(target: IrActivationTarget, exit: &str, body: Vec<IrStmt>) -> IrStmt {
    IrStmt::ActivationScope {
        target,
        exit: exit.to_owned(),
        body,
    }
}

fn void_function(name: &str, body: Vec<IrStmt>) -> IrFunc {
    IrFunc::new(name.to_owned(), None, vec![], vec![], vec![], body)
}

fn call(function: usize) -> IrStmt {
    IrStmt::Call(Box::new(IrCall::new(
        function,
        vec![],
        IrDepth::PROC,
        vec![],
        vec![],
    )))
}

#[test]
fn functions_without_resume_points_or_disable_have_no_checks() {
    let source = render(numeric_model());
    assert!(!source.contains(CHECK), "{source}");
}

#[test]
fn named_block_checks_only_after_its_resume_point() {
    let mut model = numeric_model();
    let target = IrActivationTarget::new(100, 1);
    model.processes[0].body = vec![scope(
        target,
        "block_exit",
        vec![assign(1), delay(), assign(2), assign(3)],
    )];
    let source = render(model);
    assert_eq!(source.matches(CHECK).count(), 1, "{source}");
    let resume = source.find("llg_arm_time(").expect("delay await");
    let check = source.find(CHECK).expect("check");
    assert!(resume < check, "{source}");
}

#[test]
fn disable_in_one_branch_is_checked_inside_that_branch() {
    let mut model = numeric_model();
    let target = IrActivationTarget::new(100, 1);
    model.processes[0].body = vec![scope(
        target,
        "block_exit",
        vec![
            IrStmt::If {
                cond: IrExpr::new(IrExprKind::SigRead(0), 65, false, None),
                then_: vec![IrStmt::DisableTarget { target }, assign(1)],
                els: Some(vec![assign(2)]),
                check: IrUniquePriorityCheck::None,
            },
            assign(3),
        ],
    )];
    let source = render(model);
    assert_eq!(source.matches(CHECK).count(), 1, "{source}");
    let disable = source.find("llg_disable_target(").expect("disable");
    let check = source.find(CHECK).expect("check");
    let other_branch = source[disable..].find("else").expect("else branch") + disable;
    assert!(disable < check && check < other_branch, "{source}");
}

#[test]
fn only_calls_that_may_disable_are_checked() {
    let mut model = numeric_model();
    let target = IrActivationTarget::new(100, 1);
    model.funcs = vec![
        void_function("f_plain", vec![assign(4)]),
        void_function("f_stop", vec![IrStmt::DisableTarget { target }]),
    ];
    model.processes[0].body = vec![scope(
        target,
        "block_exit",
        vec![call(0), assign(1), call(1), assign(2)],
    )];
    let source = render(model);
    let start = source.find("p_numeric_frame_t* F =").expect("process body");
    let end = source[start..].find("block_exit: ;").expect("scope exit") + start;
    let process = &source[start..end];
    assert_eq!(process.matches(CHECK).count(), 1, "{source}");
    let stop = process.find("f_stop(0);").expect("disabling call");
    assert!(process.find("f_plain(0);").expect("plain call") < stop);
    assert!(process.find(CHECK).expect("check") > stop, "{source}");
    assert!(
        source.contains(&format!("if ({CHECK}) goto _llg_return;")),
        "the disabling callee returns early: {source}"
    );
}

#[test]
fn nested_activation_exit_rechecks_the_enclosing_activation() {
    let mut model = numeric_model();
    let outer = IrActivationTarget::new(101, 1);
    let inner = IrActivationTarget::new(102, 1);
    model.processes[0].body = vec![scope(
        outer,
        "outer_exit",
        vec![scope(inner, "inner_exit", vec![delay()]), assign(1)],
    )];
    let source = render(model);
    assert_eq!(source.matches(CHECK).count(), 2, "{source}");
    let inner_label = source.find("inner_exit: ;").expect("inner exit label");
    let recheck = source[inner_label..].find(CHECK).expect("recheck") + inner_label;
    let outer_goto = source[recheck..]
        .find("goto outer_exit;")
        .expect("outer goto")
        + recheck;
    assert!(outer_goto < source.find("outer_exit: ;").expect("outer label"));
}
