//! Owned-emitter regressions for the first Group 1 repair batch.
use super::*;

fn call(function: usize, args: Vec<IrCallArg>, width: u32) -> IrExpr {
    IrExpr::new(
        IrExprKind::CallFn(Box::new(IrCallExpr::new(
            function,
            args,
            IrDepth::PROC,
            false,
        ))),
        width,
        false,
        None,
    )
}

#[test]
fn callback_expansions_rename_all_internal_labels_and_gotos() {
    let mut model = IrModel::new("review_inline_labels".to_owned(), 1).unwrap();
    let ty = IrType::Packed {
        width: 32,
        signed: false,
        two_state: false,
    };
    model.funcs.push(IrFunc::new(
        "f_inner".to_owned(),
        Some(ty),
        vec![],
        vec![],
        vec![],
        vec![
            IrStmt::Block(vec![
                IrStmt::Goto("loop_exit".to_owned()),
                IrStmt::Label("loop_exit".to_owned()),
            ]),
            IrStmt::Return {
                value: Some(Box::new(number(7, 32))),
            },
        ],
    ));
    model.funcs.push(IrFunc::new(
        "f_outer".to_owned(),
        Some(ty),
        vec![],
        vec![],
        vec![],
        vec![IrStmt::Return {
            value: Some(Box::new(add(call(0, vec![], 32), call(0, vec![], 32), 32))),
        }],
    ));
    let ctx = RCtx {
        model: &model,
        func: None,
        sampled: false,
        activation_label: None,
    };
    let mut frame = Frame::new(&ctx);
    frame.read_only_callback = true;
    for _ in 0..2 {
        let value = frame.expression(&call(1, vec![], 32)).unwrap();
        frame.discard(value);
    }
    let labels: Vec<_> = frame
        .body()
        .lines()
        .filter_map(|line| line.trim().strip_suffix(": ;"))
        .collect();
    let unique: std::collections::HashSet<_> = labels.iter().copied().collect();
    assert_eq!(
        labels.len(),
        unique.len(),
        "duplicate C function-scoped label"
    );
    assert_eq!(
        labels
            .iter()
            .filter(|label| label.ends_with("_loop_exit"))
            .count(),
        4
    );
    for jump in frame.body().lines().filter_map(|line| {
        line.trim()
            .strip_prefix("goto ")
            .and_then(|line| line.strip_suffix(';'))
    }) {
        assert!(unique.contains(jump), "missing target for {jump}");
    }
    assert!(!frame.body().contains("goto loop_exit;"));
    assert!(frame.slots.iter().all(|used| !used));
}

#[test]
fn real_callback_results_are_declared_in_the_callers_scope() {
    for shortreal in [false, true] {
        let mut model = IrModel::new("review_inline_real".to_owned(), 1).unwrap();
        let mut formal = IrFormal::new(false, 1, false).unwrap();
        formal.real = true;
        formal.shortreal = shortreal;
        model.funcs.push(IrFunc::new(
            "f_real".to_owned(),
            Some(IrType::Real { shortreal }),
            vec![formal],
            vec![],
            vec![],
            vec![IrStmt::Return {
                value: Some(Box::new(IrExpr::new(
                    IrExprKind::FormalRead(0),
                    0,
                    false,
                    None,
                ))),
            }],
        ));
        let ctx = RCtx {
            model: &model,
            func: None,
            sampled: false,
            activation_label: None,
        };
        let mut frame = Frame::new(&ctx);
        frame.read_only_callback = true;
        let arg = IrExpr::new(IrExprKind::Const(IrConst::real(1.25)), 0, false, None);
        let result = frame
            .expression(&call(0, vec![IrCallArg::Val(arg)], 0))
            .unwrap();
        let declaration = format!("double {} = 0.0;", result.code);
        let source = frame.body();
        assert!(
            source.find(&declaration).unwrap() < source.find("{\n").unwrap(),
            "real escape owner was declared inside the inline block: {source}"
        );
        assert!(source.contains(&format!("{} = ", result.code)));
        frame.discard(result);
        assert!(frame.slots.iter().all(|used| !used));
    }
}

fn stream_model() -> IrModel {
    let mut model = IrModel::new("review_stream".to_owned(), 1).unwrap();
    for name in ["G_a", "G_b"] {
        model
            .arrays
            .push(IrArray::new(name.to_owned(), name.to_owned(), 8, false, vec![(0, 3)]).unwrap());
    }
    model
}

#[test]
fn fixed_stream_index_owners_use_registered_slots_and_are_destroyed() {
    let model = stream_model();
    let ctx = RCtx {
        model: &model,
        func: None,
        sampled: false,
        activation_label: None,
    };
    let mut frame = Frame::new(&ctx);
    let targets: Vec<_> = (0..2)
        .map(|array| IrStreamTarget::FixedSelector {
            array,
            selector: IrStreamSelector::Range {
                left: number(0, 32),
                right: number(1, 32),
            },
        })
        .collect();
    frame
        .stream_assignment(
            &number(0x1234_5678, 32),
            1,
            IrStreamDirection::LeftToRight,
            &targets,
        )
        .unwrap();
    let source = frame.body();
    let mut indexes = 0;
    for (position, line) in source.lines().enumerate() {
        if !line.contains("sv4_from_i64(llg_fixed_stream_index_at(") {
            continue;
        }
        indexes += 1;
        let owner = line
            .trim()
            .strip_prefix("sv4_replace(&")
            .unwrap()
            .split(',')
            .next()
            .unwrap();
        assert!(
            owner.starts_with("_llg_t["),
            "unregistered packed owner: {line}"
        );
        let release = format!("sv4_destroy(&{owner});");
        assert!(source
            .lines()
            .skip(position + 1)
            .any(|line| line.trim() == release));
    }
    assert_eq!(indexes, 2);
    assert!(!source.contains("sv4_t _llg_scalar_"));
    assert!(frame.slots.iter().all(|used| !used));
    assert!(frame.bindings.iter().all(|scope| scope.is_empty()));
}

#[test]
fn stream_checks_each_component_before_publication_using_the_actual_source_width() {
    let model = stream_model();
    let ctx = RCtx {
        model: &model,
        func: None,
        sampled: false,
        activation_label: None,
    };
    let mut frame = Frame::new(&ctx);
    let targets = [
        IrStreamTarget::FixedSelector {
            array: 0,
            selector: IrStreamSelector::Range {
                left: number(0, 32),
                right: number(1, 32),
            },
        },
        IrStreamTarget::FixedSelector {
            array: 1,
            selector: IrStreamSelector::Range {
                left: number(0, 32),
                right: number(1, 32),
            },
        },
    ];
    frame
        .stream_assignment(
            &number(0x123456, 24),
            1,
            IrStreamDirection::LeftToRight,
            &targets,
        )
        .unwrap();
    let source = frame.body();
    assert!(source.contains(".width;"));
    assert_eq!(source.matches("llg_stream_require_bits(").count(), 2);
    let first_write_loop = source.find("for (size_t _llg_fs_offset_").unwrap();
    let first_check = source.find("llg_stream_require_bits(").unwrap();
    let last_check = source.rfind("llg_stream_require_bits(").unwrap();
    let last_write_loop = source.rfind("for (size_t _llg_fs_offset_").unwrap();
    assert!(first_check < first_write_loop);
    assert!(first_write_loop < last_check && last_check < last_write_loop);
    let first_report = source.find("llg_rt_mark_failed();").unwrap();
    assert!(first_check < first_report);
    assert!(first_report < first_write_loop);
    assert!(frame.slots.iter().all(|used| !used));
}
