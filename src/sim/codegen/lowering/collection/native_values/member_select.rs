//! Run-time indices into fixed-array members of native records (SIM-007).
//!
//! A module native record keeps each element of a member array as its own
//! leaf, and a native subroutine value addresses each element by a constant
//! item path. A run-time index (`r.s[k]`) therefore selects one leaf by
//! comparing the index with every declared index; an index that matches
//! none (out of range, X or Z) reads the element default and writes nothing
//! (SV 7.4.6). The comparison chain is bounded by
//! [`NATIVE_MEMBER_SELECT_LIMIT`] elements. An index with side effects is
//! evaluated once into a local before the chain.

use super::*;
use crate::sim::ir::{IrElementRef, IrElementRefKind, IrElementRoot, IrLeafTarget};

/// Largest member array selected by a run-time index. Each element is one
/// comparison, so larger arrays are rejected rather than expanded.
pub(in crate::sim::codegen) const NATIVE_MEMBER_SELECT_LIMIT: usize = 64;

/// The elements of a member array selected by a run-time index.
pub(in crate::sim::codegen) struct NativeMemberSelect {
    /// Declared index and leaf of every element, in declaration order.
    elements: Vec<(i32, NativeEndpointLeaf)>,
    index: NodeId,
    /// The index has side effects, so the chain compares a local copy.
    hoist: bool,
}

