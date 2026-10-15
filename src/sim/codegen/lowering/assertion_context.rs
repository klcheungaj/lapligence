//! Assertion context.

use super::*;

/// System tasks that change which assertions are checked or which of their
/// actions execute (IEEE 1800-2009 20.11-20.12).
const ASSERTION_CONTROL_TASKS: [&str; 10] = [
    "$asserton",
    "$assertoff",
    "$assertkill",
    "$assertcontrol",
    "$assertpasson",
    "$assertpassoff",
    "$assertfailon",
    "$assertfailoff",
    "$assertnonvacuouson",
    "$assertvacuousoff",
];

/// Where the property-level locals of the instance being lowered (its
/// initialized formals and declared locals, 16.10) are initialized: at the
/// start of each property attempt, which is the start of the antecedent's
/// attempt or, without an antecedent, of the consequent's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PropertyLocalInit {
    /// The graph being lowered starts with the property attempt.
    Here,
    /// The graph is an implication consequent that inherits the antecedent's.
    Inherited,
    /// The graph starts later, e.g. a property operand.
    Unsupported,
}

impl<'a> Codegen<'a> {
    /// Whether the design calls any assertion control task. Without one,
    /// control state never leaves its defaults, so immediate assertions need
    /// no runtime control query (a static fact of the whole design).
    pub(super) fn design_uses_assertion_controls(&mut self) -> bool {
        if let Some(used) = self.assertion_controls_used {
            return used;
        }
        let used = self.db.nodes().iter().any(|node| {
            matches!(node.kind(), NodeKind::SysCall { name } if ASSERTION_CONTROL_TASKS.contains(&name.as_str()))
        });
        self.assertion_controls_used = Some(used);
        used
    }

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

    /// Initialize a local variable once per attempt, at attempt entry: a
    /// local input formal from its actual, a declared local from its
    /// declaration assignment (16.8.2, 16.10). Locals of an instance below the
    /// attempt's first tick are initialized by its inlined form instead
    /// (`lower_instance_with_locals`); output and inout formals always are.
    pub(super) fn ensure_assertion_local_initializer(
        &mut self,
        path: &str,
        target: NodeId,
        binding: AssertionLocalBinding,
    ) -> Result<(), String> {
        if self.assertion_inlined_formals.contains(&target) {
            return Ok(());
        }
        let formal_actual = self.assertion_formal_actual(target);
        let declared = if formal_actual.is_none() {
            self.db.assertion_local_initializer(target)
        } else {
            None
        };
        if formal_actual.is_none() && declared.is_none() {
            return Ok(());
        }
        let name = self.node(target).name.clone();
        let location = self.source_location(target);
        if self.assertion_property_locals.contains(&target) {
            match self.assertion_property_local_init {
                PropertyLocalInit::Here => {}
                // The antecedent attempt initialized it; the consequent
                // inherits its value at the antecedent's match.
                PropertyLocalInit::Inherited => return Ok(()),
                PropertyLocalInit::Unsupported => {
                    return Err(format!(
                        "local variable `{name}` of the property declared at {location} is initialized at the property attempt's start, which this property form does not support"
                    ));
                }
            }
        }
        if self.assertion_instance_depth != 1 || !self.assertion_local_capture_at_attempt_entry {
            return Err(format!(
                "local variable `{name}` declared at {location} is initialized by an instance that is not supported here (`{path}`)"
            ));
        }
        if formal_actual.is_some() {
            match self.db.assertion_formal_direction(target) {
                Some(DbDirection::Input) => {}
                Some(direction) => {
                    return Err(format!(
                        "assertion formal `{name}` declared at {location} uses unsupported {direction:?} direction in `{path}`"
                    ));
                }
                None => return Ok(()),
            }
        }
        if self
            .assertion_local_initializers
            .as_ref()
            .is_some_and(|initializers| initializers.contains_key(&target))
        {
            return Ok(());
        }
        let source = formal_actual.or(declared).expect("one initializer source");
        let value = self.lower_expr(path, source)?;
        if value.is_real() || !assertions::sampled_compatible(&value) {
            return Err(format!(
                "local variable `{name}` declared at {location} needs one packed sampled initial value"
            ));
        }
        let initializer = assertions::local_assignment(binding, value);
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
                sorted_node_ids(bindings).into_iter().find_map(|formal| {
                    let formal_node = self.node(formal);
                    (formal_node.name == target_node.name
                        && formal_node.file == target_node.file
                        && formal_node.line == target_node.line
                        && formal_node.col == target_node.col
                        && formal_node.end_line == target_node.end_line
                        && formal_node.end_col == target_node.end_col)
                        .then_some(bindings[&formal])
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
