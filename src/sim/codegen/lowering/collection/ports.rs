//! Ports.

use super::*;

#[cfg(test)]
mod tests;

#[derive(Clone)]
struct FixedArrayPortShape {
    dims: Vec<(i32, i32)>,
    elem_width: u32,
    signed: bool,
    real: bool,
    two_state: bool,
}

#[derive(Clone)]
struct FixedArrayPortActual {
    array: ArrayInfo,
    /// Shape after leading element selections or an unpacked slice.
    dims: Vec<(i32, i32)>,
    /// Leading element selections on a whole-array actual. These remain
    /// expressions so runtime-selected input rows can retain their link
    /// dependencies; output targets validate them as constants below.
    prefix: Vec<NodeId>,
    /// Full coordinates for an unpacked slice. A slice can reverse the
    /// expression order, so it cannot be represented by `prefix` plus the
    /// declaration bounds alone.
    coordinates: Option<Vec<Vec<i32>>>,
}

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
            if !self.net_lvalue_selects_are_constant(actual) {
                return Err(format!(
                    "reference port `{}` requires constant actual selectors; runtime reference rewiring is not supported",
                    self.display_name(port)
                ));
            }
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

        let mut signal_bindings = self.reference_signals.keys().copied().collect::<Vec<_>>();
        signal_bindings.sort_unstable();
        for child in signal_bindings {
            let target = self.reference_lhs(IrLhs::Whole(child))?;
            self.reference_signals.insert(child, target.clone());
            if let IrLhs::Whole(target) = target {
                let canonical = self.model.signals[target].clone();
                self.model.signals[child].alias = Some(target);
                self.model.signals[child].c_name = canonical.c_name;
            }
        }
        let mut array_bindings = self.reference_arrays.keys().copied().collect::<Vec<_>>();
        array_bindings.sort_unstable();
        for child in array_bindings {
            let target = self.reference_array(child);
            self.reference_arrays.insert(child, target);
        }
        let mut object_bindings = self.reference_objects.keys().copied().collect::<Vec<_>>();
        object_bindings.sort_unstable();
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
        // Canonicalization rewrote `ir` in place.
        self.rebuild_sig_global_index();
        Ok(())
    }

    fn bind_reference_aggregate(
        &mut self,
        port: NodeId,
        _path: &str,
        actual: NodeId,
        child: UnpackedAggregateInfo,
    ) -> Result<(), String> {
        if self.unpacked_aggregate_info(actual).is_none()
            && self.unpacked_path_for_expr(actual).is_none()
            && child
                .leaves
                .iter()
                .all(|leaf| leaf.signal.is_some() && leaf.object.is_none())
        {
            let descriptor = self
                .query_descriptor(actual)
                .cloned()
                .ok_or("fixed reference actual has no type")?;
            let target = self
                .fixed_storage_lhs(_path, actual)?
                .ok_or("fixed reference actual has no storage")?;
            if !self.reference_actual_is_variable(actual)
                || !self.reference_lhs_is_variable(&target)
            {
                return Err("fixed reference port requires variable storage".into());
            }
            for leaf in &child.leaves {
                let signal = leaf
                    .signal
                    .as_ref()
                    .ok_or("fixed reference member has no storage")?;
                let (_, offset) =
                    super::fixed_values::fixed_path_descriptor(&descriptor, &leaf.path)
                        .ok_or("fixed reference member has no matching actual path")?;
                let projection = IrLhs::PackedSelect {
                    target: Box::new(target.clone()),
                    steps: vec![crate::sim::ir::IrPackedSelect {
                        base: lhs_integer_expr(i128::from(offset)),
                        width: signal.width,
                    }],
                    signed: signal.signed,
                    two_state: signal.two_state,
                };
                self.reference_signals
                    .insert(signal.ir, self.reference_lhs(projection)?);
            }
            return Ok(());
        }
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
        let mut remap = |index: usize| {
            self.model
                .signals
                .get(index)
                .and_then(|signal| signal.net_driver.map(|(group, _)| group))
                .and_then(|group| self.structural_driver_signal(source, group))
                .unwrap_or(index)
        };
        Self::remap_structural_lhs_with(lhs, &mut remap)
    }

    pub(super) fn structural_group_for_lhs(&self, lhs: &IrLhs) -> Option<usize> {
        let signal_group = |index: usize| {
            self.model
                .signals
                .get(index)
                .and_then(|signal| signal.net_driver.map(|(group, _)| group))
        };
        match lhs {
            IrLhs::PackedSelect { target, .. } | IrLhs::TaggedSelect { target, .. } => {
                self.structural_group_for_lhs(target)
            }
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

    pub(super) fn structural_groups_for_lhs(&self, lhs: &IrLhs) -> Vec<usize> {
        fn append(model: &IrModel, lhs: &IrLhs, groups: &mut Vec<usize>) {
            match lhs {
                IrLhs::PackedSelect { target, .. } | IrLhs::TaggedSelect { target, .. } => {
                    append(model, target, groups);
                }
                IrLhs::Whole(index)
                | IrLhs::Bit(index, ..)
                | IrLhs::Part(index, ..)
                | IrLhs::IdxPart(index, ..) => {
                    if let Some(group) = model.signals[*index].net_driver.map(|(group, _)| group) {
                        if !groups.contains(&group) {
                            groups.push(group);
                        }
                    }
                }
                IrLhs::Stream { parts, .. } => {
                    for (part, _) in parts {
                        append(model, part, groups);
                    }
                }
                IrLhs::WholeRef { .. } | IrLhs::Ref { .. } | IrLhs::ArrayElem { .. } => {}
            }
        }

        let mut groups = Vec::new();
        append(&self.model, lhs, &mut groups);
        groups
    }

    pub(super) fn remap_structural_lhs_for_terminal(
        &self,
        lhs: IrLhs,
        source: NodeId,
        terminal: usize,
    ) -> IrLhs {
        let mut remap = |index: usize| {
            self.model
                .signals
                .get(index)
                .and_then(|signal| signal.net_driver.map(|(group, _)| group))
                .and_then(|group| {
                    self.structural_driver_signal_for_terminal(source, group, terminal)
                })
                .unwrap_or(index)
        };
        Self::remap_structural_lhs_with(lhs, &mut remap)
    }

    pub(super) fn remap_structural_lhs_for_terminals(
        &self,
        lhs: IrLhs,
        source: NodeId,
        terminals: &HashMap<usize, usize>,
    ) -> IrLhs {
        let mut remap = |index: usize| {
            self.model
                .signals
                .get(index)
                .and_then(|signal| signal.net_driver.map(|(group, _)| group))
                .and_then(|group| {
                    terminals.get(&group).and_then(|terminal| {
                        self.structural_driver_signal_for_terminal(source, group, *terminal)
                    })
                })
                .unwrap_or(index)
        };
        Self::remap_structural_lhs_with(lhs, &mut remap)
    }

    fn remap_structural_lhs_with(lhs: IrLhs, remap: &mut dyn FnMut(usize) -> usize) -> IrLhs {
        match lhs {
            IrLhs::PackedSelect {
                target,
                steps,
                signed,
                two_state,
            } => IrLhs::PackedSelect {
                target: Box::new(Self::remap_structural_lhs_with(*target, remap)),
                steps,
                signed,
                two_state,
            },
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
                        (Self::remap_structural_lhs_with(part, remap), part_width)
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

    pub(super) fn unmapped_structural_group_for_terminals(
        &self,
        lhs: &IrLhs,
        source: NodeId,
        terminals: &HashMap<usize, usize>,
    ) -> Option<usize> {
        let mut mapped = |group| {
            terminals.get(&group).and_then(|terminal| {
                self.structural_driver_signal_for_terminal(source, group, *terminal)
            })
        };
        self.unmapped_structural_group_for(lhs, &mut mapped)
    }

    fn unmapped_structural_group_for(
        &self,
        lhs: &IrLhs,
        mapped: &mut dyn FnMut(usize) -> Option<usize>,
    ) -> Option<usize> {
        match lhs {
            IrLhs::PackedSelect { target, .. } | IrLhs::TaggedSelect { target, .. } => {
                self.unmapped_structural_group_for(target, mapped)
            }
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

    /// Resolve a fixed-array value-port actual to the array being connected,
    /// its remaining shape, and any selected coordinates. A whole array (`a`)
    /// has no prefix and its full declared shape; an element select of a
    /// higher-rank array (`a[i]`) contributes the select indices and leaves
    /// only the remaining dimensions as the connected shape. Unpacked part
    /// selects retain expression order in `coordinates`, including a reversed
    /// slice, so output links write the same cells that an input value link
    /// would read.
    fn port_array_prefix(&self, node: NodeId) -> Option<(ArrayInfo, Vec<NodeId>)> {
        if let Some(array) = self.array_of(node) {
            return Some((array.clone(), Vec::new()));
        }
        match self.kind(node) {
            NodeKind::Expr(ExprKind::Cast { operand, .. }) => self.port_array_prefix(*operand),
            NodeKind::Expr(ExprKind::ArraySelect { base, indices }) => {
                let (array, mut prefix) = self.port_array_prefix(*base)?;
                if prefix.len().saturating_add(indices.len()) >= array.dims.len() {
                    return None;
                }
                prefix.extend(indices.iter().copied());
                Some((array, prefix))
            }
            NodeKind::Expr(ExprKind::BitSelect { base, index }) => {
                let (array, mut prefix) = self.port_array_prefix(*base)?;
                if prefix.len().saturating_add(1) >= array.dims.len() {
                    return None;
                }
                prefix.push(*index);
                Some((array, prefix))
            }
            _ => None,
        }
    }

    fn port_array_actual(&self, actual: NodeId) -> Option<FixedArrayPortActual> {
        if let Some(array) = self.array_of(actual) {
            return Some(FixedArrayPortActual {
                array: array.clone(),
                dims: array.dims.clone(),
                prefix: Vec::new(),
                coordinates: None,
            });
        }
        if let Some((array, prefix)) = self.port_array_prefix(actual) {
            let dims = array.dims[prefix.len()..].to_vec();
            return Some(FixedArrayPortActual {
                array,
                dims,
                prefix,
                coordinates: None,
            });
        }
        let NodeKind::Expr(ExprKind::PartSelect { base, left, right }) = self.kind(actual) else {
            return None;
        };
        let (array, prefix) = self.port_array_prefix(*base)?;
        let dimension = prefix.len();
        let (decl_left, decl_right) = *array.dims.get(dimension)?;
        let left = i32::try_from(self.eval_bound_i128(*left).ok()?).ok()?;
        let right = i32::try_from(self.eval_bound_i128(*right).ok()?).ok()?;
        if left < decl_left.min(decl_right)
            || left > decl_left.max(decl_right)
            || right < decl_left.min(decl_right)
            || right > decl_left.max(decl_right)
        {
            return None;
        }
        let step = if left >= right { -1 } else { 1 };
        let suffix = port_array_index_vectors(&array.dims[dimension + 1..]);
        let mut coordinates = Vec::new();
        let mut index = left;
        loop {
            for suffix_indices in &suffix {
                let mut coordinate = prefix
                    .iter()
                    .map(|node| i32::try_from(self.eval_bound_i128(*node).ok()?).ok())
                    .collect::<Option<Vec<_>>>()?;
                coordinate.push(index);
                coordinate.extend(suffix_indices.iter().copied());
                coordinates.push(coordinate);
            }
            if index == right {
                break;
            }
            index = index.checked_add(step)?;
        }
        let mut dims = vec![(left, right)];
        dims.extend_from_slice(&array.dims[dimension + 1..]);
        Some(FixedArrayPortActual {
            array,
            dims,
            prefix: Vec::new(),
            coordinates: Some(coordinates),
        })
    }

    /// Storage an output port drives as an implied continuous assignment,
    /// for the multiple-driver rule (SV 6.5). A constant row or slice of a
    /// dense array drives only its cells; a runtime-selected actual drives
    /// its longest static prefix, the whole array. A constant row or slice of
    /// descriptor storage has no bounded cell set, so it is not registered
    /// rather than reported as a false whole-array conflict.
    pub(super) fn output_port_continuous_writes(
        &self,
        actual: NodeId,
        writes: &HashSet<IrDependency>,
    ) -> Option<HashSet<IrDependency>> {
        let Some(selected) = self.port_array_actual(actual) else {
            return Some(writes.clone());
        };
        let array = &selected.array;
        let constant_prefix = selected
            .prefix
            .iter()
            .map(|index| {
                self.eval_bound_i128(*index)
                    .ok()
                    .and_then(|index| i32::try_from(index).ok())
            })
            .collect::<Option<Vec<_>>>();
        let whole = selected.prefix.is_empty() && selected.coordinates.is_none();
        if whole || constant_prefix.is_none() {
            return Some(HashSet::from([IrDependency::ArrayContents(
                self.reference_array(array.ir),
            )]));
        }
        if self.model.arrays[array.ir].sparse() {
            return None;
        }
        let cells = selected.coordinates.clone().unwrap_or_else(|| {
            let prefix = constant_prefix.unwrap_or_default();
            port_array_index_vectors(&selected.dims)
                .into_iter()
                .map(|suffix| prefix.iter().copied().chain(suffix).collect())
                .collect()
        });
        cells
            .into_iter()
            .map(|cell| {
                let indices = cell
                    .into_iter()
                    .map(|index| lhs_integer_expr(i128::from(index)))
                    .collect::<Vec<_>>();
                Self::array_constant_linear_index(array, &indices).map(|index| {
                    IrDependency::ArrayElement {
                        array: self.reference_array(array.ir),
                        index,
                    }
                })
            })
            .collect()
    }

    /// Cell pairs for an inout port whose formal is a fixed net array. Each
    /// entry pairs `(formal owner, formal cell, width)` with `(actual owner,
    /// actual cell)` in left-to-left order (SV 23.3.3.5); whole arrays,
    /// constant rows and constant slices are accepted. Returns `None` when
    /// the formal is not a net array.
    #[allow(clippy::type_complexity)]
    pub(super) fn net_array_inout_pairs(
        &self,
        port: NodeId,
        actual: NodeId,
        formal: NodeId,
    ) -> Result<Option<Vec<((NodeId, u64, u32), (NodeId, u64))>>, String> {
        let Some(formal_array) = self.array_of(formal).filter(|array| array.is_net) else {
            return Ok(None);
        };
        let error = |reason: &str| {
            format!(
                "inout net-array port `{}` {reason} at {}:{}:{}",
                self.display_name(port),
                self.node(port).file.as_deref().unwrap_or("<unknown>"),
                self.node(port).line,
                self.node(port).col,
            )
        };
        let actual_array = self
            .port_array_actual(actual)
            .ok_or_else(|| error("requires a fixed net-array actual with constant selections"))?;
        if !actual_array.array.is_net {
            return Err(error("cannot connect a variable array (SV 23.3.3.3)"));
        }
        if actual_array.array.elem_width != formal_array.elem_width
            || actual_array.dims.len() != formal_array.dims.len()
            || actual_array
                .dims
                .iter()
                .zip(&formal_array.dims)
                .any(|(actual, formal)| actual.0.abs_diff(actual.1) != formal.0.abs_diff(formal.1))
        {
            return Err(error("has an incompatible actual shape"));
        }
        let actual_indices = if let Some(coordinates) = actual_array.coordinates {
            coordinates
        } else {
            let prefix = actual_array
                .prefix
                .iter()
                .map(|index| {
                    self.eval_bound_i128(*index)
                        .ok()
                        .and_then(|index| i32::try_from(index).ok())
                })
                .collect::<Option<Vec<_>>>()
                .ok_or_else(|| error("requires constant actual selectors"))?;
            port_array_index_vectors(&actual_array.dims)
                .into_iter()
                .map(|suffix| prefix.iter().copied().chain(suffix).collect())
                .collect()
        };
        let owner_of = |ir: usize| {
            sorted_node_ids(&self.array_globals)
                .into_iter()
                .find(|owner| self.array_globals[owner].ir == ir)
        };
        let formal_owner =
            owner_of(formal_array.ir).ok_or_else(|| error("has no formal storage"))?;
        let actual_owner =
            owner_of(actual_array.array.ir).ok_or_else(|| error("has no actual storage"))?;
        let linear = |array: &ArrayInfo, indices: Vec<i32>| {
            let indices = indices
                .into_iter()
                .map(|index| lhs_integer_expr(i128::from(index)))
                .collect::<Vec<_>>();
            Self::array_constant_linear_index(array, &indices)
        };
        let formal_indices = port_array_index_vectors(&formal_array.dims);
        if formal_indices.len() != actual_indices.len() {
            return Err(error("has an incompatible element count"));
        }
        formal_indices
            .into_iter()
            .zip(actual_indices)
            .map(|(formal_index, actual_index)| {
                let formal_cell = linear(formal_array, formal_index)
                    .ok_or_else(|| error("has an invalid formal cell"))?;
                let actual_cell = linear(&actual_array.array, actual_index)
                    .ok_or_else(|| error("has an invalid actual cell"))?;
                Ok((
                    (formal_owner, formal_cell, formal_array.elem_width),
                    (actual_owner, actual_cell),
                ))
            })
            .collect::<Result<Vec<_>, String>>()
            .map(Some)
    }

    /// Type information for a fixed-array value, including expressions that
    /// do not have a collected storage array. Input value ports copy these
    /// expressions element by element, so their source shape must remain
    /// available independently of storage identity.
    fn fixed_array_port_shape(&self, node: NodeId) -> Option<FixedArrayPortShape> {
        let descriptor = self.query_descriptor(node)?;
        let TypeShape::FixedArray {
            dimensions,
            element,
        } = &descriptor.shape
        else {
            return None;
        };
        let (real, elem_width) = match &element.shape {
            TypeShape::Real { .. } => (true, 0),
            _ => (false, Self::fixed_descriptor_width(element)?),
        };
        Some(FixedArrayPortShape {
            dims: dimensions.clone(),
            elem_width,
            signed: element.info.signed,
            real,
            two_state: element.two_state,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn emit_fixed_array_input_port_link(
        &mut self,
        parent_path: &str,
        child_path: &str,
        port: NodeId,
        actual: NodeId,
        internal: NodeId,
        child_array: ArrayInfo,
        actual_shape: FixedArrayPortShape,
    ) -> Result<bool, String> {
        if actual_shape.real || child_array.real {
            // Real fixed arrays retain the existing storage-only path until
            // their value ownership contract is represented in the packed
            // activation helpers.
            return Ok(false);
        }
        if child_array.dims.len() != actual_shape.dims.len()
            || child_array.dims.iter().zip(&actual_shape.dims).any(
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
        if child_array.elem_width != actual_shape.elem_width {
            return Err(format!(
                "fixed array port `{}` has incompatible element types in `{child_path}`",
                self.display_name(port)
            ));
        }

        let mut captures = Vec::new();
        let mut captured_indices = HashMap::new();
        let values = self.p30_lower_source_values(
            parent_path,
            internal,
            actual,
            &child_array.dims,
            &mut captures,
            &mut captured_indices,
        )?;
        let target_indices = port_array_index_vectors(&child_array.dims);
        if values.len() != target_indices.len() {
            return Err(format!(
                "fixed array port `{}` source has {} elements; expected {} in `{child_path}`",
                self.display_name(port),
                values.len(),
                target_indices.len()
            ));
        }

        let gathered_width = u64::from(child_array.elem_width) * values.len() as u64;
        if child_array.is_net && gathered_width <= u64::from(crate::sim::emit_c::LLG_MAX_WIDTH) {
            if let Some(bindings) = self.alias_lvalue_bindings(port, internal)? {
                // A net-array formal resolves each cell; the link is the
                // port's contribution beside the formal's internal drivers.
                // Cells are concatenated in declared order, first cell most
                // significant, as the bindings number them.
                let parts = values
                    .into_iter()
                    .map(|value| {
                        let value = if actual_shape.two_state {
                            IrExpr::to_two_state(value)
                        } else {
                            value
                        };
                        IrExpr::convert_to(value, child_array.elem_width, false)
                    })
                    .collect::<Vec<_>>();
                let width = child_array
                    .elem_width
                    .checked_mul(
                        u32::try_from(parts.len()).map_err(|_| "net-array port is too wide")?,
                    )
                    .ok_or("net-array port is too wide")?;
                let value = IrExpr::new(IrExprKind::Concat { parts }, width, false, None);
                for (driver, rhs) in
                    self.alias_driver_assignments(port, &bindings, &value, |_| 0)?
                {
                    captures.push(IrStmt::Assign {
                        lhs: IrLhs::Whole(driver),
                        rhs,
                        nba: false,
                    });
                }
                let reads = self.collect_read_signals(parent_path, actual)?;
                self.emit_link_process(
                    parent_path,
                    child_path,
                    port,
                    reads,
                    IrStmt::Block(captures),
                );
                return Ok(true);
            }
        }
        let target_array = self.reference_array(child_array.ir);
        let mut assignments = Vec::with_capacity(values.len());
        for (indices, value) in target_indices.into_iter().zip(values) {
            let lhs = IrLhs::ArrayElem {
                arr: target_array,
                indices: indices
                    .into_iter()
                    .map(|index| lhs_integer_expr(i128::from(index)))
                    .collect(),
                elem_sel: IrElemSel::Whole,
            };
            let value = if actual_shape.two_state {
                IrExpr::to_two_state(value)
            } else {
                value
            };
            let value = if value.width == actual_shape.elem_width {
                IrExpr::new(value.kind, value.width, actual_shape.signed, value.fill)
            } else {
                value
            };
            assignments.push(IrStmt::Assign {
                lhs: lhs.clone(),
                rhs: apply_lhs_assignment_context(&self.model, &lhs, value),
                nba: false,
            });
        }
        captures.extend(assignments);
        let reads = self.collect_read_signals(parent_path, actual)?;
        self.emit_link_process(
            parent_path,
            child_path,
            port,
            reads,
            IrStmt::Block(captures),
        );
        Ok(true)
    }

    /// Link a fixed-array port as one implied continuous assignment
    /// (SV 23.3.3.2) through the procedural fixed-array assignment lowering.
    /// Descriptor-backed ports must not expand per cell, since that emits
    /// code proportional to their logical extent: whole copies, selected
    /// rows, converting casts, conditionals and calls use
    /// `FixedArrayCopy`/`FixedValueAssign`, which snapshot the source before
    /// publishing destination cells. Dense outputs into nested aggregate
    /// members reuse the same assignment owner.
    #[allow(clippy::too_many_arguments)]
    fn emit_descriptor_port_link(
        &mut self,
        parent_path: &str,
        child_path: &str,
        port: NodeId,
        direction: DbDirection,
        actual: NodeId,
        internal: NodeId,
        child: usize,
    ) -> Result<(), String> {
        let (target, source) = if direction == DbDirection::Input {
            (internal, actual)
        } else {
            (actual, internal)
        };
        let statement = self
            .lower_p30_fixed_array_assignment(
                parent_path,
                target,
                source,
                true,
                Operation::Assignment,
            )?
            .ok_or_else(|| {
                format!(
                    "fixed array port `{}` in `{child_path}` requires a fixed-array actual of the same shape",
                    self.display_name(port)
                )
            })?;
        let reads = if direction == DbDirection::Input {
            self.collect_read_signals(parent_path, actual)?
        } else {
            let child_dependency = IrDependency::ArrayContents(self.reference_array(child));
            let mut reads = vec![child_dependency.clone()];
            let mut seen = HashSet::from([child_dependency]);
            self.walk_lhs_select_reads(
                parent_path,
                actual,
                &mut seen,
                &mut HashSet::new(),
                &mut reads,
            )?;
            reads
        };
        self.emit_link_process(parent_path, child_path, port, reads, statement);
        Ok(())
    }

    /// A fixed output may target a nested aggregate member (for example
    /// `s.rows[1].row`) whose storage is a set of aggregate leaves rather
    /// than a collected array. The child cells are read as one packed value
    /// in declaration order and published through the member lvalue, which
    /// maps the leftmost child element to the leftmost member element.
    fn emit_member_array_output_link(
        &mut self,
        parent_path: &str,
        child_path: &str,
        port: NodeId,
        actual: NodeId,
        child: &ArrayInfo,
    ) -> Result<(), String> {
        let lhs = match self.fixed_storage_lhs(parent_path, actual)? {
            Some(lhs) => lhs,
            None => self.lower_lhs(parent_path, actual)?,
        };
        let child_array = self.reference_array(child.ir);
        let parts = port_array_index_vectors(&child.dims)
            .into_iter()
            .map(|indices| {
                IrExpr::new(
                    IrExprKind::ArrayRead {
                        arr: child_array,
                        indices: indices
                            .into_iter()
                            .map(|index| lhs_integer_expr(i128::from(index)))
                            .collect(),
                        elem_sel: IrElemSel::Whole,
                    },
                    child.elem_width,
                    false,
                    None,
                )
            })
            .collect::<Vec<_>>();
        let width = u32::try_from(parts.len())
            .ok()
            .and_then(|count| count.checked_mul(child.elem_width))
            .filter(|width| *width <= LLG_MAX_WIDTH)
            .ok_or_else(|| {
                format!(
                    "fixed array port `{}` exceeds the packed member link capacity in `{child_path}`",
                    self.display_name(port)
                )
            })?;
        let target_width = match &lhs {
            IrLhs::Stream { width, .. } => Some(*width),
            other => packed_lhs_width(&self.model, other),
        };
        if target_width != Some(width) {
            return Err(format!(
                "fixed array port `{}` has incompatible member destination width in `{child_path}`",
                self.display_name(port)
            ));
        }
        let rhs = IrExpr::new(IrExprKind::Concat { parts }, width, false, None);
        let child_dependency = IrDependency::ArrayContents(child_array);
        let mut reads = vec![child_dependency.clone()];
        let mut seen = HashSet::from([child_dependency]);
        self.walk_lhs_select_reads(
            parent_path,
            actual,
            &mut seen,
            &mut HashSet::new(),
            &mut reads,
        )?;
        let statement = IrStmt::Assign {
            rhs: apply_lhs_assignment_context(&self.model, &lhs, rhs),
            lhs,
            nba: false,
        };
        self.emit_link_process(parent_path, child_path, port, reads, statement);
        Ok(())
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
        if let Some(child) = child_array
            .as_ref()
            .filter(|child| self.model.arrays[child.ir].sparse())
        {
            let child = child.ir;
            self.emit_descriptor_port_link(
                parent_path,
                child_path,
                port,
                direction,
                actual,
                internal,
                child,
            )?;
            return Ok(true);
        }
        if direction == DbDirection::Input {
            if let (Some(child_array), Some(actual_shape)) =
                (child_array.clone(), self.fixed_array_port_shape(actual))
            {
                if self.emit_fixed_array_input_port_link(
                    parent_path,
                    child_path,
                    port,
                    actual,
                    internal,
                    child_array,
                    actual_shape,
                )? {
                    return Ok(true);
                }
            }
        }
        let actual_resolved = self.port_array_actual(actual);
        let (child_array, actual_array) = match (child_array, actual_resolved) {
            (None, None) => return Ok(false),
            (Some(child), None) if direction == DbDirection::Output && !child.real => {
                self.emit_member_array_output_link(parent_path, child_path, port, actual, &child)?;
                return Ok(true);
            }
            (Some(_), None) | (None, Some(_)) => {
                return Err(format!(
                    "port `{}` connects a fixed array to a non-array actual in `{child_path}`",
                    self.display_name(port)
                ));
            }
            (Some(child), Some(actual)) => (child, actual),
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
            (&actual_array.array, &child_array)
        } else {
            (&child_array, &actual_array.array)
        };
        let target_array = self.reference_array(target.ir);
        let source_array = self.reference_array(source.ir);
        // An element actual (`a[i]`) selects leading dimensions of a
        // higher-rank array; those element indices must be elaboration
        // constants so the connected sub-array is fixed at build time.
        let actual_prefix = actual_array
            .prefix
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
        let target_dims = if actual_is_target {
            &actual_array.dims
        } else {
            &child_array.dims
        };
        let source_dims = if actual_is_target {
            &child_array.dims
        } else {
            &actual_array.dims
        };
        let target_indices = port_array_index_vectors(target_dims);
        let source_indices = port_array_index_vectors(source_dims);
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
        let actual_coordinates = actual_array.coordinates.as_ref().map(|coordinates| {
            coordinates
                .iter()
                .map(|indices| {
                    indices
                        .iter()
                        .map(|index| lhs_integer_expr(i128::from(*index)))
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>()
        });
        let mut assignments = Vec::with_capacity(target_indices.len());
        for (ordinal, (target_indices, source_indices)) in
            target_indices.iter().zip(&source_indices).enumerate()
        {
            let actual_indices = actual_coordinates
                .as_ref()
                .and_then(|coordinates| coordinates.get(ordinal))
                .cloned();
            let (target_index_exprs, source_index_exprs) = if actual_is_target {
                (
                    actual_indices.unwrap_or_else(|| with_prefix(&actual_prefix, target_indices)),
                    with_prefix(&[], source_indices),
                )
            } else {
                (
                    with_prefix(&[], target_indices),
                    actual_indices.unwrap_or_else(|| with_prefix(&actual_prefix, source_indices)),
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
        if self.fixed_value_width(internal).is_some()
            && self.query_descriptor(internal).is_some_and(|descriptor| {
                matches!(&descriptor.shape, TypeShape::Aggregate(layout) if matches!(layout.kind,
                    AggregateKind::UnpackedStruct | AggregateKind::UnpackedUnion))
            })
        {
            let (target_node, source_node, target_path, source_path) =
                if direction == DbDirection::Input {
                    (internal, actual, child_path, parent_path)
                } else {
                    (actual, internal, parent_path, child_path)
                };
            let target = self
                .fixed_storage_lhs(target_path, target_node)?
                .ok_or("fixed port has no destination storage")?;
            let target = self.remap_structural_lhs(target, port);
            let rhs = self.lower_expr(source_path, source_node)?;
            let reads = self.collect_read_signals(source_path, source_node)?;
            self.emit_link_process(
                parent_path,
                child_path,
                port,
                reads,
                IrStmt::Assign {
                    lhs: target,
                    rhs,
                    nba: false,
                },
            );
            return Ok(true);
        }
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

    /// Input links are emitted before procedural bodies, but a hierarchical
    /// port actual may name a static declaration inside a named process block.
    /// Allocate that persistent storage before resolving the actual so the
    /// link and its sensitivity set share the process-local declaration.
    fn collect_static_hierarchical_port_actual(&mut self, actual: NodeId) -> Result<(), String> {
        let target = match self.kind(actual) {
            NodeKind::Expr(ExprKind::HierPath { refs, .. }) => refs.last().copied().flatten(),
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) if self.db.semantic_detail(actual) == Some("HierarchicalValue") => Some(*target),
            _ => None,
        };
        let Some(target) = target else {
            return Ok(());
        };
        // Interface members also have static lifetime, but collect_design
        // already assigned them owned signal storage. Only process-local
        // static declarations need a late allocation here.
        if !matches!(self.kind(target), NodeKind::Var { .. })
            || self.db.variable_lifetime(target) != VariableLifetime::Static
            || self.signal_of(target).is_some()
        {
            return Ok(());
        }
        let Some(instance) = self.owning_inst(target) else {
            return Ok(());
        };
        if self.proc_local_instances.contains_key(&(instance, target)) {
            return Ok(());
        }

        let path = self.instance_path_of(instance);
        let previous_instance = self.inst;
        self.inst = instance;
        let result = self.collect_loop_var(&path, target).map(|_| ());
        self.inst = previous_instance;
        result
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
                if direction == DbDirection::Input {
                    match self.db.unconnected_drive(child_inst) {
                        UnconnectedDrive::None => {}
                        drive => {
                            let Some(internal) = low else {
                                continue;
                            };
                            self.emit_unconnected_drive(
                                parent_path,
                                &child_path,
                                port,
                                internal,
                                drive,
                            )?;
                        }
                    }
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
                self.collect_static_hierarchical_port_actual(actual)?;
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

impl Codegen<'_> {
    /// An omitted input under `` `unconnected_drive`` (IEEE 1364-2001 19.9)
    /// is pulled like a `pullup`/`pulldown` on the formal: a net formal gets
    /// a pull-strength contribution in its own driver slot, so it competes
    /// with internal drivers and the formal's net type by the strength
    /// tables. A variable formal has no strength and receives the value.
    fn emit_unconnected_drive(
        &mut self,
        parent_path: &str,
        child_path: &str,
        port: NodeId,
        internal: NodeId,
        drive: UnconnectedDrive,
    ) -> Result<(), String> {
        if let Some(array) = self.array_of(internal).cloned() {
            if array.real {
                return Ok(());
            }
            let gathered_width = u64::from(array.elem_width) * self.model.arrays[array.ir].total;
            if array.is_net && gathered_width <= u64::from(crate::sim::emit_c::LLG_MAX_WIDTH) {
                if let Some(bindings) = self.alias_lvalue_bindings(port, internal)? {
                    let width = bindings.len();
                    let value = unconnected_drive_expr(
                        drive,
                        u32::try_from(width).map_err(|_| "net-array port is too wide")?,
                    )?;
                    let body = self
                        .alias_driver_assignments(port, &bindings, &value, |_| 0)?
                        .into_iter()
                        .map(|(driver, rhs)| IrStmt::Assign {
                            lhs: IrLhs::Whole(driver),
                            rhs,
                            nba: false,
                        })
                        .collect();
                    return self.push_unconnected_process(parent_path, child_path, port, body);
                }
            }
            let value = unconnected_drive_expr(drive, array.elem_width)?;
            let target = self.reference_array(array.ir);
            let body = if self.model.arrays[array.ir].sparse() {
                vec![IrStmt::FixedArrayFill {
                    array: target,
                    value: IrExpr::convert_to(value, array.elem_width, array.signed),
                    nba: false,
                }]
            } else {
                // Net arrays are dense: each element write publishes through
                // the element's own resolved cell, as a connected link does.
                port_array_index_vectors(&array.dims)
                    .iter()
                    .map(|indices| {
                        let lhs = IrLhs::ArrayElem {
                            arr: target,
                            indices: indices
                                .iter()
                                .map(|index| lhs_integer_expr(i128::from(*index)))
                                .collect(),
                            elem_sel: IrElemSel::Whole,
                        };
                        IrStmt::Assign {
                            rhs: apply_lhs_assignment_context(&self.model, &lhs, value.clone()),
                            lhs,
                            nba: false,
                        }
                    })
                    .collect()
            };
            return self.push_unconnected_process(parent_path, child_path, port, body);
        }
        let (_, child_info) = self.resolve_signal_id(child_path, internal)?;
        let value = unconnected_drive_expr(drive, child_info.width)?;
        let body = if let Some(bindings) = self.alias_lvalue_bindings(port, internal)? {
            self.alias_driver_assignments(port, &bindings, &value, |_| 0)?
                .into_iter()
                .map(|(driver, rhs)| IrStmt::Assign {
                    lhs: IrLhs::Whole(driver),
                    rhs,
                    nba: false,
                })
                .collect()
        } else {
            let lhs = self.remap_structural_lhs(IrLhs::Whole(child_info.ir), port);
            if let Some(group) = self.unmapped_structural_group(&lhs, port) {
                return Err(format!(
                    "unconnected input port `{}` has no structural driver mapping for resolved net group {group}",
                    self.display_name(port),
                ));
            }
            let rhs = apply_lhs_assignment_context(&self.model, &lhs, value);
            vec![IrStmt::Assign {
                lhs,
                rhs,
                nba: false,
            }]
        };
        self.push_unconnected_process(parent_path, child_path, port, body)
    }

    fn push_unconnected_process(
        &mut self,
        parent_path: &str,
        child_path: &str,
        port: NodeId,
        body: Vec<IrStmt>,
    ) -> Result<(), String> {
        let fn_name = self.new_fn_name(parent_path, "unconnected");
        let origin = self.origin(port);
        self.model.processes.push(IrProcess::new_with_origin(
            fn_name,
            format!("{child_path}.unconnected"),
            IrShape::RunOnce,
            Vec::new(),
            body,
            origin,
        ));
        Ok(())
    }
}

fn unconnected_drive_expr(drive: UnconnectedDrive, width: u32) -> Result<IrExpr, String> {
    if width == 0 {
        return Err("unconnected_drive requires packed input storage".to_owned());
    }
    let limbs = (width as usize).div_ceil(64);
    let mut bits = if drive == UnconnectedDrive::Pull1 {
        vec![u64::MAX; limbs]
    } else {
        vec![0; limbs]
    };
    if let Some(last) = bits.last_mut() {
        let tail = width % 64;
        if tail != 0 {
            *last &= (1_u64 << tail) - 1;
        }
    }
    let constant = IrConst::packed(bits, vec![], vec![], width, false, None)
        .map_err(|error| error.to_string())?;
    Ok(IrExpr::new(IrExprKind::Const(constant), width, false, None))
}
