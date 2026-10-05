//! Resizable containers in subroutine storage (SIM-006): lexical
//! declarations, container formals and call operands.
use super::*;

fn queue(element: IrContainerElement, activation: bool, name: &str) -> IrContainer {
    IrContainer {
        c_name: name.into(),
        element,
        kind: IrContainerKind::Queue {
            maximum_elements: None,
        },
        initial_size: None,
        activation,
        class_field: None,
    }
}

fn int_element() -> IrContainerElement {
    IrContainerElement::Packed {
        width: 32,
        signed: true,
        two_state: true,
    }
}

fn container_formal(container: usize, is_out: bool) -> IrFormal {
    let mut formal = IrFormal::new(is_out, 1, false).unwrap();
    formal.width = 0;
    formal.container = Some(container);
    formal
}

fn container_model() -> IrModel {
    let mut model = valid_model();
    // 0: module queue, 1: the callee's formal storage, 2: a caller temporary.
    model.containers.push(queue(int_element(), false, "C_q"));
    model
        .containers
        .push(queue(int_element(), true, "C_llg_sub_1"));
    model
        .containers
        .push(queue(int_element(), true, "C_llg_sub_2"));
    model.funcs.push(IrFunc::new(
        "f".into(),
        None,
        vec![container_formal(1, false)],
        vec![],
        vec![],
        vec![],
    ));
    model
}

fn call(args: Vec<IrCallArg>) -> IrStmt {
    IrStmt::Call(Box::new(IrCall::new(
        0,
        args,
        IrDepth::PROC,
        Vec::new(),
        Vec::new(),
    )))
}

fn process(body: Vec<IrStmt>) -> IrProcess {
    IrProcess::new("p0".into(), "top.p".into(), IrShape::RunOnce, vec![], body)
}

fn declare(container: usize) -> IrStmt {
    IrStmt::Container(Box::new(IrContainerStmt::Declare(container)))
}

#[test]
fn only_activation_containers_have_lexical_declarations() {
    let mut model = container_model();
    model.processes.push(process(vec![declare(2)]));
    model.spawns.push("p0".into());
    model.validate().unwrap();
    model.processes[0].body = vec![declare(0)];
    assert!(model.validate().is_err(), "model storage is not lexical");
    model.processes[0].body = vec![declare(9)];
    assert!(model.validate().is_err(), "declaration out of bounds");
}

#[test]
fn container_formals_take_matching_container_operands() {
    let mut model = container_model();
    model
        .processes
        .push(process(vec![call(vec![IrCallArg::Container(0)])]));
    model.spawns.push("p0".into());
    model.validate().unwrap();

    let mut real = container_model();
    real.containers[0].element = IrContainerElement::Real { shortreal: false };
    real.processes
        .push(process(vec![call(vec![IrCallArg::Container(0)])]));
    real.spawns.push("p0".into());
    assert!(real.validate().is_err(), "element types must agree");

    let cases = [
        (vec![IrCallArg::Val(packed_const(1, 32))], false),
        (vec![IrCallArg::Container(7)], false),
        (
            vec![IrCallArg::ContainerValues {
                container: 2,
                values: vec![packed_const(4, 32), packed_const(5, 32)],
            }],
            true,
        ),
        // A pattern operand is built in fresh activation storage.
        (
            vec![IrCallArg::ContainerValues {
                container: 0,
                values: vec![packed_const(4, 32)],
            }],
            false,
        ),
    ];
    for (index, (args, valid)) in cases.into_iter().enumerate() {
        model.processes[0].body = vec![call(args)];
        assert_eq!(model.validate().is_ok(), valid, "case {index}");
    }
}

#[test]
fn container_formal_storage_is_exclusive() {
    let mut model = container_model();
    model.validate().unwrap();
    model.funcs[0].formals[0].width = 8;
    assert!(
        model.validate().is_err(),
        "container formal has no payload width"
    );
    model.funcs[0].formals[0].width = 0;
    model.funcs[0].formals[0].string = true;
    assert!(
        model.validate().is_err(),
        "container formal is not a string"
    );
    model.funcs[0].formals[0].string = false;
    model.funcs[0].formals[0].container = Some(11);
    assert!(model.validate().is_err(), "formal storage out of bounds");
}
