use super::super::repeated_values::REPEAT_VALUE_MIN_COUNT;
use super::*;

fn render(parts: Vec<IrExpr>) -> (String, usize) {
    let model = IrModel::new("repeated_values".to_owned(), 1).unwrap();
    render_model(&model, parts)
}

fn render_model(model: &IrModel, parts: Vec<IrExpr>) -> (String, usize) {
    let ctx = RCtx {
        value_backend: crate::sim::value_backend::ValueBackend::Legacy,
        model,
        func: None,
        sampled: false,
        activation_label: None,
        constants: None,
    };
    let mut frame = Frame::new(&ctx);
    let expr = IrExpr::new(
        IrExprKind::Concat {
            parts: parts.clone(),
        },
        parts.iter().map(|part| part.width).sum(),
        false,
        None,
    );
    model.validate_expr(&expr, None).unwrap();
    let value = frame.expression(&expr).unwrap();
    assert_eq!(value.width, expr.width);
    frame.discard(value);
    assert!(frame.slots.iter().all(|live| !live));
    (frame.body().to_owned(), frame.slots.len())
}

#[test]
fn repeated_mutation_evaluates_and_publishes_each_iteration() {
    let mut model = IrModel::new("repeated_mutation".to_owned(), 1).unwrap();
    model.signals.push(
        IrSignal::new(
            "G_counter".to_owned(),
            None,
            IrType::Packed {
                width: 8,
                signed: false,
                two_state: false,
            },
            None,
        )
        .unwrap(),
    );
    let operand = IrExpr::new(
        IrExprKind::Mutation(Box::new(IrMutationExpr {
            lhs: IrLhs::Whole(0),
            value: Box::new(add(
                IrExpr::new(
                    IrExprKind::LocalRead("_llg_mut_current".to_owned()),
                    8,
                    false,
                    None,
                ),
                number(1, 8),
                8,
            )),
            current_width: 8,
            current_signed: false,
            reads_current: true,
            post: true,
        })),
        8,
        false,
        None,
    );
    let render_count = |count| {
        let mut parts = vec![number(0xaa, 8)];
        parts.extend(vec![operand.clone(); count]);
        render_model(&model, parts)
    };
    let (small, small_slots) = render_count(REPEAT_VALUE_MIN_COUNT);
    let (large, large_slots) = render_count(REPEAT_VALUE_MIN_COUNT * 8);
    assert_eq!(small_slots, large_slots);
    assert_eq!(small.lines().count(), large.lines().count());
    let start = small.find("for (").unwrap();
    let read = small.find("sv4_clone(").unwrap();
    let write = small.find("llg_ba(").unwrap();
    let append = small.find("sv4_concat(").unwrap();
    assert!(start < read && read < write && write < append);
    let (expanded, _) = render_count(REPEAT_VALUE_MIN_COUNT - 1);
    assert!(!expanded.contains("for ("));
    assert_eq!(
        expanded.matches("sv4_concat(").count(),
        REPEAT_VALUE_MIN_COUNT - 1
    );
}

#[test]
fn repeated_calls_keep_each_occurrence_expanded() {
    let model = numeric_model();
    let call = IrExpr::new(
        IrExprKind::CallFn(Box::new(IrCallExpr::new(
            0,
            vec![IrCallArg::Val(number(1, 65))],
            IrDepth::PROC,
            false,
        ))),
        65,
        false,
        None,
    );
    let count = REPEAT_VALUE_MIN_COUNT + 1;
    let (body, _) = render_model(&model, vec![call; count]);
    assert!(!body.contains("value_repeat"));
    assert_eq!(body.matches("f_increment(").count(), count);
}

