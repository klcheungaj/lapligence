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
            && cg.query_descriptor(node).is_some_and(|descriptor| {
                matches!(descriptor.shape, TypeShape::FixedArray { .. })
            })
        {
            if let Some(instance) = cg.owning_inst(node) {
                cg.inst = instance;
            }
            assert!(cg.fixed_activation_read("tb", node).unwrap().is_none());
            assert!(cg.lower_bitstream_source("tb", node).unwrap().is_none());
            let value = cg.lower_expr("tb", node).unwrap();
            assert!(matches!(value.kind, IrExprKind::CallFn(_)), "typed conversion helper retained");
            conversions += 1;
        }
    }
    assert!(conversions >= 2, "reach both sides of the four/two/four-state conversion");
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
    let (port, actual, internal) = database.node_ids().find_map(|node| {
        let NodeKind::Port { high_expr, high, low: Some(internal), .. } = cg.kind(node) else {
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
    }).expect("input cast actual");
    cg.inst = cg.owning_inst(port).unwrap();
    let dims = cg.array_of(internal).unwrap().dims.clone();
    let mut captures = Vec::new();
    let values = cg.p30_lower_source_values(
        "tb", internal, actual, &dims, &mut captures, &mut HashMap::new(),
    ).unwrap();
    assert_eq!(values.len(), 2);
    let IrStmt::DeclLocal { init: Some(value), .. } = &captures[0] else {
        panic!("complete input value capture");
    };
    assert!(matches!(value.kind, IrExprKind::CallFn(_)), "do not scatter the unconverted operand");
    assert!(cg.model.funcs.iter().any(|function| {
        function.c_name.starts_with("_llg_fixed_cast_")
            && function.ret.is_some_and(|ty| ty.two_state())
    }));
}
