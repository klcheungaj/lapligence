//! Native record `ref` formals bound to declaration-owned records (SIM-008).
//!
//! A subroutine record actual passes its value's address to a native record
//! `ref` formal. A module, static or procedural-block record has no such
//! value: it keeps one storage cell per leaf (`collect_aggregate`). A call
//! passing one, or a constant member/index selection of one, therefore calls
//! a specialization of the subroutine whose body names that record's leaves
//! directly ([`IrCallArg::NativeRefBound`]). Writes through the formal are
//! ordinary leaf writes, so they are visible at once, publish change markers
//! and wake `@`/`always_comb` readers; reads see writes other processes make
//! while the callee is suspended; two formals bound to one record share its
//! leaves (SV 13.5.2). One specialization exists per subroutine, instance and
//! set of bound records, so code grows with distinct actuals, not with calls,
//! and a recursive or forwarding call with the same binding reuses it.
//!
//! An element of an array, a member of a subroutine record and members of
//! such elements have a value but no stable address: those actuals lower to an
//! [`IrElementRef`] walking from the root through member and index steps.

use super::*;
use crate::sim::ir::{IrElementRef, IrElementRefKind, IrElementRoot, IrElementStep, IrValueSlot};

/// Whether two record types are the same nominal type, as the frontend
/// already requires of a `ref` actual (SV 6.22.1); a mismatch here means the
/// selection resolved to other storage than the actual names.
fn same_record_type(left: &TypeDescriptor, right: &TypeDescriptor) -> bool {
    let (TypeShape::Aggregate(left_layout), TypeShape::Aggregate(right_layout)) =
        (&left.shape, &right.shape)
    else {
        return false;
    };
    let same_identity = match (&left_layout.type_identity, &right_layout.type_identity) {
        (Some(left), Some(right)) => left == right,
        (None, None) => left.id == right.id,
        _ => false,
    };
    same_identity
        && left_layout.kind == right_layout.kind
        && left_layout.members.len() == right_layout.members.len()
        && left_layout
            .members
            .iter()
            .zip(&right_layout.members)
            .all(|(left, right)| left.name == right.name)
}

/// Native storage selected for a `ref` actual: its root, the selections
/// below it and the type there.
struct NativePlace {
    root: IrElementRoot,
    steps: Vec<IrElementStep>,
    descriptor: TypeDescriptor,
}

impl NativePlace {
    /// Step down one record member.
    fn member(mut self, name: &str) -> Option<Self> {
        let TypeShape::Aggregate(layout) = &self.descriptor.shape else {
            return None;
        };
        if layout.kind == AggregateKind::TaggedUnion {
            return None;
        }
        let index = layout
            .members
            .iter()
            .position(|member| member.name == name)?;
        let descriptor = layout.members[index].descriptor.clone();
        self.steps
            .push(IrElementStep::Item(u32::try_from(index).ok()?));
        self.descriptor = descriptor;
        Some(self)
    }
}

