//! Ports.

use super::*;

impl<'a> Codegen<'a> {
    // ── Port links ─────────────────────────────────────────────────────────

    pub(in super::super) fn bind_reference_ports(&mut self) -> Result<(), String> {
        for port in self.design_nodes() {
            let NodeKind::Port {
                direction: DbDirection::Ref,
                high_expr,
                high,
                low,
                ..
            } = self.kind(port)
            else {
                continue;
            };
            let actual = high_expr.or(*high).ok_or_else(|| {
                format!("reference port `{}` has no actual", self.display_name(port))
            })?;
            let internal = low.ok_or_else(|| {
                format!(
                    "reference port `{}` has no storage",
                    self.display_name(port)
                )
            })?;
            let path = self
                .owning_inst(port)
                .map(|instance| self.instance_path_of(instance))
                .unwrap_or_default();
            if let Some(child) = self.signal_of(internal).cloned() {
                let target = self.lower_lhs(&path, actual).map_err(|error| {
                    format!(
                        "reference port `{}` has an invalid actual: {error}",
                        self.display_name(port)
                    )
                })?;
                let target = self.reference_lhs(target)?;
                let Some(target_ty) = self.reference_lhs_type(&target) else {
                    return Err(format!(
                        "reference port `{}` requires a typed variable actual",
                        self.display_name(port)
                    ));
                };
                if !self.reference_actual_is_variable(actual)
                    || !self.reference_lhs_is_variable(&target)
                    || self.model.signals[child.ir].ty != target_ty
                {
                    return Err(format!(
                        "reference port `{}` requires matching variable storage",
                        self.display_name(port)
                    ));
                }
                self.reference_signals.insert(child.ir, target);
                continue;
            }

            if let Some(child) = self.array_of(internal).cloned() {
                let Some(actual_array) = self.array_of(actual).cloned() else {
                    return Err(format!(
                        "reference port `{}` requires a matching fixed-array variable actual",
                        self.display_name(port)
                    ));
                };
                if !self.reference_actual_is_variable(actual)
                    || child.is_net
                    || actual_array.is_net
                    || child.dims != actual_array.dims
                    || child.elem_width != actual_array.elem_width
                    || child.signed != actual_array.signed
                    || self.model.arrays[child.ir].two_state
                        != self.model.arrays[actual_array.ir].two_state
                    || child.real != actual_array.real
                    || child.shortreal != actual_array.shortreal
                {
                    return Err(format!(
                        "reference port `{}` requires matching fixed-array variable storage (variable {}/{}, formal {:?}/{}/{}/{}, actual {:?}/{}/{}/{})",
                        self.display_name(port),
                        self.reference_actual_is_variable(actual),
                        child.is_net || actual_array.is_net,
                        child.dims,
                        child.elem_width,
                        child.signed,
                        self.model.arrays[child.ir].two_state,
                        actual_array.dims,
                        actual_array.elem_width,
                        actual_array.signed,
                        self.model.arrays[actual_array.ir].two_state,
                    ));
                }
                self.reference_arrays
                    .insert(child.ir, self.reference_array(actual_array.ir));
                continue;
            }

            if let Some(child) = self.unpacked_aggregates.get(&internal).cloned() {
                self.bind_reference_aggregate(port, &path, actual, child)?;
                continue;
            }

            if let Some(child_object) = self.object_of(&path, internal) {
                let Some(actual_object) = self.object_of(&path, actual) else {
                    return Err(format!(
                        "reference port `{}` requires a matching object variable actual",
                        self.display_name(port)
                    ));
                };
                if !self.reference_actual_is_variable(actual)
                    || self.model.objects[child_object].ty != self.model.objects[actual_object].ty
                {
                    return Err(format!(
                        "reference port `{}` requires matching object storage",
                        self.display_name(port)
                    ));
                }
                self.reference_objects
                    .insert(child_object, self.reference_object(actual_object));
                continue;
            }

            if self.container_of(internal).is_some() {
                return Err(format!(
                    "reference port `{}` cannot bind resizable container storage",
                    self.display_name(port)
                ));
            }
            return Err(format!(
                "reference port `{}` requires a supported variable, array, aggregate, or object actual",
                self.display_name(port)
            ));
        }

        let signal_bindings = self.reference_signals.keys().copied().collect::<Vec<_>>();
        for child in signal_bindings {
            let target = self.reference_lhs(IrLhs::Whole(child))?;
            self.reference_signals.insert(child, target.clone());
            if let IrLhs::Whole(target) = target {
                let canonical = self.model.signals[target].clone();
                self.model.signals[child].alias = Some(target);
                self.model.signals[child].c_name = canonical.c_name;
            }
        }
        let array_bindings = self.reference_arrays.keys().copied().collect::<Vec<_>>();
        for child in array_bindings {
            let target = self.reference_array(child);
            self.reference_arrays.insert(child, target);
        }
        let object_bindings = self.reference_objects.keys().copied().collect::<Vec<_>>();
        for child in object_bindings {
            let target = self.reference_object(child);
            self.reference_objects.insert(child, target);
        }

        let signals = &self.model.signals;
        let canonicalize = |info: &mut SignalInfo| {
            if let Some(target) = signals[info.ir].alias {
                info.ir = target;
                info.global = signals[target].c_name.clone();
            }
        };
        for info in self.sig_globals.values_mut() {
            canonicalize(info);
        }
        for info in &mut self.signals {
            canonicalize(info);
        }
        for scope in self.scope_sig_names.values_mut() {
            for info in scope.values_mut() {
                canonicalize(info);
            }
        }
        Ok(())
    }

