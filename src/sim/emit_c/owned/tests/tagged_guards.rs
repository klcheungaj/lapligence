//! Tagged read and target guards must account for every value-returning helper.
use super::*;

fn model() -> IrModel {
    let mut model = IrModel::new("tagged_guard_owners".to_owned(), 1).unwrap();
    model.signals.push(
        IrSignal::new(
            "G_tagged".to_owned(),
            None,
            IrType::Packed {
                width: 10,
                signed: false,
                two_state: false,
            },
            None,
        )
        .unwrap(),
    );
    model
}

fn steps() -> Vec<IrTaggedSelectStep> {
    [9, 8]
        .into_iter()
        .map(|width| IrTaggedSelectStep {
            selection: IrPackedSelect {
                base: number(0, 32),
                width,
            },
            two_state: false,
            guard: Some(IrTaggedMemberGuard {
                member_index: 1,
                tag_width: 1,
                member_name: format!("member_{width}"),
            }),
        })
        .collect()
}

fn read() -> IrExpr {
    IrExpr::new(
        IrExprKind::TaggedSelect {
            base: Box::new(IrExpr::new(IrExprKind::SigRead(0), 10, false, None)),
            steps: steps(),
            location: "tagged.sv:1:1".to_owned(),
        },
        8,
        false,
        None,
    )
}

fn target() -> IrLhs {
    IrLhs::TaggedSelect {
        target: Box::new(IrLhs::Whole(0)),
        steps: steps(),
        signed: false,
        two_state: false,
        location: "tagged.sv:1:1".to_owned(),
    }
}

fn check_owners(frame: &Frame<'_, '_>) {
    assert!(frame.slots.iter().all(|used| !used));
    let source = frame.body();
    assert!(!source.contains("sv4_to_bool(sv4_case_eq("), "{source}");
    assert_eq!(source.matches("sv4_case_eq_to(").count(), 2);
    assert_eq!(source.matches(", 1ULL, 1, 0);").count(), 2);
    for line in source
        .lines()
        .filter(|line| line.contains("sv4_case_eq_to(") || line.contains(", 1ULL, 1, 0);"))
    {
        assert!(
            line.trim_start().starts_with("sv4_case_eq_to(&_llg_t[")
                || line.trim_start().starts_with("sv4_from_u64_to(&_llg_t["),
            "{line}"
        );
    }
}

#[test]
fn tagged_read_guards_track_comparison_owners() {
    let model = model();
    let ctx = RCtx {
        value_backend: crate::sim::value_backend::ValueBackend::Legacy,
        model: &model,
        func: None,
        sampled: false,
        activation_label: None,
        constants: None,
    };
    let mut frame = Frame::new(&ctx);
    let value = frame.expression(&read()).unwrap();
    frame.discard(value);
    check_owners(&frame);
}

#[test]
fn tagged_write_guards_track_comparison_owners() {
    let model = model();
    let ctx = RCtx {
        value_backend: crate::sim::value_backend::ValueBackend::Legacy,
        model: &model,
        func: None,
        sampled: false,
        activation_label: None,
        constants: None,
    };
    let mut frame = Frame::new(&ctx);
    let target = frame.target(&target()).unwrap();
    let value = frame.expression(&number(0x5a, 8)).unwrap();
    frame.store(&target, value, false, "0").unwrap();
    frame.release_target(target);
    check_owners(&frame);
}

#[test]
fn generated_nested_tagged_guards_execute_with_owned_temporaries() {
    assert!(crate::sim::build::cmake_available(), "CMake is required");
    for optimized in [false, true] {
        let mut model = model();
        model.processes.push(IrProcess::new(
            "p_tagged".to_owned(),
            "tagged".to_owned(),
            IrShape::RunOnce,
            Vec::new(),
            vec![
                IrStmt::Assign {
                    lhs: IrLhs::Whole(0),
                    rhs: number(0x35a, 10), // Both tag bits are one; payload is 0x5a.
                    nba: false,
                },
                IrStmt::Repeat {
                    count: number(1000, 32),
                    body: vec![IrStmt::Assign {
                        lhs: target(),
                        rhs: read(),
                        nba: false,
                    }],
                },
                IrStmt::Display {
                    fmt: "\"%0d\"".to_owned(),
                    args: vec![(read(), false)],
                    newline: true,
                    default_radix: IrDisplayRadix::Decimal,
                },
            ],
        ));
        model.spawns.push("p_tagged".to_owned());
        let mut execution = ExecutionModel::lower(model).unwrap();
        let config = if optimized {
            crate::sim::opt::OptConfig::default()
        } else {
            crate::sim::opt::OptConfig::none()
        };
        crate::sim::opt::run(&mut execution, &config).unwrap();
        let source = super::super::super::model::render(&execution).unwrap();
        let directory = toolchain::Directory::new("tagged-guards");
        let binary =
            crate::sim::build::build_model_cmake(directory.path(), &[("model.c", &source)])
                .unwrap();
        // LLG_CFLAGS and ASAN_OPTIONS enable leak checks of this emitted model,
        // rather than of a handwritten reconstruction of the guard expression.
        let result = toolchain::execute(&binary);
        assert!(result.status.success(), "{result:?}");
        assert_eq!(String::from_utf8_lossy(&result.stdout), "90\n");
        toolchain::assert_quiet_end(&result, 0);
    }
}