#[test]
fn repeated_concat_body_and_slots_are_independent_of_count() {
    let render_count = |count| render(vec![number(0xa5, 65); count + 1]);
    let (small, small_slots) = render_count(REPEAT_VALUE_MIN_COUNT);
    let (large, large_slots) = render_count(REPEAT_VALUE_MIN_COUNT * 8);
    assert_eq!(small.matches("for (").count(), 1);
    assert_eq!(large.matches("for (").count(), 1);
    assert_eq!(small.matches("sv4_concat(").count(), 1);
    assert_eq!(large.matches("sv4_concat(").count(), 1);
    assert_eq!(small_slots, large_slots);
    assert_eq!(small.lines().count(), large.lines().count());
    assert_eq!(small_slots, 2);
    let start = small.find("for (").unwrap();
    let read = small[start..].find("sv4_replace(&_llg_t[1]").unwrap() + start;
    let append = small.find("sv4_concat(").unwrap();
    let destroy = small.find("sv4_destroy(&_llg_t[1]").unwrap();
    let end = small.find("\n    }").unwrap();
    assert!(start < read && read < append && append < destroy && destroy < end);
}

#[test]
fn repeated_concat_below_threshold_keeps_expanded_operations() {
    let count = REPEAT_VALUE_MIN_COUNT - 1;
    let (body, slots) = render(vec![number(0xa5, 8); count + 1]);
    assert!(!body.contains("for ("));
    let mut expected =
        String::from("    sv4_replace(&_llg_t[0], SV4_INIT(165ULL, 0ULL, 0ULL, 8, 0));\n");
    for _ in 0..count {
        expected.push_str("    sv4_replace(&_llg_t[1], SV4_INIT(165ULL, 0ULL, 0ULL, 8, 0));\n    sv4_replace(&_llg_t[0], sv4_concat(_llg_t[0], _llg_t[1]));\n    sv4_destroy(&_llg_t[1]);\n");
    }
    expected.push_str("    llg_sv4_set_signed(&_llg_t[0], 0);\n    sv4_destroy(&_llg_t[0]);\n");
    assert_eq!(body, expected);
    assert_eq!(slots, 2);
}

#[test]
fn repeated_concat_preserves_neighbor_order_and_typed_boundaries() {
    let count = REPEAT_VALUE_MIN_COUNT;
    let mut parts = vec![number(0xaa, 8)];
    parts.extend(vec![number(0x12, 8); count]);
    parts.push(number(0xbb, 8));
    parts.extend(vec![number(0x12, 9); count]);
    parts.push(number(0xcc, 8));
    let (body, _) = render(parts);
    assert_eq!(body.matches("for (").count(), 2);
    let positions = [
        "170ULL",
        "18ULL, 0ULL, 0ULL, 8",
        "187ULL",
        "18ULL, 0ULL, 0ULL, 9",
        "204ULL",
    ]
    .map(|text| body.find(text).unwrap());
    assert!(positions.windows(2).all(|pair| pair[0] < pair[1]));
}

#[test]
fn repeated_array_conditional_keeps_branch_evaluation_inside_loop() {
    let operand = IrExpr::new(
        IrExprKind::ArrayMux {
            sel: Box::new(number(0, 1)),
            a: Box::new(number(0xa55a, 16)),
            b: Box::new(number(0xa65a, 16)),
            element_default: Box::new(
                IrConst::packed(vec![], vec![0xff], vec![], 8, false, None).unwrap(),
            ),
        },
        16,
        false,
        None,
    );
    let render_count = |count| {
        let mut parts = vec![number(0, 1)];
        parts.extend(vec![operand.clone(); count]);
        render(parts)
    };
    let (small, small_slots) = render_count(REPEAT_VALUE_MIN_COUNT);
    let (large, large_slots) = render_count(REPEAT_VALUE_MIN_COUNT * 8);
    assert_eq!(small_slots, large_slots);
    assert_eq!(small.lines().count(), large.lines().count());
    assert_eq!(small.matches("sv4_array_conditional_merge(").count(), 1);
    let start = small.find("for (").unwrap();
    assert!(start < small.find("if (").unwrap());
    assert!(start < small.find("sv4_array_conditional_merge(").unwrap());
    let (expanded, _) = render_count(REPEAT_VALUE_MIN_COUNT - 1);
    assert!(!expanded.contains("for ("));
    assert_eq!(
        expanded.matches("sv4_array_conditional_merge(").count(),
        REPEAT_VALUE_MIN_COUNT - 1
    );
}