    fn bind_reference_aggregate(
        &mut self,
        port: NodeId,
        _path: &str,
        actual: NodeId,
        child: UnpackedAggregateInfo,
    ) -> Result<(), String> {
        let (actual_target, prefix) = self
            .unpacked_aggregate_target(actual)
            .map(|target| (target, Vec::new()))
            .or_else(|| self.unpacked_path_for_expr(actual))
            .ok_or_else(|| {
                format!(
                    "reference port `{}` requires a matching aggregate variable actual",
                    self.display_name(port)
                )
            })?;
        let parent = self
            .unpacked_aggregates
            .get(&actual_target)
            .cloned()
            .ok_or_else(|| {
                format!(
                    "reference port `{}` actual aggregate has no captured storage",
                    self.display_name(port)
                )
            })?;
        if !self.reference_actual_is_variable(actual) {
            return Err(format!(
                "reference port `{}` requires a variable aggregate actual",
                self.display_name(port)
            ));
        }
        if prefix.is_empty()
            && child
                .type_identity
                .as_deref()
                .zip(parent.type_identity.as_deref())
                .is_some_and(|(left, right)| left != right)
        {
            return Err(format!(
                "reference port `{}` requires matching aggregate type",
                self.display_name(port)
            ));
        }
        for child_leaf in &child.leaves {
            let mut parent_path = prefix.clone();
            parent_path.extend(child_leaf.path.iter().cloned());
            let parent_leaf = parent
                .leaves
                .iter()
                .find(|leaf| leaf.path == parent_path)
                .ok_or_else(|| {
                    format!(
                        "reference port `{}` aggregate member `{}` has no matching actual storage",
                        self.display_name(port),
                        aggregate_path_suffix(&child_leaf.path)
                    )
                })?;
            match (
                &child_leaf.signal,
                &parent_leaf.signal,
                child_leaf.object,
                parent_leaf.object,
            ) {
                (Some(child_signal), Some(parent_signal), _, _) => {
                    let target = self.reference_lhs(IrLhs::Whole(parent_signal.ir))?;
                    let Some(target_ty) = self.reference_lhs_type(&target) else {
                        return Err(format!(
                            "reference port `{}` aggregate member `{}` has invalid storage",
                            self.display_name(port),
                            aggregate_path_suffix(&child_leaf.path)
                        ));
                    };
                    if !self.reference_lhs_is_variable(&target)
                        || self.model.signals[child_signal.ir].ty != target_ty
                    {
                        return Err(format!(
                            "reference port `{}` aggregate member `{}` has incompatible storage",
                            self.display_name(port),
                            aggregate_path_suffix(&child_leaf.path)
                        ));
                    }
                    self.reference_signals.insert(child_signal.ir, target);
                }
                (None, None, Some(child_object), Some(parent_object)) => {
                    if self.model.objects[child_object].ty != self.model.objects[parent_object].ty {
                        return Err(format!(
                            "reference port `{}` aggregate member `{}` has incompatible object storage",
                            self.display_name(port),
                            aggregate_path_suffix(&child_leaf.path)
                        ));
                    }
                    self.reference_objects
                        .insert(child_object, self.reference_object(parent_object));
                }
                _ => {
                    return Err(format!(
                        "reference port `{}` aggregate member `{}` has incompatible shape",
                        self.display_name(port),
                        aggregate_path_suffix(&child_leaf.path)
                    ));
                }
            }
        }
        Ok(())
    }

