//! Indexing.

use super::*;

impl<'a> Codegen<'a> {
    /// The index argument of `insert` or `delete`. Like a select index it keeps
    /// its width, signedness and X/Z bits instead of converting to the `int`
    /// formal, so an unknown, negative or beyond-32-bit value is an invalid
    /// index and the call changes nothing (SV 7.10.1).
    fn lower_queue_method_index(&mut self, path: &str, node: NodeId) -> Result<IrExpr, String> {
        self.lower_container_index(path, node)
    }

    pub(in super::super) fn is_unbounded_node(&self, node: NodeId) -> bool {
        match self.kind(node) {
            NodeKind::Expr(ExprKind::Unbounded) => true,
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) => self.is_unbounded_node(*target),
            _ => false,
        }
    }

    pub(super) fn lower_queue_method_index_with_end(
        &mut self,
        path: &str,
        container: usize,
        node: NodeId,
        allow_end: bool,
    ) -> Result<IrExpr, String> {
        if self.is_unbounded_node(node) {
            let size = IrExpr::new(
                IrExprKind::Container(Box::new(IrContainerExpr::Size(container))),
                32,
                true,
                None,
            );
            if allow_end {
                return Ok(size);
            }
            let one = pattern_key_expr(1, 32, true, false);
            return Ok(IrExpr::new(
                IrExprKind::Bin {
                    op: IrBinOp::Sub,
                    a: Box::new(size),
                    b: Box::new(one),
                },
                32,
                true,
                None,
            ));
        }
        self.lower_queue_method_index(path, node)
    }

    /// The index of a queue element select. The value keeps its width,
    /// signedness and X/Z bits so the runtime can reject an unknown, negative
    /// or beyond-32-bit index as invalid (SV 7.10.1); narrowing it to `int`
    /// would read or write another element. `$` inside the expression is the
    /// queue's last index (SV 7.10).
    pub(in super::super) fn lower_queue_index(
        &mut self,
        path: &str,
        container: usize,
        node: NodeId,
    ) -> Result<IrExpr, String> {
        // A selector evaluated earlier (a `$cast` binding) keeps its value.
        if self.hoisted_values.contains_key(&node) {
            return self.lower_container_index(path, node);
        }
        if self.is_unbounded_node(node) {
            return self.lower_queue_method_index_with_end(path, container, node, false);
        }
        let previous = self.queue_end.replace(container);
        let index = self.lower_container_index(path, node);
        self.queue_end = previous;
        index
    }

    fn lower_queue_bound(&mut self, path: &str, node: NodeId) -> Result<IrQueueBound, String> {
        if self.is_unbounded_node(node) {
            return Ok(IrQueueBound::Unbounded);
        }
        Ok(IrQueueBound::Value(self.lower_container_index(path, node)?))
    }

    pub(super) fn lower_queue_sources(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<Vec<IrQueueSource>>, String> {
        let node = self.p30_unwrap_cast(node);
        match self.kind(node) {
            NodeKind::Expr(ExprKind::Operation {
                op: Operation::Concat,
                operands,
                reordered,
                ..
            }) => {
                let mut sources = Vec::new();
                for operand in operands {
                    let Some(mut nested) = self.lower_queue_sources(path, *operand)? else {
                        return Ok(None);
                    };
                    sources.append(&mut nested);
                }
                if *reordered {
                    sources.reverse();
                }
                Ok(Some(sources))
            }
            NodeKind::Expr(ExprKind::PartSelect { base, left, right }) => {
                let Some(source) = self.container_of_select(node, *base) else {
                    return Ok(None);
                };
                if !matches!(
                    self.model.containers[source.ir].kind,
                    IrContainerKind::Queue { .. }
                ) {
                    return Ok(None);
                }
                Ok(Some(vec![IrQueueSource::Slice {
                    container: source.ir,
                    left: self.lower_queue_bound(path, *left)?,
                    right: self.lower_queue_bound(path, *right)?,
                }]))
            }
            _ => Ok(self
                .container_of(node)
                .filter(|source| {
                    matches!(
                        self.model.containers[source.ir].kind,
                        IrContainerKind::Queue { .. }
                    )
                })
                .map(|source| vec![IrQueueSource::Whole(source.ir)])),
        }
    }

    pub(in crate::sim::codegen) fn lower_container_index(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<IrExpr, String> {
        // A class handle key travels as its packed form (`IrAssocKey::Class`).
        // Generated-loop iterators and multidimensional selects stand here
        // as logical indices: their type is the element's, not the index's.
        let logical_index = self.inline_declaration(node).is_some()
            || self.multidimensional_element(node).is_some();
        if !logical_index
            && (self.is_chandle_expr(path, node) || self.class_node_for_type_expr(node).is_some())
        {
            let handle = self.lower_chandle(path, node)?;
            return Ok(class_key_expr(handle));
        }
        // An implicit conversion to the index type would turn X/Z bits into
        // zeros; the runtime validates the original value first and treats
        // an unknown index as invalid (SV 7.4.6, 7.8.6).
        let mut source = node;
        while let NodeKind::Expr(ExprKind::Cast { operand, .. }) = self.kind(source) {
            if !self.db.is_implicit_conversion(source) {
                break;
            }
            source = *operand;
        }
        let value = if source != node {
            match self.lower_expr(path, source) {
                Ok(value) if !value.is_real() && value.width > 0 => value,
                _ => self.lower_expr(path, node)?,
            }
        } else {
            self.lower_expr(path, node)?
        };
        if value.is_real() {
            return Err(format!(
                "resizable container index in `{path}` must be integral"
            ));
        }
        Ok(value)
    }

    pub(super) fn container_key_address(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<(String, Option<usize>, u32, bool, bool), String> {
        let target = match self.kind(node) {
            NodeKind::Expr(ExprKind::Ref { target }) => *target,
            NodeKind::Expr(ExprKind::HierPath { refs, .. }) => refs
                .last()
                .copied()
                .flatten()
                .or_else(|| refs.first().copied().flatten()),
            _ => Some(node),
        };
        if let Some(target) = target {
            if let Some(signal) = self.sig_globals.get(&target) {
                return Ok((
                    format!("&{}", signal.global),
                    Some(signal.ir),
                    signal.width,
                    signal.signed,
                    signal.two_state,
                ));
            }
            if let Some(local) = self.proc_local_info(target) {
                return Ok((
                    format!("&{}", local.c_name),
                    local.static_signal.as_ref().map(|signal| signal.ir),
                    local.width,
                    local.signed,
                    local.two_state,
                ));
            }
            if let Some(function) = &self.func {
                if let Some((name, width, signed, two_state, _shortreal)) =
                    function.locals.get(&target)
                {
                    return Ok((format!("&{name}"), None, *width, *signed, *two_state));
                }
            }
        }
        let source_name = &self.node(node).name;
        if let Some(signal) = self
            .scope_sig_names
            .get(path)
            .and_then(|names| names.get(source_name))
        {
            return Ok((
                format!("&{}", signal.global),
                Some(signal.ir),
                signal.width,
                signal.signed,
                signal.two_state,
            ));
        }
        let mut scopes = self.scope_sig_names.keys().collect::<Vec<_>>();
        scopes.sort_unstable();
        if let Some(signal) = scopes
            .into_iter()
            .find_map(|scope| self.scope_sig_names[scope].get(source_name))
        {
            return Ok((
                format!("&{}", signal.global),
                Some(signal.ir),
                signal.width,
                signal.signed,
                signal.two_state,
            ));
        }
        let lhs = self.lower_lhs(path, node).map_err(|error| {
            format!(
                "{error}; associative traversal key `{source_name}` resolved target is {:?}",
                target.map(|target| self.kind(target))
            )
        })?;
        match lhs {
            IrLhs::Whole(index) => {
                let ty = self.model.signals[index].ty;
                Ok((
                    format!("&{}", self.model.signals[index].c_name),
                    Some(index),
                    ty.width(),
                    ty.signed(),
                    ty.two_state(),
                ))
            }
            IrLhs::WholeRef {
                addr,
                width,
                signed,
                two_state,
                ..
            } => Ok((addr, None, width, signed, two_state)),
            _ => Err(format!(
                "associative traversal key in `{path}` must be a whole packed variable"
            )),
        }
    }

    /// A multidimensional fixed view is addressed whole, by an element
    /// select of every dimension ([`Self::multidimensional_element`]) or by
    /// a row select ([`Self::fixed_view_slice`]); any other select (a
    /// context that lowers only a one-dimensional index) is a located error
    /// rather than an access to the wrong cell.
    pub(in super::super) fn reject_multidimensional_view(
        &self,
        path: &str,
        container: usize,
    ) -> Result<(), String> {
        match self.fixed_view_dims.get(&container) {
            Some(_) if self.model.containers[container].class_field.is_some() => Err(format!(
                "selecting a row or slice of a multidimensional class property in `{path}` is not supported; it is accessed whole, by element or through memory file tasks (SIM-011)"
            )),
            Some(_) => Err(format!(
                "this select of a multidimensional fixed array of native elements in `{path}` is not supported; select one element or one row"
            )),
            None => Ok(()),
        }
    }

    /// The container and logical index of a select of one element: a
    /// one-index select (`a[i]`, `q[i]`, `m["k"]`), or an element select of
    /// every dimension of a multidimensional view, whose logical index is
    /// the select itself (see [`Self::multidimensional_element`]).
    pub(in super::super) fn element_select(&self, node: NodeId) -> Option<(ContainerInfo, NodeId)> {
        if let Some(ir) = self.multidimensional_element(node) {
            return Some((ContainerInfo { ir }, node));
        }
        // `sq[k][i]`: an element of the staged copy of `sq[k]`.
        if let Some((ir, tail)) = self.select_below_staged_element(node) {
            return match tail.as_slice() {
                [index] => Some((ContainerInfo { ir }, *index)),
                _ => None,
            };
        }
        let selected = match self.kind(node) {
            NodeKind::Expr(ExprKind::BitSelect { base, index }) => self
                .container_of_select(node, *base)
                .map(|container| (container, *index)),
            NodeKind::Expr(ExprKind::ArraySelect { base, indices }) if indices.len() == 1 => self
                .container_of_select(node, *base)
                .map(|container| (container, indices[0])),
            _ => None,
        };
        // One index of a multidimensional view selects a row, not an element.
        selected.filter(|(container, _)| !self.fixed_view_dims.contains_key(&container.ir))
    }

    /// The view of `select` when it selects one element of every dimension
    /// of a multidimensional fixed view (`s[i][j]` of `string s[2][3]`).
    /// The select node itself then stands as the element's one logical
    /// index: [`Self::lower_container_top_index`] maps all its indices to
    /// one storage position.
    pub(in super::super) fn multidimensional_element(&self, select: NodeId) -> Option<usize> {
        if self.fixed_view_dims.is_empty() {
            return None;
        }
        let NodeKind::Expr(ExprKind::ArraySelect { base, indices }) = self.kind(select) else {
            return None;
        };
        let container = self.container_of_select(select, *base)?.ir;
        self.fixed_view_dims
            .get(&container)
            .is_some_and(|dimensions| dimensions.len() == indices.len())
            .then_some(container)
    }

    /// The storage position of the first cell selected by `indices` (one
    /// index per leading dimension of `dimensions`) in a row-major view.
    /// An index outside its dimension makes the position -1 and an unknown
    /// index keeps X bits, so the runtime treats either as invalid
    /// (SV 7.4.6). Constant indices fold to a constant position.
    pub(in super::super) fn lower_view_position(
        &mut self,
        path: &str,
        dimensions: &[(i32, i32)],
        indices: &[NodeId],
    ) -> Result<IrExpr, String> {
        let extent = |(left, right): (i32, i32)| (i128::from(left) - i128::from(right)).abs() + 1;
        let strides = dimensions
            .iter()
            .rev()
            .scan(1i128, |stride, dimension| {
                let current = *stride;
                *stride *= extent(*dimension);
                Some(current)
            })
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>();
        let offset = |(left, right): (i32, i32), index: i128| {
            if left <= right {
                index - i128::from(left)
            } else {
                i128::from(left) - index
            }
        };
        let constants = indices
            .iter()
            .map(|index| {
                self.eval_bits(*index)
                    .ok()
                    .filter(|value| !value.is_unknown())
                    .and_then(|value| {
                        if value.signed {
                            value.to_i128()
                        } else {
                            value.to_u128().and_then(|value| value.try_into().ok())
                        }
                    })
            })
            .collect::<Option<Vec<_>>>();
        if let Some(constants) = constants {
            let mut position = 0i128;
            for ((dimension, stride), index) in dimensions.iter().zip(&strides).zip(constants) {
                let offset = offset(*dimension, index);
                if !(0..extent(*dimension)).contains(&offset) {
                    return Ok(pattern_key_expr(-1, 64, true, false));
                }
                position += offset * stride;
            }
            return Ok(pattern_key_expr(position, 64, true, false));
        }
        let mut offsets = Vec::with_capacity(indices.len());
        for (dimension, index) in dimensions.iter().zip(indices) {
            let index = self.lower_container_index(path, *index)?;
            // One extra bit keeps the subtraction exact for every source width.
            let width = index.width.max(32) + 1;
            let index = IrExpr::convert_to(index, width, true);
            let base = pattern_key_expr(i128::from(dimension.0), width, true, false);
            let (a, b) = if dimension.0 <= dimension.1 {
                (index, base)
            } else {
                (base, index)
            };
            offsets.push(IrExpr::new(
                IrExprKind::Bin {
                    op: IrBinOp::Sub,
                    a: Box::new(a),
                    b: Box::new(b),
                },
                width,
                true,
                None,
            ));
        }
        // Every valid offset times its stride fits in 64 bits (the view's
        // cell count does); one more bit keeps the sum signed.
        let width = offsets
            .iter()
            .map(|offset| offset.width)
            .max()
            .unwrap_or(0)
            .max(64)
            + 1;
        let mut valid: Option<IrExpr> = None;
        let mut position: Option<IrExpr> = None;
        for ((offset, dimension), stride) in offsets.into_iter().zip(dimensions).zip(&strides) {
            let offset = IrExpr::convert_to(offset, width, true);
            // A negative offset is a large unsigned value, so one unsigned
            // comparison checks both bounds.
            let in_range = IrExpr::new(
                IrExprKind::Bin {
                    op: IrBinOp::Lt,
                    a: Box::new(IrExpr::convert_to(offset.clone(), width, false)),
                    b: Box::new(pattern_key_expr(extent(*dimension), width, false, false)),
                },
                1,
                false,
                None,
            );
            valid = Some(match valid {
                None => in_range,
                Some(valid) => IrExpr::new(
                    IrExprKind::Bin {
                        op: IrBinOp::LogAnd,
                        a: Box::new(valid),
                        b: Box::new(in_range),
                    },
                    1,
                    false,
                    None,
                ),
            });
            let term = if *stride == 1 {
                offset
            } else {
                IrExpr::new(
                    IrExprKind::Bin {
                        op: IrBinOp::Mul,
                        a: Box::new(offset),
                        b: Box::new(pattern_key_expr(*stride, width, true, false)),
                    },
                    width,
                    true,
                    None,
                )
            };
            position = Some(match position {
                None => term,
                Some(position) => IrExpr::new(
                    IrExprKind::Bin {
                        op: IrBinOp::Add,
                        a: Box::new(position),
                        b: Box::new(term),
                    },
                    width,
                    true,
                    None,
                ),
            });
        }
        let (Some(valid), Some(position)) = (valid, position) else {
            return Ok(pattern_key_expr(0, 64, true, false));
        };
        Ok(IrExpr::new(
            IrExprKind::Mux {
                sel: Box::new(valid),
                a: Box::new(position),
                b: Box::new(pattern_key_expr(-1, width, true, false)),
            },
            width,
            true,
            None,
        ))
    }

    pub(in super::super) fn container_of(&self, node: NodeId) -> Option<ContainerInfo> {
        if let Some(ir) = self.staged_nested_element(node) {
            return Some(ContainerInfo { ir });
        }
        if let Some(ir) = self.call_result_containers.get(&node) {
            return Some(ContainerInfo { ir: *ir });
        }
        match self.kind(node) {
            // A subroutine's implicit result variable names its result.
            NodeKind::Array { .. } => self.container_declaration(node).or_else(|| {
                let parent = self.node(node).parent()?;
                (matches!(self.kind(parent), NodeKind::FuncTask { .. })
                    && self.node(parent).name == self.node(node).name)
                    .then(|| self.container_declaration(parent))?
            }),
            NodeKind::NamedEvent | NodeKind::FuncArg { .. } | NodeKind::FuncTask { .. } => {
                self.container_declaration(node)
            }
            NodeKind::Expr(
                ExprKind::Ref { .. } | ExprKind::HierPath { .. } | ExprKind::MemberSelect { .. },
            ) if self.receiver_containers.contains_key(&node) => Some(ContainerInfo {
                ir: self.receiver_containers[&node],
            }),
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) => self.container_declaration(*target),
            NodeKind::Expr(ExprKind::HierPath { refs, .. }) => refs
                .first()
                .copied()
                .flatten()
                // `this.q`: the enclosing method's property.
                .filter(|target| self.node(*target).name != "this")
                .or_else(|| refs.last().copied().flatten())
                .and_then(|target| self.container_declaration(target))
                .or_else(|| self.static_property_container(refs))
                .or_else(|| self.record_member_container(node)),
            NodeKind::Expr(ExprKind::Operation {
                op: Operation::Assignment,
                operands,
                ..
            }) => operands
                .first()
                .and_then(|operand| self.container_of(*operand)),
            _ => self.record_member_container(node),
        }
    }

    /// A static container class property named through an object
    /// (`obj.items`): the one model-global container the class scope also
    /// names, whatever the receiver (SV 8.8).
    fn static_property_container(&self, refs: &[Option<NodeId>]) -> Option<ContainerInfo> {
        let property = refs.last().copied().flatten()?;
        let class = self.node(property).parent?;
        if !self.class_nodes.contains_key(&class)
            || self.db.variable_lifetime(property) != VariableLifetime::Static
        {
            return None;
        }
        self.container_globals
            .get(&property)
            .filter(|info| self.model.containers[info.ir].class_field.is_none())
            .cloned()
    }

    /// A queue, dynamic or associative member of a record (SIM-007), which
    /// owns its own container storage: a member of a module or static
    /// record, a companion container of a native subroutine value, or the
    /// staged copy of a container record element's member.
    fn record_member_container(&self, node: NodeId) -> Option<ContainerInfo> {
        if let Some(ir) = self.staged_element_member(node) {
            return Some(ContainerInfo { ir });
        }
        if let Ok(Some((value, path))) = self.native_path_of(node) {
            return self
                .native_companion(value, &path)
                .map(|ir| ContainerInfo { ir });
        }
        self.unpacked_member_info(node)
            .and_then(|(_, _, member)| member.container)
    }

    /// A diagnostic for a select whose base is a resizable container with an
    /// associative array nested in its elements (`int aa[string][int]`,
    /// `int qa[$][int]`): element values have no nested associative form, so
    /// such elements cannot be addressed. Selects into a queue or dynamic
    /// array nested in a string-keyed associative array are reported too.
    pub(in super::super) fn nested_associative_select_error(
        &self,
        base: NodeId,
        path: &str,
    ) -> Option<String> {
        let container = self.container_of(base)?;
        let mut element = &self.model.containers[container.ir].element;
        while let IrContainerElement::Container {
            key, element: next, ..
        } = element
        {
            if key.is_some() {
                return Some(format!(
                    "elements of `{}` in `{path}` are not supported here: a statement reaches one associative array nested in container elements, and this one sits below another or outside a procedural statement",
                    self.node(base).name
                ));
            }
            element = next;
        }
        None
    }

    /// The container selected by `select`, whose frontend `base` may be the
    /// record member's declaration (`m.q[i]` selects through field `q`); the
    /// select then carries its owning record and member path.
    /// The receiver alias bound to the path `select` selects from. A
    /// flattened select (`h.m[i][j]`) keeps its earlier indices as a nested
    /// select child, so the path sits below that chain.
    fn select_receiver_container(&self, select: NodeId) -> Option<usize> {
        let mut node = select;
        loop {
            let children = &self.node(node).children;
            if let Some(ir) = children
                .iter()
                .find_map(|child| self.receiver_containers.get(child))
            {
                return Some(*ir);
            }
            let NodeKind::Expr(ExprKind::ArraySelect { indices, .. }) = self.kind(node) else {
                return None;
            };
            node = children.iter().copied().find(|child| {
                !indices.contains(child)
                    && matches!(
                        self.kind(*child),
                        NodeKind::Expr(ExprKind::ArraySelect { .. })
                    )
            })?;
        }
    }

    pub(in super::super) fn container_of_select(
        &self,
        select: NodeId,
        base: NodeId,
    ) -> Option<ContainerInfo> {
        // An element of a call's result: the call is a child of the select.
        if !self.call_result_containers.is_empty() {
            if let Some(ir) = self
                .node(select)
                .children
                .iter()
                .find_map(|child| self.call_result_containers.get(child))
            {
                return Some(ContainerInfo { ir: *ir });
            }
        }
        // An element of a class container property selected through an
        // explicit handle: the select's base names only the declaration,
        // its path expression child names the receiver (SIM-011).
        if !self.receiver_containers.is_empty() {
            if let Some(ir) = self.select_receiver_container(select) {
                return Some(ContainerInfo { ir });
            }
        }
        if let Some(container) = self.container_of(base) {
            return Some(container);
        }
        let (owner, members) = self.db.array_select_path(select)?;
        if let Some(ir) = self.staged_owner_member(owner, members) {
            return Some(ContainerInfo { ir });
        }
        if let Some(value) = self.native_roots.get(&owner) {
            let path = members
                .iter()
                .cloned()
                .map(AggregatePathPart::Member)
                .collect::<Vec<_>>();
            return self
                .native_companion(*value, &path)
                .map(|ir| ContainerInfo { ir });
        }
        // The owner is an element select when records nest (`m.e[1].q[0]`), and
        // each element of `m.e` has its own member container.
        if matches!(
            self.kind(owner),
            NodeKind::Expr(ExprKind::ArraySelect { .. })
        ) {
            let (target, mut path) = self.unpacked_path_for_expr(owner)?;
            path.extend(members.iter().cloned().map(AggregatePathPart::Member));
            return self
                .unpacked_aggregates
                .get(&target)?
                .leaves
                .iter()
                .find(|leaf| leaf.path == path)
                .and_then(|leaf| leaf.container.clone());
        }
        self.unpacked_aggregates
            .get(&owner)?
            .leaves
            .iter()
            .find(|leaf| {
                leaf.path.len() == members.len()
                    && leaf.path.iter().zip(members).all(|(part, member)| {
                        matches!(part, AggregatePathPart::Member(name) if name == member)
                    })
            })
            .and_then(|leaf| leaf.container.clone())
    }

    /// A nested container element (`q[i]` of `int q[$][$]`) used as a method
    /// receiver: its container and integral index path. String-keyed
    /// associative outer levels are excluded.
    pub(in super::super) fn nested_container_receiver(
        &self,
        node: NodeId,
    ) -> Option<(usize, Vec<NodeId>)> {
        let (container, indices) = self.container_element_path(node)?;
        if matches!(
            self.model.containers[container].kind,
            IrContainerKind::Associative {
                key: IrAssocKey::String
            }
        ) {
            return None;
        }
        matches!(
            self.container_element_type(container, indices.len()),
            Some(IrContainerElement::Container { .. })
        )
        .then_some((container, indices))
    }

    /// The name of a container-result function whose call result is selected
    /// directly (`f()[i]`), which needs an expression temporary.
    pub(in super::super) fn container_result_base(&self, node: NodeId) -> Option<String> {
        let target = match self.kind(node) {
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) => *target,
            NodeKind::FuncCall { .. } => {
                let NodeKind::FuncCall { name, callee, .. } = self.kind(node) else {
                    return None;
                };
                self.resolve_callee_env(self.inst, name, false, *callee)
                    .ok()?
                    .0
            }
            _ => node,
        };
        (matches!(self.kind(target), NodeKind::FuncTask { .. }) && self.container_return(target))
            .then(|| self.node(target).name.clone())
    }

    /// Start lowering expression `node`, a call or select: `true` when the
    /// caller must lower it again and close with `leave_call_operands`, which
    /// runs the statements that build its operands where it is evaluated. A
    /// container result used as a method receiver or select base is one.
    pub(in super::super) fn enter_call_operands(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<CallOperands>, String> {
        let operand = match self.kind(node) {
            NodeKind::FuncCall { .. } => None,
            NodeKind::MethodCall { receiver, .. } => *receiver,
            NodeKind::Expr(
                ExprKind::ArraySelect { base, .. } | ExprKind::BitSelect { base, .. },
            ) => {
                // The select's base names the function; its call is a child.
                let call = self.container_result_base(*base).and_then(|_| {
                    self.node(node)
                        .children
                        .iter()
                        .copied()
                        .find(|child| self.select_on_call(*child))
                });
                if call.is_none() {
                    return Ok(None);
                }
                call
            }
            _ => return Ok(None),
        };
        if self.call_operand_scope == Some(node) {
            return Ok(None);
        }
        let saved = CallOperands {
            scope: self.call_operand_scope.replace(node),
            prelude: self.call_operand_prelude.replace(Vec::new()),
            result: None,
        };
        let mut saved = saved;
        if let Some(call) = operand {
            match self.call_result_temporary(call) {
                Ok(Some(temporary)) => {
                    let build = self.lower_container_into(path, call, temporary, call);
                    let build = match build {
                        Ok(build) => build,
                        Err(error) => {
                            self.leave_call_operands(saved);
                            return Err(error);
                        }
                    };
                    if let Some(prelude) = self.call_operand_prelude.as_mut() {
                        prelude.push(IrStmt::Container(Box::new(IrContainerStmt::Declare(
                            temporary,
                        ))));
                        prelude.push(build);
                    }
                    self.call_result_containers.insert(call, temporary);
                    saved.result = Some(call);
                }
                Ok(None) => {}
                Err(error) => {
                    self.leave_call_operands(saved);
                    return Err(error);
                }
            }
        }
        Ok(Some(saved))
    }

    /// Whether select base `base` is a call, not a variable.
    fn select_on_call(&self, base: NodeId) -> bool {
        matches!(
            self.kind(base),
            NodeKind::FuncCall { .. } | NodeKind::MethodCall { .. }
        )
    }

    /// Close `enter_call_operands`, returning the statements to run first.
    pub(in super::super) fn leave_call_operands(&mut self, saved: CallOperands) -> Vec<IrStmt> {
        if let Some(call) = saved.result {
            self.call_result_containers.remove(&call);
        }
        self.call_operand_scope = saved.scope;
        std::mem::replace(&mut self.call_operand_prelude, saved.prelude).unwrap_or_default()
    }

    /// A lexical container for the result of `call`, when it yields one.
    fn call_result_temporary(&mut self, call: NodeId) -> Result<Option<usize>, String> {
        let callee = match self.kind(call) {
            NodeKind::FuncCall { name, callee, .. } => {
                let (name, callee) = (name.clone(), *callee);
                // A built-in, such as `process::self()`, has no container result.
                let Ok((function, _)) = self.resolve_callee_env(self.inst, &name, false, callee)
                else {
                    return Ok(None);
                };
                function
            }
            NodeKind::MethodCall {
                callee: Some(callee),
                ..
            } => *callee,
            _ => return Ok(None),
        };
        if !self.container_return(callee) {
            return Ok(None);
        }
        let result = self.container_formal_type(callee)?;
        Ok(Some(self.container_temporary_like(result)))
    }

    /// The name of a per-object container property selected through an
    /// explicit handle (`h.q`), which only `this`-relative accesses inside
    /// the class's own methods can address.
    pub(in super::super) fn foreign_class_container(&self, node: NodeId) -> Option<String> {
        let NodeKind::Expr(ExprKind::HierPath { refs, .. }) = self.kind(node) else {
            return None;
        };
        let target = refs.last().copied().flatten()?;
        self.container_globals
            .get(&target)
            .filter(|info| self.model.containers[info.ir].class_field.is_some())
            .map(|_| self.node(target).name.clone())
    }

    /// Container storage of a declaration. A function's result storage is
    /// visible only inside the body being lowered; elsewhere the function
    /// name denotes a call.
    pub(in super::super) fn container_declaration(&self, node: NodeId) -> Option<ContainerInfo> {
        if matches!(self.kind(node), NodeKind::FuncTask { .. })
            && self.func.as_ref().and_then(|function| function.ret_node) != Some(node)
        {
            return None;
        }
        self.container_globals.get(&node).cloned()
    }

    pub(in super::super) fn container_element_path(
        &self,
        node: NodeId,
    ) -> Option<(usize, Vec<NodeId>)> {
        // A generated method loop's `item` selects the element at the loop
        // key; the iterator declaration stands for that index.
        if let Some((container, iterator, string_key)) = self.inline_element_path(node) {
            return (!string_key).then(|| (container, vec![iterator]));
        }
        // A staged nested element is a whole container of its own.
        if self.staged_nested_element(node).is_some() {
            return None;
        }
        // `sq[k][i]`: `sq[k]` is a staged copy; the rest selects inside it.
        if let Some((staged, tail)) = self.select_below_staged_element(node) {
            // A string key is no index path (`associative_string_element`).
            return (!matches!(
                self.model.containers[staged].kind,
                IrContainerKind::Associative {
                    key: IrAssocKey::String
                }
            ))
            .then_some((staged, tail));
        }
        let (base, indices) = match self.kind(node) {
            NodeKind::Expr(ExprKind::BitSelect { base, index }) => (*base, vec![*index]),
            NodeKind::Expr(ExprKind::ArraySelect { base, indices }) if !indices.is_empty() => {
                (*base, indices.clone())
            }
            _ => return None,
        };
        // Only a nested-container element accepts another container index.
        // Selects of a scalar element (`d[i][3]`, `a[k][7:0]`) are packed or
        // string selects of that element, not deeper container accesses.
        let nested = |container: usize, depth: usize| {
            matches!(
                self.container_element_type(container, depth),
                Some(IrContainerElement::Container { .. })
            )
        };
        if let Some((container, mut prefix)) = self.container_element_path(base) {
            if !nested(container, prefix.len()) {
                return None;
            }
            prefix.extend(indices);
            return Some((container, prefix));
        }
        if let Some((container, key)) = self.associative_integral_element(base) {
            if !nested(container, 1) {
                return None;
            }
            let mut prefix = vec![key];
            prefix.extend(indices);
            return Some((container, prefix));
        }
        let container = self.container_of_select(node, base)?;
        // One element of a multidimensional view is one logical index: the
        // select itself, which lowers to the cell's storage position.
        if self.multidimensional_element(node) == Some(container.ir) {
            return Some((container.ir, vec![node]));
        }
        if self.fixed_view_dims.contains_key(&container.ir) {
            return None;
        }
        // The frontend flattens `d[i][b]` into one select; indices beyond the
        // container depth select bits of a packed element.
        if indices.len() > self.container_index_depth(container.ir)
            || self.crosses_associative(container.ir, indices.len())
        {
            return None;
        }
        match self.model.containers[container.ir].kind {
            IrContainerKind::Dynamic | IrContainerKind::Queue { .. } => {
                Some((container.ir, indices))
            }
            IrContainerKind::Associative {
                key: IrAssocKey::Integral { .. } | IrAssocKey::Wildcard | IrAssocKey::Class,
            } => Some((container.ir, indices)),
            IrContainerKind::Associative {
                key: IrAssocKey::String,
            } => None,
        }
    }

    fn associative_integral_element(&self, node: NodeId) -> Option<(usize, NodeId)> {
        let (base, key) = match self.kind(node) {
            NodeKind::Expr(ExprKind::BitSelect { base, index }) => (*base, *index),
            NodeKind::Expr(ExprKind::ArraySelect { base, indices }) if indices.len() == 1 => {
                (*base, indices[0])
            }
            _ => return None,
        };
        let container = self.container_of_select(node, base)?;
        matches!(
            self.model.containers[container.ir].kind,
            IrContainerKind::Associative {
                key: IrAssocKey::Integral { .. } | IrAssocKey::Wildcard | IrAssocKey::Class
            }
        )
        .then_some((container.ir, key))
    }

    /// The staged nested element a flattened select sits below and the
    /// indices left inside it: `sq[k][i]` keeps `sq[k]` as a child.
    pub(in crate::sim::codegen) fn select_below_staged_element(
        &self,
        node: NodeId,
    ) -> Option<(usize, Vec<NodeId>)> {
        if self.staged_nested_elements.is_empty() {
            return None;
        }
        let NodeKind::Expr(ExprKind::ArraySelect { indices, .. }) = self.kind(node) else {
            return None;
        };
        let (staged, depth) = self.staged_select_below(node)?;
        let tail = indices.get(depth..)?.to_vec();
        (!self.crosses_associative(staged, tail.len())).then_some((staged, tail))
    }

    /// The nearest staged element in the chain of outer selects a flattened
    /// select keeps as children, and how many indices it covers.
    fn staged_select_below(&self, node: NodeId) -> Option<(usize, usize)> {
        let NodeKind::Expr(ExprKind::ArraySelect { indices, .. }) = self.kind(node) else {
            return None;
        };
        self.node(node)
            .children
            .iter()
            .filter(|child| !indices.contains(child))
            .find_map(|child| {
                let Some(staged) = self.staged_nested_element(*child) else {
                    return self.staged_select_below(*child);
                };
                let depth = match self.kind(*child) {
                    NodeKind::Expr(ExprKind::ArraySelect { indices, .. }) => indices.len(),
                    _ => 1,
                };
                Some((staged, depth))
            })
    }

    /// Whether `depth` indices into `container` select through an associative
    /// array nested in an element, which has no multi-index element path.
    fn crosses_associative(&self, container: usize, depth: usize) -> bool {
        (1..depth).any(|level| {
            matches!(
                self.container_element_type(container, level),
                Some(IrContainerElement::Container { key: Some(_), .. })
            )
        })
    }

    pub(in super::super) fn associative_string_element(
        &self,
        node: NodeId,
    ) -> Option<(usize, NodeId)> {
        if let Some((container, iterator, true)) = self.inline_element_path(node) {
            return Some((container, iterator));
        }
        if self.staged_nested_element(node).is_some() {
            return None;
        }
        if let Some((staged, tail)) = self.select_below_staged_element(node) {
            return match (tail.as_slice(), &self.model.containers[staged].kind) {
                (
                    [key],
                    IrContainerKind::Associative {
                        key: IrAssocKey::String,
                    },
                ) => Some((staged, *key)),
                _ => None,
            };
        }
        let (base, key) = match self.kind(node) {
            NodeKind::Expr(ExprKind::BitSelect { base, index }) => (*base, *index),
            NodeKind::Expr(ExprKind::ArraySelect { base, indices }) if indices.len() == 1 => {
                (*base, indices[0])
            }
            _ => return None,
        };
        let container = self.container_of_select(node, base)?;
        matches!(
            self.model.containers[container.ir].kind,
            IrContainerKind::Associative {
                key: IrAssocKey::String
            }
        )
        .then_some((container.ir, key))
    }

    pub(in super::super) fn container_element_type(
        &self,
        container: usize,
        depth: usize,
    ) -> Option<IrContainerElement> {
        let mut element = self.model.containers[container].element.clone();
        for _ in 1..depth {
            let IrContainerElement::Container { element: next, .. } = element else {
                return None;
            };
            element = *next;
        }
        Some(element)
    }

    pub(in super::super) fn lower_container_path_indices(
        &mut self,
        path: &str,
        container: usize,
        indices: Vec<NodeId>,
    ) -> Result<Vec<IrExpr>, String> {
        indices
            .into_iter()
            .enumerate()
            .map(|(depth, index)| {
                if depth == 0 {
                    self.lower_container_top_index(path, container, index)
                } else {
                    self.lower_container_index(path, index)
                }
            })
            .collect()
    }

    /// Lower the outermost index of `container`: `$` for queues, and the
    /// declared range of a fixed handle-array view mapped to a storage
    /// position. An index outside the range becomes negative or too large
    /// and is rejected by the runtime as invalid (SV 7.4.6); X stays X.
    pub(in super::super) fn lower_container_top_index(
        &mut self,
        path: &str,
        container: usize,
        node: NodeId,
    ) -> Result<IrExpr, String> {
        if let Some(dimensions) = self.fixed_view_dims.get(&container).cloned() {
            if self.multidimensional_element(node) == Some(container) {
                let NodeKind::Expr(ExprKind::ArraySelect { indices, .. }) = self.kind(node) else {
                    unreachable!("a multidimensional element is an array select");
                };
                let indices = indices.clone();
                return self.lower_view_position(path, &dimensions, &indices);
            }
        }
        self.reject_multidimensional_view(path, container)?;
        if matches!(
            self.model.containers[container].kind,
            IrContainerKind::Queue { .. }
        ) {
            return self.lower_queue_index(path, container, node);
        }
        let index = self.lower_container_index(path, node)?;
        let Some((left, right)) = self.fixed_view_ranges.get(&container).copied() else {
            return Ok(index);
        };
        let lowest = left.min(right);
        if left <= right && lowest == 0 {
            return Ok(index);
        }
        // One extra bit keeps the subtraction exact for every source width.
        let width = index.width.max(32) + 1;
        let index = IrExpr::convert_to(index, width, true);
        let base = pattern_key_expr(i128::from(left), width, true, false);
        let (a, b) = if left <= right {
            (index, base)
        } else {
            (base, index)
        };
        Ok(IrExpr::new(
            IrExprKind::Bin {
                op: IrBinOp::Sub,
                a: Box::new(a),
                b: Box::new(b),
            },
            width,
            true,
            None,
        ))
    }
}

impl Codegen<'_> {
    /// A row of a multidimensional fixed view (`s[i]` of `string s[2][3]`):
    /// fewer indices than dimensions select the contiguous run of cells
    /// below them, as its view, first storage position and cell count.
    fn multidimensional_row(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<(usize, IrExpr, u64)>, String> {
        if self.fixed_view_dims.is_empty() {
            return Ok(None);
        }
        let NodeKind::Expr(ExprKind::ArraySelect { base, indices }) = self.kind(node) else {
            return Ok(None);
        };
        let (base, indices) = (*base, indices.clone());
        let Some(container) = self.container_of_select(node, base).map(|info| info.ir) else {
            return Ok(None);
        };
        let Some(dimensions) = self.fixed_view_dims.get(&container).cloned() else {
            return Ok(None);
        };
        if indices.len() >= dimensions.len() {
            return Ok(None);
        }
        self.reject_class_view_select(path, container)?;
        let count = dimensions[indices.len()..]
            .iter()
            .map(|(left, right)| (i64::from(*left) - i64::from(*right)).unsigned_abs() + 1)
            .product();
        let start = self.lower_view_position(path, &dimensions, &indices)?;
        Ok(Some((container, start, count)))
    }

    /// Whether `container` is a fixed-array view (one or more dimensions).
    pub(in super::super) fn is_fixed_view(&self, container: usize) -> bool {
        self.fixed_view_ranges.contains_key(&container)
            || self.fixed_view_dims.contains_key(&container)
    }

    /// Whether `node` is a row of a multidimensional fixed view or a slice
    /// of a fixed view (the selections [`Self::fixed_view_slice`] lowers).
    pub(in super::super) fn is_fixed_view_slice(&self, node: NodeId) -> bool {
        let node = self.p30_unwrap_cast(node);
        match self.kind(node) {
            NodeKind::Expr(ExprKind::ArraySelect { base, indices }) => self
                .container_of_select(node, *base)
                .and_then(|container| self.fixed_view_dims.get(&container.ir))
                .is_some_and(|dimensions| indices.len() < dimensions.len()),
            NodeKind::Expr(
                ExprKind::PartSelect { base, .. } | ExprKind::IndexedPartSelect { base, .. },
            ) => self
                .container_of_select(node, *base)
                .is_some_and(|container| self.is_fixed_view(container.ir)),
            _ => false,
        }
    }

    /// Rows of multidimensional class properties stay rejected (SIM-011):
    /// whole accesses, single elements and memory file tasks reach them.
    fn reject_class_view_select(&self, path: &str, container: usize) -> Result<(), String> {
        if self.model.containers[container].class_field.is_some() {
            return self.reject_multidimensional_view(path, container);
        }
        Ok(())
    }

    /// A slice of a fixed-array view (`a[l:r]`, `a[b+:w]`, `a[b-:w]`): its
    /// container, first storage position (signed, possibly out of range)
    /// and element count. Constant bounds are folded; an indexed base is
    /// evaluated once, keeping X/Z so the runtime treats it as invalid.
    pub(in super::super) fn fixed_view_slice(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<(usize, IrExpr, u64)>, String> {
        let node = self.p30_unwrap_cast(node);
        if let Some(row) = self.multidimensional_row(path, node)? {
            return Ok(Some(row));
        }
        let base = match self.kind(node) {
            NodeKind::Expr(ExprKind::PartSelect { base, .. })
            | NodeKind::Expr(ExprKind::IndexedPartSelect { base, .. }) => *base,
            _ => return Ok(None),
        };
        let Some(container) = self
            .container_of_select(node, base)
            .map(|container| container.ir)
        else {
            return Ok(None);
        };
        self.reject_multidimensional_view(path, container)?;
        let Some((left, right)) = self.fixed_view_ranges.get(&container).copied() else {
            return Ok(None);
        };
        let (left, right) = (i128::from(left), i128::from(right));
        let ascending = left <= right;
        match self.kind(node) {
            NodeKind::Expr(ExprKind::PartSelect {
                left: first,
                right: last,
                ..
            }) => {
                let first = self.eval_bound_i128(*first)?;
                let last = self.eval_bound_i128(*last)?;
                let count = u64::try_from(first.abs_diff(last) + 1)
                    .map_err(|_| format!("fixed-array slice in `{path}` is too large"))?;
                let start = if ascending {
                    first - left
                } else {
                    left - first
                };
                Ok(Some((
                    container,
                    pattern_key_expr(start, 64, true, false),
                    count,
                )))
            }
            NodeKind::Expr(ExprKind::IndexedPartSelect {
                base_expr,
                width_expr,
                neg,
                ..
            }) => {
                let width = self.eval_bound_i128(*width_expr)?;
                let count = u64::try_from(width)
                    .ok()
                    .filter(|count| *count > 0)
                    .ok_or_else(|| {
                        format!("fixed-array slice width in `{path}` is not positive")
                    })?;
                // The lowest declared index of the selection, then its
                // storage position: ascending ranges store the lowest index
                // first, descending ranges the highest.
                let index = self.lower_container_index(path, *base_expr)?;
                let width_bits = index.width.max(32) + 2;
                let index = IrExpr::convert_to(index, width_bits, true);
                let (offset, negate) = match (ascending, neg) {
                    (true, false) => (-left, false),
                    (true, true) => (1 - width - left, false),
                    (false, false) => (left - width + 1, true),
                    (false, true) => (left, true),
                };
                let offset = pattern_key_expr(offset, width_bits, true, false);
                let (op, a, b) = if negate {
                    (IrBinOp::Sub, offset, index)
                } else {
                    (IrBinOp::Add, index, offset)
                };
                Ok(Some((
                    container,
                    IrExpr::new(
                        IrExprKind::Bin {
                            op,
                            a: Box::new(a),
                            b: Box::new(b),
                        },
                        width_bits,
                        true,
                        None,
                    ),
                    count,
                )))
            }
            _ => Ok(None),
        }
    }
}
