use super::*;

fn delay() -> IrStmt {
    IrStmt::Delay {
        ticks: IrDelay::Constant(1),
    }
}

fn local(name: &str) -> IrStmt {
    IrStmt::DeclLocal {
        name: name.to_owned(),
        width: 65,
        signed: false,
        two_state: false,
        init: Some(Box::new(number(42, 65))),
    }
}

fn render(body: Vec<IrStmt>) -> String {
    let mut model = numeric_model();
    model.processes[0].body = body;
    super::super::super::model::render(&ExecutionModel::lower(model).unwrap()).unwrap()
}

#[test]
fn debug_overlay_poison_follows_owner_cleanup_and_preserves_flattened_parents() {
    let source = render(vec![
        local("parent"),
        IrStmt::Block(vec![local("left"), delay()]),
        IrStmt::Block(vec![local("right"), delay()]),
    ]);
    let lines = source.lines().collect::<Vec<_>>();
    let mut poisons = 0;
    for (index, line) in lines.iter().enumerate() {
        if line.contains("LLG_CO_DEBUG_POISON(") {
            poisons += 1;
            assert!(line.contains(".b"), "{line}");
            assert_eq!(lines[index - 1], "#ifdef LLG_CO_DEBUG");
            assert!(lines[index - 2].contains("llg_value_scopes_end_since("));
            assert_eq!(lines[index + 1], "#endif");
        }
    }
    assert_eq!(poisons, 2, "{source}");
    assert!(!source.contains("__llg_poison_block_"));
    let flattened = render(vec![
        local("parent"),
        IrStmt::Block(vec![local("child"), delay()]),
    ]);
    assert!(!flattened.contains("LLG_CO_DEBUG_POISON("), "{flattened}");
}

#[test]
fn debug_nonlocal_overlay_exit_poisons_after_scope_unwind_before_jump() {
    let source = render(vec![
        IrStmt::Block(vec![local("left"), delay(), IrStmt::Goto("done".into())]),
        IrStmt::Block(vec![local("right"), delay()]),
        IrStmt::Label("done".into()),
    ]);
    let jump = source.find("    goto done;").unwrap();
    let prefix = &source[..jump];
    let cleanup = prefix.rfind("llg_value_scopes_end_since(").unwrap();
    let poison = prefix.rfind("LLG_CO_DEBUG_POISON(").unwrap();
    assert!(cleanup < poison, "{source}");
    assert!(prefix.ends_with("#endif\n"), "{prefix}");
}

#[test]
fn debug_embedded_callee_poison_precedes_argument_transfer() {
    let mut model = numeric_model();
    model.funcs[0].ret = None;
    model.funcs[0].is_task = true;
    model.funcs[0].body = vec![delay()];
    model.processes[0].body = vec![IrStmt::Call(Box::new(IrCall::new(
        0,
        vec![IrCallArg::Val(number(23, 65))],
        IrDepth::PROC,
        Vec::new(),
        Vec::new(),
    )))];
    let source =
        super::super::super::model::render(&ExecutionModel::lower(model).unwrap()).unwrap();
    let poison = source.find("LLG_CO_DEBUG_POISON_FRAME(&F->").unwrap();
    let after = &source[poison..];
    assert!(
        after.find(".a0 =").unwrap() < after.find("LLG_CO_CALL(").unwrap(),
        "{source}"
    );
    assert!(source[..poison].trim_end().ends_with("#ifdef LLG_CO_DEBUG"));
    assert!(after.find("#endif").unwrap() < after.find(".a0 =").unwrap());
}

#[test]
fn debug_loop_break_and_common_return_poison_without_rereading_dead_fields() {
    let source = render(vec![
        IrStmt::While {
            cond: number(0, 1),
            body: vec![delay()],
        },
        IrStmt::While {
            cond: number(0, 1),
            body: vec![delay()],
        },
    ]);
    assert_eq!(
        source.matches("LLG_CO_DEBUG_POISON_LOOP_EXIT(").count(),
        2,
        "{source}"
    );
    let returned = source.find("return LLG_CO_DONE;").unwrap();
    let prefix = &source[..returned];
    let cleanup = prefix
        .rfind("llg_value_scopes_end_since(_llg_frame_base)")
        .unwrap();
    let poison = prefix
        .rfind("LLG_CO_DEBUG_POISON_FRAME(F, sizeof(*F));")
        .unwrap();
    assert!(cleanup < poison, "{source}");
}