    pub(super) fn remap_structural_lhs(&self, lhs: IrLhs, source: NodeId) -> IrLhs {
        let remap = |index: usize| {
            self.model
                .signals
                .get(index)
                .and_then(|signal| signal.net_driver.map(|(group, _)| group))
                .and_then(|group| self.structural_driver_signal(source, group))
                .unwrap_or(index)
        };
        match lhs {
            IrLhs::Whole(index) => IrLhs::Whole(remap(index)),
            IrLhs::Bit(index, expression, two_state) => {
                IrLhs::Bit(remap(index), expression, two_state)
            }
            IrLhs::Part(index, left, right, two_state) => {
                IrLhs::Part(remap(index), left, right, two_state)
            }
            IrLhs::IdxPart(index, base, width, selected_width, negative, two_state) => {
                IrLhs::IdxPart(
                    remap(index),
                    base,
                    width,
                    selected_width,
                    negative,
                    two_state,
                )
            }
            IrLhs::Stream {
                parts,
                width,
                slice,
                direction,
            } => IrLhs::Stream {
                parts: parts
                    .into_iter()
                    .map(|(part, part_width)| (self.remap_structural_lhs(part, source), part_width))
                    .collect(),
                width,
                slice,
                direction,
            },
            other => other,
        }
    }

    pub(super) fn structural_group_for_lhs(&self, lhs: &IrLhs) -> Option<usize> {
        let signal_group = |index: usize| {
            self.model
                .signals
                .get(index)
                .and_then(|signal| signal.net_driver.map(|(group, _)| group))
        };
        match lhs {
            IrLhs::Whole(index)
            | IrLhs::Bit(index, ..)
            | IrLhs::Part(index, ..)
            | IrLhs::IdxPart(index, ..) => signal_group(*index),
            IrLhs::Stream { parts, .. } => parts
                .iter()
                .find_map(|(part, _)| self.structural_group_for_lhs(part)),
            IrLhs::WholeRef { .. } | IrLhs::Ref { .. } | IrLhs::ArrayElem { .. } => None,
        }
    }

    pub(super) fn remap_structural_lhs_for_terminal(
        &self,
        lhs: IrLhs,
        source: NodeId,
        terminal: usize,
    ) -> IrLhs {
        let remap = |index: usize| {
            self.model
                .signals
                .get(index)
                .and_then(|signal| signal.net_driver.map(|(group, _)| group))
                .and_then(|group| {
                    self.structural_driver_signal_for_terminal(source, group, terminal)
                })
                .unwrap_or(index)
        };
        match lhs {
            IrLhs::Whole(index) => IrLhs::Whole(remap(index)),
            IrLhs::Bit(index, expression, two_state) => {
                IrLhs::Bit(remap(index), expression, two_state)
            }
            IrLhs::Part(index, left, right, two_state) => {
                IrLhs::Part(remap(index), left, right, two_state)
            }
            IrLhs::IdxPart(index, base, width, selected_width, negative, two_state) => {
                IrLhs::IdxPart(
                    remap(index),
                    base,
                    width,
                    selected_width,
                    negative,
                    two_state,
                )
            }
            IrLhs::Stream {
                parts,
                width,
                slice,
                direction,
            } => IrLhs::Stream {
                parts: parts
                    .into_iter()
                    .map(|(part, part_width)| {
                        (
                            self.remap_structural_lhs_for_terminal(part, source, terminal),
                            part_width,
                        )
                    })
                    .collect(),
                width,
                slice,
                direction,
            },
            other => other,
        }
    }

    pub(super) fn unmapped_structural_group(&self, lhs: &IrLhs, source: NodeId) -> Option<usize> {
        let mut mapped = |group| self.structural_driver_signal(source, group);
        self.unmapped_structural_group_for(lhs, &mut mapped)
    }

    pub(super) fn unmapped_structural_group_for_terminal(
        &self,
        lhs: &IrLhs,
        source: NodeId,
        terminal: usize,
    ) -> Option<usize> {
        let mut mapped =
            |group| self.structural_driver_signal_for_terminal(source, group, terminal);
        self.unmapped_structural_group_for(lhs, &mut mapped)
    }

