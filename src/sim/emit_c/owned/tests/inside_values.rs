//! Typed array elements use native signedness, including limb-crossing slices.
use super::*;

fn membership(width: u32, signed: bool) -> IrExpr {
    IrExpr::new(
        IrExprKind::Inside {
            value: Box::new(IrExpr::new(IrExprKind::SigRead(0), width + 4, true, None)),
            items: vec![IrInsideItem::FixedArray {
                value: IrExpr::new(IrExprKind::SigRead(1), width * 2, false, None),
                elements: vec![IrInsideArrayElement {
                    left: i64::from(width * 2 - 1),
                    right: i64::from(width),
                    width,
                    signed,
                }],
            }],
        },
        1,
        false,
        None,
    )
}

fn model(width: u32) -> IrModel {
    let mut model = IrModel::new("inside_signed".to_owned(), 1).unwrap();
    for (name, width, signed) in [("G_key", width + 4, true), ("G_array", width * 2, false)] {
        model.signals.push(IrSignal::new(
            name.to_owned(), None,
            IrType::Packed { width, signed, two_state: false }, None,
        ).unwrap());
    }
    model
}

#[test]
fn captured_inside_elements_retag_native_values_before_comparison() {
    for width in [4, 7, 65, 129] {
        for signed in [false, true] {
            let model = model(width);
            let ctx = RCtx { model: &model, func: None, sampled: false, activation_label: None };
            let mut frame = Frame::new(&ctx);
            let result = frame.expression(&membership(width, signed)).unwrap();
            let body = frame.body();
            let select = body.find("sv4_part_select(").unwrap();
            let sign = body[select..].find(&format!(".is_signed = {};", u8::from(signed))).unwrap();
            let compare = body[select..].find("sv4_wild_eq(").unwrap();
            assert!(sign < compare, "{body}");
            frame.discard(result);
            assert!(frame.slots.iter().all(|used| !used));
        }
    }
}

#[test]
fn generated_inside_array_value_preserves_signed_common_width() {
    assert!(crate::sim::build::cmake_available(), "CMake is required");
    for optimized in [false, true] {
        let mut model = model(4);
        model.processes.push(IrProcess::new(
            "p_inside".to_owned(), "inside".to_owned(), IrShape::RunOnce, Vec::new(),
            vec![
                IrStmt::Assign { lhs: IrLhs::Whole(0), rhs: number(255, 8), nba: false },
                IrStmt::Assign { lhs: IrLhs::Whole(1), rhs: number(240, 8), nba: false },
                IrStmt::Display {
                    fmt: "\"%0d %0d\"".to_owned(),
                    args: vec![(membership(4, true), false), (membership(4, false), false)],
                    newline: true, default_radix: IrDisplayRadix::Decimal,
                },
            ],
        ));
        model.spawns.push("p_inside".to_owned());
        let mut execution = ExecutionModel::lower(model).unwrap();
        let config = if optimized { crate::sim::opt::OptConfig::default() }
                     else { crate::sim::opt::OptConfig::none() };
        crate::sim::opt::run(&mut execution, &config).unwrap();
        let source = super::super::super::model::render(&execution).unwrap();
        let directory = toolchain::Directory::new("inside-signed");
        let binary = crate::sim::build::build_model_cmake(
            directory.path(), &[("model.c", &source)],
        ).unwrap();
        let output = toolchain::execute(&binary);
        assert!(output.status.success(), "{output:?}");
        assert_eq!(String::from_utf8_lossy(&output.stdout), "1 0\n");
        assert!(output.stderr.is_empty(), "{output:?}");
    }
}
