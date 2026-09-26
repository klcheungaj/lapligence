//! Recursive pattern checks inherit their case mode without untracked owners.
use super::*;
use crate::sim::ir::{IrPatternCheck, IrPatternExpr, IrPatternMatchKind};

#[test]
fn recursive_pattern_checks_use_the_selected_comparator() {
    for (mode, comparator) in [
        (IrPatternMatchKind::Exact, "sv4_case_eq("),
        (IrPatternMatchKind::Casez, "sv4_casez_eq("),
        (IrPatternMatchKind::Casex, "sv4_casex_eq("),
    ] {
        let model = IrModel::new("pattern_modes".to_owned(), 1).unwrap();
        let ctx = RCtx {
            model: &model,
            func: None,
            sampled: false,
            activation_label: None,
        };
        let mut frame = Frame::new(&ctx);
        let expression = IrExpr::new(
            IrExprKind::Pattern(Box::new(IrPatternExpr {
                value: Box::new(number(0, 6)),
                constant: None,
                binding: None,
                match_kind: mode,
                checks: [5, 4, 0]
                    .into_iter()
                    .map(|offset| IrPatternCheck {
                        offset,
                        width: 1,
                        signed: false,
                        two_state: false,
                        exact: false,
                        constant: Some(Box::new(number(0, 1))),
                        binding: None,
                    })
                    .collect(),
            })),
            1,
            false,
            None,
        );
        model.validate_expr(&expression, None).unwrap();
        let result = frame.expression(&expression).unwrap();
        frame.discard(result);
        assert_eq!(frame.body().matches(comparator).count(), 3);
        assert!(frame.slots.iter().all(|live| !live));
    }
}
