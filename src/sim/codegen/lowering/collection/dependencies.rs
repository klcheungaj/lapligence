//! Dependencies.

use super::*;

impl<'a> Codegen<'a> {
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
        if let Some((_, _, member)) = self.unpacked_member_info(node) {
            if let Some(info) = member.signal.as_ref().filter(|info| !info.real) {
                return Some(self.signal_dependency(info));
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
                    .or_else(|| self.hier_path_signal(node));
                return info
                    .filter(|info| !info.real)
                    .map(|info| self.signal_dependency(info));
            }
        };
        if !indices.is_empty() {
            if let Some(array) = self.array_of(base).filter(|array| !array.real) {
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
        let mut lsb = offset;
        let mut width = base_width;
        if let Some((a, b, indexed)) = bounds {
            let Ok(a) = self.eval_bound_i128(a) else {
                return Some(prefix);
            };
            let Ok(b) = self.eval_bound_i128(b) else {
                return Some(prefix);
            };
            let (left, right) = match indexed {
                None => (a, b),
                Some(neg) => {
                    let delta = b.checked_sub(1).filter(|delta| *delta >= 0)?;
                    (
                        a,
                        if neg {
                            a.checked_sub(delta)?
                        } else {
                            a.checked_add(delta)?
                        },
                    )
                }
            };
            let range = ranges[0];
            let stride =
                u128::from(base_width) / (range.left.abs_diff(range.right).checked_add(1)?);
            let x = self.packed_range_slot(range, left, "dependency").ok()?;
            let y = self.packed_range_slot(range, right, "dependency").ok()?;
            lsb = lsb.checked_add(u32::try_from(x.min(y).checked_mul(stride)?).ok()?)?;
            width = u32::try_from(x.abs_diff(y).checked_add(1)?.checked_mul(stride)?).ok()?;
        } else {
            for (index, range) in indices.iter().zip(&ranges) {
                let Ok(value) = self.eval_bound_i128(*index) else {
                    return Some(self.slice_dependency(storage, lsb, width));
                };
                let extent = range.left.abs_diff(range.right).checked_add(1)?;
                let stride = u128::from(width) / extent;
                let slot = self.packed_range_slot(*range, value, "dependency").ok()?;
                lsb = lsb.checked_add(u32::try_from(slot.checked_mul(stride)?).ok()?)?;
                width = u32::try_from(stride).ok()?;
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
            let whole_write = writes.contains(&read);
            if whole_write {
                return vec![];
            }
            if writes.iter().any(|write| self.same_storage(&read, write)) {
                return (0..self.model.arrays[*array].total())
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

    pub(super) fn same_storage(&self, read: &IrDependency, write: &IrDependency) -> bool {
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
        self.walk_process_writes(root, &mut writes, &mut visited)?;
        Ok(writes)
    }

    fn walk_process_writes(
        &self,
        node: NodeId,
        writes: &mut HashSet<IrDependency>,
        visited_functions: &mut HashSet<NodeId>,
    ) -> Result<(), String> {
        self.walk_process_writes_bound(node, writes, visited_functions, &HashMap::new())
    }

    fn walk_process_writes_bound(
        &self,
        node: NodeId,
        writes: &mut HashSet<IrDependency>,
        visited_functions: &mut HashSet<NodeId>,
        bindings: &HashMap<NodeId, IrDependency>,
    ) -> Result<(), String> {
        if self.is_process_self_call(node) {
            return Ok(());
        }
        if self.is_semaphore_constructor_call(node) {
            for child in &self.node(node).children {
                self.walk_process_writes_bound(*child, writes, visited_functions, bindings)?;
            }
            return Ok(());
        }
        if self.is_mailbox_constructor_call(node) {
            for child in &self.node(node).children {
                self.walk_process_writes_bound(*child, writes, visited_functions, bindings)?;
            }
            return Ok(());
        }
        match self.kind(node) {
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
                // A declaration itself is local storage, not a process write
                // for implicit-sensitivity purposes. Its initializer can
                // still call a side-effecting function.
                if let Some(initializer) = self.db.var_initializer(*declaration) {
                    self.walk_process_writes_bound(
                        initializer,
                        writes,
                        visited_functions,
                        bindings,
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
                        )?;
                    }
                    visited_functions.remove(&ft);
                }
            }
            _ => {}
        }
        for child in &self.node(node).children {
            self.walk_process_writes_bound(*child, writes, visited_functions, bindings)?;
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
                if let Some(array) = self.array_of(*base) {
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
        let mut dependencies = Vec::new();
        let mut seen = HashSet::new();
        self.add_fixed_array_dependency(array, indices, &mut seen, &mut dependencies);
        writes.extend(dependencies);
    }

    /// Collect force-expression dependencies, including real-valued storage.
    /// Unlike a combinational process sensitivity list, a force evaluator is
    /// driven by the runtime's typed dependency table, so real reads are
    /// observable without requiring a packed wait source.
    pub(in super::super) fn collect_force_read_signals(
        &self,
        scope_path: &str,
        root: NodeId,
    ) -> Result<Vec<String>, String> {
        let dependencies = self.collect_read_dependencies(scope_path, root, true)?;
        dependencies
            .into_iter()
            .map(|dependency| match dependency {
                IrDependency::Scalar(name) | IrDependency::Real(name) => Ok(name),
                IrDependency::PackedRange { storage, .. } => storage.scalar_name().map(str::to_owned)
                    .ok_or_else(|| format!("array dependencies cannot yet drive force evaluators in `{scope_path}`")),
                IrDependency::ArrayElement { .. }
                | IrDependency::ArrayContents(_)
                | IrDependency::ContainerContents(_)
                | IrDependency::ContainerShape(_)
                | IrDependency::Object(_) => Err(format!(
                    "array/container dependencies cannot yet drive force evaluators in `{scope_path}`"
                )),
            })
            .collect()
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
        if self.object_of(scope_path, node).is_some() {
            return Err(format!("string/chandle changes cannot yet be used in sensitivity or wait expressions in `{scope_path}`"));
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
                if let Some(array) = self.array_of(*base) {
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
                        self.walk_read_signals_bound(
                            scope_path,
                            body,
                            seen,
                            visited,
                            out,
                            include_function_bodies,
                            &callee_bindings,
                        )?;
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
        if let Some((_, _, member)) = self.unpacked_member_info(node) {
            if let Some(signal) = member.signal.as_ref() {
                self.add_dependency(self.signal_dependency(signal), seen, out);
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
            NodeKind::Expr(ExprKind::HierPath { .. }) => {
                // A hierarchical LHS base signal must not trigger the owning
                // process (same rule as a plain LHS ref). The backend supports only
                // constant indices/bounds on hierarchical targets, so there
                // are no index/bounds reads to collect.
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
                if let Some(info) = self.signal_of(*t) {
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
