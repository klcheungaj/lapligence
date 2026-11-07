//! Forks.

use super::*;

/// How a `join` branch reaches a `ref` formal of its subroutine.
#[derive(Clone, Copy, PartialEq, Eq)]
enum RefFormalBinding {
    /// Through the formal's descriptor parameter `rN`.
    Descriptor(usize),
    /// Through a handle formal's descriptor parameter `rN` and the actual's
    /// change marker parameter.
    Handle(usize),
    /// Through the actual a specialization binds statically.
    Actual,
    /// A native record formal: its value and companion parameters, then its
    /// change marker when it has one, in `.1` slots.
    Record(usize, usize),
}

impl EmitCtx<'_, '_> {
    /// Lower one `fork … join` site.  Each branch becomes its own coroutine
    /// function attached to the enclosing process's pre-functions; nested
    /// constructs inside a branch append their own pre-functions first,
    /// mirroring the pre-IR emission order.
    ///
    /// Detached branches may retain automatic subroutine storage through an
    /// activation frame.  A synchronous function can only use `join_none`.
    pub(super) fn lower_fork(
        &mut self,
        target: Option<NodeId>,
        join_kind: DbJoinKind,
        branches: &[NodeId],
    ) -> Result<Vec<IrStmt>, String> {
        // Declarations of the fork's own block items (`fork automatic int
        // k = i; ... join_none`) arrive among the branch statements. They are
        // not processes: the process executing the fork statement creates and
        // initializes them before any branch starts (SV 9.3.2), and the
        // branches then capture them like enclosing automatics.
        let mut declarations = Vec::new();
        let mut processes = Vec::with_capacity(branches.len());
        for branch in branches {
            if let NodeKind::Stmt(StmtKind::VariableDecl { declaration }) = self.cg.kind(*branch) {
                let declaration = *declaration;
                declarations.extend(self.lower_variable_decl(declaration)?);
            } else {
                processes.push(*branch);
            }
        }
        let detached = self.func.as_ref().is_some_and(|function| !function.is_task);
        let outer = self.cg.detached_function_branch;
        self.cg.detached_function_branch = outer || detached;
        let statements = self.lower_fork_branches(target, join_kind, &processes);
        self.cg.detached_function_branch = outer;
        let mut statements = statements?;
        if !declarations.is_empty() {
            declarations.append(&mut statements);
            statements = declarations;
        }
        Ok(statements)
    }

