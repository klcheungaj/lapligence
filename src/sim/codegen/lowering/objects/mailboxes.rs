//! Mailboxes.

use super::*;

impl Codegen<'_> {
    fn mailbox_descriptor_name(&self, node: NodeId) -> Option<&str> {
        self.query_descriptor(node).and_then(|descriptor| {
            descriptor
                .name
                .strip_prefix("mailbox#(")
                .and_then(|name| name.strip_suffix(')'))
        })
    }

    pub(in super::super) fn is_mailbox_expr(&self, _path: &str, node: NodeId) -> bool {
        self.mailbox_descriptor_name(node).is_some()
    }

    pub(in super::super) fn mailbox_element_for_decl(&self, node: NodeId) -> IrMailboxElement {
        let Some(descriptor) = self.query_descriptor(node) else {
            return IrMailboxElement::Untyped;
        };
        // The captured element is `T` resolved in its specializing scope, so a
        // typedef keeps its own shape wherever it is declared; the rendered
        // parameter spelling is neither unique nor scope-qualified.
        match self.db.mailbox_element(descriptor.id) {
            Some(element) => self.mailbox_element_from_descriptor(element),
            None => IrMailboxElement::Untyped,
        }
    }

    fn mailbox_element_from_descriptor(&self, descriptor: &TypeDescriptor) -> IrMailboxElement {
        if Self::is_mailbox_message_type(descriptor) {
            return IrMailboxElement::Value {
                type_id: self.mailbox_message_key(descriptor),
            };
        }
        match &descriptor.shape {
            TypeShape::PackedAtom { .. } => IrMailboxElement::Packed {
                width: descriptor.info.width.unwrap_or_default(),
                signed: descriptor.info.signed,
                two_state: descriptor.two_state,
            },
            TypeShape::Aggregate(layout)
                if matches!(
                    layout.kind,
                    AggregateKind::PackedStruct | AggregateKind::PackedUnion
                ) =>
            {
                IrMailboxElement::Packed {
                    width: descriptor.info.width.unwrap_or_default(),
                    signed: descriptor.info.signed,
                    two_state: descriptor.two_state,
                }
            }
            TypeShape::Real { shortreal } => IrMailboxElement::Real {
                shortreal: *shortreal,
            },
            TypeShape::String => IrMailboxElement::String,
            _ => IrMailboxElement::Handle,
        }
    }

    fn mailbox_constructor_bound(&mut self, path: &str, node: NodeId) -> Result<IrExpr, String> {
        let NodeKind::Expr(ExprKind::NewClass { constructor, .. }) = self.kind(node) else {
            return Err("mailbox constructor edge is malformed".to_owned());
        };
        let args = constructor
            .map(|constructor| self.node(constructor).children.clone())
            .unwrap_or_default();
        if args.len() > 1 {
            return Err(format!("mailbox new takes at most one bound in {path}"));
        }
        if let Some(arg) = args.first() {
            return self.lower_expr(path, *arg);
        }
        let zero = IrConst::packed(vec![0], vec![], vec![], 32, true, None)
            .map_err(|error| error.to_string())?;
        Ok(IrExpr::new(IrExprKind::Const(zero), 32, true, None))
    }

    pub(in super::super) fn lower_mailbox_expr(
        &mut self,
        path: &str,
        node: NodeId,
        element: IrMailboxElement,
    ) -> Result<IrMailboxExpr, String> {
        if matches!(self.kind(node), NodeKind::Expr(ExprKind::NewClass { .. })) {
            return Ok(IrMailboxExpr::New {
                bound: self.mailbox_constructor_bound(path, node)?,
                element,
            });
        }
        if matches!(
            self.kind(node),
            NodeKind::Expr(ExprKind::Constant {
                const_type: ConstantType::Null,
                ..
            })
        ) {
            return Ok(IrMailboxExpr::Null);
        }
        Ok(IrMailboxExpr::Read(self.lower_chandle(path, node)?))
    }

    fn mailbox_target_from_lhs(&self, lhs: &IrLhs) -> Result<IrMailboxTarget, String> {
        match lhs {
            IrLhs::Whole(index) => {
                let signal = self.model.signal(*index);
                let addr = format!("&{}", signal.c_name());
                match signal.ty() {
                    IrType::Real { shortreal } => Ok(IrMailboxTarget::Real { addr, shortreal }),
                    IrType::Packed {
                        width,
                        signed,
                        two_state,
                    } => Ok(IrMailboxTarget::Packed {
                        addr,
                        width,
                        signed,
                        two_state,
                    }),
                }
            }
            IrLhs::WholeRef {
                addr,
                width,
                signed,
                two_state,
                shortreal,
            } => {
                if *width == 0 {
                    Ok(IrMailboxTarget::Real {
                        addr: addr.clone(),
                        shortreal: *shortreal,
                    })
                } else {
                    Ok(IrMailboxTarget::Packed {
                        addr: addr.clone(),
                        width: *width,
                        signed: *signed,
                        two_state: *two_state,
                    })
                }
            }
            IrLhs::Ref {
                addr,
                const_ref,
                bit: None,
                ..
            } => {
                if *const_ref {
                    return Err("mailbox output/ref target cannot be const-ref".to_owned());
                }
                Ok(IrMailboxTarget::Ref { addr: addr.clone() })
            }
            _ => Err("mailbox output/ref target must be a whole scalar lvalue".to_owned()),
        }
    }

    /// Canonical type IDs come from the owned snapshot. Typedefs already
    /// resolve to their canonical type; no source-name search participates.
    fn mailbox_nominal_type(&self, node: NodeId) -> Result<Option<u64>, String> {
        let descriptor = self
            .query_descriptor(node)
            .ok_or_else(|| "mailbox actual has no owned type descriptor".to_owned())?;
        let nominal = self.db.enum_type_metadata(descriptor.id).is_some()
            || matches!(descriptor.shape, TypeShape::Opaque { .. });
        if nominal {
            Ok(Some(descriptor.id.0.checked_add(1).ok_or_else(|| {
                "mailbox nominal type identity overflow".to_owned()
            })?))
        } else {
            Ok(None)
        }
    }

    pub(super) fn lower_mailbox_target(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<IrMailboxTarget, String> {
        let nominal = self.mailbox_nominal_type(node)?;
        let target = self.lower_mailbox_target_storage(path, node)?;
        Ok(match nominal {
            Some(type_id) => IrMailboxTarget::Typed {
                type_id,
                target: Box::new(target),
            },
            None => target,
        })
    }

    pub(super) fn lower_mailbox_value(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<IrMailboxValue, String> {
        let nominal = self.mailbox_nominal_type(node)?;
        let value = self.lower_mailbox_value_storage(path, node)?;
        Ok(match nominal {
            Some(type_id) => IrMailboxValue::Typed {
                type_id,
                value: Box::new(value),
            },
            None => value,
        })
    }

    fn lower_mailbox_target_storage(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<IrMailboxTarget, String> {
        // A mailbox handle message (`mailbox #(mailbox)`) shares its mailbox.
        if self.is_mailbox_expr(path, node) {
            let (target, _) = self.lower_chandle_lvalue(path, node)?;
            return Ok(IrMailboxTarget::Handle {
                addr: self.chandle_target_address(&target),
            });
        }
        if self.is_string_expr(path, node) {
            self.ensure_string_actual_writable(path, node)?;
            return Ok(IrMailboxTarget::String {
                addr: self.lower_string_actual_address(path, node)?,
            });
        }
        if self.is_counted_process_source(path, node) && self.is_process_typed(node) {
            return Err(format!(
                "mailbox try_get or try_peek into a process variable in `{}` is not supported; use get or peek (SIM-015)",
                self.source_path(path)
            ));
        }
        if self.is_chandle_expr(path, node) {
            let (target, _) = self.lower_chandle_lvalue(path, node)?;
            return Ok(IrMailboxTarget::Handle {
                addr: self.chandle_target_address(&target),
            });
        }
        let lhs = self.lower_lhs(path, node)?;
        self.mailbox_target_from_lhs(&lhs)
    }

    fn lower_mailbox_value_storage(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<IrMailboxValue, String> {
        if self.is_string_expr(path, node) {
            return Ok(IrMailboxValue::String(self.lower_string(path, node)?));
        }
        // A process handle travels as a pinned identity (SIM-015).
        if self.is_chandle_expr(path, node)
            || self.is_process_value(path, node)
            || self.is_mailbox_expr(path, node)
        {
            return Ok(IrMailboxValue::Handle(self.lower_chandle(path, node)?));
        }
        let value = self.lower_expr(path, node)?;
        Ok(if value.is_real() {
            let shortreal = self.query_descriptor(node).is_some_and(|descriptor| {
                matches!(descriptor.shape, TypeShape::Real { shortreal: true })
            });
            IrMailboxValue::Real { value, shortreal }
        } else {
            IrMailboxValue::Packed {
                value,
                two_state: self.db.is_two_state_type(node),
            }
        })
    }

    /// The message of `put`/`try_put` and the statements that must run
    /// before the operation: an aggregate is built in a lexical value.
    pub(in super::super) fn lower_mailbox_put_operand(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<(Vec<IrStmt>, IrMailboxValue), String> {
        if self.is_event_operand(node) {
            // An event message is the identity of its synchronization object
            // (SV 15.5.5), like a class handle.
            let event = if self.is_null_event_expression(node) {
                IrChandleExpr::Null
            } else {
                let target = self.event_target_of(node).ok_or_else(|| {
                    format!("mailbox event message in `{path}` must name an event or null")
                })?;
                IrChandleExpr::EventObject(Box::new(self.event_ref_of(&target, path)?))
            };
            let value = IrMailboxValue::Handle(event);
            return Ok((
                Vec::new(),
                match self.mailbox_nominal_type(node)? {
                    Some(type_id) => IrMailboxValue::Typed {
                        type_id,
                        value: Box::new(value),
                    },
                    None => value,
                },
            ));
        }
        if let Some(descriptor) = self.mailbox_message_type(node) {
            let type_id = self.mailbox_message_key(&descriptor);
            let message = self.mailbox_message_source(path, &descriptor, node)?;
            return Ok((
                message.before,
                IrMailboxValue::Native {
                    value: message.value,
                    type_id,
                },
            ));
        }
        Ok((Vec::new(), self.lower_mailbox_value(path, node)?))
    }

    /// The destination of `get`/`peek`/`try_get`/`try_peek` with statements
    /// to run before the operation and, after a successful retrieval, the
    /// copy-out. Whole variables are written directly; an aggregate, or a
    /// packed or real element, member or select, is received into a lexical
    /// value and copied out with its selectors frozen at the call (the
    /// argument is a `ref`, Annex G.4).
    pub(in super::super) fn lower_mailbox_get_operand(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<(Vec<IrStmt>, IrMailboxTarget, Vec<IrStmt>), String> {
        if self.is_event_operand(node) {
            // The event variable is rebound to the received object after
            // the retrieval, as `e = other` does.
            let target = self.event_target_of(node).ok_or_else(|| {
                format!("mailbox event destination in `{path}` must be an event variable")
            })?;
            let target = self.event_ref_of(&target, path)?;
            let local = format!("_llg_mailbox_event_{}", node.index());
            let before = vec![IrStmt::Object(Box::new(IrObjectStmt::ChandleDeclareLocal(
                local.clone(),
                None,
            )))];
            let storage = IrMailboxTarget::Handle {
                addr: format!("&{local}"),
            };
            let storage = match self.mailbox_nominal_type(node)? {
                Some(type_id) => IrMailboxTarget::Typed {
                    type_id,
                    target: Box::new(storage),
                },
                None => storage,
            };
            let after = vec![IrStmt::EventAssign {
                target,
                source: Some(IrEventRef::Handle(Box::new(IrChandleExpr::LocalRead(
                    local,
                )))),
            }];
            return Ok((before, storage, after));
        }
        if let Some(descriptor) = self.mailbox_message_type(node) {
            let type_id = self.mailbox_message_key(&descriptor);
            let message = self.mailbox_message_destination(path, &descriptor, node)?;
            return Ok((
                message.before,
                IrMailboxTarget::Native {
                    value: message.value,
                    type_id,
                },
                message.after,
            ));
        }
        if !self.is_selected_scalar_destination(path, node) {
            return Ok((
                Vec::new(),
                self.lower_mailbox_target(path, node)?,
                Vec::new(),
            ));
        }
        let descriptor = self
            .query_descriptor(node)
            .cloned()
            .ok_or_else(|| "mailbox destination has no owned type descriptor".to_owned())?;
        let tag = format!("mbx{}", node.index());
        let (mut writeback, _, captures) = self.lower_call_writeback(path, node, &tag, false)?;
        let mut before = Vec::new();
        for (name, width, signed, two_state, init) in captures {
            before.push(IrStmt::DeclLocal {
                name,
                width,
                signed,
                two_state,
                init: Some(Box::new(init)),
            });
        }
        before.extend(writeback.take_prelude());
        let local = format!("_llg_mailbox_dest_{}", node.index());
        let addr = format!("&{local}");
        let (width, signed, two_state, target) = match descriptor.shape {
            TypeShape::Real { shortreal } => {
                (0, false, false, IrMailboxTarget::Real { addr, shortreal })
            }
            _ => {
                let width = descriptor
                    .info
                    .width
                    .filter(|width| *width != 0)
                    .ok_or_else(|| {
                        format!("mailbox destination in `{path}` has no packed width")
                    })?;
                let signed = descriptor.info.signed;
                let two_state = self.db.is_two_state_type(node);
                (
                    width,
                    signed,
                    two_state,
                    IrMailboxTarget::Packed {
                        addr,
                        width,
                        signed,
                        two_state,
                    },
                )
            }
        };
        let init = (width == 0).then(|| {
            Box::new(IrExpr::new(
                IrExprKind::Const(IrConst::real(0.0)),
                0,
                false,
                None,
            ))
        });
        before.push(IrStmt::DeclLocal {
            name: local.clone(),
            width,
            signed,
            two_state,
            init,
        });
        let target = match self.mailbox_nominal_type(node)? {
            Some(type_id) => IrMailboxTarget::Typed {
                type_id,
                target: Box::new(target),
            },
            None => target,
        };
        let read = IrExpr::new(IrExprKind::LocalRead(local), width, signed, None);
        let after = vec![writeback.store(&self.model, read)?];
        Ok((before, target, after))
    }

    /// Hoist the statements an expression-form mailbox method needs before
    /// the enclosing statement; only statements that evaluate the call once
    /// accept them.
    pub(in super::super) fn mailbox_expression_prelude(
        &mut self,
        path: &str,
        name: &str,
        statements: Vec<IrStmt>,
    ) -> Result<(), String> {
        if statements.is_empty() {
            return Ok(());
        }
        match self.mailbox_statement_prelude.as_mut() {
            Some(prelude) => {
                prelude.extend(statements);
                Ok(())
            }
            None => Err(format!(
                "mailbox `{name}` with an aggregate or selected operand in `{}` is supported only as a statement, in an assignment, a system-task argument or an if condition (SIM-017)",
                self.source_path(path)
            )),
        }
    }

    /// Append a `try_get`/`try_peek` whose result lands in a status local,
    /// followed by the copy-out when it succeeded (a mismatch or an empty
    /// mailbox leaves the destination untouched). Returns the status read.
    pub(in super::super) fn mailbox_try_get_status(
        &self,
        node: NodeId,
        mailbox: IrChandleExpr,
        target: IrMailboxTarget,
        peek: bool,
        after: Vec<IrStmt>,
        statements: &mut Vec<IrStmt>,
    ) -> IrExpr {
        let status = format!("_llg_mailbox_status_{}", node.index());
        statements.push(IrStmt::DeclLocal {
            name: status.clone(),
            width: 32,
            signed: true,
            two_state: false,
            init: Some(Box::new(object_query(
                IrObjectQuery::MailboxTryGet {
                    mailbox,
                    target,
                    peek,
                },
                32,
                true,
            ))),
        });
        let read = IrExpr::new(IrExprKind::LocalRead(status), 32, true, None);
        if !after.is_empty() {
            let zero = IrExpr::new(
                IrExprKind::Const(
                    IrConst::packed(vec![0], vec![], vec![], 32, true, None)
                        .expect("32-bit zero constant"),
                ),
                32,
                true,
                None,
            );
            statements.push(IrStmt::If {
                cond: IrExpr::new(
                    IrExprKind::Bin {
                        op: IrBinOp::Gt,
                        a: Box::new(read.clone()),
                        b: Box::new(zero),
                    },
                    1,
                    false,
                    None,
                ),
                then_: after,
                els: None,
                check: IrUniquePriorityCheck::None,
            });
        }
        read
    }

    fn is_event_operand(&self, node: NodeId) -> bool {
        self.query_descriptor(node).is_some_and(
            |descriptor| matches!(&descriptor.shape, TypeShape::Opaque { kind } if kind == "Event"),
        )
    }

    /// A packed or real destination that is not a whole variable, ref
    /// formal or other directly addressable cell.
    fn is_selected_scalar_destination(&mut self, path: &str, node: NodeId) -> bool {
        if self.is_mailbox_expr(path, node)
            || self.is_string_expr(path, node)
            || self.is_chandle_expr(path, node)
            || self.is_process_value(path, node)
        {
            return false;
        }
        !matches!(
            self.lower_lhs(path, node),
            Ok(IrLhs::Whole(_) | IrLhs::WholeRef { .. } | IrLhs::Ref { bit: None, .. })
        )
    }

    pub(in super::super) fn lower_mailbox_method(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<IrStmt, String> {
        let (name, receiver) = match self.kind(node) {
            NodeKind::MethodCall {
                name,
                receiver: Some(receiver),
                ..
            } => (name.clone(), *receiver),
            _ => return Err("mailbox method has no receiver".to_owned()),
        };
        if !self.is_mailbox_expr(path, receiver) {
            return Err("mailbox method receiver is not a mailbox".to_owned());
        }
        if matches!(name.as_str(), "put" | "get" | "peek")
            && self.func.as_ref().is_some_and(|function| !function.is_task)
        {
            return Err(format!(
                "blocking mailbox method `{name}` inside a function body in `{path}` is not supported"
            ));
        }
        let mailbox = self.lower_chandle(path, receiver)?;
        let index = self.object_of(path, receiver);
        if let Some(index) = index {
            if self.model.objects[index].ty != IrObjectType::Chandle {
                return Err("mailbox receiver does not have mailbox storage".to_owned());
            }
        }
        // The local spelling is only used to select the local IR variant and
        // to retain the context's storage identity. The emitter renders the
        // complete mailbox expression so formal/alias receivers remain valid.
        let local = index.is_none().then(|| "_llg_mailbox_receiver".to_owned());
        let args = self.node(node).children.get(1..).unwrap_or_default();
        let operation = match (name.as_str(), args) {
            ("num", []) => {
                return Err("mailbox num() is an expression, not a statement".to_owned());
            }
            ("put" | "try_put", [value]) => {
                let (mut before, value) = self.lower_mailbox_put_operand(path, *value)?;
                let put = match (index, name == "put") {
                    (Some(index), true) => IrObjectStmt::MailboxPut(index, mailbox, value, false),
                    (None, true) => IrObjectStmt::MailboxPutLocal(
                        local.expect("local mailbox name"),
                        mailbox,
                        value,
                        false,
                    ),
                    (Some(index), false) => IrObjectStmt::MailboxTryPut(index, mailbox, value),
                    (None, false) => IrObjectStmt::MailboxTryPutLocal(
                        local.expect("local mailbox name"),
                        mailbox,
                        value,
                    ),
                };
                if before.is_empty() {
                    put
                } else {
                    before.push(IrStmt::Object(Box::new(put)));
                    return Ok(IrStmt::Block(before));
                }
            }
            ("get" | "peek", [target]) => {
                // A process variable receives through a handle temporary and
                // a counted assignment once the message is delivered.
                let process_target = (self.is_counted_process_source(path, *target)
                    && self.is_process_typed(*target))
                .then(|| self.lower_process_lvalue(path, *target))
                .transpose()?
                .map(|(target, _)| target);
                let temporary = format!("_llg_mailbox_process_{}", node.0);
                let target = match process_target {
                    Some(_) => {
                        let nominal = self.mailbox_nominal_type(*target)?;
                        let storage = IrMailboxTarget::Handle {
                            addr: format!("&{temporary}"),
                        };
                        match nominal {
                            Some(type_id) => IrMailboxTarget::Typed {
                                type_id,
                                target: Box::new(storage),
                            },
                            None => storage,
                        }
                    }
                    None => {
                        let (before, target, after) =
                            self.lower_mailbox_get_operand(path, *target)?;
                        let peek = name == "peek";
                        let get = match index {
                            Some(index) => IrObjectStmt::MailboxGet(index, mailbox, target, peek),
                            None => IrObjectStmt::MailboxGetLocal(
                                local.expect("local mailbox name"),
                                mailbox,
                                target,
                                peek,
                            ),
                        };
                        if before.is_empty() && after.is_empty() {
                            return Ok(IrStmt::Object(Box::new(get)));
                        }
                        let mut statements = before;
                        statements.push(IrStmt::Object(Box::new(get)));
                        statements.extend(after);
                        return Ok(IrStmt::Block(statements));
                    }
                };
                let peek = name == "peek";
                let get = match index {
                    Some(index) => IrObjectStmt::MailboxGet(index, mailbox, target, peek),
                    None => IrObjectStmt::MailboxGetLocal(
                        local.expect("local mailbox name"),
                        mailbox,
                        target,
                        peek,
                    ),
                };
                if let Some(process_target) = process_target {
                    let value = IrProcessExpr::Handle(Box::new(IrChandleExpr::LocalRead(
                        temporary.clone(),
                    )));
                    let store = match process_target {
                        ProcessTarget::Object(index) => IrObjectStmt::ProcessAssign(index, value),
                        ProcessTarget::Local(name) => IrObjectStmt::ProcessAssignLocal(name, value),
                    };
                    return Ok(IrStmt::Block(vec![
                        IrStmt::Object(Box::new(IrObjectStmt::ChandleDeclareLocal(
                            temporary, None,
                        ))),
                        IrStmt::Object(Box::new(get)),
                        IrStmt::Object(Box::new(store)),
                    ]));
                }
                get
            }
            ("try_get" | "try_peek", [target]) => {
                let (before, target, after) = self.lower_mailbox_get_operand(path, *target)?;
                let peek = name == "try_peek";
                if after.is_empty() && before.is_empty() {
                    match index {
                        Some(index) => IrObjectStmt::MailboxTryGet(index, mailbox, target, peek),
                        None => IrObjectStmt::MailboxTryGetLocal(
                            local.expect("local mailbox name"),
                            mailbox,
                            target,
                            peek,
                        ),
                    }
                } else {
                    let mut statements = before;
                    let _status = self.mailbox_try_get_status(
                        node,
                        mailbox,
                        target,
                        peek,
                        after,
                        &mut statements,
                    );
                    return Ok(IrStmt::Block(statements));
                }
            }
            ("put" | "try_put" | "get" | "peek" | "try_get" | "try_peek", _) => {
                return Err(format!(
                    "mailbox method `{name}` has the wrong argument count in `{path}`"
                ));
            }
            _ => return Err(format!("unsupported mailbox statement method: {name}")),
        };
        Ok(IrStmt::Object(Box::new(operation)))
    }
}
