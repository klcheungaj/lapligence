//! Events.

use super::*;

impl<'a> Codegen<'a> {
    // ── Signal resolution ──────────────────────────────────────────────────

    pub(super) fn collect_named_event(
        &mut self,
        path: &str,
        declaration: NodeId,
        seen: &mut HashSet<String>,
    ) -> Result<(), String> {
        let name = self.node(declaration).name.clone();
        if name.is_empty() || !seen.insert(name.clone()) {
            return Ok(());
        }
        let Some(metadata) = self.db.event_array_meta(declaration) else {
            let info = self.new_event_info(event_global_name(path, &name));
            self.event_globals.insert(declaration, info);
            return Ok(());
        };
        if !matches!(metadata.kind(), ArrayKind::Static) {
            return Err(format!(
                "named event array `{name}` in `{path}` has unsupported storage kind"
            ));
        }
        let mut total = 1u64;
        let mut dims = Vec::with_capacity(metadata.dimensions().len());
        for (dimension, bounds) in metadata.dimensions().iter().enumerate() {
            let Some((left, right)) = bounds else {
                return Err(format!(
                    "named event array `{name}` in `{path}` has unresolved dimension {dimension}"
                ));
            };
            let extent = (i64::from(*left) - i64::from(*right)).unsigned_abs() + 1;
            total = total
                .checked_mul(extent)
                .ok_or_else(|| format!("named event array `{name}` in `{path}` is too large"))?;
            dims.push((*left, *right));
        }
        if dims.is_empty() {
            return Err(format!(
                "named event array `{name}` in `{path}` has no dimensions"
            ));
        }
        for linear in 0..total {
            let info = self.new_event_info(event_global_name(path, &format!("{name}_{linear}")));
            self.event_elements.insert((declaration, linear), info);
        }
        let elements = (0..total)
            .map(|linear| {
                self.event_elements
                    .get(&(declaration, linear))
                    .map(|info| info.ir)
                    .ok_or_else(|| {
                        format!("named event array `{name}` in `{path}` lost element {linear}")
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let array = self.model.events.len();
        self.model.events.push(crate::sim::ir::IrEvent::new_array(
            event_global_name(path, &name),
            dims,
            elements,
        ));
        self.event_arrays.insert(declaration, array);
        Ok(())
    }

    pub(in super::super) fn new_event_info(&mut self, global: String) -> EventInfo {
        let ir = self.model.events.len();
        let info = EventInfo { global, ir };
        self.model.events.push(IrEvent::new(info.global.clone()));
        self.events.push(info.clone());
        info
    }

    /// Resolve an event expression to its declaration and any unpacked-array
    /// indices. The expression remains owned by the DB until this point so a
    /// reassigned handle can be lowered separately from its synchronization
    /// object.
    pub(in super::super) fn event_target_of(&self, node: NodeId) -> Option<EventTarget> {
        let clocking = self
            .db
            .resolve_clocking_member(node)
            .or_else(|| self.db.is_clocking_block(node).then_some(node))
            .or_else(|| match self.kind(node) {
                NodeKind::Expr(ExprKind::Ref {
                    target: Some(target),
                }) if self.db.is_clocking_block(*target) => Some(*target),
                NodeKind::Expr(ExprKind::HierPath { refs, .. }) => refs
                    .iter()
                    .flatten()
                    .copied()
                    .find(|target| self.db.is_clocking_block(*target)),
                _ => None,
            });
        if let Some(block) = clocking.filter(|block| self.db.is_clocking_block(*block)) {
            return Some(EventTarget {
                declaration: block,
                indices: Vec::new(),
            });
        }
        match self.kind(node) {
            NodeKind::NamedEvent => Some(EventTarget {
                declaration: node,
                indices: Vec::new(),
            }),
            NodeKind::Var { .. }
                if self.event_globals.contains_key(&node)
                    || self.event_arrays.contains_key(&node)
                    || self.db.event_array_meta(node).is_some() =>
            {
                Some(EventTarget {
                    declaration: node,
                    indices: Vec::new(),
                })
            }
            NodeKind::FuncArg { ty, .. } if ty.kind == "event" => Some(EventTarget {
                declaration: node,
                indices: Vec::new(),
            }),
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) if (matches!(self.kind(*target), NodeKind::NamedEvent)
                || matches!(self.kind(*target), NodeKind::Var { .. })
                    && (self.event_globals.contains_key(target)
                        || self.event_arrays.contains_key(target)
                        || self.db.event_array_meta(*target).is_some()))
                || matches!(self.kind(*target), NodeKind::FuncArg { ty, .. } if ty.kind == "event") =>
            {
                Some(EventTarget {
                    declaration: *target,
                    indices: Vec::new(),
                })
            }
            NodeKind::Expr(ExprKind::ArraySelect { base, indices }) => {
                let mut target = self.event_target_of(*base)?;
                target.indices.extend(indices.iter().copied());
                Some(target)
            }
            NodeKind::Expr(ExprKind::ScopeRef { target }) => self.event_target_of(*target),
            NodeKind::Expr(ExprKind::Cast { operand, .. }) => self.event_target_of(*operand),
            NodeKind::Expr(ExprKind::HierPath { refs, .. }) => refs
                .iter()
                .rev()
                .copied()
                .flatten()
                .find(|target| {
                    matches!(self.kind(*target), NodeKind::NamedEvent)
                        || matches!(self.kind(*target), NodeKind::Var { .. })
                            && (self.event_globals.contains_key(target)
                                || self.event_arrays.contains_key(target)
                                || self.db.event_array_meta(*target).is_some())
                })
                .map(|declaration| EventTarget {
                    declaration,
                    indices: Vec::new(),
                }),
            _ => None,
        }
    }

    pub(in super::super) fn is_null_event_expression(&self, node: NodeId) -> bool {
        match self.kind(node) {
            NodeKind::Expr(ExprKind::Constant {
                const_type: ConstantType::Null,
                ..
            }) => true,
            NodeKind::Expr(ExprKind::Cast { operand, .. }) => {
                self.is_null_event_expression(*operand)
            }
            _ => false,
        }
    }

    /// Model index of a captured named-event expression.
    pub(in super::super) fn event_index_of(
        &self,
        target: &EventTarget,
        scope_path: &str,
    ) -> Result<usize, String> {
        if let Some(metadata) = self.db.event_array_meta(target.declaration) {
            if target.indices.len() != metadata.dimensions().len() {
                return Err(format!(
                    "named event array `{}` in `{scope_path}` requires {} indices",
                    self.node(target.declaration).name,
                    metadata.dimensions().len()
                ));
            }
            let mut linear = 0u64;
            for (bounds, index) in metadata.dimensions().iter().zip(&target.indices) {
                let Some((left, right)) = *bounds else {
                    return Err(format!(
                        "named event array `{}` in `{scope_path}` has unresolved bounds",
                        self.node(target.declaration).name
                    ));
                };
                let value = self.eval_bound_i128(*index).map_err(|_| {
                    format!(
                        "named event array index for `{}` in `{scope_path}` must be a known constant",
                        self.node(target.declaration).name
                    )
                })?;
                let lo = i128::from(left.min(right));
                let hi = i128::from(left.max(right));
                if value < lo || value > hi {
                    return Err(format!(
                        "named event array index for `{}` in `{scope_path}` is out of range",
                        self.node(target.declaration).name
                    ));
                }
                let offset = if left >= right {
                    i128::from(left) - value
                } else {
                    value - i128::from(left)
                } as u64;
                let extent = (i64::from(left) - i64::from(right)).unsigned_abs() + 1;
                linear = linear
                    .checked_mul(extent)
                    .and_then(|value| value.checked_add(offset))
                    .ok_or_else(|| {
                        format!(
                            "named event array index for `{}` in `{scope_path}` is too large",
                            self.node(target.declaration).name
                        )
                    })?;
            }
            return self
                .event_elements
                .get(&(target.declaration, linear))
                .map(|info| info.ir)
                .ok_or_else(|| {
                    format!(
                        "cannot resolve named event array element `{}` in `{scope_path}`",
                        self.node(target.declaration).name
                    )
                });
        }
        if !target.indices.is_empty() {
            return Err(format!(
                "named event `{}` in `{scope_path}` is not an array",
                self.node(target.declaration).name
            ));
        }
        self.event_globals
            .get(&target.declaration)
            .map(|info| info.ir)
            .ok_or_else(|| {
                format!(
                    "cannot resolve named event reference `{}` in `{scope_path}`",
                    self.node(target.declaration).name
                )
            })
    }

    /// Lower an event target without losing a runtime array index. Constant
    /// selects retain the compact scalar handle path; variable selects carry
    /// typed index expressions to the runtime pointer table.
    pub(in super::super) fn event_ref_of(
        &mut self,
        target: &EventTarget,
        scope_path: &str,
    ) -> Result<IrEventRef, String> {
        if let Some(event) = self
            .func
            .as_ref()
            .and_then(|function| function.event_args.get(&target.declaration))
        {
            if !target.indices.is_empty() {
                return Err(format!(
                    "event formal `{}` cannot be indexed in `{scope_path}`",
                    self.node(target.declaration).name
                ));
            }
            return Ok(event.clone());
        }
        if matches!(
            self.kind(target.declaration),
            NodeKind::FuncArg { ty, .. } if ty.kind == "event"
        ) {
            // Event-formal definitions are never called through a C ABI
            // function: call sites inline them so the handle identity remains
            // an alias of the actual. Keep the separately emitted body
            // structurally valid for model construction; its null fallback is
            // unreachable from an admitted call.
            return Ok(IrEventRef::Null);
        }
        if self.db.event_array_meta(target.declaration).is_some() {
            if target.indices.len()
                != self
                    .db
                    .event_array_meta(target.declaration)
                    .expect("event metadata checked above")
                    .dimensions()
                    .len()
            {
                return Err(format!(
                    "named event array `{}` requires {} indices",
                    self.node(target.declaration).name,
                    self.db
                        .event_array_meta(target.declaration)
                        .expect("event metadata checked above")
                        .dimensions()
                        .len()
                ));
            }
            if target
                .indices
                .iter()
                .all(|index| self.eval_bound_i128(*index).is_ok())
            {
                return Ok(IrEventRef::Static(self.event_index_of(target, scope_path)?));
            }
            let array = self
                .event_arrays
                .get(&target.declaration)
                .copied()
                .ok_or_else(|| {
                    format!(
                        "cannot resolve named event array `{}` in `{scope_path}`",
                        self.node(target.declaration).name
                    )
                })?;
            let indices = target
                .indices
                .iter()
                .map(|index| {
                    let expression = self.lower_expr(scope_path, *index)?;
                    if expression.is_real() {
                        return Err(format!(
                            "named event array index for `{}` in `{scope_path}` must be integral",
                            self.node(target.declaration).name
                        ));
                    }
                    Ok(expression)
                })
                .collect::<Result<Vec<_>, _>>()?;
            return Ok(IrEventRef::Array { array, indices });
        }
        Ok(IrEventRef::Static(self.event_index_of(target, scope_path)?))
    }
}
