//! Stable receiver coordinates shared by call copy-back and fixed ordering.

use super::*;
use crate::sim::ir::{IrCallExpr, IrDepth, IrPackedSelect};

fn selector(function: usize) -> IrExpr {
    IrExpr::new(
        IrExprKind::CallFn(Box::new(IrCallExpr::new(
            function,
            Vec::new(),
            IrDepth::FUNC,
            false,
        ))),
        32,
        true,
        None,
    )
}

fn read_path<'a>(value: &'a IrExpr, indices: &mut Vec<String>) -> &'a IrExprKind {
    match &value.kind {
        IrExprKind::Resize { a } | IrExprKind::ToTwoState { a } => read_path(a, indices),
        IrExprKind::IdxPartSel { base, base_idx, .. } => {
            let root = read_path(base, indices);
            let IrExprKind::LocalRead(name) = &base_idx.kind else {
                panic!("selector must be a captured local, not an executable expression");
            };
            indices.push(name.clone());
            root
        }
        kind @ (IrExprKind::LocalRead(_) | IrExprKind::FormalRead(_)) => kind,
        other => panic!("unexpected receiver read: {other:?}"),
    }
}

#[test]
fn frozen_activation_receivers_share_one_ordered_selector_capture() {
    let db = Db::from_test_nodes("receiver", Vec::new(), Vec::new(), HashMap::new()).unwrap();
    let semantic = crate::sim::semantic::SemanticModel::from_db(&db);
    let cg = Codegen::new(&semantic);
    let roots = [
        IrLhs::WholeRef {
            addr: "&matrix".to_owned(),
            width: 192,
            signed: false,
            two_state: false,
            shortreal: false,
        },
        IrLhs::WholeRef {
            addr: "&a0".to_owned(),
            width: 192,
            signed: false,
            two_state: false,
            shortreal: false,
        },
        IrLhs::WholeRef {
            addr: "o0".to_owned(),
            width: 192,
            signed: false,
            two_state: false,
            shortreal: false,
        },
        IrLhs::Ref {
            addr: "r0".to_owned(),
            width: 192,
            signed: false,
            two_state: false,
            const_ref: false,
            bit: None,
        },
    ];
    for root in roots {
        let target = IrLhs::PackedSelect {
            target: Box::new(root.clone()),
            steps: vec![
                IrPackedSelect {
                    base: selector(0),
                    width: 96,
                },
                IrPackedSelect {
                    base: selector(1),
                    width: 24,
                },
            ],
            signed: false,
            two_state: false,
        };
        let mut sequence = 0;
        let mut captures = Vec::new();
        let (target, source) = cg
            .freeze_call_lhs(target, "ordering", &mut sequence, &mut captures)
            .unwrap();
        assert_eq!(sequence, 2);
        assert_eq!(captures.len(), 2);
        for (index, (_, width, signed, two_state, init)) in captures.iter().enumerate() {
            assert_eq!((*width, *signed, *two_state), (32, true, false));
            let IrExprKind::CallFn(call) = &init.kind else {
                panic!("the original selector belongs only in its capture initializer");
            };
            assert_eq!(call.f, index);
        }
        let IrLhs::PackedSelect { target, steps, .. } = target else {
            panic!("the selected destination must retain its activation root");
        };
        assert_eq!(*target, root);
        let mut read_indices = Vec::new();
        let source = source.unwrap();
        let read_root = read_path(&source, &mut read_indices);
        let expected_root = match &root {
            IrLhs::WholeRef { addr, .. } if addr.starts_with('&') => {
                IrExprKind::LocalRead(addr[1..].to_owned())
            }
            _ => IrExprKind::FormalRead(0),
        };
        assert_eq!(read_root, &expected_root);
        for (index, step) in steps.iter().enumerate() {
            assert_eq!(
                step.base.kind,
                IrExprKind::LocalRead(captures[index].0.clone())
            );
            assert_eq!(read_indices[index], captures[index].0);
        }
        // Cloning the read/target for a comparison or swap cannot invoke the
        // original selectors; the root remains a live read rather than a copy.
        assert_eq!(read_indices.len(), captures.len());
    }
}

#[test]
fn receiver_capture_rejects_a_real_selector_instead_of_relowering_it() {
    let db = Db::from_test_nodes("receiver", Vec::new(), Vec::new(), HashMap::new()).unwrap();
    let semantic = crate::sim::semantic::SemanticModel::from_db(&db);
    let cg = Codegen::new(&semantic);
    let target = IrLhs::PackedSelect {
        target: Box::new(IrLhs::WholeRef {
            addr: "&matrix".to_owned(),
            width: 48,
            signed: false,
            two_state: false,
            shortreal: false,
        }),
        steps: vec![IrPackedSelect {
            base: IrExpr::new(IrExprKind::FormalRead(0), 0, false, None),
            width: 24,
        }],
        signed: false,
        two_state: false,
    };
    let error = cg
        .freeze_call_lhs(target, "ordering", &mut 0, &mut Vec::new())
        .unwrap_err();
    assert!(error.contains("selector must be integral"), "{error}");
}