    fn lower_fork_branches(
        &mut self,
        target: Option<NodeId>,
        join_kind: DbJoinKind,
        branches: &[NodeId],
    ) -> Result<Vec<IrStmt>, String> {
        if let Some(function) = &self.func {
            if !function.is_task && join_kind != DbJoinKind::None {
                return Err(format!(
                    "blocking fork/join in function `{}` is not allowed; use join_none",
                    self.path
                ));
            }
        }
        if self.in_final {
            // A join suspends the process; a final block may not suspend
            // (LRM 1800-2005 §10.7).
            return Err(format!(
                "fork/join inside a final block in `{}` is not allowed \
                 (no timing controls or waits in final)",
                self.path
            ));
        }
        let join = match join_kind {
            DbJoinKind::All => IrJoinKind::Join,
            DbJoinKind::None => IrJoinKind::None,
            DbJoinKind::Any => IrJoinKind::Any,
            k => {
                return Err(format!(
                    "unsupported fork join type {k:?} in `{}`",
                    self.path
                ))
            }
        };
        let target = target
            .map(|target| self.cg.activation_target(target))
            .transpose()?;
        if let Some(name) = branches
            .iter()
            .find_map(|branch| self.cg.native_activation_capture(*branch))
        {
            return Err(format!(
                "fork branch in `{}` references native record or container `{name}` of the enclosing activation; capturing native subroutine storage in a fork is not supported (SIM-010)",
                self.path
            ));
        }
        let capture_targets = branches
            .iter()
            .map(|branch| self.cg.fork_capture_targets(*branch))
            .collect::<Vec<_>>();
        self.check_ref_formal_captures(join, branches, &capture_targets)?;
        // A class method's branches read `this`; the receiver cannot be
        // reassigned, so each branch copies the handle. This includes the
        // branches of a fork nested in a detached method branch.
        let this_receiver = match self.cg.method_receiver() {
            Some(IrChandleExpr::LocalRead(name)) => Some(name),
            _ => None,
        };
        let has_captures =
            this_receiver.is_some() || capture_targets.iter().any(|targets| !targets.is_empty());
        let enclosing_func = self.func.clone();
        let detached_function_branch = enclosing_func
            .as_ref()
            .is_some_and(|function| !function.is_task && function.def_node.is_some());
        let process_kind = self.process_kind;
        let mut names = Vec::with_capacity(branches.len());
        let mut captured_branches = Vec::with_capacity(branches.len());
        for (k, branch) in branches.iter().enumerate() {
            let fn_name = format!("{}_b{}", self.cg.new_fn_name(&self.path, "fork"), k);
            let label = format!("{}.br{k}", self.cg.source_path(&self.path));
            if !has_captures {
                // Branches live in the same instance scope: same path, same
                // owning instance; refs resolve to the instance globals.
                let saved_cg_func = self.cg.func.clone();
                let saved_cg_depth = self.cg.depth_arg.clone();
                let (body_result, mut nested_pre_fns) = {
                    let mut bctx = EmitCtx::new(
                        self.cg,
                        self.path.clone(),
                        self.inst,
                        "0",
                        (!detached_function_branch)
                            .then(|| enclosing_func.clone())
                            .flatten(),
                        false,
                    );
                    bctx.process_kind = process_kind;
                    let body_result = bctx.lower_stmt(*branch);
                    (body_result, std::mem::take(&mut bctx.pre_fns))
                };
                self.cg.func = saved_cg_func;
                self.cg.depth_arg = saved_cg_depth;
                let body = body_result?;
                self.pre_fns.append(&mut nested_pre_fns);
                self.pre_fns.push(crate::sim::ir::IrPreFn::Branch {
                    c_name: fn_name.clone(),
                    body,
                });
                names.push((fn_name, label));
                continue;
            }

            let frame = self.cg.new_frame_id()?;
            let previous_captures = self.cg.capture_locals.clone();
            let mut captures = Vec::with_capacity(capture_targets[k].len());
            for target in &capture_targets[k] {
                // Slots stay dense: a statically bound `ref` formal takes none.
                let slot = captures.len();
                // A `ref` formal reaches a `join` branch as its descriptor:
                // the branch names the formal exactly as the subroutine
                // does, so its reads, writes and waits act on the actual
                // (SV 13.5.2). The subroutine waits for every branch, so the
                // descriptor outlives them.
                if join == IrJoinKind::Join {
                    let binding = self.ref_formal_descriptor(*target);
                    // A statically bound formal names the actual directly.
                    if binding == Some(RefFormalBinding::Actual) {
                        continue;
                    }
                    if let Some(RefFormalBinding::Record(index, slots)) = binding {
                        for offset in 0..slots {
                            let storage = StorageRef::new(
                                frame,
                                (slot + offset) as u32,
                                StorageLifetime::Automatic,
                                StorageOwnership::Owned,
                            )
                            .with_kind(StorageKind::Opaque);
                            captures.push(IrCapture::new(
                                storage,
                                IrExpr::new(IrExprKind::FormalRead(index), 1, false, None),
                            ));
                        }
                        continue;
                    }
                    if let Some(
                        RefFormalBinding::Descriptor(index) | RefFormalBinding::Handle(index),
                    ) = binding
                    {
                        // A handle formal passes its change marker in a
                        // second slot right after the descriptor's, so
                        // branch stores publish it and waits observe it.
                        let slots = if binding == Some(RefFormalBinding::Handle(index)) {
                            2
                        } else {
                            1
                        };
                        for offset in 0..slots {
                            let storage = StorageRef::new(
                                frame,
                                (slot + offset) as u32,
                                StorageLifetime::Automatic,
                                StorageOwnership::Owned,
                            )
                            .with_kind(StorageKind::Opaque);
                            captures.push(IrCapture::new(
                                storage,
                                IrExpr::new(IrExprKind::FormalRead(index), 1, false, None),
                            ));
                        }
                        continue;
                    }
                }
                let source = self.cg.capture_source(*target).ok_or_else(|| {
                    format!(
                        "automatic declaration `{}` was not collected before fork capture in `{}`",
                        self.cg.node(*target).name,
                        self.path
                    )
                })?;
                let initial = source.initial;
                let storage = StorageRef::for_declaration(
                    frame,
                    slot as u32,
                    self.cg.declaration_identity(*target)?,
                    source.lifetime,
                    if join == IrJoinKind::Join
                        && source.lifetime == StorageLifetime::Automatic
                        && !matches!(
                            source.kind,
                            StorageKind::Opaque
                                | StorageKind::String
                                | StorageKind::Container
                                | StorageKind::Native
                        )
                        && !matches!(
                            self.cg.kind(*target),
                            NodeKind::FuncArg {
                                direction: DbDirection::Ref,
                                ..
                            }
                        )
                        // A detached fork inside the branch can outlive it
                        // and aliases its cell through the branch's frame.
                        && !(self.cg.shared_locals.contains(target)
                            && self.cg.holds_detached_fork(*branch))
                    {
                        StorageOwnership::Borrowed
                    } else if self.cg.shared_locals.contains(target)
                        && source.lifetime == StorageLifetime::Automatic
                        && matches!(
                            source.kind,
                            StorageKind::Packed
                                | StorageKind::Real
                                | StorageKind::String
                                | StorageKind::Container
                                | StorageKind::Native
                                | StorageKind::Opaque
                        )
                    {
                        // A detached branch shares the declaring frame's cell.
                        StorageOwnership::Shared
                    } else {
                        StorageOwnership::Owned
                    },
                )
                .with_kind(source.kind);
                // Selected accesses in the branch name the variable by the
                // declaring subroutine's local, which a nested branch's
                // initializer (the outer capture) does not carry.
                let capture_name = Codegen::capture_local_name(storage);
                let alias = self
                    .cg
                    .func
                    .as_ref()
                    .and_then(|func| func.locals.get(target))
                    .map(|(name, ..)| name.clone())
                    .or_else(|| self.cg.proc_local_info(*target).map(|info| info.c_name.clone()))
                    .filter(|alias| {
                        *alias != capture_name
                            && !matches!(initial.kind(), IrExprKind::LocalRead(local) if local == alias)
                    });
                let local = ProcLocalInfo {
                    c_name: Codegen::capture_local_name(storage),
                    width: source.info.width,
                    signed: source.info.signed,
                    two_state: source.info.two_state,
                    static_signal: None,
                };
                self.cg
                    .capture_locals
                    .insert(*target, CaptureBinding { storage, local });
                captures.push(IrCapture::new(storage, initial).with_alias(alias));
            }
            if let Some(receiver) = &this_receiver {
                let storage = StorageRef::new(
                    frame,
                    captures.len() as u32,
                    StorageLifetime::Automatic,
                    StorageOwnership::Owned,
                )
                .with_kind(StorageKind::Opaque);
                captures.push(IrCapture::new(
                    storage,
                    IrExpr::new(
                        IrExprKind::ObjectQuery(Box::new(IrObjectQuery::HandleCapture(
                            IrChandleExpr::LocalRead(receiver.clone()),
                        ))),
                        1,
                        false,
                        None,
                    ),
                ));
            }

            let saved_cg_func = self.cg.func.clone();
            let saved_cg_depth = self.cg.depth_arg.clone();
            // A detached branch has no subroutine context; it names the
            // receiver it captured above (bound to the same local name).
            let saved_branch_receiver = std::mem::replace(
                &mut self.cg.fork_branch_receiver,
                this_receiver.clone().map(IrChandleExpr::LocalRead),
            );
            let (body_result, mut nested_pre_fns) = {
                let mut bctx = EmitCtx::new(
                    self.cg,
                    self.path.clone(),
                    self.inst,
                    "0",
                    (!detached_function_branch)
                        .then(|| enclosing_func.clone())
                        .flatten(),
                    false,
                );
                bctx.process_kind = process_kind;
                let body_result = bctx.lower_stmt(*branch);
                (body_result, std::mem::take(&mut bctx.pre_fns))
            };
            self.cg.func = saved_cg_func;
            self.cg.depth_arg = saved_cg_depth;
            self.cg.fork_branch_receiver = saved_branch_receiver;
            self.cg.capture_locals = previous_captures;
            let body = body_result?;
            self.pre_fns.append(&mut nested_pre_fns);
            self.pre_fns.push(crate::sim::ir::IrPreFn::CapturedBranch {
                c_name: fn_name.clone(),
                frame,
                captures: captures.clone(),
                body,
            });
            names.push((fn_name, label));
            let (branch_name, branch_label) = names
                .last()
                .cloned()
                .expect("captured branch name was just pushed");
            captured_branches.push(IrCapturedBranch::new(
                branch_name,
                branch_label,
                frame,
                captures,
            ));
        }
        // A fork is a wait even for join_none: a wait-free `always` containing
        // `fork … join_none` must not be wrapped as a comb process (its
        // branches are child coroutines, not combinational re-evaluation).
        self.saw_wait = true;
        if has_captures {
            Ok(vec![IrStmt::CapturedFork {
                join_kind: join,
                branches: captured_branches,
                target,
            }])
        } else {
            Ok(vec![IrStmt::Fork {
                join_kind: join,
                branches: names,
                target,
            }])
        }
    }

