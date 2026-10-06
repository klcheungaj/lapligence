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

use super::*;

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

impl Codegen<'_> {
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
            return Ok(template);
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
