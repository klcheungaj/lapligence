//! Typed member and array projections preserve their original storage owner.
use super::fixed_values::{fixed_width, two_state};
use super::*;
use crate::sim::ir::IrPackedSelect;

#[allow(clippy::large_enum_variant)]
enum FixedRoot {
    Activation(NodeId),
    /// A reduction iterator is a read-only lexical value, not an lvalue.
    ReadOnly(IrExpr),
    Cell {
        read: IrExpr,
        target: IrLhs,
    },
}

struct Projection {
    root: FixedRoot,
    descriptor: TypeDescriptor,
    steps: Vec<(IrPackedSelect, bool, Option<TaggedMemberGuard>)>,
    /// Steps whose dynamic unpacked index selects a mixed-state aggregate
    /// element. A read reapplies that element's leaf state domains, so an
    /// invalid index yields X four-state leaves and zero two-state leaves.
    element_states: Vec<(usize, TypeDescriptor)>,
    signed: bool,
    ref_legal: bool,
}

#[derive(Clone)]
struct TaggedMemberGuard {
    tag_width: u32,
    member_index: usize,
    member_name: String,
}

impl Codegen<'_> {
    fn fixed_activation_root(&self, node: NodeId) -> Result<Option<Projection>, String> {
        let node = match self.kind(node) {
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) => *target,
            _ => node,
        };
        let root = self.canonical_func_target(node).unwrap_or(node);
        if self
            .array_globals
            .get(&root)
            .is_some_and(|array| self.model.arrays[array.ir].activation)
        {
            return Ok(None);
        }
        let Some(function) = self.func.as_ref() else {
            return Ok(None);
        };
        if !function.arg_ir.contains_key(&root)
            && !function.locals.contains_key(&root)
            && function.ret_node != Some(root)
        {
            return Ok(None);
        }
        let Some(descriptor) = self.query_descriptor(root).cloned() else {
            return Ok(None);
        };
        if !matches!(
            &descriptor.shape,
            TypeShape::FixedArray { .. }
                | TypeShape::Aggregate(crate::core::db::AggregateLayout {
                    kind: AggregateKind::PackedStruct
                        | AggregateKind::PackedUnion
                        | AggregateKind::TaggedUnion
                        | AggregateKind::UnpackedStruct
                        | AggregateKind::UnpackedUnion,
                    ..
                })
        ) {
            return Ok(None);
        }
        if fixed_width(&descriptor).is_none() {
            return Ok(None);
        }
        let storage = if let Some(signal) = function.persistent.get(&root) {
            FixedRoot::Cell {
                read: self.signal_read_expr(signal)?,
                target: self.reference_lhs(IrLhs::Whole(signal.ir))?,
            }
        } else {
            FixedRoot::Activation(root)
        };
        Ok(Some(Projection {
            root: storage,
            signed: descriptor.info.signed,
            descriptor,
            steps: Vec::new(),
            element_states: Vec::new(),
            ref_legal: true,
        }))
    }

    fn fixed_root(&mut self, path: &str, node: NodeId) -> Result<Option<Projection>, String> {
        let declaration = match self.kind(node) {
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) => *target,
            _ => node,
        };
        if let Some(iterator) = self.fixed_method_iterators.get(&declaration) {
            let descriptor = iterator.descriptor.clone();
            let width =
                fixed_width(&descriptor).ok_or("fixed reduction iterator width overflow")?;
            return Ok(Some(Projection {
                root: FixedRoot::ReadOnly(IrExpr::new(
                    IrExprKind::LocalRead(iterator.item_name.clone()),
                    width,
                    descriptor.info.signed,
                    None,
                )),
                signed: descriptor.info.signed,
                descriptor,
                steps: Vec::new(),
                element_states: Vec::new(),
                ref_legal: false,
            }));
        }
        if let Some(root) = self.fixed_activation_root(node)? {
            return Ok(Some(root));
        }
        // A hierarchical reference can reach a static subroutine declaration
        // after its activation has returned. Its fixed selections must use the
        // same persistent signal as writes made inside the subroutine.
        if self.db.variable_lifetime(declaration) == VariableLifetime::Static
            && self.enclosing_func_task(declaration).is_some()
        {
            if let (Some(signal), Some(descriptor)) = (
                self.static_proc_local_signal(declaration)
                    .or_else(|| self.signal_of(declaration)),
                self.query_descriptor(declaration),
            ) {
                if matches!(
                    &descriptor.shape,
                    TypeShape::FixedArray { .. } | TypeShape::Aggregate(_)
                ) && fixed_width(descriptor).is_some()
                {
                    return Ok(Some(Projection {
                        root: FixedRoot::Cell {
                            read: self.signal_read_expr(signal)?,
                            target: self.reference_lhs(IrLhs::Whole(signal.ir))?,
                        },
                        signed: descriptor.info.signed,
                        descriptor: descriptor.clone(),
                        steps: Vec::new(),
                        element_states: Vec::new(),
                        ref_legal: true,
                    }));
                }
            }
        }
        let declaration = match self.kind(node) {
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) => *target,
            _ => node,
        };
        if let Some(info) = self.proc_local_info(declaration) {
            if let Some(descriptor) =
                self.query_descriptor(declaration)
                    .cloned()
                    .filter(|descriptor| {
                        matches!(
                            &descriptor.shape,
                            TypeShape::FixedArray { .. } | TypeShape::Aggregate(_)
                        )
                    })
            {
                let (read, target) = if let Some(signal) = &info.static_signal {
                    (
                        self.signal_read_expr(signal)?,
                        self.reference_lhs(IrLhs::Whole(signal.ir))?,
                    )
                } else {
                    (
                        IrExpr::new(
                            IrExprKind::LocalRead(info.c_name.clone()),
                            info.width,
                            info.signed,
                            None,
                        ),
                        IrLhs::WholeRef {
                            addr: format!("&{}", info.c_name),
                            width: info.width,
                            signed: info.signed,
                            two_state: info.two_state,
                            shortreal: false,
                        },
                    )
                };
                return Ok(Some(Projection {
                    root: FixedRoot::Cell { read, target },
                    descriptor,
                    steps: Vec::new(),
                    element_states: Vec::new(),
                    signed: info.signed,
                    ref_legal: true,
                }));
            }
        }
        if let Some((owner, aggregate)) = self.unpacked_aggregate_info(node) {
            let descriptor = self
                .query_descriptor(owner)
                .cloned()
                .ok_or("aggregate has no descriptor")?;
            if let Some(width) = fixed_width(&descriptor) {
                if aggregate.kind == AggregateKind::UnpackedUnion
                    || aggregate
                        .leaves
                        .first()
                        .is_some_and(|leaf| leaf.path.is_empty())
                {
                    if let Some(signal) = aggregate
                        .leaves
                        .first()
                        .and_then(|leaf| leaf.signal.as_ref())
                    {
                        return Ok(Some(Projection {
                            root: FixedRoot::Cell {
                                read: self.signal_read_expr(signal)?,
                                target: self.reference_lhs(IrLhs::Whole(signal.ir))?,
                            },
                            descriptor,
                            steps: Vec::new(),
                            element_states: Vec::new(),
                            signed: false,
                            ref_legal: true,
                        }));
                    }
                }
                let mut values = Vec::new();
                let mut parts = Vec::new();
                for leaf in &aggregate.leaves {
                    let value = self.aggregate_leaf_read(leaf)?;
                    parts.push((self.aggregate_leaf_lhs(leaf)?, value.width));
                    values.push(value);
                }
                let read = Self::join_bitstream_parts(path, values)?;
                if read.width != width {
                    return Err("aggregate backing layout disagrees with its type".into());
                }
                return Ok(Some(Projection {
                    root: FixedRoot::Cell {
                        read,
                        target: IrLhs::Stream {
                            parts,
                            width,
                            slice: 1,
                            direction: IrStreamDirection::LeftToRight,
                        },
                    },
                    descriptor,
                    steps: Vec::new(),
                    element_states: Vec::new(),
                    signed: false,
                    ref_legal: true,
                }));
            }
        }
        if let Some(array) = self.array_of(node).cloned().filter(|array| !array.real) {
            if let Some(descriptor) = self.query_descriptor(node).cloned() {
                if let Some(width) = fixed_width(&descriptor) {
                    let mut parts = Vec::new();
                    let mut values = Vec::new();
                    let arr = self.reference_array(array.ir);
                    for coordinates in port_array_index_vectors(&array.dims) {
                        let indices = coordinates
                            .into_iter()
                            .map(|index| lhs_integer_expr(i128::from(index)))
                            .collect::<Vec<_>>();
                        values.push(IrExpr::new(
                            IrExprKind::ArrayRead {
                                arr,
                                indices: indices.clone(),
                                elem_sel: IrElemSel::Whole,
                            },
                            array.elem_width,
                            array.signed,
                            None,
                        ));
                        parts.push((
                            self.reference_lhs(IrLhs::ArrayElem {
                                arr,
                                indices,
                                elem_sel: IrElemSel::Whole,
                            })?,
                            array.elem_width,
                        ));
                    }
                    return Ok(Some(Projection {
                        root: FixedRoot::Cell {
                            read: Self::join_bitstream_parts(path, values)?,
                            target: IrLhs::Stream {
                                parts,
                                width,
                                slice: 1,
                                direction: IrStreamDirection::LeftToRight,
                            },
                        },
                        descriptor,
                        signed: false,
                        steps: Vec::new(),
                        element_states: Vec::new(),
                        ref_legal: true,
                    }));
                }
            }
        }
        let (base, mut indices) = match self.kind(node) {
            NodeKind::Expr(ExprKind::ArraySelect { base, indices }) => (*base, indices.clone()),
            NodeKind::Expr(ExprKind::BitSelect { base, index })
                if self.array_of(*base).is_some() =>
            {
                (*base, vec![*index])
            }
            _ => return Ok(None),
        };
        let Some(array) = self.array_of(base).cloned() else {
            return Ok(None);
        };
        if indices.len() < array.dims.len() {
            return Ok(None);
        }
        let packed_indices = indices.split_off(array.dims.len());
        let Some(TypeDescriptor {
            shape: TypeShape::FixedArray { element, .. },
            ..
        }) = self.query_descriptor(base)
        else {
            return Ok(None);
        };
        let descriptor = *element.clone();
        let Some(width) = fixed_width(&descriptor) else {
            return Ok(None);
        };
        let indices = indices
            .into_iter()
            .map(|index| self.lower_expr(path, index))
            .collect::<Result<Vec<_>, _>>()?;
        let arr = self.reference_array(array.ir);
        let mut projection = Projection {
            root: FixedRoot::Cell {
                read: IrExpr::new(
                    IrExprKind::ArrayRead {
                        arr,
                        indices: indices.clone(),
                        elem_sel: IrElemSel::Whole,
                    },
                    width,
                    array.signed,
                    None,
                ),
                target: self.reference_lhs(IrLhs::ArrayElem {
                    arr,
                    indices,
                    elem_sel: IrElemSel::Whole,
                })?,
            },
            signed: descriptor.info.signed,
            descriptor,
            steps: Vec::new(),
            element_states: Vec::new(),
            ref_legal: true,
        };
        for index in packed_indices {
            self.fixed_index(path, &mut projection, index)?;
        }
        Ok(Some(projection))
    }

    /// A module or static signal holding a finite tagged union is one packed
    /// owner. Member arrays inside it (`u.member[i]`) project from that
    /// owner with the member's tag guard rather than from separate storage.
    fn tagged_signal_root(&self, root: NodeId) -> Result<Option<Projection>, String> {
        let (Some(signal), Some(descriptor)) = (self.signal_of(root), self.query_descriptor(root))
        else {
            return Ok(None);
        };
        if !matches!(&descriptor.shape, TypeShape::Aggregate(layout) if layout.kind == AggregateKind::TaggedUnion)
        {
            return Ok(None);
        }
        Ok(Some(Projection {
            root: FixedRoot::Cell {
                read: self.signal_read_expr(signal)?,
                target: self.reference_lhs(IrLhs::Whole(signal.ir))?,
            },
            signed: descriptor.info.signed,
            descriptor: descriptor.clone(),
            steps: Vec::new(),
            element_states: Vec::new(),
            ref_legal: true,
        }))
    }

    fn fixed_projection(&mut self, path: &str, node: NodeId) -> Result<Option<Projection>, String> {
        if let Some(root) = self.fixed_root(path, node)? {
            return Ok(Some(root));
        }
        let kind = self.kind(node);
        match kind {
            NodeKind::Expr(ExprKind::HierPath { parts, refs }) => {
                let (parts, refs) = (parts.clone(), refs.clone());
                for (index, root) in refs.iter().enumerate() {
                    let Some(root) = root else {
                        continue;
                    };
                    let projection = self.fixed_projection(path, *root)?;
                    let projection = match projection {
                        Some(projection) => Some(projection),
                        None if index + 1 < parts.len() => {
                            match (self.signal_of(*root), self.query_descriptor(*root)) {
                                (Some(signal), Some(descriptor))
                                    if matches!(
                                        &descriptor.shape,
                                        TypeShape::Aggregate(layout)
                                            if matches!(layout.kind, AggregateKind::PackedStruct | AggregateKind::PackedUnion | AggregateKind::TaggedUnion)
                                    ) =>
                                {
                                    Some(Projection {
                                        root: FixedRoot::Cell {
                                            read: self.signal_read_expr(signal)?,
                                            target: self.reference_lhs(IrLhs::Whole(signal.ir))?,
                                        },
                                        signed: descriptor.info.signed,
                                        descriptor: descriptor.clone(),
                                        steps: Vec::new(),
                                        element_states: Vec::new(),
                                        ref_legal: true,
                                    })
                                }
                                _ => None,
                            }
                        }
                        None => None,
                    };
                    let Some(mut projection) = projection else {
                        continue;
                    };
                    for member in &parts[index + 1..] {
                        Self::fixed_member(&mut projection, member)?;
                    }
                    return Ok(Some(projection));
                }
                Ok(None)
            }
            NodeKind::Expr(ExprKind::ArraySelect { base, indices }) => {
                let base = *base;
                let indices = indices.clone();
                let projection = if let Some((root, members)) = self.db.array_select_path(node) {
                    let members = members.to_vec();
                    let root_projection = match self.fixed_projection(path, root)? {
                        Some(projection) => Some(projection),
                        None => self.tagged_signal_root(root)?,
                    };
                    if let Some(mut projection) = root_projection {
                        for member in members {
                            Self::fixed_member(&mut projection, &member)?;
                        }
                        Some(projection)
                    } else {
                        self.fixed_projection(path, base)?
                    }
                } else {
                    self.fixed_projection(path, base)?
                };
                let Some(mut projection) = projection else {
                    return Ok(None);
                };
                for index in indices {
                    self.fixed_index(path, &mut projection, index)?;
                }
                Ok(Some(projection))
            }
            NodeKind::Expr(ExprKind::BitSelect { base, index }) => {
                let (base, index) = (*base, *index);
                let Some(mut projection) = self.fixed_projection(path, base)? else {
                    return Ok(None);
                };
                self.fixed_index(path, &mut projection, index)?;
                Ok(Some(projection))
            }
            NodeKind::Expr(ExprKind::PartSelect { base, left, right }) => {
                let (base, left, right) = (*base, *left, *right);
                let Some(mut projection) = self.fixed_projection(path, base)? else {
                    return Ok(None);
                };
                if let TypeShape::FixedArray {
                    dimensions,
                    element,
                } = &projection.descriptor.shape
                {
                    let (decl_left, decl_right) =
                        *dimensions.first().ok_or("fixed slice has no dimension")?;
                    let left = i32::try_from(self.eval_bound_i128(left)?)
                        .map_err(|_| "fixed slice left bound overflow")?;
                    let right = i32::try_from(self.eval_bound_i128(right)?)
                        .map_err(|_| "fixed slice right bound overflow")?;
                    if left.min(right) < decl_left.min(decl_right)
                        || left.max(right) > decl_left.max(decl_right)
                    {
                        return Err("fixed slice bounds exceed its declaration".into());
                    }
                    let parent_width =
                        fixed_width(&projection.descriptor).ok_or("fixed slice has no width")?;
                    let extent = i64::from(decl_left).abs_diff(i64::from(decl_right)) + 1;
                    let stride = u32::try_from(u64::from(parent_width) / extent)
                        .map_err(|_| "fixed slice stride overflow")?;
                    let offset =
                        i64::from(right).abs_diff(i64::from(decl_right)) * u64::from(stride);
                    let width = u32::try_from(
                        (i64::from(left).abs_diff(i64::from(right)) + 1) * u64::from(stride),
                    )
                    .map_err(|_| "fixed slice width overflow")?;
                    let mut dimensions = dimensions.clone();
                    dimensions[0] = (left, right);
                    projection.descriptor.shape = TypeShape::FixedArray {
                        dimensions,
                        element: element.clone(),
                    };
                    projection.steps.push((
                        IrPackedSelect {
                            base: lhs_integer_expr(i128::from(offset)),
                            width,
                        },
                        false,
                        None,
                    ));
                    projection.signed = false;
                    projection.ref_legal = false;
                    return Ok(Some(projection));
                }
                let mut width =
                    fixed_width(&projection.descriptor).ok_or("fixed value has no width")?;
                let mut steps = Vec::new();
                self.packed_selection_steps(
                    path,
                    base,
                    super::packed_elements::Select::Part(left, right),
                    &mut width,
                    &mut steps,
                )?;
                projection
                    .steps
                    .extend(steps.into_iter().map(|step| (step, false, None)));
                projection.signed = false;
                projection.ref_legal = false;
                Ok(Some(projection))
            }
            NodeKind::Expr(ExprKind::IndexedPartSelect {
                base,
                base_expr,
                width_expr,
                neg,
            }) => {
                let (base, base_expr, width_expr, neg) = (*base, *base_expr, *width_expr, *neg);
                let Some(mut projection) = self.fixed_projection(path, base)? else {
                    return Ok(None);
                };
                let mut width =
                    fixed_width(&projection.descriptor).ok_or("fixed value has no width")?;
                let mut steps = Vec::new();
                self.packed_selection_steps(
                    path,
                    base,
                    super::packed_elements::Select::Indexed(base_expr, width_expr, neg),
                    &mut width,
                    &mut steps,
                )?;
                projection
                    .steps
                    .extend(steps.into_iter().map(|step| (step, false, None)));
                projection.signed = false;
                projection.ref_legal = false;
                Ok(Some(projection))
            }
            _ => Ok(None),
        }
    }

    fn fixed_member(projection: &mut Projection, name: &str) -> Result<(), String> {
        let TypeShape::Aggregate(layout) = &projection.descriptor.shape else {
            return Err("member of non-aggregate fixed value".into());
        };
        let index = layout
            .members
            .iter()
            .position(|member| member.name == name)
            .ok_or_else(|| format!("fixed value has no member `{name}`"))?;
        let member = &layout.members[index];
        let width = fixed_width(&member.descriptor).ok_or("fixed member has no width")?;
        let tagged_guard = if layout.kind == AggregateKind::TaggedUnion {
            fixed_width(&projection.descriptor).ok_or("tagged union has no fixed width")?;
            let tag_width = layout.tag_bits().ok_or("tagged union tag width overflow")?;
            (tag_width > 0).then_some(TaggedMemberGuard {
                tag_width,
                member_index: index,
                member_name: name.to_owned(),
            })
        } else {
            None
        };
        let offset = if layout.kind == AggregateKind::UnpackedUnion
            && matches!(&member.descriptor.shape, TypeShape::Aggregate(member_layout) if member_layout.kind == AggregateKind::UnpackedStruct)
        {
            fixed_width(&projection.descriptor).ok_or("union has no width")? - width
        } else if matches!(
            layout.kind,
            AggregateKind::PackedUnion | AggregateKind::TaggedUnion | AggregateKind::UnpackedUnion
        ) {
            0
        } else {
            layout.members[index + 1..]
                .iter()
                .try_fold(0u32, |sum, member| {
                    sum.checked_add(fixed_width(&member.descriptor)?)
                })
                .ok_or("fixed member offset overflow")?
        };
        projection.ref_legal = matches!(
            layout.kind,
            AggregateKind::UnpackedStruct | AggregateKind::UnpackedUnion
        );
        projection.steps.push((
            IrPackedSelect {
                base: lhs_integer_expr(i128::from(offset)),
                width,
            },
            member.two_state,
            tagged_guard,
        ));
        projection.signed = member.ty.signed;
        projection.descriptor = member.descriptor.clone();
        Ok(())
    }

    fn fixed_index(
        &mut self,
        path: &str,
        projection: &mut Projection,
        index: NodeId,
    ) -> Result<(), String> {
        let parent_width = fixed_width(&projection.descriptor).ok_or("fixed array has no width")?;
        let (left, right, mut element, unpacked) = match &projection.descriptor.shape {
            TypeShape::FixedArray {
                dimensions,
                element,
            } => {
                let &(left, right) = dimensions.first().ok_or("fixed array has no dimension")?;
                let selected = if dimensions.len() == 1 {
                    *element.clone()
                } else {
                    TypeDescriptor {
                        shape: TypeShape::FixedArray {
                            dimensions: dimensions[1..].to_vec(),
                            element: element.clone(),
                        },
                        ..projection.descriptor.clone()
                    }
                };
                (i128::from(left), i128::from(right), selected, true)
            }
            TypeShape::PackedAtom { ranges } => {
                let range = ranges
                    .first()
                    .copied()
                    .unwrap_or(crate::core::db::PackedRange {
                        left: i128::from(parent_width - 1),
                        right: 0,
                    });
                let count = range.left.abs_diff(range.right) + 1;
                let width = u32::try_from(u128::from(parent_width) / count)
                    .map_err(|_| "packed element width overflow")?;
                let mut element = projection.descriptor.clone();
                element.info.width = Some(width);
                element.info.signed = false;
                element.shape = TypeShape::PackedAtom {
                    ranges: ranges.get(1..).unwrap_or(&[]).to_vec(),
                };
                (range.left, range.right, element, false)
            }
            _ => return Err("selected fixed value is not an array".into()),
        };
        let width = fixed_width(&element).ok_or("fixed element width overflow")?;
        element.info.width = Some(width);
        let known_valid = self
            .eval_bound_i128(index)
            .is_ok_and(|index| (left.min(right)..=left.max(right)).contains(&index));
        let index = self.lower_expr(path, index)?;
        let base = super::packed_elements::packed_lsb(
            index,
            crate::core::db::PackedRange { left, right },
            width,
            0,
        )?;
        projection.steps.push((
            IrPackedSelect { base, width },
            unpacked && two_state(&element),
            None,
        ));
        if unpacked && !known_valid && !two_state(&element) && has_two_state_leaf(&element) {
            projection
                .element_states
                .push((projection.steps.len() - 1, element.clone()));
        }
        projection.ref_legal = unpacked;
        projection.signed = element.info.signed;
        projection.descriptor = element;
        Ok(())
    }

    pub(in super::super) fn fixed_activation_read(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<IrExpr>, String> {
        // array_of follows conversions for storage discovery, not value
        // evaluation. A cast must run its state/type conversion before any
        // projection; otherwise nested four/two/four-state casts lose zeros.
        if matches!(self.kind(node), NodeKind::Expr(ExprKind::Cast { .. })) {
            return Ok(None);
        }
        let Some(projection) = self.fixed_projection(path, node)? else {
            return Ok(None);
        };
        let mut value = match &projection.root {
            FixedRoot::Cell { read, .. } | FixedRoot::ReadOnly(read) => read.clone(),
            FixedRoot::Activation(root) => {
                let function = self.func.as_ref().ok_or("fixed value outside activation")?;
                if let Some(value) = function.arg_ir.get(root) {
                    value.clone()
                } else if let Some((name, width, signed, ..)) = function.locals.get(root) {
                    IrExpr::new(IrExprKind::LocalRead(name.clone()), *width, *signed, None)
                } else {
                    let ret = function.ret.as_ref().ok_or("fixed return has no storage")?;
                    IrExpr::new(
                        IrExprKind::LocalRead(ret.c_name.clone()),
                        ret.width,
                        ret.signed,
                        None,
                    )
                }
            }
        };
        let has_tagged_member = projection.steps.iter().any(|(_, _, guard)| guard.is_some());
        if has_tagged_member {
            let steps = projection
                .steps
                .into_iter()
                .map(|(selection, two_state, tagged_guard)| {
                    let guard = tagged_guard
                        .map(|guard| -> Result<_, String> {
                            Ok(crate::sim::ir::IrTaggedMemberGuard {
                                member_index: u32::try_from(guard.member_index)
                                    .map_err(|_| "tagged union member index overflow")?,
                                tag_width: guard.tag_width,
                                member_name: guard.member_name,
                            })
                        })
                        .transpose()?;
                    Ok(crate::sim::ir::IrTaggedSelectStep {
                        selection,
                        two_state,
                        guard,
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;
            let width = steps
                .last()
                .map(|step| step.selection.width)
                .ok_or("tagged member access has no projection steps")?;
            let selected = IrExpr::new(
                IrExprKind::TaggedSelect {
                    base: Box::new(value),
                    steps,
                    location: self.source_location(node),
                },
                width,
                projection.signed,
                None,
            );
            // An inactive-member read yields X; a two-state result type
            // still converts it, as an ordinary two-state member read does.
            return Ok(Some(if two_state(&projection.descriptor) {
                IrExpr::to_two_state(selected)
            } else {
                selected
            }));
        }
        let mut element_states = projection.element_states.into_iter().peekable();
        for (ordinal, (step, state, _tagged_guard)) in projection.steps.into_iter().enumerate() {
            value = super::packed_formals::packed_step_read(value, step);
            if state {
                value = IrExpr::to_two_state(value);
            }
            if let Some((_, element)) = element_states.next_if(|(at, _)| *at == ordinal) {
                // Stored two-state leaves are already known, so the helper is
                // the identity for a valid index and supplies the element's
                // default-uninitialized value otherwise (SV 7.4.6).
                value = self.convert_fixed_descriptor_payload(&element, value)?;
            }
        }
        let width = value.width;
        Ok(Some(IrExpr::resize_to(value, width, projection.signed)))
    }

    pub(in super::super) fn fixed_ref_is_legal(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<bool, String> {
        Ok(self
            .fixed_projection(path, node)?
            .is_some_and(|projection| {
                projection.ref_legal && !matches!(projection.root, FixedRoot::ReadOnly(_))
            }))
    }

    pub(in super::super) fn fixed_activation_lhs(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<IrLhs>, String> {
        self.fixed_projection_lhs(path, node, false)
    }

    pub(super) fn fixed_reference_lhs(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<IrLhs>, String> {
        self.fixed_projection_lhs(path, node, true)
    }

    fn fixed_projection_lhs(
        &mut self,
        path: &str,
        node: NodeId,
        allow_const: bool,
    ) -> Result<Option<IrLhs>, String> {
        let Some(projection) = self.fixed_projection(path, node)? else {
            return Ok(None);
        };
        let target = match projection.root {
            FixedRoot::ReadOnly(_) => {
                return Err("fixed-array reduction iterator is read-only".into())
            }
            FixedRoot::Cell { target, .. } => target,
            FixedRoot::Activation(root) => {
                if allow_const
                    && self
                        .func
                        .as_ref()
                        .is_some_and(|function| function.const_refs.contains(&root))
                {
                    self.lower_const_ref_lhs(root, path)?
                } else {
                    let target = self
                        .func_write_target(root, "")
                        .ok_or("fixed value is not writable")?;
                    self.lhs_to_ir(target)?
                }
            }
        };
        if projection.steps.iter().any(|(_, _, guard)| guard.is_some()) {
            let steps = projection
                .steps
                .into_iter()
                .map(|(selection, two_state, tagged_guard)| {
                    let guard = tagged_guard
                        .map(|guard| -> Result<_, String> {
                            Ok(crate::sim::ir::IrTaggedMemberGuard {
                                member_index: u32::try_from(guard.member_index)
                                    .map_err(|_| "tagged union member index overflow")?,
                                tag_width: guard.tag_width,
                                member_name: guard.member_name,
                            })
                        })
                        .transpose()?;
                    Ok(crate::sim::ir::IrTaggedSelectStep {
                        selection,
                        two_state,
                        guard,
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;
            return Ok(Some(IrLhs::TaggedSelect {
                target: Box::new(target),
                steps,
                signed: projection.signed,
                two_state: two_state(&projection.descriptor),
                location: self.source_location(node),
            }));
        }
        if projection.steps.is_empty() {
            return Ok(Some(target));
        }
        if let IrLhs::ArrayElem {
            arr,
            indices,
            elem_sel: IrElemSel::Whole,
        } = &target
        {
            if two_state(&projection.descriptor) == self.model.arrays[*arr].two_state {
                return Ok(Some(IrLhs::ArrayElem {
                    arr: *arr,
                    indices: indices.clone(),
                    elem_sel: IrElemSel::PackedChain(
                        projection
                            .steps
                            .into_iter()
                            .map(|(step, _, _)| step)
                            .collect(),
                    ),
                }));
            }
        }
        Ok(Some(IrLhs::PackedSelect {
            target: Box::new(target),
            steps: projection
                .steps
                .into_iter()
                .map(|(step, _, _)| step)
                .collect(),
            signed: projection.signed,
            two_state: two_state(&projection.descriptor),
        }))
    }
}

/// Whether a four-state fixed aggregate contains a two-state leaf.
fn has_two_state_leaf(descriptor: &TypeDescriptor) -> bool {
    two_state(descriptor)
        || match &descriptor.shape {
            TypeShape::Aggregate(layout) => layout
                .members
                .iter()
                .any(|member| member.two_state || has_two_state_leaf(&member.descriptor)),
            TypeShape::FixedArray { element, .. } => has_two_state_leaf(element),
            _ => false,
        }
}
