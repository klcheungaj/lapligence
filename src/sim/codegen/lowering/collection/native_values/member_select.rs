//! Run-time indices into fixed-array members of native records (SIM-007).
//!
//! A module native record keeps each element of a member array as its own
//! leaf, and a native subroutine value addresses each element by a constant
//! item path. A run-time index (`r.s[k]`) therefore selects one leaf by
//! comparing the index with every declared index; an index that matches
//! none (out of range, X or Z) reads the element default and writes nothing
//! (SV 7.4.6). Small arrays compare in a chain; above
//! [`NATIVE_MEMBER_SELECT_LINEAR`] elements a balanced decision tree over the
//! declared indices keeps each access logarithmic. The generated code stays
//! linear in the array per access site, so arrays are bounded by
//! [`NATIVE_MEMBER_SELECT_LIMIT`]. An index with side effects is evaluated
//! once into a local before the comparisons.

use super::*;
use crate::sim::ir::{IrElementRef, IrElementRefKind, IrElementRoot, IrLeafTarget};

/// Largest member array selected by a run-time index: each access site
/// holds one branch per element, so larger arrays are rejected rather than
/// expanded. Equal to the dense record member threshold, above which module
/// records already reject such a member.
pub(in crate::sim::codegen) const NATIVE_MEMBER_SELECT_LIMIT: usize = 4096;

/// Element count up to which a select compares in a chain; a chain is the
/// smaller code and needs at most this many comparisons.
const NATIVE_MEMBER_SELECT_LINEAR: usize = 8;

/// Largest member array selected whole by a run-time index: every branch
/// copies one whole record.
const NATIVE_RECORD_SELECT_LIMIT: usize = 256;

/// The elements of a member array of records selected whole by a run-time
/// index, each as a transfer endpoint.
pub(in crate::sim::codegen) struct NativeRecordSelect {
    elements: Vec<(i32, NativeEndpoint)>,
    index: NodeId,
    hoist: bool,
}

/// The elements of a member array selected by a run-time index.
pub(in crate::sim::codegen) struct NativeMemberSelect {
    /// Declared index and leaf of every element, in declaration order.
    elements: Vec<(i32, NativeEndpointLeaf)>,
    index: NodeId,
    /// The index has side effects, so the chain compares a local copy.
    hoist: bool,
}

