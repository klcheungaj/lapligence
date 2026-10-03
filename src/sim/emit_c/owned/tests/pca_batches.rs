use super::*;
use crate::sim::emit_c::model::render_with_sharing_threshold;
use crate::sim::emit_c::statements::pca_batches::PCA_BATCH_MIN_ASSIGNMENTS;

fn model(count: usize) -> IrModel {
    let mut model = IrModel::new("pca_batches".to_owned(), 1).unwrap();
    for index in 0..count {
        for (prefix, width) in [("source", 8), ("target", 8), ("enable", 1)] {
            model.signals.push(
                IrSignal::new(
                    format!("G_{prefix}_{index}"),
                    None,
                    IrType::Packed {
                        width,
                        signed: false,
                        two_state: false,
                    },
                    None,
                )
                .unwrap(),
            );
        }
    }
    model.processes.push(IrProcess::new(
        "p_assign".to_owned(),
        "tb".to_owned(),
        IrShape::RunOnce,
        Vec::new(),
        assignments(0, count),
    ));
    model.spawns.push("p_assign".to_owned());
    model
}

fn assignments(first: usize, count: usize) -> Vec<IrStmt> {
    (first..first + count)
        .map(|index| IrStmt::PcaAssign {
            sig: index * 3 + 1,
            enable: index * 3 + 2,
            site: index,
            value: IrExpr::new(IrExprKind::SigRead(index * 3), 8, false, None),
        })
        .collect()
}

fn render(model: IrModel) -> String {
    render_with_sharing_threshold(&ExecutionModel::lower(model).unwrap(), usize::MAX).unwrap()
}

#[test]
fn homogeneous_rows_share_one_helper_and_constant_size_loop() {
    for count in [PCA_BATCH_MIN_ASSIGNMENTS, 41, 100] {
        let source = render(model(count));
        assert_eq!(
            source
                .matches("static LLG_MODEL_SHARED void llg_pca_batch_")
                .count(),
            1
        );
        assert_eq!(
            source
                .matches("static const llg_pca_row_0_t llg_pca_rows_")
                .count(),
            1
        );
        assert_eq!(source.matches("for (; _llg_pca_i_").count(), 1);
        assert_eq!(source.matches("sv4_clone(row->source)").count(), 1);
        assert_eq!(source.matches("llg_pca_assign(row->target").count(), 1);
        assert_eq!(source.matches("sv4_destroy(&_llg_t[0])").count(), 1);
        for index in 0..count {
            assert!(source.contains(&format!(
                "{{ &G_source_{index}, &G_target_{index}, &G_enable_{index}, {index}ULL }}"
            )));
        }
    }
}

#[test]
fn short_runs_keep_exact_statement_emission() {
    let model = model(PCA_BATCH_MIN_ASSIGNMENTS - 1);
    let ctx = RCtx {
        model: &model,
        func: None,
        sampled: false,
        activation_label: None,
        constants: None,
    };
    let mut original = Frame::new(&ctx);
    original.block(&model.processes[0].body).unwrap();
    let mut batched = Frame::new(&ctx);
    batched.pca_owner = Some("p_assign".to_owned());
    batched.block(&model.processes[0].body).unwrap();
    assert_eq!(original.body(), batched.body());
    assert!(batched.pca_batches.is_empty());
}

#[test]
fn static_local_sources_batch_and_lexical_cells_remain_independent() {
    let mut model = model(PCA_BATCH_MIN_ASSIGNMENTS);
    let mut function = IrFunc::new(
        "f_static".to_owned(),
        None,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );
    function
        .locals
        .push(IrLocal::new("static_source".to_owned(), 8, false).unwrap());
    model.funcs.push(function);
    for statement in &mut model.processes[0].body {
        let IrStmt::PcaAssign { value, .. } = statement else {
            unreachable!()
        };
        *value = IrExpr::new(
            IrExprKind::LocalRead("static_source".to_owned()),
            8,
            false,
            None,
        );
    }
    let source = render(model.clone());
    assert!(source.contains("{ &static_source, &G_target_0"));
    model.processes[0].body.insert(
        0,
        IrStmt::DeclLocal {
            name: "static_source".to_owned(),
            width: 8,
            signed: false,
            init: None,
            two_state: false,
        },
    );
    let source = render(model);
    assert!(!source.contains("llg_pca_batch_"));
}

