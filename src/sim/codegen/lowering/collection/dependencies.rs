//! Dependencies.

use super::super::packed_geometry::PackedSelectDim;
use super::*;
use crate::IdSet;
use std::rc::Rc;

#[cfg(test)]
thread_local! {
    /// Calls of [`Codegen::same_storage`].
    pub(super) static SAME_STORAGE_COMPARISONS: std::cell::Cell<usize> =
        const { std::cell::Cell::new(0) };
}

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

/// What the `ref` formals of a subroutine are bound to, in formal order.
#[derive(Default, PartialEq, Eq, Hash)]
struct RefBindings(Vec<(NodeId, IrDependency)>);

impl RefBindings {
    fn get(&self, formal: &NodeId) -> Option<&IrDependency> {
        self.0
            .iter()
            .find_map(|(bound, prefix)| (bound == formal).then_some(prefix))
    }
}

/// The subroutines a walk over the storage a process uses is inside and the
/// ones it has finished. A walk inlines each callee's body at its call; a
/// subroutine uses the same storage wherever it is called with the same `ref`
/// bindings, so a second such call adds nothing to the set and is not walked
/// again (a library's call graph has exponentially many paths).
#[derive(Default)]
struct CallWalk {
    /// Subroutines on the current call path. A call back into one is cut: the
    /// body being walked adds its storage, and the call's own `ref` actuals
    /// count as a whole.
    active: IdSet<NodeId>,
    walked: HashSet<(NodeId, Rc<RefBindings>)>,
    /// Dependencies so far that a read left out because a caller owns them
    /// (see [`Codegen::declared_in_callee`]). A walk that left one out is not
    /// recorded as finished.
    left_out: usize,
}

impl CallWalk {
    /// Whether to walk the body of `callee` for a call with `bindings`.
    fn enter(&mut self, callee: NodeId, bindings: &Rc<RefBindings>) -> bool {
        // The owned key holds a reference count, not a copy of the bindings.
        !self.walked.contains(&(callee, Rc::clone(bindings))) && self.active.insert(callee)
    }

    /// The walk is past the end of a body entered when `left_out`
    /// dependencies had been left out.
    fn leave(&mut self, callee: NodeId, bindings: Rc<RefBindings>, left_out: usize) {
        self.active.remove(&callee);
        if self.left_out == left_out {
            self.walked.insert((callee, bindings));
        }
    }
}

/// The storage declared inside subroutines, as the dependencies of reads and
/// writes name it.
#[derive(Default)]
pub(super) struct DeclaredStorage {
    storage: HashSet<IrDependency>,
    arrays: HashSet<usize>,
    containers: HashSet<usize>,
}

impl DeclaredStorage {
    pub(super) fn holds(&self, dependency: &IrDependency) -> bool {
        match dependency {
            IrDependency::PackedRange { storage, .. } => self.holds(storage),
            IrDependency::ArrayContents(array) | IrDependency::ArrayElement { array, .. } => {
                self.arrays.contains(array)
            }
            IrDependency::ContainerContents(container)
            | IrDependency::ContainerShape(container) => self.containers.contains(container),
            other => self.storage.contains(other),
        }
    }
}

/// The root or the subroutine body [`Codegen::walk_process_writes`] is in.
#[derive(Default)]
struct WriteBody {
    callee: Option<NodeId>,
    /// How many of the nodes still to visit belong to the bodies around it.
    outside: usize,
    bindings: Rc<RefBindings>,
    /// The dependencies left out when the body was entered.
    left_out: usize,
}

impl<'a> Codegen<'a> {
    /// The descriptor dependency of `ref` formal `target` in a typed body,
    /// where no specialization bound its actual (see `CallShape`).
    pub(in super::super) fn ref_formal_dependency(&self, target: NodeId) -> Option<IrDependency> {
        if !matches!(
            self.kind(target),
            NodeKind::FuncArg {
                direction: DbDirection::Ref,
                ..
            }
        ) {
            return None;
        }
        // A waited record formal bound to an element.
        if let Some(index) = self.record_ref_markers.get(&target) {
            return Some(IrDependency::NativeRefFormal {
                index: *index,
                handle: true,
            });
        }
        let func = self.func.as_ref()?;
        // String and handle `ref` formals name the actual's change marker.
        if let Some(crate::sim::ir::IrStringExpr::FormalRead(index)) = func.string_read.get(&target)
        {
            return Some(IrDependency::NativeRefFormal {
                index: *index,
                handle: false,
            });
        }
        if let Some(IrChandleExpr::FormalRead(index)) = func.chandle_read.get(&target) {
            return Some(IrDependency::NativeRefFormal {
                index: *index,
                handle: true,
            });
        }
        let read = func.arg_ir.get(&target)?;
        let IrExprKind::FormalRead(index) = read.kind() else {
            return None;
        };
        Some(IrDependency::RefFormal {
            index: *index,
            real: read.width == 0,
        })
    }