    fn unmapped_structural_group_for(
        &self,
        lhs: &IrLhs,
        mapped: &mut dyn FnMut(usize) -> Option<usize>,
    ) -> Option<usize> {
        match lhs {
            IrLhs::Whole(index)
            | IrLhs::Bit(index, ..)
            | IrLhs::Part(index, ..)
            | IrLhs::IdxPart(index, ..) => self
                .model
                .signals
                .get(*index)
                .and_then(|signal| signal.net_driver.map(|(group, _)| group))
                .filter(|group| mapped(*group).is_none()),
            IrLhs::Stream { parts, .. } => parts
                .iter()
                .find_map(|(part, _)| self.unmapped_structural_group_for(part, mapped)),
            IrLhs::WholeRef { .. } | IrLhs::Ref { .. } | IrLhs::ArrayElem { .. } => None,
        }
    }

    fn emit_link_process(
        &mut self,
        parent_path: &str,
        child_path: &str,
        port: NodeId,
        reads: Vec<IrDependency>,
        body: IrStmt,
    ) {
        let shape = if reads.is_empty() {
            IrShape::RunOnce
        } else {
            IrShape::SensLoop { reads }
        };
        let fn_name = self.new_fn_name(parent_path, "link");
        let origin = self.origin(port);
        self.model.processes.push(IrProcess::new_with_origin(
            fn_name,
            format!("{child_path}.link"),
            shape,
            Vec::new(),
            vec![body],
            origin,
        ));
    }

    /// Resolve a fixed-array value-port actual to the array being connected
    /// plus any leading constant element indices. A whole array (`a`) has no
    /// prefix and its full declared shape; an element select of a higher-rank
    /// array (`a[i]`) contributes the select indices and leaves only the
    /// remaining dimensions as the connected shape.
    fn port_array_actual(&self, actual: NodeId) -> Option<(ArrayInfo, Vec<NodeId>)> {
        if let Some(array) = self.array_of(actual) {
            return Some((array.clone(), Vec::new()));
        }
        let NodeKind::Expr(ExprKind::ArraySelect { base, indices }) = self.kind(actual) else {
            return None;
        };
        let array = self.array_of(*base)?.clone();
        if indices.len() >= array.dims.len() {
            return None;
        }
        let mut shape = array;
        shape.dims = shape.dims[indices.len()..].to_vec();
        Some((shape, indices.clone()))
    }

