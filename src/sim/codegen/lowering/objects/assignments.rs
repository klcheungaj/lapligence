//! Assignments.

use super::*;

impl Codegen<'_> {

    pub(in super::super) fn lower_object_assignment(
        &mut self,
        path: &str,
        lhs: NodeId,
        rhs: NodeId,
        blocking: bool,
        op: Operation,
    ) -> Result<Option<IrStmt>, String> {
        let indexed = match self.kind(lhs) {
            NodeKind::Expr(ExprKind::BitSelect { base, index }) => Some((*base, *index)),
            _ => None,
        };
        let object_node = indexed.map_or(lhs, |(base, _)| base);
        if let Some(field) = self.class_field_target(object_node) {
            if matches!(self.kind(field), NodeKind::Var { ty } if is_handle_kind(&ty.kind)) {
                if indexed.is_some() {
                    return Err("virtual interface handle cannot be indexed".to_owned());
                }
                if !blocking {
                    return Err(
                        "nonblocking assignment to virtual interface handle is not supported"
                            .to_owned(),
                    );
                }
                if op != Operation::Assignment {
                    return Err(
                        "compound assignment to virtual interface handle is unsupported".to_owned(),
                    );
                }
                self.validate_virtual_interface_assignment(object_node, rhs, path)?;
                let address = self
                    .class_field_chandle_lvalue(path, object_node)?
                    .ok_or_else(|| "class virtual interface field has no storage".to_owned())?;
                return Ok(Some(IrStmt::Object(IrObjectStmt::ChandleAssignLocal(
                    address,
                    self.lower_chandle(path, rhs)?,
                ))));
            }
        }
        let target_node = match self.kind(object_node) {
            NodeKind::Expr(ExprKind::Ref { target }) => *target,
            _ => Some(object_node),
        };
        let process_target = self
            .is_process_expr(path, object_node)
            .then(|| self.lower_process_lvalue(path, object_node))
            .transpose()?;
        let mut chandle_target = self.func.as_ref().and_then(|function| {
            target_node
                .and_then(|target| function.chandle_write.get(&target).cloned())
                .or_else(|| {
                    matches!(
                        self.kind(object_node),
                        NodeKind::Expr(ExprKind::Ref { target: None })
                    )
                    .then(|| {
                        function
                            .chandle_write
                            .iter()
                            .find(|(target, _)| {
                                self.node(**target).name == self.node(object_node).name
                            })
                            .map(|(_, target)| target.clone())
                    })
                    .flatten()
                })
        });
        if chandle_target.is_none() {
            chandle_target = self.semaphore_lvalue_target(path, object_node)?;
        }
        let string_target = self.func.as_ref().and_then(|function| {
            target_node
                .and_then(|target| function.string_write.get(&target).cloned())
                .or_else(|| {
                    matches!(
                        self.kind(object_node),
                        NodeKind::Expr(ExprKind::Ref { target: None })
                    )
                    .then(|| {
                        function
                            .string_write
                            .iter()
                            .find(|(target, _)| {
                                self.node(**target).name == self.node(object_node).name
                            })
                            .map(|(_, target)| target.clone())
                    })
                    .flatten()
                })
        });
        let string_target = string_target.or_else(|| {
            self.lexical_proc_string_local(object_node)
                .map(|(_, name)| name.to_owned())
        });
        let string_target = match string_target {
            Some(target) => Some(target),
            None => self.class_field_string_lvalue(path, object_node)?,
        };
        let string_const_ref = self.func.as_ref().is_some_and(|function| {
            target_node.is_some_and(|target| {
                function.string_read.contains_key(&target)
                    && !function.string_write.contains_key(&target)
            })
        });
        let index = self.object_of(path, object_node);
        let is_mailbox = self.is_mailbox_expr(path, object_node);
        let mailbox_target = if is_mailbox {
            index
                .map(ChandleTarget::Object)
                .or_else(|| chandle_target.clone())
                .or_else(|| {
                    target_node
                        .and_then(|target| self.proc_mailbox_local_name(target))
                        .or_else(|| {
                            self.lexical_proc_mailbox_local(object_node)
                                .map(|(_, name)| name)
                        })
                        .map(|name| ChandleTarget::Local(name.to_owned()))
                })
                .or_else(|| {
                    target_node
                        .and_then(|target| {
                            self.proc_mailbox_static_objects
                                .get(&(self.inst, target))
                                .copied()
                        })
                        .map(ChandleTarget::Object)
                })
        } else {
            None
        };
        if string_const_ref && string_target.is_none() {
            return Err("cannot mutate a const-ref string formal".to_owned());
        }
        if !is_mailbox
            && index.is_none()
            && process_target.is_none()
            && chandle_target.is_none()
            && string_target.is_none()
        {
            return Ok(None);
        }
        if is_mailbox && mailbox_target.is_none() {
            return Err("mailbox handle has no writable storage in this context".to_owned());
        }
        if !blocking {
            return Err(
                "nonblocking assignment to dynamic string/chandle storage is not supported"
                    .to_owned(),
            );
        }
        if op != Operation::Assignment {
            return Err("compound assignment to non-integral storage is unsupported".to_owned());
        }
        if let Some(target) = mailbox_target {
            if indexed.is_some() {
                return Err("mailbox handle cannot be indexed".to_owned());
            }
            let declaration = target_node.unwrap_or(object_node);
            let value =
                self.lower_mailbox_expr(path, rhs, self.mailbox_element_for_decl(declaration))?;
            return Ok(Some(IrStmt::Object(match target {
                ChandleTarget::Object(index) => IrObjectStmt::MailboxAssign(index, value),
                ChandleTarget::Local(name) => IrObjectStmt::MailboxAssignLocal(name, value),
            })));
        }
        if let Some((target, _)) = process_target {
            if indexed.is_some() {
                return Err("process handle cannot be indexed".to_owned());
            }
            let value = self.lower_process(path, rhs)?;
            let operation = match target {
                ProcessTarget::Object(index) => IrObjectStmt::ProcessAssign(index, value),
                ProcessTarget::Local(name) => IrObjectStmt::ProcessAssignLocal(name, value),
            };
            return Ok(Some(IrStmt::Object(operation)));
        }
        if let Some(target) = chandle_target {
            if indexed.is_some() {
                return Err("chandle cannot be indexed".to_owned());
            }
            self.validate_virtual_interface_assignment(object_node, rhs, path)?;
            let value = self.lower_chandle(path, rhs)?;
            return Ok(Some(IrStmt::Object(match target {
                ChandleTarget::Object(index) => IrObjectStmt::ChandleAssign(index, value),
                ChandleTarget::Local(name) => IrObjectStmt::ChandleAssignLocal(name, value),
            })));
        }
        if let Some(target) = string_target {
            if let Some((_, position)) = indexed {
                return Ok(Some(IrStmt::Object(IrObjectStmt::StringPutcLocal(
                    target,
                    self.object_int_argument(path, position, 32)?,
                    self.object_int_argument(path, rhs, 8)?,
                ))));
            }
            return Ok(Some(IrStmt::Object(IrObjectStmt::StringAssignLocal(
                target,
                self.lower_string(path, rhs)?,
            ))));
        }
        let index = index.expect("object target checked above");
        let operation = match self.model.objects[index].ty {
            IrObjectType::String => match indexed {
                Some((_, position)) => IrObjectStmt::StringPutc(
                    index,
                    self.object_int_argument(path, position, 32)?,
                    self.object_int_argument(path, rhs, 8)?,
                ),
                None => IrObjectStmt::StringAssign(index, self.lower_string(path, rhs)?),
            },
            IrObjectType::Chandle => {
                if indexed.is_some() {
                    return Err("chandle cannot be indexed".to_owned());
                }
                self.validate_virtual_interface_assignment(object_node, rhs, path)?;
                IrObjectStmt::ChandleAssign(index, self.lower_chandle(path, rhs)?)
            }
            IrObjectType::Semaphore => {
                if indexed.is_some() {
                    return Err("semaphore handle cannot be indexed".to_owned());
                }
                IrObjectStmt::ChandleAssign(index, self.lower_chandle(path, rhs)?)
            }
            IrObjectType::Process => {
                if indexed.is_some() {
                    return Err("process handle cannot be indexed".to_owned());
                }
                IrObjectStmt::ProcessAssign(index, self.lower_process(path, rhs)?)
            }
        };
        Ok(Some(IrStmt::Object(operation)))
    }
}
