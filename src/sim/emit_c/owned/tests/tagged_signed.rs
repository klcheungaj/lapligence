//! Typed member reads must retag the native payload, not only emitter metadata.
use super::*;

fn member(signed: bool, two_state: bool) -> IrExpr {
    IrExpr::new(
        IrExprKind::TaggedSelect {
            base: Box::new(IrExpr::new(IrExprKind::SigRead(0), 5, false, None)),
            steps: vec![IrTaggedSelectStep {
                selection: IrPackedSelect {
                    base: number(0, 32),
                    width: 4,
                },
                two_state,
                guard: Some(IrTaggedMemberGuard {
                    member_index: 1,
                    tag_width: 1,
                    member_name: "Value".to_owned(),
                }),
            }],
            location: "tagged_signed.sv:1:1".to_owned(),
        },
        4,
        signed,
        None,
    )
}

fn model() -> IrModel {
    let mut model = IrModel::new("tagged_signed".to_owned(), 1).unwrap();
    model.signals.push(
        IrSignal::new(
            "G_tagged".to_owned(),
            None,
            IrType::Packed {
                width: 5,
                signed: false,
                two_state: false,
            },
            None,
        )
        .unwrap(),
    );
    model
}

#[test]
fn tagged_member_retags_after_the_invalid_access_branch() {
    for signed in [false, true] {
        for two_state in [false, true] {
            let model = model();
            let ctx = RCtx {
                model: &model,
                func: None,
                sampled: false,
                activation_label: None,
            };
            let mut frame = Frame::new(&ctx);
            let result = frame.expression(&member(signed, two_state)).unwrap();
            assert_eq!(result.signed, signed);
            assert!(frame.body().ends_with(&format!(
                "    }}\n    {}.is_signed = {};\n",
                result.code,
                u8::from(signed)
            )));
            assert_eq!(frame.body().contains("sv4_to_two_state("), two_state);
            frame.discard(result);
            assert!(frame.slots.iter().all(|used| !used));
        }
    }
}

#[test]
fn generated_tagged_member_widening_preserves_the_source_sign() {
    assert!(crate::sim::build::cmake_available(), "CMake is required");
    for optimized in [false, true] {
        let mut model = model();
        model.processes.push(IrProcess::new(
            "p_signed".to_owned(),
            "signed".to_owned(),
            IrShape::RunOnce,
            Vec::new(),
            vec![
                IrStmt::Assign {
                    lhs: IrLhs::Whole(0),
                    rhs: number(0x1e, 5),
                    nba: false,
                },
                IrStmt::Display {
                    fmt: "\"%0d %0d %0d\"".to_owned(),
                    args: vec![
                        (IrExpr::convert_to(member(true, false), 8, false), false),
                        (IrExpr::convert_to(member(false, false), 8, false), false),
                        (IrExpr::convert_to(member(true, true), 8, true), false),
                    ],
                    newline: true,
                    default_radix: IrDisplayRadix::Decimal,
                },
            ],
        ));
        model.spawns.push("p_signed".to_owned());
        let mut execution = ExecutionModel::lower(model).unwrap();
        let config = if optimized {
            crate::sim::opt::OptConfig::default()
        } else {
            crate::sim::opt::OptConfig::none()
        };
        crate::sim::opt::run(&mut execution, &config).unwrap();
        let source = super::super::super::model::render(&execution).unwrap();
        let directory = toolchain::Directory::new("tagged-signed");
        let binary =
            crate::sim::build::build_model_cmake(directory.path(), &[("model.c", &source)])
                .unwrap();
        let output = toolchain::execute(&binary);
        assert!(output.status.success(), "{output:?}");
        assert_eq!(String::from_utf8_lossy(&output.stdout), "254 14 -2\n");
        assert!(output.stderr.is_empty(), "{output:?}");
    }
}
