//! Callbacks.

use super::*;

impl<'a> Codegen<'a> {
    pub(super) fn container_method_arguments(
        &self,
        path: &str,
        call: NodeId,
        receiver: NodeId,
    ) -> Result<Vec<NodeId>, String> {
        let mut arguments = self.node(call).children.clone();
        let receiver_index = arguments
            .iter()
            .position(|child| *child == receiver)
            .ok_or_else(|| format!("container method in `{path}` has no receiver child"))?;
        arguments.remove(receiver_index);
        Ok(arguments)
    }

    /// Return the structural child carrying a method's `with` expression.
    /// Slang visits that expression before adding the receiver edge, so this
    /// intentionally removes the receiver by identity instead of relying on
    /// child order.
    fn container_method_with_node(
        &self,
        path: &str,
        call: NodeId,
        receiver: NodeId,
    ) -> Result<Option<NodeId>, String> {
        if !self.db.method_call_has_with_clause(call) {
            return Ok(None);
        }
        let mut arguments = self.container_method_arguments(path, call, receiver)?;
        if arguments.len() != 1 {
            return Err(format!(
                "container method in `{path}` has an invalid with-clause argument list"
            ));
        }
        Ok(arguments.pop())
    }

    /// Lower one packed iterator expression into a callback understood by the
    /// C container runtime. The callback uses the frontend-captured iterator
    /// declaration identity, never a source spelling guessed from `item`.
    pub(super) fn lower_container_method_callback(
        &mut self,
        path: &str,
        call: NodeId,
        receiver: NodeId,
        container: usize,
    ) -> Result<Option<(String, u32, bool, bool)>, String> {
        let Some(with_node) = self.container_method_with_node(path, call, receiver)? else {
            return Ok(None);
        };
        if self.event_context(with_node)?.is_some() {
            return Err(format!(
                "container method with-clause in `{path}` cannot capture automatic locals or formals"
            ));
        }
        let Some(iterator) = self.db.method_call_iterator(call) else {
            return Err(format!(
                "container method with-clause in `{path}` has no iterator binding"
            ));
        };
        let Some((source_width, source_signed, _source_two_state)) =
            self.model.containers[container].element.packed()
        else {
            return Err(format!(
                "container method with-clause in `{path}` requires a packed element type"
            ));
        };
        let (index_width, index_signed) = match self.model.containers[container].kind {
            IrContainerKind::Dynamic | IrContainerKind::Queue { .. } => (32, true),
            IrContainerKind::Associative {
                key:
                    IrAssocKey::Integral {
                        width,
                        signed,
                        two_state: _,
                    },
            } => (width, signed),
            IrContainerKind::Associative {
                key: IrAssocKey::String | IrAssocKey::Wildcard,
            } => (0, false),
        };
        let saved_iterator = self.container_iterator.replace(ContainerIterator {
            node: iterator,
            item_width: source_width,
            item_signed: source_signed,
            index_width,
            index_signed,
        });
        let value = self.lower_expr(path, with_node);
        self.container_iterator = saved_iterator;
        let value = value?;
        let result_width = value.width;
        let result_signed = value.signed;
        let result_two_state = self.db.is_two_state_type(with_node);
        if value.is_real() || value.width == 0 {
            return Err(format!(
                "container method with-clause in `{path}` must produce an integral value"
            ));
        }
        let callback = self.new_fn_name(path, "container_eval");
        self.pending_container_pre_fns
            .push(crate::sim::ir::IrPreFn::MonEval {
                c_name: callback.clone(),
                args: vec![value],
                context: None,
                item: true,
            });
        Ok(Some((
            callback,
            result_width,
            result_signed,
            result_two_state,
        )))
    }

    pub(super) fn container_method_result(
        &mut self,
        path: &str,
        dst: usize,
        rhs: NodeId,
    ) -> Result<Option<IrContainerStmt>, String> {
        let rhs = self.p30_unwrap_cast(rhs);
        let (name, receiver) = match self.kind(rhs) {
            NodeKind::MethodCall {
                name,
                receiver: Some(receiver),
                ..
            } => (name.clone(), *receiver),
            _ => return Ok(None),
        };
        let Some(source) = self.container_of(receiver) else {
            return Ok(None);
        };
        let method = match name.as_str() {
            "find" => IrContainerMethod::Find,
            "find_index" => IrContainerMethod::FindIndex,
            "find_first" => IrContainerMethod::FindFirst,
            "find_first_index" => IrContainerMethod::FindFirstIndex,
            "find_last" => IrContainerMethod::FindLast,
            "find_last_index" => IrContainerMethod::FindLastIndex,
            "min" => IrContainerMethod::Min,
            "max" => IrContainerMethod::Max,
            "unique" => IrContainerMethod::Unique,
            "unique_index" => IrContainerMethod::UniqueIndex,
            _ => return Ok(None),
        };
        if !matches!(
            self.model.containers[dst].kind,
            IrContainerKind::Queue { .. }
        ) {
            return Err(format!(
                "array method `{name}` in `{path}` returns a queue and requires a queue destination"
            ));
        }
        if !self.model.containers[source.ir].element.is_packed()
            || !self.model.containers[dst].element.is_packed()
        {
            return Err(format!(
                "array method `{name}` in `{path}` currently requires packed source and destination elements"
            ));
        }
        if matches!(
            self.model.containers[source.ir].kind,
            IrContainerKind::Associative {
                key: IrAssocKey::Wildcard | IrAssocKey::String
            }
        ) && matches!(
            method,
            IrContainerMethod::FindIndex
                | IrContainerMethod::FindFirstIndex
                | IrContainerMethod::FindLastIndex
                | IrContainerMethod::UniqueIndex
        ) {
            return Err(format!(
                "array method `{name}` in `{path}` requires an integral-key associative array for packed index results"
            ));
        }
        let callback = self
            .lower_container_method_callback(path, rhs, receiver, source.ir)?
            .map(|(callback, _, _, _)| callback);
        if matches!(
            method,
            IrContainerMethod::Find
                | IrContainerMethod::FindIndex
                | IrContainerMethod::FindFirst
                | IrContainerMethod::FindFirstIndex
                | IrContainerMethod::FindLast
                | IrContainerMethod::FindLastIndex
        ) && callback.is_none()
        {
            return Err(format!(
                "array locator method `{name}` in `{path}` requires a with clause"
            ));
        }
        Ok(Some(IrContainerStmt::MethodAssign {
            dst,
            src: source.ir,
            method,
            callback,
        }))
    }

    pub(super) fn lower_container_value(
        &mut self,
        path: &str,
        container: usize,
        node: NodeId,
    ) -> Result<IrExpr, String> {
        let element = self.model.containers[container].element.clone();
        self.lower_container_value_for_element(path, &element, node)
    }

    pub(super) fn lower_container_value_for_element(
        &mut self,
        path: &str,
        element: &IrContainerElement,
        node: NodeId,
    ) -> Result<IrExpr, String> {
        if element.is_real() {
            let value = self.lower_expr(path, node)?;
            return if value.is_real() {
                Ok(value)
            } else {
                Ok(IrExpr::new(
                    IrExprKind::CastToReal {
                        a: Box::new(value),
                        shortreal: matches!(element, IrContainerElement::Real { shortreal: true }),
                    },
                    0,
                    true,
                    None,
                ))
            };
        }
        let Some((width, signed, two_state)) = element.packed() else {
            return Err(format!("container element type is not packed in {path}"));
        };
        let value = self.lower_expr(path, node)?;
        let value = apply_assignment_expression_width(value, width);
        ir_to_storage(value, width, signed, two_state)
    }
}
