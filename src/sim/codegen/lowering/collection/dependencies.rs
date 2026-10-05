//! Dependencies.

use super::super::packed_geometry::PackedSelectDim;
use super::*;

/// Widest constant array row whose cells are recorded individually in
/// writer analysis. A wider row is one flattened cell interval (see
/// [`Codegen::array_row_write`]), so every row costs one record regardless of
/// its width while bounded rows keep per-cell read exclusion.
const PRECISE_ROW_WRITE_CELLS: u64 = 256;

/// Largest array whose whole-contents read an always_comb/always_latch
/// process re-expresses as per-cell reads to exclude cells it writes itself.
/// Larger (including descriptor-backed) arrays keep the one contents marker:
/// only this process may write those cells, and its own blocking writes
/// complete before it waits again, so the coarser set wakes on the same
/// external changes without one generated dependency per cell (a nonblocking
/// self-write can cause one extra re-evaluation).
const PRECISE_EXCLUSION_CELLS: u64 = 256;

#[derive(Clone, Copy, PartialEq, Eq)]
enum ProcessWriteMode {
    Sensitivity,
    ContinuousConflict,
    /// SV 9.2.2.2-9.2.2.4 single-writer ownership: ordinary assignments,
    /// not force/release/assign/deassign overrides.
    Ownership,
}

impl<'a> Codegen<'a> {
    fn unpacked_storage_dependencies(&self, node: NodeId) -> Option<Vec<IrDependency>> {
        let (root, prefix) = self.unpacked_path_for_expr(node).or_else(|| {
            self.unpacked_aggregate_info(node)
                .map(|(root, _)| (root, Vec::new()))
        })?;
        let storage = self.unpacked_aggregates.get(&root)?;
        let descriptor = self.query_descriptor(root)?;
        let selected = super::fixed_values::fixed_path_descriptor(descriptor, &prefix);
        let mut dependencies = Vec::new();
        for leaf in &storage.leaves {
            if !prefix.starts_with(&leaf.path) && !leaf.path.starts_with(&prefix) {
                continue;
            }
            if let Some(array) = &leaf.array {
                dependencies.push(IrDependency::ArrayContents(self.reference_array(array.ir)));
                continue;
            }
            let Some(signal) = &leaf.signal else {
                // A string member publishes its own change marker; other
                // native members have none (see `walk_read_signals_bound`).
                if let Some(object) = leaf.object.map(|object| self.reference_object(object)) {
                    if self.model.objects[object].ty == IrObjectType::String {
                        dependencies.push(IrDependency::Object(object));
                    }
                }
                continue;
            };
            let dependency = self.signal_dependency(signal);
            if prefix.len() > leaf.path.len() && !signal.real {
                if let (Some((selected, offset)), Some((_, base))) = (
                    &selected,
                    super::fixed_values::fixed_path_descriptor(descriptor, &leaf.path),
                ) {
                    if let Some(width) = Self::fixed_descriptor_width(selected) {
                        if let Some(offset) = offset.checked_sub(base) {
                            dependencies.push(self.slice_dependency(dependency, offset, width));
                            continue;
                        }
                    }
                }
            }
            dependencies.push(dependency);
        }
        (!dependencies.is_empty()).then_some(dependencies)
    }

    // ── Signal reads for sensitivity ───────────────────────────────────────

    /// Collect every storage dependency read anywhere in the
    /// statement/expression tree rooted at `root` (deduped, deterministic
    /// order).
    /// Function/task calls descend into the callee bodies (guarded against
    /// recursion), so reads hidden behind a function call contribute to the
    /// sensitivity set.
    pub(in super::super) fn collect_read_signals(
        &self,
        scope_path: &str,
        root: NodeId,
    ) -> Result<Vec<IrDependency>, String> {
        self.collect_read_dependencies(scope_path, root, true)
    }

    /// Collect the implicit sensitivity set for a plain `always @*` block.
    ///
    /// Verilog wildcard event controls use the expressions visible at the
    /// call site.  In particular, reads hidden inside a called function are
    /// not inspected; call arguments themselves remain ordinary reads.
    pub(in super::super) fn collect_at_star_signals(
        &self,
        scope_path: &str,
        root: NodeId,
    ) -> Result<Vec<IrDependency>, String> {
        self.collect_read_dependencies_mode(scope_path, root, false)
    }

    /// Build the implicit sensitivity set for an always-family process. The
    /// SystemVerilog forms remove every storage object written by the process
    /// (including writes performed by called subroutines); plain `@*` keeps
    /// the Verilog call-site-only read walk and has no such exclusion.
    pub(super) fn collect_process_sensitivity(
        &self,
        scope_path: &str,
        root: NodeId,
        process_kind: Option<AlwaysKind>,
    ) -> Result<Vec<IrDependency>, String> {
        let mut reads = if process_kind == Some(AlwaysKind::Always) {
            self.collect_at_star_signals(scope_path, root)?
        } else {
            self.collect_read_signals(scope_path, root)?
        };
        if matches!(process_kind, Some(AlwaysKind::Comb | AlwaysKind::Latch)) {
            let writes = self.collect_process_writes(root)?;
            reads = reads
                .into_iter()
                .flat_map(|read| self.exclude_written_prefixes(read, &writes))
                .collect();
        }
        Ok(reads)
    }

    /// Sensitivity of an expression that a process evaluates for an event
    /// control or an effectful force source. Like always_comb's implicit
    /// sensitivity (SV 9.2.2.2.1), it holds every storage read, including
    /// reads inside called functions, except storage declared within those
    /// functions (formals, locals, static result) and storage they write.
    /// Without these exclusions a helper's own stores would re-trigger its
    /// evaluation, and two such waits sharing a static helper or a counter
    /// would wake each other forever.
    pub(in super::super) fn collect_evaluator_sensitivity(
        &self,
        scope_path: &str,
        expression: NodeId,
    ) -> Result<Vec<IrDependency>, String> {
        let writes = self.collect_process_writes(expression)?;
        let mut owned = HashSet::new();
        let mut owned_arrays = HashSet::new();
        let mut visited = HashSet::new();
        self.collect_callee_storage(expression, &mut visited, &mut owned, &mut owned_arrays)?;
        let owned_storage = |dependency: &IrDependency| {
            let storage = match dependency {
                IrDependency::PackedRange { storage, .. } => storage.as_ref(),
                other => other,
            };
            match storage {
                IrDependency::ArrayContents(array) | IrDependency::ArrayElement { array, .. } => {
                    owned_arrays.contains(array)
                }
                other => owned.contains(other),
            }
        };
        Ok(self
            .collect_read_signals(scope_path, expression)?
            .into_iter()
            .filter(|read| !owned_storage(read))
            .flat_map(|read| {
                if writes.is_empty() {
                    vec![read]
                } else {
                    self.exclude_written_prefixes(read, &writes)
                }
            })
            .collect())
    }

    /// Storage declared inside every function `root` calls, transitively.
    fn collect_callee_storage(
        &self,
        root: NodeId,
        visited: &mut HashSet<NodeId>,
        owned: &mut HashSet<IrDependency>,
        owned_arrays: &mut HashSet<usize>,
    ) -> Result<(), String> {
        if let NodeKind::FuncCall {
            name,
            is_task,
            callee,
            ..
        } = self.kind(root)
        {
            let (function, _) = self.resolve_callee_env(self.inst, name, *is_task, *callee)?;
            if visited.insert(function) {
                let mut declarations = vec![function];
                while let Some(node) = declarations.pop() {
                    declarations.extend(self.node(node).children.iter().copied());
                    if !matches!(
                        self.kind(node),
                        NodeKind::FuncArg { .. } | NodeKind::Var { .. } | NodeKind::Array { .. }
                    ) {
                        continue;
                    }
                    if let Some(info) = self
                        .signal_of(node)
                        .or_else(|| self.static_proc_local_signal(node))
                    {
                        owned.insert(self.signal_dependency(info));
                    }
                    if let Some(array) = self.array_of(node) {
                        owned_arrays.insert(self.reference_array(array.ir));
                    }
                }
                if let Some(body) = self.func_body(function) {
                    self.collect_callee_storage(body, visited, owned, owned_arrays)?;
                }
            }
        }
        for child in &self.node(root).children {
            self.collect_callee_storage(*child, visited, owned, owned_arrays)?;
        }
        Ok(())
    }