    fn emit_array_port_link(
        &mut self,
        parent_path: &str,
        child_path: &str,
        port: NodeId,
        direction: DbDirection,
        actual: NodeId,
        internal: NodeId,
    ) -> Result<bool, String> {
        let child_array = self.array_of(internal).cloned();
        let actual_resolved = self.port_array_actual(actual);
        let (child_array, actual_array, actual_prefix) = match (child_array, actual_resolved) {
            (None, None) => return Ok(false),
            (Some(_), None) | (None, Some(_)) => {
                return Err(format!(
                    "port `{}` connects a fixed array to a non-array actual in `{child_path}`",
                    self.display_name(port)
                ));
            }
            (Some(child), Some((actual, prefix))) => (child, actual, prefix),
        };
        if child_array.dims.len() != actual_array.dims.len()
            || child_array.dims.iter().zip(&actual_array.dims).any(
                |((child_left, child_right), (actual_left, actual_right))| {
                    (i64::from(*child_left) - i64::from(*child_right)).unsigned_abs()
                        != (i64::from(*actual_left) - i64::from(*actual_right)).unsigned_abs()
                },
            )
        {
            return Err(format!(
                "fixed array port `{}` has incompatible rank or dimensions in `{child_path}`",
                self.display_name(port)
            ));
        }

        let actual_is_target = direction != DbDirection::Input;
        let (target, source) = if actual_is_target {
            (&actual_array, &child_array)
        } else {
            (&child_array, &actual_array)
        };
        let target_array = self.reference_array(target.ir);
        let source_array = self.reference_array(source.ir);
        // An element actual (`a[i]`) selects leading dimensions of a
        // higher-rank array; those element indices must be elaboration
        // constants so the connected sub-array is fixed at build time.
        let actual_prefix = actual_prefix
            .iter()
            .map(|index| self.eval_bound_i128(*index))
            .collect::<Result<Vec<_>, String>>()
            .map(|indices| {
                indices
                    .into_iter()
                    .map(lhs_integer_expr)
                    .collect::<Vec<IrExpr>>()
            })
            .map_err(|error| {
                format!(
                    "fixed array port `{}` has a non-constant element actual in `{child_path}`: {error}",
                    self.display_name(port)
                )
            })?;
        let target_indices = port_array_index_vectors(&target.dims);
        let source_indices = port_array_index_vectors(&source.dims);
        if target_indices.len() != source_indices.len() {
            return Err(format!(
                "fixed array port `{}` has incompatible element count in `{child_path}`",
                self.display_name(port)
            ));
        }
        let with_prefix = |prefix: &[IrExpr], indices: &[i32]| -> Vec<IrExpr> {
            prefix
                .iter()
                .cloned()
                .chain(
                    indices
                        .iter()
                        .map(|index| lhs_integer_expr(i128::from(*index))),
                )
                .collect()
        };
        let mut assignments = Vec::with_capacity(target_indices.len());
        for (target_indices, source_indices) in target_indices.iter().zip(&source_indices) {
            let (target_index_exprs, source_index_exprs) = if actual_is_target {
                (
                    with_prefix(&actual_prefix, target_indices),
                    with_prefix(&[], source_indices),
                )
            } else {
                (
                    with_prefix(&[], target_indices),
                    with_prefix(&actual_prefix, source_indices),
                )
            };
            let lhs = IrLhs::ArrayElem {
                arr: target_array,
                indices: target_index_exprs,
                elem_sel: IrElemSel::Whole,
            };
            let rhs = IrExpr::new(
                IrExprKind::ArrayRead {
                    arr: source_array,
                    indices: source_index_exprs,
                    elem_sel: IrElemSel::Whole,
                },
                if source.real { 0 } else { source.elem_width },
                source.signed,
                None,
            );
            assignments.push(IrStmt::Assign {
                lhs: lhs.clone(),
                rhs: apply_lhs_assignment_context(&self.model, &lhs, rhs),
                nba: false,
            });
        }
        self.emit_link_process(
            parent_path,
            child_path,
            port,
            vec![IrDependency::ArrayContents(source_array)],
            IrStmt::Block(assignments),
        );
        Ok(true)
    }

    fn aggregate_link_dependencies(&self, aggregate: &UnpackedAggregateInfo) -> Vec<IrDependency> {
        let mut reads = Vec::new();
        for leaf in &aggregate.leaves {
            if let Some(object) = leaf.object {
                let dependency = IrDependency::Object(self.reference_object(object));
                if !reads.contains(&dependency) {
                    reads.push(dependency);
                }
                continue;
            }
            let Some(signal) = &leaf.signal else {
                continue;
            };
            let dependency = self.signal_dependency(signal);
            if !reads.contains(&dependency) {
                reads.push(dependency);
            }
        }
        reads
    }

    fn emit_aggregate_port_link(
        &mut self,
        parent_path: &str,
        child_path: &str,
        port: NodeId,
        direction: DbDirection,
        actual: NodeId,
        internal: NodeId,
    ) -> Result<bool, String> {
        let child_aggregate = self.unpacked_aggregate_info(internal);
        let actual_aggregate = self.unpacked_aggregate_info(actual);
        let (child_aggregate, actual_aggregate) = match (child_aggregate, actual_aggregate) {
            (None, None) => return Ok(false),
            (Some(_), None) | (None, Some(_)) => {
                return Err(format!(
                    "aggregate port `{}` connects to a non-aggregate actual in `{child_path}`",
                    self.display_name(port)
                ));
            }
            (Some(child), Some(actual)) => (child, actual),
        };
        let source = if direction == DbDirection::Input {
            actual_aggregate.1.clone()
        } else {
            child_aggregate.1.clone()
        };
        let (target_node, source_node, path) = if direction == DbDirection::Input {
            (internal, actual, child_path)
        } else {
            (actual, internal, parent_path)
        };
        let reads = self.aggregate_link_dependencies(&source);
        let statement = self
            .lower_unpacked_aggregate_assignment(
                path,
                target_node,
                source_node,
                false,
                Operation::Assignment,
            )?
            .ok_or_else(|| {
                format!(
                    "aggregate port `{}` did not lower as a complete aggregate assignment",
                    self.display_name(port)
                )
            })?;
        self.emit_link_process(parent_path, child_path, port, reads, statement);
        Ok(true)
    }

