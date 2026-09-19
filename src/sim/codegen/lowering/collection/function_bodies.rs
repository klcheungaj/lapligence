//! Function bodies.

use super::*;

impl<'a> Codegen<'a> {
    /// Emit one static C function for a function/task definition.  The body
    /// statements are emitted with the io_decls mapped to the C parameters and
    /// the locals to C locals; the function-name variable maps to a local
    /// `_ret` that `return` reads.
    pub(super) fn emit_func_task(
        &mut self,
        path: &str,
        inst: NodeId,
        ft: NodeId,
    ) -> Result<(), String> {
        let automatic = matches!(
            self.kind(ft),
            NodeKind::FuncTask {
                automatic: true,
                ..
            }
        );
        let (is_task, ret, formals) = self.func_info(ft, inst)?;
        let ret_chandle = self.is_chandle_return(ft);
        let ret_string = self.is_string_return(ft);
        let (decl, _) = self.func_signature(ft, inst)?;
        let c_name = self
            .func_names
            .get(&ft)
            .cloned()
            .ok_or_else(|| format!("function `{}` has no C name", self.node(ft).name))?;
        let meta_ir = self
            .func_meta
            .get(&ft)
            .map(|meta| meta.ir)
            .ok_or_else(|| format!("function `{}` has no model entry", self.node(ft).name))?;
        if matches!(self.kind(ft), NodeKind::FuncTask { is_pure: true, .. }) {
            // Pure virtual methods have no executable source body. Keep a
            // typed fallback so the virtual dispatcher has a linkable target
            // for an invalid abstract-object call; valid concrete objects
            // always select an overriding implementation instead.
            self.model.funcs[meta_ir].body = vec![IrStmt::Return { value: None }];
            return Ok(());
        }
        let has_ret = ret.is_some() || ret_chandle || ret_string;
        // Slang binds an assignment to the function name directly to the
        // subroutine symbol; that symbol is the return-storage identity.
        let ret_var = has_ret.then_some(ft);
        let body = self
            .func_body(ft)
            .ok_or_else(|| format!("function `{}` without a body", self.node(ft).name))?;
        if self.node_has_stack_backed_subroutine_nba(body, ft, automatic) {
            let kind = if is_task { "task" } else { "function" };
            return Err(format!(
                "nonblocking assignment in {kind} `{}` targets stack-backed input/formal/local storage which cannot outlive the call",
                self.node(ft).name
            ));
        }

        // The all-X return value used by the recursion guard.
        let ret_x = match ret {
            Some((w, s, two_state, _shortreal)) if w > 0 => format!(
                "{}({w}, {})",
                if two_state { "sv4_zero" } else { "sv4_x" },
                s as u8
            ),
            Some((_, _, _, _)) => "0.0".to_owned(),
            None => String::new(),
        };
        let guard = if has_ret {
            format!(
                "if (depth >= {LLG_MAX_FUNC_DEPTH}) {{\n        fprintf(stderr, \"llg: recursion limit exceeded in %s\\n\", \"{c_name}\");\n        return {ret_x};\n    }}\n"
            )
        } else {
            format!(
                "if (depth >= {LLG_MAX_FUNC_DEPTH}) {{\n        fprintf(stderr, \"llg: recursion limit exceeded in %s\\n\", \"{c_name}\");\n        return;\n    }}\n"
            )
        };

        let mut locals: HashMap<NodeId, (String, u32, bool, bool, bool)> = HashMap::new();
        let mut chandle_locals: HashMap<NodeId, String> = HashMap::new();
        let mut process_locals: HashMap<NodeId, String> = HashMap::new();
        let mut local_seq = 0usize;
        let local_prefix = format!("_f{}_{}_", inst.index(), ft.index());
        self.collect_func_locals(
            body,
            inst,
            &mut locals,
            &mut chandle_locals,
            &mut process_locals,
            &mut local_seq,
            &local_prefix,
        )?;
        let mut declaration_initializers = HashMap::new();
        self.collect_subroutine_decl_initializers(body, &mut declaration_initializers);

        // Function-name return variable → `_ret` local.
        let ret_ctx = match (ret, ret_var) {
            (Some((w, s, two_state, shortreal)), Some(rv)) => Some(RetCtx {
                c_name: "_ret".to_string(),
                width: w,
                signed: s,
                two_state,
                shortreal,
                node: Some(rv),
            }),
            _ => None,
        };

        let mut arg_read: HashMap<NodeId, ArgMap> = HashMap::new();
        let mut arg_ir: HashMap<NodeId, IrExpr> = HashMap::new();
        let mut arg_write: HashMap<NodeId, String> = HashMap::new();
        let mut arg_lhs: HashMap<NodeId, Lhs> = HashMap::new();
        let mut const_refs: HashSet<NodeId> = HashSet::new();
        let const_ref_lhs = HashMap::new();
        let mut persistent = HashMap::new();
        let mut chandle_read = HashMap::new();
        let mut chandle_write = HashMap::new();
        let mut process_read = HashMap::new();
        let mut process_write = HashMap::new();
        let mut string_read = HashMap::new();
        let mut string_write = HashMap::new();
        let mut string_addr = HashMap::new();
        let mut static_input_copies = Vec::new();
        for (local, name) in &chandle_locals {
            if self.db.variable_lifetime(*local) == VariableLifetime::Static {
                let object = if let Some(object) = self
                    .static_task_chandle_locals
                    .get(&(inst, *local))
                    .copied()
                {
                    object
                } else {
                    let object = self.model.objects.len();
                    self.model.objects.push(crate::sim::ir::IrObject {
                        c_name: format!("O_f{}_l{}", inst.index(), local.index()),
                        ty: crate::sim::ir::IrObjectType::Chandle,
                        initial: None,
                    });
                    self.static_task_chandle_locals
                        .insert((inst, *local), object);
                    object
                };
                // Mailboxes use the same native pointer storage as other
                // handles, but their declaration initializer must construct
                // a runtime mailbox before the first function call.  Queue
                // it with the other model-time mailbox initializers instead
                // of silently dropping it with ordinary static handles.
                if self.is_mailbox_expr(path, *local) {
                    if let Some(initializer) = self.db.var_initializer(*local) {
                        self.mailbox_object_initializers.push((
                            *local,
                            object,
                            initializer,
                            path.to_owned(),
                        ));
                    }
                }
                chandle_read.insert(*local, IrChandleExpr::Read(object));
                chandle_write.insert(*local, ChandleTarget::Object(object));
            } else {
                chandle_read.insert(*local, IrChandleExpr::LocalRead(name.clone()));
                chandle_write.insert(*local, ChandleTarget::Local(name.clone()));
            }
        }
        for (local, name) in &process_locals {
            process_read.insert(
                *local,
                crate::sim::ir::IrProcessExpr::LocalRead(name.clone()),
            );
            process_write.insert(*local, ProcessTarget::Local(name.clone()));
        }
        for (local, (name, ..)) in &locals {
            if matches!(self.kind(*local), NodeKind::Var { ty } if ty.kind == "string") {
                string_read.insert(*local, IrStringExpr::LocalRead(name.clone()));
                string_write.insert(*local, name.clone());
                string_addr.insert(*local, name.clone());
            }
        }
        for (idx, (io, is_out)) in formals.iter().enumerate() {
            if matches!(self.kind(*io), NodeKind::FuncArg { ty, .. } if ty.kind == "event") {
                // Event formals are admitted only through inline call
                // lowering, which substitutes the caller's object identity.
                // The fallback C body is retained for deterministic model
                // shape but has no packed formal storage.
                continue;
            }
            if matches!(self.kind(*io), NodeKind::FuncArg { ty, .. } if is_handle_kind(&ty.kind)) {
                let is_ref = matches!(
                    self.kind(*io),
                    NodeKind::FuncArg {
                        direction: DbDirection::Ref,
                        ..
                    }
                );
                let const_ref = matches!(
                    self.kind(*io),
                    NodeKind::FuncArg {
                        direction: DbDirection::Ref,
                        const_ref: true,
                        ..
                    }
                );
                if !is_ref && !*is_out {
                    if let Some(object) = (!automatic)
                        .then(|| self.static_chandle_formals.get(&(inst, *io)).copied())
                        .flatten()
                    {
                        chandle_read.insert(*io, IrChandleExpr::Read(object));
                        chandle_write.insert(*io, ChandleTarget::Object(object));
                        static_input_copies.push(IrStmt::Object(
                            crate::sim::ir::IrObjectStmt::ChandleAssign(
                                object,
                                IrChandleExpr::FormalRead(idx),
                            ),
                        ));
                        continue;
                    }
                }
                {
                    chandle_read.insert(*io, IrChandleExpr::FormalRead(idx));
                    if !const_ref {
                        let target = if is_ref {
                            format!("*r{idx}")
                        } else if *is_out {
                            format!("*o{idx}")
                        } else {
                            format!("a{idx}")
                        };
                        chandle_write.insert(*io, ChandleTarget::Local(target));
                    }
                }
                continue;
            }
            if matches!(self.kind(*io), NodeKind::FuncArg { ty, .. } if ty.kind == "string") {
                let is_ref = matches!(
                    self.kind(*io),
                    NodeKind::FuncArg {
                        direction: DbDirection::Ref,
                        ..
                    }
                );
                let const_ref = matches!(
                    self.kind(*io),
                    NodeKind::FuncArg {
                        const_ref: true,
                        ..
                    }
                );
                if is_ref {
                    string_read.insert(*io, IrStringExpr::FormalRead(idx));
                    string_addr.insert(*io, format!("*r{idx}"));
                    if !const_ref {
                        string_write.insert(*io, format!("*r{idx}"));
                    }
                } else if let Some(object) = (!automatic)
                    .then(|| self.static_string_formals.get(&(inst, *io)).copied())
                    .flatten()
                {
                    let name = self.model.objects[object].c_name.clone();
                    string_read.insert(*io, IrStringExpr::Read(object));
                    string_write.insert(*io, name.clone());
                    string_addr.insert(*io, name);
                    if !*is_out {
                        static_input_copies.push(IrStmt::Object(IrObjectStmt::StringAssign(
                            object,
                            IrStringExpr::FormalRead(idx),
                        )));
                    }
                } else {
                    string_read.insert(*io, IrStringExpr::FormalRead(idx));
                    string_write.insert(
                        *io,
                        if *is_out {
                            format!("*o{idx}")
                        } else {
                            format!("a{idx}")
                        },
                    );
                    string_addr.insert(
                        *io,
                        if *is_out {
                            format!("*o{idx}")
                        } else {
                            format!("a{idx}")
                        },
                    );
                }
                continue;
            }
            let (w, s, two_state, _real, _shortreal) = match self.kind(*io) {
                NodeKind::FuncArg { ty, .. } => {
                    if is_real_kind(&ty.kind) {
                        (0, false, false, true, ty.kind == "shortreal")
                    } else {
                        match ty.width {
                            Some(w) if w <= LLG_MAX_WIDTH => (
                                self.effective_decl_width(*io, inst, w),
                                ty.signed,
                                self.db.is_two_state_type(*io) || is_two_state_kind(&ty.kind),
                                false,
                                false,
                            ),
                            Some(w) => {
                                return Err(format!(
                                    "formal `{}` of `{c_name}` is {w} bits wide; the runtime \
                             maximum supported width is {LLG_MAX_WIDTH}",
                                    self.node(*io).name
                                ))
                            }
                            None => {
                                return Err(format!(
                                    "formal `{}` of `{c_name}` has no width",
                                    self.node(*io).name
                                ))
                            }
                        }
                    }
                }
                _ => unreachable!("formal kind"),
            };
            let (is_ref, const_ref) = match self.kind(*io) {
                NodeKind::FuncArg {
                    direction: DbDirection::Ref,
                    const_ref,
                    ..
                } => (true, *const_ref),
                _ => (false, false),
            };
            if is_ref {
                arg_ir.insert(*io, formal_read_expr(idx, w, s));
                arg_read.insert(
                    *io,
                    ArgMap {
                        width: w,
                        signed: s,
                        two_state,
                    },
                );
                if const_ref {
                    const_refs.insert(*io);
                } else {
                    arg_lhs.insert(
                        *io,
                        Lhs::Ref {
                            addr: format!("r{idx}"),
                            width: w,
                            signed: s,
                            two_state,
                            const_ref: false,
                        },
                    );
                }
                continue;
            }
            if let Some(storage) = (!automatic)
                .then(|| self.static_formals.get(&(inst, *io)).cloned())
                .flatten()
            {
                arg_write.insert(*io, format!("&{}", storage.global));
                persistent.insert(*io, storage.clone());
                arg_ir.insert(*io, sig_read_expr_full(&storage));
                arg_read.insert(
                    *io,
                    ArgMap {
                        width: w,
                        signed: s,
                        two_state,
                    },
                );
                if !*is_out {
                    let lhs = IrLhs::Whole(storage.ir);
                    static_input_copies.push(IrStmt::Assign {
                        lhs: lhs.clone(),
                        rhs: apply_lhs_assignment_context(
                            &self.model,
                            &lhs,
                            formal_read_expr(idx, w, s),
                        ),
                        nba: false,
                    });
                }
            } else if *is_out {
                arg_write.insert(*io, format!("o{idx}"));
                arg_ir.insert(*io, formal_read_expr(idx, w, s));
                arg_read.insert(
                    *io,
                    ArgMap {
                        width: w,
                        signed: s,
                        two_state,
                    },
                );
            } else {
                // Input formal: a by-value C parameter.  Writing an input
                // formal is legal SystemVerilog (it is a local copy), so the
                // parameter itself is also a valid write target.
                arg_write.insert(*io, format!("&a{idx}"));
                arg_ir.insert(*io, formal_read_expr(idx, w, s));
                arg_read.insert(
                    *io,
                    ArgMap {
                        width: w,
                        signed: s,
                        two_state,
                    },
                );
            }
        }
        if ret_chandle {
            let return_var = ret_var.ok_or_else(|| {
                format!(
                    "chandle function `{}` has no return variable",
                    self.node(ft).name
                )
            })?;
            chandle_read.insert(return_var, IrChandleExpr::LocalRead("_ret".to_string()));
            chandle_write.insert(return_var, ChandleTarget::Local("_ret".to_string()));
        }
        if ret_string {
            let return_var = ret_var.ok_or_else(|| {
                format!(
                    "string function `{}` has no return variable",
                    self.node(ft).name
                )
            })?;
            string_read.insert(
                return_var,
                crate::sim::ir::IrStringExpr::LocalRead("_ret".to_string()),
            );
            string_write.insert(return_var, "_ret".to_string());
            string_addr.insert(return_var, "_ret".to_string());
        }

        let func_ctx = FuncCtx {
            name: self.node(ft).name.clone(),
            is_task,
            class_receiver: self
                .class_nodes
                .get(&inst)
                .filter(|_| {
                    !matches!(
                        self.kind(ft),
                        NodeKind::FuncTask {
                            is_static: true,
                            ..
                        }
                    )
                })
                .map(|_| IrChandleExpr::LocalRead("_this".to_owned())),
            ret: ret_ctx.clone(),
            arg_read,
            arg_ir,
            event_args: HashMap::new(),
            arg_dependencies: HashMap::new(),
            arg_write,
            arg_lhs,
            const_refs,
            const_ref_lhs,
            persistent,
            chandle_read,
            chandle_write,
            process_read,
            process_write,
            string_read,
            string_write,
            string_addr,
            locals: locals.clone(),
            ret_node: ret_var,
            def_node: Some(ft),
        };
        self.cur_fn_ir = Some(meta_ir);
        // Lower the body under the function context; the guard, `_ret`
        // declaration and locals are rendered by the backend from the
        // `IrFunc` metadata.
        let (mut body_stmts, mut pre_fns) = {
            let mut ctx = EmitCtx::new(
                self,
                path.to_string(),
                inst,
                "depth + 1",
                Some(func_ctx),
                None,
                false,
            );
            let mut body_stmts = static_input_copies;
            if matches!(
                ctx.cg.kind(ft),
                NodeKind::FuncTask {
                    is_constructor: true,
                    ..
                }
            ) && !ctx.cg.node_contains_super_constructor(body)
            {
                body_stmts.extend(ctx.cg.lower_implicit_class_construction(
                    path,
                    inst,
                    IrChandleExpr::LocalRead("_this".to_owned()),
                )?);
            }
            body_stmts.extend(ctx.lower_stmt(body)?);
            let pre_fns = std::mem::take(&mut ctx.pre_fns);
            (body_stmts, pre_fns)
        };
        pre_fns.extend(std::mem::take(&mut self.pending_container_pre_fns));
        // Delay-free module tasks need the same declaration-level activation
        // as an inlined timed task. Class methods have no module activation
        // environment; the bounded class subset admits only direct,
        // delay-free task calls, so they do not need this cancellation scope.
        if is_task && !self.class_nodes.contains_key(&inst) {
            body_stmts = vec![IrStmt::ActivationScope {
                target: self.activation_target(ft)?,
                exit: self.new_fn_name(path, "task_exit"),
                body: body_stmts,
            }];
        }
        let mut declaration_initializations = Vec::new();
        // Static subprogram initializers run in the model initialization
        // frame, not inside the function body, so a call they contain starts
        // at process recursion depth zero rather than this body's depth.
        self.depth_arg = "0".to_string();
        for (local, (c_name, width, signed, two_state, shortreal)) in &locals {
            if self.db.variable_lifetime(*local) != VariableLifetime::Static {
                continue;
            }
            let initializer = self
                .db
                .var_initializer(*local)
                .or_else(|| declaration_initializers.get(local).copied());
            let Some(initializer) = initializer else {
                continue;
            };
            let initialization = self
                .lower_declaration_initializer(
                    path,
                    *local,
                    initializer,
                    IrInitTarget::StaticLocal {
                        function: meta_ir,
                        name: c_name.clone(),
                    },
                    *width,
                    *signed,
                    *two_state,
                    *shortreal,
                )
                .map_err(|cause| {
                    format!(
                        "static subprogram initializer for `{}` cannot be lowered: {cause}",
                        self.node(*local).name
                    )
                })?;
            declaration_initializations.push(initialization);
        }
        self.declaration_inits.extend(declaration_initializations);
        // Restore the process-level context for whatever is lowered next
        // (continuous assignments, processes).
        self.func = None;
        self.cur_fn_ir = None;
        self.depth_arg = "0".to_string();

        let ir_locals = {
            let mut names = locals.into_iter().collect::<Vec<_>>();
            names.sort_by_key(|(id, _)| id.0);
            let mut emitted = HashSet::new();
            names
                .into_iter()
                .filter(|(local, _)| self.db.variable_lifetime(*local) == VariableLifetime::Static)
                .filter(|(_, (c_name, ..))| emitted.insert(c_name.clone()))
                .map(|(local, (c_name, width, signed, two_state, shortreal))| {
                    Ok(crate::sim::ir::IrLocal {
                        c_name,
                        width,
                        signed,
                        two_state,
                        real: width == 0,
                        shortreal,
                        string: matches!(self.kind(local), NodeKind::Var { ty } if ty.kind == "string"),
                        initial: None,
                    })
                })
                .collect::<Result<Vec<_>, String>>()?
        };
        let no_entry = format!("function `{}` has no model entry", self.node(ft).name);
        let entry = self.model.funcs.get_mut(meta_ir).ok_or(no_entry)?;
        entry.locals = ir_locals;
        entry.automatic = automatic;
        entry.pre_fns = pre_fns;
        entry.body = body_stmts;
        let _ = (guard, decl, has_ret, ret_x, c_name.as_str());
        Ok(())
    }

