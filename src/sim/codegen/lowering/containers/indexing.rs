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
                let Some(source) = self.container_of(*base) else {
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

    pub(super) fn lower_container_index(&mut self, path: &str, node: NodeId) -> Result<IrExpr, String> {
        let value = self.lower_expr(path, node)?;
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
        if let Some(signal) = self
            .scope_sig_names
            .values()
            .find_map(|names| names.get(source_name))
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
            NodeKind::Array { .. } => self.container_globals.get(&node).cloned(),
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) => self.container_globals.get(target).cloned(),
            NodeKind::Expr(ExprKind::HierPath { refs, .. }) => refs
                .first()
                .copied()
                .flatten()
                .or_else(|| refs.last().copied().flatten())
                .and_then(|target| self.container_globals.get(&target).cloned()),
            NodeKind::Expr(ExprKind::Operation {
                op: Operation::Assignment,
                operands,
                ..
            }) => operands
                .first()
                .and_then(|operand| self.container_of(*operand)),
            _ => None,
        }
    }

    pub(super) fn container_element_path(&self, node: NodeId) -> Option<(usize, Vec<NodeId>)> {
        let (base, indices) = match self.kind(node) {
            NodeKind::Expr(ExprKind::BitSelect { base, index }) => (*base, vec![*index]),
            NodeKind::Expr(ExprKind::ArraySelect { base, indices }) if !indices.is_empty() => {
                (*base, indices.clone())
            }
            _ => return None,
        };
        if let Some((container, mut prefix)) = self.container_element_path(base) {
            prefix.extend(indices);
            return Some((container, prefix));
        }
        if let Some((container, key)) = self.associative_integral_element(base) {
            let mut prefix = vec![key];
            prefix.extend(indices);
            return Some((container, prefix));
        }
        let container = self.container_of(base)?;
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
        let container = self.container_of(base)?;
        matches!(
            self.model.containers[container.ir].kind,
            IrContainerKind::Associative {
                key: IrAssocKey::Integral { .. } | IrAssocKey::Wildcard
            }
        )
        .then_some((container.ir, key))
    }

    pub(super) fn associative_string_element(&self, node: NodeId) -> Option<(usize, NodeId)> {
        let (base, key) = match self.kind(node) {
            NodeKind::Expr(ExprKind::BitSelect { base, index }) => (*base, *index),
            NodeKind::Expr(ExprKind::ArraySelect { base, indices }) if indices.len() == 1 => {
                (*base, indices[0])
            }
            _ => return None,
        };
        let container = self.container_of(base)?;
        matches!(
            self.model.containers[container.ir].kind,
            IrContainerKind::Associative {
                key: IrAssocKey::String
            }
        )
        .then_some((container.ir, key))
    }

    pub(super) fn container_element_type(&self, container: usize, depth: usize) -> Option<IrContainerElement> {
        let mut element = self.model.containers[container].element.clone();
        for _ in 1..depth {
            let IrContainerElement::Container { element: next, .. } = element else {
                return None;
            };
            element = *next;
        }
        Some(element)
    }

    pub(super) fn lower_container_path_indices(
        &mut self,
        path: &str,
        container: usize,
        indices: Vec<NodeId>,
    ) -> Result<Vec<IrExpr>, String> {
        indices
            .into_iter()
            .enumerate()
            .map(|(depth, index)| {
                if depth == 0
                    && matches!(
                        self.model.containers[container].kind,
                        IrContainerKind::Queue { .. }
                    )
                {
                    self.lower_queue_index(path, container, index)
                } else {
                    self.lower_container_index(path, index)
                }
            })
            .collect()
    }
}