    fn emit_container_port_link(
        &mut self,
        parent_path: &str,
        child_path: &str,
        port: NodeId,
        direction: DbDirection,
        actual: NodeId,
        internal: NodeId,
    ) -> Result<bool, String> {
        let child_container = self.container_of(internal);
        let actual_container = self.container_of(actual);
        let (child_container, actual_container) = match (child_container, actual_container) {
            (None, None) => return Ok(false),
            (Some(_), None) | (None, Some(_)) => {
                return Err(format!(
                    "resizable port `{}` connects to a non-container actual in `{child_path}`",
                    self.display_name(port)
                ));
            }
            (Some(child), Some(actual)) => (child, actual),
        };
        let (target, source) = if direction == DbDirection::Input {
            (child_container.ir, actual_container.ir)
        } else {
            (actual_container.ir, child_container.ir)
        };
        self.emit_link_process(
            parent_path,
            child_path,
            port,
            vec![
                IrDependency::ContainerContents(source),
                IrDependency::ContainerShape(source),
            ],
            IrStmt::Container(IrContainerStmt::Copy {
                dst: target,
                src: source,
            }),
        );
        Ok(true)
    }

    fn emit_object_port_link(
        &mut self,
        parent_path: &str,
        child_path: &str,
        port: NodeId,
        direction: DbDirection,
        actual: NodeId,
        internal: NodeId,
    ) -> Result<bool, String> {
        let child_object = self.object_of(child_path, internal);
        let actual_object = self.object_of(parent_path, actual);
        let (child_object, actual_object) = match (child_object, actual_object) {
            (None, None) => return Ok(false),
            (Some(_), None) | (None, Some(_)) => {
                return Err(format!(
                    "object port `{}` connects to a non-object actual in `{child_path}`",
                    self.display_name(port)
                ));
            }
            (Some(child), Some(actual)) => (child, actual),
        };
        let (target, source) = if direction == DbDirection::Input {
            (child_object, actual_object)
        } else {
            (actual_object, child_object)
        };
        if self.model.objects[target].ty != IrObjectType::String
            || self.model.objects[source].ty != IrObjectType::String
        {
            return Err(format!(
                "chandle port `{}` is not supported by value links in `{child_path}`",
                self.display_name(port)
            ));
        }
        self.emit_link_process(
            parent_path,
            child_path,
            port,
            vec![IrDependency::Object(source)],
            IrStmt::Object(IrObjectStmt::StringAssign(
                self.reference_object(target),
                IrStringExpr::Read(self.reference_object(source)),
            )),
        );
        Ok(true)
    }

