//! Indexing.

use super::*;

impl<'a> Codegen<'a> {
    fn lower_queue_method_index(&mut self, path: &str, node: NodeId) -> Result<IrExpr, String> {
        let value = self.lower_expr(path, node)?;
        ir_to_storage(value, 32, true, true)
    }

    fn is_unbounded_node(&self, node: NodeId) -> bool {
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

    pub(in super::super) fn lower_queue_index(
        &mut self,
        path: &str,
        container: usize,
        node: NodeId,
    ) -> Result<IrExpr, String> {
        self.lower_queue_method_index_with_end(path, container, node, false)
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

    pub(in super::super) fn container_of(&self, node: NodeId) -> Option<ContainerInfo> {
        match self.kind(node) {
            NodeKind::Array { .. }
            | NodeKind::NamedEvent
            | NodeKind::FuncArg { .. }
            | NodeKind::FuncTask { .. } => self.container_declaration(node),
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) => self.container_declaration(*target),
            NodeKind::Expr(ExprKind::HierPath { .. })
                if self.receiver_containers.contains_key(&node) =>
            {
                Some(ContainerInfo {
                    ir: self.receiver_containers[&node],
                })
            }
            NodeKind::Expr(ExprKind::HierPath { refs, .. }) => refs
                .first()
                .copied()
                .flatten()
                .or_else(|| refs.last().copied().flatten())
                .and_then(|target| self.container_declaration(target))
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

    /// The container selected by `select`, whose frontend `base` may be the
    /// record member's declaration (`m.q[i]` selects through field `q`); the
    /// select then carries its owning record and member path.
    pub(in super::super) fn container_of_select(
        &self,
        select: NodeId,
        base: NodeId,
    ) -> Option<ContainerInfo> {
        // An element of a class container property selected through an
        // explicit handle: the select's base names only the declaration,
        // its path expression child names the receiver (SIM-011).
        if !self.receiver_containers.is_empty() {
            if let Some(ir) = self
                .node(select)
                .children
                .iter()
                .find_map(|child| self.receiver_containers.get(child))
            {
                return Some(ContainerInfo { ir: *ir });
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
    fn container_declaration(&self, node: NodeId) -> Option<ContainerInfo> {
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
        // The frontend flattens `d[i][b]` into one select; indices beyond the
        // container depth select bits of a packed element.
        if indices.len() > self.container_index_depth(container.ir) {
            return None;
        }
        match self.model.containers[container.ir].kind {
            IrContainerKind::Dynamic | IrContainerKind::Queue { .. } => {
                Some((container.ir, indices))
            }
            IrContainerKind::Associative {
                key: IrAssocKey::Integral { .. } | IrAssocKey::Wildcard,
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
                key: IrAssocKey::Integral { .. } | IrAssocKey::Wildcard
            }
        )
        .then_some((container.ir, key))
    }

    pub(in super::super) fn associative_string_element(
        &self,
        node: NodeId,
    ) -> Option<(usize, NodeId)> {
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
