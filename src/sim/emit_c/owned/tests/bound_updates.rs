//! Nonblocking writes whose updates end with their issuer (decision S09-D8).
use super::*;
use crate::sim::emit_c::model::render_with_sharing_threshold;
use crate::sim::ir::LLG_OUTLINE_MIN_RUN_WEIGHT;

const BEGIN: &str = "llg_nba_bind_begin();";
const END: &str = "llg_nba_bind_end();";

fn render(body: Vec<IrStmt>) -> String {
    let mut model = IrModel::new("bound".to_owned(), 1).unwrap();
    model.signals.push(
        IrSignal::new(
            "G_a".to_owned(),
            None,
            IrType::Packed {
                width: 32,
                signed: false,
                two_state: false,
            },
            None,
        )
        .unwrap(),
    );
    model.events.push(IrEvent::new("E_e".to_owned()));
    let mut process = IrProcess::new(
        "p_run".to_owned(),
        "tb".to_owned(),
        IrShape::RunOnce,
        Vec::new(),
        body,
    );
    process.origin = crate::sim::semantic::Origin::Synthetic {
        reason: "one source process".to_owned(),
    };
    model.spawns.push(process.c_name.clone());
    model.processes.push(process);
    render_with_sharing_threshold(&ExecutionModel::lower(model).unwrap(), 4).unwrap()
}

/// `G_a <= G_a + ... + G_a;` with `reads` operands.
fn sum(reads: usize) -> IrStmt {
    let read = || IrExpr::new(IrExprKind::SigRead(0), 32, false, None);
    let rhs = (1..reads).fold(read(), |sum, _| {
        IrExpr::new(
            IrExprKind::Bin {
                op: IrBinOp::Add,
                a: Box::new(sum),
                b: Box::new(read()),
            },
            32,
            false,
            None,
        )
    });
    IrStmt::Assign {
        lhs: IrLhs::Whole(0),
        rhs,
        nba: true,
    }
}

fn bound(statement: IrStmt) -> Vec<IrStmt> {
    let mark = |operation| IrStmt::Container(Box::new(operation));
    vec![
        mark(IrContainerStmt::NbaBindOpen),
        statement,
        mark(IrContainerStmt::NbaBindClose),
    ]
}

/// Only the write between the marks is bound, and its value is computed
/// before the updates it queues are told apart.
#[test]
fn a_bound_write_marks_the_updates_it_queues() {
    let mut body = vec![sum(1)];
    body.extend(bound(sum(2)));
    body.push(sum(3));
    let source = render(body);
    assert_eq!(source.matches(BEGIN).count(), 1, "{source}");
    let begin = source.find(BEGIN).unwrap();
    let end = source.find(END).unwrap();
    assert!(source[begin..end].contains("llg_nba_after("));
    assert!(!source[begin..end].contains("sv4_add"), "{source}");
    assert_eq!(source.matches("llg_nba_after(").count(), 3);
}

/// A bound write heavy enough for a helper of its own stays bound there.
#[test]
fn a_bound_write_stays_bound_in_an_outlined_run() {
    let source = render(
        (0..12)
            .flat_map(|_| bound(sum(LLG_OUTLINE_MIN_RUN_WEIGHT / 2)))
            .collect(),
    );
    let helpers = source.matches("static int p_run_llg_run_").count();
    assert!(helpers > 0, "{source}");
    assert_eq!(source.matches(BEGIN).count(), 12);
    for (begin, _) in source.match_indices(BEGIN) {
        let end = source[begin..].find(END).unwrap() + begin;
        assert_eq!(source[begin..end].matches("llg_nba_after(").count(), 1);
    }
}

/// A plain trigger stays pending when its process ends, a timed one does not:
/// the runtime entries differ even for a zero delay.
#[test]
fn plain_and_timed_triggers_use_their_own_entries() {
    let trigger = |ticks| IrStmt::NonblockingEventTrigger {
        ev: IrEventRef::Static(0),
        ticks,
    };
    let source = render(vec![
        trigger(None),
        trigger(Some(IrDelay::Constant(0))),
        trigger(Some(IrDelay::Constant(4))),
    ]);
    assert_eq!(source.matches("llg_nba_event(").count(), 1, "{source}");
    assert_eq!(source.matches("llg_nba_event_after(").count(), 2);
}