    pub(super) fn emit_links(
        &mut self,
        parent_path: &str,
        child_inst: NodeId,
    ) -> Result<(), String> {
        let child_path = self.instance_path_of(child_inst);
        self.inst = self.owning_inst(child_inst).unwrap_or(child_inst);
        for c in &self.node(child_inst).children {
            let port = *c;
            let (direction, high, low, high_expr, high_present, high_open) = match self.kind(port) {
                NodeKind::Port {
                    direction,
                    high,
                    low,
                    high_expr,
                    high_present,
                    high_open,
                    ..
                } => (
                    *direction,
                    *high,
                    *low,
                    *high_expr,
                    *high_present,
                    *high_open,
                ),
                _ => continue,
            };
            if let Some(actual) =
                self.node(port)
                    .children
                    .iter()
                    .find_map(|cc| match self.kind(*cc) {
                        NodeKind::IfaceConn { actual, .. } => Some(*actual),
                        _ => None,
                    })
            {
                if high != Some(actual) {
                    return Err(format!(
                        "interface port `{}` of `{child_path}` has inconsistent bound actual identities",
                        self.node(port).name
                    ));
                }
                continue;
            }
            if matches!(direction, DbDirection::Inout | DbDirection::Ref) {
                continue;
            }
            let Some(actual) = high_expr.or(high) else {
                if high_present && !high_open {
                    return Err(format!(
                        "port `{}` of `{child_path}` declares a connection but has no actual expression at {}:{}:{}",
                        self.node(port).name,
                        self.node(port).file.as_deref().unwrap_or("<unknown>"),
                        self.node(port).line,
                        self.node(port).col,
                    ));
                }
                continue;
            };
            let Some(internal) = low else {
                return Err(format!(
                    "port `{}` of `{child_path}` has an actual connection but no child-side storage at {}:{}:{}",
                    self.node(port).name,
                    self.node(port).file.as_deref().unwrap_or("<unknown>"),
                    self.node(port).line,
                    self.node(port).col,
                ));
            };
            if self.emit_array_port_link(
                parent_path,
                &child_path,
                port,
                direction,
                actual,
                internal,
            )? {
                continue;
            }
            if self.emit_aggregate_port_link(
                parent_path,
                &child_path,
                port,
                direction,
                actual,
                internal,
            )? {
                continue;
            }
            if self.emit_container_port_link(
                parent_path,
                &child_path,
                port,
                direction,
                actual,
                internal,
            )? {
                continue;
            }
            if self.emit_object_port_link(
                parent_path,
                &child_path,
                port,
                direction,
                actual,
                internal,
            )? {
                continue;
            }
            let (_, child_info) = self.resolve_signal_id(&child_path, internal)?;
            let alias_target = if direction == DbDirection::Input {
                internal
            } else {
                actual
            };
            let alias_bindings = self.alias_lvalue_bindings(port, alias_target)?;
            let (lhs, rhs, reads) = if direction == DbDirection::Input {
                let rhs = self.lower_expr(parent_path, actual)?;
                let reads = self.collect_read_signals(parent_path, actual)?;
                let lhs = self.remap_structural_lhs(IrLhs::Whole(child_info.ir), port);
                if let Some(group) = self.unmapped_structural_group(&lhs, port) {
                    return Err(format!(
                        "input port `{}` has no structural driver mapping for resolved net group {} at {}:{}:{}",
                        self.display_name(port),
                        group,
                        self.node(port).file.as_deref().unwrap_or("<unknown>"),
                        self.node(port).line,
                        self.node(port).col,
                    ));
                }
                (lhs, rhs, reads)
            } else {
                let lhs = self.lower_lhs(parent_path, actual)?;
                if let Some(group) = self.unmapped_structural_group(&lhs, port) {
                    return Err(format!(
                        "output port `{}` has no structural driver mapping for resolved net group {} at {}:{}:{}",
                        self.display_name(port),
                        group,
                        self.node(port).file.as_deref().unwrap_or("<unknown>"),
                        self.node(port).line,
                        self.node(port).col,
                    ));
                }
                let lhs = self.remap_structural_lhs(lhs, port);
                let child_dependency = self.signal_dependency(&child_info);
                let mut reads = vec![child_dependency.clone()];
                let mut seen = HashSet::from([child_dependency]);
                self.walk_lhs_select_reads(
                    parent_path,
                    actual,
                    &mut seen,
                    &mut HashSet::new(),
                    &mut reads,
                )?;
                (lhs, sig_read_expr_full(&child_info), reads)
            };
            let rhs = apply_lhs_assignment_context(&self.model, &lhs, rhs);
            let shape = if reads.is_empty() {
                IrShape::RunOnce
            } else {
                IrShape::SensLoop { reads }
            };
            let fn_name = self.new_fn_name(parent_path, "link");
            let origin = self.origin(port);
            let body = if let Some(bindings) = alias_bindings {
                let rhs_name = format!("_alias_port_rhs_{}", port.index());
                let rhs_value = IrExpr::new(
                    IrExprKind::LocalRead(rhs_name.clone()),
                    rhs.width(),
                    rhs.signed(),
                    None,
                );
                let mut body = vec![IrStmt::DeclLocal {
                    name: rhs_name,
                    width: rhs.width(),
                    signed: rhs.signed(),
                    init: Some(Box::new(rhs)),
                    two_state: false,
                }];
                for (driver, value) in
                    self.alias_driver_assignments(port, &bindings, &rhs_value, |_| 0)?
                {
                    body.push(IrStmt::Assign {
                        lhs: IrLhs::Whole(driver),
                        rhs: value,
                        nba: false,
                    });
                }
                body
            } else {
                vec![IrStmt::Assign {
                    lhs,
                    rhs,
                    nba: false,
                }]
            };
            self.model.processes.push(IrProcess::new_with_origin(
                fn_name,
                format!("{child_path}.link"),
                shape,
                Vec::new(),
                body,
                origin,
            ));
        }
        Ok(())
    }
}
