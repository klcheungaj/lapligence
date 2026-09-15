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
        let Some(name) = self.mailbox_descriptor_name(node) else {
            return IrMailboxElement::Untyped;
        };
        if let Some(element) = Self::mailbox_element_from_name(name) {
            return element;
        }
        // Slang keeps a typedef/enum mailbox parameter in the rendered
        // mailbox name but does not attach the parameter type descriptor to
        // the mailbox class descriptor. Resolve the owned declaration by its
        // spelling so aliases retain their packed width/state metadata.
        let short_name = name.rsplit("::").next().unwrap_or(name);
        self.db
            .node_ids()
            .find(|candidate| {
                self.node(*candidate).name == short_name
                    && self.query_descriptor(*candidate).is_some_and(|descriptor| {
                        !matches!(&descriptor.shape, TypeShape::Opaque { kind } if kind == "Class")
                    })
            })
            .and_then(|candidate| {
                self.query_descriptor(candidate)
                    .map(|descriptor| self.mailbox_element_from_descriptor(candidate, descriptor))
            })
            .unwrap_or(IrMailboxElement::Handle)
    }

    fn mailbox_element_from_name(name: &str) -> Option<IrMailboxElement> {
        let name = name.trim();
        if name.eq_ignore_ascii_case("untyped") {
            return Some(IrMailboxElement::Untyped);
        }
        let lower = name.to_ascii_lowercase();
        if lower == "string" {
            return Some(IrMailboxElement::String);
        }
        if lower == "real" || lower == "shortreal" {
            return Some(IrMailboxElement::Real {
                shortreal: lower == "shortreal",
            });
        }
        let keyword = |word: &str| {
            lower == word
                || lower.starts_with(&format!("{word} "))
                || lower.starts_with(&format!("{word}["))
        };
        let two_state = keyword("bit")
            || keyword("int")
            || keyword("longint")
            || keyword("shortint")
            || keyword("byte");
        let signed = keyword("int")
            || keyword("integer")
            || keyword("longint")
            || keyword("shortint")
            || keyword("byte")
            || lower.contains(" signed");
        let scalar_width = if keyword("longint") || keyword("time") {
            64
        } else if keyword("shortint") {
            16
        } else if keyword("byte") {
            8
        } else if keyword("int") || keyword("integer") {
            32
        } else if keyword("bit") || keyword("logic") || keyword("reg") {
            1
        } else {
            0
        };
        let width = lower
            .find('[')
            .and_then(|start| {
                lower[start + 1..]
                    .find(']')
                    .map(|end| (start + 1, start + 1 + end))
            })
            .and_then(|(start, end)| lower[start..end].split_once(':'))
            .and_then(|(left, right)| {
                let left = left.trim().parse::<i64>().ok()?;
                let right = right.trim().parse::<i64>().ok()?;
                left.checked_sub(right)
                    .and_then(|delta| delta.unsigned_abs().checked_add(1))
                    .and_then(|width| u32::try_from(width).ok())
            })
            .unwrap_or(scalar_width);
        if width != 0 {
            Some(IrMailboxElement::Packed {
                width,
                signed,
                two_state,
            })
        } else {
            None
        }
    }

    fn mailbox_element_from_descriptor(
        &self,
        node: NodeId,
        descriptor: &TypeDescriptor,
    ) -> IrMailboxElement {
        match &descriptor.shape {
            TypeShape::PackedAtom { .. } => IrMailboxElement::Packed {
                width: descriptor.info.width.unwrap_or_default(),
                signed: descriptor.info.signed,
                two_state: self.db.is_two_state_type(node)
                    || is_two_state_kind(&descriptor.info.kind),
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
                    two_state: self.db.is_two_state_type(node)
                        || is_two_state_kind(&descriptor.info.kind),
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
        if self.is_mailbox_expr(path, node) {
            return Err(format!(
                "mailbox handle is not a writable message target in {path}"
            ));
        }
        if self.is_string_expr(path, node) {
            self.ensure_string_actual_writable(path, node)?;
            return Ok(IrMailboxTarget::String {
                addr: self.lower_string_actual_address(path, node)?,
            });
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
        if self.is_chandle_expr(path, node) {
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
            ("put", [value]) => match index {
                Some(index) => IrObjectStmt::MailboxPut(
                    index,
                    mailbox,
                    self.lower_mailbox_value(path, *value)?,
                    false,
                ),
                None => IrObjectStmt::MailboxPutLocal(
                    local.expect("local mailbox name"),
                    mailbox,
                    self.lower_mailbox_value(path, *value)?,
                    false,
                ),
            },
            ("try_put", [value]) => match index {
                Some(index) => IrObjectStmt::MailboxTryPut(
                    index,
                    mailbox,
                    self.lower_mailbox_value(path, *value)?,
                ),
                None => IrObjectStmt::MailboxTryPutLocal(
                    local.expect("local mailbox name"),
                    mailbox,
                    self.lower_mailbox_value(path, *value)?,
                ),
            },
            ("get" | "peek", [target]) => {
                let target = self.lower_mailbox_target(path, *target)?;
                let peek = name == "peek";
                match index {
                    Some(index) => IrObjectStmt::MailboxGet(index, mailbox, target, peek),
                    None => IrObjectStmt::MailboxGetLocal(
                        local.expect("local mailbox name"),
                        mailbox,
                        target,
                        peek,
                    ),
                }
            }
            ("try_get" | "try_peek", [target]) => {
                let target = self.lower_mailbox_target(path, *target)?;
                let peek = name == "try_peek";
                match index {
                    Some(index) => IrObjectStmt::MailboxTryGet(index, mailbox, target, peek),
                    None => IrObjectStmt::MailboxTryGetLocal(
                        local.expect("local mailbox name"),
                        mailbox,
                        target,
                        peek,
                    ),
                }
            }
            ("put" | "try_put" | "get" | "peek" | "try_get" | "try_peek", _) => {
                return Err(format!(
                    "mailbox method `{name}` has the wrong argument count in `{path}`"
                ));
            }
            _ => return Err(format!("unsupported mailbox statement method: {name}")),
        };
        Ok(IrStmt::Object(operation))
    }
}
