//! System timing checks of specify blocks (IEEE 1364-2001 15, IEEE
//! 1800-2009 31).
//!
//! Every module instance with checks becomes one [`IrTimingCheckSet`]: the
//! resolved terminals, each check's reference and data events (edge mask,
//! condition), limits in scheduler ticks and notifier index, plus one hidden
//! process that runs the runtime step whenever a terminal changes and then
//! toggles each notifier the step flagged with an ordinary blocking
//! assignment, so `@(notifier)`, continuous readers and UDP inputs wake as
//! for any variable write. `$setuphold`, `$recrem`, `$timeskew`,
//! `$fullskew` and negative `$nochange` offsets (ADV-010) are rejected with
//! their location.

use super::*;
use crate::core::db::{
    ExprKind, Operation, TimingCheck, TimingCheckKind, TimingEdges, TimingEvent,
};
use crate::sim::ir::{
    IrBinOp, IrConst, IrRuntimeQuery, IrTimingCheck, IrTimingCheckKind, IrTimingCheckSet,
    IrTimingCondition, IrTimingEvent, IrTimingTerminal, IrUniquePriorityCheck,
};

/// Placeholders of [`IrTimingCheck::text`] for the reference and the data
/// event time of a reported violation.
const REFERENCE_TIME: &str = "\u{1}";
const DATA_TIME: &str = "\u{2}";

/// One lowered check of an instance before duplicates are removed.
struct LoweredCheck {
    check: IrTimingCheck,
    /// The notifier variable and its expression, if any.
    notifier: Option<(NodeId, NodeId)>,
    name: &'static str,
}

