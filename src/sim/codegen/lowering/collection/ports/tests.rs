//! Input values and structural targets retain their distinct contracts.
use super::*;

fn cast_database() -> Db {
    let result = crate::core::compile::compile_sources_checked(
        &[crate::core::compile::OwnedSource::compilation_unit(
            "input_casts.sv",
            include_str!("../../../../../../tests/fixtures/sim/continuation_24_27/input_casts.sv"),
        )],
        &crate::core::compile::CompileOpts {
            top: Some("tb".to_owned()),
            ..Default::default()
        },
    )
    .expect("legal nested fixed-array casts");
    Db::from_slang(&result.snapshot).unwrap()
}

#[test]
fn fixed_input_casts_are_values_not_storage_aliases() {
    let database = cast_database();
    database.validate().unwrap();
    let semantic = crate::sim::semantic::SemanticModel::from_db(&database);
    let mut cg = Codegen::new(&semantic);
    cg.collect_design().unwrap();
    cg.bind_reference_ports().unwrap();
    let mut conversions = 0;
    for node in database.node_ids() {
        if matches!(cg.kind(node), NodeKind::Expr(ExprKind::Cast { .. }))
            && cg
                .query_descriptor(node)
                .is_some_and(|descriptor| matches!(descriptor.shape, TypeShape::FixedArray { .. }))
        {
            if let Some(instance) = cg.owning_inst(node) {
                cg.inst = instance;
            }
            assert!(cg.fixed_activation_read("tb", node).unwrap().is_none());
            assert!(cg.lower_bitstream_source("tb", node).unwrap().is_none());
            let value = cg.lower_expr("tb", node).unwrap();
            assert!(
                matches!(value.kind, IrExprKind::CallFn(_)),
                "typed conversion helper retained"
            );
            conversions += 1;
        }
    }
    assert!(
        conversions >= 2,
        "reach both sides of the four/two/four-state conversion"
    );
    assert!(cg.model.funcs.iter().any(|function| {
        function.c_name.starts_with("_llg_fixed_cast_")
            && function.ret.is_some_and(|ty| ty.two_state())
    }));
}

#[test]
fn fixed_input_scatter_snapshots_the_complete_converted_expression() {
    let database = cast_database();
    let semantic = crate::sim::semantic::SemanticModel::from_db(&database);
    let mut cg = Codegen::new(&semantic);
    cg.collect_design().unwrap();
    cg.bind_reference_ports().unwrap();
    let (port, actual, internal) = database
        .node_ids()
        .find_map(|node| {
            let NodeKind::Port {
                high_expr,
                high,
                low: Some(internal),
                ..
            } = cg.kind(node)
            else {
                return None;
            };
            let actual = (*high_expr).or(*high)?;
            // An implicit formal-shape conversion may wrap the explicit chain.
            let mut candidate = actual;
            let mut nested_two_state = false;
            while let NodeKind::Expr(ExprKind::Cast { operand, .. }) = cg.kind(candidate) {
                if cg.query_descriptor(candidate).is_some_and(|descriptor| {
                    descriptor.two_state && matches!(descriptor.shape, TypeShape::FixedArray { .. })
                }) {
                    nested_two_state = true;
                }
                candidate = *operand;
            }
            (nested_two_state && cg.fixed_array_port_shape(actual).is_some())
                .then_some((node, actual, *internal))
        })
        .expect("input cast actual");
    cg.inst = cg.owning_inst(port).unwrap();
    let dims = cg.array_of(internal).unwrap().dims.clone();
    let mut captures = Vec::new();
    let values = cg
        .p30_lower_source_values(
            "tb",
            internal,
            actual,
            &dims,
            &mut captures,
            &mut HashMap::new(),
        )
        .unwrap();
    assert_eq!(values.len(), 2);
    let IrStmt::DeclLocal {
        init: Some(value), ..
    } = &captures[0]
    else {
        panic!("complete input value capture");
    };
    assert!(
        matches!(value.kind, IrExprKind::CallFn(_)),
        "do not scatter the unconverted operand"
    );
    assert!(cg.model.funcs.iter().any(|function| {
        function.c_name.starts_with("_llg_fixed_cast_")
            && function.ret.is_some_and(|ty| ty.two_state())
    }));
}

#[test]
fn selected_terminal_remapping_keeps_each_output_contribution_distinct() {
    // Exercise the non-alias fallback directly: ordinary source terminals may
    // use the alias projection route, which must not hide a missing IR branch.
    let database = cast_database();
    let semantic = crate::sim::semantic::SemanticModel::from_db(&database);
    let mut cg = Codegen::new(&semantic);
    let source = database.node_ids().next().unwrap();
    cg.model.net_groups.push(crate::sim::ir::IrNetGroup {
        c_name: "selected_outputs".to_owned(),
        width: 129,
        signed: false,
        kind: crate::sim::ir::IrNetKind::Wire,
        n_drivers: 0,
        driver_strengths: Vec::new(),
        propagation_delay: None,
        strength_view: None,
    });
    let first = cg
        .add_structural_driver_for_terminal(0, source, (6, 6), 0)
        .unwrap();
    let second = cg
        .add_structural_driver_for_terminal(0, source, (6, 6), 1)
        .unwrap();
    assert_ne!(first, second);
    let steps = vec![crate::sim::ir::IrPackedSelect {
        base: lhs_integer_expr(32),
        width: 65,
    }];
    let target = IrLhs::PackedSelect {
        target: Box::new(IrLhs::Stream {
            parts: vec![(IrLhs::Whole(first), 129)],
            width: 129,
            slice: 1,
            direction: IrStreamDirection::LeftToRight,
        }),
        steps: steps.clone(),
        signed: true,
        two_state: false,
    };
    for (terminal, expected) in [(0, first), (1, second)] {
        let mapped = cg.remap_structural_lhs_for_terminal(target.clone(), source, terminal);
        assert_eq!(cg.structural_group_for_lhs(&mapped), Some(0));
        assert!(cg
            .unmapped_structural_group_for_terminal(&mapped, source, terminal)
            .is_none());
        let IrLhs::PackedSelect {
            target,
            steps: actual,
            signed,
            two_state,
        } = mapped
        else {
            panic!("typed selection must remain intact");
        };
        assert_eq!(actual, steps);
        assert!(signed);
        assert!(!two_state);
        let IrLhs::Stream { parts, .. } = *target else {
            panic!("composite target retained");
        };
        assert_eq!(parts, vec![(IrLhs::Whole(expected), 129)]);
    }
    assert_eq!(cg.model.net_groups[0].n_drivers, 2);
}
