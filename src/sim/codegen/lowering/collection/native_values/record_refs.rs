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
//!
//! A record element selected at run time (`t(m.e[i])`) has no single binding:
//! the call lowers once per element, under a decision tree over the index
//! taken at the call, each branch reaching that element's specialization
//! (`EmitCtx::lower_record_ref_select`). Specializations are shared, so code
//! grows with the elements and calls of each site, bounded by
//! `NATIVE_RECORD_SELECT_LIMIT`.
//!
//! A class method keeps its receiver in a specialization. A virtual call
//! specializes the whole override family under a fresh virtual slot, so the
//! ordinary dispatchers serve it and nothing changes for unbound calls. A
//! virtual-interface call likewise reaches a new method of its descriptor
//! holding each instance's specialization.

use super::*;
use crate::sim::ir::{IrElementRef, IrElementRefKind, IrElementRoot, IrElementStep, IrValueSlot};

/// How a call selects its implementation, which decides how record `ref`
/// actuals can reach it.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::sim::codegen::lowering) enum RecordRefDispatch {
    /// The callee itself: a subroutine, or a method called non-virtually.
    Direct,
    /// A class method's virtual slot.
    Virtual,
    /// A virtual-interface method.
    Interface,
}

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
                            let Some(next) = place.member(member) else {
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
            return self.module_leaf_ref(path, node, kind, const_ref);
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
        ) || matches!(
            (&place.descriptor.shape, kind),
            (TypeShape::Real { shortreal: leaf }, IrElementRefKind::Real { shortreal })
                if *leaf == shortreal
        );
        if !selects_kind {
            return Ok(None);
        }
        Ok(Some(IrCallArg::ElementRef(Box::new(IrElementRef {
            root: place.root,
            steps: place.steps,
            kind,
            const_ref,
            companions: Vec::new(),
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
        let (root, prefix) = match self.record_ref_pins.get(&source) {
            Some(RecordRefPin::Element { root, prefix }) => (*root, prefix.clone()),
            Some(RecordRefPin::Default(_)) => return None,
            None if matches!(self.native_path_of(source), Ok(None)) => {
                let selection = self.resolve_unpacked_aggregate(source)?;
                if selection.storage.columns
                    || !same_record_type(&layout.descriptor, &selection.descriptor)
                {
                    return None;
                }
                (selection.root, selection.prefix)
            }
            None => return None,
        };
        // A formal that is itself bound forwards the record it names.
        let (root, prefix) = match self.record_ref_aliases.get(&root) {
            Some(alias) => (
                alias.root,
                alias.prefix.iter().cloned().chain(prefix).collect(),
            ),
            None => (root, prefix),
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
    /// specialization binding its record `ref` actuals. A specialization
    /// keeps its method's receiver; a virtual call (`dispatch`) reaches the
    /// family of `record_ref_family`. A virtual-interface call with bindings
    /// takes `record_ref_interface_method` instead.
    pub(in crate::sim::codegen::lowering) fn record_ref_callee(
        &mut self,
        function: NodeId,
        inst: NodeId,
        formals: &[(NodeId, bool)],
        bound: &[BoundArg],
        template: usize,
        dispatch: RecordRefDispatch,
    ) -> Result<usize, String> {
        let records = self.record_ref_bindings(formals, bound);
        if records.is_empty() {
            // An open-array DPI import calls the specialization for its
            // actuals; every other callee is the template itself.
            return self.dpi_open_callee(template, formals, bound);
        }
        if dispatch == RecordRefDispatch::Virtual {
            return self.record_ref_family(function, records);
        }
        self.task_specialization(function, inst, Vec::new(), records)
    }

    /// How the call node `call` reaches its method.
    pub(in crate::sim::codegen::lowering) fn record_ref_dispatch(
        &self,
        call: NodeId,
        virtual_interface: bool,
    ) -> RecordRefDispatch {
        if virtual_interface {
            RecordRefDispatch::Interface
        } else if self.class_method_virtual_dispatch(call) {
            RecordRefDispatch::Virtual
        } else {
            RecordRefDispatch::Direct
        }
    }

    /// A dispatch entry of virtual-interface method `method` for one set of
    /// record bindings: a new method of the descriptor whose implementations
    /// are each instance's specialization, so the ordinary dispatchers serve
    /// the call and code grows with distinct actuals. Returns the method and
    /// its representative function. Instances name their own formals, so a
    /// binding carries over by formal position.
    pub(in crate::sim::codegen::lowering) fn record_ref_interface_method(
        &mut self,
        interface: usize,
        method: usize,
        function: NodeId,
        records: &[RecordRefBinding],
    ) -> Result<(usize, usize), String> {
        let positions = self.record_ref_positions(function, records)?;
        let key = (interface, method, positions.clone());
        if let Some(entry) = self.record_ref_interface_methods.get(&key) {
            return Ok(*entry);
        }
        let entry = self.model.virtual_interfaces[interface].methods[method].clone();
        let mut implementations = Vec::with_capacity(entry.instances.len());
        let mut representative = None;
        for implementation in &entry.instances {
            let Some(implementation) = *implementation else {
                implementations.push(None);
                continue;
            };
            let (&concrete, _) = self
                .func_meta
                .iter()
                .find(|(_, meta)| meta.ir == implementation)
                .ok_or_else(|| {
                    format!(
                        "virtual interface method `{}` has no function metadata",
                        entry.name
                    )
                })?;
            let inst = self.callable_environment(concrete).ok_or_else(|| {
                format!(
                    "virtual interface method `{}` has no concrete interface environment",
                    entry.name
                )
            })?;
            let bindings = self.record_ref_rebind(concrete, &positions)?;
            let index = self.task_specialization(concrete, inst, Vec::new(), bindings)?;
            if implementation == entry.function {
                representative = Some(index);
            }
            implementations.push(Some(index));
        }
        let representative = representative.ok_or_else(|| {
            format!(
                "virtual interface method `{}` has no representative implementation",
                entry.name
            )
        })?;
        let descriptor = &mut self.model.virtual_interfaces[interface];
        let bound = descriptor.methods.len();
        for (instance, implementation) in descriptor.instances.iter_mut().zip(&implementations) {
            instance.methods.push(*implementation);
        }
        descriptor
            .methods
            .push(crate::sim::ir::IrVirtualInterfaceMethod {
                name: entry.name,
                function: representative,
                instances: implementations,
            });
        self.record_ref_interface_methods
            .insert(key, (bound, representative));
        Ok((bound, representative))
    }

    /// The formal position, record and prefix of each binding of a call of
    /// `function`.
    fn record_ref_positions(
        &self,
        function: NodeId,
        records: &[RecordRefBinding],
    ) -> Result<RecordRefPositions, String> {
        let formals = self.func_formals(function);
        records
            .iter()
            .map(|binding| {
                let position = formals
                    .iter()
                    .position(|(formal, _)| *formal == binding.formal)
                    .ok_or_else(|| {
                        format!(
                            "record `ref` formal `{}` is not a formal of `{}`",
                            self.node(binding.formal).name,
                            self.node(function).name
                        )
                    })?;
                Ok((position, binding.root, binding.prefix.clone()))
            })
            .collect()
    }

    /// `positions` as bindings of the formals of `function`.
    fn record_ref_rebind(
        &self,
        function: NodeId,
        positions: &RecordRefPositions,
    ) -> Result<Vec<RecordRefBinding>, String> {
        let formals = self.func_formals(function);
        positions
            .iter()
            .map(|(position, root, prefix)| {
                let (formal, _) = formals.get(*position).ok_or_else(|| {
                    format!(
                        "override `{}` has no formal {position}",
                        self.node(function).name
                    )
                })?;
                Ok(RecordRefBinding {
                    formal: *formal,
                    root: *root,
                    prefix: prefix.clone(),
                })
            })
            .collect()
    }

    /// A specialization of every override of a virtual method for one set
    /// of record bindings, joined by a fresh virtual slot, so that a call
    /// dispatches on the receiver exactly as the unbound family does and
    /// code grows with distinct actuals. Overrides name their own formals,
    /// so a binding carries over by formal position.
    fn record_ref_family(
        &mut self,
        function: NodeId,
        records: Vec<RecordRefBinding>,
    ) -> Result<usize, String> {
        let slot = *self
            .method_virtual_slots
            .get(&function)
            .ok_or_else(|| format!("method `{}` has no virtual slot", self.node(function).name))?;
        let positions = self.record_ref_positions(function, &records)?;
        if let Some(family) = self.record_ref_families.get(&(slot, positions.clone())) {
            if let Some(index) = family.get(&function) {
                return Ok(*index);
            }
        }
        let family_slot = self.next_virtual_slot;
        self.next_virtual_slot += 1;
        let mut family = HashMap::new();
        for class in self.db.classes().to_vec() {
            for method in self.class_method_nodes(class) {
                if self.method_virtual_slots.get(&method) != Some(&slot)
                    || !self.func_meta.contains_key(&method)
                {
                    continue;
                }
                let bindings = self.record_ref_rebind(method, &positions)?;
                let index = self.task_specialization(method, class, Vec::new(), bindings)?;
                self.model.funcs[index].virtual_slot = Some(family_slot);
                family.insert(method, index);
            }
        }
        let index = family.get(&function).copied().ok_or_else(|| {
            format!(
                "virtual method `{}` is outside its slot's classes",
                self.node(function).name
            )
        })?;
        self.record_ref_families.insert((slot, positions), family);
        Ok(index)
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