    /// The index of `ref` formal `target` of the enclosing subroutine when a
    /// `join` branch names it through its descriptor (`rN`): a packed, real,
    /// string, event or handle formal the subroutine reads through its
    /// parameter. Containers and records are aliased through shared frames
    /// instead. A process handle formal binds like other handle formals: the
    /// branch addresses the caller's counted slot and publishes its marker.
    fn ref_formal_descriptor(&self, target: NodeId) -> Option<RefFormalBinding> {
        if !self.cg.is_ref_formal(target) {
            return None;
        }
        if let Some((index, slots)) = self.cg.native_ref_formal_slots(target) {
            return Some(RefFormalBinding::Record(index, slots));
        }
        if self.cg.capture_source(target).is_some_and(|source| {
            matches!(source.kind, StorageKind::Container | StorageKind::Native)
        }) {
            return None;
        }
        let function = self.func.as_ref().or(self.cg.func.as_ref())?;
        if matches!(self.cg.kind(target), NodeKind::FuncArg { ty, .. } if is_handle_kind(&ty.kind))
        {
            return match function.chandle_read.get(&target) {
                Some(IrChandleExpr::FormalRead(index)) => Some(RefFormalBinding::Handle(*index)),
                _ => None,
            };
        }
        // An event formal's marker slot is filled when the model needs it,
        // which is known only after every body is lowered.
        if let Some(IrEventRef::Formal(index)) = function.event_args.get(&target) {
            return Some(RefFormalBinding::Handle(*index));
        }
        if let Some(IrStringExpr::FormalRead(index)) = function.string_read.get(&target) {
            return Some(RefFormalBinding::Descriptor(*index));
        }
        if let Some(IrExprKind::FormalRead(index)) = function.arg_ir.get(&target).map(IrExpr::kind)
        {
            return Some(RefFormalBinding::Descriptor(*index));
        }
        // A specialization binds the formal to the caller's actual itself.
        (function.arg_lhs.contains_key(&target) || function.const_ref_lhs.contains_key(&target))
            .then_some(RefFormalBinding::Actual)
    }

