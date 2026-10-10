//! Arm-only specification arrays are compound literals in the arm call, never
//! frame fields (design §7.1 rule 2, §13.2 rule 3); arrays the runtime keeps
//! stay in the frame.
use super::*;

fn render(statements: Vec<IrStmt>) -> String {
    let mut model = numeric_model();
    model
        .signals
        .push(IrSignal::new("G_other".to_owned(), None, model.signals[0].ty, None).unwrap());
    model.events.push(IrEvent::new("E_first".to_owned()));
    model.events.push(IrEvent::new("E_second".to_owned()));
    model.processes[0].body = statements;
    let execution = ExecutionModel::lower(model).expect("validate arm model");
    super::super::super::model::render(&execution).expect("emit arm model")
}

fn arm_line(source: &str, arm: &str) -> String {
    let lines = source
        .lines()
        .filter(|line| line.contains(arm))
        .collect::<Vec<_>>();
    assert_eq!(lines.len(), 1, "{arm}: {source}");
    lines[0].replace("LLG_CO_OWNER(ch, llg_proc_t)", "self")
}

fn assert_no_array_copy(source: &str) {
    assert!(
        !source.contains("memcpy(F->"),
        "arm arrays must not be copied into the frame: {source}"
    );
}

fn events() -> Vec<IrEventRef> {
    vec![IrEventRef::Static(0), IrEventRef::Static(1)]
}

fn signal(name: &str, edge: IrEdge) -> (IrWaitSrc, IrEdge) {
    (IrWaitSrc::Sig(name.to_owned()), edge)
}

#[test]
fn event_list_wait_passes_a_compound_literal() {
    let source = render(vec![IrStmt::WaitEvents {
        specs: events()
            .into_iter()
            .map(|event| (IrWaitSrc::Event(event), IrEdge::Any))
            .collect(),
        refresh: false,
    }]);
    let line = arm_line(&source, "llg_arm_events(");
    assert!(
        line.contains("LLG_CO_AWAIT(co, ch, 1, llg_arm_events(self, (const llg_event_t*[]){ &E_first, &E_second }, 2));"),
        "{line}"
    );
    assert_no_array_copy(&source);
    assert!(!source.contains("_llg_events_"), "{source}");
}

#[test]
fn edge_list_wait_passes_a_compound_literal() {
    let source = render(vec![IrStmt::WaitEvents {
        specs: vec![
            signal("G_value", IrEdge::Posedge),
            signal("G_other", IrEdge::Negedge),
        ],
        refresh: false,
    }]);
    let line = arm_line(&source, "llg_arm_any_events(");
    assert!(
        line.contains("llg_arm_any_events(self, (llg_event_spec_t[]){ { .sig = &G_value, .kind = LLG_EV_POSEDGE }, { .sig = &G_other, .kind = LLG_EV_NEGEDGE } }, 2)"),
        "{line}"
    );
    assert_no_array_copy(&source);
    assert!(!source.contains("_llg_event_specs_"), "{source}");
}

#[test]
fn mixed_wait_passes_a_compound_literal() {
    let source = render(vec![IrStmt::WaitEvents {
        specs: vec![
            signal("G_value", IrEdge::Posedge),
            (IrWaitSrc::Event(IrEventRef::Static(0)), IrEdge::Any),
        ],
        refresh: false,
    }]);
    let line = arm_line(&source, "llg_arm_mixed(");
    assert!(line.contains("llg_arm_mixed(self, (llg_wait_src_t[]){ { .sig = &G_value,"));
    assert!(line.contains("{ .sig = NULL, .kind = LLG_EV_ANY, .ev = &E_first } }, 2)"));
    assert_no_array_copy(&source);
    assert!(!source.contains("_llg_wait_sources_"), "{source}");
}

#[test]
fn dependency_wait_passes_a_compound_literal() {
    let source = render(vec![IrStmt::WaitAny {
        sens: vec![
            IrDependency::Scalar("G_value".to_owned()),
            IrDependency::Scalar("G_other".to_owned()),
        ],
        refresh: false,
    }]);
    // An event control's dependency wait (SV 9.7 resensitization on resume).
    let line = arm_line(&source, "llg_arm_event_dependencies(");
    assert!(
        line.contains("llg_arm_event_dependencies(self, (llg_wait_dependency_t[]){ { .sig = &G_value }, { .sig = &G_other } }, 2)"),
        "{line}"
    );
    assert_no_array_copy(&source);
    assert!(!source.contains("_llg_dependencies_"), "{source}");
}

#[test]
fn wait_order_copies_its_list_but_keeps_the_result_in_the_frame() {
    let source = render(vec![IrStmt::WaitOrder {
        events: events(),
        success: vec![IrStmt::Nop],
        failure: vec![IrStmt::Nop],
    }]);
    let line = arm_line(&source, "llg_arm_order(");
    assert!(
        line.contains("llg_arm_order(self, (const llg_event_t*[]){ &E_first, &E_second }, 2, &F->"),
        "{line}"
    );
    assert_no_array_copy(&source);
    assert!(!source.contains("_llg_ordered_events_"), "{source}");
}

#[test]
fn clocking_cycle_wait_repeats_its_literal_at_both_arm_sites() {
    let source = render(vec![IrStmt::ClockingCycleWait {
        count: number(2, 32),
        specs: vec![
            signal("G_value", IrEdge::Posedge),
            (IrWaitSrc::Event(IrEventRef::Static(0)), IrEdge::Any),
        ],
    }]);
    assert_eq!(
        source
            .lines()
            .filter(|line| line.contains(
                "llg_arm_clocking_cycle(LLG_CO_OWNER(ch, llg_proc_t), (llg_wait_src_t[]){"
            ))
            .count(),
        2,
        "{source}"
    );
    assert_no_array_copy(&source);
    assert!(!source.contains("_llg_clocking_sources_"), "{source}");
}

#[test]
fn nonblocking_registration_keeps_its_declared_array() {
    let source = render(vec![
        IrStmt::Delay {
            ticks: IrDelay::Constant(1),
        },
        IrStmt::NonblockingEventTriggerWhen {
            ev: IrEventRef::Static(0),
            specs: vec![signal("G_value", IrEdge::Any)],
            repeat: None,
        },
    ]);
    assert!(
        source.contains("memcpy(F->") && source.contains("_llg_events_"),
        "only arms may take a compound literal: {source}"
    );
    assert!(
        arm_line(&source, "llg_nba_event_when(").contains("llg_nba_event_when(F->"),
        "{source}"
    );
}