impl Codegen<'_> {
    /// `r.a[k]` with a non-constant `k`, where `r.a` is a one-dimensional
    /// array member of scalar elements in a native record.
    pub(in crate::sim::codegen) fn native_member_select(
        &self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<NativeMemberSelect>, String> {
        // `r.e[k].s`: a member path below a run-time selected record element
        // of a member array selects that member of every element.
        let (select, trailing) = match self.kind(node) {
            NodeKind::Expr(ExprKind::HierPath { parts, refs }) => match refs.first() {
                Some(Some(first))
                    if matches!(
                        self.kind(*first),
                        NodeKind::Expr(ExprKind::ArraySelect { .. })
                    ) =>
                {
                    let trailing = parts
                        .iter()
                        .skip(1)
                        .filter(|part| !part.is_empty())
                        .cloned()
                        .map(AggregatePathPart::Member)
                        .collect::<Vec<_>>();
                    (*first, trailing)
                }
                _ => return Ok(None),
            },
            _ => (node, Vec::new()),
        };
        let (base, index) = match self.kind(select) {
            NodeKind::Expr(ExprKind::ArraySelect { base, indices }) if indices.len() == 1 => {
                (*base, indices[0])
            }
            NodeKind::Expr(ExprKind::BitSelect { base, index }) => (*base, *index),
            _ => return Ok(None),
        };
        if self.eval_bound_i128(index).is_ok() {
            return Ok(None);
        }
        let node = select;
        let select_path = self.db.array_select_path(node);
        let value_owned = match &select_path {
            Some((owner, _)) => self.native_roots.contains_key(owner),
            None => self.native_path_of(base)?.is_some(),
        };
        let module_prefix = match select_path {
            _ if value_owned => None,
            Some((owner, members)) if self.unpacked_aggregates.contains_key(&owner) => Some((
                owner,
                members
                    .iter()
                    .cloned()
                    .map(AggregatePathPart::Member)
                    .collect::<Vec<_>>(),
            )),
            _ => self.unpacked_path_for_expr(base),
        };
        let elements: Vec<(i32, NativeEndpointLeaf)> = if let Some((root, prefix)) = module_prefix {
            if Self::fixed_descriptor_width_bits(
                self.query_descriptor(root)
                    .ok_or("native record has no type")?,
            )
            .is_some()
            {
                return Ok(None);
            }
            let Some(storage) = self.unpacked_aggregates.get(&root) else {
                return Ok(None);
            };
            element_leaves(&prefix, &trailing, storage.leaves.iter(), |leaf| &leaf.path)
                .map(|(index, leaf)| (index, NativeEndpointLeaf::Module(Box::new(leaf.clone()))))
                .collect()
        } else {
            let rooted = match self.db.array_select_path(node) {
                Some((owner, members)) => self.native_roots.get(&owner).map(|value| {
                    (
                        *value,
                        members
                            .iter()
                            .cloned()
                            .map(AggregatePathPart::Member)
                            .collect::<Vec<_>>(),
                    )
                }),
                None => self.native_path_of(base)?,
            };
            let Some((value, prefix)) = rooted else {
                return Ok(None);
            };
            let layout = self.native_layout_of_value(value)?;
            element_leaves(&prefix, &trailing, layout.leaves.iter(), |leaf| &leaf.path)
                .map(|(index, leaf)| (index, NativeEndpointLeaf::Value(value, leaf.clone())))
                .collect()
        };
        if elements.is_empty() {
            return Ok(None);
        }
        if elements.len() > NATIVE_MEMBER_SELECT_LIMIT {
            return Err(format!(
                "run-time index into a native record member array of {} elements in `{path}` is not supported; at most {NATIVE_MEMBER_SELECT_LIMIT} are",
                elements.len()
            ));
        }
        let hoist = !self.side_effect_free(index);
        Ok(Some(NativeMemberSelect {
            elements,
            index,
            hoist,
        }))
    }

    /// `ref` actual `r.a[k]` of a module-like record: its leaves are stable
    /// storage, so the formal takes the one the index selects at the call.
    pub(in crate::sim::codegen::lowering) fn module_leaf_ref(
        &mut self,
        path: &str,
        node: NodeId,
        kind: IrElementRefKind,
        const_ref: bool,
    ) -> Result<Option<IrCallArg>, String> {
        let Some(select) = self.native_member_select(path, node)? else {
            return Ok(None);
        };
        let mut leaves = Vec::with_capacity(select.elements.len());
        for (declared, leaf) in &select.elements {
            let NativeEndpointLeaf::Module(leaf) = leaf else {
                return Ok(None);
            };
            let target = match (kind, leaf.object) {
                (IrElementRefKind::String | IrElementRefKind::Handle, Some(object)) => {
                    let object = &self.model.objects[self.reference_object(object)];
                    let selects_kind = match object.ty {
                        IrObjectType::String => kind == IrElementRefKind::String,
                        IrObjectType::Process => false,
                        _ => kind == IrElementRefKind::Handle,
                    };
                    if !selects_kind {
                        return Ok(None);
                    }
                    IrLeafTarget::Address(format!("&{}", object.c_name))
                }
                (IrElementRefKind::Packed { .. }, None) => IrLeafTarget::Packed {
                    lhs: self.aggregate_leaf_lhs(leaf)?,
                    read: self.aggregate_leaf_read(leaf)?,
                },
                _ => return Ok(None),
            };
            leaves.push((*declared, target));
        }
        let index = self.lower_container_index(path, select.index)?;
        let width = index.width.max(32) + 1;
        Ok(Some(IrCallArg::ElementRef(Box::new(IrElementRef {
            root: IrElementRoot::Leaves {
                index: IrExpr::convert_to(index, width, true),
                leaves,
            },
            steps: Vec::new(),
            kind,
            const_ref,
        }))))
    }

    /// One known test per element: the index equals its declared index.
    /// A side-effecting index is first stored in a local, declared by the
    /// statement pushed to `prelude`.
    fn member_select_tests(
        &mut self,
        path: &str,
        select: &NativeMemberSelect,
        prelude: &mut Vec<IrStmt>,
    ) -> Result<Vec<(IrExpr, NativeEndpointLeaf)>, String> {
        let index = self.lower_container_index(path, select.index)?;
        let width = index.width.max(32) + 1;
        let mut index = IrExpr::convert_to(index, width, true);
        if select.hoist {
            let name = format!("_llg_member_index_{}", self.native_copy_sequence);
            self.native_copy_sequence += 1;
            prelude.push(IrStmt::DeclLocal {
                name: name.clone(),
                width,
                signed: true,
                two_state: false,
                init: Some(Box::new(index)),
            });
            index = IrExpr::new(IrExprKind::LocalRead(name), width, true, None);
        }
        Ok(select
            .elements
            .iter()
            .map(|(declared, leaf)| {
                let declared = crate::sim::codegen::lowering::containers::pattern_key_expr(
                    i128::from(*declared),
                    width,
                    true,
                    false,
                );
                (
                    cmp_expr_ir(IrBinOp::CaseEq, index.clone(), declared),
                    leaf.clone(),
                )
            })
            .collect())
    }

    /// The selected element as a string, packed, real or handle value; an
    /// unmatched index reads the element type's default.
    pub(in crate::sim::codegen) fn native_member_select_read(
        &mut self,
        path: &str,
        select: &NativeMemberSelect,
    ) -> Result<IrNativeLeafExpr, String> {
        let mut prelude = Vec::new();
        let tests = self.member_select_tests(path, select, &mut prelude)?;
        let two_state = match tests.first().map(|(_, leaf)| leaf) {
            Some(NativeEndpointLeaf::Value(_, leaf)) => {
                matches!(
                    leaf.ty,
                    IrClassFieldType::Packed {
                        two_state: true,
                        ..
                    }
                )
            }
            Some(NativeEndpointLeaf::Module(leaf)) => leaf.member.two_state,
            Some(NativeEndpointLeaf::Container(_)) => {
                return Err(CONTAINER_LEAF_UNSUPPORTED.to_owned())
            }
            None => false,
        };
        let mut values = Vec::with_capacity(tests.len());
        for (test, leaf) in tests {
            values.push((test, self.endpoint_leaf_read(&leaf)?));
        }
        let mut result = match values.first().map(|(_, value)| value) {
            Some(LeafValue::Container(_)) => return Err(CONTAINER_LEAF_UNSUPPORTED.to_owned()),
            Some(LeafValue::String(_)) => LeafValue::String(IrStringExpr::Literal(Vec::new())),
            Some(LeafValue::Chandle(_)) => LeafValue::Chandle(IrChandleExpr::Null),
            Some(LeafValue::Real(_)) => LeafValue::Real(real_literal_expr(0.0)),
            Some(LeafValue::Packed(value)) => {
                let (width, signed) = (value.width, value.signed);
                let mut default = IrConst::integral_default(width, two_state);
                default.signed = signed;
                LeafValue::Packed(IrExpr::new(IrExprKind::Const(default), width, signed, None))
            }
            None => return Err("member array selection has no elements".to_owned()),
        };
        for (test, value) in values.into_iter().rev() {
            result = match (value, result) {
                (LeafValue::String(then), LeafValue::String(otherwise)) => {
                    LeafValue::String(IrStringExpr::Conditional {
                        predicate: Box::new(test),
                        then: Box::new(then),
                        otherwise: Box::new(otherwise),
                    })
                }
                (LeafValue::Chandle(then), LeafValue::Chandle(otherwise)) => {
                    LeafValue::Chandle(IrChandleExpr::Conditional {
                        predicate: Box::new(test),
                        then: Box::new(then),
                        otherwise: Box::new(otherwise),
                    })
                }
                (LeafValue::Real(a), LeafValue::Real(b)) => LeafValue::Real(IrExpr::new(
                    IrExprKind::Mux {
                        sel: Box::new(test),
                        a: Box::new(a),
                        b: Box::new(b),
                    },
                    0,
                    false,
                    None,
                )),
                (LeafValue::Packed(a), LeafValue::Packed(b)) => {
                    let (width, signed) = (a.width, a.signed);
                    LeafValue::Packed(IrExpr::new(
                        IrExprKind::Mux {
                            sel: Box::new(test),
                            a: Box::new(a),
                            b: Box::new(b),
                        },
                        width,
                        signed,
                        None,
                    ))
                }
                _ => {
                    return Err(format!(
                        "member array elements in `{path}` have mixed types"
                    ))
                }
            };
        }
        let value = result.into_leaf_expr()?;
        if prelude.is_empty() {
            return Ok(value);
        }
        // The hoisted index runs first: as the statements of a sequence, or
        // of an always-true predicate for string and handle values.
        let sequence = |value: IrExpr| {
            let (width, signed) = (value.width, value.signed);
            IrExpr::new(
                IrExprKind::Sequence(Box::new(crate::sim::ir::IrSequenceExpr {
                    statements: prelude.clone(),
                    value,
                })),
                width,
                signed,
                None,
            )
        };
        let first = || {
            sequence(crate::sim::codegen::lowering::containers::pattern_key_expr(
                1, 1, false, false,
            ))
        };
        Ok(match value {
            IrNativeLeafExpr::Packed(value) => IrNativeLeafExpr::Packed(sequence(value)),
            IrNativeLeafExpr::Real(value) => IrNativeLeafExpr::Real(sequence(value)),
            IrNativeLeafExpr::String(value) => {
                IrNativeLeafExpr::String(IrStringExpr::Conditional {
                    predicate: Box::new(first()),
                    then: Box::new(value),
                    otherwise: Box::new(IrStringExpr::Literal(Vec::new())),
                })
            }
            IrNativeLeafExpr::Chandle(value) => {
                IrNativeLeafExpr::Chandle(IrChandleExpr::Conditional {
                    predicate: Box::new(first()),
                    then: Box::new(value),
                    otherwise: Box::new(IrChandleExpr::Null),
                })
            }
        })
    }

    /// Store `rhs` into the selected element; an unmatched index writes
    /// nothing. The value is evaluated once, before the index is tested.
    pub(in crate::sim::codegen) fn native_member_select_write(
        &mut self,
        path: &str,
        select: &NativeMemberSelect,
        rhs: NodeId,
    ) -> Result<IrStmt, String> {
        let mut prelude = Vec::new();
        let tests = self.member_select_tests(path, select, &mut prelude)?;
        let (_, first) = tests
            .first()
            .ok_or("member array selection has no elements")?;
        let ty = match first {
            NativeEndpointLeaf::Container(_) => return Err(CONTAINER_LEAF_UNSUPPORTED.to_owned()),
            NativeEndpointLeaf::Value(_, leaf) => leaf.ty,
            NativeEndpointLeaf::Module(leaf) => match leaf.object {
                Some(object) => match self.model.objects[self.reference_object(object)].ty {
                    IrObjectType::String => IrClassFieldType::String,
                    _ => IrClassFieldType::Chandle,
                },
                None => IrClassFieldType::Real { shortreal: false },
            },
        };
        let sequence = self.native_copy_sequence;
        self.native_copy_sequence += 1;
        let mut statements = Vec::new();
        let value = self.native_leaf_source(path, ty, rhs)?;
        let value = capture_leaf(
            value,
            format!("_llg_member_select_{sequence}"),
            &mut statements,
        );
        // The value is evaluated before a hoisted index.
        statements.append(&mut prelude);
        let mut chain: Option<IrStmt> = None;
        for (test, leaf) in tests.into_iter().rev() {
            let write = self.endpoint_leaf_write(path, &leaf, value.clone(), false)?;
            chain = Some(IrStmt::If {
                cond: test,
                then_: vec![write],
                els: chain.map(|statement| vec![statement]),
                check: IrUniquePriorityCheck::None,
            });
        }
        statements.extend(chain);
        Ok(IrStmt::Block(statements))
    }
}

/// Leaves one index below `prefix` and then `trailing` members below that
/// element, as (declared index, leaf).
fn element_leaves<'a, T: 'a>(
    prefix: &'a [AggregatePathPart],
    trailing: &'a [AggregatePathPart],
    leaves: impl Iterator<Item = &'a T> + 'a,
    path_of: impl Fn(&T) -> &Vec<AggregatePathPart> + 'a,
) -> impl Iterator<Item = (i32, &'a T)> + 'a {
    leaves.filter_map(move |leaf| {
        let path = path_of(leaf);
        let below = path.strip_prefix(prefix)?;
        let (AggregatePathPart::Index(index), rest) = below.split_first()? else {
            return None;
        };
        (rest == trailing).then_some((*index, leaf))
    })
}
