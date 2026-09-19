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
        });
        self.block(body)?;
        self.line(format!("{exit}: ;"));
        self.activations.pop();
        self.line(format!("llg_activation_exit({handle});"));
        self.end_block();
        Ok(())
    }

    pub(super) fn cancellation_check(&mut self) -> Result<(), String> {
        if let Some(activation) = self.activations.last() {
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
