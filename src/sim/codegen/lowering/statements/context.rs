//! Context.

use super::*;

impl<'c, 'a> EmitCtx<'c, 'a> {
    /// Build an emission context and record `func`/`depth_arg`/`inst` on the
    /// codegen, where expression and LHS resolution reads them (refs inside
    /// nested operations recurse through `Codegen::emit_expr`, which has no
    /// access to the `EmitCtx` itself).
    pub(in super::super) fn new(
        cg: &'c mut Codegen<'a>,
        path: String,
        inst: NodeId,
        depth_arg: &str,
        func: Option<FuncCtx>,
        inline: Option<InlineCtx>,
        in_final: bool,
    ) -> EmitCtx<'c, 'a> {
        cg.func = func.clone();
        cg.depth_arg = depth_arg.to_string();
        cg.inst = inst;
        EmitCtx {
            cg,
            path,
            saw_wait: false,
            process_kind: None,
            inst,
            depth_arg: depth_arg.to_string(),
            func,
            inline,
            pre_fns: Vec::new(),
            ctrl: Vec::new(),
            label_seq: 0,
            in_final,
        }
    }

    /// Allocate a fresh control-flow label (`_xb3`, `_bk7`, `_ct9`).  The
    /// sequence is per emitted C function (see [`EmitCtx::ctrl`]); distinct
    /// tags cannot collide with each other or with the inline-task done
    /// labels (`_id<node>`).
    pub(super) fn new_label(&mut self, tag: &str) -> String {
        self.label_seq += 1;
        format!("_{tag}{}", self.label_seq)
    }

    /// Ordinary functions cannot suspend, but a task body emitted as a typed
    /// C call runs inside the caller's libaco coroutine and may yield. Inline
    /// task expansion is still allowed for the event/cancellation paths that
    /// need caller-owned activation rebinding.
    pub(super) fn timing_forbidden(&self) -> bool {
        self.inline.is_none() && self.func.as_ref().is_some_and(|function| !function.is_task)
    }
}