    fn dependency_width(&self, dependency: &IrDependency) -> Option<u32> {
        match dependency {
            IrDependency::Scalar(name) => self
                .model
                .signals
                .iter()
                .enumerate()
                .find(|(index, signal)| {
                    signal.c_name == *name || self.signal_dependency_name(*index) == *name
                })
                .map(|(_, signal)| signal.ty.width())
                .filter(|width| *width != 0),
            IrDependency::ArrayElement { array, .. } => self
                .model
                .arrays
                .get(*array)
                .filter(|array| !array.real)
                .map(|array| array.elem_width),
            IrDependency::PackedRange { width, .. } => Some(*width),
            _ => None,
        }
    }

    fn dependency_span(&self, dependency: &IrDependency) -> Option<(IrDependency, u32, u32)> {
        if let IrDependency::PackedRange {
            storage,
            lsb,
            width,
        } = dependency
        {
            Some(((**storage).clone(), *lsb, *width))
        } else {
            self.dependency_width(dependency)
                .map(|width| (dependency.clone(), 0, width))
        }
    }

    fn slice_dependency(&self, storage: IrDependency, lsb: u32, width: u32) -> IrDependency {
        if lsb == 0 && self.dependency_width(&storage) == Some(width) {
            storage
        } else {
            IrDependency::PackedRange {
                storage: Box::new(storage),
                lsb,
                width,
            }
        }
    }

    /// Return the longest static packed prefix. Dynamic selectors keep their
    /// base prefix; they do not erase a preceding static field/dimension.
    fn packed_storage_prefix_bound(
        &self,
        node: NodeId,
        bindings: &HashMap<NodeId, IrDependency>,
    ) -> Option<IrDependency> {
        if let Some(select) = self.packed_element_member_select(node) {
            // A member of a packed-array element: the member's bits when the
            // element is static, otherwise the element chain's own prefix.
            let prefix = self.packed_storage_prefix_bound(select, bindings)?;
            let NodeKind::Expr(ExprKind::HierPath { parts, .. }) = self.kind(node) else {
                return Some(prefix);
            };
            let element = self
                .query_descriptor(select)
                .and_then(Self::fixed_descriptor_width);
            let member = self.packed_member_layout(select, &parts[1..]);
            let (storage, lsb, width) = self.dependency_span(&prefix)?;
            return Some(match (element, member) {
                (Some(element), Some(member)) if element == width => {
                    self.slice_dependency(storage, lsb.checked_add(member.lsb)?, member.width)
                }
                _ => prefix,
            });
        }
        if let NodeKind::Expr(ExprKind::HierPath { parts, refs }) = self.kind(node) {
            // A ref formal has no independent signal. Resolve its selected
            // field within the actual's prefix, not through global storage.
            if let Some((base_index, target, prefix)) =
                refs.iter().enumerate().find_map(|(index, target)| {
                    let target = (*target)?;
                    bindings.get(&target).map(|prefix| (index, target, prefix))
                })
            {
                let (storage, mut offset, _) = self.dependency_span(prefix)?;
                if base_index + 1 == parts.len() {
                    return Some(prefix.clone());
                }
                let mut layout = self.db.aggregate_layout(target)?;
                let mut selected = None;
                for (part_index, name) in parts.iter().enumerate().skip(base_index + 1) {
                    if !matches!(
                        layout.kind,
                        AggregateKind::PackedStruct | AggregateKind::PackedUnion
                    ) {
                        return None;
                    }
                    let index = layout
                        .members
                        .iter()
                        .position(|member| member.name == *name)?;
                    let member = &layout.members[index];
                    if layout.kind == AggregateKind::PackedStruct {
                        offset = offset.checked_add(
                            layout.members[index + 1..]
                                .iter()
                                .try_fold(0u32, |offset, member| {
                                    offset.checked_add(member.ty.width?)
                                })?,
                        )?;
                    }
                    selected = Some(member.ty.width?);
                    if part_index + 1 < parts.len() {
                        layout = member.aggregate_layout()?;
                    }
                }
                return Some(self.slice_dependency(storage, offset, selected?));
            }
        }
        if let Some((info, member)) = self.packed_member_info(node) {
            return Some(self.slice_dependency(
                self.signal_dependency(&info),
                member.lsb,
                member.width,
            ));
        }
        if let Some(dependencies) = self.unpacked_storage_dependencies(node) {
            if let [dependency] = dependencies.as_slice() {
                return Some(dependency.clone());
            }
        }
        let (base, indices, bounds) = match self.kind(node) {
            NodeKind::Expr(ExprKind::BitSelect { base, index }) => (*base, vec![*index], None),
            NodeKind::Expr(ExprKind::ArraySelect { base, indices }) => {
                (*base, indices.clone(), None)
            }
            NodeKind::Expr(ExprKind::PartSelect { base, left, right }) => {
                (*base, vec![], Some((*left, *right, None)))
            }
            NodeKind::Expr(ExprKind::IndexedPartSelect {
                base,
                base_expr,
                width_expr,
                neg,
            }) => (*base, vec![], Some((*base_expr, *width_expr, Some(*neg)))),
            _ => {
                if let Some(expression) = self.modport_expression_target(node) {
                    return self.packed_storage_prefix_bound(expression, bindings);
                }
                let target = match self.kind(node) {
                    NodeKind::Expr(ExprKind::Ref {
                        target: Some(target),
                    }) => *target,
                    _ => node,
                };
                if let Some(prefix) = bindings.get(&target) {
                    return Some(prefix.clone());
                }
                let info = self
                    .signal_of(target)
                    .or_else(|| self.static_proc_local_signal(target))
                    .or_else(|| self.hier_path_signal(node));
                return info
                    .filter(|info| !info.real)
                    .map(|info| self.signal_dependency(info));
            }
        };
        if !indices.is_empty() {
            if let Some(array) = self.select_array_of(node, base).filter(|array| !array.real) {
                let mut out = Vec::new();
                self.add_fixed_array_dependency(array, &indices, &mut HashSet::new(), &mut out);
                return out
                    .into_iter()
                    .find(|dep| matches!(dep, IrDependency::ArrayElement { .. }));
            }
        }
        let prefix = self.packed_storage_prefix_bound(base, bindings)?;
        let (storage, offset, base_width) = self.dependency_span(&prefix)?;
        let ranges = match self
            .query_descriptor(base)
            .map(|descriptor| &descriptor.shape)
        {
            Some(TypeShape::PackedAtom { ranges }) if !ranges.is_empty() => ranges.clone(),
            _ => vec![crate::core::db::PackedRange {
                left: i128::from(base_width) - 1,
                right: 0,
            }],
        };
        // A selected span outside its parent has no static storage bound.
        let inside = |offset: i128, width: u32, parent: u32| {
            u32::try_from(offset)
                .ok()
                .filter(|offset| offset.checked_add(width).is_some_and(|end| end <= parent))
        };
        let mut lsb = offset;
        let mut width = base_width;
        if let Some((a, b, indexed)) = bounds {
            let Ok(a) = self.eval_bound_i128(a) else {
                return Some(prefix);
            };
            let Ok(b) = self.eval_bound_i128(b) else {
                return Some(prefix);
            };
            let dim = PackedSelectDim::new(base_width, ranges.first().copied()).ok()?;
            let (offset, selected) = match indexed {
                None => dim.part(a, b),
                Some(neg) => dim.indexed(a, u32::try_from(b).ok()?, neg),
            }
            .ok()?;
            lsb = lsb.checked_add(inside(offset, selected, base_width)?)?;
            width = selected;
        } else {
            for (index, range) in indices.iter().zip(&ranges) {
                let Ok(value) = self.eval_bound_i128(*index) else {
                    return Some(self.slice_dependency(storage, lsb, width));
                };
                let dim = PackedSelectDim::new(width, Some(*range)).ok()?;
                let (offset, selected) = dim.element(value).ok()?;
                lsb = lsb.checked_add(inside(offset, selected, width)?)?;
                width = selected;
            }
        }
        (width != 0).then(|| self.slice_dependency(storage, lsb, width))
    }