#[test]
fn computed_sources_and_dynamic_indices_are_ineligible() {
    for dynamic in [false, true] {
        let mut model = model(PCA_BATCH_MIN_ASSIGNMENTS);
        for statement in &mut model.processes[0].body {
            let IrStmt::PcaAssign { value, .. } = statement else {
                unreachable!()
            };
            *value = if dynamic {
                IrExpr::new(
                    IrExprKind::Resize {
                        a: Box::new(IrExpr::new(
                            IrExprKind::BitSel {
                                base: Box::new(value.clone()),
                                idx: Box::new(value.clone()),
                            },
                            1,
                            false,
                            None,
                        )),
                    },
                    8,
                    false,
                    None,
                )
            } else {
                add(value.clone(), number(0, 8), 8)
            };
        }
        let source = render(model);
        assert!(!source.contains("llg_pca_batch_"));
        assert_eq!(
            source.matches("llg_pca_assign(&G_target_").count(),
            PCA_BATCH_MIN_ASSIGNMENTS
        );
    }
}

#[test]
fn constant_indices_borrow_pool_beside_live_loop_values() {
    let mut model = model(PCA_BATCH_MIN_ASSIGNMENTS);
    for statement in &mut model.processes[0].body {
        let IrStmt::PcaAssign { value, .. } = statement else {
            unreachable!()
        };
        *value = IrExpr::new(
            IrExprKind::Resize {
                a: Box::new(IrExpr::new(
                    IrExprKind::BitSel {
                        base: Box::new(value.clone()),
                        idx: Box::new(number(3, 32)),
                    },
                    1,
                    false,
                    None,
                )),
            },
            8,
            false,
            None,
        );
    }
    model.processes[0].body = vec![IrStmt::Repeat {
        count: number(5, 32),
        body: model.processes[0].body.clone(),
    }];
    let source = render(model);
    assert!(source.contains("llg_pca_batch_0(_llg_t + 1,"));
    assert!(source.contains("llg_value_scope_begin(2)"));
    assert!(source.contains("sv4_bit_select(_llg_t[0], sv4_to_index(llg_constant_"));
}

#[test]
fn real_rows_have_typed_addresses_and_no_packed_temporary() {
    let mut model = model(PCA_BATCH_MIN_ASSIGNMENTS);
    for index in 0..PCA_BATCH_MIN_ASSIGNMENTS {
        model.signals[index * 3].ty = IrType::Real { shortreal: false };
        model.signals[index * 3 + 1].ty = IrType::Real { shortreal: false };
        let IrStmt::PcaAssign { value, .. } = &mut model.processes[0].body[index] else {
            unreachable!()
        };
        *value = IrExpr::new(IrExprKind::SigRead(index * 3), 0, false, None);
    }
    let source = render(model);
    assert!(source.contains("const double* source; double* target;"));
    assert!(source.contains("llg_pca_assign_d(row->target"));
    assert!(source.contains("llg_pca_batch_0(NULL,"));
    assert!(!source.contains("sv4_clone(row->source)"));
}

#[test]
fn mixed_selections_casts_and_widths_split_shapes() {
    let count = PCA_BATCH_MIN_ASSIGNMENTS;
    let mut model = model(count * 3);
    for index in count..count * 2 {
        let IrStmt::PcaAssign { value, .. } = &mut model.processes[0].body[index] else {
            unreachable!()
        };
        *value = IrExpr::new(
            IrExprKind::Resize {
                a: Box::new(IrExpr::new(
                    IrExprKind::PartSel {
                        base: Box::new(value.clone()),
                        left: 6,
                        right: 3,
                    },
                    4,
                    false,
                    None,
                )),
            },
            8,
            true,
            None,
        );
    }
    for index in count * 2..count * 3 {
        model.signals[index * 3 + 1].ty = IrType::Packed {
            width: 16,
            signed: true,
            two_state: true,
        };
        let IrStmt::PcaAssign { value, .. } = &mut model.processes[0].body[index] else {
            unreachable!()
        };
        *value = IrExpr::new(
            IrExprKind::Convert {
                a: Box::new(value.clone()),
            },
            16,
            true,
            None,
        );
    }
    let source = render(model);
    assert_eq!(
        source
            .matches("static LLG_MODEL_SHARED void llg_pca_batch_")
            .count(),
        3
    );
    assert_eq!(source.matches("for (; _llg_pca_i_").count(), 3);
    assert!(source.contains("sv4_part_select(_llg_t[0], 6, 3)"));
    assert!(source.contains("sv4_to_two_state(_llg_t[0])"));
}