impl Codegen<'_> {
    /// Lower the timing checks of every elaborated instance. Runs after the
    /// module paths, before any process is lowered. A design without timing
    /// checks costs one emptiness test and gets no tables or processes.
    pub(in super::super) fn lower_timing_checks(&mut self) -> Result<(), String> {
        let db = self.db;
        if db.timing_checks().is_empty() {
            return Ok(());
        }
        let design = self.design_nodes().into_iter().collect::<HashSet<_>>();
        let mut instances: Vec<(NodeId, Vec<&TimingCheck>)> = Vec::new();
        let mut instance_index = HashMap::new();
        for check in db.timing_checks() {
            if !design.contains(&check.node) {
                continue;
            }
            match check.kind {
                TimingCheckKind::SetupHold
                | TimingCheckKind::RecRem
                | TimingCheckKind::TimeSkew
                | TimingCheckKind::FullSkew
                | TimingCheckKind::Unknown => {
                    return Err(self.specify_unsupported(
                        check.node,
                        &format!("timing check `{}` (not simulated yet)", check.kind.name()),
                    ));
                }
                _ => {}
            }
            let instance = self.owning_inst(check.node).ok_or_else(|| {
                self.specify_unsupported(check.node, "timing check outside a module")
            })?;
            let slot = *instance_index.entry(instance).or_insert_with(|| {
                instances.push((instance, Vec::new()));
                instances.len() - 1
            });
            instances[slot].1.push(check);
        }
        for (instance, checks) in instances {
            let previous_inst = self.inst;
            self.inst = instance;
            let result = self.lower_instance_timing_checks(instance, &checks);
            self.inst = previous_inst;
            result?;
        }
        Ok(())
    }

    fn lower_instance_timing_checks(
        &mut self,
        instance: NodeId,
        checks: &[&TimingCheck],
    ) -> Result<(), String> {
        let scope = self.instance_path_of(instance);
        let mut terminals: Vec<IrTimingTerminal> = Vec::new();
        let mut reads = Vec::new();
        let mut conditions: Vec<IrExpr> = Vec::new();
        let mut lowered = Vec::with_capacity(checks.len());
        for check in checks {
            lowered.push(self.lower_timing_check(
                &scope,
                check,
                &mut terminals,
                &mut reads,
                &mut conditions,
            )?);
        }
        // Validation applies whatever the options; disabled checks then
        // leave no trace in the model.
        let options = self.model.timing_check_options;
        if !options.checks {
            return Ok(());
        }
        let lowered = self.drop_duplicate_timing_checks(&scope, lowered);
        let mut notifiers: Vec<(NodeId, NodeId)> = Vec::new();
        let mut set_checks = Vec::with_capacity(lowered.len());
        for LoweredCheck {
            mut check,
            notifier,
            ..
        } in lowered
        {
            check.notifier = notifier.map(|(target, expression)| {
                let index = notifiers
                    .iter()
                    .position(|(known, _)| *known == target)
                    .unwrap_or_else(|| {
                        notifiers.push((target, expression));
                        notifiers.len() - 1
                    });
                index as u32
            });
            set_checks.push(check);
        }
        let set = self.model.timing_checks.len();
        let label = self.source_path(&scope);
        let mut body = vec![IrStmt::TimingCheckStep { set }];
        if options.notifiers {
            for (index, (_, expression)) in notifiers.iter().enumerate() {
                body.push(self.notifier_toggle(&scope, set, index, *expression)?);
            }
        }
        self.model.timing_checks.push(IrTimingCheckSet::new(
            label.clone(),
            terminals,
            set_checks,
            conditions,
            notifiers.len() as u32,
        ));
        reads.sort_by_key(|dependency| self.dependency_sort_key(dependency));
        reads.dedup();
        let block = checks[0].block;
        let name = self.new_fn_name(&scope, "timing_check");
        let origin = self.origin(block);
        self.model.processes.push(IrProcess::new_with_origin(
            name,
            format!("{label}.specify"),
            IrShape::SensLoop { reads },
            Vec::new(),
            body,
            origin,
        ));
        Ok(())
    }

    /// Lower one check: roles of its event arguments (Tables 31-1..31-12),
    /// limits, events and notifier, with located errors for the forms that
    /// cannot be simulated.
    fn lower_timing_check(
        &mut self,
        scope: &str,
        check: &TimingCheck,
        terminals: &mut Vec<IrTimingTerminal>,
        reads: &mut Vec<IrDependency>,
        conditions: &mut Vec<IrExpr>,
    ) -> Result<LoweredCheck, String> {
        let location = self.specify_location(check.node);
        let name = check.kind.name();
        let error = |message: String| format!("timing check `{name}` at {location}: {message}");
        let kind = match check.kind {
            TimingCheckKind::Setup => IrTimingCheckKind::Setup,
            TimingCheckKind::Hold => IrTimingCheckKind::Hold,
            TimingCheckKind::Recovery => IrTimingCheckKind::Recovery,
            TimingCheckKind::Removal => IrTimingCheckKind::Removal,
            TimingCheckKind::Skew => IrTimingCheckKind::Skew,
            TimingCheckKind::Period => IrTimingCheckKind::Period,
            TimingCheckKind::Width => IrTimingCheckKind::Width,
            TimingCheckKind::NoChange => IrTimingCheckKind::NoChange,
            _ => return Err(error("the check is not simulated yet".into())),
        };
        let event = |index: usize| {
            check.events[index]
                .ok_or_else(|| error(format!("event argument {} is missing", index + 1)))
        };
        // `$setup` alone lists its data event first (31.3.1).
        let (reference, data) = match kind {
            IrTimingCheckKind::Setup => (event(1)?, Some(event(0)?)),
            IrTimingCheckKind::Period | IrTimingCheckKind::Width => (event(0)?, None),
            _ => (event(0)?, Some(event(1)?)),
        };
        if kind.single_signal() {
            let bits = reference.edges.bits();
            if bits & TimingEdges::RISING != 0 && bits & TimingEdges::FALLING != 0 {
                return Err(error(
                    "the reference event of a single-signal check needs only rising or only \
                     falling transitions"
                        .into(),
                ));
            }
        }
        // `$width`: a notifier needs an explicit threshold (31.4.4).
        if kind == IrTimingCheckKind::Width
            && check.argument_count >= 4
            && check.empty_arguments & (1 << 3) == 0
            && check.empty_arguments & (1 << 2) != 0
        {
            return Err(error(
                "the threshold argument must be present when the notifier is given (IEEE \
                 1800-2009 31.4.4, IEEE 1364-2001 15.3.4)"
                    .into(),
            ));
        }
        let mut limit = |index: usize, required: bool| -> Result<(u64, bool), String> {
            match check.limits.get(index).copied().flatten() {
                Some(expression) => self
                    .timing_limit_ticks(check.node, expression)
                    .map_err(&error),
                None if required => Err(error(format!("limit argument {} is missing", index + 1))),
                None => Ok((0, false)),
            }
        };
        // A negative limit of a check whose limits are non-negative counts as
        // zero, as the frontend warns; negative `$nochange` offsets shrink its
        // window, which is ADV-010.
        let (first, first_negative) = limit(0, true)?;
        let (second, second_negative) = match kind {
            IrTimingCheckKind::Width => limit(1, false)?,
            IrTimingCheckKind::NoChange => limit(1, true)?,
            _ => (0, false),
        };
        if kind == IrTimingCheckKind::NoChange && (first_negative || second_negative) {
            return Err(self.specify_unsupported(
                check.node,
                "negative `$nochange` offset (not simulated yet)",
            ));
        }
        let mut lower_event =
            |codegen: &mut Self, event: &TimingEvent| -> Result<IrTimingEvent, String> {
                let terminal = codegen.path_terminal(event.terminal).map_err(&error)?;
                let entry = IrTimingTerminal::new(terminal.info.ir, terminal.lsb, terminal.width);
                let index = match terminals.iter().position(|known| *known == entry) {
                    Some(index) => index,
                    None => {
                        terminals.push(entry);
                        reads.push(codegen.signal_dependency(&terminal.info));
                        terminals.len() - 1
                    }
                };
                let condition = match event.condition {
                    None => None,
                    Some(expression) => {
                        let value = codegen.lower_expr(scope, expression).map_err(|message| {
                            error(format!("the condition is not simulated: {message}"))
                        })?;
                        if value.is_real() {
                            return Err(error("the condition is not an integral value".into()));
                        }
                        let index = match conditions.iter().position(|known| *known == value) {
                            Some(index) => index,
                            None => {
                                conditions.push(value);
                                conditions.len() - 1
                            }
                        };
                        Some(IrTimingCondition::new(
                            index as u32,
                            codegen.nondeterministic_condition(expression),
                        ))
                    }
                };
                Ok(IrTimingEvent::new(
                    index as u32,
                    event.edges.bits(),
                    condition,
                ))
            };
        let reference_event = lower_event(self, &reference)?;
        let data_event = match (kind, data) {
            (_, Some(data)) => lower_event(self, &data)?,
            // `$period`: the same edge again (31.4.5).
            (IrTimingCheckKind::Period, None) => reference_event,
            // `$width`: the opposite edge (31.4.4): 0 and 1 swap, x stays.
            (_, None) => IrTimingEvent::new(
                reference_event.terminal,
                opposite_edges(reference_event.edges),
                reference_event.condition,
            ),
        };
        let notifier = match check.notifier {
            None => None,
            Some(expression) => Some((
                self.notifier_target(expression).map_err(&error)?,
                expression,
            )),
        };
        let text = self.timing_check_text(check, kind, &reference, data.as_ref(), first, second);
        Ok(LoweredCheck {
            check: IrTimingCheck::new(
                kind,
                reference_event,
                data_event,
                first,
                second,
                None,
                location,
                text,
            ),
            notifier,
            name,
        })
    }

    /// Of several checks of one kind with the same reference and data events
    /// only the one with the larger limit (the first of equals) is simulated;
    /// the others are reported and dropped (decision A9-D6).
    fn drop_duplicate_timing_checks(
        &mut self,
        scope: &str,
        lowered: Vec<LoweredCheck>,
    ) -> Vec<LoweredCheck> {
        let key = |check: &IrTimingCheck| (check.kind, check.reference, check.data);
        let mut kept: Vec<LoweredCheck> = Vec::with_capacity(lowered.len());
        for candidate in lowered {
            let Some(index) = kept
                .iter()
                .position(|known| key(&known.check) == key(&candidate.check))
            else {
                kept.push(candidate);
                continue;
            };
            let dropped = if candidate.check.limit > kept[index].check.limit {
                std::mem::replace(&mut kept[index], candidate)
            } else {
                candidate
            };
            self.warnings.push(format!(
                "duplicate timing check `{}` at {} in `{}` is disabled; the one at {} with the \
                 larger limit is simulated",
                dropped.name,
                dropped.check.origin,
                self.source_path(scope),
                kept[index].check.origin,
            ));
        }
        kept
    }

    /// The variable a notifier argument names: a scalar (one-bit) variable
    /// of the instance (31.6; vector and integer notifiers are rejected, as
    /// is common practice for `notify_reg`).
    fn notifier_target(&self, expression: NodeId) -> Result<NodeId, String> {
        let target = match self.kind(expression) {
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) => *target,
            _ => return Err("the notifier must name a variable".to_owned()),
        };
        let info = self.signal_of(target).ok_or_else(|| {
            format!(
                "notifier `{}` has no simulated storage",
                self.display_name(target)
            )
        })?;
        if !matches!(self.kind(target), NodeKind::Var { .. })
            || info.real
            || info.shortreal
            || info.width != 1
        {
            return Err(format!(
                "notifier `{}` must be a scalar variable (IEEE 1800-2009 31.6)",
                self.display_name(target)
            ));
        }
        Ok(target)
    }

    /// `if (notify) n = (n === 1'b0) ? 1'b1 : (n === 1'bz) ? 1'bz : 1'b0;` --
    /// Table 31-13 with x becoming 0 (V Table 61, decision A9-D5).
    fn notifier_toggle(
        &mut self,
        scope: &str,
        set: usize,
        notifier: usize,
        expression: NodeId,
    ) -> Result<IrStmt, String> {
        let lhs = self.lower_lhs(scope, expression)?;
        let current = IrExpr::convert_to(self.lower_expr(scope, expression)?, 1, false);
        let bit = |bits: u64, z: u64| -> Result<IrExpr, String> {
            let constant = IrConst::packed(vec![bits], vec![0], vec![z], 1, false, None)
                .map_err(|error| error.to_string())?;
            Ok(IrExpr::new(IrExprKind::Const(constant), 1, false, None))
        };
        let is = |value: IrExpr| {
            IrExpr::new(
                IrExprKind::Bin {
                    op: IrBinOp::CaseEq,
                    a: Box::new(current.clone()),
                    b: Box::new(value),
                },
                1,
                false,
                None,
            )
        };
        let mux = |sel: IrExpr, a: IrExpr, b: IrExpr| {
            IrExpr::new(
                IrExprKind::Mux {
                    sel: Box::new(sel),
                    a: Box::new(a),
                    b: Box::new(b),
                },
                1,
                false,
                None,
            )
        };
        let next = mux(
            is(bit(0, 0)?),
            bit(1, 0)?,
            mux(is(bit(0, 1)?), bit(0, 1)?, bit(0, 0)?),
        );
        let signed = self
            .notifier_target(expression)
            .ok()
            .and_then(|target| self.signal_of(target))
            .is_some_and(|info| info.signed);
        let query = IrExpr::new(
            IrExprKind::RuntimeQuery(IrRuntimeQuery::TimingNotify {
                set: set as u32,
                notifier: notifier as u32,
            }),
            1,
            false,
            None,
        );
        Ok(IrStmt::If {
            cond: query,
            then_: vec![IrStmt::Assign {
                lhs,
                rhs: IrExpr::convert_to(next, 1, signed),
                nba: false,
            }],
            els: None,
            check: IrUniquePriorityCheck::None,
        })
    }

    /// A `==` or `!=` condition is nondeterministic: an x or z result
    /// enables its event; every other form disables it (31.7).
    fn nondeterministic_condition(&self, expression: NodeId) -> bool {
        let mut node = expression;
        loop {
            match self.kind(node) {
                NodeKind::Expr(ExprKind::Operation {
                    op: Operation::Cast,
                    operands,
                    ..
                }) if operands.len() == 1 => node = operands[0],
                NodeKind::Expr(ExprKind::Operation {
                    op: Operation::Equal | Operation::NotEqual,
                    ..
                }) => return true,
                _ => return false,
            }
        }
    }

    /// The check as reported, in source argument order with the event time
    /// placeholders: `$setup(d:<data>, posedge c:<reference>, 10)`.
    fn timing_check_text(
        &self,
        check: &TimingCheck,
        kind: IrTimingCheckKind,
        reference: &TimingEvent,
        data: Option<&TimingEvent>,
        first: u64,
        second: u64,
    ) -> String {
        let event = |event: &TimingEvent, time: &str| {
            let condition = if event.condition.is_some() {
                " &&& ..."
            } else {
                ""
            };
            format!(
                "{}{}{condition}:{time}",
                edges_text(event.edges.bits()),
                self.terminal_text(event.terminal)
            )
        };
        let reference = event(reference, REFERENCE_TIME);
        let mut arguments = match (kind, data) {
            (IrTimingCheckKind::Setup, Some(data)) => vec![event(data, DATA_TIME), reference],
            (_, Some(data)) => vec![reference, event(data, DATA_TIME)],
            (_, None) => vec![reference, format!(":{DATA_TIME}")],
        };
        arguments.push(first.to_string());
        if matches!(kind, IrTimingCheckKind::Width | IrTimingCheckKind::NoChange) {
            arguments.push(second.to_string());
        }
        format!("{}({})", check.kind.name(), arguments.join(", "))
    }

    /// A terminal as written: a name or a constant select of one.
    fn terminal_text(&self, expression: NodeId) -> String {
        let name = |base: NodeId| match self.kind(base) {
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) => self.node(*target).name.clone(),
            _ => self.node(base).name.clone(),
        };
        let bound = |node: NodeId| {
            self.eval_bound_i128(node)
                .map_or_else(|_| "?".to_owned(), |value| value.to_string())
        };
        match self.kind(expression) {
            NodeKind::Expr(ExprKind::BitSelect { base, index }) => {
                format!("{}[{}]", name(*base), bound(*index))
            }
            NodeKind::Expr(ExprKind::PartSelect {
                base, left, right, ..
            }) => format!("{}[{}:{}]", name(*base), bound(*left), bound(*right)),
            NodeKind::Expr(ExprKind::IndexedPartSelect {
                base,
                base_expr,
                width_expr,
                neg,
                ..
            }) => format!(
                "{}[{}{}:{}]",
                name(*base),
                bound(*base_expr),
                if *neg { "-" } else { "+" },
                bound(*width_expr)
            ),
            _ => name(expression),
        }
    }
}

