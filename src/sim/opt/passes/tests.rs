use super::*;
use crate::sim::ir::{
    IrAssertion, IrCall, IrCallArg, IrCaseItem, IrConcurrentAssertionKind, IrDependency, IrDepth,
    IrEdge, IrEventRef, IrFunc, IrLocal, IrProcess, IrShape, IrSignal, IrType,
    IrUniquePriorityCheck,
};

// ── builders ──────────────────────────────────────────────────────────

fn konst(v: u64, w: u32) -> IrExpr {
    IrExpr::new(
        IrExprKind::Const(IrConst {
            bits: vec![v],
            x: vec![0],
            z: vec![0],
            width: w,
            signed: false,
            real: None,
            fill: None,
        }),
        w,
        false,
        None,
    )
}

fn xkonst(w: u32) -> IrExpr {
    IrExpr::new(
        IrExprKind::Const(IrConst {
            bits: vec![0],
            x: vec![u64::MAX],
            z: vec![0],
            width: w,
            signed: false,
            real: None,
            fill: None,
        }),
        w,
        false,
        None,
    )
}

fn zkonst(w: u32) -> IrExpr {
    IrExpr::new(
        IrExprKind::Const(IrConst {
            bits: vec![0],
            x: vec![0],
            z: vec![u64::MAX],
            width: w,
            signed: false,
            real: None,
            fill: None,
        }),
        w,
        false,
        None,
    )
}

fn masked_konst(bits: u64, x: u64, z: u64, w: u32) -> IrExpr {
    IrExpr::new(
        IrExprKind::Const(IrConst {
            bits: vec![bits],
            x: vec![x],
            z: vec![z],
            width: w,
            signed: false,
            real: None,
            fill: None,
        }),
        w,
        false,
        None,
    )
}

fn rkonst(v: f64) -> IrExpr {
    IrExpr::new(
        IrExprKind::Const(IrConst {
            bits: vec![0],
            x: vec![0],
            z: vec![0],
            width: 0,
            signed: true,
            real: Some(v),
            fill: None,
        }),
        0,
        true,
        None,
    )
}

fn bin(op: IrBinOp, a: IrExpr, b: IrExpr, w: u32) -> IrExpr {
    IrExpr::new(
        IrExprKind::Bin {
            op,
            a: Box::new(a),
            b: Box::new(b),
        },
        w,
        false,
        None,
    )
}

fn bitneg(a: IrExpr) -> IrExpr {
    let (w, sg) = (a.width, a.signed);
    IrExpr::new(
        IrExprKind::Un {
            op: IrUnOp::BitNeg,
            a: Box::new(a),
        },
        w,
        sg,
        None,
    )
}

fn assign(lhs: IrLhs, rhs: IrExpr) -> IrStmt {
    IrStmt::Assign {
        lhs,
        rhs,
        nba: false,
    }
}

fn sigs(n: usize) -> Vec<IrSignal> {
    (0..n)
        .map(|i| IrSignal {
            c_name: format!("G_s{i}"),
            hdl_name: Some(format!("t.s{i}")),
            ty: IrType::Packed {
                width: 8,
                signed: false,
                two_state: false,
            },
            net_driver: None,
            net_alias: Vec::new(),
            alias: None,
            omit: false,
        })
        .collect()
}

fn model_with(body: Vec<IrStmt>, signals: Vec<IrSignal>) -> IrModel {
    IrModel {
        design_name: "t".to_string(),
        precision_fs: 1,
        waveform: false,
        signals,
        net_groups: Vec::new(),
        arrays: Vec::new(),
        objects: Vec::new(),
        classes: Vec::new(),
        containers: Vec::new(),
        events: Vec::new(),
        funcs: Vec::new(),
        assertions: Vec::new(),
        sampled_domains: Vec::new(),
        virtual_interfaces: Vec::new(),
        processes: vec![IrProcess {
            c_name: "p_t_proc_0".to_string(),
            label: "t.always".to_string(),
            kind: crate::sim::ir::IrProcessKind::Synthetic,
            shape: IrShape::RunOnce,
            writes: Vec::new(),
            pre_fns: Vec::new(),
            body,
            program: None,
            origin: crate::sim::semantic::Origin::Synthetic {
                reason: "optimizer fixture".to_owned(),
            },
        }],
        init_steps: Vec::new(),
        spawns: Vec::new(),
        final_spawns: Vec::new(),
        vpi_objects: Vec::new(),
        vpi_compile_calls: Vec::new(),
    }
}

fn fold_only() -> OptConfig {
    OptConfig {
        fold_constants: true,
        identities: false,
        prune_branches: false,
        unused_storage: false,
    }
}

fn idents_only() -> OptConfig {
    OptConfig {
        fold_constants: false,
        identities: true,
        prune_branches: false,
        unused_storage: false,
    }
}

fn prune_only() -> OptConfig {
    OptConfig {
        fold_constants: false,
        identities: false,
        prune_branches: true,
        unused_storage: false,
    }
}

fn storage_only() -> OptConfig {
    OptConfig {
        fold_constants: false,
        identities: false,
        prune_branches: false,
        unused_storage: true,
    }
}

fn proc_body(m: &IrModel) -> &[IrStmt] {
    &m.processes[0].body
}

fn const_payload(e: &IrExpr) -> Option<(u64, u32)> {
    match &e.kind {
        IrExprKind::Const(c) if c.real.is_none() => Some((c.bits[0], c.width)),
        _ => None,
    }
}

fn first_assign_rhs_of(s: &IrStmt) -> &IrExpr {
    match s {
        IrStmt::Assign { rhs, .. } => rhs,
        other => panic!("expected assignment, got {other:?}"),
    }
}

