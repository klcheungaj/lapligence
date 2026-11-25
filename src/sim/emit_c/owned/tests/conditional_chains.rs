//! A conditional operator renders code in proportion to its source: every
//! predicate and alternative once, however deep the operators nest.
use super::control::CONDITIONAL_REPEAT_LEVELS;
use super::*;

fn read() -> IrExpr {
    IrExpr::new(IrExprKind::SigRead(0), 65, false, None)
}

fn equals(value: u64) -> IrExpr {
    IrExpr::new(
        IrExprKind::Bin {
            op: IrBinOp::Eq,
            a: Box::new(read()),
            b: Box::new(number(value, 65)),
        },
        1,
        false,
        None,
    )
}

fn mux(sel: IrExpr, a: IrExpr, b: IrExpr) -> IrExpr {
    let (width, signed) = (a.width, a.signed);
    IrExpr::new(
        IrExprKind::Mux {
            sel: Box::new(sel),
            a: Box::new(a),
            b: Box::new(b),
        },
        width,
        signed,
        None,
    )
}

/// The constant only alternative `index` holds.
fn marker(index: usize) -> u64 {
    70_000 + index as u64
}

fn alternative(index: usize) -> IrExpr {
    add(read(), number(marker(index), 65), 65)
}

/// `G == 0 ? a0 : G == 1 ? a1 : ... : a<levels>`.
fn priority_chain(levels: usize) -> IrExpr {
    (0..levels).rev().fold(alternative(levels), |rest, level| {
        mux(equals(level as u64), alternative(level), rest)
    })
}

/// `G == 0 ? (G == 1 ? (... a<levels>) : a1) : a0`.
fn first_alternative_chain(levels: usize) -> IrExpr {
    (0..levels).rev().fold(alternative(levels), |rest, level| {
        mux(equals(level as u64), rest, alternative(level))
    })
}

fn rendered(expr: &IrExpr) -> String {
    let model = numeric_model();
    let ctx = RCtx {
        value_backend: crate::sim::value_backend::ValueBackend::Legacy,
        model: &model,
        effects: &crate::sim::execution::EffectSummaries::new(&model),
        func: None,
        sampled: false,
        activation_label: None,
        constants: None,
    };
    let mut frame = Frame::new(&ctx);
    model.validate_expr(expr, None).unwrap();
    let value = frame.expression(expr).unwrap();
    frame.discard(value);
    assert!(
        frame.slots.iter().all(|live| !live),
        "every temporary is released"
    );
    frame.body().into_owned()
}

fn occurrences(body: &str, index: usize) -> usize {
    body.matches(&format!("{}ULL", marker(index))).count()
}

