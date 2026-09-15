//! Forks.

use super::*;

impl EmitCtx<'_, '_> {

    /// Lower one `fork … join` site.  Each branch becomes its own coroutine
    /// function attached to the enclosing process's pre-functions; nested
    /// constructs inside a branch append their own pre-functions first,
    /// mirroring the pre-IR emission order.
    ///
    /// Detached branches may retain automatic subroutine storage through an
    /// activation frame.  A synchronous function can only use `join_none`,
    /// while an inlined task body may use the ordinary join variants.
    pub(super) fn lower_fork(
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
        let capture_targets = branches
            .iter()
            .map(|branch| self.cg.fork_capture_targets(*branch))
            .collect::<Vec<_>>();
        let has_captures = capture_targets.iter().any(|targets| !targets.is_empty());
        let enclosing_func = self.func.clone();
        let detached_function_branch = enclosing_func
            .as_ref()
            .is_some_and(|function| !function.is_task && function.def_node.is_some())
            || self.inline.is_some();
        let process_kind = self.process_kind;
        let mut names = Vec::with_capacity(branches.len());
        let mut captured_branches = Vec::with_capacity(branches.len());
        for (k, branch) in branches.iter().enumerate() {
            let fn_name = format!("{}_b{}", self.cg.new_fn_name(&self.path, "fork"), k);
            let label = format!("{}.br{k}", self.path);
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
                        None,
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
            for (slot, target) in capture_targets[k].iter().enumerate() {
                let source = self.cg.capture_source(*target).ok_or_else(|| {
                    format!(
                        "automatic declaration `{}` was not collected before fork capture in `{}`",
                        self.cg.node(*target).name,
                        self.path
                    )
                })?;
                let initial = previous_captures
                    .get(target)
                    .map(|binding| {
                        IrExpr::new(
                            IrExprKind::LocalRead(Codegen::capture_local_name(binding.storage)),
                            binding.local.width,
                            binding.local.signed,
                            None,
                        )
                    })
                    .unwrap_or(source.initial);
                let storage = StorageRef::for_declaration(
                    frame,
                    slot as u32,
                    self.cg.declaration_identity(*target)?,
                    source.lifetime,
                    StorageOwnership::Owned,
                )
                .with_kind(source.kind);
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
                captures.push(IrCapture::new(storage, initial));
            }

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
                    None,
                    false,
                );
                bctx.process_kind = process_kind;
                let body_result = bctx.lower_stmt(*branch);
                (body_result, std::mem::take(&mut bctx.pre_fns))
            };
            self.cg.func = saved_cg_func;
            self.cg.depth_arg = saved_cg_depth;
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
}