/// The assigned RHS of the first statement, for payload assertions.
fn first_assign_rhs(m: &IrModel) -> &IrExpr {
    match &m.processes[0].body[0] {
        IrStmt::Assign { rhs, .. } => rhs,
        other => panic!("expected assignment, got {other:?}"),
    }
}

/// The sole statement of a pruned body, unwrapped from its splice Block.
fn single_pruned_stmt(m: &IrModel) -> &IrStmt {
    assert_eq!(m.processes[0].body.len(), 1, "one statement expected");
    match &m.processes[0].body[0] {
        IrStmt::Block(b) => {
            assert_eq!(b.len(), 1, "single spliced statement expected");
            &b[0]
        }
        other => other,
    }
}

// ── fold_constants ────────────────────────────────────────────────────

#[test]
fn fold_add_constants() {
    let mut m = model_with(
        vec![assign(
            IrLhs::Whole(0),
            bin(IrBinOp::Add, konst(5, 8), konst(6, 8), 8),
        )],
        sigs(1),
    );
    run(&mut m, &fold_only());
    assert_eq!(const_payload(first_assign_rhs(&m)), Some((11, 8)));
}

#[test]
fn fold_constants_visits_function_local_initializers() {
    let mut local = IrLocal::new("local".to_string(), 8, false).expect("valid local");
    local.initial = Some(bin(IrBinOp::Add, konst(5, 8), konst(6, 8), 8));
    let mut m = model_with(Vec::new(), Vec::new());
    m.funcs.push(IrFunc::new(
        "f".to_string(),
        None,
        Vec::new(),
        vec![local],
        Vec::new(),
        Vec::new(),
    ));

    run(&mut m, &fold_only());

    assert_eq!(
        m.funcs[0].locals[0]
            .initial
            .as_ref()
            .and_then(const_payload),
        Some((11, 8))
    );
}

#[test]
fn fold_x_propagation_yields_the_x_constant() {
    // elab arithmetic propagates X/Z exactly like the runtime ops, so
    // Add(5, X) folds to the 8-bit all-X constant.  (Only Div/Mod/Pow
    // stay symbolic for unknown operands.)
    let mut m = model_with(
        vec![assign(
            IrLhs::Whole(0),
            bin(IrBinOp::Add, konst(5, 8), xkonst(8), 8),
        )],
        sigs(1),
    );
    run(&mut m, &fold_only());
    match &first_assign_rhs(&m).kind {
        IrExprKind::Const(c) => {
            assert_eq!(c.width, 8);
            assert!(c.x.iter().any(|&v| v != 0), "result carries X bits");
            assert_eq!((c.bits[0], c.z[0]), (0, 0));
        }
        other => panic!("expected folded X constant, got {other:?}"),
    }
}

#[test]
fn fold_division_guard() {
    // Unknown operand: stays symbolic.
    let mut m = model_with(
        vec![assign(
            IrLhs::Whole(0),
            bin(IrBinOp::Div, konst(6, 32), xkonst(32), 32),
        )],
        sigs(1),
    );
    run(&mut m, &fold_only());
    if let IrStmt::Assign { rhs, .. } = &proc_body(&m)[0] {
        assert!(matches!(rhs.kind, IrExprKind::Bin { .. }));
    }
    // Known 64-bit-or-less operands fold.
    let mut m = model_with(
        vec![assign(
            IrLhs::Whole(0),
            bin(IrBinOp::Div, konst(6, 32), konst(3, 32), 32),
        )],
        sigs(1),
    );
    run(&mut m, &fold_only());
    assert_eq!(const_payload(first_assign_rhs(&m)), Some((2, 32)));
}

#[test]
fn fold_case_equality_is_literal_over_x_z() {
    // === over an X operand yields 0 (unequal), not X.
    let mut m = model_with(
        vec![assign(
            IrLhs::Whole(0),
            bin(IrBinOp::CaseEq, xkonst(8), konst(255, 8), 1),
        )],
        sigs(1),
    );
    run(&mut m, &fold_only());
    assert_eq!(const_payload(first_assign_rhs(&m)), Some((0, 1)));

    let mut m = model_with(
        vec![assign(
            IrLhs::Whole(0),
            bin(IrBinOp::CaseEq, konst(255, 8), konst(255, 8), 1),
        )],
        sigs(1),
    );
    run(&mut m, &fold_only());
    assert_eq!(const_payload(first_assign_rhs(&m)), Some((1, 1)));
}

#[test]
fn fold_logical_equality_honors_known_mismatch_before_unknown_bits() {
    // bit 3 differs while bit 1 is X in both operands, so == is known 0.
    let mut m = model_with(
        vec![assign(
            IrLhs::Whole(0),
            bin(
                IrBinOp::Eq,
                masked_konst(0b1000, 0b0010, 0, 4),
                masked_konst(0, 0b0010, 0, 4),
                1,
            ),
        )],
        sigs(1),
    );
    run(&mut m, &fold_only());
    assert_eq!(const_payload(first_assign_rhs(&m)), Some((0, 1)));

    // With no known mismatch, the unknown bit keeps != unknown.
    let mut m = model_with(
        vec![assign(
            IrLhs::Whole(0),
            bin(
                IrBinOp::Neq,
                masked_konst(0b1000, 0b0010, 0, 4),
                masked_konst(0b1000, 0, 0b0010, 4),
                1,
            ),
        )],
        sigs(1),
    );
    run(&mut m, &fold_only());
    let IrExprKind::Const(value) = &first_assign_rhs(&m).kind else {
        panic!("expected folded constant");
    };
    assert_eq!(value.x[0] & 1, 1);
}