/// Deepest nesting of C blocks in `body`.
fn block_depth(body: &str) -> usize {
    let mut depth = 0usize;
    let mut deepest = 0;
    for byte in body.bytes() {
        match byte {
            b'{' => {
                depth += 1;
                deepest = deepest.max(depth);
            }
            b'}' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    deepest
}

#[test]
fn a_priority_chain_renders_in_proportion_to_its_levels() {
    let sizes = [8usize, 16, 32].map(|levels| {
        let body = rendered(&priority_chain(levels));
        for index in 0..=levels {
            assert_eq!(
                occurrences(&body, index),
                1,
                "alternative {index} of {levels} levels is rendered once"
            );
        }
        assert_eq!(body.matches("do {").count(), 1, "one flat chain");
        assert_eq!(body.matches("sv4_eq_to(").count(), levels);
        body.len()
    });
    // Each further level adds the same code: doubling the levels may at
    // most double the size.
    assert!(sizes[1] < sizes[0] * 2, "{sizes:?}");
    assert!(sizes[2] < sizes[1] * 2, "{sizes:?}");
}

#[test]
fn a_priority_chain_does_not_nest_c_blocks_per_level() {
    let shallow = block_depth(&rendered(&priority_chain(8)));
    // Building, validating and dropping the tree recurse per level, which a
    // test thread's stack does not hold for 256 levels.
    let deep = std::thread::Builder::new()
        .stack_size(256 << 20)
        .spawn(|| [64, 256].map(|levels| block_depth(&rendered(&priority_chain(levels)))))
        .unwrap()
        .join()
        .unwrap();
    assert_eq!(deep, [shallow, shallow]);
}

#[test]
fn conditionals_nested_in_the_first_alternative_render_each_level_once() {
    let sizes = [8usize, 16, 32].map(|levels| {
        let body = rendered(&first_alternative_chain(levels));
        // The outer levels, beyond the innermost repeated ones.
        for index in 0..levels - CONDITIONAL_REPEAT_LEVELS as usize {
            assert_eq!(occurrences(&body, index), 1, "{index} of {levels}");
        }
        body.len()
    });
    assert!(sizes[1] < sizes[0] * 2, "{sizes:?}");
    assert!(sizes[2] < sizes[1] * 2, "{sizes:?}");
}

#[test]
fn a_short_conditional_repeats_its_alternatives_for_an_ambiguous_predicate() {
    // The known paths of an ordinary multiplexer test the predicate and
    // nothing else; the ambiguous branch evaluates both alternatives again.
    let body = rendered(&priority_chain(1));
    assert!(body.contains("} else if (!"));
    assert!(!body.contains("do {"));
    assert_eq!(occurrences(&body, 0), 2);
    assert_eq!(occurrences(&body, 1), 2);
    let levels = CONDITIONAL_REPEAT_LEVELS as usize;
    let body = rendered(&priority_chain(levels));
    assert!(!body.contains("do {"));
    assert_eq!(occurrences(&body, levels), 1 << levels);
    // One level more renders every alternative once.
    let body = rendered(&priority_chain(levels + 1));
    assert_eq!(occurrences(&body, levels + 1), 1);
}

#[test]
fn an_ambiguous_level_keeps_its_alternative_and_merges_after_the_chain() {
    let body = rendered(&priority_chain(4));
    // The predicate is reduced to a state before any alternative runs.
    assert_eq!(body.matches("? 1u : sv4_is_unknown(").count(), 4);
    assert_eq!(body.matches("== 1u) break;").count(), 4);
    // The first ambiguous level keeps its alternative; each later one
    // combines with what is kept, and so does the end of the chain.
    assert_eq!(body.matches("sv4_mux_to(").count(), 4);
    let end = body.rfind("} while (0);").unwrap();
    assert_eq!(body[end..].matches("sv4_mux_to(").count(), 1);
    assert_eq!(body[end..].matches("sv4_destroy(").count(), 3);
}

#[test]
fn a_chain_with_side_effects_calls_each_function_once() {
    let call = |value| {
        IrExpr::new(
            IrExprKind::CallFn(Box::new(IrCallExpr::new(
                0,
                vec![IrCallArg::Val(number(value, 65))],
                IrDepth::PROC,
                false,
            ))),
            65,
            false,
            None,
        )
    };
    let expr = mux(equals(0), call(1), mux(equals(1), call(2), call(3)));
    let body = rendered(&expr);
    assert_eq!(body.matches("f_increment(").count(), 3);
    // An alternative with an effect is never rendered twice, also in one
    // conditional.
    let body = rendered(&mux(equals(0), call(1), call(2)));
    assert_eq!(body.matches("f_increment(").count(), 2);
}

#[test]
fn a_borrowed_alternative_is_copied_not_moved() {
    // A fill wider than one word is a pooled constant and a signal read is
    // the signal itself: the result takes a copy of either.
    let model = numeric_model();
    let constants = super::super::super::constants::PackedConstants::default();
    let ctx = RCtx {
        value_backend: crate::sim::value_backend::ValueBackend::Legacy,
        model: &model,
        effects: &crate::sim::execution::EffectSummaries::new(&model),
        func: None,
        sampled: false,
        activation_label: None,
        constants: Some(&constants),
    };
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
    let fill = IrExpr::new(IrExprKind::Fill(3), 65, false, Some(3));
    let expr = mux(equals(0), call, mux(equals(1), read(), fill));
    let mut frame = Frame::new(&ctx);
    model.validate_expr(&expr, None).unwrap();
    let value = frame.expression(&expr).unwrap();
    frame.discard(value);
    let body = frame.body().into_owned();
    assert!(body.contains("do {"), "{body}");
    for line in body.lines().filter(|line| line.contains("sv4_move(")) {
        let source = line.rsplit(", ").next().unwrap();
        assert!(
            source.starts_with("&_llg_t["),
            "moves from a borrow: {line}"
        );
    }
    assert!(find_copy(&body, "&G_value").is_some(), "{body}");
    assert!(frame.slots.iter().all(|live| !live));
}

#[test]
fn a_known_predicate_selects_without_a_state_or_a_merge() {
    let same = IrExpr::new(
        IrExprKind::Bin {
            op: IrBinOp::CaseEq,
            a: Box::new(read()),
            b: Box::new(number(0, 65)),
        },
        1,
        false,
        None,
    );
    let chain = (0..8).rev().fold(alternative(8), |rest, level| {
        mux(same.clone(), alternative(level), rest)
    });
    let body = rendered(&chain);
    assert!(!body.contains("sv4_mux_to("));
    assert!(!body.contains("sv4_is_unknown("));
    assert_eq!(body.matches("break;").count(), 8);
    for index in 0..=8 {
        assert_eq!(occurrences(&body, index), 1);
    }
}

#[test]
fn a_real_chain_yields_zero_for_an_ambiguous_predicate() {
    let real = |value: f64| IrExpr::new(IrExprKind::Const(IrConst::real(value)), 0, false, None);
    let call = IrExpr::new(
        IrExprKind::CastToReal {
            a: Box::new(IrExpr::new(
                IrExprKind::CallFn(Box::new(IrCallExpr::new(
                    0,
                    vec![IrCallArg::Val(number(7, 65))],
                    IrDepth::PROC,
                    false,
                ))),
                65,
                false,
                None,
            )),
            shortreal: false,
        },
        0,
        false,
        None,
    );
    let expr = mux(equals(0), real(2.5), mux(equals(1), call, real(4.5)));
    let body = rendered(&expr);
    assert_eq!(body.matches("f_increment(").count(), 1);
    assert_eq!(body.matches("2.5").count(), 1);
    let end = body.rfind("} while (0);").unwrap();
    assert!(body[end..].contains("= 0.0;"));
    assert!(!body.contains("sv4_mux_to("));
}

#[test]
fn string_and_handle_chains_render_each_alternative_once() {
    let model = IrModel::new("native_chain".into(), 1).unwrap();
    let ctx = RCtx {
        value_backend: crate::sim::value_backend::ValueBackend::Legacy,
        model: &model,
        effects: &crate::sim::execution::EffectSummaries::new(&model),
        func: None,
        sampled: false,
        activation_label: None,
        constants: None,
    };
    let predicate = |value: u64| {
        IrExpr::new(
            IrExprKind::Bin {
                op: IrBinOp::Eq,
                a: Box::new(number(3, 8)),
                b: Box::new(number(value, 8)),
            },
            1,
            false,
            None,
        )
    };
    let sizes = [8usize, 16, 32].map(|levels| {
        let literal = |index: usize| IrStringExpr::Literal(format!("alt{index};").into_bytes());
        let chain =
            (0..levels)
                .rev()
                .fold(literal(levels), |rest, level| IrStringExpr::Conditional {
                    predicate: Box::new(predicate(level as u64)),
                    then: Box::new(literal(level)),
                    otherwise: Box::new(rest),
                });
        let mut frame = Frame::new(&ctx);
        let value = frame.string(&chain).unwrap();
        frame.native_discard(value);
        let body = frame.body().into_owned();
        assert_eq!(body.matches("llg_string_bytes_to(").count(), levels + 1);
        // One merge per ambiguous level after the first, one at the end.
        assert_eq!(
            body.matches("llg_string_conditional_merge(").count(),
            levels
        );
        let handles = (0..levels).rev().fold(IrChandleExpr::Null, |rest, level| {
            IrChandleExpr::Conditional {
                predicate: Box::new(predicate(level as u64)),
                then: Box::new(IrChandleExpr::Null),
                otherwise: Box::new(rest),
            }
        });
        let mut frame = Frame::new(&ctx);
        let handle = frame.chandle(&handles).unwrap();
        assert!(!handle.is_empty());
        let handles = frame.body().into_owned();
        assert_eq!(handles.matches("sv4_eq_to(").count(), levels);
        body.len() + handles.len()
    });
    assert!(sizes[1] < sizes[0] * 2, "{sizes:?}");
    assert!(sizes[2] < sizes[1] * 2, "{sizes:?}");
}
