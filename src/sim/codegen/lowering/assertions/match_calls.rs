//! Subroutine calls attached to a sequence match (IEEE 1800-2009 16.11).
//!
//! "The subroutines are scheduled in the Reactive region, like an action
//! block", and arguments passed by value "use sampled values of the
//! underlying variables and are consistent with the variable values used to
//! evaluate the sequence match". A call is therefore lowered like a deferred
//! assertion action: its by-value arguments become captures of an owned
//! frame, filled by the match item in the Observed region from sampled
//! values and the thread's locals, and its body is a callback that runs with
//! that frame in the Reactive region. Reference arguments are read when the
//! call runs. The callback lives on a synthetic process that is never
//! spawned, so every model pass sees its reads and writes.

use super::*;

impl Codegen<'_> {
    /// Whether match item `node` is a subroutine call (16.11) rather than a
    /// local variable assignment or increment (16.10).
    pub(super) fn is_match_item_call(&self, node: NodeId) -> bool {
        matches!(
            self.kind(node),
            NodeKind::FuncCall { .. } | NodeKind::SysCall { .. } | NodeKind::MethodCall { .. }
        )
    }

    /// Lower subroutine-call match item `node` to a [`IrExprKind::MatchCall`].
    pub(super) fn lower_match_item_call(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<IrExpr, String> {
        let location = self.source_location(node);
        let label = format!("{path}.match_call[{}]", node.index());
        // EmitCtx records its function context on the codegen; a match item
        // can be lowered inside a procedural `expect`, whose context returns.
        let saved = (self.func.clone(), self.depth_arg.clone(), self.inst);
        let inst = self.inst;
        let lowered = {
            let mut ctx = EmitCtx::new(self, label.clone(), inst, "0", None, false);
            ctx.call_owner = "sequence match-item call";
            ctx.lower_deferred_assertion_action(node)
                .map(|action| (action, std::mem::take(&mut ctx.pre_fns)))
        };
        (self.func, self.depth_arg, self.inst) = saved;
        let (action, mut pre_fns) =
            lowered.map_err(|error| format!("{error} (sequence match item at {location})"))?;
        pre_fns.extend(std::mem::take(&mut self.pending_container_pre_fns));
        let writes = self.ir_process_writes(self.collect_process_writes(node)?);
        let name = self.new_fn_name(&label, "match_call");
        self.model
            .processes
            .push(IrProcess::new_with_kind_and_writes(
                name.clone(),
                label,
                IrProcessKind::Synthetic,
                IrShape::RunOnce,
                writes,
                pre_fns,
                Vec::new(),
                self.origin(node),
            ));
        self.assertion_action_procs.insert(name);
        Ok(IrExpr::new(
            IrExprKind::MatchCall(Box::new(action)),
            1,
            false,
            None,
        ))
    }
}
