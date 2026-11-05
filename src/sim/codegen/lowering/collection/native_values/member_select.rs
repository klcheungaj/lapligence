//! Run-time indices into fixed-array members of native records (SIM-007).
//!
//! A native value and a record element of container storage are trees of
//! items, so a selection below them locates its element with a position
//! computed at run time (`IrChandleExpr::ItemElement`): the code of one site
//! and the cost of one access do not grow with the array. An invalid index
//! locates a default scratch value, so reads see the default and writes are
//! dropped (SV 7.4.6).
//!
//! A module native record keeps each element of a member array as its own
//! object or signal, with its own change marker, so a run-time index selects
//! one leaf by comparing the index with every declared index; an index that
//! matches none (out of range, X or Z) reads the element default and writes
//! nothing. Small arrays compare in a chain; above
//! [`NATIVE_MEMBER_SELECT_LINEAR`] elements a balanced decision tree over the
//! declared indices keeps each access logarithmic. The generated code stays
//! linear in the array per access site, so such selections are bounded by
//! [`NATIVE_MEMBER_SELECT_LIMIT`]. An index with side effects is evaluated
//! once into a local before the comparisons.

use super::elements::{element_leaf_value, ElementSelection};
use super::nba::element_leaf_nba;
use super::*;
use crate::sim::codegen::lowering::collection::packed_elements::Select;
use crate::sim::codegen::lowering::collection::packed_formals::packed_step_read;
use crate::sim::codegen::lowering::containers::pattern_key_expr;
use crate::sim::ir::{
    IrElementRef, IrElementRefKind, IrElementRoot, IrLeafTarget, IrValueItemRoot, IrValueSlot,
};

/// Largest number of leaves a run-time selection of a module record compares:
/// each access site holds one branch per leaf, so larger selections are
/// rejected rather than expanded. Equal to the dense record member threshold,
/// above which module records already reject such a member.
pub(in crate::sim::codegen) const NATIVE_MEMBER_SELECT_LIMIT: usize = 4096;

/// Element count up to which a select compares in a chain; a chain is the
/// smaller code and needs at most this many comparisons.
const NATIVE_MEMBER_SELECT_LINEAR: usize = 8;

/// Largest member array selected whole by a run-time index: every branch
/// copies one whole record, in module records and native values alike.
const NATIVE_RECORD_SELECT_LIMIT: usize = 256;

/// The elements of a member array of records selected whole by a run-time
/// index, each as a transfer endpoint.
pub(in crate::sim::codegen) struct NativeRecordSelect {
    elements: Vec<(i32, NativeEndpoint)>,
    index: NodeId,
    hoist: bool,
}

/// One step of a selection below a record root.
#[derive(Clone)]
enum SelectStep {
    Member(String),
    Constant(i32),
    Run(NodeId),
}

/// The storage a selection starts from.
enum SelectRoot {
    Value(usize),
    Element(ElementSelection),
    Module(NodeId),
}

/// The value tree a located selection starts from.
#[derive(Clone)]
enum ItemRoot {
    Value(usize),
    Element(ElementSelection),
}

/// A subscript of one array level of a located selection.
#[derive(Clone)]
enum ItemOperand {
    Constant(i32),
    /// Position in [`NativeMemberSelect::indices`].
    Run(usize),
}

/// One level of the item tree between the root and the selected leaf.
#[derive(Clone)]
enum ItemLevel {
    Item(u32),
    Select {
        operands: Vec<ItemOperand>,
        dimensions: Vec<(i32, i32)>,
    },
}

/// How a selection finds its leaf.
enum NativeSelectForm {
    /// Located in a value tree: code per site is constant.
    Located {
        root: ItemRoot,
        levels: Vec<ItemLevel>,
        ty: IrClassFieldType,
    },
    /// The declared indices and leaf of every candidate, one index per entry
    /// of [`NativeMemberSelect::indices`], in declaration order.
    Leaves(Vec<(Vec<i32>, NativeEndpointLeaf)>),
}

/// One leaf of a record selected through run-time indices (`r.a[k]`,
/// `m.e[k].a[j]`, `nb[k].a[j]`, `r.e[k].b[2]`).
pub(in crate::sim::codegen) struct NativeMemberSelect {
    form: NativeSelectForm,
    /// The run-time indices, outermost first.
    indices: Vec<NodeId>,
    /// Packed selects inside the leaf, innermost first.
    selects: Vec<(NodeId, Select)>,
}

/// The indices of one evaluation of a selection.
struct LoweredSelect {
    element: Option<IrValueSlot>,
    indices: Vec<IrExpr>,
}

