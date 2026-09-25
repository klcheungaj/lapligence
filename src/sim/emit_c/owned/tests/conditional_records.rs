//! Immediate member defaults, including cross-limb payloads, own their storage.
use super::*;

fn constant(value: u64, width: u32) -> IrConst {
    IrConst::packed(vec![value], vec![], vec![], width, false, None).unwrap()
}

fn unknown(width: u32) -> IrConst {
    let mut mask = vec![u64::MAX; width.div_ceil(64) as usize];
    if !width.is_multiple_of(64) {
        *mask.last_mut().unwrap() = (1u64 << (width % 64)) - 1;
    }
    IrConst::packed(vec![], mask, vec![], width, false, None).unwrap()
}

fn model(width: u32) -> IrModel {
    let mut model = IrModel::new("record_defaults".to_owned(), 1).unwrap();
    for (name, bits) in [("G_selector", 1), ("G_left", width + 9),
                         ("G_right", width + 9), ("G_result", width + 9)] {
        model.signals.push(IrSignal::new(name.to_owned(), None,
            IrType::Packed { width: bits, signed: false, two_state: false }, None).unwrap());
    }
    model
}

fn merge(width: u32) -> IrExpr {
    let read = |id, bits| IrExpr::new(IrExprKind::SigRead(id), bits, false, None);
    IrExpr::new(IrExprKind::StructMux {
        sel: Box::new(read(0, 1)), a: Box::new(read(1, width + 9)), b: Box::new(read(2, width + 9)),
        members: vec![
            IrConditionalMember { offset: 0, width: 8, default: unknown(8) },
            IrConditionalMember { offset: 8, width: 1, default: constant(0, 1) },
            IrConditionalMember { offset: 9, width, default: unknown(width) },
        ],
    }, width + 9, false, None)
}

#[test]
fn cross_limb_structure_merge_owns_each_member_and_default() {
    for width in [1, 7, 33, 65, 129] {
        let model = model(width);
        let ctx = RCtx { model: &model, func: None, sampled: false, activation_label: None };
        let mut frame = Frame::new(&ctx);
        let expr = merge(width);
        model.validate_expr(&expr, None).unwrap();
        let value = frame.expression(&expr).unwrap();
        frame.discard(value);
        assert_eq!(frame.body().matches("sv4_array_conditional_merge(").count(), 3);
        assert!(!frame.body().contains("sv4_mux("));
        assert!(frame.slots.iter().all(|live| !live));
    }
}

#[test]
fn generated_record_member_defaults_preserve_equal_members_and_limb_boundaries() {
    assert!(crate::sim::build::cmake_available(), "CMake is required");
    for optimized in [false, true] {
        let mut model = model(129);
        let assign = |id, rhs| IrStmt::Assign { lhs: IrLhs::Whole(id), rhs, nba: false };
        let arm = |data, flag| IrExpr::new(IrExprKind::Concat {
            parts: vec![number(data, 129), number(flag, 1), number(0x5a, 8)],
        }, 138, false, None);
        let mut body = vec![assign(1, arm(1, 1)), assign(2, arm(0, 0))];
        for state in 0..4 {
            let selector = IrConst::packed(vec![u64::from(state == 1)],
                vec![u64::from(state == 2)], vec![u64::from(state == 3)], 1, false, None).unwrap();
            body.push(assign(0, IrExpr::new(IrExprKind::Const(selector), 1, false, None)));
            body.push(assign(3, merge(129)));
            body.push(IrStmt::Display {
                fmt: "\"%b\"".to_owned(),
                args: vec![(IrExpr::new(IrExprKind::SigRead(3), 138, false, None), false)],
                newline: true, default_radix: IrDisplayRadix::Binary,
            });
        }
        model.processes.push(IrProcess::new("p_record".to_owned(), "record".to_owned(),
            IrShape::RunOnce, Vec::new(), body));
        model.spawns.push("p_record".to_owned());
        let mut execution = ExecutionModel::lower(model).unwrap();
        let config = if optimized { crate::sim::opt::OptConfig::default() }
            else { crate::sim::opt::OptConfig::none() };
        crate::sim::opt::run(&mut execution, &config).unwrap();
        let source = super::super::super::model::render(&execution).unwrap();
        let directory = toolchain::Directory::new("record-defaults");
        let binary = crate::sim::build::build_model_cmake(directory.path(), &[("model.c", &source)]).unwrap();
        let output = toolchain::execute(&binary);
        assert!(output.status.success(), "{output:?}");
        let expected = format!("{}001011010\n{}1101011010\n{}001011010\n{}001011010\n",
            "0".repeat(129), "0".repeat(128), "x".repeat(129), "x".repeat(129));
        assert_eq!(String::from_utf8_lossy(&output.stdout), expected);
        assert!(output.stderr.is_empty(), "{output:?}");
    }
}