/// The transitions of the opposite edge: 0 and 1 swap, x stays (01 <-> 10,
/// 0x <-> 1x, x0 <-> x1).
fn opposite_edges(edges: u8) -> u8 {
    let pairs = [
        (TimingEdges::T01, TimingEdges::T10),
        (TimingEdges::T0X, TimingEdges::T1X),
        (TimingEdges::TX0, TimingEdges::TX1),
    ];
    let mut opposite = 0;
    for (a, b) in pairs {
        if edges & a != 0 {
            opposite |= b;
        }
        if edges & b != 0 {
            opposite |= a;
        }
    }
    opposite
}

/// The edge control of an event as written before its terminal.
fn edges_text(edges: u8) -> String {
    match edges {
        0 => String::new(),
        TimingEdges::RISING => "posedge ".to_owned(),
        TimingEdges::FALLING => "negedge ".to_owned(),
        _ => {
            let names = [
                (TimingEdges::T01, "01"),
                (TimingEdges::T0X, "0x"),
                (TimingEdges::T10, "10"),
                (TimingEdges::T1X, "1x"),
                (TimingEdges::TX0, "x0"),
                (TimingEdges::TX1, "x1"),
            ];
            let list = names
                .iter()
                .filter(|(bit, _)| edges & bit != 0)
                .map(|(_, name)| *name)
                .collect::<Vec<_>>();
            format!("edge[{}] ", list.join(","))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opposite_edges_swap_zero_and_one() {
        assert_eq!(opposite_edges(TimingEdges::RISING), TimingEdges::FALLING);
        assert_eq!(opposite_edges(TimingEdges::FALLING), TimingEdges::RISING);
        assert_eq!(
            opposite_edges(TimingEdges::T01 | TimingEdges::TX1),
            TimingEdges::T10 | TimingEdges::TX0
        );
        assert_eq!(opposite_edges(0), 0);
    }

    #[test]
    fn edge_text_names_shorthands_and_lists() {
        assert_eq!(edges_text(0), "");
        assert_eq!(edges_text(TimingEdges::RISING), "posedge ");
        assert_eq!(edges_text(TimingEdges::FALLING), "negedge ");
        assert_eq!(
            edges_text(TimingEdges::T01 | TimingEdges::TX1),
            "edge[01,x1] "
        );
    }
}
