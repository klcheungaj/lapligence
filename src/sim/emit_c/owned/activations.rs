//! Named activation edges unwind lexical owners before crossing a C scope.
use super::*;

impl Frame<'_, '_> {
    pub(super) fn activation_scope(
        &mut self,
        target: IrActivationTarget,
        exit: &str,
        body: &[IrStmt],
    ) -> Result<(), String> {
        // The exit belongs to the wrapper, not its nested body: cancellation
        // must drop all body locals, including scopes created by a pending call.
        self.begin_block(&[IrStmt::Label(exit.to_owned())]);
        let handle = self.scalar(
            "llg_activation_t*",
            format!(
                "llg_activation_enter({}u, {}u)",
                target.declaration(),
                target.instance()
            ),
        );
        self.activations.push(Activation {
            exit: exit.to_owned(),
            handle: handle.clone(),
            lexical_depth: self.labels.len() - 1,
            checked: false,
        });
        self.block(body)?;
        // Some lowered named blocks cannot be cancelled themselves, but keep
        // the common cleanup label shape. A dead reference is standard C and
        // prevents warning-clean builds from diagnosing that label as unused.
        self.line(format!("if (0) goto {exit};"));
        self.line(format!("{exit}: ;"));
        let checked = self
            .activations
            .pop()
            .is_some_and(|activation| activation.checked);
        self.line(format!("llg_activation_exit({handle});"));
        self.end_block();
        if checked {
            // Control reaches here from a check that found some activation
            // cancelled, possibly an enclosing one: check again at once.
            let mark = self.cancellation_mark();
            self.cancellation_point();
            self.cancellation_check_covering(mark)?;
        }
        Ok(())
    }

    /// Check straight after the cancellation points emitted since `mark`
    /// (a call or an await with no branch in between). Every path from those
    /// points passes this check, so enclosing constructs need not repeat it.
    pub(super) fn cancellation_check_covering(&mut self, mark: usize) -> Result<(), String> {
        self.cancellation_check_since(mark)?;
        self.cancellation_points = mark;
        Ok(())
    }

    /// Current cancellation-point count, taken at the start of a construct
    /// whose end may need a cancellation check.
    pub(super) fn cancellation_mark(&self) -> usize {
        self.cancellation_points
    }

    /// Record a synchronous cancellation point (`disable`, or a call that may
    /// disable). Resume points are recorded by `Frame::line`.
    pub(super) fn cancellation_point(&mut self) {
        self.cancellation_points += 1;
    }

    /// Whether calling `function` may disable an activation synchronously.
    pub(super) fn callee_may_disable(&mut self, function: usize) -> bool {
        if let Some(known) = self.may_disable.get(&function) {
            return *known;
        }
        let model = self.ctx.model;
        let result =
            crate::sim::execution::effects_for_statements(model, &model.func(function).body)
                .contains(&crate::sim::execution::ExecutionEffect::Disable);
        self.may_disable.insert(function, result);
        result
    }

    /// Emit a cancellation check when a cancellation point was emitted since
    /// `mark`. Checks follow the innermost enclosing construct, so a point in
    /// one branch is checked before control merges; a nested activation's exit
    /// is covered by the check after its enclosing statement.
    pub(super) fn cancellation_check_since(&mut self, mark: usize) -> Result<(), String> {
        if self.cancellation_points == mark {
            return Ok(());
        }
        if let Some(activation) = self.activations.last_mut() {
            activation.checked = true;
            let exit = activation.exit.clone();
            self.line("if (llg_activation_cancelled()) {");
            self.goto(&exit)?;
            self.line("}");
        } else if self.cancellation_return {
            // A caller can own the disabled activation. Do not continue a
            // callee or copy its outputs back after a cooperative wake-up.
            self.line("if (llg_activation_cancelled()) goto _llg_return;");
        }
        Ok(())
    }

    pub(super) fn leave_activations(&mut self, target_depth: Option<usize>) {
        let handles = self
            .activations
            .iter()
            .rev()
            .take_while(|activation| {
                target_depth.is_none_or(|depth| activation.lexical_depth > depth)
            })
            .map(|activation| activation.handle.clone())
            .collect::<Vec<_>>();
        for handle in handles {
            self.line(format!("llg_activation_exit({handle});"));
        }
    }

    pub(super) fn fork_group(&mut self, kind: &str, target: Option<IrActivationTarget>) -> String {
        let initializer = if let Some(target) = target {
            format!(
                "llg_fork_group_new_target({kind}, {}u, {}u)",
                target.declaration(),
                target.instance()
            )
        } else {
            format!("llg_fork_group_new({kind})")
        };
        self.scalar("llg_fork_group_t*", initializer)
    }
}
