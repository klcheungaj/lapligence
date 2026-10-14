//! Deferred effects of static initializers.
//!
//! SV 13.4.4 lets a function schedule work that becomes active only after
//! it returns (a `fork`-`join_none`, a nonblocking assignment or a
//! nonblocking event trigger) only when the calling thread was created by an
//! initial or always procedure or a fork of one. A static declaration
//! initializer runs before any process exists (SV 6.8, 10.5), so a call it
//! makes, directly or through other subroutines or a class constructor, must
//! not reach such a statement; the text requires an error.

use super::*;

/// The first statement an initializer reaches that would schedule work after
/// its call returns, with the subroutine that contains it.
struct DeferredEffect {
    statement: NodeId,
    subroutine: NodeId,
}

impl Codegen<'_> {
    /// Reject every SystemVerilog static initializer whose calls reach a
    /// statement that schedules work after the call returns. Verilog-2001
    /// initializers run as active-region processes, where such statements
    /// are legal.
    pub(super) fn validate_static_initializer_effects(&self) -> Result<(), String> {
        if self.db.edition() != LanguageEdition::SystemVerilog2009 {
            return Ok(());
        }
        let mut sources = self
            .initializer_sources
            .iter()
            .flat_map(|(declaration, sources)| {
                sources.iter().map(move |source| (*declaration, *source))
            })
            .collect::<Vec<_>>();
        // Report the earliest declaration first, independent of hash order.
        sources.sort_unstable_by_key(|(declaration, source)| (*declaration, source.index()));
        let mut checked = HashSet::new();
        for (declaration, source) in sources {
            if let Some(effect) = self.deferred_effect(source, &mut checked) {
                let declaration = NodeId::from_index(declaration as usize);
                let what = match self.kind(effect.statement) {
                    NodeKind::Stmt(StmtKind::Fork { .. }) => "fork",
                    NodeKind::Stmt(StmtKind::EventTrigger { .. }) => "nonblocking event trigger",
                    _ => "nonblocking assignment",
                };
                return Err(format!(
                    "static initializer of `{}` at {} calls `{}`, whose {what} at {} schedules work after the call returns; a function may do that only when called from a process (SV 13.4.4), and static initialization runs before any process starts",
                    self.node(declaration).name,
                    self.source_location(source),
                    self.node(effect.subroutine).name,
                    self.source_location(effect.statement),
                ));
            }
        }
        Ok(())
    }

    /// Search the subroutines `source` calls, transitively, for a deferred
    /// effect. `checked` holds subroutines already searched without one.
    fn deferred_effect(
        &self,
        source: NodeId,
        checked: &mut HashSet<NodeId>,
    ) -> Option<DeferredEffect> {
        let mut pending = Vec::new();
        let mut stack = vec![source];
        // The initializer expression itself: collect its callees.
        while let Some(current) = stack.pop() {
            pending.extend(self.deferred_effect_callee(current));
            stack.extend(self.node(current).children.iter().copied());
        }
        while let Some(subroutine) = pending.pop() {
            if !checked.insert(subroutine) {
                continue;
            }
            let mut stack = vec![subroutine];
            while let Some(current) = stack.pop() {
                match self.kind(current) {
                    NodeKind::Stmt(StmtKind::Fork { .. })
                    | NodeKind::Stmt(StmtKind::Assign {
                        blocking: false, ..
                    })
                    | NodeKind::Stmt(StmtKind::EventTrigger {
                        blocking: false, ..
                    }) => {
                        return Some(DeferredEffect {
                            statement: current,
                            subroutine,
                        });
                    }
                    _ => {}
                }
                pending.extend(self.deferred_effect_callee(current));
                stack.extend(self.node(current).children.iter().copied());
            }
        }
        None
    }

    /// The resolved subroutine a call, method call or class construction
    /// at `node` runs.
    fn deferred_effect_callee(&self, node: NodeId) -> Option<NodeId> {
        match self.kind(node) {
            NodeKind::FuncCall {
                name,
                is_task,
                callee,
                ..
            } => callee.or_else(|| {
                let instance = self.owner_instance(node)?;
                self.resolve_callee_env(instance, name, *is_task, None)
                    .ok()
                    .map(|(subroutine, _)| subroutine)
            }),
            NodeKind::MethodCall { callee, .. } => *callee,
            NodeKind::Expr(ExprKind::NewClass { constructor, .. }) => *constructor,
            _ => None,
        }
        .filter(|subroutine| matches!(self.kind(*subroutine), NodeKind::FuncTask { .. }))
    }
}