#[test]
fn fold_two_state_conversion_zeros_x_and_z_bits() {
    let converted = IrExpr::to_two_state(masked_konst(0b1111, 0b0010, 0b0100, 4));
    let mut m = model_with(vec![assign(IrLhs::Whole(0), converted)], sigs(1));
    run(&mut m, &fold_only());
    assert_eq!(const_payload(first_assign_rhs(&m)), Some((0b1001, 4)));

    // The conversion is not an identity for a runtime value even though
    // it preserves width and signedness.
    let converted = IrExpr::to_two_state(IrExpr::new(IrExprKind::SigRead(0), 8, false, None));
    let mut m = model_with(vec![assign(IrLhs::Whole(0), converted)], sigs(1));
    run(&mut m, &idents_only());
    assert!(matches!(
        first_assign_rhs(&m).kind,
        IrExprKind::ToTwoState { .. }
    ));
}

#[test]
fn fold_conditional_treats_known_one_with_unknown_as_true() {
    let mux = IrExpr::new(
        IrExprKind::Mux {
            sel: Box::new(masked_konst(0b10, 0b01, 0, 2)),
            a: Box::new(konst(9, 8)),
            b: Box::new(konst(3, 8)),
        },
        8,
        false,
        None,
    );
    let mut m = model_with(vec![assign(IrLhs::Whole(0), mux)], sigs(1));
    run(&mut m, &fold_only());
    assert_eq!(const_payload(first_assign_rhs(&m)), Some((9, 8)));
}

#[test]
fn fold_real_arithmetic_on_literals_only() {
    let mut m = model_with(
        vec![assign(
            IrLhs::Whole(0),
            IrExpr::new(
                IrExprKind::RealBin {
                    op: IrRealBinOp::Add,
                    a: Box::new(rkonst(1.5)),
                    b: Box::new(rkonst(2.0)),
                },
                0,
                true,
                None,
            ),
        )],
        sigs(1),
    );
    run(&mut m, &fold_only());
    match &proc_body(&m)[0] {
        IrStmt::Assign { rhs, .. } => match &rhs.kind {
            IrExprKind::Const(c) => {
                assert_eq!(c.real, Some(3.5));
            }
            other => panic!("expected folded real constant, got {other:?}"),
        },
        other => panic!("expected assignment, got {other:?}"),
    }
}

// ── identities ────────────────────────────────────────────────────────

#[test]
fn identity_double_bitneg_collapses() {
    let inner = konst(5, 4);
    let e = bitneg(bitneg(inner));
    let mut m = model_with(vec![assign(IrLhs::Whole(0), e)], sigs(1));
    run(&mut m, &idents_only());
    assert_eq!(const_payload(first_assign_rhs(&m)), Some((5, 4)));
}

#[test]
fn identity_double_bitneg_preserves_z_semantics() {
    let e = bitneg(bitneg(zkonst(1)));
    let mut m = model_with(vec![assign(IrLhs::Whole(0), e)], sigs(1));
    run(&mut m, &idents_only());
    assert!(matches!(
        first_assign_rhs(&m).kind,
        IrExprKind::Un {
            op: IrUnOp::BitNeg,
            ..
        }
    ));
}

#[test]
fn identity_shift_by_zero_returns_base() {
    let e = bin(IrBinOp::Shl, konst(7, 8), konst(0, 8), 8);
    let mut m = model_with(vec![assign(IrLhs::Whole(0), e)], sigs(1));
    run(&mut m, &idents_only());
    assert_eq!(const_payload(first_assign_rhs(&m)), Some((7, 8)));
}

#[test]
fn identity_single_part_concat() {
    let e = IrExpr::new(
        IrExprKind::Concat {
            parts: vec![konst(9, 8)],
        },
        8,
        false,
        None,
    );
    let mut m = model_with(vec![assign(IrLhs::Whole(0), e)], sigs(1));
    run(&mut m, &idents_only());
    assert_eq!(const_payload(first_assign_rhs(&m)), Some((9, 8)));

    // Two parts stay concatenated.
    let e = IrExpr::new(
        IrExprKind::Concat {
            parts: vec![konst(1, 4), konst(2, 4)],
        },
        8,
        false,
        None,
    );
    let mut m = model_with(vec![assign(IrLhs::Whole(0), e)], sigs(1));
    run(&mut m, &idents_only());
    assert!(matches!(
        &proc_body(&m)[0],
        IrStmt::Assign { rhs: IrExpr { kind: IrExprKind::Concat { parts }, .. }, .. } if parts.len() == 2
    ));
}

#[test]
fn identity_mux_with_known_select_picks_branch() {
    let mk_mux = |sel: IrExpr| {
        IrExpr::new(
            IrExprKind::Mux {
                sel: Box::new(sel),
                a: Box::new(konst(9, 8)),
                b: Box::new(konst(3, 8)),
            },
            8,
            false,
            None,
        )
    };
    // Select 1 → a.
    let mut m = model_with(vec![assign(IrLhs::Whole(0), mk_mux(konst(1, 1)))], sigs(1));
    run(&mut m, &idents_only());
    assert_eq!(const_payload(first_assign_rhs(&m)), Some((9, 8)));
    // Select 0 → b.
    let mut m = model_with(vec![assign(IrLhs::Whole(0), mk_mux(konst(0, 1)))], sigs(1));
    run(&mut m, &idents_only());
    assert_eq!(const_payload(first_assign_rhs(&m)), Some((3, 8)));
    // X select must NOT choose.
    let mut m = model_with(vec![assign(IrLhs::Whole(0), mk_mux(xkonst(1)))], sigs(1));
    run(&mut m, &idents_only());
    assert!(matches!(
        &proc_body(&m)[0],
        IrStmt::Assign {
            rhs: IrExpr {
                kind: IrExprKind::Mux { .. },
                ..
            },
            ..
        }
    ));
}

