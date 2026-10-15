//! The expression walker reaches every selector slot of lvalues, event
//! references and delays, so the folding and identity passes see them.
use super::*;
use crate::sim::ir::{IrDelay, IrPackedSelect};

fn sig_read(index: usize) -> IrExpr {
    IrExpr::new(IrExprKind::SigRead(index), 32, false, None)
}

fn event_array(index: usize) -> IrEventRef {
    IrEventRef::Array {
        array: 0,
        indices: vec![sig_read(index)],
    }
}

/// Signals read by the expression slots the walker visits in `body`.
fn visited_reads(body: Vec<IrStmt>) -> Vec<usize> {
    let mut model = model_with(body, sigs(4));
    let mut seen = Vec::new();
    walk_model_exprs_mut(&mut model, &mut |expr| {
        if let IrExprKind::SigRead(index) = expr.kind {
            seen.push(index);
        }
    });
    seen
}

#[test]
fn release_target_selectors_are_visited() {
    let lhs = IrLhs::Bit(0, Box::new(sig_read(1)), false);
    assert_eq!(visited_reads(vec![IrStmt::Release { lhs }]), [1]);
}

#[test]
fn packed_select_steps_of_an_assignment_target_are_visited() {
    let lhs = IrLhs::PackedSelect {
        target: Box::new(IrLhs::Whole(0)),
        steps: vec![IrPackedSelect {
            base: sig_read(2),
            width: 1,
        }],
        signed: false,
        two_state: false,
    };
    let body = vec![IrStmt::Assign {
        lhs,
        rhs: konst(1, 1),
        nba: false,
    }];
    assert_eq!(visited_reads(body), [2]);
}

#[test]
fn event_reference_indices_are_visited_in_every_event_statement() {
    let body = vec![
        IrStmt::EventTrigger { ev: event_array(1) },
        IrStmt::ClockingEventTrigger { ev: event_array(2) },
        IrStmt::EventAssign {
            target: event_array(3),
            source: Some(event_array(1)),
        },
        IrStmt::WaitOrder {
            events: vec![event_array(2), event_array(3)],
            success: Vec::new(),
            failure: Vec::new(),
        },
    ];
    let mut seen = visited_reads(body);
    seen.sort_unstable();
    assert_eq!(seen, [1, 1, 2, 2, 3, 3]);
}

#[test]
fn triggered_expression_index_is_visited() {
    let triggered = IrExpr::new(IrExprKind::EventTriggered(event_array(1)), 1, false, None);
    let body = vec![IrStmt::If {
        cond: triggered,
        then_: Vec::new(),
        els: None,
        check: IrUniquePriorityCheck::None,
    }];
    assert_eq!(visited_reads(body), [1]);
}

#[test]
fn clocking_wait_sources_event_indices_are_visited() {
    let body = vec![IrStmt::ClockingCycleWait {
        count: konst(1, 32),
        specs: vec![(IrWaitSrc::Event(event_array(2)), IrEdge::Any)],
    }];
    assert_eq!(visited_reads(body), [2]);
}

#[test]
fn string_and_handle_delayed_assignments_expose_their_delay_expression() {
    let ticks = || IrDelay::Runtime {
        value: Box::new(sig_read(1)),
        unit_ticks: 1,
        precision_ticks: 1,
    };
    let body = vec![
        IrStmt::DelayedStringAssign {
            target: "s".to_string(),
            rhs: crate::sim::ir::IrStringExpr::Literal(Vec::new()),
            ticks: ticks(),
        },
        IrStmt::DelayedChandleAssign {
            target: "h".to_string(),
            rhs: crate::sim::ir::IrChandleExpr::Null,
            ticks: ticks(),
        },
    ];
    assert_eq!(visited_reads(body), [1, 1]);
}
