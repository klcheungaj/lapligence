//! Local variable formal arguments of sequence instances (IEEE 1800-2009
//! 16.8.2).
//!
//! An instance whose local formals cannot be captured once at attempt entry
//! is lowered as its inlined form (the `seq2_inlined` example of 16.8.2):
//! `(1, f = actual) ##0 body` initializes every input and inout formal on the
//! instance's first tick, and every match of the body assigns each inout and
//! output formal back to its actual local variable (`(body, actual = f)`).
//! Each runtime thread owns its copy of every local, so overlapping attempts
//! and the branches of one attempt initialize and copy out independently.

use super::*;
use crate::sim::codegen::lowering::collection::Select;
use crate::sim::ir::IrMutationExpr;

/// One local variable formal of an instance and the actual bound to it.
#[derive(Clone, Copy)]
struct LocalFormal {
    formal: NodeId,
    direction: DbDirection,
    actual: NodeId,
}

impl Codegen<'_> {
    /// The local variables instance `expression` initializes: its local
    /// formals in declaration order, each with the local variable its body
    /// reads and writes, and its declared locals that have a declaration
    /// assignment, in declaration order. Non-local formals are substituted by
    /// Slang and need nothing. A local the body never names needs no slot: an
    /// input is unread, an inout copies back the value it was given, and Slang
    /// rejects an output that a match leaves unassigned.
    fn instance_locals(&self, expression: NodeId) -> (Vec<LocalFormal>, Vec<NodeId>) {
        let NodeKind::Expr(ExprKind::AssertionInstance { body, bindings, .. }) =
            self.kind(expression)
        else {
            return (Vec::new(), Vec::new());
        };
        let locals = self.instance_referenced_locals(*body);
        if locals.is_empty() {
            return (Vec::new(), Vec::new());
        }
        let formals = bindings
            .iter()
            .filter_map(|binding| {
                let direction = self.db.assertion_formal_direction(binding.formal)?;
                let port = self.node(binding.formal);
                let formal = locals.iter().copied().find(|local| {
                    let node = self.node(*local);
                    node.name == port.name
                        && node.file == port.file
                        && node.line == port.line
                        && node.col == port.col
                })?;
                Some(LocalFormal {
                    formal,
                    direction,
                    actual: binding.actual,
                })
            })
            .collect::<Vec<_>>();
        let mut declared = locals
            .into_iter()
            .filter(|local| self.db.assertion_local_initializer(*local).is_some())
            .collect::<Vec<_>>();
        declared.sort_by_key(|local| (self.node(*local).line, self.node(*local).col));
        (formals, declared)
    }

    /// The locals of property instance `expression` initialized at the start
    /// of each property attempt (16.10).
    pub(super) fn instance_initialized_locals(&self, expression: NodeId) -> Vec<NodeId> {
        let (formals, declared) = self.instance_locals(expression);
        formals
            .into_iter()
            .filter(|formal| formal.direction == DbDirection::Input)
            .map(|formal| formal.formal)
            .chain(declared)
            .collect()
    }

    /// The local assertion variables of the instance whose body is `body`:
    /// those it references, including in the actuals of the instances it
    /// contains and in its locals' declaration assignments, but not those of
    /// the contained instances themselves.
    fn instance_referenced_locals(&self, body: NodeId) -> Vec<NodeId> {
        let mut found = Vec::new();
        let mut seen = HashSet::new();
        let mut pending = vec![body];
        while let Some(current) = pending.pop() {
            if !seen.insert(current) {
                continue;
            }
            match self.kind(current) {
                NodeKind::Expr(ExprKind::Ref {
                    target: Some(target),
                }) if self.db.is_assertion_local_var(*target) => {
                    if !found.contains(target) {
                        found.push(*target);
                        if let Some(initializer) = self.db.assertion_local_initializer(*target) {
                            pending.push(initializer);
                        }
                    }
                }
                NodeKind::Expr(ExprKind::AssertionInstance { bindings, .. }) => {
                    pending.extend(bindings.iter().map(|binding| binding.actual));
                    continue;
                }
                _ => {}
            }
            pending.extend(self.node(current).children.iter().copied());
        }
        found
    }

    /// Lower sequence instance `expression` through its inlined form when it
    /// initializes a local that the attempt-entry capture does not cover: an
    /// instance below the attempt's first tick or nested in another instance,
    /// or one with an inout or output formal. Returns `None` for every other
    /// instance, which keeps the existing lowering (and its cost) unchanged.
    pub(super) fn lower_instance_with_locals(
        &mut self,
        path: &str,
        expression: NodeId,
        builder: &mut SequenceBuilder,
        role: &str,
    ) -> Result<Option<SequenceFragment>, String> {
        let (formals, declared) = self.instance_locals(expression);
        if formals.is_empty() && declared.is_empty() {
            return Ok(None);
        }
        let at_entry = self.assertion_instance_depth == 0 && builder.next_state == 0;
        if at_entry
            && formals
                .iter()
                .all(|formal| formal.direction == DbDirection::Input)
        {
            return Ok(None);
        }
        let location = self.source_location(expression);
        let mut initializers = Vec::new();
        let mut copy_outs = Vec::new();
        for formal in &formals {
            let name = self.node(formal.formal).name.clone();
            let binding = self.assertion_local_binding(formal.formal)?.ok_or_else(|| {
                format!("local formal `{name}` of the sequence instance at {location} is outside a sequence")
            })?;
            match formal.direction {
                DbDirection::Input | DbDirection::Inout => {
                    let actual = self.lower_expr(path, formal.actual)?;
                    if actual.is_real() || !sampled_compatible(&actual) {
                        return Err(format!(
                            "local formal `{name}` of the sequence instance at {location} needs one packed sampled actual (SV 16.8.2)"
                        ));
                    }
                    initializers.push(local_assignment(binding, actual));
                }
                DbDirection::Output => {}
                direction => {
                    return Err(format!(
                        "local formal `{name}` of the sequence instance at {location} has unsupported direction {direction:?} (SV 16.8.2)"
                    ));
                }
            }
            if matches!(formal.direction, DbDirection::Inout | DbDirection::Output) {
                // 16.8.2: "The entire actual argument expression bound to an
                // inout or output local variable formal argument shall itself
                // be a reference to a local variable".
                let Some(target) = self.assertion_local_lhs_target(formal.actual) else {
                    return Err(format!(
                        "the actual of {} local formal `{name}` at {} must be a local variable (SV 16.8.2)",
                        direction_word(formal.direction),
                        self.source_location(formal.actual)
                    ));
                };
                let outer = self.assertion_local_binding(target)?.ok_or_else(|| {
                    format!("local variable actual of `{name}` at {location} is outside a sequence")
                })?;
                let value = IrExpr::new(
                    IrExprKind::LocalRead(format!(
                        "llg_sequence_local_read(data, {}u)",
                        binding.slot
                    )),
                    binding.width,
                    binding.signed,
                    None,
                );
                copy_outs.push(local_assignment(outer, value));
            }
        }
        // The body reads its locals from their slots; the initialization
        // here replaces the attempt-entry capture. Declaration assignments
        // follow the formals' (16.10) and see the instance's own bindings.
        let inlined = formals
            .iter()
            .map(|formal| formal.formal)
            .chain(declared.iter().copied())
            .collect::<Vec<_>>();
        self.assertion_inlined_formals
            .extend(inlined.iter().copied());
        let body = self.lower_assertion_instance(expression, false, |this, body| {
            for local in &declared {
                let binding = this
                    .assertion_local_binding(*local)?
                    .ok_or_else(|| format!("local variable at {location} is outside a sequence"))?;
                let source = this
                    .db
                    .assertion_local_initializer(*local)
                    .ok_or_else(|| "declared local lost its initializer".to_owned())?;
                let value = this.lower_expr(path, source)?;
                if value.is_real() || !sampled_compatible(&value) {
                    return Err(format!(
                        "local variable `{}` declared at {} needs one packed sampled initial value",
                        this.node(*local).name,
                        this.source_location(*local)
                    ));
                }
                initializers.push(local_assignment(binding, value));
            }
            this.lower_sequence_fragment(path, body, builder, role)
        });
        for formal in &inlined {
            self.assertion_inlined_formals.remove(formal);
        }
        let body = body?.ok_or_else(|| format!("sequence instance at {location} has no body"))?;
        if body.empty {
            return Err(if copy_outs.is_empty() {
                format!(
                    "a sequence instance that initializes local variables and admits an empty match is not supported at {location}"
                )
            } else {
                // 16.8.2: "It shall be an error for an instance of a named
                // sequence with an inout or output local variable formal
                // argument to admit an empty match."
                format!(
                    "the sequence instance at {location} has an inout or output local formal and admits an empty match (SV 16.8.2)"
                )
            });
        }
        let mut fragment = body;
        if !initializers.is_empty() {
            let clock = self.sampled_clock.or(body.leading_clock);
            let start = builder.state()?;
            let accept = builder.state()?;
            builder.edge_with_clock(start, accept, zero_range(), Some(true_atom()), clock)?;
            builder.attach_match_items(accept, initializers)?;
            let prefix = SequenceFragment {
                empty: false,
                start,
                accept,
                leading_clock: clock,
                trailing_clock: clock,
            };
            fragment = builder.concatenate(prefix, body, zero_range())?;
        }
        if !copy_outs.is_empty() {
            let accept = builder.state()?;
            builder.epsilon_with_clock(
                fragment.accept,
                accept,
                zero_range(),
                fragment.trailing_clock,
            )?;
            builder.attach_match_items(accept, copy_outs)?;
            fragment.accept = accept;
        }
        Ok(Some(fragment))
    }

    /// A packed selection of a local assertion variable as an assignment
    /// target (`v[3:0] = e` in a match item, 16.10).
    pub(in super::super) fn assertion_local_select_lhs(
        &mut self,
        path: &str,
        lhs: NodeId,
    ) -> Result<Option<IrLhs>, String> {
        let mut selects = Vec::new();
        let mut current = lhs;
        let root = loop {
            match self.kind(current) {
                NodeKind::Expr(ExprKind::BitSelect { base, index }) => {
                    selects.push((*base, Select::Elements(vec![*index])));
                    current = *base;
                }
                NodeKind::Expr(ExprKind::ArraySelect { base, indices }) => {
                    selects.push((*base, Select::Elements(indices.clone())));
                    current = *base;
                }
                NodeKind::Expr(ExprKind::PartSelect { base, left, right }) => {
                    selects.push((*base, Select::Part(*left, *right)));
                    current = *base;
                }
                NodeKind::Expr(ExprKind::IndexedPartSelect {
                    base,
                    base_expr,
                    width_expr,
                    neg,
                }) => {
                    selects.push((*base, Select::Indexed(*base_expr, *width_expr, *neg)));
                    current = *base;
                }
                _ => break current,
            }
        };
        if selects.is_empty() {
            return Ok(None);
        }
        let Some(target) = self.assertion_local_lhs_target(root) else {
            return Ok(None);
        };
        let binding = self
            .assertion_local_binding(target)?
            .ok_or_else(|| "local assertion variable is outside a sequence graph".to_owned())?;
        let mut width = binding.width;
        let mut steps = Vec::new();
        for (base, select) in selects.into_iter().rev() {
            self.packed_selection_steps(path, base, select, &mut width, &mut steps)?;
        }
        Ok(Some(IrLhs::PackedSelect {
            target: Box::new(local_lhs(binding)),
            steps,
            signed: false,
            two_state: binding.two_state,
        }))
    }
}

fn direction_word(direction: DbDirection) -> &'static str {
    match direction {
        DbDirection::Output => "output",
        _ => "inout",
    }
}

fn local_lhs(binding: AssertionLocalBinding) -> IrLhs {
    IrLhs::WholeRef {
        addr: format!("llg_sequence_local_addr(data, {}u)", binding.slot),
        width: binding.width,
        signed: binding.signed,
        two_state: binding.two_state,
        shortreal: false,
    }
}

/// `local = value`, cast to the local's type (16.8.2: "The value of the
/// actual argument shall be cast to the type of the local variable formal
/// argument").
pub(in super::super) fn local_assignment(binding: AssertionLocalBinding, value: IrExpr) -> IrExpr {
    let value = IrExpr::resize_to(value, binding.width, binding.signed);
    IrExpr::new(
        IrExprKind::Mutation(Box::new(IrMutationExpr {
            lhs: local_lhs(binding),
            value: Box::new(value),
            current_width: binding.width,
            current_signed: binding.signed,
            reads_current: false,
            post: false,
        })),
        binding.width,
        binding.signed,
        None,
    )
}