#[test]
fn identity_resize_same_shape_collapses() {
    let inner = konst(300, 16);
    let e = IrExpr::resize_to(inner, 16, false);
    let mut m = model_with(vec![assign(IrLhs::Whole(0), e)], sigs(1));
    run(&mut m, &idents_only());
    assert_eq!(const_payload(first_assign_rhs(&m)), Some((300, 16)));

    // Different target width keeps the resize.
    let e = IrExpr::resize_to(konst(300, 16), 8, false);
    let mut m = model_with(vec![assign(IrLhs::Whole(0), e)], sigs(1));
    run(&mut m, &idents_only());
    assert!(matches!(
        &proc_body(&m)[0],
        IrStmt::Assign {
            rhs: IrExpr {
                kind: IrExprKind::Resize { .. },
                ..
            },
            ..
        }
    ));
}

// ── prune_branches ────────────────────────────────────────────────────

fn if_stmt(cond: IrExpr, then_: Vec<IrStmt>, els: Option<Vec<IrStmt>>) -> IrStmt {
    IrStmt::If {
        cond,
        then_,
        els,
        check: IrUniquePriorityCheck::None,
    }
}

fn marker(tag: u64) -> IrStmt {
    assign(IrLhs::Whole(0), konst(tag, 8))
}

#[test]
fn prune_if_true_takes_then() {
    let mut m = model_with(
        vec![if_stmt(
            konst(1, 1),
            vec![marker(10)],
            Some(vec![marker(20)]),
        )],
        sigs(1),
    );
    run(&mut m, &prune_only());
    assert_eq!(proc_body(&m).len(), 1);
    assert_eq!(const_payload(first_assign_rhs(&m)), Some((10, 8)));
}

#[test]
fn prune_if_false_takes_else() {
    let mut m = model_with(
        vec![if_stmt(
            konst(0, 1),
            vec![marker(10)],
            Some(vec![marker(20)]),
        )],
        sigs(1),
    );
    run(&mut m, &prune_only());
    assert_eq!(const_payload(first_assign_rhs(&m)), Some((20, 8)));
}

#[test]
fn prune_if_unknown_condition_takes_else_per_runtime_contract() {
    let mut m = model_with(
        vec![if_stmt(xkonst(1), vec![marker(10)], Some(vec![marker(20)]))],
        sigs(1),
    );
    run(&mut m, &prune_only());
    assert_eq!(const_payload(first_assign_rhs(&m)), Some((20, 8)));
}

#[test]
fn real_truthiness_matches_runtime_nan_is_truthy() {
    // The C runtime's llg_real_to_bool is `v != 0.0`: NaN is truthy.
    assert_eq!(truthy_const(&rkonst(f64::NAN)), Some(true));
    assert_eq!(truthy_const(&rkonst(0.0)), Some(false));
    assert_eq!(truthy_const(&rkonst(-1.5)), Some(true));
}

#[test]
fn wide_constant_truthiness_checks_every_limb() {
    let high_bit = IrExpr::new(
        IrExprKind::Const(IrConst {
            bits: vec![0, 1u64 << 36],
            x: vec![0, 0],
            z: vec![0, 0],
            width: 128,
            signed: false,
            real: None,
            fill: None,
        }),
        128,
        false,
        None,
    );
    assert_eq!(truthy_const(&high_bit), Some(true));

    let high_one_with_low_x = IrExpr::new(
        IrExprKind::Const(IrConst {
            bits: vec![0, 1u64 << 36],
            x: vec![1, 0],
            z: vec![0, 0],
            width: 128,
            signed: false,
            real: None,
            fill: None,
        }),
        128,
        false,
        None,
    );
    assert_eq!(truthy_const(&high_one_with_low_x), Some(true));
}

#[test]
fn prune_if_folded_nan_real_takes_then_zero_takes_else() {
    // fold + prune: 0.0/0.0 folds to a NaN real constant, and runtime
    // truthiness (`llg_real_to_bool`) treats NaN as truthy — pruning to
    // the ELSE arm here would diverge from an opt-off run.
    let cfg = OptConfig {
        fold_constants: true,
        identities: false,
        prune_branches: true,
        unused_storage: false,
    };
    let nan_div = IrExpr::new(
        IrExprKind::RealBin {
            op: IrRealBinOp::Div,
            a: Box::new(rkonst(0.0)),
            b: Box::new(rkonst(0.0)),
        },
        0,
        true,
        None,
    );
    let mut m = model_with(
        vec![if_stmt(nan_div, vec![marker(10)], Some(vec![marker(20)]))],
        sigs(1),
    );
    run(&mut m, &cfg);
    assert_eq!(
        const_payload(first_assign_rhs(&m)),
        Some((10, 8)),
        "a NaN condition must keep the THEN arm"
    );

    // A folded zero real takes the ELSE arm.
    let zero_mul = IrExpr::new(
        IrExprKind::RealBin {
            op: IrRealBinOp::Mul,
            a: Box::new(rkonst(5.0)),
            b: Box::new(rkonst(0.0)),
        },
        0,
        true,
        None,
    );
    let mut m = model_with(
        vec![if_stmt(zero_mul, vec![marker(10)], Some(vec![marker(20)]))],
        sigs(1),
    );
    run(&mut m, &cfg);
    assert_eq!(const_payload(first_assign_rhs(&m)), Some((20, 8)));
}

#[test]
fn prune_while_false_becomes_empty_block() {
    let mut m = model_with(
        vec![IrStmt::While {
            cond: konst(0, 1),
            body: vec![marker(10)],
        }],
        sigs(1),
    );
    run(&mut m, &prune_only());
    assert!(matches!(proc_body(&m)[0], IrStmt::Block(ref b) if b.is_empty()));
}