    /// Reject fork branches that name a `ref` formal of the enclosing
    /// subroutine where the branch would not reach the caller's variable.
    /// Inside `join_any`/`join_none` such a reference is illegal except in a
    /// fork block-item initializer, which the forking process evaluates
    /// (SV 9.3.2). A `join` branch is legal and binds the formal's
    /// descriptor; a formal it cannot bind is rejected.
    fn check_ref_formal_captures(
        &self,
        join: IrJoinKind,
        branches: &[NodeId],
        capture_targets: &[Vec<NodeId>],
    ) -> Result<(), String> {
        for (branch, targets) in branches.iter().zip(capture_targets) {
            for target in targets {
                if !self.cg.is_ref_formal(*target) {
                    continue;
                }
                let name = &self.cg.node(*target).name;
                let location = self.cg.source_location(*branch);
                if join != IrJoinKind::Join {
                    return Err(format!(
                        "fork branch at {location} in `{}` refers to `ref` formal `{name}` inside a fork-join_any or fork-join_none block; this is illegal except in the initializer of a fork block-item declaration (IEEE 1800-2009 9.3.2)",
                        self.cg.source_path(&self.path)
                    ));
                }
                let aliased = self.cg.capture_source(*target).is_some_and(|source| {
                    matches!(source.kind, StorageKind::Container | StorageKind::Native)
                });
                if !aliased && self.ref_formal_descriptor(*target).is_none() {
                    return Err(format!(
                        "fork branch at {location} in `{}` refers to `ref` formal `{name}` of the enclosing task, which the branch cannot bind to the caller's actual",
                        self.cg.source_path(&self.path)
                    ));
                }
            }
        }
        Ok(())
    }
}
