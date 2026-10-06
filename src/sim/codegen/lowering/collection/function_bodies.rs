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
        self.emit_func_task_with(path, inst, ft, None)
    }

    /// Lower the bodies of the task specializations requested by call sites.
    /// Lowering one can request more (a specialization forwards its bound
    /// `ref` formals to other tasks), so drain until none remain.
    pub(in super::super) fn emit_task_specializations(&mut self) -> Result<(), String> {
        while let Some(specialization) = self.pending_specializations.pop() {
            let path = self.instance_path_of(specialization.inst);
            self.emit_func_task_with(
                &path,
                specialization.inst,
                specialization.task,
                Some(&specialization),
            )?;
        }
        Ok(())
    }

    /// The model function for `task` (a task or function) with its static
    /// `ref` formals bound to `statics` and its native record `ref` formals
    /// bound to `records`, creating it on first use. The clone keeps the
    /// template's signature: callers still pass the reference descriptors,
    /// which the body ignores for the bound formals, and null parameters for
    /// the bound records ([`IrCallArg::NativeRefBound`]).
    pub(in super::super) fn task_specialization(
        &mut self,
        task: NodeId,
        inst: NodeId,
        statics: Vec<StaticRef>,
        records: Vec<RecordRefBinding>,
    ) -> Result<usize, String> {
        let signals = statics
            .iter()
            .map(|bound| match bound.lhs {
                IrLhs::Whole(signal) => Ok(signal),
                _ => Err("task specialization requires a whole-signal actual".to_owned()),
            })
            .collect::<Result<Vec<_>, String>>()?;
        let key = (task, inst, signals, records.clone());
        if let Some(index) = self.task_specializations.get(&key) {
            return Ok(*index);
        }
        let formals = self.func_formals(task);
        let bound_native_refs = records
            .iter()
            .map(|binding| {
                formals
                    .iter()
                    .position(|(formal, _)| *formal == binding.formal)
                    .ok_or_else(|| {
                        format!(
                            "record `ref` formal `{}` is not a formal of `{}`",
                            self.node(binding.formal).name,
                            self.node(task).name
                        )
                    })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let template = self
            .func_meta
            .get(&task)
            .map(|meta| meta.ir)
            .ok_or_else(|| format!("task `{}` has no model entry", self.node(task).name))?;
        let mut function = self.model.funcs[template].clone();
        function.c_name = format!(
            "{}__spec{}",
            function.c_name,
            self.task_specializations.len()
        );
        function.inline_expanded = false;
        function.bound_native_refs = bound_native_refs;
        let ir = self.model.funcs.len();
        self.model.funcs.push(function);
        self.task_specializations.insert(key, ir);
        self.pending_specializations.push(PendingSpecialization {
            ir,
            task,
            inst,
            statics,
            records,
        });
        Ok(ir)
    }

    fn emit_func_task_with(
        &mut self,
        path: &str,
        inst: NodeId,
        ft: NodeId,
        specialization: Option<&PendingSpecialization>,
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
        let meta_ir =
            match specialization {
                Some(specialization) => specialization.ir,
                None => self.func_meta.get(&ft).map(|meta| meta.ir).ok_or_else(|| {
                    format!("function `{}` has no model entry", self.node(ft).name)
                })?,
            };
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
        let ret_var = (has_ret
            || self.nonflatten_function(ft)
            || self.record_return(ft)
            || self.native_return(ft)
            || self.container_return(ft)
            || self.real_array_return(ft))
        .then_some(ft);
        self.bind_native_function(inst, ft);
        if let Some(specialization) = specialization {
            self.bind_record_refs(&specialization.records)?;
        }
        self.bind_container_function(inst, ft);
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
            &mut locals,
            &mut chandle_locals,
            &mut process_locals,
            &mut local_seq,
            &local_prefix,
        )?;
        // Pattern variables are lexical temporaries owned by a conditional
        // predicate, so Slang does not expose them as declaration statements
        // for the ordinary local walk. Give automatic `.name` bindings the
        // same per-activation storage as source-declared integral locals.
        let pattern_targets = self.conditional_pattern_targets(body);
        // String pattern variables are activation string locals written when
        // their pattern matches, as in processes (SIM-007).
        let mut string_pattern_targets = Vec::new();
        for target in &pattern_targets {
            // A case item's pattern variable is also a declaration of the
            // ordinary local walk; a string one is still a string local.
            if matches!(self.kind(*target), NodeKind::Var { ty } if ty.kind == "string") {
                locals.remove(target);
                string_pattern_targets.push(*target);
                continue;
            }
            if locals.contains_key(target) {
                continue;
            }
            // Bindings of values beyond packed capacity (column-layout
            // records and descriptor arrays) get lexical column storage
            // where the pattern is tested.
            if self.column_binding_target(*target) {
                continue;
            }
            let ty = match self.kind(*target) {
                NodeKind::Var { ty } | NodeKind::Array { ty } => ty,
                other => {
                    return Err(format!(
                        "unsupported conditional pattern binding `{}` (node kind {other:?})",
                        self.node(*target).name
                    ));
                }
            };
            if is_real_kind(&ty.kind) {
                let name = format!("{local_prefix}_l{local_seq}");
                local_seq += 1;
                locals.insert(*target, (name, 0, false, false, ty.kind == "shortreal"));
                continue;
            }
            if is_handle_kind(&ty.kind) {
                return Err(format!(
                    "conditional pattern binding `{}` of a handle type in a subroutine is not supported",
                    self.node(*target).name
                ));
            }
            let width = self
                .fixed_value_width(*target)
                .or(ty.width)
                .ok_or_else(|| {
                    format!(
                        "conditional pattern binding `{}` has no resolved width",
                        self.node(*target).name
                    )
                })?;
            if width > LLG_MAX_WIDTH {
                return Err(format!(
                    "conditional pattern binding `{}` is {width} bits wide; the runtime supports at most {LLG_MAX_WIDTH}",
                    self.node(*target).name
                ));
            }
            let signed = self
                .query_descriptor(*target)
                .map_or(ty.signed, |descriptor| descriptor.info.signed);
            let two_state = self.db.is_two_state_type(*target) || is_two_state_kind(&ty.kind);
            let name = format!("{local_prefix}_l{local_seq}");
            local_seq += 1;
            locals.insert(*target, (name, width, signed, two_state, false));
        }
        let mut declaration_initializers = HashMap::new();
        self.collect_subroutine_decl_initializers(body, &mut declaration_initializers);
        let local_ids = sorted_node_ids(&locals);
        let chandle_local_ids = sorted_node_ids(&chandle_locals);
        let process_local_ids = sorted_node_ids(&process_locals);

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
        let mut const_ref_lhs = HashMap::new();
        let mut arg_dependencies: HashMap<NodeId, Vec<IrDependency>> = HashMap::new();
        let mut persistent = HashMap::new();
        let mut static_local_signals = HashMap::new();
        let mut chandle_read = HashMap::new();
        let mut chandle_write = HashMap::new();
        let mut process_read = HashMap::new();
        let mut process_write = HashMap::new();
        let mut string_read = HashMap::new();
        let mut string_write = HashMap::new();
        let mut string_addr = HashMap::new();
        let mut static_input_copies = Vec::new();
        let mut shared_outputs = Vec::new();
        let mut callback_private_formal_copies = Vec::new();
        let mut event_args: HashMap<NodeId, IrEventRef> = HashMap::new();

        // A static fixed-value local has persistent storage for the whole
        // simulation. Model it as an ordinary hidden signal so function-body
        // accesses and legal hierarchical continuous assignments share the
        // same typed storage. Automatic locals and native-object locals keep
        // their existing activation/subprogram storage.
        for local in local_ids.iter().copied() {
            let (_, width, signed, two_state, shortreal) = &locals[&local];
            if self.db.variable_lifetime(local) != VariableLifetime::Static {
                continue;
            }
            let (NodeKind::Var { ty } | NodeKind::Array { ty }) = self.kind(local) else {
                continue;
            };
            if ty.kind == "string" || is_handle_kind(&ty.kind) || ty.kind == "event" {
                continue;
            }
            if let Some(info) = self.static_task_locals.get(&(inst, local)).cloned() {
                self.insert_sig_global(local, info.clone());
                persistent.insert(local, info.clone());
                static_local_signals.insert(local, info);
                continue;
            }
            let real = is_real_kind(&ty.kind);
            let info = SignalInfo {
                global: format!("S_f{}_l{}", inst.index(), local.index()),
                width: *width,
                signed: *signed,
                two_state: *two_state,
                real,
                shortreal: *shortreal,
                net_driver: None,
                ir: self.model.signals.len(),
            };
            self.model.signals.push(IrSignal {
                fixed_default: self.fixed_default_literal(local),
                c_name: info.global.clone(),
                hdl_name: None,
                ty: if real {
                    IrType::Real {
                        shortreal: info.shortreal,
                    }
                } else {
                    IrType::Packed {
                        width: info.width,
                        signed: info.signed,
                        two_state: info.two_state,
                    }
                },
                net_driver: None,
                net_alias: Vec::new(),
                alias: None,
                omit: false,
            });
            self.signals.push(info.clone());
            self.insert_sig_global(local, info.clone());
            persistent.insert(local, info.clone());
            static_local_signals.insert(local, info);
        }
        // A statically allocated numeric function result normally uses the
        // emitter's persistent `_ret` cell. If hierarchy exposes that result
        // as an assignment target or a user-defined `ref` actual, register
        // the same storage as a hidden signal so reads and writes share one
        // owner.
        let static_return_signal = if !automatic {
            let return_variable = self
                .node(ft)
                .children
                .iter()
                .copied()
                .find(|child| matches!(self.kind(*child), NodeKind::Var { .. }))
                .filter(|variable| {
                    self.db.variable_lifetime(*variable) == VariableLifetime::Static
                        && self.static_return_requires_signal(*variable)
                });
            match (return_variable, ret) {
                (Some(return_variable), Some((width, signed, two_state, shortreal))) => {
                    let real = width == 0;
                    let info = SignalInfo {
                        // Reuse the persistent return cell's emitted name;
                        // the typed IR link prevents a second declaration.
                        global: format!("_llg_ret_{meta_ir}"),
                        width,
                        signed,
                        two_state,
                        real,
                        shortreal,
                        net_driver: None,
                        ir: self.model.signals.len(),
                    };
                    self.model.signals.push(IrSignal {
                        fixed_default: self
                            .fixed_default_literal(return_variable)
                            .or_else(|| self.fixed_default_literal(ft)),
                        c_name: info.global.clone(),
                        hdl_name: None,
                        ty: if real {
                            IrType::Real { shortreal }
                        } else {
                            IrType::Packed {
                                width,
                                signed,
                                two_state,
                            }
                        },
                        net_driver: None,
                        net_alias: Vec::new(),
                        alias: None,
                        omit: false,
                    });
                    self.signals.push(info.clone());
                    self.insert_sig_global(return_variable, info.clone());
                    persistent.insert(return_variable, info.clone());
                    persistent.insert(ft, info.clone());
                    Some(info)
                }
                _ => None,
            }
        } else {
            None
        };
        for local in chandle_local_ids.iter().copied() {
            let name = &chandle_locals[&local];
            if self.db.variable_lifetime(local) == VariableLifetime::Static {
                let object = if let Some(object) =
                    self.static_task_chandle_locals.get(&(inst, local)).copied()
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
                        .insert((inst, local), object);
                    object
                };
                // Mailboxes use the same native pointer storage as other
                // handles, but their declaration initializer must construct
                // a runtime mailbox before the first function call.  Queue
                // it with the other model-time mailbox initializers instead
                // of silently dropping it with ordinary static handles.
                if self.is_mailbox_expr(path, local) {
                    if let Some(initializer) = self.db.var_initializer(local) {
                        self.mailbox_object_initializers.push((
                            local,
                            object,
                            initializer,
                            path.to_owned(),
                        ));
                    }
                }
                chandle_read.insert(local, IrChandleExpr::Read(object));
                chandle_write.insert(local, ChandleTarget::Object(object));
            } else {
                chandle_read.insert(local, IrChandleExpr::LocalRead(name.clone()));
                chandle_write.insert(local, ChandleTarget::Local(name.clone()));
            }
        }
        for local in process_local_ids {
            let name = &process_locals[&local];
            if self.fork_shared(local) {
                // Fork branches share it through an opaque frame slot
                // (`ChandleDeclareShared`), which holds pinned identities.
                chandle_read.insert(local, IrChandleExpr::LocalRead(name.clone()));
                chandle_write.insert(local, ChandleTarget::Local(name.clone()));
                continue;
            }
            process_read.insert(
                local,
                crate::sim::ir::IrProcessExpr::LocalRead(name.clone()),
            );
            process_write.insert(local, ProcessTarget::Local(name.clone()));
        }
        for local in local_ids.iter().copied() {
            let (name, ..) = &locals[&local];
            if matches!(self.kind(local), NodeKind::Var { ty } if ty.kind == "string") {
                string_read.insert(local, IrStringExpr::LocalRead(name.clone()));
                string_write.insert(local, name.clone());
                string_addr.insert(local, name.clone());
            }
        }
        let event_formals = self.event_read_formals(body);
        for (idx, (io, is_out)) in formals.iter().enumerate() {
            // A container formal that a fork branch names is copied into a
            // shared frame at entry (SV 9.3.2, 13.3); see
            // `IrFormal::shared_local`.
            if automatic && self.is_subroutine_container(*io) && self.fork_shared(*io) {
                if let Some(container) = self.container_globals.get(io).map(|info| info.ir) {
                    self.shared_locals.insert(*io);
                    shared_outputs.push((
                        idx,
                        crate::sim::ir::shared_container_capture_name(container),
                    ));
                }
            }
            if self.fixed_formal_array(*io).is_some()
                || self.is_native_declaration(*io)
                || self.is_subroutine_container(*io)
                || self.real_formal_array(*io).is_some()
            {
                continue;
            }
            if matches!(self.kind(*io), NodeKind::FuncArg { ty, .. } if ty.kind == "event") {
                // A by-value input event formal is the activation's own
                // handle to the object the caller named; output, inout and
                // ref formals bind the caller's handle storage
                // (`IrCallArg::EventAddr`).
                event_args.insert(*io, IrEventRef::Formal(idx));
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
                if !is_ref
                    && !*is_out
                    && automatic
                    && self.is_process_formal(*io)
                    && self.fork_shared(*io)
                {
                    // Shared with fork branches through an opaque frame slot,
                    // which holds a pinned identity (plain handle storage).
                    let name = format!("_llg_shared_formal_{idx}");
                    self.shared_locals.insert(*io);
                    static_input_copies.push(IrStmt::Object(Box::new(
                        crate::sim::ir::IrObjectStmt::ChandleDeclareShared(
                            name.clone(),
                            Some(IrChandleExpr::PinnedProcess(Box::new(
                                crate::sim::ir::IrProcessExpr::FormalRead(idx),
                            ))),
                        ),
                    )));
                    chandle_read.insert(*io, IrChandleExpr::LocalRead(name.clone()));
                    chandle_write.insert(*io, ChandleTarget::Local(name));
                    continue;
                }
                if !is_ref && !*is_out && automatic && self.is_process_formal(*io) {
                    // A by-value process formal borrows the caller's counted
                    // handle for the activation; reads retain their copies.
                    process_read.insert(*io, crate::sim::ir::IrProcessExpr::FormalRead(idx));
                    chandle_read.insert(*io, IrChandleExpr::FormalRead(idx));
                    continue;
                }
                if is_ref && self.is_process_formal(*io) {
                    // A `ref` process formal aliases the caller's counted
                    // slot (see `process_formal_binding`): writes retain.
                    process_read.insert(
                        *io,
                        crate::sim::ir::IrProcessExpr::Handle(Box::new(IrChandleExpr::FormalRead(
                            idx,
                        ))),
                    );
                    chandle_read.insert(*io, IrChandleExpr::FormalRead(idx));
                    if !const_ref {
                        process_write.insert(*io, ProcessTarget::Local(format!("*r{idx}")));
                    }
                    continue;
                }
                if !is_ref && !*is_out && automatic && self.fork_shared(*io) {
                    // Shared with a fork branch like a handle local (SV 9.3.2,
                    // 13.3): copied into an opaque frame slot at entry.
                    let name = format!("_llg_shared_formal_{idx}");
                    self.shared_locals.insert(*io);
                    static_input_copies.push(IrStmt::Object(Box::new(
                        crate::sim::ir::IrObjectStmt::ChandleDeclareShared(
                            name.clone(),
                            Some(IrChandleExpr::FormalRead(idx)),
                        ),
                    )));
                    chandle_read.insert(*io, IrChandleExpr::LocalRead(name.clone()));
                    chandle_write.insert(*io, ChandleTarget::Local(name));
                    continue;
                }
                if !is_ref && !*is_out {
                    if let Some(object) = (!automatic)
                        .then(|| self.static_chandle_formals.get(&(inst, *io)).copied())
                        .flatten()
                    {
                        chandle_read.insert(*io, IrChandleExpr::Read(object));
                        chandle_write.insert(*io, ChandleTarget::Object(object));
                        // A process formal's copy outlives the call in the
                        // static object, so it holds a pinned identity.
                        let value = if self.is_process_formal(*io) {
                            IrChandleExpr::PinnedProcess(Box::new(
                                crate::sim::ir::IrProcessExpr::FormalRead(idx),
                            ))
                        } else {
                            IrChandleExpr::FormalRead(idx)
                        };
                        static_input_copies.push(IrStmt::Object(Box::new(
                            crate::sim::ir::IrObjectStmt::ChandleAssign(object, value),
                        )));
                        continue;
                    }
                }
                {
                    // An event evaluator copies a by-value handle formal from
                    // the activation local the callee binds it to.
                    chandle_read.insert(
                        *io,
                        if !is_ref && !*is_out && event_formals.contains(io) {
                            IrChandleExpr::LocalRead(format!("a{idx}"))
                        } else {
                            IrChandleExpr::FormalRead(idx)
                        },
                    );
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
                        static_input_copies.push(IrStmt::Object(Box::new(
                            IrObjectStmt::StringAssign(object, IrStringExpr::FormalRead(idx)),
                        )));
                    }
                } else if automatic && *is_out && self.fork_shared(*io) {
                    // A shared string output: the emitter creates the cell
                    // from `*o{idx}` and copies it back at return.
                    let name = format!("_llg_shared_formal_{idx}");
                    locals.insert(*io, (name.clone(), 0, false, true, false));
                    self.shared_locals.insert(*io);
                    string_read.insert(*io, IrStringExpr::LocalRead(name.clone()));
                    string_write.insert(*io, name.clone());
                    string_addr.insert(*io, name.clone());
                    shared_outputs.push((idx, name));
                } else if automatic && !*is_out && self.fork_shared(*io) {
                    // Shared with a detached fork branch like a packed input
                    // formal (SV 9.3.2, 13.3).
                    let name = format!("_llg_shared_formal_{idx}");
                    locals.insert(*io, (name.clone(), 0, false, true, false));
                    self.shared_locals.insert(*io);
                    static_input_copies.push(IrStmt::SharedString {
                        name: name.clone(),
                        init: Some(IrStringExpr::FormalRead(idx)),
                    });
                    string_read.insert(*io, IrStringExpr::LocalRead(name.clone()));
                    string_write.insert(*io, name.clone());
                    string_addr.insert(*io, name);
                } else {
                    // An event evaluator copies a by-value string formal from the
                    // activation local the callee binds it to.
                    string_read.insert(
                        *io,
                        if !*is_out && event_formals.contains(io) {
                            IrStringExpr::LocalRead(format!("a{idx}"))
                        } else {
                            IrStringExpr::FormalRead(idx)
                        },
                    );
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
                    } else if let Some(w) = self.formal_value_width(*io) {
                        if w > LLG_MAX_WIDTH {
                            return Err(format!(
                                "formal `{}` of `{c_name}` is {w} bits wide; the runtime \
                             maximum supported width is {LLG_MAX_WIDTH}",
                                self.node(*io).name
                            ));
                        }
                        (
                            w,
                            ty.signed,
                            self.db.is_two_state_type(*io) || is_two_state_kind(&ty.kind),
                            false,
                            false,
                        )
                    } else {
                        match ty.width {
                            Some(w) if w <= LLG_MAX_WIDTH => (
                                w,
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
                                return Err(match self.unrepresented_storage_reason(*io) {
                                    Some(reason) => format!(
                                        "formal `{}` of `{c_name}`: {reason}",
                                        self.node(*io).name
                                    ),
                                    None => format!(
                                        "formal `{}` of `{c_name}` has no width",
                                        self.node(*io).name
                                    ),
                                })
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
                    callback_private_formal_copies.push((storage.ir, idx));
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
            } else if automatic && *is_out && self.fork_shared(*io) {
                // An output or inout formal shared with a detached branch
                // starts from the caller's value cell, lives in a shared
                // frame cell during the activation and is copied back to
                // `o{idx}` when the subroutine returns (SV 13.3, 9.3.2). The
                // emitter creates the cell (`IrFormal::shared_local`).
                let name = format!("_llg_shared_formal_{idx}");
                let shortreal = matches!(
                    self.kind(*io),
                    NodeKind::FuncArg { ty, .. } if ty.kind == "shortreal"
                );
                locals.insert(*io, (name.clone(), w, s, two_state, shortreal));
                self.shared_locals.insert(*io);
                shared_outputs.push((idx, name));
                arg_read.insert(
                    *io,
                    ArgMap {
                        width: w,
                        signed: s,
                        two_state,
                    },
                );
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
            } else if automatic && self.fork_shared(*io) {
                // An input formal that a detached fork branch names is one
                // variable shared with the branch (SV 9.3.2, 13.3): the
                // activation copies the argument into a shared frame cell at
                // entry and the body uses it as a local.
                let name = format!("_llg_shared_formal_{idx}");
                let shortreal = matches!(
                    self.kind(*io),
                    NodeKind::FuncArg { ty, .. } if ty.kind == "shortreal"
                );
                locals.insert(*io, (name.clone(), w, s, two_state, shortreal));
                self.shared_locals.insert(*io);
                static_input_copies.push(IrStmt::SharedLocal {
                    name,
                    width: w,
                    signed: s,
                    two_state,
                    init: Some(Box::new(formal_read_expr(idx, w, s))),
                });
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
        // A specialization reads and writes the bound actual directly, exactly
        // as an expansion does, and waits on its dependencies.
        for bound in specialization.iter().flat_map(|spec| &spec.statics) {
            arg_ir.insert(bound.formal, bound.read.clone());
            arg_dependencies.insert(bound.formal, bound.dependencies.clone());
            if bound.const_ref {
                const_ref_lhs.insert(bound.formal, Lhs::Canonical(bound.lhs.clone()));
            } else {
                arg_lhs.insert(bound.formal, Lhs::Canonical(bound.lhs.clone()));
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
            event_args,
            arg_dependencies,
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
        let pattern_decls = pattern_targets
            .iter()
            .filter(|target| self.db.variable_lifetime(**target) == VariableLifetime::Automatic)
            .filter_map(|target| locals.get(target))
            .map(
                |(name, width, signed, two_state, _shortreal)| IrStmt::DeclLocal {
                    name: name.clone(),
                    width: *width,
                    signed: *signed,
                    two_state: *two_state,
                    // A real pattern variable (width 0) starts at 0.0.
                    init: (*width == 0).then(|| Box::new(real_literal_expr(0.0))),
                },
            )
            .collect::<Vec<_>>();
        let mut pattern_decls = pattern_decls;
        for target in string_pattern_targets {
            pattern_decls.push(IrStmt::DeclString {
                name: self.pattern_string_local(target),
                init: None,
            });
        }
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
            let mut body_stmts = pattern_decls;
            body_stmts.extend(static_input_copies);
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
        for local in local_ids.iter().copied() {
            let (c_name, width, signed, two_state, shortreal) = &locals[&local];
            if self.db.variable_lifetime(local) != VariableLifetime::Static {
                continue;
            }
            // A specialization only exists for a task that waits, whose
            // persistent locals are model signals initialized once with their
            // declaration; it must not initialize them again.
            if self.static_task_locals.contains_key(&(inst, local)) || specialization.is_some() {
                continue;
            }
            let initializer = self
                .db
                .var_initializer(local)
                .or_else(|| self.db.array_meta(local).and_then(|array| array.init))
                .or_else(|| declaration_initializers.get(&local).copied());
            let Some(initializer) = initializer else {
                continue;
            };
            let initialization = self
                .lower_declaration_initializer(
                    path,
                    local,
                    initializer,
                    static_local_signals.get(&local).map_or_else(
                        || IrInitTarget::StaticLocal {
                            function: meta_ir,
                            name: c_name.clone(),
                        },
                        |storage| IrInitTarget::Signal(storage.ir),
                    ),
                    *width,
                    *signed,
                    *two_state,
                    *shortreal,
                )
                .map_err(|cause| {
                    format!(
                        "static subprogram initializer for `{}` cannot be lowered: {cause}",
                        self.node(local).name
                    )
                })?;
            declaration_initializations.push(initialization);
        }
        self.declaration_inits.extend(declaration_initializations);
        // Static descriptor-backed locals have persistent storage prepared by
        // `prepare_fixed_function`; their initializers use the module-array
        // descriptor transport and run once in the static schedule.
        if specialization.is_none() {
            self.queue_static_descriptor_initializers(body);
        }
        // Restore the process-level context for whatever is lowered next
        // (continuous assignments, processes).
        if let Some(specialization) = specialization {
            self.unbind_record_refs(&specialization.records);
        }
        self.func = None;
        self.cur_fn_ir = None;
        self.depth_arg = "0".to_string();

        let ir_locals = {
            let mut emitted = HashSet::new();
            local_ids
                .into_iter()
                .filter(|local| self.db.variable_lifetime(*local) == VariableLifetime::Static)
                .filter(|local| !static_local_signals.contains_key(local))
                .filter_map(|local| {
                    let (c_name, width, signed, two_state, shortreal) = &locals[&local];
                    emitted.insert(c_name.clone()).then(|| crate::sim::ir::IrLocal {
                        fixed_default: self.fixed_default_literal(local),
                        c_name: c_name.clone(),
                        width: *width,
                        signed: *signed,
                        two_state: *two_state,
                        real: *width == 0,
                        shortreal: *shortreal,
                        string: matches!(self.kind(local), NodeKind::Var { ty } if ty.kind == "string"),
                        initial: None,
                    })
                })
                .collect::<Vec<_>>()
        };
        // Static local signals are persistent observable state. Keep such a
        // function out of private evaluator callbacks even though its
        // storage is no longer represented by `IrFunc.locals`.
        let callback_return_independent = automatic
            || ret.is_none()
            || (static_local_signals.is_empty()
                && static_return_signal.is_none()
                && self.static_return_is_callback_independent(body, ft));
        let no_entry = format!("function `{}` has no model entry", self.node(ft).name);
        let entry = self.model.funcs.get_mut(meta_ir).ok_or(no_entry)?;
        entry.locals = ir_locals;
        entry.automatic = automatic;
        entry.return_signal = static_return_signal.map(|storage| storage.ir);
        entry.callback_private_formal_copies = callback_private_formal_copies;
        entry.callback_return_independent = callback_return_independent;
        entry.pre_fns = pre_fns;
        entry.body = body_stmts;
        for (index, name) in shared_outputs {
            if let Some(formal) = entry.formals.get_mut(index) {
                formal.shared_local = Some(name);
            }
        }
        let _ = (guard, decl, has_ret, ret_x, c_name.as_str());
        Ok(())
    }

    fn static_return_requires_signal(&self, return_variable: NodeId) -> bool {
        self.design_nodes()
            .into_iter()
            .any(|node| match self.kind(node) {
                NodeKind::ContAssign { .. } => {
                    self.node(node).children.first().is_some_and(|lhs| {
                        self.lhs_references_return_variable(*lhs, return_variable)
                    })
                }
                NodeKind::Stmt(StmtKind::Assign { .. }) => {
                    self.node(node).children.first().is_some_and(|lhs| {
                        self.hierarchical_lhs_references_return_variable(*lhs, return_variable)
                    })
                }
                NodeKind::FuncCall {
                    name,
                    is_task,
                    callee,
                    ..
                } => self.call_passes_hierarchical_return_by_ref(
                    node,
                    name,
                    *is_task,
                    *callee,
                    return_variable,
                ),
                _ => false,
            })
    }

    fn call_passes_hierarchical_return_by_ref(
        &self,
        call: NodeId,
        name: &str,
        is_task: bool,
        callee: Option<NodeId>,
        return_variable: NodeId,
    ) -> bool {
        let Some(caller_inst) = self.owning_inst(call) else {
            return false;
        };
        let Ok((callee, _)) = self.resolve_callee_env(caller_inst, name, is_task, callee) else {
            return false;
        };
        let arguments = self.call_argument_nodes(call);
        self.func_formals(callee)
            .iter()
            .zip(arguments)
            .any(|((formal, _), actual)| {
                matches!(
                    self.kind(*formal),
                    NodeKind::FuncArg {
                        direction: DbDirection::Ref,
                        ..
                    }
                ) && self.hierarchical_lhs_references_return_variable(actual, return_variable)
            })
    }

    fn hierarchical_lhs_references_return_variable(
        &self,
        node: NodeId,
        return_variable: NodeId,
    ) -> bool {
        match self.kind(node) {
            NodeKind::Expr(ExprKind::HierPath { refs, .. })
                if refs
                    .iter()
                    .flatten()
                    .any(|target| *target == return_variable) =>
            {
                true
            }
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) if *target == return_variable
                && self.db.semantic_detail(node) == Some("HierarchicalValue") =>
            {
                true
            }
            _ => self.node(node).children.iter().any(|child| {
                self.hierarchical_lhs_references_return_variable(*child, return_variable)
            }),
        }
    }

    fn lhs_references_return_variable(&self, node: NodeId, return_variable: NodeId) -> bool {
        match self.kind(node) {
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) => *target == return_variable,
            NodeKind::Expr(ExprKind::HierPath { refs, .. })
                if refs
                    .iter()
                    .flatten()
                    .any(|target| *target == return_variable) =>
            {
                true
            }
            _ => self
                .node(node)
                .children
                .iter()
                .any(|child| self.lhs_references_return_variable(*child, return_variable)),
        }
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
        if let NodeKind::Var { ty } | NodeKind::Array { ty } = self.kind(node) {
            if self.node(node).parent.is_none() {
                // A detached member declaration reached through a pattern
                // key or a select names a record member, not a local.
                return Ok(());
            }
            if self.is_native_declaration(node) || self.record_declaration(node) {
                // Native record locals are descriptor-backed values declared
                // by `NativeValueDeclare` (see `native_values`); column-layout
                // record locals are declared column by column.
                return Ok(());
            }
            if self.is_subroutine_container(node) {
                // Container locals are declared by `IrContainerStmt::Declare`
                // or persist as static containers (`subroutine_containers`).
                return Ok(());
            }
            if self.array_globals.get(&node).is_some_and(|array| {
                self.model.arrays[array.ir].sparse() || self.model.arrays[array.ir].real
            }) {
                return Ok(());
            }
            self.explicit_local_lifetime(node)?;
            // An automatic process handle is a counted local; a static one
            // is plain handle storage holding pinned identities.
            if ty.kind == "class"
                && ty.type_name.as_deref() == Some("process")
                && self.db.variable_lifetime(node) == VariableLifetime::Automatic
            {
                process_locals.entry(node).or_insert_with(|| {
                    let cname = format!("{prefix}_p{seq}");
                    *seq += 1;
                    cname
                });
                return Ok(());
            }
            if is_handle_kind(&ty.kind) {
                chandle_locals.entry(node).or_insert_with(|| {
                    let cname = format!("{prefix}_l{seq}");
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
                    match self.fixed_value_width(node).or(ty.width) {
                        Some(w) if w <= LLG_MAX_WIDTH => w,
                        Some(w) => {
                            return Err(format!(
                                "local `{}` is {w} bits wide; the runtime supports at most \
                         {LLG_MAX_WIDTH}",
                                self.node(node).name
                            ))
                        }
                        None => {
                            return Err(match self.unrepresented_storage_reason(node) {
                                Some(reason) => {
                                    format!("local `{}`: {reason}", self.node(node).name)
                                }
                                None => format!("local `{}` has no width", self.node(node).name),
                            })
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
                    if w == 0 {
                        false
                    } else {
                        self.query_descriptor(node)
                            .map_or(ty.signed, |descriptor| descriptor.info.signed)
                    },
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
            self.collect_func_locals(*c, locals, chandle_locals, process_locals, seq, prefix)?;
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
