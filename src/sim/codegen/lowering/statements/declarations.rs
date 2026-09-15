//! Declarations.

use super::*;

impl EmitCtx<'_, '_> {

    pub(super) fn lower_variable_decl(&mut self, declaration: NodeId) -> Result<Vec<IrStmt>, String> {
        if self.func.is_some() {
            return match self.cg.db.variable_lifetime(declaration) {
                VariableLifetime::Static => Ok(Vec::new()),
                VariableLifetime::Automatic => {
                    if matches!(
                        self.cg.kind(declaration),
                        NodeKind::Var { ty }
                            if ty.kind == "class" && ty.type_name.as_deref() == Some("process")
                    ) {
                        let name = self
                            .func
                            .as_ref()
                            .and_then(|function| function.process_read.get(&declaration))
                            .and_then(|value| match value {
                                crate::sim::ir::IrProcessExpr::LocalRead(name) => {
                                    Some(name.clone())
                                }
                                _ => None,
                            })
                            .ok_or_else(|| {
                                format!(
                                    "automatic process variable `{}` has no local storage",
                                    self.cg.node(declaration).name
                                )
                            })?;
                        let init = self
                            .cg
                            .db
                            .var_initializer(declaration)
                            .map(|initializer| self.cg.lower_process(&self.path, initializer))
                            .transpose()?;
                        return Ok(vec![IrStmt::Object(IrObjectStmt::ProcessDeclareLocal(
                            name, init,
                        ))]);
                    }
                    if matches!(self.cg.kind(declaration), NodeKind::Var { ty } if ty.kind == "class")
                        && self.cg.is_mailbox_expr(&self.path, declaration)
                    {
                        let name = self
                            .func
                            .as_ref()
                            .and_then(|function| function.chandle_read.get(&declaration))
                            .and_then(|value| match value {
                                IrChandleExpr::LocalRead(name) => Some(name.clone()),
                                _ => None,
                            })
                            .ok_or_else(|| {
                                format!(
                                    "automatic mailbox variable `{}` has no local storage",
                                    self.cg.node(declaration).name
                                )
                            })?;
                        let init = self
                            .cg
                            .db
                            .var_initializer(declaration)
                            .map(|initializer| {
                                self.cg.lower_mailbox_expr(
                                    &self.path,
                                    initializer,
                                    self.cg.mailbox_element_for_decl(declaration),
                                )
                            })
                            .transpose()?;
                        let mut statements = vec![IrStmt::Object(
                            IrObjectStmt::ChandleDeclareLocal(name.clone(), None),
                        )];
                        if let Some(init) = init {
                            statements
                                .push(IrStmt::Object(IrObjectStmt::MailboxAssignLocal(name, init)));
                        }
                        return Ok(statements);
                    }
                    if matches!(self.cg.kind(declaration), NodeKind::Var { ty } if is_handle_kind(&ty.kind))
                    {
                        let name = self
                            .func
                            .as_ref()
                            .and_then(|function| function.chandle_read.get(&declaration))
                            .and_then(|value| match value {
                                IrChandleExpr::LocalRead(name) => Some(name.clone()),
                                _ => None,
                            })
                            .ok_or_else(|| {
                                format!(
                                    "automatic chandle variable `{}` has no local storage",
                                    self.cg.node(declaration).name
                                )
                            })?;
                        let init = self
                            .cg
                            .db
                            .var_initializer(declaration)
                            .map(|initializer| self.cg.lower_chandle(&self.path, initializer))
                            .transpose()?;
                        return Ok(vec![IrStmt::Object(IrObjectStmt::ChandleDeclareLocal(
                            name, init,
                        ))]);
                    }
                    if matches!(self.cg.kind(declaration), NodeKind::Var { ty } if ty.kind == "string")
                    {
                        let name = self
                            .func
                            .as_ref()
                            .and_then(|function| function.locals.get(&declaration))
                            .map(|(name, ..)| name.clone())
                            .ok_or_else(|| {
                                format!(
                                    "automatic string variable `{}` has no local storage",
                                    self.cg.node(declaration).name
                                )
                            })?;
                        let init = self
                            .cg
                            .db
                            .var_initializer(declaration)
                            .map(|initializer| self.cg.lower_string(&self.path, initializer))
                            .transpose()?;
                        return Ok(vec![IrStmt::DeclString { name, init }]);
                    }
                    let (_, width, signed, two_state, shortreal) = self
                        .func
                        .as_ref()
                        .and_then(|function| function.locals.get(&declaration))
                        .cloned()
                        .ok_or_else(|| {
                            format!(
                                "automatic subprogram variable `{}` has no local storage",
                                self.cg.node(declaration).name
                            )
                        })?;
                    let init = self
                        .cg
                        .db
                        .var_initializer(declaration)
                        .map(|initializer| {
                            let expression = self.cg.lower_expr(&self.path, initializer)?;
                            if width == 0 {
                                Ok(IrExpr::new(
                                    IrExprKind::CastToReal {
                                        a: Box::new(expression),
                                        shortreal,
                                    },
                                    0,
                                    false,
                                    None,
                                ))
                            } else {
                                ir_to_storage(expression, width, signed, two_state)
                            }
                            .map(Box::new)
                        })
                        .transpose()?;
                    let init = init.or_else(|| default_real_local_initializer(width));
                    let name = self
                        .func
                        .as_ref()
                        .and_then(|function| function.locals.get(&declaration))
                        .map(|(name, ..)| name.clone())
                        .expect("automatic local storage checked above");
                    Ok(vec![IrStmt::DeclLocal {
                        name,
                        width,
                        signed,
                        two_state,
                        init,
                    }])
                }
                VariableLifetime::Unavailable => Err(format!(
                    "resolved lifetime is unavailable for subprogram variable `{}` in `{}`",
                    self.cg.node(declaration).name,
                    self.path
                )),
            };
        }

        if matches!(
            self.cg.kind(declaration),
            NodeKind::Var { ty }
                if ty.kind == "class" && ty.type_name.as_deref() == Some("process")
        ) {
            let target = self.cg.collect_process_local(&self.path, declaration)?;
            let init = self
                .cg
                .db
                .var_initializer(declaration)
                .map(|initializer| self.cg.lower_process(&self.path, initializer))
                .transpose()?;
            return Ok(match target {
                ProcessTarget::Local(name) => vec![IrStmt::Object(
                    IrObjectStmt::ProcessDeclareLocal(name, init),
                )],
                ProcessTarget::Object(index) => init
                    .map(|value| vec![IrStmt::Object(IrObjectStmt::ProcessAssign(index, value))])
                    .unwrap_or_default(),
            });
        }

        if matches!(
            self.cg.kind(declaration),
            NodeKind::Var { ty }
                if ty.kind == "class" && ty.type_name.as_deref() == Some("semaphore")
        ) {
            let target = self.cg.collect_semaphore_local(&self.path, declaration)?;
            let init = self
                .cg
                .db
                .var_initializer(declaration)
                .map(|initializer| self.cg.lower_chandle(&self.path, initializer))
                .transpose()?;
            return Ok(match target {
                ChandleTarget::Local(name) => vec![IrStmt::Object(
                    IrObjectStmt::ChandleDeclareLocal(name, init),
                )],
                ChandleTarget::Object(_) => Vec::new(),
            });
        }

        if self.cg.is_mailbox_expr(&self.path, declaration) {
            if self.cg.db.variable_lifetime(declaration) == VariableLifetime::Static {
                self.cg
                    .collect_mailbox_static_object(&self.path, declaration)?;
                return Ok(Vec::new());
            }
            let name = self.cg.collect_mailbox_local(&self.path, declaration)?;
            let init = self
                .cg
                .db
                .var_initializer(declaration)
                .map(|initializer| {
                    self.cg.lower_mailbox_expr(
                        &self.path,
                        initializer,
                        self.cg.mailbox_element_for_decl(declaration),
                    )
                })
                .transpose()?;
            let mut statements = vec![IrStmt::Object(IrObjectStmt::ChandleDeclareLocal(
                name.clone(),
                None,
            ))];
            if let Some(init) = init {
                statements.push(IrStmt::Object(IrObjectStmt::MailboxAssignLocal(name, init)));
            }
            return Ok(statements);
        }

        let info = self.cg.collect_loop_var(&self.path, declaration)?;
        if info.static_signal.is_some() {
            return Ok(Vec::new());
        }
        let init = self
            .cg
            .db
            .var_initializer(declaration)
            .map(|initializer| {
                let expr = self.cg.lower_expr(&self.path, initializer)?;
                ir_to_storage(expr, info.width, info.signed, info.two_state).map(Box::new)
            })
            .transpose()?;
        let init = init.or_else(|| default_real_local_initializer(info.width));
        Ok(vec![IrStmt::DeclLocal {
            name: info.c_name,
            width: info.width,
            signed: info.signed,
            two_state: info.two_state,
            init,
        }])
    }
}