    fn exclude_written_prefixes(
        &self,
        read: IrDependency,
        writes: &HashSet<IrDependency>,
    ) -> Vec<IrDependency> {
        if let IrDependency::ArrayContents(array) = &read {
            let total = self.model.arrays[*array].total();
            let whole_write = writes.contains(&read)
                || writes.iter().any(|write| {
                    Self::is_array_row_write(write)
                        && self.array_cell_interval(write) == Some((*array, 0, total))
                });
            if whole_write {
                return vec![];
            }
            if writes.iter().any(|write| self.same_storage(&read, write)) {
                if total > PRECISE_EXCLUSION_CELLS || self.model.arrays[*array].sparse() {
                    return vec![read];
                }
                return (0..total)
                    .flat_map(|index| {
                        self.exclude_written_prefixes(
                            IrDependency::ArrayElement {
                                array: *array,
                                index,
                            },
                            writes,
                        )
                    })
                    .collect();
            }
        }
        // A cell inside a written row interval is written storage.
        if writes
            .iter()
            .any(|write| Self::is_array_row_write(write) && self.same_storage(&read, write))
        {
            return vec![];
        }
        let Some((storage, lsb, width)) = self.dependency_span(&read) else {
            return if writes.iter().any(|write| self.same_storage(&read, write)) {
                vec![]
            } else {
                vec![read]
            };
        };
        let Some(end) = lsb.checked_add(width) else {
            return vec![read];
        };
        let mut intervals = vec![(lsb, end)];
        for write in writes {
            let Some((base, low, size)) = self.dependency_span(write) else {
                if self.same_storage(&storage, write) {
                    return vec![];
                }
                continue;
            };
            if base != storage {
                continue;
            }
            let high = low.saturating_add(size);
            intervals = intervals
                .into_iter()
                .flat_map(|(a, b)| {
                    if high <= a || b <= low {
                        return vec![(a, b)];
                    }
                    let mut pieces = Vec::new();
                    if a < low {
                        pieces.push((a, low));
                    }
                    if high < b {
                        pieces.push((high, b));
                    }
                    pieces
                })
                .collect();
        }
        intervals
            .into_iter()
            .map(|(a, b)| self.slice_dependency(storage.clone(), a, b - a))
            .collect()
    }

    /// Writer-analysis key for the flattened cells `first .. first + count`
    /// of one array. It is a `PackedRange` over `ArrayContents` whose bit
    /// coordinates are cell coordinates; it never reaches the IR
    /// (`ir_process_writes` widens it to the array contents).
    pub(super) fn array_row_write(
        &self,
        array: usize,
        first: u64,
        count: u64,
    ) -> Option<IrDependency> {
        Some(IrDependency::PackedRange {
            storage: Box::new(IrDependency::ArrayContents(array)),
            lsb: u32::try_from(first).ok()?,
            width: u32::try_from(count).ok().filter(|count| *count != 0)?,
        })
    }

    fn is_array_row_write(dependency: &IrDependency) -> bool {
        matches!(
            dependency,
            IrDependency::PackedRange { storage, .. }
                if matches!(storage.as_ref(), IrDependency::ArrayContents(_))
        )
    }

    /// Flattened `(array, first cell, count)` covered by an array storage key.
    fn array_cell_interval(&self, dependency: &IrDependency) -> Option<(usize, u64, u64)> {
        match dependency {
            IrDependency::ArrayElement { array, index } => Some((*array, *index, 1)),
            IrDependency::ArrayContents(array) => {
                Some((*array, 0, self.model.arrays.get(*array)?.total()))
            }
            IrDependency::PackedRange {
                storage,
                lsb,
                width,
            } => match storage.as_ref() {
                IrDependency::ArrayContents(array) => {
                    Some((*array, u64::from(*lsb), u64::from(*width)))
                }
                IrDependency::ArrayElement { array, index } => Some((*array, *index, 1)),
                _ => None,
            },
            _ => None,
        }
    }

    /// Process writes as IR storage keys: writer-analysis row intervals widen
    /// to their array contents, sorted by the deterministic storage key.
    pub(in super::super) fn ir_process_writes(
        &self,
        writes: HashSet<IrDependency>,
    ) -> Vec<IrDependency> {
        let mut writes = writes
            .into_iter()
            .map(|write| match write {
                IrDependency::PackedRange { storage, .. }
                    if matches!(storage.as_ref(), IrDependency::ArrayContents(_)) =>
                {
                    *storage
                }
                write => write,
            })
            .collect::<HashSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        writes.sort_by_key(|dependency| self.dependency_sort_key(dependency));
        writes
    }

    pub(super) fn same_storage(&self, read: &IrDependency, write: &IrDependency) -> bool {
        if Self::is_array_row_write(read) || Self::is_array_row_write(write) {
            return match (
                self.array_cell_interval(read),
                self.array_cell_interval(write),
            ) {
                (Some((a, x, n)), Some((b, y, m))) => {
                    a == b && x < y.saturating_add(m) && y < x.saturating_add(n)
                }
                _ => false,
            };
        }
        if matches!(read, IrDependency::PackedRange { .. })
            || matches!(write, IrDependency::PackedRange { .. })
        {
            if let (Some((a, x, n)), Some((b, y, m))) =
                (self.dependency_span(read), self.dependency_span(write))
            {
                return a == b
                    && u64::from(x) < u64::from(y) + u64::from(m)
                    && u64::from(y) < u64::from(x) + u64::from(n);
            }
            let a = if let IrDependency::PackedRange { storage, .. } = read {
                storage.as_ref()
            } else {
                read
            };
            let b = if let IrDependency::PackedRange { storage, .. } = write {
                storage.as_ref()
            } else {
                write
            };
            return self.same_storage(a, b);
        }
        match (read, write) {
            (IrDependency::Scalar(read), IrDependency::Scalar(write))
            | (IrDependency::Real(read), IrDependency::Real(write))
            | (IrDependency::Scalar(read), IrDependency::Real(write))
            | (IrDependency::Real(read), IrDependency::Scalar(write)) => read == write,
            (
                IrDependency::ArrayElement {
                    array: read_array,
                    index: read_index,
                },
                IrDependency::ArrayElement {
                    array: write_array,
                    index: write_index,
                },
            ) => read_array == write_array && read_index == write_index,
            (IrDependency::ArrayElement { array, .. }, IrDependency::ArrayContents(write))
            | (IrDependency::ArrayContents(array), IrDependency::ArrayContents(write))
            | (
                IrDependency::ArrayContents(array),
                IrDependency::ArrayElement { array: write, .. },
            ) => array == write,
            (
                IrDependency::ContainerContents(container)
                | IrDependency::ContainerShape(container),
                IrDependency::ContainerContents(write) | IrDependency::ContainerShape(write),
            ) => container == write,
            (IrDependency::Object(read), IrDependency::Object(write)) => read == write,
            _ => false,
        }
    }

    pub(in super::super) fn collect_process_writes(
        &self,
        root: NodeId,
    ) -> Result<HashSet<IrDependency>, String> {
        let mut writes = HashSet::new();
        let mut visited = HashSet::new();
        self.walk_process_writes_bound(
            root,
            &mut writes,
            &mut visited,
            &HashMap::new(),
            ProcessWriteMode::Sensitivity,
        )?;
        Ok(writes)
    }

    /// Whether evaluating `reader` can observe storage written through any
    /// of the lvalue nodes in `targets`. A target whose writes cannot be
    /// represented as storage dependencies is treated as overlapping.
    pub(in super::super) fn reads_overlap_lvalue_writes(
        &self,
        scope_path: &str,
        reader: NodeId,
        targets: &[NodeId],
    ) -> Result<bool, String> {
        let mut writes = HashSet::new();
        for target in targets {
            let mut target_writes = HashSet::new();
            self.add_process_lhs_write_bound(*target, &mut target_writes, &HashMap::new());
            if target_writes.is_empty() {
                return Ok(true);
            }
            writes.extend(target_writes);
        }
        let reads = self.collect_read_signals(scope_path, reader)?;
        Ok(reads
            .iter()
            .any(|read| writes.iter().any(|write| self.same_storage(read, write))))
    }