    /// Collect the local variables declared by a function/task body's begin
    /// blocks (recursively) into `locals`, keyed by the var arena node.
    /// `prefix` disambiguates the C local names across inline sites (each
    /// inlined task body gets its own prefix); pass `""` for C function
    /// bodies, whose locals are scoped per function.
    #[allow(clippy::too_many_arguments)]
    pub(in super::super) fn collect_func_locals(
        &self,
        node: NodeId,
        inst: NodeId,
        locals: &mut HashMap<NodeId, (String, u32, bool, bool, bool)>,
        chandle_locals: &mut HashMap<NodeId, String>,
        process_locals: &mut HashMap<NodeId, String>,
        seq: &mut usize,
        prefix: &str,
    ) -> Result<(), String> {
        if let NodeKind::Stmt(StmtKind::For { body, .. }) = self.kind(node) {
            // For-declaration variables have loop-entry lifetime and are
            // collected by `lower_for`; they are not function-entry locals.
            return self.collect_func_locals(
                *body,
                inst,
                locals,
                chandle_locals,
                process_locals,
                seq,
                prefix,
            );
        }
        if let NodeKind::Stmt(StmtKind::Foreach { body, .. }) = self.kind(node) {
            // Foreach iterator variables have the same loop-entry lifetime;
            // omitted slots carry no declaration and are skipped naturally.
            return self.collect_func_locals(
                *body,
                inst,
                locals,
                chandle_locals,
                process_locals,
                seq,
                prefix,
            );
        }
        if self.is_foreach_iterator(node) {
            // Foreach iterators are declared by `lower_foreach` at loop entry,
            // not as function-entry locals.
            return Ok(());
        }
        if let NodeKind::Var { ty } = self.kind(node) {
            self.explicit_local_lifetime(node)?;
            if is_handle_kind(&ty.kind) {
                chandle_locals.entry(node).or_insert_with(|| {
                    let cname = format!("{prefix}_l{seq}");
                    *seq += 1;
                    cname
                });
                return Ok(());
            }
            if ty.kind == "class" && ty.type_name.as_deref() == Some("process") {
                if self.db.variable_lifetime(node) != VariableLifetime::Automatic {
                    return Err(format!(
                        "static process handle `{}` in subprogram is not supported",
                        self.node(node).name
                    ));
                }
                process_locals.entry(node).or_insert_with(|| {
                    let cname = format!("{prefix}_p{seq}");
                    *seq += 1;
                    cname
                });
                return Ok(());
            }
            if locals.contains_key(&node) {
                return Ok(());
            }
            let (w, shortreal) = if ty.kind == "string" {
                (0, false)
            } else if is_real_kind(&ty.kind) {
                (0, ty.kind == "shortreal")
            } else {
                (
                    match ty.width {
                        Some(w) if w <= LLG_MAX_WIDTH => self.effective_decl_width(node, inst, w),
                        Some(w) => {
                            return Err(format!(
                                "local `{}` is {w} bits wide; the runtime supports at most \
                         {LLG_MAX_WIDTH}",
                                self.node(node).name
                            ))
                        }
                        None => {
                            return Err(format!("local `{}` has no width", self.node(node).name))
                        }
                    },
                    false,
                )
            };
            let cname = format!("{prefix}_l{seq}");
            *seq += 1;
            locals.insert(
                node,
                (
                    cname,
                    w,
                    if w == 0 { false } else { ty.signed },
                    if w == 0 {
                        false
                    } else {
                        is_two_state_kind(&ty.kind)
                    },
                    shortreal,
                ),
            );
            return Ok(());
        }
        for c in &self.node(node).children {
            self.collect_func_locals(
                *c,
                inst,
                locals,
                chandle_locals,
                process_locals,
                seq,
                prefix,
            )?;
        }
        Ok(())
    }

    fn explicit_local_lifetime(&self, node: NodeId) -> Result<Option<&'static str>, String> {
        use crate::core::db::VariableLifetimeQualifier;
        match self.db.variable_lifetime_qualifier(node) {
            VariableLifetimeQualifier::None => Ok(None),
            VariableLifetimeQualifier::Static => Ok(Some("static")),
            VariableLifetimeQualifier::Automatic => Ok(Some("automatic")),
            VariableLifetimeQualifier::Ambiguous => Err(format!(
                "subprogram local `{}` has ambiguous explicit lifetime provenance",
                self.node(node).name
            )),
            VariableLifetimeQualifier::Unavailable => Err(format!(
                "cannot determine whether subprogram local `{}` has an explicit lifetime qualifier because admitted source provenance is unavailable",
                self.node(node).name
            )),
        }
    }
}
