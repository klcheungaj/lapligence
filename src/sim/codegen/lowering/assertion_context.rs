//! Assertion context.

use super::*;

impl<'a> Codegen<'a> {

    /// Allocate (or retrieve) the bounded runtime slot for a Slang local
    /// assertion variable. Returning `None` keeps ordinary reference
    /// resolution unchanged when no assertion graph is active.
    pub(super) fn assertion_local_binding(
        &mut self,
        target: NodeId,
    ) -> Result<Option<AssertionLocalBinding>, String> {
        if !self.db.is_assertion_local_var(target) {
            return Ok(None);
        }
        let Some(existing) = self
            .assertion_local_bindings
            .as_ref()
            .and_then(|bindings| bindings.get(&target).copied())
        else {
            if self.assertion_local_bindings.is_none() {
                return Ok(None);
            }
            // Continue below and allocate the first slot for this declaration.
            let NodeKind::Var { ty } = self.kind(target) else {
                return Err(format!(
                    "local assertion declaration `{}` has no packed value type",
                    self.node(target).name
                ));
            };
            let width = ty.width.ok_or_else(|| {
                format!(
                    "local assertion variable `{}` has an unsupported non-packed type",
                    self.node(target).name
                )
            })?;
            if width == 0 || width > LLG_MAX_WIDTH {
                return Err(format!(
                    "local assertion variable `{}` has invalid width {width}",
                    self.node(target).name
                ));
            }
            let slot = u32::try_from(
                self.assertion_local_bindings
                    .as_ref()
                    .map_or(0, HashMap::len),
            )
            .map_err(|_| "sequence has too many local assertion variables".to_owned())?;
            let binding = AssertionLocalBinding {
                slot,
                width,
                signed: ty.signed,
                two_state: self.db.is_two_state_type(target),
            };
            self.assertion_local_bindings
                .as_mut()
                .expect("assertion local map checked above")
                .insert(target, binding);
            return Ok(Some(binding));
        };
        Ok(Some(existing))
    }

    /// Capture a local input formal's actual value once per sequence attempt.
    /// Output, inout, and ref assertion formals need copy-out/aliasing rules
    /// that are not represented by the bounded sequence callback ABI, so they
    /// remain explicit lowering boundaries.
    pub(super) fn ensure_assertion_local_initializer(
        &mut self,
        path: &str,
        target: NodeId,
        binding: AssertionLocalBinding,
    ) -> Result<(), String> {
        let Some(actual) = self.assertion_formal_actual(target) else {
            return Ok(());
        };
        if self.assertion_instance_depth != 1 || !self.assertion_local_capture_at_attempt_entry {
            return Err(format!(
                "local input formal `{}` is only supported at sequence-attempt entry in `{path}`",
                self.node(target).name
            ));
        }
        match self.db.assertion_formal_direction(target) {
            Some(DbDirection::Input) => {}
            Some(direction) => {
                return Err(format!(
                    "assertion formal `{}` uses unsupported {direction:?} direction in `{path}`",
                    self.node(target).name
                ));
            }
            None => return Ok(()),
        }
        if self
            .assertion_local_initializers
            .as_ref()
            .is_some_and(|initializers| initializers.contains_key(&target))
        {
            return Ok(());
        }
        let actual = self.lower_expr(path, actual)?;
        if actual.is_real() || !assertions::sampled_compatible(&actual) {
            return Err(format!(
                "assertion local input formal `{}` requires one packed sampled actual in `{path}`",
                self.node(target).name
            ));
        }
        let value = IrExpr::resize_to(actual, binding.width, binding.signed);
        let lhs = IrLhs::WholeRef {
            addr: format!("llg_sequence_local_addr(data, {}u)", binding.slot),
            width: binding.width,
            signed: binding.signed,
            two_state: binding.two_state,
            shortreal: false,
        };
        let initializer = IrExpr::new(
            IrExprKind::Mutation(Box::new(crate::sim::ir::IrMutationExpr {
                lhs,
                value: Box::new(value),
                current_width: binding.width,
                current_signed: binding.signed,
                reads_current: false,
                post: false,
            })),
            binding.width,
            binding.signed,
            None,
        );
        self.assertion_local_initializers
            .as_mut()
            .ok_or_else(|| "assertion local initializer is outside a sequence graph".to_owned())?
            .insert(target, initializer);
        Ok(())
    }

    fn assertion_formal_actual(&self, target: NodeId) -> Option<NodeId> {
        let bindings = self.assertion_formal_actuals.as_ref()?;
        bindings
            .get(&target)
            .or_else(|| bindings.get(&self.db.source_identity(target)))
            .copied()
            .or_else(|| {
                let target_node = self.node(target);
                bindings.iter().find_map(|(formal, actual)| {
                    let formal_node = self.node(*formal);
                    (formal_node.name == target_node.name
                        && formal_node.file == target_node.file
                        && formal_node.line == target_node.line
                        && formal_node.col == target_node.col
                        && formal_node.end_line == target_node.end_line
                        && formal_node.end_col == target_node.end_col)
                        .then_some(*actual)
                })
            })
    }

    /// Lower an owned assertion-instance body with its formal-to-actual
    /// bindings in scope. Slang has already materialized the expanded body;
    /// this small context is only needed for legal `local input` formals that
    /// must be copied into each runtime attempt before the first sampled atom.
    pub(super) fn lower_assertion_instance<T>(
        &mut self,
        expression: NodeId,
        capture_at_attempt_entry: bool,
        lower: impl FnOnce(&mut Self, NodeId) -> Result<T, String>,
    ) -> Result<Option<T>, String> {
        let (body, bindings) = match self.kind(expression) {
            NodeKind::Expr(ExprKind::AssertionInstance { body, bindings, .. }) => {
                (*body, bindings.clone())
            }
            _ => return Ok(None),
        };
        let previous = self.assertion_formal_actuals.take();
        let previous_depth = self.assertion_instance_depth;
        let previous_capture = self.assertion_local_capture_at_attempt_entry;
        let next_depth = previous_depth
            .checked_add(1)
            .ok_or_else(|| "assertion instance nesting depth overflow".to_owned())?;
        let mut active = previous.clone().unwrap_or_default();
        active.extend(
            bindings
                .into_iter()
                .map(|binding| (binding.formal, binding.actual)),
        );
        self.assertion_formal_actuals = Some(active);
        self.assertion_instance_depth = next_depth;
        self.assertion_local_capture_at_attempt_entry =
            capture_at_attempt_entry && previous_depth == 0;
        let result = lower(self, body);
        self.assertion_formal_actuals = previous;
        self.assertion_instance_depth = previous_depth;
        self.assertion_local_capture_at_attempt_entry = previous_capture;
        result.map(Some)
    }

    pub(super) fn assertion_local_lhs_target(&self, lhs: NodeId) -> Option<NodeId> {
        match self.kind(lhs) {
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) if self.db.is_assertion_local_var(*target) => Some(*target),
            _ if self.db.is_assertion_local_var(lhs) => Some(lhs),
            _ => None,
        }
    }
}