    /// Storage a process owns under the always_comb/always_latch/always_ff
    /// single-writer rule. Overrides (force, release and procedural
    /// continuous assignments) are not ordinary assignments and stay legal
    /// on storage another process owns; their operands still count.
    pub(super) fn collect_ownership_writes(
        &self,
        root: NodeId,
    ) -> Result<HashSet<IrDependency>, String> {
        let mut writes = HashSet::new();
        self.walk_process_writes_bound(
            root,
            &mut writes,
            &mut HashSet::new(),
            &HashMap::new(),
            ProcessWriteMode::Ownership,
        )?;
        Ok(writes)
    }

    /// Ordinary procedural assignments conflict with a continuous variable
    /// driver. Force/release are overrides, not competing assignments (SV 6.5).
    pub(super) fn collect_continuous_conflict_writes(
        &self,
        root: NodeId,
    ) -> Result<HashSet<IrDependency>, String> {
        let mut writes = HashSet::new();
        self.walk_process_writes_bound(
            root,
            &mut writes,
            &mut HashSet::new(),
            &HashMap::new(),
            ProcessWriteMode::ContinuousConflict,
        )?;
        Ok(writes)
    }

    fn walk_process_writes_bound(
        &self,
        node: NodeId,
        writes: &mut HashSet<IrDependency>,
        visited_functions: &mut HashSet<NodeId>,
        bindings: &HashMap<NodeId, IrDependency>,
        mode: ProcessWriteMode,
    ) -> Result<(), String> {
        if self.is_process_self_call(node) {
            return Ok(());
        }
        if self.is_semaphore_constructor_call(node) {
            for child in &self.node(node).children {
                self.walk_process_writes_bound(*child, writes, visited_functions, bindings, mode)?;
            }
            return Ok(());
        }
        if self.is_mailbox_constructor_call(node) {
            for child in &self.node(node).children {
                self.walk_process_writes_bound(*child, writes, visited_functions, bindings, mode)?;
            }
            return Ok(());
        }
        match self.kind(node) {
            NodeKind::Stmt(StmtKind::Force { lhs, rhs })
                if mode != ProcessWriteMode::Sensitivity =>
            {
                // The generic children can also contain Slang's assignment
                // wrapper. Visit only the typed operands so that wrapper does
                // not reclassify the override as an ordinary variable write.
                self.walk_process_writes_bound(*lhs, writes, visited_functions, bindings, mode)?;
                self.walk_process_writes_bound(*rhs, writes, visited_functions, bindings, mode)?;
                return Ok(());
            }
            NodeKind::Stmt(StmtKind::Release { lhs } | StmtKind::Deassign { lhs })
                if mode != ProcessWriteMode::Sensitivity =>
            {
                self.walk_process_writes_bound(*lhs, writes, visited_functions, bindings, mode)?;
                return Ok(());
            }
            NodeKind::Stmt(StmtKind::ProcContAssign { .. })
                if mode == ProcessWriteMode::Ownership =>
            {
                for child in self.node(node).children.iter().skip(1) {
                    self.walk_process_writes_bound(
                        *child,
                        writes,
                        visited_functions,
                        bindings,
                        mode,
                    )?;
                }
                return Ok(());
            }
            NodeKind::Stmt(StmtKind::Assign { .. })
            | NodeKind::Stmt(StmtKind::ProcContAssign { .. })
            | NodeKind::Stmt(StmtKind::Force { .. })
            | NodeKind::ContAssign { .. } => {
                if let Some(lhs) = self.node(node).children.first() {
                    self.add_process_lhs_write_bound(*lhs, writes, bindings);
                }
            }
            NodeKind::Stmt(StmtKind::Release { lhs })
            | NodeKind::Stmt(StmtKind::Deassign { lhs }) => {
                self.add_process_lhs_write_bound(*lhs, writes, bindings);
                return Ok(());
            }
            NodeKind::Stmt(StmtKind::VariableDecl { declaration }) => {
                // Initializers are assignments for the continuous-driver
                // conflict rule, but not writes in implicit sensitivity.
                if let Some(initializer) = self.db.var_initializer(*declaration).or_else(|| {
                    if mode == ProcessWriteMode::ContinuousConflict {
                        self.db
                            .array_meta(*declaration)
                            .filter(|meta| meta.net_type.is_none())
                            .and_then(|meta| meta.init)
                    } else {
                        None
                    }
                }) {
                    if mode == ProcessWriteMode::ContinuousConflict {
                        self.add_process_lhs_write_bound(*declaration, writes, bindings);
                    }
                    self.walk_process_writes_bound(
                        initializer,
                        writes,
                        visited_functions,
                        bindings,
                        mode,
                    )?;
                }
                return Ok(());
            }
            NodeKind::Expr(ExprKind::Operation {
                op:
                    Operation::PostIncrement
                    | Operation::PreIncrement
                    | Operation::PostDecrement
                    | Operation::PreDecrement
                    | Operation::Assignment,
                operands,
                ..
            }) => {
                if let Some(lhs) = operands.first() {
                    self.add_process_lhs_write_bound(*lhs, writes, bindings);
                }
            }
            NodeKind::MethodCall {
                name,
                receiver: Some(receiver),
                ..
            } if Self::mutating_container_method(name) => {
                self.add_process_lhs_write_bound(*receiver, writes, bindings);
            }
            NodeKind::FuncCall {
                name,
                is_task,
                callee,
                ..
            } => {
                let (ft, callee_inst) =
                    self.resolve_callee_env(self.inst, name, *is_task, *callee)?;
                let (_, _, formals) = self.func_info(ft, callee_inst)?;
                let mut callee_bindings = HashMap::new();
                let recursive = visited_functions.contains(&ft);
                for ((formal, is_out), actual) in formals.iter().zip(&self.node(node).children) {
                    let is_ref = matches!(
                        self.kind(*formal),
                        NodeKind::FuncArg {
                            direction: DbDirection::Ref,
                            ..
                        }
                    );
                    let actual = self.unwrap_output_actual(*actual);
                    if is_ref && !recursive {
                        if let Some(prefix) = self.packed_storage_prefix_bound(actual, bindings) {
                            callee_bindings.insert(*formal, prefix);
                        } else if !matches!(
                            self.kind(*formal),
                            NodeKind::FuncArg {
                                const_ref: true,
                                ..
                            }
                        ) {
                            self.add_process_lhs_write_bound(actual, writes, bindings);
                        }
                    } else if *is_out
                        || (is_ref
                            && !matches!(
                                self.kind(*formal),
                                NodeKind::FuncArg {
                                    const_ref: true,
                                    ..
                                }
                            ))
                    {
                        self.add_process_lhs_write_bound(actual, writes, bindings);
                    }
                }
                if visited_functions.insert(ft) {
                    if let Some(body) = self.func_body(ft) {
                        self.walk_process_writes_bound(
                            body,
                            writes,
                            visited_functions,
                            &callee_bindings,
                            mode,
                        )?;
                    }
                    visited_functions.remove(&ft);
                }
            }
            _ => {}
        }
        for child in &self.node(node).children {
            self.walk_process_writes_bound(*child, writes, visited_functions, bindings, mode)?;
        }
        Ok(())
    }

    fn mutating_container_method(name: &str) -> bool {
        matches!(
            name,
            "delete"
                | "push_front"
                | "push_back"
                | "pop_front"
                | "pop_back"
                | "insert"
                | "sort"
                | "rsort"
                | "reverse"
                | "shuffle"
        )
    }

    fn unwrap_output_actual(&self, node: NodeId) -> NodeId {
        match self.kind(node) {
            NodeKind::Expr(ExprKind::Operation { op, operands, .. })
                if *op == Operation::Assignment =>
            {
                operands.first().copied().unwrap_or(node)
            }
            _ => node,
        }
    }

    pub(super) fn add_process_lhs_write(&self, lhs: NodeId, writes: &mut HashSet<IrDependency>) {
        self.add_process_lhs_write_bound(lhs, writes, &HashMap::new());
    }