/// The constant steps of a member/index path.
fn path_steps(path: Vec<AggregatePathPart>) -> Vec<SelectStep> {
    path.into_iter()
        .map(|part| match part {
            AggregatePathPart::Member(name) => SelectStep::Member(name),
            AggregatePathPart::Index(index) => SelectStep::Constant(index),
        })
        .collect()
}

/// The declared indices `steps` assign to a leaf at `path`, and how many steps
/// the leaf consumes; `None` when the leaf does not match or no run-time step
/// selects it.
fn match_leaf_path(path: &[AggregatePathPart], steps: &[SelectStep]) -> Option<Vec<i32>> {
    if path.len() > steps.len() {
        return None;
    }
    let mut keys = Vec::new();
    for (part, step) in path.iter().zip(steps) {
        match (part, step) {
            (AggregatePathPart::Member(name), SelectStep::Member(member)) if name == member => {}
            (AggregatePathPart::Index(index), SelectStep::Constant(value)) if index == value => {}
            (AggregatePathPart::Index(index), SelectStep::Run(_)) => keys.push(*index),
            _ => return None,
        }
    }
    Some(keys)
}

/// The zero-based flat position of constant `indices` in an array of
/// `dimensions`, or `None` when one is out of range.
fn constant_position(indices: &[i32], dimensions: &[(i32, i32)]) -> Option<u32> {
    let mut position = 0u64;
    for (index, (left, right)) in indices.iter().zip(dimensions) {
        let (low, high) = (*left.min(right), *left.max(right));
        if *index < low || *index > high {
            return None;
        }
        let offset = i64::from(*index).abs_diff(i64::from(*left));
        position = position * (u64::from(left.abs_diff(*right)) + 1) + offset;
    }
    u32::try_from(position).ok()
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

    /// The root and steps of the selection `node`, however its frontend
    /// shape nests detached arrays, member paths and bit selects.
    fn selected_steps(&self, node: NodeId) -> Option<(SelectRoot, Vec<SelectStep>)> {
        let node = self.p30_unwrap_cast(node);
        if let Some(selection) = self.record_element_of(node) {
            return Some((SelectRoot::Element(selection), Vec::new()));
        }
        if let Ok(Some((value, path))) = self.native_path_of(node) {
            return Some((SelectRoot::Value(value), path_steps(path)));
        }
        if self.unpacked_aggregates.contains_key(&node) {
            return Some((SelectRoot::Module(node), Vec::new()));
        }
        let step = |index: NodeId| match self
            .eval_bound_i128(index)
            .ok()
            .and_then(|index| i32::try_from(index).ok())
        {
            Some(index) => SelectStep::Constant(index),
            None => SelectStep::Run(index),
        };
        match self.kind(node) {
            NodeKind::Expr(ExprKind::HierPath { parts, refs }) => {
                if let Some(Some(first)) = refs.first() {
                    if matches!(
                        self.kind(*first),
                        NodeKind::Expr(ExprKind::ArraySelect { .. } | ExprKind::BitSelect { .. })
                    ) {
                        let (root, mut steps) = self.selected_steps(*first)?;
                        steps.extend(
                            parts
                                .iter()
                                .skip(1)
                                .filter(|part| !part.is_empty())
                                .cloned()
                                .map(SelectStep::Member),
                        );
                        return Some((root, steps));
                    }
                }
            }
            NodeKind::Expr(ExprKind::ArraySelect { base, indices }) => {
                let (root, mut steps) = match self.db.array_select_path(node) {
                    Some((owner, members)) => {
                        let (root, mut steps) = self.selected_steps(owner)?;
                        steps.extend(members.iter().cloned().map(SelectStep::Member));
                        (root, steps)
                    }
                    None => self.selected_steps(*base)?,
                };
                steps.extend(indices.iter().map(|index| step(*index)));
                return Some((root, steps));
            }
            NodeKind::Expr(ExprKind::BitSelect { base, index }) => {
                let (root, mut steps) = self.selected_steps(*base)?;
                steps.push(step(*index));
                return Some((root, steps));
            }
            _ => {}
        }
        let (root, path) = self.unpacked_path_for_expr(node)?;
        self.unpacked_aggregates
            .contains_key(&root)
            .then(|| (SelectRoot::Module(root), path_steps(path)))
    }

    /// The levels from `element` to the leaf `steps` name, the leaf type and
    /// the number of steps that select inside the leaf. `None` for a whole
    /// sub-record, a container member or a selection without a run-time
    /// index above its leaf.
    fn located_levels(
        element: &IrContainerElement,
        steps: &[SelectStep],
        indices: &mut Vec<NodeId>,
    ) -> Result<Option<(Vec<ItemLevel>, IrClassFieldType, usize)>, String> {
        let mut levels = Vec::new();
        let mut level = element;
        let mut used = 0;
        loop {
            match level {
                IrContainerElement::Aggregate { members, .. } => {
                    let Some(SelectStep::Member(name)) = steps.get(used) else {
                        return Ok(None);
                    };
                    let Some(position) = members.iter().position(|member| member.name == *name)
                    else {
                        return Ok(None);
                    };
                    levels.push(ItemLevel::Item(
                        u32::try_from(position).map_err(|_| "native record is too wide")?,
                    ));
                    level = &members[position].element;
                    used += 1;
                }
                IrContainerElement::FixedArray {
                    dimensions,
                    element,
                } => {
                    let Some(subscripts) = steps.get(used..used + dimensions.len()) else {
                        return Ok(None);
                    };
                    let mut operands = Vec::with_capacity(subscripts.len());
                    for subscript in subscripts {
                        operands.push(match subscript {
                            SelectStep::Constant(index) => ItemOperand::Constant(*index),
                            SelectStep::Run(index) => {
                                indices.push(*index);
                                ItemOperand::Run(indices.len() - 1)
                            }
                            SelectStep::Member(_) => return Ok(None),
                        });
                    }
                    let constants = operands
                        .iter()
                        .map(|operand| match operand {
                            ItemOperand::Constant(index) => Some(*index),
                            ItemOperand::Run(_) => None,
                        })
                        .collect::<Option<Vec<_>>>();
                    levels.push(
                        match constants.and_then(|list| constant_position(&list, dimensions)) {
                            Some(position) => ItemLevel::Item(position),
                            None => ItemLevel::Select {
                                operands,
                                dimensions: dimensions.clone(),
                            },
                        },
                    );
                    level = element;
                    used += dimensions.len();
                }
                IrContainerElement::Container { .. } => return Ok(None),
                leaf => {
                    let ty = leaf_ty(leaf)?;
                    let inside = steps.len() - used;
                    let selected = levels
                        .iter()
                        .any(|level| matches!(level, ItemLevel::Select { .. }));
                    if !selected
                        || (inside > 0 && !matches!(ty, IrClassFieldType::Packed { .. }))
                        || steps[used..]
                            .iter()
                            .any(|step| matches!(step, SelectStep::Member(_)))
                    {
                        return Ok(None);
                    }
                    return Ok(Some((levels, ty, inside)));
                }
            }
        }
    }

    /// The packed selects of the last `inside` subscripts of the selection
    /// `node`, innermost first.
    fn inside_selects(&self, node: NodeId, mut inside: usize) -> Option<Vec<(NodeId, Select)>> {
        let mut selects = Vec::new();
        let mut current = node;
        while inside > 0 {
            match self.kind(current) {
                NodeKind::Expr(ExprKind::BitSelect { base, index }) => {
                    selects.push((*base, Select::Elements(vec![*index])));
                    inside -= 1;
                    current = *base;
                }
                NodeKind::Expr(ExprKind::ArraySelect { base, indices }) => {
                    let count = inside.min(indices.len());
                    let trailing = indices[indices.len() - count..].to_vec();
                    selects.push((*base, Select::Elements(trailing)));
                    inside -= count;
                    current = *base;
                }
                _ => return None,
            }
        }
        selects.reverse();
        Some(selects)
    }

    /// `r.a[k]` with a non-constant `k`, and any selection that has such an
    /// index above one leaf of a native record: one leaf, or a select inside
    /// a packed leaf.
    pub(in crate::sim::codegen) fn native_member_select(
        &self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<NativeMemberSelect>, String> {
        let mut leaf_node = self.p30_unwrap_cast(node);
        let mut part_selects = Vec::new();
        loop {
            let (base, select) = match self.kind(leaf_node) {
                NodeKind::Expr(ExprKind::PartSelect { base, left, right }) => {
                    (*base, Select::Part(*left, *right))
                }
                NodeKind::Expr(ExprKind::IndexedPartSelect {
                    base,
                    base_expr,
                    width_expr,
                    neg,
                }) => (*base, Select::Indexed(*base_expr, *width_expr, *neg)),
                _ => break,
            };
            part_selects.push((base, select));
            leaf_node = base;
        }
        let Some((root, steps)) = self.selected_steps(leaf_node) else {
            return Ok(None);
        };
        if !steps.iter().any(|step| matches!(step, SelectStep::Run(_))) {
            return Ok(None);
        }
        let mut indices = Vec::new();
        let (form, inside) = match root {
            SelectRoot::Value(value) => {
                let ty = self.model.native_values[value].ty;
                let element = self.model.native_types[ty].clone();
                let Some((levels, ty, inside)) =
                    Self::located_levels(&element, &steps, &mut indices)?
                else {
                    return Ok(None);
                };
                (
                    NativeSelectForm::Located {
                        root: ItemRoot::Value(value),
                        levels,
                        ty,
                    },
                    inside,
                )
            }
            SelectRoot::Element(selection) => {
                let Some(element) =
                    self.container_element_type(selection.container, selection.depth())
                else {
                    return Ok(None);
                };
                let Some((levels, ty, inside)) =
                    Self::located_levels(&element, &steps, &mut indices)?
                else {
                    return Ok(None);
                };
                (
                    NativeSelectForm::Located {
                        root: ItemRoot::Element(selection),
                        levels,
                        ty,
                    },
                    inside,
                )
            }
            SelectRoot::Module(root) => {
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
                let mut elements = Vec::new();
                let mut consumed = 0;
                for leaf in &storage.leaves {
                    if let Some(keys) = match_leaf_path(&leaf.path, &steps) {
                        consumed = leaf.path.len();
                        elements.push((keys, NativeEndpointLeaf::Module(Box::new(leaf.clone()))));
                    }
                }
                indices.extend(
                    steps[..consumed.min(steps.len())]
                        .iter()
                        .filter_map(|step| match step {
                            SelectStep::Run(index) => Some(*index),
                            _ => None,
                        }),
                );
                // A run-time subscript inside the leaf only is a packed
                // select, which the signal paths lower.
                if elements.is_empty() || indices.is_empty() {
                    return Ok(None);
                }
                if elements.len() > NATIVE_MEMBER_SELECT_LIMIT {
                    return Err(format!(
                        "run-time index into a native record member array of {} elements in `{path}` is not supported; at most {NATIVE_MEMBER_SELECT_LIMIT} are",
                        elements.len()
                    ));
                }
                let inside = steps.len() - consumed;
                (NativeSelectForm::Leaves(elements), inside)
            }
        };
        let Some(mut selects) = self.inside_selects(leaf_node, inside) else {
            return Ok(None);
        };
        part_selects.reverse();
        selects.append(&mut part_selects);
        Ok(Some(NativeMemberSelect {
            form,
            indices,
            selects,
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
        let NativeSelectForm::Leaves(elements) = &select.form else {
            return Ok(None);
        };
        let [index] = select.indices[..] else {
            return Ok(None);
        };
        if !select.selects.is_empty() {
            return Ok(None);
        }
        let mut leaves = Vec::with_capacity(elements.len());
        for (declared, leaf) in elements {
            let ([declared], NativeEndpointLeaf::Module(leaf)) = (&declared[..], leaf) else {
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
        let index = self.lower_container_index(path, index)?;
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
        let hoist = select.hoist || self.native_select_at_issue;
        let (index, width) =
            self.member_select_index(path, select.index, hoist, &mut statements)?;
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
        let hoist = select.hoist || self.native_select_at_issue;
        let (index, width) =
            self.member_select_index(path, select.index, hoist, &mut statements)?;
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

    /// Evaluate the indices of `select` for one use. An index with side
    /// effects runs once, into a local declared by `prelude`.
    fn lower_select_indices(
        &mut self,
        path: &str,
        select: &NativeMemberSelect,
        prelude: &mut Vec<IrStmt>,
    ) -> Result<LoweredSelect, String> {
        let element = match &select.form {
            NativeSelectForm::Located {
                root: ItemRoot::Element(selection),
                ..
            } => Some(
                if selection.side_effect_free(self) && !self.native_select_at_issue {
                    self.lower_element_slot(path, selection)?
                } else {
                    self.element_nba_slot(path, selection, prelude)?
                },
            ),
            _ => None,
        };
        let mut indices = Vec::with_capacity(select.indices.len());
        for node in &select.indices {
            let hoist = self.native_select_at_issue || !self.side_effect_free(*node);
            let mut index = self.lower_container_index(path, *node)?;
            if matches!(select.form, NativeSelectForm::Leaves(_)) {
                let width = index.width.max(32) + 1;
                index = IrExpr::convert_to(index, width, true);
            }
            if hoist {
                let name = format!("_llg_member_index_{}", self.native_copy_sequence);
                self.native_copy_sequence += 1;
                let (width, signed) = (index.width, index.signed);
                prelude.push(IrStmt::DeclLocal {
                    name: name.clone(),
                    width,
                    signed,
                    two_state: false,
                    init: Some(Box::new(index)),
                });
                index = IrExpr::new(IrExprKind::LocalRead(name), width, signed, None);
            }
            indices.push(index);
        }
        Ok(LoweredSelect { element, indices })
    }

    /// Packed selects of `select` inside a leaf of `width` bits, as steps.
    fn select_steps(
        &mut self,
        path: &str,
        select: &NativeMemberSelect,
        mut width: u32,
    ) -> Result<Vec<crate::sim::ir::IrPackedSelect>, String> {
        let mut steps = Vec::new();
        for (base, part) in &select.selects {
            self.packed_selection_steps_folding(
                path,
                *base,
                part.clone(),
                &mut width,
                &mut steps,
                true,
            )?;
        }
        Ok(steps)
    }

    /// The locator of a located selection and the items below it. A `write`
    /// locator publishes a store to a container element.
    fn item_locator(
        &mut self,
        root: &ItemRoot,
        levels: &[ItemLevel],
        lowered: &LoweredSelect,
        write: bool,
    ) -> Result<(IrChandleExpr, Vec<u32>), String> {
        let mut receiver = match (root, &lowered.element) {
            (ItemRoot::Value(value), _) => IrChandleExpr::NativeRoot(*value),
            (ItemRoot::Element(selection), Some(IrValueSlot::Element { indices, key })) => {
                IrChandleExpr::ContainerElement {
                    container: selection.container,
                    indices: indices.clone(),
                    key: key.clone().map(Box::new),
                    write,
                }
            }
            _ => return Err("element selection has no slot".to_owned()),
        };
        let mut items = Vec::new();
        for level in levels {
            match level {
                ItemLevel::Item(item) => items.push(*item),
                ItemLevel::Select {
                    operands,
                    dimensions,
                } => {
                    let indices = operands
                        .iter()
                        .map(|operand| match operand {
                            ItemOperand::Constant(index) => {
                                pattern_key_expr(i128::from(*index), 33, true, false)
                            }
                            ItemOperand::Run(position) => lowered.indices[*position].clone(),
                        })
                        .collect();
                    receiver = IrChandleExpr::ItemElement {
                        base: Box::new(receiver),
                        items: std::mem::take(&mut items),
                        indices,
                        dimensions: dimensions.clone(),
                    };
                }
            }
        }
        Ok((receiver, items))
    }

    /// A fresh access to the leaf of a located selection.
    fn item_access(
        &mut self,
        root: &ItemRoot,
        levels: &[ItemLevel],
        ty: IrClassFieldType,
        lowered: &LoweredSelect,
        write: bool,
    ) -> Result<String, String> {
        let (receiver, item_path) = self.item_locator(root, levels, lowered, write)?;
        let name = format!("_llg_access_{}", self.model.native_accesses.len());
        self.model
            .native_accesses
            .push(crate::sim::ir::IrNativeAccess {
                name: name.clone(),
                receiver,
                kind: IrNativeAccessKind::ElementItem { ty },
                site: None,
                item_path,
                function: self.cur_fn_ir,
            });
        Ok(name)
    }

    /// The selected value once `lowered` holds its indices.
    fn select_value(
        &mut self,
        path: &str,
        select: &NativeMemberSelect,
        lowered: &LoweredSelect,
    ) -> Result<LeafValue, String> {
        match &select.form {
            NativeSelectForm::Located { root, levels, ty } => {
                let name = self.item_access(root, levels, *ty, lowered, false)?;
                let value = element_leaf_value(name, *ty);
                let LeafValue::Packed(packed) = value else {
                    return Ok(value);
                };
                let steps = self.select_steps(path, select, packed.width)?;
                Ok(LeafValue::Packed(
                    steps.into_iter().fold(packed, packed_step_read),
                ))
            }
            NativeSelectForm::Leaves(elements) => {
                let mut values = Vec::with_capacity(elements.len());
                for (declared, leaf) in elements {
                    let mut value = self.endpoint_leaf_read(leaf)?;
                    if let LeafValue::Packed(packed) = value {
                        let steps = self.select_steps(path, select, packed.width)?;
                        value = LeafValue::Packed(steps.into_iter().fold(packed, packed_step_read));
                    }
                    values.push((declared.clone(), value));
                }
                let two_state = match elements.first().map(|(_, leaf)| leaf) {
                    Some(NativeEndpointLeaf::Module(leaf)) => leaf.member.two_state,
                    Some(NativeEndpointLeaf::Container(_)) => {
                        return Err(CONTAINER_LEAF_UNSUPPORTED.to_owned())
                    }
                    _ => false,
                };
                let default = match values.first().map(|(_, value)| value) {
                    Some(LeafValue::Container(_)) => {
                        return Err(CONTAINER_LEAF_UNSUPPORTED.to_owned())
                    }
                    Some(LeafValue::String(_)) => {
                        LeafValue::String(IrStringExpr::Literal(Vec::new()))
                    }
                    Some(LeafValue::Chandle(_)) => LeafValue::Chandle(IrChandleExpr::Null),
                    Some(LeafValue::Real(_)) => LeafValue::Real(real_literal_expr(0.0)),
                    Some(LeafValue::Packed(value)) => {
                        let (width, signed) = (value.width, value.signed);
                        let mut default = IrConst::integral_default(width, two_state);
                        default.signed = signed;
                        LeafValue::Packed(IrExpr::new(
                            IrExprKind::Const(default),
                            width,
                            signed,
                            None,
                        ))
                    }
                    None => return Err("member array selection has no elements".to_owned()),
                };
                values.sort_by(|a, b| a.0.cmp(&b.0));
                member_select_nest(
                    &values,
                    &lowered.indices,
                    0,
                    &default,
                    &mut |test, then, els| leaf_conditional(path, test, then, els),
                )
            }
        }
    }

    /// The selected element as a string, packed, real or handle value; an
    /// unmatched index reads the element type's default.
    pub(in crate::sim::codegen) fn native_member_select_read(
        &mut self,
        path: &str,
        select: &NativeMemberSelect,
    ) -> Result<IrNativeLeafExpr, String> {
        let mut prelude = Vec::new();
        let lowered = self.lower_select_indices(path, select, &mut prelude)?;
        let value = self
            .select_value(path, select, &lowered)?
            .into_leaf_expr()?;
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
        let first = || sequence(pattern_key_expr(1, 1, false, false));
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

    /// The kind of value the selected leaf holds.
    fn select_leaf_type(&self, select: &NativeMemberSelect) -> Result<IrClassFieldType, String> {
        match &select.form {
            NativeSelectForm::Located { ty, .. } => Ok(*ty),
            NativeSelectForm::Leaves(elements) => {
                let (_, first) = elements
                    .first()
                    .ok_or("member array selection has no elements")?;
                match first {
                    NativeEndpointLeaf::Container(_) => Err(CONTAINER_LEAF_UNSUPPORTED.to_owned()),
                    NativeEndpointLeaf::Value(_, leaf) => Ok(leaf.ty),
                    NativeEndpointLeaf::Module(leaf) => Ok(match leaf.object {
                        Some(object) => {
                            match self.model.objects[self.reference_object(object)].ty {
                                IrObjectType::String => IrClassFieldType::String,
                                _ => IrClassFieldType::Chandle,
                            }
                        }
                        None => IrClassFieldType::Real { shortreal: false },
                    }),
                }
            }
        }
    }

    /// The located nonblocking write of `value`: the position is fixed at
    /// issue and the item found at commit, so a whole write of the root in
    /// between cannot leave it behind (SV 10.4.2).
    #[allow(clippy::too_many_arguments)]
    fn located_nba(
        &mut self,
        path: &str,
        select: &NativeMemberSelect,
        root: &ItemRoot,
        levels: &[ItemLevel],
        ty: IrClassFieldType,
        lowered: &LoweredSelect,
        value: LeafValue,
        ticks: IrDelay,
    ) -> Result<Vec<IrStmt>, String> {
        let item_root = match (root, &lowered.element) {
            (ItemRoot::Value(native), _) => {
                self.require_persistent_native(path, *native)?;
                IrValueItemRoot::Value(*native)
            }
            (ItemRoot::Element(selection), Some(IrValueSlot::Element { indices, key })) => {
                self.require_persistent_view(path, selection.container)?;
                IrValueItemRoot::Element(IrChandleExpr::ContainerElement {
                    container: selection.container,
                    indices: indices.clone(),
                    key: key.clone().map(Box::new),
                    write: false,
                })
            }
            _ => return Err("element selection has no slot".to_owned()),
        };
        let mut items = Vec::with_capacity(levels.len());
        let mut selects = Vec::new();
        for level in levels {
            match level {
                ItemLevel::Item(item) => items.push(*item),
                ItemLevel::Select {
                    operands,
                    dimensions,
                } => {
                    selects.push(crate::sim::ir::IrItemSelect {
                        slot: items.len(),
                        indices: operands
                            .iter()
                            .map(|operand| match operand {
                                ItemOperand::Constant(index) => {
                                    pattern_key_expr(i128::from(*index), 33, true, false)
                                }
                                ItemOperand::Run(position) => lowered.indices[*position].clone(),
                            })
                            .collect(),
                        dimensions: dimensions.clone(),
                    });
                    items.push(0);
                }
            }
        }
        let name = self.item_access(root, levels, ty, lowered, false)?;
        let write = match (ty, select.selects.is_empty()) {
            (
                IrClassFieldType::Packed {
                    width,
                    two_state,
                    signed,
                },
                false,
            ) => {
                let steps = self.select_steps(path, select, width)?;
                let LeafValue::Packed(value) = value else {
                    return Err("element member write pairs different leaf kinds".to_owned());
                };
                let lhs = IrLhs::PackedSelect {
                    target: Box::new(IrLhs::WholeRef {
                        addr: format!("&{name}"),
                        width,
                        signed,
                        two_state,
                        shortreal: false,
                    }),
                    steps,
                    signed: false,
                    two_state,
                };
                let rhs = apply_lhs_assignment_context(&self.model, &lhs, value);
                IrStmt::DelayedAssign { lhs, rhs, ticks }
            }
            _ => element_leaf_nba(&self.model, name, ty, value, ticks)?,
        };
        Ok(vec![
            IrStmt::Container(Box::new(IrContainerStmt::NbaLocate {
                root: item_root,
                items,
                selects,
            })),
            write,
            IrStmt::Container(Box::new(IrContainerStmt::NbaLocateEnd)),
        ])
    }

    /// Store `value`, which is already captured, in the selected leaf.
    fn select_store(
        &mut self,
        path: &str,
        select: &NativeMemberSelect,
        lowered: &LoweredSelect,
        value: LeafValue,
        nba: bool,
        statements: &mut Vec<IrStmt>,
    ) -> Result<(), String> {
        match &select.form {
            NativeSelectForm::Located { root, levels, ty } if nba => {
                let writes = self.located_nba(
                    path,
                    select,
                    root,
                    levels,
                    *ty,
                    lowered,
                    value,
                    IrDelay::Constant(0),
                )?;
                statements.extend(writes);
            }
            NativeSelectForm::Located { root, levels, ty } => {
                let name = self.item_access(root, levels, *ty, lowered, true)?;
                statements.push(match (ty, value) {
                    (IrClassFieldType::String, LeafValue::String(value)) => {
                        IrStmt::Object(Box::new(IrObjectStmt::StringAssignLocal(name, value)))
                    }
                    (IrClassFieldType::Chandle, LeafValue::Chandle(value)) => {
                        IrStmt::Object(Box::new(IrObjectStmt::ChandleAssignLocal(name, value)))
                    }
                    (ty, LeafValue::Packed(value) | LeafValue::Real(value)) => {
                        let (width, signed, two_state, shortreal) = match *ty {
                            IrClassFieldType::Packed {
                                width,
                                signed,
                                two_state,
                            } => (width, signed, two_state, false),
                            IrClassFieldType::Real { shortreal } => (0, false, false, shortreal),
                            _ => return Err("selected leaf pairs different kinds".to_owned()),
                        };
                        let mut lhs = IrLhs::WholeRef {
                            addr: format!("&{name}"),
                            width,
                            signed,
                            two_state,
                            shortreal,
                        };
                        if !select.selects.is_empty() {
                            lhs = IrLhs::PackedSelect {
                                target: Box::new(lhs),
                                steps: self.select_steps(path, select, width)?,
                                signed: false,
                                two_state,
                            };
                        }
                        IrStmt::Assign {
                            rhs: apply_lhs_assignment_context(&self.model, &lhs, value),
                            lhs,
                            nba: false,
                        }
                    }
                    _ => return Err("selected leaf pairs different kinds".to_owned()),
                });
            }
            NativeSelectForm::Leaves(elements) => {
                let mut branches = Vec::with_capacity(elements.len());
                for (declared, leaf) in elements {
                    let write = if select.selects.is_empty() {
                        self.endpoint_leaf_write(path, leaf, value.clone(), nba)?
                    } else {
                        let NativeEndpointLeaf::Module(leaf) = leaf else {
                            return Err("a select needs a packed member".to_owned());
                        };
                        let LeafValue::Packed(value) = value.clone() else {
                            return Err("a select needs a packed value".to_owned());
                        };
                        let width = leaf.member.ty.width.ok_or("member has no width")?;
                        let steps = self.select_steps(path, select, width)?;
                        let lhs = match self.aggregate_leaf_lhs(leaf)? {
                            IrLhs::ArrayElem { arr, indices, .. } => IrLhs::ArrayElem {
                                arr,
                                indices,
                                elem_sel: IrElemSel::PackedChain(steps),
                            },
                            IrLhs::Part(signal, ..) => IrLhs::PackedSelect {
                                target: Box::new(IrLhs::Whole(signal)),
                                steps,
                                signed: false,
                                two_state: leaf.member.two_state,
                            },
                            _ => return Err("a select needs a packed member".to_owned()),
                        };
                        IrStmt::Assign {
                            rhs: apply_lhs_assignment_context(&self.model, &lhs, value),
                            lhs,
                            nba,
                        }
                    };
                    branches.push((declared.clone(), write));
                }
                branches.sort_by(|a, b| a.0.cmp(&b.0));
                let branches = branches
                    .into_iter()
                    .map(|(declared, write)| (declared, Some(write)))
                    .collect::<Vec<_>>();
                statements.extend(member_select_nest(
                    &branches,
                    &lowered.indices,
                    0,
                    &None,
                    &mut |test, then, els: Option<IrStmt>| {
                        Ok(Some(IrStmt::If {
                            cond: test,
                            then_: then.into_iter().collect(),
                            els: els.map(|statement| vec![statement]),
                            check: IrUniquePriorityCheck::None,
                        }))
                    },
                )?);
            }
        }
        Ok(())
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
        let ty = self.select_leaf_type(select)?;
        let sequence = self.native_copy_sequence;
        self.native_copy_sequence += 1;
        let mut statements = Vec::new();
        // An event-controlled write evaluates its selectors first.
        let early = self
            .native_select_at_issue
            .then(|| self.lower_select_indices(path, select, &mut statements))
            .transpose()?;
        let value = self.native_leaf_source(path, ty, rhs)?;
        let value = capture_leaf(
            value,
            format!("_llg_member_select_{sequence}"),
            &mut statements,
        );
        // Otherwise the value is evaluated before a hoisted index.
        let lowered = match early {
            Some(lowered) => lowered,
            None => self.lower_select_indices(path, select, &mut statements)?,
        };
        self.select_store(path, select, &lowered, value, nba, &mut statements)?;
        Ok(IrStmt::Block(statements))
    }

    /// Evaluate the selection once and store the value `update` computes
    /// from the current one in the same element (SV 11.4.1), returning the
    /// old and the new value.
    fn select_update(
        &mut self,
        path: &str,
        select: &NativeMemberSelect,
        update: &mut dyn FnMut(&mut Self, IrExpr) -> Result<IrExpr, String>,
        statements: &mut Vec<IrStmt>,
    ) -> Result<(LeafValue, LeafValue), String> {
        let lowered = self.lower_select_indices(path, select, statements)?;
        let current = self.select_value(path, select, &lowered)?;
        let sequence = self.native_copy_sequence;
        self.native_copy_sequence += 2;
        let current = capture_leaf(
            current,
            format!("_llg_member_select_{sequence}"),
            statements,
        );
        let (LeafValue::Packed(old) | LeafValue::Real(old)) = current.clone() else {
            return Err(format!(
                "assignment operator on a string or handle member in `{path}` is illegal"
            ));
        };
        let value = update(self, old)?;
        let value = capture_leaf(
            if value.is_real() {
                LeafValue::Real(value)
            } else {
                LeafValue::Packed(value)
            },
            format!("_llg_member_select_{}", sequence + 1),
            statements,
        );
        self.select_store(path, select, &lowered, value.clone(), false, statements)?;
        Ok((current, value))
    }

    /// `r.a[k] += v;`, `r.a[k]++;` as a statement.
    pub(in crate::sim::codegen) fn native_member_select_update(
        &mut self,
        path: &str,
        select: &NativeMemberSelect,
        update: &mut dyn FnMut(&mut Self, IrExpr) -> Result<IrExpr, String>,
    ) -> Result<IrStmt, String> {
        let mut statements = Vec::new();
        self.select_update(path, select, update, &mut statements)?;
        Ok(IrStmt::Block(statements))
    }

    /// `x = r.a[k]++`: the update as an expression, yielding the old value
    /// when `post` and else the new one (SV 11.4.2).
    pub(in crate::sim::codegen) fn native_member_select_mutation(
        &mut self,
        path: &str,
        select: &NativeMemberSelect,
        post: bool,
        update: &mut dyn FnMut(&mut Self, IrExpr) -> Result<IrExpr, String>,
    ) -> Result<IrExpr, String> {
        let mut statements = Vec::new();
        let (old, new) = self.select_update(path, select, update, &mut statements)?;
        let (LeafValue::Packed(value) | LeafValue::Real(value)) = (if post { old } else { new })
        else {
            return Err("selected value is not packed or real".to_owned());
        };
        let (width, signed) = (value.width, value.signed);
        Ok(IrExpr::new(
            IrExprKind::Sequence(Box::new(crate::sim::ir::IrSequenceExpr {
                statements,
                value,
            })),
            width,
            signed,
            None,
        ))
    }
}

/// [`member_select_tree`] over one index per level: `entries` are sorted by
/// their declared indices and each tree selects a subtree of the next index.
fn member_select_nest<V: Clone>(
    entries: &[(Vec<i32>, V)],
    indices: &[IrExpr],
    level: usize,
    otherwise: &V,
    join: &mut impl FnMut(IrExpr, V, V) -> Result<V, String>,
) -> Result<V, String> {
    let index = &indices[level];
    let last = level + 1 == indices.len();
    let mut groups: Vec<(i32, V)> = Vec::new();
    let mut start = 0;
    while start < entries.len() {
        let key = entries[start].0[level];
        let end = start
            + entries[start..]
                .iter()
                .take_while(|entry| entry.0[level] == key)
                .count();
        let selected = if last {
            entries[start].1.clone()
        } else {
            member_select_nest(&entries[start..end], indices, level + 1, otherwise, join)?
        };
        groups.push((key, selected));
        start = end;
    }
    member_select_tree(&groups, index, index.width, otherwise, join)
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