#[test]
fn prune_wait_cond_true_splices_body_false_stays() {
    let wait_true = IrStmt::WaitCond {
        cond: konst(1, 1),
        sens: vec![IrDependency::scalar("G_s0")],
        body: vec![marker(42)],
    };
    let mut m = model_with(vec![wait_true], sigs(1));
    run(&mut m, &prune_only());
    assert_eq!(
        const_payload(first_assign_rhs_of(single_pruned_stmt(&m))),
        Some((42, 8))
    );

    // A false constant condition STAYS: zero-delay guard spin semantics.
    let wait_false = IrStmt::WaitCond {
        cond: konst(0, 1),
        sens: vec![],
        body: vec![marker(42)],
    };
    let mut m = model_with(vec![wait_false], sigs(1));
    run(&mut m, &prune_only());
    assert!(matches!(proc_body(&m)[0], IrStmt::WaitCond { .. }));
}

#[test]
fn prune_case_picks_matching_arm_and_respects_order() {
    let case = |items: Vec<IrCaseItem>| IrStmt::Case {
        sel: konst(2, 4),
        kind: IrCaseKind::Exact,
        items,
        check: IrUniquePriorityCheck::None,
    };
    let item = |vals: &[u64], tag: u64| IrCaseItem {
        exprs: vals.iter().map(|v| konst(*v, 4)).collect(),
        body: vec![marker(tag)],
    };
    // Matching item wins over later default.
    let mut m = model_with(
        vec![case(vec![item(&[8], 10), item(&[2], 20), item(&[], 30)])],
        sigs(1),
    );
    run(&mut m, &prune_only());
    assert_eq!(
        const_payload(first_assign_rhs_of(single_pruned_stmt(&m))),
        Some((20, 8))
    );

    // An earlier non-constant item could match: leave the case alone.
    let blocking = IrCaseItem {
        exprs: vec![IrExpr::new(IrExprKind::SigRead(0), 4, false, None)],
        body: vec![marker(99)],
    };
    let mut m = model_with(vec![case(vec![blocking, item(&[2], 20)])], sigs(1));
    run(&mut m, &prune_only());
    assert!(matches!(proc_body(&m)[0], IrStmt::Case { .. }));

    // All items unmatched, default present → default body.
    let mut m = model_with(vec![case(vec![item(&[8], 10), item(&[], 30)])], sigs(1));
    run(&mut m, &prune_only());
    assert_eq!(
        const_payload(first_assign_rhs_of(single_pruned_stmt(&m))),
        Some((30, 8))
    );
}

#[test]
fn prune_keeps_qualified_case_for_runtime_diagnostics() {
    let item = |value: u64, tag: u64| IrCaseItem {
        exprs: vec![konst(value, 4)],
        body: vec![marker(tag)],
    };
    let mut m = model_with(
        vec![IrStmt::Case {
            sel: konst(2, 4),
            kind: IrCaseKind::Exact,
            items: vec![item(2, 10), item(2, 20)],
            check: IrUniquePriorityCheck::Unique(crate::sim::semantic::Origin::Synthetic {
                reason: "qualified case test".to_owned(),
            }),
        }],
        sigs(1),
    );
    run(&mut m, &prune_only());
    assert!(matches!(
        proc_body(&m)[0],
        IrStmt::Case {
            check: IrUniquePriorityCheck::Unique(_),
            ..
        }
    ));
}

// ── orphaned-label strip (post-prune) ─────────────────────────────────

#[test]
fn prune_strips_label_whose_only_goto_was_pruned() {
    // `if (0) break;` prunes the then-arm carrying the only goto to
    // `_bk1`; the trailing label must not survive as dead C.
    let mut m = model_with(
        vec![
            if_stmt(konst(0, 1), vec![IrStmt::Goto("_bk1".to_string())], None),
            marker(10),
            IrStmt::Label("_bk1".to_string()),
        ],
        sigs(1),
    );
    run(&mut m, &prune_only());
    let body = proc_body(&m);
    assert_eq!(body.len(), 1, "label stripped with its pruned goto");
    assert_eq!(const_payload(first_assign_rhs_of(&body[0])), Some((10, 8)));
}

#[test]
fn prune_keeps_labels_that_are_still_goto_targets() {
    // A live goto (unknown condition) keeps its target; a DIFFERENT
    // unreferenced label in the same tree still goes away.  Nested
    // definitions are covered too.
    let mut m = model_with(
        vec![
            if_stmt(
                IrExpr::new(IrExprKind::SigRead(0), 1, false, None),
                vec![IrStmt::Goto("_ct1".to_string())],
                None,
            ),
            IrStmt::Block(vec![
                IrStmt::Label("_ct1".to_string()),
                IrStmt::Label("_xb9".to_string()),
                marker(11),
            ]),
            IrStmt::Label("_bk2".to_string()),
            IrStmt::Goto("_bk2".to_string()),
        ],
        sigs(1),
    );
    run(&mut m, &prune_only());
    let labels: Vec<&str> = collect_all_labels(proc_body(&m));
    assert!(labels.contains(&"_ct1"), "live target kept");
    assert!(labels.contains(&"_bk2"), "goto-referenced label kept");
    assert!(
        !labels.contains(&"_xb9"),
        "unreferenced nested label stripped"
    );
}

fn collect_all_labels(stmts: &[IrStmt]) -> Vec<&str> {
    let mut out = Vec::new();
    fn walk<'a>(stmts: &'a [IrStmt], out: &mut Vec<&'a str>) {
        for s in stmts {
            match s {
                IrStmt::Label(l) => out.push(l.as_str()),
                IrStmt::Goto(l) => out.push(l.as_str()),
                IrStmt::Block(b) | IrStmt::Forever { body: b } => walk(b, out),
                IrStmt::If { then_, els, .. } => {
                    walk(then_, out);
                    if let Some(els) = els {
                        walk(els, out);
                    }
                }
                _ => {}
            }
        }
    }
    walk(stmts, &mut out);
    out
}

// ── unused_storage ────────────────────────────────────────────────────

