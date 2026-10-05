//! Declarations.

use super::*;

impl EmitCtx<'_, '_> {
    pub(super) fn lower_variable_decl(
        &mut self,
        declaration: NodeId,
    ) -> Result<Vec<IrStmt>, String> {
        if self.func.is_some() && self.cg.record_declaration(declaration) {
            return self.cg.lower_record_local(&self.path, declaration);
        }
        if self.func.is_none()
            && self.cg.db.variable_lifetime(declaration) == VariableLifetime::Automatic
            && self.cg.automatic_block_record(&self.path, declaration)?
        {
            // Automatic procedural-block records are lexical columns
            // declared and initialized at each block entry.
            return self.cg.lower_record_local(&self.path, declaration);
        }
        if let Some(value) = self
            .func
            .is_some()
            .then(|| self.cg.native_roots.get(&declaration).copied())
            .flatten()
        {
            // Native record locals: automatic storage is a lexical value
            // initialized at each entry; static storage persists.
            if !self.cg.model.native_values[value].activation {
                if self.cg.db.var_initializer(declaration).is_some() {
                    return Err(format!(
                        "initializer of static native record local `{}` in `{}` is not supported",
                        self.cg.node(declaration).name,
                        self.path
                    ));
                }
                return Ok(Vec::new());
            }
            let mut statements = vec![IrStmt::NativeValueDeclare(value)];
            if let Some(initializer) = self.cg.db.var_initializer(declaration) {
                statements.push(
                    self.cg
                        .lower_native_value_assignment(
                            &self.path,
                            declaration,
                            initializer,
                            false,
                            Operation::Assignment,
                        )?
                        .ok_or("native local initializer has no record assignment")?,
                );
            }
            return Ok(statements);
        }
        // Procedural-block descriptor arrays: static storage is persistent
        // and initializes once in the static schedule (SV §6.21); automatic
        // storage is a lexical activation initialized at each entry.
        if self.func.is_none()
            && self.cg.descriptor_declaration(declaration)
            && !self.cg.array_globals.contains_key(&declaration)
        {
            let automatic =
                self.cg.db.variable_lifetime(declaration) == VariableLifetime::Automatic;
            let mut info = self.cg.fixed_activation_array(declaration)?;
            if automatic {
                self.cg.model.arrays[info.ir].activation = true;
            } else {
                self.cg.make_fixed_array_persistent(&mut info);
            }
            self.cg.array_globals.insert(declaration, info);
            if !automatic {
                if let Some(initializer) = self.cg.db.var_initializer(declaration).or_else(|| {
                    self.cg
                        .db
                        .array_meta(declaration)
                        .and_then(|meta| meta.init)
                }) {
                    self.cg.reserve_initializer_order(declaration);
                    self.cg.array_initializers.push((declaration, initializer));
                }
                return Ok(Vec::new());
            }
        }
        if let Some(array) = self
            .cg
            .array_globals
            .get(&declaration)
            .cloned()
            .filter(|array| self.cg.model.arrays[array.ir].activation)
        {
            let mut statements = vec![IrStmt::FixedArrayDeclare(array.ir)];
            if let Some(value) = self.cg.db.var_initializer(declaration).or_else(|| {
                self.cg
                    .db
                    .array_meta(declaration)
                    .and_then(|meta| meta.init)
            }) {
                statements.push(
                    self.cg
                        .lower_p30_fixed_array_assignment(
                            &self.path,
                            declaration,
                            value,
                            true,
                            Operation::Assignment,
                        )?
                        .ok_or("fixed local initializer has no array assignment")?,
                );
            }
            return Ok(statements);
        }
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
                        return Ok(vec![IrStmt::Object(Box::new(
                            IrObjectStmt::ProcessDeclareLocal(name, init),
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
                        let mut statements = vec![IrStmt::Object(Box::new(
                            IrObjectStmt::ChandleDeclareLocal(name.clone(), None),
                        ))];
                        if let Some(init) = init {
                            statements.push(IrStmt::Object(Box::new(
                                IrObjectStmt::MailboxAssignLocal(name, init),
                            )));
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
                        return Ok(vec![IrStmt::Object(Box::new(
                            IrObjectStmt::ChandleDeclareLocal(name, init),
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
                        .or_else(|| {
                            self.cg
                                .db
                                .array_meta(declaration)
                                .and_then(|array| array.init)
                        })
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
                    let init = init
                        .or_else(|| {
                            self.cg.fixed_default_literal(declaration).map(|value| {
                                Box::new(IrExpr::new(IrExprKind::Const(value), width, signed, None))
                            })
                        })
                        .or_else(|| default_real_local_initializer(width));
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
                ProcessTarget::Local(name) => vec![IrStmt::Object(Box::new(
                    IrObjectStmt::ProcessDeclareLocal(name, init),
                ))],
                ProcessTarget::Object(index) => init
                    .map(|value| {
                        vec![IrStmt::Object(Box::new(IrObjectStmt::ProcessAssign(
                            index, value,
                        )))]
                    })
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
                ChandleTarget::Local(name) => vec![IrStmt::Object(Box::new(
                    IrObjectStmt::ChandleDeclareLocal(name, init),
                ))],
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
            let mut statements = vec![IrStmt::Object(Box::new(IrObjectStmt::ChandleDeclareLocal(
                name.clone(),
                None,
            )))];
            if let Some(init) = init {
                statements.push(IrStmt::Object(Box::new(IrObjectStmt::MailboxAssignLocal(
                    name, init,
                ))));
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
            .or_else(|| {
                self.cg
                    .db
                    .array_meta(declaration)
                    .and_then(|array| array.init)
            })
            .map(|initializer| {
                let expr = self.cg.lower_expr(&self.path, initializer)?;
                ir_to_storage(expr, info.width, info.signed, info.two_state).map(Box::new)
            })
            .transpose()?;
        let init = init
            .or_else(|| {
                self.cg.fixed_default_literal(declaration).map(|value| {
                    Box::new(IrExpr::new(
                        IrExprKind::Const(value),
                        info.width,
                        info.signed,
                        None,
                    ))
                })
            })
            .or_else(|| default_real_local_initializer(info.width));
        Ok(vec![IrStmt::DeclLocal {
            name: info.c_name,
            width: info.width,
            signed: info.signed,
            two_state: info.two_state,
            init,
        }])
    }
}
