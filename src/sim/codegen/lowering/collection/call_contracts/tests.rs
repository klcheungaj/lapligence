//! Read-only eligibility is decided on all target leaves and operation flags.
use super::*;

#[test]
fn arithmetic_assignment_flags_are_not_mistaken_for_reads() {
    use crate::core::db::{ConstantSource, ConstantType, Node};
    use crate::core::model::TypeInfo;
    for assignment in [false, true] {
        let kinds = vec![
            (
                NodeKind::Var {
                    ty: TypeInfo {
                        kind: "logic".into(),
                        width: Some(8),
                        signed: false,
                        type_name: None,
                    },
                },
                vec![],
            ),
            (
                NodeKind::Expr(ExprKind::Ref {
                    target: Some(NodeId(0)),
                }),
                vec![],
            ),
            (
                NodeKind::Expr(ExprKind::Constant {
                    const_type: ConstantType::Binary,
                    value: ValueData::Bin("1".into()),
                    size: 1,
                    source: ConstantSource::NotCaptured,
                    time_scale: None,
                }),
                vec![],
            ),
            (
                NodeKind::Expr(ExprKind::Operation {
                    op: Operation::Add,
                    reordered: false,
                    assignment,
                    operands: vec![NodeId(1), NodeId(2)],
                }),
                vec![NodeId(1), NodeId(2)],
            ),
        ];
        let nodes = kinds
            .into_iter()
            .map(|(kind, children)| Node {
                kind,
                children,
                parent: None,
                name: String::new(),
                full_name: String::new(),
                file: None,
                line: 0,
                col: 0,
                end_line: 0,
                end_col: 0,
            })
            .collect();
        let database =
            Db::from_test_nodes("callback-flags", nodes, vec![], HashMap::new()).unwrap();
        let semantic = crate::sim::semantic::SemanticModel::from_db(&database);
        let cg = Codegen::new(&semantic);
        let result = cg.check_event_expression_effects(NodeId(3), "tb");
        if assignment {
            assert!(result
                .unwrap_err()
                .contains("writes external or persistent storage"));
        } else {
            result.unwrap();
        }
    }
}

#[test]
fn composite_helper_targets_are_all_private_or_rejected_at_source() {
    for (name, source, allowed, callee_name) in [
        ("helper_private.sv", include_str!("../../../../../../tests/fixtures/sim/continuation_24_27/helper_private.sv"), true, "computed"),
        ("helper_external_concat.sv", include_str!("../../../../../../tests/fixtures/sim/continuation_24_27/helper_external_concat.sv"), false, "bad"),
        ("helper_external_compound.sv", include_str!("../../../../../../tests/fixtures/sim/continuation_24_27/helper_external_compound.sv"), false, "bad"),
    ] {
        let database = {
            let output = crate::core::compile::compile_sources_checked(
                &[crate::core::compile::OwnedSource::compilation_unit(name, source)],
                &crate::core::compile::CompileOpts { top: Some("tb".to_owned()), ..Default::default() },
            ).unwrap();
            Db::from_slang(&output.snapshot).unwrap()
        };
        let semantic = crate::sim::semantic::SemanticModel::from_db(&database);
        let mut cg = Codegen::new(&semantic);
        let tops = cg.collect_design().unwrap();
        cg.bind_reference_ports().unwrap();
        for top in tops { cg.emit_func_prototypes(top).unwrap(); }
        let call = database.node_ids().find(|node| {
            matches!(cg.kind(*node), NodeKind::FuncCall { name, .. } if name == callee_name)
        }).unwrap();
        if let Some(instance) = cg.owning_inst(call) { cg.inst = instance; }
        let result = cg.check_event_expression_effects(call, "tb");
        if allowed { result.unwrap(); }
        else { assert!(result.unwrap_err().contains("writes external or persistent storage")); }
    }
}
