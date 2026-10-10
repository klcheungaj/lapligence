//! Process handles in plain storage and random-stream methods (SIM-015).
use super::*;
use crate::sim::ir::{
    IrChandleExpr, IrObject, IrObjectStmt, IrObjectType, IrProcessExpr, IrProcessRandom,
    IrStringExpr,
};

fn process_model(body: Vec<IrStmt>) -> IrModel {
    let mut model = valid_model();
    for (c_name, ty) in [
        ("P_p", IrObjectType::Process),
        ("O_h", IrObjectType::Chandle),
    ] {
        model.objects.push(IrObject {
            c_name: c_name.into(),
            ty,
            initial: None,
        });
    }
    model.processes.push(IrProcess {
        c_name: "proc".to_string(),
        label: "top.initial".to_string(),
        kind: IrProcessKind::Synthetic,
        shape: IrShape::RunOnce,
        writes: Vec::new(),
        pre_fns: Vec::new(),
        body,
        program: None,
        origin: crate::sim::semantic::Origin::Synthetic {
            reason: "validation fixture".to_owned(),
        },
    });
    model.spawns.push("proc".to_string());
    model
}

fn random(op: IrProcessRandom) -> IrStmt {
    IrStmt::Object(Box::new(IrObjectStmt::ProcessRandom {
        target: IrProcessExpr::Read(0),
        op,
    }))
}

#[test]
fn process_random_methods_validate_their_operands() {
    process_model(vec![
        random(IrProcessRandom::Seed(packed_const(7, 32))),
        random(IrProcessRandom::SetState(IrStringExpr::Literal(
            b"s".to_vec(),
        ))),
    ])
    .validate()
    .expect("32-bit seed and string state are valid");

    let error = process_model(vec![random(IrProcessRandom::Seed(packed_const(7, 16)))])
        .validate()
        .expect_err("a seed narrower than 32 bits must fail");
    assert!(error.detail().contains("32-bit"), "{error:?}");
}

#[test]
fn pinned_process_reads_counted_process_storage_only() {
    let store = |source| {
        vec![IrStmt::Object(Box::new(IrObjectStmt::ChandleAssign(
            1,
            IrChandleExpr::PinnedProcess(Box::new(source)),
        )))]
    };
    process_model(store(IrProcessExpr::Read(0)))
        .validate()
        .expect("a process object can be pinned into a handle object");
    process_model(store(IrProcessExpr::Read(1)))
        .validate()
        .expect_err("a chandle object is not a process source");
}

#[test]
fn another_processes_random_state_is_a_runtime_string() {
    let state = IrStringExpr::ProcessRandState(Box::new(IrProcessExpr::Handle(Box::new(
        IrChandleExpr::Read(1),
    ))));
    process_model(vec![IrStmt::Object(Box::new(IrObjectStmt::StringPrint(
        state,
    )))])
    .validate()
    .expect("a handle object read is a process source");
    let bad = IrStringExpr::ProcessRandState(Box::new(IrProcessExpr::Read(1)));
    process_model(vec![IrStmt::Object(Box::new(IrObjectStmt::StringPrint(
        bad,
    )))])
    .validate()
    .expect_err("a chandle object is not a process object");
}

fn object_random(op: IrProcessRandom) -> IrStmt {
    IrStmt::Object(Box::new(IrObjectStmt::ObjectRandom {
        target: IrChandleExpr::Read(1),
        op,
    }))
}

#[test]
fn object_random_methods_require_object_stream_storage() {
    let statements = || {
        vec![
            object_random(IrProcessRandom::Seed(packed_const(7, 32))),
            IrStmt::Object(Box::new(IrObjectStmt::StringPrint(
                IrStringExpr::ObjectRandState(Box::new(IrChandleExpr::Read(1))),
            ))),
        ]
    };
    let error = process_model(statements())
        .validate()
        .expect_err("object streams need per-object storage in the class layout");
    assert!(
        error.detail().contains("object stream storage"),
        "{error:?}"
    );

    let mut model = process_model(statements());
    model.random.objects = true;
    model
        .validate()
        .expect("a 32-bit seed and a handle receiver are valid");

    let mut narrow = process_model(vec![object_random(IrProcessRandom::Seed(packed_const(
        7, 16,
    )))]);
    narrow.random.objects = true;
    let error = narrow
        .validate()
        .expect_err("a seed narrower than 32 bits must fail");
    assert!(error.detail().contains("32-bit"), "{error:?}");
}
