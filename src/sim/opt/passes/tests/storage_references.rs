//! The unused-storage pass keeps every signal an IR construct references,
//! including references that are not plain expression reads: event-handle
//! indices, wait specifications, call arguments and C-address targets.
use super::*;

fn read(signal: usize) -> IrExpr {
    IrExpr::new(IrExprKind::SigRead(signal), 32, false, None)
}

/// `events[s<signal>]`: an element of event array 0 selected by a signal.
fn event_element(signal: usize) -> IrEventRef {
    IrEventRef::Array {
        array: 0,
        indices: vec![read(signal)],
    }
}

fn with_event_array(mut model: IrModel) -> IrModel {
    model.events.push(crate::sim::ir::IrEvent {
        c_name: "E_t_events".to_string(),
        array_dims: Some(vec![(0, 1)]),
        array_elements: Vec::new(),
    });
    model
}

/// Signal 0 is never referenced; every other signal must keep its storage.
fn assert_only_first_omitted(model: &IrModel) {
    assert!(model.signals[0].omit, "untouched signal is still omitted");
    for (index, signal) in model.signals.iter().enumerate().skip(1) {
        assert!(!signal.omit, "signal {index} lost its storage");
    }
}

#[test]
fn unused_storage_keeps_event_handle_indices_of_every_event_operation() {
    // `->events[i]`, `->>events[i]`, `events[i] = events[j]`, a captured
    // handle, `wait(events[i].triggered)`, `wait_order(events[i], ...)` and
    // `events[i].triggered` evaluate the index where the operation is issued.
    let body = vec![
        IrStmt::EventTrigger {
            ev: event_element(1),
        },
        IrStmt::ClockingEventTrigger {
            ev: event_element(2),
        },
        IrStmt::NonblockingEventTrigger {
            ev: event_element(3),
            ticks: None,
        },
        IrStmt::NonblockingEventTriggerWhen {
            ev: event_element(4),
            specs: Vec::new(),
            repeat: None,
        },
        IrStmt::EventAssign {
            target: event_element(5),
            source: Some(event_element(6)),
        },
        IrStmt::EventCapture {
            name: "captured".to_string(),
            source: event_element(7),
        },
        IrStmt::WaitEventTriggered {
            event: event_element(8),
            body: Vec::new(),
        },
        IrStmt::WaitOrder {
            events: vec![event_element(9)],
            success: Vec::new(),
            failure: Vec::new(),
        },
        IrStmt::If {
            cond: IrExpr::new(
                IrExprKind::EventTriggered(event_element(10)),
                1,
                false,
                None,
            ),
            then_: Vec::new(),
            els: None,
            check: IrUniquePriorityCheck::None,
        },
    ];
    let mut model = with_event_array(model_with(body, sigs(11)));
    run(&mut model, &storage_only());
    assert_only_first_omitted(&model);
}

#[test]
fn unused_storage_keeps_event_handle_indices_of_call_arguments() {
    // `t(events[i])` passes an event handle selected at the call site.
    let body = vec![IrStmt::Call(Box::new(IrCall::new(
        0,
        vec![
            IrCallArg::EventVal(event_element(1)),
            IrCallArg::EventAddr(event_element(2)),
        ],
        IrDepth::PROC,
        Vec::new(),
        Vec::new(),
    )))];
    let mut model = with_event_array(model_with(body, sigs(3)));
    run(&mut model, &storage_only());
    assert_only_first_omitted(&model);
}

#[test]
fn unused_storage_keeps_wait_sources_of_clocked_nonblocking_operations() {
    // `->> repeat(n) @(posedge c) ev`, `a <= @(posedge c) b` and a clocking
    // drive register their wait sources when issued.
    let posedge = |signal: usize| (IrWaitSrc::Sig(format!("G_s{signal}")), IrEdge::Posedge);
    let body = vec![
        IrStmt::NonblockingEventTriggerWhen {
            ev: IrEventRef::Static(0),
            specs: vec![posedge(1)],
            repeat: Some(read(2)),
        },
        IrStmt::NonblockingEventAssignWhen {
            lhs: IrLhs::Whole(3),
            rhs: Box::new(konst(0, 8)),
            specs: vec![posedge(4)],
            repeat: None,
            action: "a_t_nba_0".to_string(),
            frame: crate::sim::ir::FrameId::new(0),
            captures: Vec::new(),
        },
        IrStmt::ClockingDrive {
            lhs: IrLhs::Whole(5),
            rhs: konst(0, 8),
            ticks: crate::sim::ir::IrDelay::Constant(0),
            specs: vec![posedge(6)],
            cycles: None,
        },
    ];
    let mut model = with_event_array(model_with(body, sigs(7)));
    run(&mut model, &storage_only());
    assert_only_first_omitted(&model);
}