#[test]
fn unused_storage_omits_only_fully_dead_signals() {
    let body = vec![
        // Writes s1, reads s2.
        assign(IrLhs::Whole(1), konst(1, 8)),
        assign(
            IrLhs::Whole(2),
            IrExpr::new(IrExprKind::SigRead(1), 8, false, None),
        ),
    ];
    let mut signals = sigs(4);
    signals[3].c_name = "G_s3".to_string();
    let mut m = model_with(body, signals);
    // s3 is read by a combinational sensitivity list.
    m.processes[0].shape = IrShape::SensLoop {
        reads: vec![IrDependency::scalar("G_s3")],
    };
    run(&mut m, &storage_only());
    assert!(m.signals[0].omit, "untouched signal is omitted");
    assert!(!m.signals[1].omit, "written signal stays");
    assert!(!m.signals[2].omit, "read signal stays");
    assert!(!m.signals[3].omit, "sensitivity-read signal stays");
}

#[test]
fn unused_storage_counts_function_local_initializer_reads() {
    let mut local = IrLocal::new("local".to_string(), 8, false).expect("valid local");
    local.initial = Some(IrExpr::new(IrExprKind::SigRead(0), 8, false, None));
    let mut m = model_with(Vec::new(), sigs(2));
    m.funcs.push(IrFunc::new(
        "f".to_string(),
        None,
        Vec::new(),
        vec![local],
        Vec::new(),
        Vec::new(),
    ));

    run(&mut m, &storage_only());

    assert!(!m.signals[0].omit, "initializer-read signal stays");
    assert!(m.signals[1].omit, "unreferenced signal is omitted");
}

#[test]
fn waveform_keeps_user_storage_but_not_synthesized_storage() {
    let mut signals = sigs(2);
    signals[1].hdl_name = None;
    let mut m = model_with(Vec::new(), signals);
    m.waveform = true;

    run(&mut m, &storage_only());

    assert!(!m.signals[0].omit, "waveform-visible user signal stays");
    assert!(
        m.signals[1].omit,
        "unreferenced synthesized signal is omitted"
    );
}

#[test]
fn unused_storage_keeps_concurrent_assertion_sources() {
    let mut m = model_with(Vec::new(), sigs(3));
    m.assertions.push(IrAssertion::new(
        7,
        "tb".to_string(),
        "a".to_string(),
        "tb:1".to_string(),
        IrConcurrentAssertionKind::Assert,
        0,
        true,
        Some(1),
        Some(IrExpr::new(IrExprKind::SigRead(1), 8, false, None)),
        IrExpr::new(IrExprKind::SigRead(2), 8, false, None),
        false,
        None,
        None,
    ));

    run(&mut m, &storage_only());

    assert!(!m.signals[0].omit, "assertion clock stays");
    assert!(!m.signals[1].omit, "assertion disable signal stays");
    assert!(!m.signals[2].omit, "assertion predicate signal stays");
}

#[test]
fn unused_storage_keeps_wait_event_sources() {
    // s1 appears ONLY in an @(posedge …) or-list nested in a block; s2
    // only in a wait_any sensitivity set under an if arm.  Both keep
    // their declarations: emission references their globals in the wait
    // arrays (`&G_s1`), so omitting them breaks the generated C.
    let body = vec![
        assign(IrLhs::Whole(0), konst(1, 8)),
        IrStmt::Block(vec![IrStmt::WaitEvents {
            specs: vec![(IrWaitSrc::Sig("G_s1".to_string()), IrEdge::Posedge)],
        }]),
        IrStmt::If {
            cond: IrExpr::new(IrExprKind::SigRead(0), 8, false, None),
            then_: vec![IrStmt::WaitAny {
                sens: vec![IrDependency::scalar("G_s2")],
            }],
            els: None,
            check: IrUniquePriorityCheck::None,
        },
    ];
    let mut m = model_with(body, sigs(3));
    run(&mut m, &storage_only());
    assert!(!m.signals[0].omit, "written signal stays");
    assert!(!m.signals[1].omit, "event-source-only signal stays");
    assert!(!m.signals[2].omit, "wait-any-source-only signal stays");
}

#[test]
fn unused_storage_handles_mixed_event_wait_sources() {
    // A mixed @(a or ev) wait keeps the signal half's storage (its
    // address is emitted into the wait table); the named-event half is
    // not storage — it must be skipped without matching any signal name.
    let body = vec![IrStmt::WaitEvents {
        specs: vec![
            (IrWaitSrc::Sig("G_s1".to_string()), IrEdge::Any),
            (IrWaitSrc::Event(IrEventRef::Static(0)), IrEdge::Any),
        ],
    }];
    let mut m = model_with(body, sigs(3));
    m.events.push(crate::sim::ir::IrEvent {
        c_name: "E_tb_ev".to_string(),
        array_dims: None,
        array_elements: Vec::new(),
    });
    run(&mut m, &storage_only());
    assert!(m.signals[0].omit, "untouched signal is still omitted");
    assert!(
        !m.signals[1].omit,
        "mixed-list signal source keeps its storage"
    );
    assert!(m.signals[2].omit, "untouched signal is still omitted");
}

#[test]
fn unused_storage_keeps_wait_cond_sens_only_signal() {
    // s3 is named only in the `wait (cond)` sensitivity list — the
    // condition itself is a constant and never reads it.  Nested inside
    // a while body to cover non-toplevel statement positions.
    let body = vec![IrStmt::While {
        cond: konst(1, 1),
        body: vec![IrStmt::WaitCond {
            cond: konst(0, 1),
            sens: vec![IrDependency::scalar("G_s3")],
            body: vec![marker(42)],
        }],
    }];
    let mut m = model_with(body, sigs(4));
    run(&mut m, &storage_only());
    assert!(!m.signals[3].omit, "wait-cond sens-only signal stays");
    assert!(m.signals[1].omit && m.signals[2].omit, "dead ones go");
}