    /// The dependency of `path` on the column `array` at member depth
    /// `depth`: its one cell when `path` goes on to a constant in-range
    /// element, so that other cells do not wake the reader.
    fn column_dependency(
        &self,
        array: &ArrayInfo,
        path: &[AggregatePathPart],
        depth: usize,
    ) -> IrDependency {
        let array_index = self.reference_array(array.ir);
        let contents = IrDependency::ArrayContents(array_index);
        let Some(indices) = path.get(depth..depth + array.dims.len()) else {
            return contents;
        };
        let mut linear = 0u64;
        for ((left, right), part) in array.dims.iter().zip(indices) {
            let AggregatePathPart::Index(value) = part else {
                return contents;
            };
            let (left, right, value) = (i64::from(*left), i64::from(*right), i64::from(*value));
            if value < left.min(right) || value > left.max(right) {
                return contents;
            }
            linear = linear * (left.abs_diff(right) + 1) + left.abs_diff(value);
        }
        IrDependency::ArrayElement {
            array: array_index,
            index: linear,
        }
    }

    fn unpacked_storage_dependencies(&self, node: NodeId) -> Option<Vec<IrDependency>> {
        let (root, prefix) = self.unpacked_path_for_expr(node).or_else(|| {
            self.unpacked_aggregate_info(node)
                .map(|(root, _)| (root, Vec::new()))
        })?;
        let storage = self.unpacked_aggregates.get(&root)?;
        let descriptor = self.query_descriptor(root)?;
        let selected = super::fixed_values::fixed_path_descriptor(descriptor, &prefix);
        let mut dependencies = Vec::new();
        let mut member_columns = HashSet::new();
        for leaf in &storage.leaves {
            if !prefix.starts_with(&leaf.path) && !leaf.path.starts_with(&prefix) {
                continue;
            }
            if let Some(array) = &leaf.array {
                // The cell leaves of a member column stand for the column.
                let member_cell = matches!(leaf.path.last(), Some(AggregatePathPart::Index(_)));
                let base = leaf.path.len() - if member_cell { array.dims.len() } else { 0 };
                if member_cell && !member_columns.insert(array.ir) {
                    continue;
                }
                // The tag cell of a column-layout tagged union shares its
                // path with the union, whose members have columns of their own.
                let cell = !member_cell
                    && super::record_columns::record_cell_leaf(leaf)
                    && !super::fixed_values::fixed_path_descriptor(descriptor, &leaf.path)
                        .is_some_and(|(owner, _)| {
                            super::record_columns::column_tagged_union(&owner)
                        });
                let depth = base + if cell { 0 } else { array.dims.len() };
                let dependency = if cell {
                    IrDependency::ArrayElement {
                        array: self.reference_array(array.ir),
                        index: 0,
                    }
                } else {
                    self.column_dependency(array, &prefix[..], base)
                };
                // A member inside one cell is that cell's bits, so writers of
                // different members stay separate (SV 9.2.2.2).
                let member = (prefix.len() > depth).then(|| {
                    let (selected, offset) = selected.as_ref()?;
                    let (_, base) = super::fixed_values::fixed_path_descriptor(
                        descriptor,
                        prefix.get(..depth)?,
                    )?;
                    Some((
                        Self::fixed_descriptor_width(selected)?,
                        offset.checked_sub(base)?,
                    ))
                });
                dependencies.push(match (member.flatten(), &dependency) {
                    (Some((width, offset)), IrDependency::ArrayElement { .. })
                        if offset + width <= array.elem_width =>
                    {
                        self.slice_dependency(dependency, offset, width)
                    }
                    _ => dependency,
                });
                continue;
            }
            // A queue, dynamic or associative member (SIM-007) is read whole.
            if let Some(container) = &leaf.container {
                dependencies.push(IrDependency::ContainerContents(container.ir));
                dependencies.push(IrDependency::ContainerShape(container.ir));
                continue;
            }
            let Some(signal) = &leaf.signal else {
                // String and handle members publish their own change marker;
                // other native members have none (see `walk_read_signals_bound`).
                if let Some(object) = leaf.object.map(|object| self.reference_object(object)) {
                    if self.model.objects[object].ty.has_change_marker() {
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
            // SV 9.4.2.2: `@*` adds every identifier in the statement, so a
            // handle `h` in `h.x` stays in its list.
            self.collect_at_star_signals(scope_path, root)?
        } else if matches!(process_kind, Some(AlwaysKind::Comb | AlwaysKind::Latch)) {
            let previous = self.comb_excludes_class_references.replace(true);
            let reads = self.collect_read_signals(scope_path, root);
            self.comb_excludes_class_references.set(previous);
            reads?
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
    /// functions (formals, locals, static result), which no read walk
    /// reports, and storage they write. Without these exclusions a helper's
    /// own stores would re-trigger its evaluation, and two such waits sharing
    /// a static helper or a counter would wake each other forever.
    pub(in super::super) fn collect_evaluator_sensitivity(
        &self,
        scope_path: &str,
        expression: NodeId,
    ) -> Result<Vec<IrDependency>, String> {
        let writes = self.collect_process_writes(expression)?;
        Ok(self
            .collect_read_signals(scope_path, expression)?
            .into_iter()
            .flat_map(|read| {
                if writes.is_empty() {
                    vec![read]
                } else {
                    self.exclude_written_prefixes(read, &writes)
                }
            })
            .collect())
    }

    /// Add the storage declared inside subroutine `function` (its formals,
    /// result and locals) to `declared`.
    pub(super) fn add_declared_storage(&self, function: NodeId, declared: &mut DeclaredStorage) {
        let mut declarations = vec![function];
        while let Some(node) = declarations.pop() {
            declarations.extend(self.node(node).children.iter().copied());
            if !matches!(
                self.kind(node),
                NodeKind::FuncArg { .. }
                    | NodeKind::FuncTask { .. }
                    | NodeKind::Var { .. }
                    | NodeKind::Array { .. }
            ) {
                continue;
            }
            if let Some(info) = self
                .signal_of(node)
                .or_else(|| self.static_proc_local_signal(node))
            {
                declared.storage.insert(self.signal_dependency(info));
            }
            if let Some(array) = self.array_of(node) {
                declared.arrays.insert(self.reference_array(array.ir));
            }
            if let Some(container) = self.container_of(node) {
                declared.containers.insert(container.ir);
            }
            if let Some(object) = self.object_of("", node) {
                declared.storage.insert(IrDependency::Object(object));
            }
            for dependency in self
                .unpacked_storage_dependencies(node)
                .into_iter()
                .flatten()
            {
                match dependency {
                    IrDependency::ArrayContents(array)
                    | IrDependency::ArrayElement { array, .. } => {
                        declared.arrays.insert(array);
                    }
                    IrDependency::ContainerContents(container)
                    | IrDependency::ContainerShape(container) => {
                        declared.containers.insert(container);
                    }
                    storage => {
                        declared.storage.insert(storage);
                    }
                }
            }
        }
    }

    fn dependency_width(&self, dependency: &IrDependency) -> Option<u32> {
        match dependency {
            IrDependency::Scalar(name) => self
                .signal_by_dependency_name(name)
                .map(|index| self.model.signals[index].ty.width())
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
        bindings: &RefBindings,
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
                    .or_else(|| self.sampled_signal_of(target))
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
        #[cfg(test)]
        SAME_STORAGE_COMPARISONS.with(|work| work.set(work.get() + 1));
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
        self.walk_process_writes(root, ProcessWriteMode::Sensitivity)
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
            self.add_process_lhs_write_bound(*target, &mut target_writes, &RefBindings::default());
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

    /// Whether the run-time selectors of the lvalue `lhs` (its indices and
    /// part-select bounds, not its own storage) can observe storage written
    /// through any of the lvalue nodes in `targets`. A target whose writes
    /// cannot be represented as storage dependencies is treated as
    /// overlapping; an lvalue with only constant selectors never overlaps.
    pub(in super::super) fn lhs_selectors_read_lvalue_writes(
        &self,
        scope_path: &str,
        lhs: NodeId,
        targets: &[NodeId],
    ) -> Result<bool, String> {
        if targets.is_empty() || !self.lhs_has_runtime_selector(lhs) {
            return Ok(false);
        }
        let mut writes = HashSet::new();
        for target in targets {
            let mut target_writes = HashSet::new();
            self.add_process_lhs_write_bound(*target, &mut target_writes, &RefBindings::default());
            if target_writes.is_empty() {
                return Ok(true);
            }
            writes.extend(target_writes);
        }
        let mut reads = Vec::new();
        self.walk_lhs_select_reads(scope_path, lhs, &mut HashSet::new(), &mut reads)?;
        Ok(reads
            .iter()
            .any(|read| writes.iter().any(|write| self.same_storage(read, write))))
    }

    /// Whether `lhs` selects through a non-constant index or bound anywhere
    /// in its selection chain.
    fn lhs_has_runtime_selector(&self, lhs: NodeId) -> bool {
        let mut node = lhs;
        loop {
            let (base, selectors): (NodeId, Vec<NodeId>) = match self.kind(node) {
                NodeKind::Expr(ExprKind::BitSelect { base, index }) => (*base, vec![*index]),
                NodeKind::Expr(ExprKind::ArraySelect { base, indices }) => (*base, indices.clone()),
                NodeKind::Expr(ExprKind::PartSelect { base, left, right }) => {
                    (*base, vec![*left, *right])
                }
                NodeKind::Expr(ExprKind::IndexedPartSelect {
                    base,
                    base_expr,
                    width_expr,
                    ..
                }) => (*base, vec![*base_expr, *width_expr]),
                NodeKind::Expr(ExprKind::MemberSelect { base, .. }) => (*base, Vec::new()),
                _ => return false,
            };
            if selectors.iter().any(|selector| {
                !matches!(
                    self.kind(*selector),
                    NodeKind::Expr(ExprKind::Constant { .. })
                )
            }) {
                return true;
            }
            node = base;
        }
    }

    /// Storage a process owns under the always_comb/always_latch/always_ff
    /// single-writer rule. Overrides (force, release and procedural
    /// continuous assignments) are not ordinary assignments and stay legal
    /// on storage another process owns; their operands still count. Storage
    /// declared inside a called subroutine is not a variable the process
    /// owns: a static local is written by each caller (decision S09-D16).
    pub(super) fn collect_ownership_writes(
        &self,
        root: NodeId,
    ) -> Result<HashSet<IrDependency>, String> {
        self.walk_process_writes(root, ProcessWriteMode::Ownership)
    }

    /// Ordinary procedural assignments conflict with a continuous variable
    /// driver. Force/release are overrides, not competing assignments (SV 6.5).
    pub(super) fn collect_continuous_conflict_writes(
        &self,
        root: NodeId,
    ) -> Result<HashSet<IrDependency>, String> {
        self.walk_process_writes(root, ProcessWriteMode::ContinuousConflict)
    }

    /// The storage `root` writes, with the bodies of the subroutines it calls
    /// inlined. The nodes still to visit are a list, not the thread's stack: a
    /// chain of calls is as long as the design makes it.
    fn walk_process_writes(
        &self,
        root: NodeId,
        mode: ProcessWriteMode,
    ) -> Result<HashSet<IrDependency>, String> {
        let mut writes = HashSet::new();
        let mut calls = CallWalk::default();
        let mut nodes = vec![root];
        let mut body = WriteBody::default();
        let mut outer = Vec::new();
        loop {
            let node = if nodes.len() > body.outside {
                nodes.pop()
            } else {
                None
            };
            let Some(node) = node else {
                let Some(caller) = outer.pop() else {
                    if mode != ProcessWriteMode::Sensitivity {
                        writes.retain(|write| !self.activation_dependency(write));
                    }
                    if mode == ProcessWriteMode::Ownership && !writes.is_empty() {
                        let mut declared = DeclaredStorage::default();
                        for (callee, _) in &calls.walked {
                            self.add_declared_storage(*callee, &mut declared);
                        }
                        writes.retain(|write| !declared.holds(write));
                    }
                    return Ok(writes);
                };
                let left = std::mem::replace(&mut body, caller);
                if let Some(callee) = left.callee {
                    calls.leave(callee, left.bindings, left.left_out);
                }
                continue;
            };
            let children = &self.node(node).children;
            match self.kind(node) {
                NodeKind::Stmt(StmtKind::Force { lhs, rhs })
                    if mode != ProcessWriteMode::Sensitivity =>
                {
                    // The generic children can also contain Slang's assignment
                    // wrapper. Visit only the typed operands so that wrapper does
                    // not reclassify the override as an ordinary variable write.
                    nodes.extend([*rhs, *lhs]);
                    continue;
                }
                NodeKind::Stmt(StmtKind::Release { lhs } | StmtKind::Deassign { lhs })
                    if mode != ProcessWriteMode::Sensitivity =>
                {
                    nodes.push(*lhs);
                    continue;
                }
                NodeKind::Stmt(StmtKind::Assign { .. })
                | NodeKind::Stmt(StmtKind::ProcContAssign { .. })
                | NodeKind::Stmt(StmtKind::Force { .. })
                | NodeKind::ContAssign { .. } => {
                    if let Some(lhs) = children.first() {
                        self.add_process_lhs_write_bound(*lhs, &mut writes, &body.bindings);
                    }
                }
                NodeKind::Stmt(StmtKind::Release { lhs })
                | NodeKind::Stmt(StmtKind::Deassign { lhs }) => {
                    self.add_process_lhs_write_bound(*lhs, &mut writes, &body.bindings);
                    continue;
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
                            self.add_process_lhs_write_bound(
                                *declaration,
                                &mut writes,
                                &body.bindings,
                            );
                        }
                        nodes.push(initializer);
                    }
                    continue;
                }
                NodeKind::Expr(ExprKind::Operation {
                    op:
                        Operation::PostIncrement
                        | Operation::PreIncrement
                        | Operation::PostDecrement
                        | Operation::PreDecrement
                        | Operation::Assignment
                        | Operation::OverloadUpdate
                        | Operation::OverloadPostUpdate,
                    operands,
                    ..
                }) => {
                    if let Some(lhs) = operands.first() {
                        self.add_process_lhs_write_bound(*lhs, &mut writes, &body.bindings);
                    }
                }
                NodeKind::MethodCall {
                    name,
                    receiver: Some(receiver),
                    ..
                } if Self::mutating_container_method(name) => {
                    self.add_process_lhs_write_bound(*receiver, &mut writes, &body.bindings);
                }
                NodeKind::FuncCall { .. } if self.is_process_self_call(node) => continue,
                NodeKind::FuncCall { .. }
                    if self.is_semaphore_constructor_call(node)
                        || self.is_mailbox_constructor_call(node) => {}
                NodeKind::FuncCall {
                    name,
                    is_task,
                    callee,
                    ..
                } => {
                    let (ft, callee_inst) =
                        self.resolve_callee_env(self.inst, name, *is_task, *callee)?;
                    let (_, _, formals) = self.func_info(ft, callee_inst)?;
                    let mut callee_bindings = RefBindings::default();
                    let recursive = calls.active.contains(&ft);
                    for ((formal, is_out), actual) in formals.iter().zip(children) {
                        let is_ref = matches!(
                            self.kind(*formal),
                            NodeKind::FuncArg {
                                direction: DbDirection::Ref,
                                ..
                            }
                        );
                        let actual = self.unwrap_output_actual(*actual);
                        if is_ref && !recursive {
                            if let Some(prefix) =
                                self.packed_storage_prefix_bound(actual, &body.bindings)
                            {
                                callee_bindings.0.push((*formal, prefix));
                            } else if !matches!(
                                self.kind(*formal),
                                NodeKind::FuncArg {
                                    const_ref: true,
                                    ..
                                }
                            ) {
                                self.add_process_lhs_write_bound(
                                    actual,
                                    &mut writes,
                                    &body.bindings,
                                );
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
                            self.add_process_lhs_write_bound(actual, &mut writes, &body.bindings);
                        }
                    }
                    let callee_bindings = Rc::new(callee_bindings);
                    if calls.enter(ft, &callee_bindings) {
                        // The body is walked before the call's other children.
                        for child in children.iter().rev() {
                            nodes.push(*child);
                        }
                        let entered = WriteBody {
                            callee: Some(ft),
                            outside: nodes.len(),
                            bindings: callee_bindings,
                            left_out: calls.left_out,
                        };
                        nodes.extend(self.func_body(ft));
                        outer.push(std::mem::replace(&mut body, entered));
                        continue;
                    }
                }
                _ => {}
            }
            let first = nodes.len();
            nodes.extend(self.db.walk_children(node));
            nodes[first..].reverse();
        }
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
        self.add_process_lhs_write_bound(lhs, writes, &RefBindings::default());
    }

    fn add_process_lhs_write_bound(
        &self,
        lhs: NodeId,
        writes: &mut HashSet<IrDependency>,
        bindings: &RefBindings,
    ) {
        if let Some(expression) = self.modport_expression_target(lhs) {
            self.add_process_lhs_write_bound(expression, writes, bindings);
            return;
        }
        // A string or handle store publishes the same marker its readers
        // wait on, so it is both a writer identity and a combinational read
        // exclusion.
        if let Some(object) = self.object_of("", lhs).filter(|object| {
            self.model
                .objects
                .get(*object)
                .is_some_and(|o| o.ty.has_change_marker())
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
                    if let Some(container) = &member.container {
                        writes.insert(IrDependency::ContainerContents(container.ir));
                        writes.insert(IrDependency::ContainerShape(container.ir));
                    }
                } else if let Some(info) = self.hier_path_signal(lhs) {
                    writes.insert(self.signal_dependency(info));
                }
            }
            NodeKind::Expr(ExprKind::BitSelect { base, index }) => {
                if let Some(array) = self.array_of(*base) {
                    self.add_process_array_write(array, &[*index], writes);
                } else if let Some(container) = self.container_of_select(lhs, *base) {
                    writes.insert(IrDependency::ContainerContents(container.ir));
                    writes.insert(IrDependency::ContainerShape(container.ir));
                } else {
                    self.add_process_lhs_write_bound(*base, writes, bindings);
                }
            }
            NodeKind::Expr(ExprKind::ArraySelect { base, indices }) => {
                if let Some(array) = self.select_array_of(lhs, *base) {
                    self.add_process_array_write(array, indices, writes);
                } else if let Some(container) = self.container_of_select(lhs, *base) {
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
                | IrDependency::Object(_)
                | IrDependency::NativeValue(_)
                | IrDependency::SharedCell { .. }
                | IrDependency::SharedNative { .. }
                | IrDependency::RefFormal { .. }
                | IrDependency::NativeRefFormal { .. }
                | IrDependency::NativeAccess(_)
                | IrDependency::InterfaceInstance { .. } => {
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
        let mut calls = CallWalk::default();
        self.walk_read_signals_mode(
            scope_path,
            root,
            &mut seen,
            &mut calls,
            &mut out,
            include_function_bodies,
        )?;
        Ok(out)
    }

    fn walk_read_signals_mode(
        &self,
        scope_path: &str,
        node: NodeId,
        seen: &mut HashSet<IrDependency>,
        calls: &mut CallWalk,
        out: &mut Vec<IrDependency>,
        include_function_bodies: bool,
    ) -> Result<(), String> {
        self.walk_read_signals_bound(
            scope_path,
            node,
            seen,
            calls,
            out,
            include_function_bodies,
            &RefBindings::default(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn walk_read_signals_bound(
        &self,
        scope_path: &str,
        node: NodeId,
        seen: &mut HashSet<IrDependency>,
        calls: &mut CallWalk,
        out: &mut Vec<IrDependency>,
        include_function_bodies: bool,
        bindings: &RefBindings,
    ) -> Result<(), String> {
        self.check_stack(node)?;
        // Reaching a procedural concurrent assertion only queues it (IEEE
        // 1800-2009 16.15.6): its arguments are sampled at its clock and its
        // actions run in their own processes, so the procedure reads none of
        // them (llg decision S99-D2). Lowering rejects the const-cast and
        // automatic arguments that would be evaluated here.
        if matches!(
            self.kind(node),
            NodeKind::Stmt(StmtKind::ConcurrentAssertion { kind, .. })
                if *kind != ConcurrentAssertionKind::Expect
        ) {
            return Ok(());
        }
        if let Some(expression) = self.modport_expression_target(node) {
            return self.walk_read_signals_bound(
                scope_path,
                expression,
                seen,
                calls,
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
        if let Some(error) = self.dynamic_read_errors.get(&node) {
            return Err(error.clone());
        }
        for dependency in self.dynamic_reads.get(&node).into_iter().flatten() {
            self.add_dependency(dependency.clone(), seen, out);
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
                    calls,
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
                    calls,
                    out,
                    include_function_bodies,
                    bindings,
                )?;
            }
            return Ok(());
        }
        if self.comb_excludes_class_references.get()
            && matches!(self.kind(node), NodeKind::Expr(ExprKind::Ref { .. }))
            && self
                .query_descriptor(node)
                .is_some_and(super::ports::is_class_handle_descriptor)
        {
            // SV 9.2.2.2.1: a direct reference to a class object, such as
            // `h` in `h == null`, adds nothing either (decision AB-N2).
            return Ok(());
        }
        if let Some(object) = self.object_of(scope_path, node) {
            // Native strings and handles publish a change marker on every
            // changed store, including record members with their own owner.
            // Other native objects have no change marker.
            if self
                .model
                .objects
                .get(object)
                .is_some_and(|object| object.ty.has_change_marker())
            {
                self.add_dependency(IrDependency::Object(object), seen, out);
                return Ok(());
            }
            return Err(format!("semaphore or mailbox handle changes cannot yet be used in sensitivity or wait expressions in `{scope_path}`"));
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
                    calls,
                    out,
                    include_function_bodies,
                    bindings,
                );
            }
        }
        if self.comb_excludes_class_references.get() {
            // SV 9.2.2.2.1: a class property path adds nothing, not even its
            // handle variables; a class method call adds only its arguments.
            if matches!(self.kind(node), NodeKind::Expr(ExprKind::HierPath { .. }))
                && self.class_field_target(node).is_some()
            {
                return Ok(());
            }
            if let NodeKind::MethodCall {
                receiver: Some(receiver),
                ..
            } = self.kind(node)
            {
                if self.is_class_method_call(node) {
                    let receiver = *receiver;
                    for child in &self.node(node).children {
                        if *child != receiver {
                            self.walk_read_signals_bound(
                                scope_path,
                                *child,
                                seen,
                                calls,
                                out,
                                include_function_bodies,
                                bindings,
                            )?;
                        }
                    }
                    return Ok(());
                }
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
                        calls,
                        out,
                        include_function_bodies,
                        bindings,
                    );
                }
                if let Some(container) = self.container_of_select(node, *base) {
                    self.add_container_dependencies(node, container.ir, true, true, seen, out);
                    return self.walk_read_signals_bound(
                        scope_path,
                        *index,
                        seen,
                        calls,
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
                            calls,
                            out,
                            include_function_bodies,
                            bindings,
                        )?;
                    }
                    return Ok(());
                }
                if let Some(container) = self.container_of_select(node, *base) {
                    self.add_container_dependencies(node, container.ir, true, true, seen, out);
                    for index in indices {
                        self.walk_read_signals_bound(
                            scope_path,
                            *index,
                            seen,
                            calls,
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
                        self.add_container_dependencies(
                            *receiver,
                            container.ir,
                            contents,
                            shape,
                            seen,
                            out,
                        );
                    }
                    for child in &self.node(node).children {
                        if *child != *receiver {
                            self.walk_read_signals_bound(
                                scope_path,
                                *child,
                                seen,
                                calls,
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
                // after every write).  Only the LHS's index/bounds
                // expressions are reads.
                if let Some(rhs) = self.node(node).children.get(1) {
                    self.walk_read_signals_bound(
                        scope_path,
                        *rhs,
                        seen,
                        calls,
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
                        calls,
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
                        calls,
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
                    | Operation::PreDecrement
                    | Operation::OverloadUpdate
                    | Operation::OverloadPostUpdate,
                operands,
                ..
            }) => {
                if let Some(rhs) = operands.get(1) {
                    self.walk_read_signals_bound(
                        scope_path,
                        *rhs,
                        seen,
                        calls,
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
                        calls,
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
                            calls,
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
                        calls,
                        out,
                        include_function_bodies,
                        bindings,
                    )?;
                }
                self.walk_read_signals_bound(
                    scope_path,
                    *cond,
                    seen,
                    calls,
                    out,
                    include_function_bodies,
                    bindings,
                )?;
                self.walk_read_signals_bound(
                    scope_path,
                    *body,
                    seen,
                    calls,
                    out,
                    include_function_bodies,
                    bindings,
                )?;
                for statement in incr {
                    self.walk_read_signals_bound(
                        scope_path,
                        *statement,
                        seen,
                        calls,
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
                        calls,
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
                let mut callee_bindings = RefBindings::default();
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
                        prefix.filter(|_| include_function_bodies && !calls.active.contains(&ft))
                    {
                        callee_bindings.0.push((*formal, prefix));
                        self.walk_lhs_select_reads_bound(
                            scope_path,
                            actual,
                            seen,
                            calls,
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
                            calls,
                            out,
                            include_function_bodies,
                            bindings,
                        )?;
                    } else {
                        self.walk_read_signals_bound(
                            scope_path,
                            actual,
                            seen,
                            calls,
                            out,
                            include_function_bodies,
                            bindings,
                        )?;
                    }
                }
                let callee_bindings = Rc::new(callee_bindings);
                if include_function_bodies && calls.enter(ft, &callee_bindings) {
                    let left_out = calls.left_out;
                    if let Some(body) = self.func_body(ft) {
                        let first = out.len();
                        self.walk_read_signals_bound(
                            scope_path,
                            body,
                            seen,
                            calls,
                            out,
                            include_function_bodies,
                            &callee_bindings,
                        )?;
                        // Storage declared within a called function adds
                        // nothing (9.2.2.2.1): an activation exists only
                        // during one call, and a static formal, local or
                        // result that a call stores would wake every other
                        // caller. The actuals are tracked themselves.
                        if out.len() > first {
                            let mut declared = DeclaredStorage::default();
                            self.add_declared_storage(ft, &mut declared);
                            let callee = out.split_off(first);
                            for dependency in callee {
                                if self.activation_dependency(&dependency)
                                    || declared.holds(&dependency)
                                {
                                    seen.remove(&dependency);
                                } else {
                                    out.push(dependency);
                                }
                            }
                        }
                    }
                    calls.leave(ft, callee_bindings, left_out);
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
                            calls,
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
            self.add_container_dependencies(node, container.ir, true, true, seen, out);
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
            } else if let Some(dependency) = self.ref_formal_dependency(*target) {
                self.add_dependency(dependency, seen, out);
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
                self.add_container_dependencies(node, container.ir, true, true, seen, out);
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
        if let Some(dependency) = self.static_record_dependency(node, calls) {
            self.add_dependency(dependency, seen, out);
        }
        for dependency in self.static_column_dependencies(node, calls) {
            self.add_dependency(dependency, seen, out);
        }
        self.add_node_read(node, seen, out);
        for c in &self.node(node).children {
            self.walk_read_signals_bound(
                scope_path,
                *c,
                seen,
                calls,
                out,
                include_function_bodies,
                bindings,
            )?;
        }
        Ok(())
    }

    /// The model-lifetime native record `node` reads below: a static
    /// subroutine record with string, real, handle or container members is
    /// one runtime value, whose leaves have no storage a wait could name.
    pub(in super::super) fn static_record_read(&self, node: NodeId) -> Option<usize> {
        let (value, _) = self.native_path_of(node).ok().flatten()?;
        let storage = self.model.native_values.get(value)?;
        (!storage.activation && storage.class_field.is_none()).then_some(value)
    }

    /// Whether `declaration` belongs to a subroutine whose body the walk is
    /// in (`calls`). Such storage adds nothing: the callee's own stores
    /// would wake its caller again, as for the storage
    /// `collect_evaluator_sensitivity` leaves out.
    fn declared_in_callee(&self, declaration: NodeId, node: NodeId, calls: &mut CallWalk) -> bool {
        let owner = if matches!(self.kind(declaration), NodeKind::FuncTask { .. }) {
            Some(declaration)
        } else {
            self.enclosing_func_task(declaration)
        };
        let left_out = owner.is_some_and(|owner| calls.active.contains(&owner));
        // Outside its own body the answer depends on the call path.
        if left_out && owner != self.enclosing_func_task(node) {
            calls.left_out += 1;
        }
        left_out
    }

    /// The contents marker of the static record `node` reads below, which
    /// every store into the record toggles.
    fn static_record_dependency(&self, node: NodeId, calls: &mut CallWalk) -> Option<IrDependency> {
        let value = self.static_record_read(node)?;
        let declaration = self.native_value_layouts.get(&value).copied()?;
        if self.declared_in_callee(declaration, node, calls) {
            return None;
        }
        self.native_value_markers.borrow_mut().insert(value);
        Some(IrDependency::NativeValue(value))
    }

    /// The storage a read of a column-layout static subroutine record names
    /// at `node` beyond its member-array columns, which the select walk
    /// adds: the one-cell column of each scalar member and the contents
    /// marker of the native value that holds the string, real and handle
    /// members. An automatic record's columns live in its activation and
    /// publish nothing.
    fn static_column_dependencies(&self, node: NodeId, calls: &mut CallWalk) -> Vec<IrDependency> {
        let Some((root, path)) = self.activation_record_path(node) else {
            return Vec::new();
        };
        let Some(record) = self.activation_records.get(&root) else {
            return Vec::new();
        };
        if self.declared_in_callee(root, node, calls) {
            return Vec::new();
        }
        let mut dependencies = Vec::new();
        for (column_path, column) in &record.columns {
            if !column_path.starts_with(&path) && !path.starts_with(column_path) {
                continue;
            }
            match column {
                super::RecordColumn::Cell(array) => {
                    let dependency = IrDependency::ArrayContents(self.reference_array(*array));
                    if !self.activation_dependency(&dependency) {
                        dependencies.push(dependency);
                    }
                }
                super::RecordColumn::Native(value, _)
                    if !self.model.native_values[*value].activation =>
                {
                    self.native_value_markers.borrow_mut().insert(*value);
                    dependencies.push(IrDependency::NativeValue(*value));
                }
                _ => {}
            }
        }
        dependencies
    }

    /// Whether evaluating `root` reads a native record value of a subroutine
    /// or block. A static one publishes through one contents marker, whose
    /// toggle is no change of the value read, and an automatic one has no
    /// storage a wait could name: an event control on either compares
    /// values in the waiting process, which also rejects the automatic one
    /// a fork shares (`reject_shared_automatic_records`).
    pub(in super::super) fn reads_native_record(&self, root: NodeId) -> bool {
        let mut pending = vec![root];
        while let Some(node) = pending.pop() {
            if matches!(self.native_path_of(node), Ok(Some(_)))
                || self.record_native_leaf(node).is_some()
            {
                return true;
            }
            pending.extend(self.node(node).children.iter().copied());
        }
        false
    }

    /// Walk only the index/bounds expressions of an assignment LHS.
    pub(super) fn walk_lhs_select_reads(
        &self,
        scope_path: &str,
        lhs: NodeId,
        seen: &mut HashSet<IrDependency>,
        out: &mut Vec<IrDependency>,
    ) -> Result<(), String> {
        self.walk_lhs_select_reads_bound(
            scope_path,
            lhs,
            seen,
            &mut CallWalk::default(),
            out,
            true,
            &RefBindings::default(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn walk_lhs_select_reads_bound(
        &self,
        scope_path: &str,
        lhs: NodeId,
        seen: &mut HashSet<IrDependency>,
        calls: &mut CallWalk,
        out: &mut Vec<IrDependency>,
        include_function_bodies: bool,
        bindings: &RefBindings,
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
                    calls,
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
                calls,
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
                calls,
                out,
                include_function_bodies,
                bindings,
            ),
            NodeKind::Expr(ExprKind::PartSelect { left, right, .. }) => {
                self.walk_read_signals_bound(
                    scope_path,
                    *left,
                    seen,
                    calls,
                    out,
                    include_function_bodies,
                    bindings,
                )?;
                self.walk_read_signals_bound(
                    scope_path,
                    *right,
                    seen,
                    calls,
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
                    calls,
                    out,
                    include_function_bodies,
                    bindings,
                )?;
                self.walk_read_signals_bound(
                    scope_path,
                    *width_expr,
                    seen,
                    calls,
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
                        calls,
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
                            calls,
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
                    .or_else(|| self.sampled_signal_of(*t))
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

    /// Whether `dependency` names the one storage record that stands for
    /// every activation of an automatic declaration (or for what a `ref`
    /// formal aliases). Each activation has its own copy (6.21), so a write
    /// competes with no other process, and no wait outside the activation
    /// can name it.
    pub(super) fn activation_dependency(&self, dependency: &IrDependency) -> bool {
        match dependency {
            IrDependency::ArrayContents(array) | IrDependency::ArrayElement { array, .. } => self
                .model
                .arrays
                .get(*array)
                .is_some_and(|array| array.activation),
            IrDependency::ContainerContents(container)
            | IrDependency::ContainerShape(container) => self
                .model
                .containers
                .get(*container)
                .is_some_and(|container| container.activation),
            IrDependency::NativeValue(value) => self
                .model
                .native_values
                .get(*value)
                .is_some_and(|value| value.activation),
            IrDependency::PackedRange { storage, .. } => self.activation_dependency(storage),
            _ => false,
        }
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

    /// The dependencies of a read of the container `node` names. A class
    /// container property a wait observes is named by the native access
    /// `with_dynamic_reads` resolved for `node` instead.
    fn add_container_dependencies(
        &self,
        node: NodeId,
        container: usize,
        contents: bool,
        shape: bool,
        seen: &mut HashSet<IrDependency>,
        out: &mut Vec<IrDependency>,
    ) {
        // A select names its base declaration; the container expression the
        // dependencies were resolved for is its child.
        let resolved = self.dynamic_reads.get(&node).or_else(|| {
            self.node(node)
                .children
                .iter()
                .find(|child| {
                    self.container_of(**child).is_some_and(|child| {
                        self.model
                            .containers
                            .get(child.ir)
                            .is_some_and(|child| child.class_field.is_some())
                    })
                })
                .and_then(|child| self.dynamic_reads.get(child))
        });
        if let Some(dependencies) = resolved {
            for dependency in dependencies {
                self.add_dependency(dependency.clone(), seen, out);
            }
            return;
        }
        if self.dynamic_read_walk
            && self
                .model
                .containers
                .get(container)
                .is_some_and(|container| container.class_field.is_some())
        {
            return;
        }
        // A mirror of a watched record's member follows the record's marker:
        // loading a mirror publishes nothing.
        if let Some(index) = self.record_ref_markers.iter().find_map(|(formal, index)| {
            self.native_roots
                .get(formal)
                .filter(|value| {
                    self.model.native_values[**value]
                        .companions
                        .contains(&container)
                })
                .map(|_| *index)
        }) {
            self.add_dependency(
                IrDependency::NativeRefFormal {
                    index,
                    handle: true,
                },
                seen,
                out,
            );
            return;
        }
        if contents {
            self.add_dependency(IrDependency::ContainerContents(container), seen, out);
        }
        if shape {
            self.add_dependency(IrDependency::ContainerShape(container), seen, out);
        }
    }

    pub(super) fn add_fixed_array_dependency(
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