impl Codegen<'_> {
    /// The native storage `node` selects: a record element of a container,
    /// a native value, or a member or member-array element below either.
    /// Selections are evaluated once, now. `None` for other actuals, and for
    /// selections that cross a container member (only its element holds it,
    /// see `native_call_argument`).
    fn native_place(&mut self, path: &str, node: NodeId) -> Result<Option<NativePlace>, String> {
        let node = self.p30_unwrap_cast(node);
        if let Some(selection) = self.record_element_of(node) {
            let Some(descriptor) =
                self.container_element_descriptor(selection.container, selection.depth())
            else {
                return Ok(None);
            };
            let IrValueSlot::Element { mut indices, key } =
                self.lower_element_slot(path, &selection)?
            else {
                unreachable!("element selections lower to element slots");
            };
            if indices.len() > 1 {
                return Ok(None);
            }
            return Ok(Some(NativePlace {
                root: IrElementRoot::Element {
                    container: selection.container,
                    index: indices.pop(),
                    key,
                },
                steps: Vec::new(),
                descriptor,
            }));
        }
        if let Ok(Some((value, prefix))) = self.native_path_of(node) {
            return Ok(self.native_value_place(value, &prefix));
        }
        match self.kind(node) {
            NodeKind::Expr(ExprKind::HierPath { parts, refs }) => {
                let Some(Some(first)) = refs.first().copied() else {
                    return Ok(None);
                };
                if !matches!(
                    self.kind(first),
                    NodeKind::Expr(ExprKind::ArraySelect { .. } | ExprKind::BitSelect { .. })
                ) {
                    return Ok(None);
                }
                let Some(mut place) = self.native_place(path, first)? else {
                    return Ok(None);
                };
                for part in parts.iter().skip(1).filter(|part| !part.is_empty()) {
                    let Some(next) = place.member(part) else {
                        return Ok(None);
                    };
                    place = next;
                }
                Ok(Some(place))
            }
            NodeKind::Expr(ExprKind::ArraySelect { base, indices }) => {
                let place = match self.db.array_select_path(node) {
                    Some((owner, members)) => {
                        let owner_place = if let Some(value) = self.native_roots.get(&owner) {
                            self.native_value_place(*value, &[])
                        } else {
                            self.native_place(path, owner)?
                        };
                        let Some(mut place) = owner_place else {
                            return Ok(None);
                        };
                        for member in members {
                            let Some(next) = place.member(&member) else {
                                return Ok(None);
                            };
                            place = next;
                        }
                        Some(place)
                    }
                    None => self.native_place(path, *base)?,
                };
                match place {
                    Some(place) => self.native_place_index(path, place, indices),
                    None => Ok(None),
                }
            }
            NodeKind::Expr(ExprKind::BitSelect { base, index }) => {
                match self.native_place(path, *base)? {
                    Some(place) => self.native_place_index(path, place, &[*index]),
                    None => Ok(None),
                }
            }
            _ => Ok(None),
        }
    }

    /// The place of a constant path below a native value root.
    fn native_value_place(
        &self,
        value: usize,
        prefix: &[AggregatePathPart],
    ) -> Option<NativePlace> {
        let descriptor = self.native_layout_of_value(value).ok()?.descriptor.clone();
        let mut place = NativePlace {
            root: IrElementRoot::Value(value),
            steps: Vec::new(),
            descriptor,
        };
        let mut parts = prefix.iter().peekable();
        while let Some(part) = parts.next() {
            place = match part {
                AggregatePathPart::Member(name) => place.member(name)?,
                AggregatePathPart::Index(index) => {
                    let TypeShape::FixedArray {
                        dimensions,
                        element,
                    } = &place.descriptor.shape
                    else {
                        return None;
                    };
                    let mut position = 0i64;
                    let mut declared = *index;
                    for (slot, (left, right)) in dimensions.iter().enumerate() {
                        let extent = i64::from(*left).abs_diff(i64::from(*right)) + 1;
                        let offset = if left <= right {
                            i64::from(declared) - i64::from(*left)
                        } else {
                            i64::from(*left) - i64::from(declared)
                        };
                        position = position * i64::try_from(extent).ok()? + offset;
                        if slot + 1 < dimensions.len() {
                            match parts.next()? {
                                AggregatePathPart::Index(next) => declared = *next,
                                AggregatePathPart::Member(_) => return None,
                            }
                        }
                    }
                    let element = element.as_ref().clone();
                    place
                        .steps
                        .push(IrElementStep::Item(u32::try_from(position).ok()?));
                    place.descriptor = element;
                    place
                }
            };
        }
        Some(place)
    }

    /// Step into a member array by its declared indices, all of them at once:
    /// a member array is one flat run of items.
    fn native_place_index(
        &mut self,
        path: &str,
        mut place: NativePlace,
        indices: &[NodeId],
    ) -> Result<Option<NativePlace>, String> {
        let TypeShape::FixedArray {
            dimensions,
            element,
        } = &place.descriptor.shape
        else {
            return Ok(None);
        };
        if dimensions.len() != indices.len() {
            return Ok(None);
        }
        let dimensions = dimensions.clone();
        let element = element.as_ref().clone();
        let position = self.lower_view_position(path, &dimensions, indices)?;
        place.steps.push(IrElementStep::Position(position));
        place.descriptor = element;
        Ok(Some(place))
    }

    /// The call operand binding `node`, a native record element or a member
    /// or member-array element of a native record, to a `ref` formal of
    /// `kind`. A whole value root has an address of its own.
    pub(in crate::sim::codegen::lowering) fn member_ref(
        &mut self,
        path: &str,
        node: NodeId,
        kind: IrElementRefKind,
        const_ref: bool,
    ) -> Result<Option<IrCallArg>, String> {
        let Some(place) = self.native_place(path, node)? else {
            return Ok(None);
        };
        if matches!(place.root, IrElementRoot::Value(_)) && place.steps.is_empty() {
            return Ok(None);
        }
        let selects_kind = matches!(
            (&place.descriptor.shape, kind),
            (TypeShape::Aggregate(_), IrElementRefKind::Record)
                | (TypeShape::String, IrElementRefKind::String)
                | (TypeShape::Opaque { .. }, IrElementRefKind::Handle)
                | (
                    TypeShape::PackedAtom { .. },
                    IrElementRefKind::Packed { .. }
                )
        );
        if !selects_kind {
            return Ok(None);
        }
        Ok(Some(IrCallArg::ElementRef(Box::new(IrElementRef {
            root: place.root,
            steps: place.steps,
            kind,
            const_ref,
        }))))
    }

    /// The record a native record `ref` formal binds to when `actual` names
    /// declaration-owned record storage of the formal's type. `None` for a
    /// subroutine record (passed by address) and for actuals without static
    /// storage, such as container elements or run-time-indexed members.
    pub(in crate::sim::codegen::lowering) fn record_ref_binding(
        &self,
        formal: NodeId,
        actual: NodeId,
    ) -> Option<RecordRefBinding> {
        if !self.is_ref_formal(formal) {
            return None;
        }
        let layout = self.native_layouts.get(&formal)?;
        let source = self.p30_unwrap_cast(actual);
        if !matches!(self.native_path_of(source), Ok(None)) {
            return None;
        }
        let selection = self.resolve_unpacked_aggregate(source)?;
        if selection.storage.columns || !same_record_type(&layout.descriptor, &selection.descriptor)
        {
            return None;
        }
        // A formal that is itself bound forwards the record it names.
        let (root, prefix) = match self.record_ref_aliases.get(&selection.root) {
            Some(alias) => (
                alias.root,
                alias
                    .prefix
                    .iter()
                    .cloned()
                    .chain(selection.prefix)
                    .collect(),
            ),
            None => (selection.root, selection.prefix),
        };
        Some(RecordRefBinding {
            formal,
            root,
            prefix,
        })
    }

    /// Record bindings of every native record `ref` formal of one call.
    pub(in crate::sim::codegen::lowering) fn record_ref_bindings(
        &self,
        formals: &[(NodeId, bool)],
        bound: &[BoundArg],
    ) -> Vec<RecordRefBinding> {
        formals
            .iter()
            .zip(bound)
            .filter_map(|((formal, _), argument)| self.record_ref_binding(*formal, argument.expr))
            .collect()
    }

    /// The model function a call of `function` reaches: `template`, or the
    /// specialization binding its record `ref` actuals. Methods dispatch
    /// through receivers and virtual slots that a specialization does not
    /// join, so they keep the subroutine-record restriction.
    #[allow(clippy::too_many_arguments)]
    pub(in crate::sim::codegen::lowering) fn record_ref_callee(
        &mut self,
        path: &str,
        function: NodeId,
        inst: NodeId,
        formals: &[(NodeId, bool)],
        bound: &[BoundArg],
        template: usize,
        method: bool,
    ) -> Result<usize, String> {
        let records = self.record_ref_bindings(formals, bound);
        if records.is_empty() {
            // An open-array DPI import calls the specialization for its
            // actuals; every other callee is the template itself.
            return self.dpi_open_callee(template, formals, bound);
        }
        self.reject_method_record_refs(path, function, &records, method)?;
        self.task_specialization(function, inst, Vec::new(), records)
    }

    /// A method call cannot bind records: its receiver and virtual-slot
    /// dispatch do not reach specializations.
    pub(in crate::sim::codegen::lowering) fn reject_method_record_refs(
        &self,
        path: &str,
        function: NodeId,
        records: &[RecordRefBinding],
        method: bool,
    ) -> Result<(), String> {
        match records.first() {
            Some(first) if method => Err(format!(
                "a module, static or procedural-block record as the actual of native record `ref` formal `{}` of method `{}` in `{path}` is not supported (SIM-008)",
                self.node(first.formal).name,
                self.node(function).name
            )),
            _ => Ok(()),
        }
    }

    /// Resolve the bound formals of the specialization being lowered to
    /// their records' leaves instead of their (unused) native values.
    pub(in crate::sim::codegen::lowering) fn bind_record_refs(
        &mut self,
        records: &[RecordRefBinding],
    ) -> Result<(), String> {
        for binding in records {
            let view = self.record_ref_view(binding)?;
            self.native_roots.remove(&binding.formal);
            self.unpacked_aggregates.insert(binding.formal, view);
            self.record_ref_aliases
                .insert(binding.formal, binding.clone());
        }
        Ok(())
    }

    pub(in crate::sim::codegen::lowering) fn unbind_record_refs(
        &mut self,
        records: &[RecordRefBinding],
    ) {
        for binding in records {
            self.unpacked_aggregates.remove(&binding.formal);
            self.record_ref_aliases.remove(&binding.formal);
        }
    }

    /// The leaf storage below a binding's prefix, with paths relative to it.
    fn record_ref_view(&self, binding: &RecordRefBinding) -> Result<UnpackedAggregateInfo, String> {
        let storage = self.unpacked_aggregates.get(&binding.root).ok_or_else(|| {
            format!(
                "record `{}` bound to `ref` formal `{}` has no leaf storage",
                self.node(binding.root).name,
                self.node(binding.formal).name
            )
        })?;
        if binding.prefix.is_empty() {
            return Ok(storage.clone());
        }
        let descriptor = self
            .query_descriptor(binding.root)
            .and_then(|descriptor| Self::descriptor_at_path(descriptor, &binding.prefix));
        let Some(TypeShape::Aggregate(layout)) = descriptor.map(|descriptor| descriptor.shape)
        else {
            return Err(format!(
                "selection bound to `ref` formal `{}` is not a record",
                self.node(binding.formal).name
            ));
        };
        let leaves: Vec<AggregateMemberInfo> = storage
            .leaves
            .iter()
            .filter_map(|leaf| {
                let relative = leaf.path.strip_prefix(binding.prefix.as_slice())?;
                let mut leaf = leaf.clone();
                leaf.path = relative.to_vec();
                Some(leaf)
            })
            .collect();
        let members = layout
            .members
            .iter()
            .map(|member| {
                let path = vec![AggregatePathPart::Member(member.name.clone())];
                leaves
                    .iter()
                    .find(|leaf| leaf.path == path)
                    .cloned()
                    .unwrap_or_else(|| AggregateMemberInfo {
                        member: member.clone(),
                        signal: None,
                        object: None,
                        array: None,
                        container: None,
                        path,
                    })
            })
            .collect();
        Ok(UnpackedAggregateInfo {
            kind: layout.kind,
            type_identity: layout.type_identity,
            columns: false,
            members,
            leaves,
        })
    }
}
