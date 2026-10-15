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
            pre_fns: Vec::new(),
            ctrl: Vec::new(),
            label_seq: 0,
            in_final,
            call_owner: "deferred immediate assertion action",
            display_scope: None,
            coverage_end_exit: None,
        }
    }

    /// `%m` text for the call at `node`: the enclosing assertion's scope when
    /// this body is a hidden assertion host, otherwise the lowering path's.
    pub(super) fn format_scope(&self, node: NodeId) -> String {
        match &self.display_scope {
            Some((base, root)) => {
                let suffix = root
                    .map(|root| self.cg.named_suffix_below(root, node))
                    .unwrap_or_default();
                format!("{base}{suffix}")
            }
            None => self.cg.format_scope(&self.path, node),
        }
    }

    /// Allocate a fresh control-flow label (`_xb3`, `_bk7`, `_ct9`).  The
    /// sequence is per emitted C function (see [`EmitCtx::ctrl`]); distinct
    /// tags cannot collide with each other.
    pub(super) fn new_label(&mut self, tag: &str) -> String {
        self.label_seq += 1;
        format!("_{tag}{}", self.label_seq)
    }

    /// Ordinary functions cannot suspend; a timing task becomes a stackless
    /// callee.
    pub(super) fn timing_forbidden(&self) -> bool {
        self.func.as_ref().is_some_and(|function| !function.is_task)
    }
}