impl Codegen<'_> {
    /// The run-time select of `node` (`r.a[k]`, `r.e[k].s`): the select
    /// node, its array, its index and the member path below it.
    #[allow(clippy::type_complexity)]
    fn member_select_parts(
        &self,
        node: NodeId,
    ) -> Result<Option<(NodeId, NodeId, NodeId, Vec<AggregatePathPart>)>, String> {
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
            // `r.e[k].a[1]`: a constant element of a member array below the
            // selected record element.
            NodeKind::Expr(ExprKind::ArraySelect { indices, .. })
                if self.db.array_select_path(node).is_some_and(|(owner, _)| {
                    matches!(
                        self.kind(owner),
                        NodeKind::Expr(ExprKind::ArraySelect { .. })
                    )
                }) =>
            {
                let Some((owner, members)) = self.db.array_select_path(node) else {
                    return Ok(None);
                };
                let mut trailing = members
                    .iter()
                    .cloned()
                    .map(AggregatePathPart::Member)
                    .collect::<Vec<_>>();
                for index in indices {
                    let Some(index) = self
                        .eval_bound_i128(*index)
                        .ok()
                        .and_then(|index| i32::try_from(index).ok())
                    else {
                        return Ok(None);
                    };
                    trailing.push(AggregatePathPart::Index(index));
                }
                (owner, trailing)
            }
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
        Ok(Some((select, base, index, trailing)))
    }

    /// `r.a[k]` with a non-constant `k`, where `r.a` is a one-dimensional
    /// array member of scalar elements in a native record.
    pub(in crate::sim::codegen) fn native_member_select(
        &self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<NativeMemberSelect>, String> {
        let Some((select, base, index, trailing)) = self.member_select_parts(node)? else {
            return Ok(None);
        };
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
                (IrElementRefKind::Packed { .. } | IrElementRefKind::Real { .. }, None) => {
                    IrLeafTarget::Packed {
                        lhs: self.aggregate_leaf_lhs(leaf)?,
                        read: self.aggregate_leaf_read(leaf)?,
                    }
                }
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

    /// `r.e[k]` or `r.e[k].inner` with a non-constant `k`, where the
    /// selection is a whole record or array rather than one leaf.
    pub(in crate::sim::codegen) fn native_record_select(
        &self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<NativeRecordSelect>, String> {
        let Some((select, base, index, trailing)) = self.member_select_parts(node)? else {
            return Ok(None);
        };
        let select_path = self.db.array_select_path(select);
        let value_root = match &select_path {
            Some((owner, members)) => self.native_roots.get(owner).map(|value| {
                let prefix = members
                    .iter()
                    .cloned()
                    .map(AggregatePathPart::Member)
                    .collect::<Vec<_>>();
                (*value, prefix)
            }),
            None => self.native_path_of(base)?,
        };
        let module_root = match (&value_root, select_path) {
            (Some(_), _) => None,
            (None, Some((owner, members))) if self.unpacked_aggregates.contains_key(&owner) => {
                Some((
                    owner,
                    members
                        .iter()
                        .cloned()
                        .map(AggregatePathPart::Member)
                        .collect::<Vec<_>>(),
                ))
            }
            (None, _) => self.unpacked_path_for_expr(base),
        };
        let (descriptor, prefix) = if let Some((value, prefix)) = &value_root {
            let layout = self.native_layout_of_value(*value)?;
            (layout.descriptor.clone(), prefix.clone())
        } else if let Some((root, prefix)) = &module_root {
            let Some(descriptor) = self.query_descriptor(*root) else {
                return Ok(None);
            };
            (descriptor.clone(), prefix.clone())
        } else {
            return Ok(None);
        };
        let Some(TypeShape::FixedArray { dimensions, .. }) =
            Self::descriptor_at_path(&descriptor, &prefix).map(|array| array.shape)
        else {
            return Ok(None);
        };
        let [(left, right)] = dimensions[..] else {
            return Ok(None);
        };
        let member = |declared: i32| {
            let mut member = prefix.clone();
            member.push(AggregatePathPart::Index(declared));
            member.extend(trailing.iter().cloned());
            member
        };
        let selected = Self::descriptor_at_path(&descriptor, &member(left));
        if !selected.is_some_and(|selected| {
            matches!(
                selected.shape,
                TypeShape::Aggregate(_) | TypeShape::FixedArray { .. }
            )
        }) {
            return Ok(None);
        }
        let count = left.abs_diff(right) as usize + 1;
        if count > NATIVE_RECORD_SELECT_LIMIT {
            return Err(format!(
                "run-time selection of a whole record among {count} elements in `{path}` is not supported; at most {NATIVE_RECORD_SELECT_LIMIT} are"
            ));
        }
        let mut elements = Vec::with_capacity(count);
        for declared in left.min(right)..=left.max(right) {
            let endpoint = if let Some((value, _)) = &value_root {
                NativeEndpoint::Value {
                    value: *value,
                    prefix: member(declared),
                }
            } else {
                let Some((root, _)) = &module_root else {
                    return Ok(None);
                };
                let Some(storage) = self.unpacked_aggregates.get(root) else {
                    return Ok(None);
                };
                let prefix = member(declared);
                let Some(descriptor) = Self::descriptor_at_path(&descriptor, &prefix) else {
                    return Ok(None);
                };
                NativeEndpoint::Module(Box::new(AggregateSelection {
                    root: *root,
                    prefix,
                    descriptor,
                    storage: storage.clone(),
                }))
            };
            elements.push((declared, endpoint));
        }
        let hoist = !self.side_effect_free(index);
        Ok(Some(NativeRecordSelect {
            elements,
            index,
            hoist,
        }))
    }

    /// Copy the selected element into `target`; an unmatched index copies
    /// the default of the element type (SV 7.4.6). `node` names the source
    /// for its type.
    pub(in crate::sim::codegen) fn native_record_select_into(
        &mut self,
        path: &str,
        select: &NativeRecordSelect,
        target: &NativeEndpoint,
        node: NodeId,
        nba: bool,
    ) -> Result<IrStmt, String> {
        let mut statements = Vec::new();
        let (index, width) =
            self.member_select_index(path, select.index, select.hoist, &mut statements)?;
        // Every element and the default fill one temporary, which then moves
        // to the target, so a queued write has one source.
        let temporary = self.native_temporary(node)?;
        statements.push(IrStmt::NativeValueDeclare(temporary));
        let value = NativeEndpoint::Value {
            value: temporary,
            prefix: Vec::new(),
        };
        let mut branches = Vec::with_capacity(select.elements.len());
        for (declared, element) in &select.elements {
            let copy = self.native_transfer(path, &value, element, false)?;
            branches.push((*declared, Some(copy)));
        }
        statements.extend(member_select_statement(branches, &index, width));
        statements.push(self.native_transfer(path, target, &value, nba)?);
        Ok(IrStmt::Block(statements))
    }

    /// Store `rhs` into the selected element; an unmatched index writes
    /// nothing. The value is evaluated once, before the index is tested.
    pub(in crate::sim::codegen) fn native_record_select_write(
        &mut self,
        path: &str,
        select: &NativeRecordSelect,
        lhs: NodeId,
        rhs: NodeId,
        nba: bool,
    ) -> Result<IrStmt, String> {
        let temporary = self.native_temporary(lhs)?;
        let descriptor = self.native_layout_of_value(temporary)?.descriptor.clone();
        let value = NativeEndpoint::Value {
            value: temporary,
            prefix: Vec::new(),
        };
        let mut statements = vec![
            IrStmt::NativeValueDeclare(temporary),
            self.native_assign_into(path, &value, &descriptor, rhs, false)?,
        ];
        let (index, width) =
            self.member_select_index(path, select.index, select.hoist, &mut statements)?;
        let mut branches = Vec::with_capacity(select.elements.len());
        for (declared, element) in &select.elements {
            let copy = self.native_transfer(path, element, &value, nba)?;
            branches.push((*declared, Some(copy)));
        }
        statements.extend(member_select_statement(branches, &index, width));
        Ok(IrStmt::Block(statements))
    }

    /// The index of a select as a signed value that holds every declared
    /// index, stored once in a local when `hoist`.
    fn member_select_index(
        &mut self,
        path: &str,
        node: NodeId,
        hoist: bool,
        prelude: &mut Vec<IrStmt>,
    ) -> Result<(IrExpr, u32), String> {
        let index = self.lower_container_index(path, node)?;
        let width = index.width.max(32) + 1;
        let mut index = IrExpr::convert_to(index, width, true);
        if hoist {
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
        Ok((index, width))
    }

    /// The selected element as a string, packed, real or handle value; an
    /// unmatched index reads the element type's default.
    pub(in crate::sim::codegen) fn native_member_select_read(
        &mut self,
        path: &str,
        select: &NativeMemberSelect,
    ) -> Result<IrNativeLeafExpr, String> {
        let mut prelude = Vec::new();
        let (index, width) =
            self.member_select_index(path, select.index, select.hoist, &mut prelude)?;
        let two_state = match select.elements.first().map(|(_, leaf)| leaf) {
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
        let mut values = Vec::with_capacity(select.elements.len());
        for (declared, leaf) in &select.elements {
            values.push((*declared, self.endpoint_leaf_read(leaf)?));
        }
        let default = match values.first().map(|(_, value)| value) {
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
        values.sort_by_key(|(declared, _)| *declared);
        let result =
            member_select_tree(&values, &index, width, &default, &mut |test, then, els| {
                leaf_conditional(path, test, then, els)
            })?;
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
        nba: bool,
    ) -> Result<IrStmt, String> {
        let mut prelude = Vec::new();
        let (index, width) =
            self.member_select_index(path, select.index, select.hoist, &mut prelude)?;
        let (_, first) = select
            .elements
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
        let mut branches = Vec::with_capacity(select.elements.len());
        for (declared, leaf) in &select.elements {
            let write = self.endpoint_leaf_write(path, leaf, value.clone(), nba)?;
            branches.push((*declared, Some(write)));
        }
        statements.extend(member_select_statement(branches, &index, width));
        Ok(IrStmt::Block(statements))
    }
}

/// Select one of `entries`, sorted by declared index, or `otherwise`: a chain
/// of equality tests for few entries, else a balanced tree. An X or Z index
/// fails every known test and reaches `otherwise`.
fn member_select_tree<V: Clone>(
    entries: &[(i32, V)],
    index: &IrExpr,
    width: u32,
    otherwise: &V,
    join: &mut impl FnMut(IrExpr, V, V) -> Result<V, String>,
) -> Result<V, String> {
    let declared = |value: i32| {
        crate::sim::codegen::lowering::containers::pattern_key_expr(
            i128::from(value),
            width,
            true,
            false,
        )
    };
    if entries.len() <= NATIVE_MEMBER_SELECT_LINEAR {
        let mut result = otherwise.clone();
        for (value, entry) in entries.iter().rev() {
            let test = cmp_expr_ir(IrBinOp::CaseEq, index.clone(), declared(*value));
            result = join(test, entry.clone(), result)?;
        }
        return Ok(result);
    }
    let (low, high) = entries.split_at(entries.len() / 2);
    let below = cmp_expr_ir(IrBinOp::Lt, index.clone(), declared(high[0].0));
    let known = cmp_expr_ir(
        IrBinOp::CaseEq,
        below,
        crate::sim::codegen::lowering::containers::pattern_key_expr(1, 1, false, false),
    );
    let then = member_select_tree(low, index, width, otherwise, join)?;
    let els = member_select_tree(high, index, width, otherwise, join)?;
    join(known, then, els)
}

/// The statement that runs the branch selected by `index`, if any.
fn member_select_statement(
    mut branches: Vec<(i32, Option<IrStmt>)>,
    index: &IrExpr,
    width: u32,
) -> Option<IrStmt> {
    branches.sort_by_key(|(declared, _)| *declared);
    member_select_tree(&branches, index, width, &None, &mut |test, then, els| {
        Ok(Some(IrStmt::If {
            cond: test,
            then_: then.into_iter().collect(),
            els: els.map(|statement| vec![statement]),
            check: IrUniquePriorityCheck::None,
        }))
    })
    .ok()
    .flatten()
}

/// `test ? then : els` over one leaf kind.
fn leaf_conditional(
    path: &str,
    test: IrExpr,
    then: LeafValue,
    els: LeafValue,
) -> Result<LeafValue, String> {
    Ok(match (then, els) {
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
    })
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