    fn add_process_lhs_write_bound(
        &self,
        lhs: NodeId,
        writes: &mut HashSet<IrDependency>,
        bindings: &HashMap<NodeId, IrDependency>,
    ) {
        if let Some(expression) = self.modport_expression_target(lhs) {
            self.add_process_lhs_write_bound(expression, writes, bindings);
            return;
        }
        // A string store publishes the same marker its readers wait on, so it
        // is both a writer identity and a combinational read exclusion.
        if let Some(object) = self.object_of("", lhs).filter(|object| {
            self.model.objects.get(*object).map(|o| o.ty) == Some(IrObjectType::String)
        }) {
            writes.insert(IrDependency::Object(object));
            return;
        }
        if let Some(dependencies) = self.unpacked_storage_dependencies(lhs) {
            writes.extend(dependencies);
            return;
        }
        if let Some(prefix) = self.packed_storage_prefix_bound(lhs, bindings) {
            writes.insert(prefix);
            return;
        }
        match self.kind(lhs) {
            NodeKind::Net { .. } | NodeKind::Var { .. } => {
                if let Some(info) = self.signal_of(lhs) {
                    writes.insert(self.signal_dependency(info));
                }
            }
            // An initialized unpacked-array declaration writes its whole
            // contents, like a scalar declaration writes its signal.
            NodeKind::Array { .. } => {
                if let Some(array) = self.array_of(lhs) {
                    writes.insert(IrDependency::ArrayContents(self.reference_array(array.ir)));
                }
            }
            NodeKind::FuncTask { .. } => {
                // Slang may bind an assignment to the function-name result
                // directly to its subroutine symbol. A statically exposed
                // result uses the signal registered for its implicit Var.
                if let Some(result) = self
                    .node(lhs)
                    .children
                    .iter()
                    .copied()
                    .find(|child| matches!(self.kind(*child), NodeKind::Var { .. }))
                {
                    if let Some(info) = self.signal_of(result) {
                        writes.insert(self.signal_dependency(info));
                    }
                }
            }
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) => {
                if let Some(array) = self.array_of(*target) {
                    writes.insert(IrDependency::ArrayContents(self.reference_array(array.ir)));
                } else if let Some(container) = self.container_of(*target) {
                    writes.insert(IrDependency::ContainerContents(container.ir));
                    writes.insert(IrDependency::ContainerShape(container.ir));
                } else if let Some(info) = self.signal_of(*target) {
                    writes.insert(self.signal_dependency(info));
                }
            }
            NodeKind::Expr(ExprKind::HierPath { .. }) => {
                if let Some((_, _, member)) = self.unpacked_member_info(lhs) {
                    if let Some(signal) = member.signal.as_ref() {
                        writes.insert(self.signal_dependency(signal));
                    }
                } else if let Some(info) = self.hier_path_signal(lhs) {
                    writes.insert(self.signal_dependency(info));
                }
            }
            NodeKind::Expr(ExprKind::BitSelect { base, index }) => {
                if let Some(array) = self.array_of(*base) {
                    self.add_process_array_write(array, &[*index], writes);
                } else if let Some(container) = self.container_of(*base) {
                    writes.insert(IrDependency::ContainerContents(container.ir));
                    writes.insert(IrDependency::ContainerShape(container.ir));
                } else {
                    self.add_process_lhs_write_bound(*base, writes, bindings);
                }
            }
            NodeKind::Expr(ExprKind::ArraySelect { base, indices }) => {
                if let Some(array) = self.select_array_of(lhs, *base) {
                    self.add_process_array_write(array, indices, writes);
                } else if let Some(container) = self.container_of(*base) {
                    writes.insert(IrDependency::ContainerContents(container.ir));
                    writes.insert(IrDependency::ContainerShape(container.ir));
                } else {
                    self.add_process_lhs_write_bound(*base, writes, bindings);
                }
            }
            NodeKind::Expr(
                ExprKind::PartSelect { base, .. } | ExprKind::IndexedPartSelect { base, .. },
            ) => {
                self.add_process_lhs_write_bound(*base, writes, bindings);
            }
            NodeKind::Expr(ExprKind::Operation { operands, .. }) => {
                for operand in operands {
                    self.add_process_lhs_write_bound(*operand, writes, bindings);
                }
            }
            _ => {}
        }
    }

    fn add_process_array_write(
        &self,
        array: &ArrayInfo,
        indices: &[NodeId],
        writes: &mut HashSet<IrDependency>,
    ) {
        // A constant row is its longest static prefix (SV 6.5): it writes
        // only its own cells. Bounded rows record those cells so disjoint row
        // writers do not conflict; wider rows record one cell interval.
        if let Some((first, count)) = self.constant_row_cells(array, indices) {
            let reference = self.reference_array(array.ir);
            if count <= PRECISE_ROW_WRITE_CELLS {
                writes.extend(
                    (first..first + count).map(|index| IrDependency::ArrayElement {
                        array: reference,
                        index,
                    }),
                );
                return;
            }
            if let Some(row) = self.array_row_write(reference, first, count) {
                writes.insert(row);
                return;
            }
        }
        let mut dependencies = Vec::new();
        let mut seen = HashSet::new();
        self.add_fixed_array_dependency(array, indices, &mut seen, &mut dependencies);
        writes.extend(dependencies);
    }

    /// Flattened `(first, count)` cells of a constant row/slab select that
    /// names fewer indices than the array has dimensions.
    pub(super) fn constant_row_cells(
        &self,
        array: &ArrayInfo,
        indices: &[NodeId],
    ) -> Option<(u64, u64)> {
        if indices.is_empty() || indices.len() >= array.dims.len() {
            return None;
        }
        let mut prefix = 0u64;
        for ((left, right), node) in array.dims.iter().zip(indices) {
            let value = self.eval_bound_i128(*node).ok()?;
            let low = i128::from((*left).min(*right));
            let high = i128::from((*left).max(*right));
            if value < low || value > high {
                return None;
            }
            let offset = if left >= right {
                i128::from(*left) - value
            } else {
                value - i128::from(*left)
            };
            let extent = (i64::from(*left) - i64::from(*right)).unsigned_abs() + 1;
            prefix = prefix
                .checked_mul(extent)?
                .checked_add(u64::try_from(offset).ok()?)?;
        }
        let count = array.dims[indices.len()..]
            .iter()
            .try_fold(1u64, |count, (left, right)| {
                count.checked_mul((i64::from(*left) - i64::from(*right)).unsigned_abs() + 1)
            })?;
        Some((prefix.checked_mul(count)?, count))
    }

    /// Collect force-expression dependencies, including real-valued storage.
    /// Unlike a combinational process sensitivity list, a force evaluator is
    /// driven by the runtime's typed dependency table, so real reads are
    /// observable without requiring a packed wait source.
    /// Split a force RHS read set into whole-signal names and fixed-array
    /// dependencies; the latter re-run the evaluator through the array's
    /// element or contents change marker. Native containers and objects
    /// have no force dependency contract.
    pub(in super::super) fn collect_force_read_signals(
        &self,
        scope_path: &str,
        root: NodeId,
    ) -> Result<(Vec<String>, Vec<IrDependency>), String> {
        let mut names = Vec::new();
        let mut arrays = Vec::new();
        for dependency in self.collect_read_dependencies(scope_path, root, true)? {
            match dependency {
                IrDependency::Scalar(name) | IrDependency::Real(name) => names.push(name),
                IrDependency::PackedRange { storage, .. } => match *storage {
                    IrDependency::Scalar(name) => names.push(name),
                    element @ IrDependency::ArrayElement { .. } => {
                        if !arrays.contains(&element) {
                            arrays.push(element);
                        }
                    }
                    _ => {
                        return Err(format!(
                            "this selected dependency cannot drive a force evaluator in `{scope_path}`"
                        ))
                    }
                },
                dependency @ (IrDependency::ArrayElement { .. }
                | IrDependency::ArrayContents(_)) => {
                    if !arrays.contains(&dependency) {
                        arrays.push(dependency);
                    }
                }
                IrDependency::ContainerContents(_)
                | IrDependency::ContainerShape(_)
                | IrDependency::Object(_) => {
                    return Err(format!(
                        "container/object dependencies cannot yet drive force evaluators in `{scope_path}`"
                    ))
                }
            }
        }
        Ok((names, arrays))
    }

    fn collect_read_dependencies(
        &self,
        scope_path: &str,
        root: NodeId,
        _allow_real: bool,
    ) -> Result<Vec<IrDependency>, String> {
        self.collect_read_dependencies_mode(scope_path, root, true)
    }

    fn collect_read_dependencies_mode(
        &self,
        scope_path: &str,
        root: NodeId,
        include_function_bodies: bool,
    ) -> Result<Vec<IrDependency>, String> {
        let mut out = Vec::new();
        let mut seen: HashSet<IrDependency> = HashSet::new();
        let mut visited: HashSet<NodeId> = HashSet::new();
        self.walk_read_signals_mode(
            scope_path,
            root,
            &mut seen,
            &mut visited,
            &mut out,
            include_function_bodies,
        )?;
        Ok(out)
    }

    pub(in super::super) fn walk_read_signals(
        &self,
        scope_path: &str,
        node: NodeId,
        seen: &mut HashSet<IrDependency>,
        visited: &mut HashSet<NodeId>,
        out: &mut Vec<IrDependency>,
    ) -> Result<(), String> {
        self.walk_read_signals_mode(scope_path, node, seen, visited, out, true)
    }

    fn walk_read_signals_mode(
        &self,
        scope_path: &str,
        node: NodeId,
        seen: &mut HashSet<IrDependency>,
        visited: &mut HashSet<NodeId>,
        out: &mut Vec<IrDependency>,
        include_function_bodies: bool,
    ) -> Result<(), String> {
        self.walk_read_signals_bound(
            scope_path,
            node,
            seen,
            visited,
            out,
            include_function_bodies,
            &HashMap::new(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn walk_read_signals_bound(
        &self,
        scope_path: &str,
        node: NodeId,
        seen: &mut HashSet<IrDependency>,
        visited: &mut HashSet<NodeId>,
        out: &mut Vec<IrDependency>,
        include_function_bodies: bool,
        bindings: &HashMap<NodeId, IrDependency>,
    ) -> Result<(), String> {
        if let Some(expression) = self.modport_expression_target(node) {
            return self.walk_read_signals_bound(
                scope_path,
                expression,
                seen,
                visited,
                out,
                include_function_bodies,
                bindings,
            );
        }
        if let NodeKind::Expr(ExprKind::Ref {
            target: Some(target),
        }) = self.kind(node)
        {
            if let Some(prefix) = bindings.get(target) {
                self.add_dependency(prefix.clone(), seen, out);
                return Ok(());
            }
        }
        if self.is_process_self_call(node) {
            return Ok(());
        }
        if self.is_semaphore_constructor_call(node) {
            for child in &self.node(node).children {
                self.walk_read_signals_bound(
                    scope_path,
                    *child,
                    seen,
                    visited,
                    out,
                    include_function_bodies,
                    bindings,
                )?;
            }
            return Ok(());
        }
        if self.is_mailbox_constructor_call(node) {
            for child in &self.node(node).children {
                self.walk_read_signals_bound(
                    scope_path,
                    *child,
                    seen,
                    visited,
                    out,
                    include_function_bodies,
                    bindings,
                )?;
            }
            return Ok(());
        }
        if let Some(object) = self.object_of(scope_path, node) {
            // Native strings publish a change marker on every changed store,
            // including record members with their own string owner. Other
            // native objects have no change marker.
            if self.model.objects.get(object).map(|object| object.ty) == Some(IrObjectType::String)
            {
                self.add_dependency(IrDependency::Object(object), seen, out);
                return Ok(());
            }
            return Err(format!("chandle/handle changes cannot yet be used in sensitivity or wait expressions in `{scope_path}`"));
        }
        if matches!(
            self.kind(node),
            NodeKind::Expr(
                ExprKind::BitSelect { .. }
                    | ExprKind::ArraySelect { .. }
                    | ExprKind::PartSelect { .. }
                    | ExprKind::IndexedPartSelect { .. }
                    | ExprKind::HierPath { .. }
            )
        ) {
            if let Some(prefix) = self.packed_storage_prefix_bound(node, bindings) {
                self.add_dependency(prefix, seen, out);
                return self.walk_lhs_select_reads_bound(
                    scope_path,
                    node,
                    seen,
                    visited,
                    out,
                    include_function_bodies,
                    bindings,
                );
            }
        }
        match self.kind(node) {
            NodeKind::Expr(ExprKind::BitSelect { base, index }) => {
                if let Some(array) = self.array_of(*base) {
                    self.add_fixed_array_dependency(array, &[*index], seen, out);
                    return self.walk_read_signals_bound(
                        scope_path,
                        *index,
                        seen,
                        visited,
                        out,
                        include_function_bodies,
                        bindings,
                    );
                }
                if let Some(container) = self.container_of(*base) {
                    self.add_container_dependencies(container.ir, true, true, seen, out);
                    return self.walk_read_signals_bound(
                        scope_path,
                        *index,
                        seen,
                        visited,
                        out,
                        include_function_bodies,
                        bindings,
                    );
                }
            }
            NodeKind::Expr(ExprKind::ArraySelect { base, indices }) => {
                if let Some(array) = self.select_array_of(node, *base) {
                    self.add_fixed_array_dependency(array, indices, seen, out);
                    for index in indices {
                        self.walk_read_signals_bound(
                            scope_path,
                            *index,
                            seen,
                            visited,
                            out,
                            include_function_bodies,
                            bindings,
                        )?;
                    }
                    return Ok(());
                }
                if let Some(container) = self.container_of(*base) {
                    self.add_container_dependencies(container.ir, true, true, seen, out);
                    for index in indices {
                        self.walk_read_signals_bound(
                            scope_path,
                            *index,
                            seen,
                            visited,
                            out,
                            include_function_bodies,
                            bindings,
                        )?;
                    }
                    return Ok(());
                }
            }
            NodeKind::MethodCall {
                name,
                receiver: Some(receiver),
                ..
            } => {
                if let Some(container) = self.container_of(*receiver) {
                    // A pure mutation statement has no read of the receiver.
                    // Keeping its storage out of an implicit @* set prevents
                    // an external mutation from re-running a process whose
                    // only container access is its own write. Value-producing
                    // methods (including pop_*) still read the receiver.
                    let receiver_is_read = !matches!(
                        name.as_str(),
                        "delete"
                            | "push_front"
                            | "push_back"
                            | "insert"
                            | "sort"
                            | "rsort"
                            | "reverse"
                            | "shuffle"
                    );
                    if receiver_is_read {
                        let (contents, shape) = match name.as_str() {
                            "size" | "num" | "exists" | "first" | "last" | "next" | "prev" => {
                                (false, true)
                            }
                            _ => (true, true),
                        };
                        self.add_container_dependencies(container.ir, contents, shape, seen, out);
                    }
                    for child in &self.node(node).children {
                        if *child != *receiver {
                            self.walk_read_signals_bound(
                                scope_path,
                                *child,
                                seen,
                                visited,
                                out,
                                include_function_bodies,
                                bindings,
                            )?;
                        }
                    }
                    return Ok(());
                }
            }
            NodeKind::Stmt(StmtKind::Assign { .. })
            | NodeKind::Stmt(StmtKind::ProcContAssign { .. }) => {
                // Sensitivity of a process body: an assignment's LHS base
                // signal must NOT trigger the process (it would self-wake
                // after every write — including the dedicated PCA guard
                // process's writes).  Only the LHS's index/bounds
                // expressions are reads.
                if let Some(rhs) = self.node(node).children.get(1) {
                    self.walk_read_signals_bound(
                        scope_path,
                        *rhs,
                        seen,
                        visited,
                        out,
                        include_function_bodies,
                        bindings,
                    )?;
                }
                if let Some(lhs) = self.node(node).children.first() {
                    self.walk_lhs_select_reads_bound(
                        scope_path,
                        *lhs,
                        seen,
                        visited,
                        out,
                        include_function_bodies,
                        bindings,
                    )?;
                }
                return Ok(());
            }
            NodeKind::Stmt(StmtKind::VariableDecl { declaration }) => {
                if let Some(initializer) = self.db.var_initializer(*declaration) {
                    self.walk_read_signals_bound(
                        scope_path,
                        initializer,
                        seen,
                        visited,
                        out,
                        include_function_bodies,
                        bindings,
                    )?;
                }
                return Ok(());
            }
            NodeKind::Expr(ExprKind::Operation {
                op:
                    Operation::Assignment
                    | Operation::PostIncrement
                    | Operation::PreIncrement
                    | Operation::PostDecrement
                    | Operation::PreDecrement,
                operands,
                ..
            }) => {
                if let Some(rhs) = operands.get(1) {
                    self.walk_read_signals_bound(
                        scope_path,
                        *rhs,
                        seen,
                        visited,
                        out,
                        include_function_bodies,
                        bindings,
                    )?;
                }
                if let Some(lhs) = operands.first() {
                    self.walk_lhs_select_reads_bound(
                        scope_path,
                        *lhs,
                        seen,
                        visited,
                        out,
                        include_function_bodies,
                        bindings,
                    )?;
                }
                return Ok(());
            }
            NodeKind::Stmt(StmtKind::For {
                vars,
                init,
                cond,
                incr,
                body,
            }) => {
                for variable in vars {
                    if let Some(initializer) = self.db.var_initializer(*variable) {
                        self.walk_read_signals_bound(
                            scope_path,
                            initializer,
                            seen,
                            visited,
                            out,
                            include_function_bodies,
                            bindings,
                        )?;
                    }
                }
                for statement in init {
                    self.walk_read_signals_bound(
                        scope_path,
                        *statement,
                        seen,
                        visited,
                        out,
                        include_function_bodies,
                        bindings,
                    )?;
                }
                self.walk_read_signals_bound(
                    scope_path,
                    *cond,
                    seen,
                    visited,
                    out,
                    include_function_bodies,
                    bindings,
                )?;
                self.walk_read_signals_bound(
                    scope_path,
                    *body,
                    seen,
                    visited,
                    out,
                    include_function_bodies,
                    bindings,
                )?;
                for statement in incr {
                    self.walk_read_signals_bound(
                        scope_path,
                        *statement,
                        seen,
                        visited,
                        out,
                        include_function_bodies,
                        bindings,
                    )?;
                }
                return Ok(());
            }
            NodeKind::Stmt(StmtKind::Fork { branches, .. }) => {
                // A fork body reads signals (a `fork … join` branch may block
                // on signals); descend into the branches for comb sensitivity.
                for b in branches {
                    self.walk_read_signals_bound(
                        scope_path,
                        *b,
                        seen,
                        visited,
                        out,
                        include_function_bodies,
                        bindings,
                    )?;
                }
                return Ok(());
            }
            NodeKind::Stmt(StmtKind::WaitFork | StmtKind::DisableFork) => {
                // Neither reads signals: they touch the fork machinery only.
                return Ok(());
            }
            // A call's arguments are walked below; the callee body's reads
            // (assignments to module signals, reads of them) are part of the
            // calling process's sensitivity too.
            NodeKind::FuncCall {
                name,
                is_task,
                callee,
                ..
            } => {
                let (ft, callee_inst) =
                    self.resolve_callee_env(self.inst, name, *is_task, *callee)?;
                // A static function result exposed as persistent signal
                // storage can be written outside the function body (for
                // example by a hierarchical continuous assignment). Calls
                // return that cell, so a continuous assignment sourced by
                // the call must also wake when the cell changes.
                if !*is_task {
                    let return_variable = self
                        .node(ft)
                        .children
                        .iter()
                        .copied()
                        .find(|child| {
                            self.node(*child).name == self.node(ft).name
                                && matches!(self.kind(*child), NodeKind::Var { .. })
                        })
                        .filter(|variable| {
                            self.db.variable_lifetime(*variable) == VariableLifetime::Static
                        });
                    if let Some(info) =
                        return_variable.and_then(|variable| self.signal_of(variable))
                    {
                        self.add_dependency(self.signal_dependency(info), seen, out);
                    }
                }
                let (_, _, formals) = self.func_info(ft, callee_inst)?;
                let mut callee_bindings = HashMap::new();
                for ((formal, is_out), actual) in formals.iter().zip(&self.node(node).children) {
                    let is_ref = matches!(
                        self.kind(*formal),
                        NodeKind::FuncArg {
                            direction: DbDirection::Ref,
                            ..
                        }
                    );
                    let actual = self.unwrap_output_actual(*actual);
                    let prefix = is_ref
                        .then(|| self.packed_storage_prefix_bound(actual, bindings))
                        .flatten();
                    if let Some(prefix) =
                        prefix.filter(|_| include_function_bodies && !visited.contains(&ft))
                    {
                        callee_bindings.insert(*formal, prefix);
                        self.walk_lhs_select_reads_bound(
                            scope_path,
                            actual,
                            seen,
                            visited,
                            out,
                            include_function_bodies,
                            bindings,
                        )?;
                    } else if *is_out
                        && matches!(
                            self.kind(*formal),
                            NodeKind::FuncArg {
                                direction: DbDirection::Output,
                                ..
                            }
                        )
                    {
                        self.walk_lhs_select_reads_bound(
                            scope_path,
                            actual,
                            seen,
                            visited,
                            out,
                            include_function_bodies,
                            bindings,
                        )?;
                    } else {
                        self.walk_read_signals_bound(
                            scope_path,
                            actual,
                            seen,
                            visited,
                            out,
                            include_function_bodies,
                            bindings,
                        )?;
                    }
                }
                if include_function_bodies && visited.insert(ft) {
                    if let Some(body) = self.func_body(ft) {
                        let first = out.len();
                        self.walk_read_signals_bound(
                            scope_path,
                            body,
                            seen,
                            visited,
                            out,
                            include_function_bodies,
                            &callee_bindings,
                        )?;
                        // The callee's lexical activation arrays (descriptor
                        // formals, locals and results) exist only during one
                        // call and publish no change marker; their contents
                        // derive from actuals that are tracked themselves.
                        let mut callee = out.split_off(first);
                        callee.retain(|dependency| !self.activation_dependency(dependency));
                        out.extend(callee);
                    }
                    visited.remove(&ft);
                }
                for (formal, _) in formals.iter().skip(self.node(node).children.len()) {
                    if let NodeKind::FuncArg {
                        default: Some(default),
                        ..
                    } = self.kind(*formal)
                    {
                        self.walk_read_signals_bound(
                            scope_path,
                            *default,
                            seen,
                            visited,
                            out,
                            include_function_bodies,
                            bindings,
                        )?;
                    }
                }
                return Ok(());
            }
            _ => {}
        }
        if let Some(container) = self.container_of(node) {
            self.add_container_dependencies(container.ir, true, true, seen, out);
        }
        if let Some(dependencies) = self.unpacked_storage_dependencies(node) {
            for dependency in dependencies {
                self.add_dependency(dependency, seen, out);
            }
            return Ok(());
        }
        if let NodeKind::Expr(ExprKind::Ref {
            target: Some(target),
        }) = self.kind(node)
        {
            if let Some(dependencies) = self
                .func
                .as_ref()
                .and_then(|func| func.arg_dependencies.get(target))
            {
                for dependency in dependencies {
                    self.add_dependency(dependency.clone(), seen, out);
                }
            }
            if let Some(array) = self.array_of(*target) {
                self.add_dependency(
                    IrDependency::ArrayContents(self.reference_array(array.ir)),
                    seen,
                    out,
                );
                return Ok(());
            }
            if let Some(container) = self.container_of(*target) {
                self.add_container_dependencies(container.ir, true, true, seen, out);
                return Ok(());
            }
        }
        if let NodeKind::Array { .. } = self.kind(node) {
            if let Some(array) = self.array_of(node) {
                self.add_dependency(
                    IrDependency::ArrayContents(self.reference_array(array.ir)),
                    seen,
                    out,
                );
                return Ok(());
            }
        }
        self.add_node_read(node, seen, out);
        for c in &self.node(node).children {
            self.walk_read_signals_bound(
                scope_path,
                *c,
                seen,
                visited,
                out,
                include_function_bodies,
                bindings,
            )?;
        }
        Ok(())
    }

    /// Walk only the index/bounds expressions of an assignment LHS.
    pub(super) fn walk_lhs_select_reads(
        &self,
        scope_path: &str,
        lhs: NodeId,
        seen: &mut HashSet<IrDependency>,
        visited: &mut HashSet<NodeId>,
        out: &mut Vec<IrDependency>,
    ) -> Result<(), String> {
        self.walk_lhs_select_reads_mode(scope_path, lhs, seen, visited, out, true)
    }

    fn walk_lhs_select_reads_mode(
        &self,
        scope_path: &str,
        lhs: NodeId,
        seen: &mut HashSet<IrDependency>,
        visited: &mut HashSet<NodeId>,
        out: &mut Vec<IrDependency>,
        include_function_bodies: bool,
    ) -> Result<(), String> {
        self.walk_lhs_select_reads_bound(
            scope_path,
            lhs,
            seen,
            visited,
            out,
            include_function_bodies,
            &HashMap::new(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn walk_lhs_select_reads_bound(
        &self,
        scope_path: &str,
        lhs: NodeId,
        seen: &mut HashSet<IrDependency>,
        visited: &mut HashSet<NodeId>,
        out: &mut Vec<IrDependency>,
        include_function_bodies: bool,
        bindings: &HashMap<NodeId, IrDependency>,
    ) -> Result<(), String> {
        if let Some(expression) = self.modport_expression_target(lhs) {
            // The port expression's own selectors are reads of the writer;
            // a concatenation target selects through each operand.
            let parts = match self.kind(expression) {
                NodeKind::Expr(ExprKind::Operation {
                    op: Operation::Concat,
                    operands,
                    ..
                }) => operands.clone(),
                _ => vec![expression],
            };
            for part in parts {
                self.walk_lhs_select_reads_bound(
                    scope_path,
                    part,
                    seen,
                    visited,
                    out,
                    include_function_bodies,
                    bindings,
                )?;
            }
            return Ok(());
        }
        if let NodeKind::Expr(
            ExprKind::BitSelect { base, .. }
            | ExprKind::ArraySelect { base, .. }
            | ExprKind::PartSelect { base, .. }
            | ExprKind::IndexedPartSelect { base, .. },
        ) = self.kind(lhs)
        {
            self.walk_lhs_select_reads_bound(
                scope_path,
                *base,
                seen,
                visited,
                out,
                include_function_bodies,
                bindings,
            )?;
        }
        match self.kind(lhs) {
            NodeKind::Expr(ExprKind::BitSelect { index, .. }) => self.walk_read_signals_bound(
                scope_path,
                *index,
                seen,
                visited,
                out,
                include_function_bodies,
                bindings,
            ),
            NodeKind::Expr(ExprKind::PartSelect { left, right, .. }) => {
                self.walk_read_signals_bound(
                    scope_path,
                    *left,
                    seen,
                    visited,
                    out,
                    include_function_bodies,
                    bindings,
                )?;
                self.walk_read_signals_bound(
                    scope_path,
                    *right,
                    seen,
                    visited,
                    out,
                    include_function_bodies,
                    bindings,
                )
            }
            NodeKind::Expr(ExprKind::IndexedPartSelect {
                base_expr,
                width_expr,
                ..
            }) => {
                self.walk_read_signals_bound(
                    scope_path,
                    *base_expr,
                    seen,
                    visited,
                    out,
                    include_function_bodies,
                    bindings,
                )?;
                self.walk_read_signals_bound(
                    scope_path,
                    *width_expr,
                    seen,
                    visited,
                    out,
                    include_function_bodies,
                    bindings,
                )
            }
            NodeKind::Expr(ExprKind::ArraySelect { indices, .. }) => {
                // The base is the array itself (not a read); only the index
                // expressions (and any element-level select bounds) are reads.
                for i in indices {
                    self.walk_read_signals_bound(
                        scope_path,
                        *i,
                        seen,
                        visited,
                        out,
                        include_function_bodies,
                        bindings,
                    )?;
                }
                Ok(())
            }
            NodeKind::Expr(ExprKind::HierPath { refs, .. }) => {
                // A hierarchical LHS base signal must not trigger the owning
                // process (same rule as a plain LHS ref). A member path rooted
                // at an element select (`s[i].lo`) keeps that select node among
                // its references; its selectors choose the written element,
                // so they are reads of the writer.
                for root in refs.iter().flatten() {
                    if matches!(self.kind(*root), NodeKind::Expr(_)) {
                        self.walk_lhs_select_reads_bound(
                            scope_path,
                            *root,
                            seen,
                            visited,
                            out,
                            include_function_bodies,
                            bindings,
                        )?;
                    }
                }
                Ok(())
            }
            _ => Ok(()), // plain ref LHS: not part of the read set
        }
    }

    /// Add `node` to the read set if it is (or resolves to) a signal.
    fn add_node_read(
        &self,
        node: NodeId,
        seen: &mut HashSet<IrDependency>,
        out: &mut Vec<IrDependency>,
    ) {
        match self.kind(node) {
            NodeKind::Net { .. } | NodeKind::Var { .. } => {
                if let Some(info) = self.signal_of(node) {
                    self.add_dependency(self.signal_dependency(info), seen, out);
                }
            }
            NodeKind::Expr(ExprKind::Ref { target: Some(t) }) => {
                if let Some(info) = self
                    .signal_of(*t)
                    .or_else(|| self.static_proc_local_signal(*t))
                {
                    self.add_dependency(self.signal_dependency(info), seen, out);
                }
            }
            NodeKind::Expr(ExprKind::HierPath { .. }) => {
                if let Some(info) = self.hier_path_signal(node) {
                    self.add_dependency(self.signal_dependency(info), seen, out);
                }
            }
            _ => {}
        }
    }

    fn activation_dependency(&self, dependency: &IrDependency) -> bool {
        let array = match dependency {
            IrDependency::ArrayContents(array) | IrDependency::ArrayElement { array, .. } => *array,
            IrDependency::PackedRange { storage, .. } => {
                return self.activation_dependency(storage);
            }
            _ => return false,
        };
        self.model
            .arrays
            .get(array)
            .is_some_and(|array| array.activation)
    }

    fn add_dependency(
        &self,
        dependency: IrDependency,
        seen: &mut HashSet<IrDependency>,
        out: &mut Vec<IrDependency>,
    ) {
        if seen.insert(dependency.clone()) {
            out.push(dependency);
        }
    }

    fn add_container_dependencies(
        &self,
        container: usize,
        contents: bool,
        shape: bool,
        seen: &mut HashSet<IrDependency>,
        out: &mut Vec<IrDependency>,
    ) {
        if contents {
            self.add_dependency(IrDependency::ContainerContents(container), seen, out);
        }
        if shape {
            self.add_dependency(IrDependency::ContainerShape(container), seen, out);
        }
    }

    fn add_fixed_array_dependency(
        &self,
        array: &ArrayInfo,
        indices: &[NodeId],
        seen: &mut HashSet<IrDependency>,
        out: &mut Vec<IrDependency>,
    ) {
        if indices.len() != array.dims.len() {
            self.add_dependency(
                IrDependency::ArrayContents(self.reference_array(array.ir)),
                seen,
                out,
            );
            return;
        }
        let mut linear = 0u64;
        for ((left, right), node) in array.dims.iter().zip(indices) {
            let Some(value) = self.eval_bound_i128(*node).ok() else {
                self.add_dependency(
                    IrDependency::ArrayContents(self.reference_array(array.ir)),
                    seen,
                    out,
                );
                return;
            };
            let lo = i128::from((*left).min(*right));
            let hi = i128::from((*left).max(*right));
            if value < lo || value > hi {
                return;
            }
            let offset = if left >= right {
                i128::from(*left) - value
            } else {
                value - i128::from(*left)
            };
            let extent = (i64::from(*left) - i64::from(*right)).unsigned_abs() + 1;
            linear = match linear
                .checked_mul(extent)
                .and_then(|value| value.checked_add(offset as u64))
            {
                Some(value) => value,
                None => {
                    self.add_dependency(
                        IrDependency::ArrayContents(self.reference_array(array.ir)),
                        seen,
                        out,
                    );
                    return;
                }
            };
        }
        self.add_dependency(
            IrDependency::ArrayElement {
                array: self.reference_array(array.ir),
                index: linear,
            },
            seen,
            out,
        );
    }
}