#[test]
fn suspensions_and_dependent_statements_break_runs() {
    let count = PCA_BATCH_MIN_ASSIGNMENTS;
    for barrier in [
        IrStmt::Delay {
            ticks: IrDelay::Constant(1),
        },
        IrStmt::Assign {
            lhs: IrLhs::Whole(0),
            rhs: number(9, 8),
            nba: false,
        },
        IrStmt::PcaDeassign { sig: 1 },
        IrStmt::Nop,
    ] {
        let mut model = model(count * 2);
        model.processes[0].body.insert(count, barrier);
        let source = render(model);
        assert_eq!(source.matches("for (; _llg_pca_i_").count(), 2);
        assert_eq!(
            source
                .matches("static LLG_MODEL_SHARED void llg_pca_batch_")
                .count(),
            1
        );
    }
}

#[test]
fn repeated_targets_and_source_dependencies_keep_row_order() {
    let mut model = model(4);
    model.processes[0].body = vec![
        IrStmt::PcaAssign {
            sig: 1,
            enable: 2,
            site: 0,
            value: IrExpr::new(IrExprKind::SigRead(0), 8, false, None),
        },
        IrStmt::PcaAssign {
            sig: 4,
            enable: 5,
            site: 1,
            value: IrExpr::new(IrExprKind::SigRead(1), 8, false, None),
        },
        IrStmt::PcaAssign {
            sig: 1,
            enable: 8,
            site: 2,
            value: IrExpr::new(IrExprKind::SigRead(3), 8, false, None),
        },
        IrStmt::PcaAssign {
            sig: 10,
            enable: 11,
            site: 3,
            value: IrExpr::new(IrExprKind::SigRead(1), 8, false, None),
        },
    ];
    let source = render(model);
    let mut last = 0;
    for row in [
        "{ &G_source_0, &G_target_0, &G_enable_0, 0ULL }",
        "{ &G_target_0, &G_target_1, &G_enable_1, 1ULL }",
        "{ &G_source_1, &G_target_0, &G_enable_2, 2ULL }",
        "{ &G_target_0, &G_target_3, &G_enable_3, 3ULL }",
    ] {
        let position = source.find(row).unwrap();
        assert!(position > last);
        last = position;
    }
}

#[test]
fn shared_instances_load_their_own_tables_from_records() {
    let count = PCA_BATCH_MIN_ASSIGNMENTS;
    let mut model = model(count * 4);
    model.processes.clear();
    model.spawns.clear();
    for instance in 0..4 {
        let name = format!("p_assign_{instance}");
        let mut process = IrProcess::new(
            name.clone(),
            format!("tb.u{instance}"),
            IrShape::RunOnce,
            Vec::new(),
            assignments(instance * count, count),
        );
        process.origin = crate::sim::semantic::Origin::Synthetic {
            reason: "one source body".to_owned(),
        };
        model.processes.push(process);
        model.spawns.push(name);
    }
    let source = render_with_sharing_threshold(&ExecutionModel::lower(model).unwrap(), 4).unwrap();
    assert_eq!(source.matches("LLG_CO_DISPATCH_BEGIN").count(), 1);
    assert_eq!(source.matches("for (; _llg_pca_i_").count(), 1);
    assert!(source.contains("const llg_pca_row_0_t* v"));
    assert!(source.contains("&I->v0[_llg_pca_i_"), "{source}");
    for instance in 0..4 {
        assert!(source.contains(&format!("= {{ llg_pca_rows_p_assign_{instance}_0 }};")));
    }
}