#[test]
fn unused_storage_keeps_wait_sources_in_fork_branches() {
    // Fork-branch coroutine bodies live on the process's pre_fns; their
    // wait sources are reads too.
    let mut m = model_with(vec![assign(IrLhs::Whole(0), konst(1, 8))], sigs(4));
    m.processes[0].pre_fns = vec![IrPreFn::Branch {
        c_name: "p_t_b0".to_string(),
        body: vec![
            IrStmt::WaitEvents {
                specs: vec![(IrWaitSrc::Sig("G_s1".to_string()), IrEdge::Any)],
            },
            IrStmt::WaitAny {
                sens: vec![IrDependency::scalar("G_s2"), IrDependency::scalar("G_s3")],
            },
        ],
    }];
    run(&mut m, &storage_only());
    assert!(!m.signals[0].omit);
    assert!(!m.signals[1].omit, "fork-branch event source stays");
    assert!(!m.signals[2].omit, "fork-branch wait-any source stays");
    assert!(!m.signals[3].omit, "fork-branch wait-any source stays");
}

#[test]
fn unused_storage_counts_call_temp_inits_and_copyouts() {
    // Statement-position task calls keep temp inits and copy-out targets
    // INSIDE the Call node: the temp init reads s2, the copy-out writes
    // s3 (its bit index is also a read slot), and the Val/OutAddr args
    // read+write s4/s1.
    let call = IrStmt::Call(IrCall {
        f: 0,
        args: vec![
            IrCallArg::Val(IrExpr::new(IrExprKind::SigRead(4), 8, false, None)),
            IrCallArg::OutAddr("&G_s1".to_string()),
        ],
        depth: IrDepth::PROC,
        receiver: None,
        virtual_dispatch: false,
        virtual_call: None,
        temps: vec![(
            "_a0".to_string(),
            0,
            Some(IrExpr::new(IrExprKind::SigRead(2), 8, false, None)),
        )],
        copyouts: vec![(
            IrLhs::Bit(3, konst(0, 3), false),
            "_a0".to_string(),
            8,
            false,
        )],
    });
    let mut m = model_with(vec![call], sigs(5));
    run(&mut m, &storage_only());
    assert!(!m.signals[1].omit, "OutAddr actual stays (read + write)");
    assert!(!m.signals[2].omit, "temp-init read stays");
    assert!(!m.signals[3].omit, "copy-out target stays");
    assert!(!m.signals[4].omit, "Val argument read stays");
    assert!(m.signals[0].omit, "untouched signal is omitted");
}

// ── shortreal cast folding ────────────────────────────────────────────

#[test]
fn fold_shortreal_cast_rounds_through_f32_like_the_runtime() {
    // 16777217 = 2^24 + 1 needs 25 mantissa bits: the runtime cast
    // renders `(double)(float)(x)` and C float rounding drops it to
    // 2^24, so the fold must produce exactly that value (a full-precision
    // fold would make downstream real math diverge from opt-off runs).
    let cast = |shortreal: bool| {
        IrExpr::new(
            IrExprKind::CastToReal {
                a: Box::new(konst(16_777_217, 32)),
                shortreal,
            },
            0,
            true,
            None,
        )
    };
    let mut m = model_with(vec![assign(IrLhs::Whole(0), cast(true))], sigs(1));
    run(&mut m, &fold_only());
    match &first_assign_rhs(&m).kind {
        IrExprKind::Const(c) => {
            assert_eq!(c.real, Some(16_777_216.0f64));
        }
        other => panic!("expected folded real constant, got {other:?}"),
    }

    // A plain real cast keeps full double precision.
    let mut m = model_with(vec![assign(IrLhs::Whole(0), cast(false))], sigs(1));
    run(&mut m, &fold_only());
    match &first_assign_rhs(&m).kind {
        IrExprKind::Const(c) => {
            assert_eq!(c.real, Some(16_777_217.0f64));
        }
        other => panic!("expected folded real constant, got {other:?}"),
    }
}

// ── case pruning provability ──────────────────────────────────────────

fn case_stmt(sel: IrExpr, kind: IrCaseKind, items: Vec<IrCaseItem>) -> IrStmt {
    IrStmt::Case {
        sel,
        kind,
        items,
        check: IrUniquePriorityCheck::None,
    }
}

fn citem(vals: &[u64], tag: u64) -> IrCaseItem {
    IrCaseItem {
        exprs: vals.iter().map(|v| konst(*v, 4)).collect(),
        body: vec![marker(tag)],
    }
}

/// A constant item with Z wildcard bits (`4'b1z0z` for bits=8, z=3).
fn czitem(bits: u64, z: u64, tag: u64) -> IrCaseItem {
    IrCaseItem {
        exprs: vec![IrExpr::new(
            IrExprKind::Const(IrConst {
                bits: vec![bits],
                x: vec![0],
                z: vec![z],
                width: 4,
                signed: false,
                real: None,
                fill: None,
            }),
            4,
            false,
            None,
        )],
        body: vec![marker(tag)],
    }
}

fn def_item(tag: u64) -> IrCaseItem {
    IrCaseItem {
        exprs: vec![],
        body: vec![marker(tag)],
    }
}

#[test]
fn prune_case_nonconst_item_blocks_default_pruning() {
    // [const-unmatched, DEFAULT, non-const]: the non-const item could
    // still match at runtime, so nothing about the default is provable.
    let items = vec![
        citem(&[8], 10),
        def_item(30),
        IrCaseItem {
            exprs: vec![IrExpr::new(IrExprKind::SigRead(0), 4, false, None)],
            body: vec![marker(99)],
        },
    ];
    let mut m = model_with(
        vec![case_stmt(konst(2, 4), IrCaseKind::Exact, items)],
        sigs(1),
    );
    run(&mut m, &prune_only());
    assert!(
        matches!(proc_body(&m)[0], IrStmt::Case { .. }),
        "a remaining non-const item must block default pruning"
    );
}

