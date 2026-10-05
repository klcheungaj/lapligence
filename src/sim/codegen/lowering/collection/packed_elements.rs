//! Packed selections of fixed-array elements, preserving each intermediate bound.

use super::super::packed_geometry::{offset_lsb, PackedSelectDim};
use super::*;
use crate::core::db::PackedMember;
use crate::sim::ir::IrPackedSelect;

pub(in super::super) enum Select {
    Elements(Vec<NodeId>),
    Part(NodeId, NodeId),
    Indexed(NodeId, NodeId, bool),
    /// A member path over the preceding element (`ps[i].hi`).
    Member(NodeId),
}

impl<'a> Codegen<'a> {
    /// Retain the typed slice chain rather than flattening offsets across
    /// intermediate bounds. The same plan feeds reads, mutations and masked
    /// NBA writes. A fully indexed unpacked root is required.
    pub(in super::super) fn packed_element_lhs_ir(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<IrLhs>, String> {
        let mut current = node;
        let mut outer = Vec::new();
        let (array_select, array_node, array_indices, packed_indices) = loop {
            match self.kind(current) {
                NodeKind::Expr(ExprKind::ArraySelect { base, indices }) => {
                    if let Some(array) = self.select_array_of(current, *base) {
                        let rank = array.dims.len();
                        if indices.len() < rank || array.real {
                            return Ok(None);
                        }
                        break (
                            current,
                            *base,
                            indices[..rank].to_vec(),
                            indices[rank..].to_vec(),
                        );
                    }
                    outer.push((*base, Select::Elements(indices.clone())));
                    current = *base;
                }
                NodeKind::Expr(ExprKind::BitSelect { base, index }) => {
                    if let Some(array) = self.array_of(*base) {
                        if array.dims.len() != 1 || array.real {
                            return Ok(None);
                        }
                        break (current, *base, vec![*index], Vec::new());
                    }
                    outer.push((*base, Select::Elements(vec![*index])));
                    current = *base;
                }
                NodeKind::Expr(ExprKind::PartSelect { base, left, right }) => {
                    outer.push((*base, Select::Part(*left, *right)));
                    current = *base;
                }
                NodeKind::Expr(ExprKind::IndexedPartSelect {
                    base,
                    base_expr,
                    width_expr,
                    neg,
                }) => {
                    outer.push((*base, Select::Indexed(*base_expr, *width_expr, *neg)));
                    current = *base;
                }
                _ => return Ok(None),
            }
        };
        if outer.is_empty() && packed_indices.is_empty() {
            return Ok(None);
        }
        let array = self
            .select_array_of(array_select, array_node)
            .cloned()
            .ok_or_else(|| format!("packed selection lost its array root in `{path}`"))?;
        let indices = array_indices
            .iter()
            .map(|index| self.lower_expr(path, *index))
            .collect::<Result<Vec<_>, _>>()?;
        let mut steps = Vec::new();
        let mut parent_width = array.elem_width;
        self.packed_selection_steps(
            path,
            array_node,
            Select::Elements(packed_indices),
            &mut parent_width,
            &mut steps,
        )?;
        for (base, select) in outer.into_iter().rev() {
            self.packed_selection_steps(path, base, select, &mut parent_width, &mut steps)?;
        }
        if let Some(guard) = self.record_select_guard(array_select)? {
            let element = IrLhs::ArrayElem {
                arr: self.reference_array(array.ir),
                indices,
                elem_sel: IrElemSel::Whole,
            };
            return Ok(Some(self.record_guarded_element_lhs(
                guard,
                element,
                array.elem_width,
                steps,
                false,
                self.source_location(node),
            )));
        }
        Ok(Some(IrLhs::ArrayElem {
            arr: self.reference_array(array.ir),
            indices,
            elem_sel: IrElemSel::PackedChain(steps),
        }))
    }

    /// Selectors of a chain rooted at a whole packed value: a signal, port,
    /// local, formal or parameter, never an unpacked-array element, member,
    /// container or modport expression port (each has its own projection).
    fn packed_value_selectors(&self, node: NodeId) -> Option<(NodeId, Vec<(NodeId, Select)>)> {
        let mut current = node;
        let mut selectors = Vec::new();
        loop {
            let (base, select) = match self.kind(current) {
                NodeKind::Expr(ExprKind::BitSelect { base, index }) => {
                    (*base, Select::Elements(vec![*index]))
                }
                NodeKind::Expr(ExprKind::ArraySelect { base, indices }) => {
                    (*base, Select::Elements(indices.clone()))
                }
                NodeKind::Expr(ExprKind::PartSelect { base, left, right }) => {
                    (*base, Select::Part(*left, *right))
                }
                NodeKind::Expr(ExprKind::IndexedPartSelect {
                    base,
                    base_expr,
                    width_expr,
                    neg,
                }) => (*base, Select::Indexed(*base_expr, *width_expr, *neg)),
                NodeKind::Expr(ExprKind::HierPath { .. }) => {
                    match self.packed_element_member_select(current) {
                        Some(select) => (select, Select::Member(current)),
                        None => break,
                    }
                }
                _ => break,
            };
            if self.array_of(base).is_some() || self.container_of(base).is_some() {
                return None;
            }
            selectors.push((base, select));
            current = base;
        }
        let root = current;
        let admitted = !selectors.is_empty()
            && matches!(
                self.kind(root),
                NodeKind::Net { .. }
                    | NodeKind::Var { .. }
                    | NodeKind::Expr(ExprKind::Ref { .. } | ExprKind::HierPath { .. })
            )
            && self.packed_member_info(root).is_none()
            && self.packed_parameter_member_info(root).is_none()
            && self.unpacked_member_info(root).is_none()
            && self.unpacked_aggregate_info(root).is_none()
            && !self.is_modport_select_root(root);
        admitted.then(|| {
            selectors.reverse();
            (root, selectors)
        })
    }

    /// Steps of a packed value chain whose outer dimension has elements wider
    /// than one bit, or `None` for a one-dimensional bit vector, whose bit
    /// coordinates the scalar select paths already use.
    fn packed_value_steps(
        &mut self,
        path: &str,
        root: NodeId,
        selectors: Vec<(NodeId, Select)>,
        root_width: u32,
    ) -> Result<Option<Vec<IrPackedSelect>>, String> {
        let Some(ranges) = self.packed_ranges_for_base(root) else {
            return Ok(None);
        };
        let Some(outer) = ranges.first().copied() else {
            return Ok(None);
        };
        let Ok(dim) = PackedSelectDim::new(root_width, Some(outer)) else {
            return Ok(None);
        };
        let through_member = selectors
            .iter()
            .any(|(_, select)| matches!(select, Select::Member(_)));
        if ranges.len() < 2 && dim.stride == 1 && !through_member {
            return Ok(None);
        }
        let mut width = root_width;
        let mut steps = Vec::new();
        for (base, select) in selectors {
            self.packed_selection_steps_folding(path, base, select, &mut width, &mut steps, true)?;
        }
        Ok(Some(steps))
    }

    /// Read a select chain over a multidimensional packed value, or a packed
    /// array of structures, as whole elements (IEEE 1800-2009 7.4.5).
    pub(in super::super) fn packed_value_read_ir(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<IrExpr>, String> {
        let Some((root, selectors)) = self.packed_value_selectors(node) else {
            return Ok(None);
        };
        let value = self.lower_expr(path, root)?;
        if value.is_real() {
            return Ok(None);
        }
        let through_member = selectors
            .iter()
            .any(|(_, select)| matches!(select, Select::Member(_)));
        let Some(steps) = self.packed_value_steps(path, root, selectors, value.width)? else {
            return Ok(None);
        };
        if let Some((lsb, width)) = constant_packed_span(value.width, &steps) {
            let selected = IrExpr::new(
                IrExprKind::PartSel {
                    base: Box::new(value),
                    left: i64::from(lsb) + i64::from(width) - 1,
                    right: i64::from(lsb),
                },
                width,
                false,
                None,
            );
            return Ok(Some(self.packed_value_select_type(
                node,
                selected,
                through_member,
            )));
        }
        // A part-select that is out of range or unknown reads X for the
        // missing bits (IEEE 1800-2009 11.5.1), as the scalar paths do.
        let value = steps.into_iter().fold(value, |value, step| {
            super::packed_formals::packed_step_read(value, step)
        });
        Ok(Some(self.packed_value_select_type(
            node,
            value,
            through_member,
        )))
    }

    /// An element select takes its element type's signedness (a select of
    /// `logic signed [3:0]` elements is signed); a part-select is unsigned.
    /// A member takes its declared signedness, and a value selected through
    /// a two-state member of a four-state structure reads as two-state
    /// (IEEE 1800-2009 7.2.1).
    fn packed_value_select_type(
        &self,
        node: NodeId,
        value: IrExpr,
        through_member: bool,
    ) -> IrExpr {
        let descriptor = self.query_descriptor(node);
        let element = matches!(
            self.kind(node),
            NodeKind::Expr(
                ExprKind::BitSelect { .. }
                    | ExprKind::ArraySelect { .. }
                    | ExprKind::HierPath { .. }
            )
        );
        let signed = element && descriptor.is_some_and(|descriptor| descriptor.info.signed);
        let value = if signed {
            let width = value.width;
            IrExpr::resize_to(value, width, true)
        } else {
            value
        };
        if through_member && descriptor.is_some_and(|descriptor| descriptor.two_state) {
            IrExpr::to_two_state(value)
        } else {
            value
        }
    }

    /// Write target of a select chain over a multidimensional packed value.
    /// An in-range constant chain is one part-select of the root signal and
    /// one runtime step is an indexed part-select, so nets, forces, NBAs and
    /// writer analysis see ordinary bit ranges. Longer runtime chains keep
    /// every intermediate bound as relative packed steps.
    pub(in super::super) fn packed_value_lhs(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<Lhs>, String> {
        let Some((root, selectors)) = self.packed_value_selectors(node) else {
            return Ok(None);
        };
        let target = self.analyze_lhs(path, root)?;
        let (root_width, two_state) = match &target {
            Lhs::Whole(info) if !info.real => (info.width, info.two_state),
            Lhs::WholeRef {
                width,
                two_state,
                shortreal: false,
                ..
            }
            | Lhs::Ref {
                width, two_state, ..
            } => (*width, *two_state),
            _ => return Ok(None),
        };
        let through_member = selectors
            .iter()
            .any(|(_, select)| matches!(select, Select::Member(_)));
        let Some(steps) = self.packed_value_steps(path, root, selectors, root_width)? else {
            return Ok(None);
        };
        // A write through a member converts to the selected bits' own state
        // domain: a two-state member of a four-state structure stores 0 for X.
        let two_state = if through_member {
            self.query_descriptor(node)
                .map_or(two_state, |descriptor| descriptor.two_state)
        } else {
            two_state
        };
        if let Lhs::Whole(info) = &target {
            if let Some((lsb, width)) = constant_packed_span(root_width, &steps) {
                return Ok(Some(Lhs::Part(
                    info.clone(),
                    i128::from(lsb) + i128::from(width) - 1,
                    i128::from(lsb),
                    two_state,
                )));
            }
            if let [step] = steps.as_slice() {
                return Ok(Some(Lhs::IdxPart(
                    info.clone(),
                    step.base.clone(),
                    lhs_integer_expr(i128::from(step.width)),
                    step.width,
                    false,
                    two_state,
                )));
            }
        }
        Ok(Some(Lhs::Canonical(IrLhs::PackedSelect {
            target: Box::new(self.lhs_to_ir(target)?),
            steps,
            signed: false,
            two_state,
        })))
    }

    /// The element select at the base of a member path over an element of a
    /// packed array (`ps[i].hi`, `w[i][j].in.lo`, `h.arr[i].hi`). Capture
    /// keeps that select as the path's first reference; paths over
    /// unpacked-array or container elements keep their own projections.
    pub(in super::super) fn packed_element_member_select(&self, node: NodeId) -> Option<NodeId> {
        let NodeKind::Expr(ExprKind::HierPath { parts, refs }) = self.kind(node) else {
            return None;
        };
        if parts.len() < 2 {
            return None;
        }
        let select = refs.first().copied().flatten()?;
        let mut current = select;
        loop {
            let base = match self.kind(current) {
                NodeKind::Expr(
                    ExprKind::BitSelect { base, .. } | ExprKind::ArraySelect { base, .. },
                ) => *base,
                _ if current == select => return None,
                _ => return Some(select),
            };
            if self.array_of(base).is_some() || self.container_of(base).is_some() {
                return None;
            }
            current = base;
        }
    }

    /// The selected element and the member's place inside it. Tagged-union
    /// members need their tag guard on the selected element, which this
    /// projection does not carry, so they reject.
    fn packed_element_member(&self, path: &str, node: NodeId) -> Result<PackedMember, String> {
        let (Some(select), NodeKind::Expr(ExprKind::HierPath { parts, .. })) =
            (self.packed_element_member_select(node), self.kind(node))
        else {
            return Err(format!(
                "packed-array element member lost its element select in `{path}`"
            ));
        };
        let members = &parts[1..];
        let label = members.join(".");
        let mut layout = self.db.aggregate_layout(select);
        for name in members {
            let Some(current) = layout else {
                break;
            };
            if current.kind == AggregateKind::TaggedUnion {
                return Err(format!(
                    "tagged-union member `{label}` of a packed-array element is not supported in `{path}`"
                ));
            }
            layout = current
                .members
                .iter()
                .find(|member| member.name == *name)
                .and_then(|member| member.aggregate_layout());
        }
        self.packed_member_layout(select, members).ok_or_else(|| {
            format!("member `{label}` of a packed-array element has no packed layout in `{path}`")
        })
    }

    pub(in super::super) fn packed_selection_steps(
        &mut self,
        path: &str,
        base: NodeId,
        select: Select,
        parent_width: &mut u32,
        steps: &mut Vec<IrPackedSelect>,
    ) -> Result<(), String> {
        self.packed_selection_steps_folding(path, base, select, parent_width, steps, false)
    }

    /// [`packed_selection_steps`](Self::packed_selection_steps), optionally
    /// folding constant selectors to constant offsets. Existing projection
    /// roots keep the offset arithmetic for their constant selectors: their
    /// selected-reference dependencies treat a constant step over an already
    /// selected actual as a nested slice.
    fn packed_selection_steps_folding(
        &mut self,
        path: &str,
        base: NodeId,
        select: Select,
        parent_width: &mut u32,
        steps: &mut Vec<IrPackedSelect>,
        fold: bool,
    ) -> Result<(), String> {
        // A modport expression port is numbered by its expression's own type.
        let dimensions = match self.db.packed_dimensions(base) {
            Some(dimensions) if self.modport_expression_target(base).is_none() => {
                dimensions.to_vec()
            }
            _ => self.packed_ranges_for_base(base).unwrap_or_default(),
        };
        match select {
            Select::Elements(indices) => {
                for (dimension, index) in indices.into_iter().enumerate() {
                    let dim =
                        PackedSelectDim::new(*parent_width, dimensions.get(dimension).copied())?;
                    // A constant label folds to its offset; an X/Z or runtime
                    // label keeps the arithmetic so its value propagates.
                    let base = match self.eval_bound_i128(index) {
                        Ok(label) if fold => lhs_integer_expr(dim.element(label)?.0),
                        _ => {
                            let index = self.lower_expr(path, index)?;
                            dim.lsb_expr(index, 0)?
                        }
                    };
                    steps.push(IrPackedSelect {
                        base,
                        width: dim.stride,
                    });
                    *parent_width = dim.stride;
                }
            }
            Select::Part(left, right) => {
                let dim = PackedSelectDim::new(*parent_width, dimensions.first().copied())?;
                let left = self.eval_bound_i128(left)?;
                let right = self.eval_bound_i128(right)?;
                let (lsb, width) = dim
                    .part(left, right)
                    .map_err(|error| format!("{error} in `{path}`"))?;
                let base = if fold {
                    lhs_integer_expr(lsb)
                } else {
                    dim.lsb_expr(lhs_integer_expr(right), 0)?
                };
                steps.push(IrPackedSelect { base, width });
                *parent_width = width;
            }
            Select::Indexed(base, width, negative) => {
                let dim = PackedSelectDim::new(*parent_width, dimensions.first().copied())?;
                let count = self.indexed_part_select_width(width, path)?;
                let width = dim.indexed_width(count)?;
                let base = match self.eval_bound_i128(base) {
                    Ok(label) if fold => lhs_integer_expr(dim.indexed(label, count, negative)?.0),
                    _ => {
                        let base = self.lower_expr(path, base)?;
                        dim.lsb_expr(base, dim.indexed_back(count, negative))?
                    }
                };
                steps.push(IrPackedSelect { base, width });
                *parent_width = width;
            }
            Select::Member(node) => {
                let member = self.packed_element_member(path, node)?;
                if member
                    .lsb
                    .checked_add(member.width)
                    .is_none_or(|end| end > *parent_width)
                {
                    return Err(format!(
                        "member `{}` lies outside its packed-array element in `{path}`",
                        member.name
                    ));
                }
                // A runtime element LSB absorbs the member offset, so
                // `ps[i].hi` stays one indexed step. The member lies inside
                // its element: an out-of-range or unknown element leaves the
                // member out of range or unknown too.
                match steps.last_mut() {
                    Some(last) if fold && !matches!(last.base.kind, IrExprKind::Const(_)) => {
                        last.base = offset_lsb(last.base.clone(), member.lsb)?;
                        last.width = member.width;
                    }
                    _ => steps.push(IrPackedSelect {
                        base: lhs_integer_expr(i128::from(member.lsb)),
                        width: member.width,
                    }),
                }
                *parent_width = member.width;
            }
        }
        Ok(())
    }
}

/// The LSB-relative span of a chain whose every step is a constant offset
/// inside its parent value. Any other chain keeps its steps so out-of-range
/// and unknown offsets clip at run time.
fn constant_packed_span(root_width: u32, steps: &[IrPackedSelect]) -> Option<(u32, u32)> {
    let mut parent = root_width;
    let mut lsb = 0u32;
    for step in steps {
        let IrExprKind::Const(offset) = &step.base.kind else {
            return None;
        };
        if offset.signed
            || offset.real.is_some()
            || offset.x.iter().chain(&offset.z).any(|word| *word != 0)
            || offset.bits.iter().skip(1).any(|word| *word != 0)
        {
            return None;
        }
        let offset = u32::try_from(offset.bits.first().copied().unwrap_or(0)).ok()?;
        if offset.checked_add(step.width)? > parent {
            return None;
        }
        lsb = lsb.checked_add(offset)?;
        parent = step.width;
    }
    Some((lsb, parent))
}