#[test]
fn prune_case_default_arm_blocks_later_items() {
    // A default arm can never be proven unmatched, so a matching item
    // after it cannot be proven reachable either.
    let mut m = model_with(
        vec![case_stmt(
            konst(2, 4),
            IrCaseKind::Exact,
            vec![def_item(30), citem(&[2], 20)],
        )],
        sigs(1),
    );
    run(&mut m, &prune_only());
    assert!(matches!(proc_body(&m)[0], IrStmt::Case { .. }));
}

#[test]
fn prune_casez_wildcard_item_vs_known_selector() {
    let sel = konst(0b1000, 4);
    // LRM 12.5.1: z item bits are wildcards against a known selector —
    // 4'b1000 ==='z' 4'b1z0z matches → prunes to that item.
    let mut m = model_with(
        vec![case_stmt(
            sel.clone(),
            IrCaseKind::Casez,
            vec![
                citem(&[0b0101], 10),
                czitem(0b1000, 0b0011, 20),
                def_item(30),
            ],
        )],
        sigs(1),
    );
    run(&mut m, &prune_only());
    assert_eq!(
        const_payload(first_assign_rhs_of(single_pruned_stmt(&m))),
        Some((20, 8))
    );

    // A known-bit mismatch dominates: 4'b1110 does not match 4'b1000
    // under casez → all items unmatched → default.
    let mut m = model_with(
        vec![case_stmt(
            sel,
            IrCaseKind::Casez,
            vec![czitem(0b1110, 0, 40), def_item(30)],
        )],
        sigs(1),
    );
    run(&mut m, &prune_only());
    assert_eq!(
        const_payload(first_assign_rhs_of(single_pruned_stmt(&m))),
        Some((30, 8))
    );
}

// ── identity guards / interleave ──────────────────────────────────────

#[test]
fn identity_mux_narrower_branch_keeps_the_mux() {
    // The selected branch is narrower than the mux node; Verilog widens
    // ?: to max(branch widths), so a narrower branch may NOT stand in.
    let e = IrExpr::new(
        IrExprKind::Mux {
            sel: Box::new(konst(0, 1)),
            a: Box::new(konst(9, 8)),
            b: Box::new(konst(3, 4)),
        },
        8,
        false,
        None,
    );
    let mut m = model_with(vec![assign(IrLhs::Whole(0), e)], sigs(1));
    run(&mut m, &idents_only());
    assert!(matches!(
        &proc_body(&m)[0],
        IrStmt::Assign {
            rhs: IrExpr {
                kind: IrExprKind::Mux { .. },
                ..
            },
            ..
        }
    ));
}

#[test]
fn fold_pow_mod_guards_stay_symbolic() {
    // Unknown operand: stays symbolic.
    for op in [IrBinOp::Pow, IrBinOp::Mod] {
        let mut m = model_with(
            vec![assign(
                IrLhs::Whole(0),
                bin(op, konst(6, 32), xkonst(32), 32),
            )],
            sigs(1),
        );
        run(&mut m, &fold_only());
        assert!(
            matches!(first_assign_rhs(&m).kind, IrExprKind::Bin { .. }),
            "{op:?} with an unknown operand must stay symbolic"
        );
        // Fully known but >64-bit operands stay runtime calls too.
        let wide = konst(6, 128);
        let mut m = model_with(
            vec![assign(IrLhs::Whole(0), bin(op, wide, konst(3, 128), 128))],
            sigs(1),
        );
        run(&mut m, &fold_only());
        assert!(
            matches!(first_assign_rhs(&m).kind, IrExprKind::Bin { .. }),
            "{op:?} over 128-bit operands must stay symbolic"
        );
    }
    // Known narrow operands fold.
    let mut m = model_with(
        vec![assign(
            IrLhs::Whole(0),
            bin(IrBinOp::Pow, konst(3, 32), konst(3, 32), 32),
        )],
        sigs(1),
    );
    run(&mut m, &fold_only());
    assert_eq!(const_payload(first_assign_rhs(&m)), Some((27, 32)));
    let mut m = model_with(
        vec![assign(
            IrLhs::Whole(0),
            bin(IrBinOp::Mod, konst(7, 32), konst(3, 32), 32),
        )],
        sigs(1),
    );
    run(&mut m, &fold_only());
    assert_eq!(const_payload(first_assign_rhs(&m)), Some((1, 32)));
}

#[test]
fn fold_and_identities_interleave_to_fixpoint() {
    // Folding produces the constant zero that the shift-by-zero identity
    // needs; identities alone see an unfoldable Sub and do nothing.
    let shift = || {
        IrExpr::new(
            IrExprKind::Bin {
                op: IrBinOp::Shl,
                a: Box::new(IrExpr::new(IrExprKind::SigRead(0), 8, false, None)),
                b: Box::new(bin(IrBinOp::Sub, konst(4, 8), konst(4, 8), 8)),
            },
            8,
            false,
            None,
        )
    };
    let mut m = model_with(vec![assign(IrLhs::Whole(0), shift())], sigs(1));
    run(&mut m, &idents_only());
    assert!(matches!(first_assign_rhs(&m).kind, IrExprKind::Bin { .. }));

    // With both passes on, fold exposes the zero, the identity replaces
    // the shift, and the result survives the second round untouched.
    let both = OptConfig {
        fold_constants: true,
        identities: true,
        prune_branches: false,
        unused_storage: false,
    };
    let mut m = model_with(vec![assign(IrLhs::Whole(0), shift())], sigs(1));
    run(&mut m, &both);
    assert!(matches!(first_assign_rhs(&m).kind, IrExprKind::SigRead(0)));
}
