//! System timing checks of specify blocks (IEEE 1364-2001 15, IEEE
//! 1800-2009 31).
//!
//! Every module instance with checks becomes one [`IrTimingCheckSet`]: the
//! resolved terminals, each check's reference and data events (edge mask,
//! conditions), limits in scheduler ticks and notifier signal. The runtime
//! watches the terminals' storage; the model's one timing process wakes on a
//! hidden doorbell variable once per batch of terminal changes, runs the
//! changed sets and lets the runtime toggle the notifiers of violated checks
//! with ordinary variable writes, so `@(notifier)`, continuous readers and
//! UDP inputs wake as for any variable write.
//!
//! `$setuphold` and `$recrem` become a `$setup`/`$hold` (`$removal`/
//! `$recovery`) pair of rows. With negative timing checks enabled, their
//! negative limits delay the reference and data signals (31.9): the delays
//! solve difference constraints per signal and edge class
//! ([`solve_signal_delays`]), the runtime drives explicit and implicit
//! delayed signals as transport delay lines, and the checks 31.9.1 names
//! read the delayed signals with adjusted limits. Without the option,
//! negative limits count as 0 and explicit delayed signals are copies
//! (31.9.4).

use super::*;
use crate::core::db::{
    ExprKind, Operation, TimingCheck, TimingCheckKind, TimingEdges, TimingEvent,
};
use crate::sim::ir::{
    IrTimingCheck, IrTimingCheckKind, IrTimingCheckSet, IrTimingCondition, IrTimingEvent,
    IrTimingLine, IrTimingLineStorage, IrTimingTerminal, TIMING_EVENT_BASED, TIMING_REMAIN_ACTIVE,
};
use std::collections::BTreeMap;

/// Placeholders of [`IrTimingCheck::text`] for the reference and the data
/// event time of a reported violation.
const REFERENCE_TIME: &str = "\u{1}";
const DATA_TIME: &str = "\u{2}";

/// One lowered check row of an instance before duplicates are removed.
struct LoweredCheck {
    check: IrTimingCheck,
    /// The IR signal of the notifier variable, if any.
    notifier: Option<usize>,
    /// The source check: the two rows of a `$setuphold` or `$recrem` are no
    /// duplicates of standalone `$setup`/`$hold` checks.
    source: TimingCheckKind,
    /// The `$setuphold`/`$recrem` the row belongs to and whether it is the
    /// part before the reference event (`$setup`, `$removal`).
    combined: Option<(usize, bool)>,
    /// The declaration and the limit of it this row holds, for the SDF
    /// annotation targets.
    node: NodeId,
    slot: crate::sim::sdf::SdfLimitSlot,
}

/// One `$setuphold` or `$recrem` of an instance.
struct CombinedCheck {
    node: NodeId,
    origin: String,
    name: &'static str,
    reference: IrTimingEvent,
    data: IrTimingEvent,
    /// The limit before the reference event (setup, removal) and after it
    /// (hold, recovery), in ticks; negative until the limits are settled.
    before: i64,
    after: i64,
    /// Resolved delayed_reference and delayed_data signals.
    delayed: [Option<DelayedTarget>; 2],
    /// The report text naming the combined check, used while a limit stays
    /// negative.
    text: String,
}

/// An explicit delayed signal (31.9.1).
#[derive(Clone)]
struct DelayedTarget {
    terminal: IrTimingTerminal,
    /// The declaration it names.
    node: NodeId,
}

/// Edge classes of a delayed signal (31.9): a rise delay for 01, 0x and x1
/// and a fall delay for 10, 1x and x0.
const RISE: usize = 0;
const FALL: usize = 1;

/// One `$setuphold`/`$recrem` as the delay solver sees it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct DelayConstraint {
    pub(super) reference: u32,
    pub(super) reference_edges: u8,
    pub(super) data: u32,
    pub(super) data_edges: u8,
    pub(super) before: i64,
    pub(super) after: i64,
}

/// Delays of the delayed signals: `(rise, fall)` ticks per terminal of a
/// constraint, and the constraints whose most negative limit was set to 0
/// because the set had no solution, in order.
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct SignalDelays {
    pub(super) delays: BTreeMap<u32, (u64, u64)>,
    pub(super) zeroed: Vec<usize>,
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
            if check.kind == TimingCheckKind::Unknown {
                return Err(self.specify_unsupported(check.node, "unrecognized timing check"));
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
        let mut first_block = None;
        for (instance, checks) in instances {
            first_block.get_or_insert(checks[0].block);
            let previous_inst = self.inst;
            self.inst = instance;
            let result = self.lower_instance_timing_checks(instance, &checks);
            self.inst = previous_inst;
            result?;
        }
        if let (false, Some(block)) = (self.model.timing_checks.is_empty(), first_block) {
            self.add_timing_check_process(block);
        }
        Ok(())
    }

    /// The model's one timing process: the runtime rings the hidden doorbell
    /// variable once per batch of terminal changes, and the process then runs
    /// every set whose terminals changed. One wake per batch replaces one per
    /// instance and terminal change.
    fn add_timing_check_process(&mut self, block: NodeId) {
        let doorbell = self.model.signals.len();
        let c_name = "llg_timing_doorbell".to_owned();
        self.model.signals.push(IrSignal {
            fixed_default: None,
            c_name: c_name.clone(),
            hdl_name: None,
            ty: IrType::Packed {
                width: 1,
                signed: false,
                two_state: true,
            },
            net_driver: None,
            net_alias: Vec::new(),
            alias: None,
            omit: false,
        });
        self.model.timing_doorbell = Some(doorbell);
        let origin = self.origin(block);
        self.model.processes.push(IrProcess::new_with_origin(
            "llg_timing_checks".to_owned(),
            "specify".to_owned(),
            IrShape::SensLoop {
                reads: vec![IrDependency::scalar(c_name)],
            },
            Vec::new(),
            vec![IrStmt::TimingCheckStep],
            origin,
        ));
    }

    fn lower_instance_timing_checks(
        &mut self,
        instance: NodeId,
        checks: &[&TimingCheck],
    ) -> Result<(), String> {
        let scope = self.instance_path_of(instance);
        let mut terminals: Vec<IrTimingTerminal> = Vec::new();
        let mut conditions: Vec<IrExpr> = Vec::new();
        let mut lowered = Vec::with_capacity(checks.len());
        let mut combined = Vec::new();
        for check in checks {
            self.lower_timing_check(
                &scope,
                check,
                &mut terminals,
                &mut conditions,
                &mut lowered,
                &mut combined,
            )?;
        }
        let options = self.model.timing_check_options;
        let label = self.source_path(&scope);
        // Validation applies whatever the options.
        let explicit = self.explicit_delayed_signals(&label, &terminals, &combined)?;
        // Disabled checks leave only the explicit delayed signals, as copies
        // (31.9.4).
        if !options.checks {
            let lines =
                self.timing_lines(&mut terminals, &explicit, &SignalDelays::default(), false)?;
            if !lines.is_empty() {
                self.model.timing_checks.push(
                    IrTimingCheckSet::new(label, terminals, Vec::new(), Vec::new(), Vec::new())
                        .with_lines(lines),
                );
            }
            return Ok(());
        }
        self.settle_combined_limits(options.negative, &mut combined);
        // With negative timing checks every combined check's limits enter
        // the module's delayed signal solve, even when it yields zero delays
        // here: a limit annotated later changes them.
        if options.negative {
            self.sdf_delayed_checks
                .extend(combined.iter().map(|check| check.node));
        }
        let delays = if options.negative {
            let mut constraints = combined
                .iter()
                .map(|check| DelayConstraint {
                    reference: check.reference.terminal,
                    reference_edges: check.reference.edges,
                    data: check.data.terminal,
                    data_edges: check.data.edges,
                    before: check.before,
                    after: check.after,
                })
                .collect::<Vec<_>>();
            let delays = solve_signal_delays(&mut constraints);
            for &index in &delays.zeroed {
                let check = &mut combined[index];
                let limit = check.before.min(check.after);
                let message = format!(
                    "the negative timing check limits of a module have no consistent delayed \
                     signal delays; the limit {limit} of `{}` at {} counts as 0 (IEEE 1800-2009 \
                     31.9.1)",
                    check.name, check.origin,
                );
                self.timing_warning(message);
                if check.before <= check.after {
                    check.before = 0;
                } else {
                    check.after = 0;
                }
            }
            delays
        } else {
            SignalDelays::default()
        };
        let delayed = delays
            .delays
            .values()
            .any(|&(rise, fall)| rise != 0 || fall != 0);
        let lines = self.timing_lines(&mut terminals, &explicit, &delays, delayed)?;
        // A check whose negative limits sum to 0 or 1 has a window of under
        // two units, which "cannot yield timing violations" (31.9.1 a); its
        // limits still shaped the delays, but its rows do not run, since a
        // delayed data event on the delayed reference event would otherwise
        // violate the hold or recovery row.
        lowered.retain(|row| {
            row.combined.is_none_or(|(index, _)| {
                let check = &combined[index];
                (check.before >= 0 && check.after >= 0) || check.before + check.after > 1
            })
        });
        // The limits as settled, and the combined text while a limit stays
        // negative (the report then names the combined check).
        for row in &mut lowered {
            let Some((index, before)) = row.combined else {
                continue;
            };
            let check = &combined[index];
            row.check.limit = if before { check.before } else { check.after };
            if check.before < 0 || check.after < 0 {
                row.check.text = check.text.clone();
            }
        }
        // Once a delay is nonzero, the checks 31.9.1 names read the delayed
        // signals; `$skew`, `$timeskew` and `$fullskew` keep the undelayed
        // ones.
        if delayed {
            for row in &lowered {
                self.warn_adjusted_limit(&row.check, &delays);
            }
            let mut delayed_of: HashMap<u32, u32> = HashMap::new();
            for line in &lines {
                delayed_of.entry(line.source()).or_insert(line.target());
            }
            let target = |terminal: u32| delayed_of.get(&terminal).copied().unwrap_or(terminal);
            for row in &mut lowered {
                if !row.check.kind.skew() {
                    row.check.reference.terminal = target(row.check.reference.terminal);
                    row.check.data.terminal = target(row.check.data.terminal);
                }
            }
        }
        let lowered = self.drop_duplicate_timing_checks(&scope, lowered);
        // `--no-notifiers`: violations are still reported, notifiers keep
        // their values.
        let mut notifiers: Vec<usize> = Vec::new();
        let mut set_checks = Vec::with_capacity(lowered.len());
        let set = self.model.timing_checks.len() as u32;
        for LoweredCheck {
            mut check,
            notifier,
            node,
            slot,
            ..
        } in lowered
        {
            self.sdf_check_rows.push((
                node,
                crate::sim::sdf::SdfCheckRow {
                    set,
                    check: set_checks.len() as u32,
                    slot,
                },
            ));
            check.notifier = notifier.filter(|_| options.notifiers).map(|signal| {
                let index = notifiers
                    .iter()
                    .position(|known| *known == signal)
                    .unwrap_or_else(|| {
                        notifiers.push(signal);
                        notifiers.len() - 1
                    });
                index as u32
            });
            set_checks.push(check);
        }
        self.model.timing_checks.push(
            IrTimingCheckSet::new(label, terminals, set_checks, conditions, notifiers)
                .with_lines(lines),
        );
        Ok(())
    }

    /// Settle the limits of each `$setuphold`/`$recrem` before the delays
    /// are solved. Without negative timing checks a negative limit counts as
    /// 0 (31.9.4). With them, a negative limit needs `setup + hold` above
    /// one precision unit (31.3.3, 31.3.6): a negative sum sets the negative
    /// limits to 0, and a sum of 0 or 1 leaves a window that cannot report
    /// (31.9.1); both are warned about.
    fn settle_combined_limits(&mut self, negative: bool, combined: &mut [CombinedCheck]) {
        for check in combined {
            if check.before >= 0 && check.after >= 0 {
                continue;
            }
            let sum = check.before + check.after;
            let reason = if !negative {
                "negative limits count as 0 unless negative timing checks are enabled \
                 (--neg-timing-checks; IEEE 1800-2009 31.9.4)"
            } else if sum < 0 {
                "the sum of its limits is negative, so its negative limits count as 0 (IEEE \
                 1800-2009 31.3.3)"
            } else {
                if sum <= 1 {
                    let message = format!(
                        "`{}` at {}: the sum of its limits is not above one precision unit, so \
                         it cannot report violations (IEEE 1800-2009 31.3.3, 31.9.1)",
                        check.name, check.origin,
                    );
                    self.timing_warning(message);
                }
                continue;
            };
            let message = format!("`{}` at {}: {reason}", check.name, check.origin);
            self.timing_warning(message);
            check.before = check.before.max(0);
            check.after = check.after.max(0);
        }
    }

    /// Warn when a nonzero limit of a check on delayed signals is adjusted to
    /// 0 or less for some pair of transitions it compares (31.9.1 L52435:
    /// "the limit shall be set to 0, and the simulator shall issue a
    /// warning"); the runtime then uses 0 for those transitions. The limit
    /// grows by the delay of the timecheck transition and shrinks by that of
    /// the timestamp transition.
    fn warn_adjusted_limit(&mut self, check: &IrTimingCheck, delays: &SignalDelays) {
        let kind = check.kind;
        if kind.skew() || kind == IrTimingCheckKind::NoChange || check.limit == 0 {
            return;
        }
        let (stamp, timecheck) = match kind {
            IrTimingCheckKind::Setup | IrTimingCheckKind::Removal => (check.data, check.reference),
            _ => (check.reference, check.data),
        };
        let delay = |terminal: u32, class: usize| {
            delays.delays.get(&terminal).map_or(0, |&(rise, fall)| {
                i64::try_from(if class == RISE { rise } else { fall }).unwrap_or(i64::MAX)
            })
        };
        let lowest = edge_classes(stamp.edges)
            .flat_map(|stamp_class| {
                edge_classes(timecheck.edges).map(move |check_class| (stamp_class, check_class))
            })
            .map(|(stamp_class, check_class)| {
                check
                    .limit
                    .saturating_add(delay(timecheck.terminal, check_class))
                    .saturating_sub(delay(stamp.terminal, stamp_class))
            })
            .min();
        if let Some(adjusted) = lowest.filter(|adjusted| *adjusted <= 0) {
            let message = format!(
                "`{}` at {}: its limit {} adjusted by the delays of its delayed signals is \
                 {adjusted}, so it counts as 0 (IEEE 1800-2009 31.9.1)",
                ir_kind_name(kind),
                check.origin,
                check.limit,
            );
            self.timing_warning(message);
        }
    }

    /// A warning about a check's source, given once for all instances.
    fn timing_warning(&mut self, message: String) {
        if !self.warnings.contains(&message) {
            self.warnings.push(message);
        }
    }

    /// The explicit delayed signals of an instance by source terminal, with
    /// located errors for one that cannot be a copy of its signal: another
    /// width, the signal itself, one name for two signals or two names for
    /// one signal.
    fn explicit_delayed_signals(
        &self,
        label: &str,
        terminals: &[IrTimingTerminal],
        combined: &[CombinedCheck],
    ) -> Result<BTreeMap<u32, DelayedTarget>, String> {
        let mut explicit: BTreeMap<u32, DelayedTarget> = BTreeMap::new();
        for check in combined {
            for (role, target) in check.delayed.iter().enumerate() {
                let Some(target) = target else {
                    continue;
                };
                let source = if role == 0 {
                    check.reference.terminal
                } else {
                    check.data.terminal
                };
                let error = |message: String| {
                    format!(
                        "timing check `{}` at {} in `{label}`: {message}",
                        check.name, check.origin
                    )
                };
                let name = self.display_name(target.node);
                let signal = &terminals[source as usize];
                if signal.width() != target.terminal.width() {
                    return Err(error(format!(
                        "delayed signal `{name}` has {} bits but its signal has {} (IEEE \
                         1800-2009 31.9.1)",
                        target.terminal.width(),
                        signal.width()
                    )));
                }
                if signal.signal() == target.terminal.signal() {
                    return Err(error(format!(
                        "delayed signal `{name}` cannot be its own signal"
                    )));
                }
                if let Some(known) = explicit.get(&source) {
                    if known.terminal != target.terminal {
                        return Err(error(format!(
                            "one signal has two delayed signals, `{}` and `{name}` (IEEE \
                             1800-2009 31.9.1)",
                            self.display_name(known.node)
                        )));
                    }
                    continue;
                }
                if explicit
                    .values()
                    .any(|known| known.terminal == target.terminal)
                {
                    return Err(error(format!(
                        "delayed signal `{name}` is named for two different signals"
                    )));
                }
                explicit.insert(source, target.clone());
            }
        }
        Ok(explicit)
    }

    /// The delay lines of an instance: one per explicit delayed signal and,
    /// once some delay is nonzero, one hidden variable per other delayed
    /// terminal. Targets join `terminals`.
    fn timing_lines(
        &mut self,
        terminals: &mut Vec<IrTimingTerminal>,
        explicit: &BTreeMap<u32, DelayedTarget>,
        delays: &SignalDelays,
        delayed: bool,
    ) -> Result<Vec<IrTimingLine>, String> {
        let mut sources = explicit.keys().copied().collect::<Vec<_>>();
        if delayed {
            for (&terminal, &(rise, fall)) in &delays.delays {
                if (rise != 0 || fall != 0) && !explicit.contains_key(&terminal) {
                    sources.push(terminal);
                }
            }
        }
        sources.sort_unstable();
        let mut lines = Vec::with_capacity(sources.len());
        for source in sources {
            let (rise, fall) = if delayed {
                delays.delays.get(&source).copied().unwrap_or((0, 0))
            } else {
                (0, 0)
            };
            let line = lines.len() as u32;
            let (mut target, storage) = match explicit.get(&source) {
                Some(target) => (target.terminal, self.delayed_signal_storage(target)?),
                None => {
                    let width = terminals[source as usize].width();
                    let signal = self.model.signals.len();
                    self.model.signals.push(IrSignal {
                        fixed_default: None,
                        c_name: format!("llg_timing_delayed_{signal}"),
                        hdl_name: None,
                        ty: IrType::Packed {
                            width,
                            signed: false,
                            two_state: false,
                        },
                        net_driver: None,
                        net_alias: Vec::new(),
                        alias: None,
                        omit: false,
                    });
                    (
                        IrTimingTerminal::new(signal, 0, width),
                        IrTimingLineStorage::Variable,
                    )
                }
            };
            target.line = Some(line);
            terminals.push(target);
            let target = (terminals.len() - 1) as u32;
            lines.push(IrTimingLine::new(source, target, storage, rise, fall));
        }
        Ok(lines)
    }

    /// Where the runtime writes an explicit delayed signal: a driver slot of
    /// its own in the net's group (so it resolves with any other driver), or
    /// the variable itself.
    fn delayed_signal_storage(
        &mut self,
        target: &DelayedTarget,
    ) -> Result<IrTimingLineStorage, String> {
        let signal = self.model.signal(target.terminal.signal());
        let Some((group, _)) = signal.net_driver else {
            return Ok(IrTimingLineStorage::Variable);
        };
        let width = match signal.ty {
            IrType::Packed { width, .. } => width,
            _ => 0,
        };
        if !signal.net_alias.is_empty()
            || signal.alias.is_some()
            || self.model.net_groups[group].width != width
        {
            return Err(format!(
                "unsupported delayed signal `{}`: it shares storage with an alias or a wider \
                 network",
                self.display_name(target.node)
            ));
        }
        let driver = self.add_structural_driver(group, target.node, (6, 6))?;
        let slot = self
            .model
            .signal(driver)
            .net_driver
            .map(|(_, slot)| slot)
            .ok_or_else(|| "a delayed signal driver has no net slot".to_owned())?;
        Ok(IrTimingLineStorage::Net { group, slot })
    }

    /// Lower one check into its rows: roles of its event arguments (Tables
    /// 31-1..31-12), limits, events, conditions, flags and notifier, with
    /// located errors for the forms that cannot be simulated. `$setuphold`
    /// and `$recrem` also record their combined form.
    fn lower_timing_check(
        &mut self,
        scope: &str,
        check: &TimingCheck,
        terminals: &mut Vec<IrTimingTerminal>,
        conditions: &mut Vec<IrExpr>,
        lowered: &mut Vec<LoweredCheck>,
        combined: &mut Vec<CombinedCheck>,
    ) -> Result<(), String> {
        let location = self.specify_location(check.node);
        let name = check.kind.name();
        let error = |message: String| format!("timing check `{name}` at {location}: {message}");
        let kind = match check.kind {
            TimingCheckKind::Setup => IrTimingCheckKind::Setup,
            TimingCheckKind::Hold | TimingCheckKind::SetupHold => IrTimingCheckKind::Hold,
            TimingCheckKind::Recovery | TimingCheckKind::RecRem => IrTimingCheckKind::Recovery,
            TimingCheckKind::Removal => IrTimingCheckKind::Removal,
            TimingCheckKind::Skew => IrTimingCheckKind::Skew,
            TimingCheckKind::TimeSkew => IrTimingCheckKind::TimeSkew,
            TimingCheckKind::FullSkew => IrTimingCheckKind::FullSkew,
            TimingCheckKind::Period => IrTimingCheckKind::Period,
            TimingCheckKind::Width => IrTimingCheckKind::Width,
            TimingCheckKind::NoChange => IrTimingCheckKind::NoChange,
            TimingCheckKind::Unknown => return Err(error("the check is not recognized".into())),
        };
        let is_combined = matches!(
            check.kind,
            TimingCheckKind::SetupHold | TimingCheckKind::RecRem
        );
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
        let limit_node = |index: usize| check.limits.get(index).copied().flatten();
        let limit = |codegen: &mut Self, index: usize, required: bool| -> Result<i64, String> {
            match limit_node(index) {
                Some(expression) => codegen
                    .timing_limit_ticks_signed(check.node, expression)
                    .map_err(&error),
                None if required => Err(error(format!("limit argument {} is missing", index + 1))),
                None => Ok(0),
            }
        };
        // A negative limit of a check whose limits are non-negative counts
        // as zero, as the frontend warns (A9-D13); `$setuphold`/`$recrem`
        // limits are settled per instance and `$nochange` offsets keep their
        // sign (31.4.6).
        let first = limit(self, 0, true)?;
        let second = match kind {
            IrTimingCheckKind::Width => limit(self, 1, false)?,
            IrTimingCheckKind::NoChange | IrTimingCheckKind::FullSkew => limit(self, 1, true)?,
            _ if is_combined => limit(self, 1, true)?,
            _ => 0,
        };
        let (first, second) = if is_combined || kind == IrTimingCheckKind::NoChange {
            (first, second)
        } else {
            (first.max(0), second.max(0))
        };
        // `$timeskew`/`$fullskew` flags follow their limits (31.4.2,
        // 31.4.3).
        let mut flags = 0u8;
        if matches!(
            kind,
            IrTimingCheckKind::TimeSkew | IrTimingCheckKind::FullSkew
        ) {
            let base = if kind == IrTimingCheckKind::FullSkew {
                2
            } else {
                1
            };
            for (offset, flag) in [TIMING_EVENT_BASED, TIMING_REMAIN_ACTIVE]
                .into_iter()
                .enumerate()
            {
                if let Some(expression) = limit_node(base + offset) {
                    if self.timing_flag(expression).map_err(&error)? {
                        flags |= flag;
                    }
                }
            }
        }
        let lower_event = |codegen: &mut Self,
                           terminals: &mut Vec<IrTimingTerminal>,
                           conditions: &mut Vec<IrExpr>,
                           event: &TimingEvent|
         -> Result<IrTimingEvent, String> {
            let terminal = codegen.path_terminal(event.terminal).map_err(&error)?;
            let entry = IrTimingTerminal::new(terminal.info.ir, terminal.lsb, terminal.width);
            let index = match terminals.iter().position(|known| *known == entry) {
                Some(index) => index,
                None => {
                    terminals.push(entry);
                    terminals.len() - 1
                }
            };
            let condition = match event.condition {
                None => None,
                Some(expression) => Some(
                    codegen
                        .lower_timing_condition(scope, expression, conditions)
                        .map_err(&error)?,
                ),
            };
            Ok(IrTimingEvent::new(
                index as u32,
                event.edges.bits(),
                condition,
            ))
        };
        let reference_event = lower_event(self, terminals, conditions, &reference)?;
        let data_event = match (kind, data) {
            (_, Some(data)) => lower_event(self, terminals, conditions, &data)?,
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
            Some(expression) => Some(self.notifier_target(expression).map_err(&error)?),
        };
        let text = self.timing_check_text(name, kind, &reference, data.as_ref(), &[first, second]);
        if !is_combined {
            lowered.push(LoweredCheck {
                check: IrTimingCheck::new(
                    kind,
                    reference_event,
                    data_event,
                    first,
                    second,
                    None,
                    location,
                    text,
                )
                .with_flags(flags),
                notifier,
                source: check.kind,
                combined: None,
                node: check.node,
                slot: match kind {
                    IrTimingCheckKind::Width
                    | IrTimingCheckKind::NoChange
                    | IrTimingCheckKind::FullSkew => crate::sim::sdf::SdfLimitSlot::Both,
                    _ => crate::sim::sdf::SdfLimitSlot::First,
                },
            });
            return Ok(());
        }
        // `$setuphold(r, d, setup, hold)` is `$setup(d, r, setup)` plus
        // `$hold(r, d, hold)`; `$recrem(r, d, recovery, removal)` is
        // `$removal(r, d, removal)` plus `$recovery(r, d, recovery)`
        // (31.3.3, 31.3.6). The timestamp condition gates the stamping event
        // of each part and the timecheck condition its checking event
        // (31.9.2).
        let Some(data) = data else {
            return Err(error("event argument 2 is missing".into()));
        };
        let mut condition = |codegen: &mut Self, expression: Option<NodeId>| {
            expression
                .map(|expression| codegen.lower_timing_condition(scope, expression, conditions))
                .transpose()
                .map_err(&error)
        };
        let stamp = condition(self, check.stamp_condition)?;
        let timecheck = condition(self, check.check_condition)?;
        let (before_kind, after_kind, before, after) = if check.kind == TimingCheckKind::SetupHold {
            (
                IrTimingCheckKind::Setup,
                IrTimingCheckKind::Hold,
                first,
                second,
            )
        } else {
            (
                IrTimingCheckKind::Removal,
                IrTimingCheckKind::Recovery,
                second,
                first,
            )
        };
        let mut delayed = [None, None];
        for (slot, expression) in delayed.iter_mut().zip(check.delayed) {
            let Some(expression) = expression else {
                continue;
            };
            let terminal = self.path_terminal(expression).map_err(|message| {
                error(format!("the delayed signal is not simulated: {message}"))
            })?;
            *slot = Some(DelayedTarget {
                terminal: IrTimingTerminal::new(terminal.info.ir, terminal.lsb, terminal.width),
                node: terminal.node,
            });
        }
        let index = combined.len();
        combined.push(CombinedCheck {
            node: check.node,
            origin: location.clone(),
            name,
            reference: reference_event,
            data: data_event,
            before,
            after,
            delayed,
            text,
        });
        for (part, part_kind, limit) in [(true, before_kind, before), (false, after_kind, after)] {
            let (reference_condition, data_condition) = if part {
                (timecheck, stamp)
            } else {
                (stamp, timecheck)
            };
            let text = self.timing_check_text(
                ir_kind_name(part_kind),
                part_kind,
                &reference,
                Some(&data),
                &[limit.max(0)],
            );
            lowered.push(LoweredCheck {
                check: IrTimingCheck::new(
                    part_kind,
                    reference_event.with_condition2(reference_condition),
                    data_event.with_condition2(data_condition),
                    limit,
                    0,
                    None,
                    location.clone(),
                    text,
                ),
                notifier,
                source: check.kind,
                combined: Some((index, part)),
                node: check.node,
                // The part before the reference event holds the setup
                // limit of `$setuphold` (its first) and the removal limit
                // of `$recrem` (its second).
                slot: match (check.kind == TimingCheckKind::SetupHold, part) {
                    (true, true) | (false, false) => crate::sim::sdf::SdfLimitSlot::First,
                    _ => crate::sim::sdf::SdfLimitSlot::Second,
                },
            });
        }
        Ok(())
    }

    /// One event condition of a set, shared by equal expressions; `==` and
    /// `!=` conditions are nondeterministic (31.7).
    fn lower_timing_condition(
        &mut self,
        scope: &str,
        expression: NodeId,
        conditions: &mut Vec<IrExpr>,
    ) -> Result<IrTimingCondition, String> {
        let value = self
            .lower_expr(scope, expression)
            .map_err(|message| format!("the condition is not simulated: {message}"))?;
        if value.is_real() {
            return Err("the condition is not an integral value".into());
        }
        let index = match conditions.iter().position(|known| *known == value) {
            Some(index) => index,
            None => {
                conditions.push(value);
                conditions.len() - 1
            }
        };
        Ok(IrTimingCondition::new(
            index as u32,
            self.nondeterministic_condition(expression),
        ))
    }

    /// A `$timeskew`/`$fullskew` flag: set when its constant value is
    /// nonzero (31.4.2, 31.4.3).
    fn timing_flag(&mut self, expression: NodeId) -> Result<bool, String> {
        match self.eval_decl_value(expression) {
            Ok(Val::Bits(value)) => value.to_u128().map(|value| value != 0).ok_or_else(|| {
                "an event_based or remain_active flag must be a known value".to_owned()
            }),
            Ok(Val::Real(value)) => Ok(value != 0.0),
            Ok(Val::Str(_)) => {
                Err("an event_based or remain_active flag cannot be a string".to_owned())
            }
            Err(message) => Err(format!(
                "an event_based or remain_active flag must be a constant: {message}"
            )),
        }
    }

    /// Of several checks of one kind and source with the same reference and
    /// data events only the one with the larger limit (the first of equals)
    /// is simulated; the others are reported and dropped (decision A9-D6).
    fn drop_duplicate_timing_checks(
        &mut self,
        scope: &str,
        lowered: Vec<LoweredCheck>,
    ) -> Vec<LoweredCheck> {
        let key = |row: &LoweredCheck| {
            (
                row.source,
                row.check.kind,
                row.check.reference,
                row.check.data,
            )
        };
        let mut kept: Vec<LoweredCheck> = Vec::with_capacity(lowered.len());
        for candidate in lowered {
            let Some(index) = kept.iter().position(|known| key(known) == key(&candidate)) else {
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
                dropped.source.name(),
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
    fn notifier_target(&self, expression: NodeId) -> Result<usize, String> {
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
        Ok(info.ir)
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
    /// placeholders and its limits in ticks: `$setup(d:<data>, posedge
    /// c:<reference>, 10)`.
    fn timing_check_text(
        &self,
        name: &str,
        kind: IrTimingCheckKind,
        reference: &TimingEvent,
        data: Option<&TimingEvent>,
        limits: &[i64],
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
        let reference_event = reference;
        let reference = event(reference, REFERENCE_TIME);
        let mut arguments = match (name, data) {
            ("$setup", Some(data)) => vec![event(data, DATA_TIME), reference],
            (_, Some(data)) => vec![reference, event(data, DATA_TIME)],
            // The derived data event: the same edge for `$period`, the
            // opposite one for `$width`.
            (_, None) => {
                let edges = match kind {
                    IrTimingCheckKind::Width => opposite_edges(reference_event.edges.bits()),
                    _ => reference_event.edges.bits(),
                };
                let derived = format!(
                    "{}{}:{DATA_TIME}",
                    edges_text(edges),
                    self.terminal_text(reference_event.terminal)
                );
                vec![reference, derived]
            }
        };
        let count = match name {
            "$setuphold" | "$recrem" | "$width" | "$nochange" | "$fullskew" => 2,
            _ => 1,
        };
        arguments.extend(limits.iter().take(count).map(i64::to_string));
        format!("{name}({})", arguments.join(", "))
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

/// The system task a row of a combined check reports as.
fn ir_kind_name(kind: IrTimingCheckKind) -> &'static str {
    match kind {
        IrTimingCheckKind::Setup => "$setup",
        IrTimingCheckKind::Hold => "$hold",
        IrTimingCheckKind::Recovery => "$recovery",
        IrTimingCheckKind::Removal => "$removal",
        IrTimingCheckKind::Skew => "$skew",
        IrTimingCheckKind::TimeSkew => "$timeskew",
        IrTimingCheckKind::FullSkew => "$fullskew",
        IrTimingCheckKind::Period => "$period",
        IrTimingCheckKind::Width => "$width",
        IrTimingCheckKind::NoChange => "$nochange",
    }
}

/// The edge classes an event's transitions touch; no edge control is both.
fn edge_classes(edges: u8) -> impl Iterator<Item = usize> {
    let rise = edges == 0 || edges & TimingEdges::RISING != 0;
    let fall = edges == 0 || edges & TimingEdges::FALLING != 0;
    [(RISE, rise), (FALL, fall)]
        .into_iter()
        .filter_map(|(class, used)| used.then_some(class))
}

/// The smallest non-negative delays of the delayed signals (31.9.1). Each
/// check with limits `before` (setup, removal) and `after` (hold, recovery)
/// and `d = delay(reference) - delay(data)` for every pair of edge classes
/// its events touch needs `before + d >= a` and `after - d >= b`, where `a`
/// (`b`) is one precision unit when its limit is nonzero and the window
/// `before + after` is at least two units, otherwise 0: the shifted window
/// then keeps its exclusive end points on either side of the delayed
/// reference event. The constraints are difference constraints, solved as
/// longest paths (Bellman-Ford). A set without a solution has a positive
/// cycle; the most negative limit of the checks on it becomes 0 (31.9.1)
/// and the delays are solved again until a solution exists. A class no
/// check constrains takes the other class's delay.
pub(super) fn solve_signal_delays(checks: &mut [DelayConstraint]) -> SignalDelays {
    let mut zeroed = Vec::new();
    loop {
        let mut nodes: BTreeMap<(u32, usize), usize> = BTreeMap::new();
        let mut node = |terminal: u32, class: usize| {
            let next = nodes.len();
            *nodes.entry((terminal, class)).or_insert(next)
        };
        // `x[head] - x[tail] >= weight`, owned by a check.
        let mut edges: Vec<(usize, usize, i64, usize)> = Vec::new();
        for (index, check) in checks.iter().enumerate() {
            for class in [RISE, FALL] {
                node(check.reference, class);
                node(check.data, class);
            }
            let window = check.before + check.after;
            let margin = i64::from(window >= 2);
            let a = if check.before != 0 { margin } else { 0 };
            let b = if check.after != 0 { margin } else { 0 };
            for reference in edge_classes(check.reference_edges) {
                for data in edge_classes(check.data_edges) {
                    let r = node(check.reference, reference);
                    let d = node(check.data, data);
                    edges.push((r, d, a - check.before, index));
                    edges.push((d, r, b - check.after, index));
                }
            }
        }
        let count = nodes.len();
        let mut distance = vec![0i64; count];
        let mut predecessor: Vec<Option<usize>> = vec![None; count];
        let mut relaxed = None;
        for _ in 0..=count {
            relaxed = None;
            for (edge, &(head, tail, weight, _)) in edges.iter().enumerate() {
                if distance[tail] + weight > distance[head] {
                    distance[head] = distance[tail] + weight;
                    predecessor[head] = Some(edge);
                    relaxed = Some(head);
                }
            }
            if relaxed.is_none() {
                break;
            }
        }
        let Some(mut at) = relaxed else {
            let mut touched = vec![false; count];
            for &(head, tail, _, _) in &edges {
                touched[head] = true;
                touched[tail] = true;
            }
            let mut delays = BTreeMap::new();
            for (&(terminal, class), &index) in &nodes {
                if class != RISE {
                    continue;
                }
                let fall = nodes[&(terminal, FALL)];
                let (mut rise_delay, mut fall_delay) = (distance[index], distance[fall]);
                if !touched[index] {
                    rise_delay = fall_delay;
                }
                if !touched[fall] {
                    fall_delay = rise_delay;
                }
                delays.insert(
                    terminal,
                    (rise_delay.max(0) as u64, fall_delay.max(0) as u64),
                );
            }
            return SignalDelays { delays, zeroed };
        };
        // Walk back onto the positive cycle, then collect its checks.
        for _ in 0..count {
            if let Some(edge) = predecessor[at] {
                at = edges[edge].1;
            }
        }
        let start = at;
        let mut on_cycle = Vec::new();
        while let Some(edge) = predecessor[at] {
            on_cycle.push(edges[edge].3);
            at = edges[edge].1;
            if at == start || on_cycle.len() > edges.len() {
                break;
            }
        }
        // Only a negative limit gives a positive weight, so the cycle has
        // one; the guard keeps the loop finite whatever the input.
        let Some(&chosen) = on_cycle
            .iter()
            .filter(|&&index| checks[index].before < 0 || checks[index].after < 0)
            .min_by_key(|&&index| (checks[index].before.min(checks[index].after), index))
        else {
            return SignalDelays {
                delays: BTreeMap::new(),
                zeroed,
            };
        };
        let check = &mut checks[chosen];
        if check.before <= check.after {
            check.before = 0;
        } else {
            check.after = 0;
        }
        zeroed.push(chosen);
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

    fn check(
        reference: u32,
        data: u32,
        data_edges: u8,
        before: i64,
        after: i64,
    ) -> DelayConstraint {
        DelayConstraint {
            reference,
            reference_edges: TimingEdges::RISING,
            data,
            data_edges,
            before,
            after,
        }
    }

    #[test]
    fn single_checks_shift_by_the_negative_limit_and_margin() {
        // (setup, hold) -> delay(reference) - delay(data) for both data
        // classes.
        for (limits, expected) in [
            ((-5, 5), 5),
            ((-5, 6), 5),
            ((-5, 7), 6),
            ((5, -5), -5),
            ((6, -5), -5),
            ((7, -5), -6),
            ((-1, 3), 2),
            ((-1, 2), 1),
            ((-1, 1), 1),
            ((3, -1), -2),
            ((2, -1), -1),
            ((3, 8), 0),
        ] {
            let mut checks = [check(0, 1, 0, limits.0, limits.1)];
            let solved = solve_signal_delays(&mut checks);
            let (reference, data) = (solved.delays[&0], solved.delays[&1]);
            let shift = |data: u64| reference.0 as i64 - data as i64;
            assert_eq!(
                (shift(data.0), shift(data.1)),
                (expected, expected),
                "{limits:?}"
            );
            assert!(solved.zeroed.is_empty());
        }
    }

    #[test]
    fn delays_are_the_smallest_non_negative_solution() {
        // LRM Figure 31-6 at 100 ticks per unit: CP 10.01, D 0, TI 20.02,
        // TE 2.02 (terminals CP 0, D 1, TI 2, TE 3).
        let mut checks = [
            check(0, 1, 0, -1000, 2000),
            check(0, 2, 0, 2000, -1000),
            check(0, 3, 0, -400, 800),
        ];
        let solved = solve_signal_delays(&mut checks);
        assert_eq!(solved.delays[&0], (1001, 1001));
        assert_eq!(solved.delays[&1], (0, 0));
        assert_eq!(solved.delays[&2], (2002, 2002));
        assert_eq!(solved.delays[&3], (202, 202));
        assert!(solved.zeroed.is_empty());
    }

    #[test]
    fn edge_classes_get_their_own_delays() {
        // The larger negative setup of the two data edges sets the reference
        // delay (31.9.1); its hold bounds the other data edge's delay.
        let mut checks = [
            check(0, 1, TimingEdges::RISING, -3, 8),
            check(0, 1, TimingEdges::FALLING, -7, 13),
        ];
        let solved = solve_signal_delays(&mut checks);
        assert_eq!(solved.delays[&0], (8, 8));
        assert_eq!(solved.delays[&1], (1, 0));
    }

    #[test]
    fn inconsistent_limits_zero_the_most_negative_one_until_solvable() {
        // A must lag B by more than B lags A: the most negative limit (-6)
        // goes first, which is not enough, then the other one.
        let mut checks = [check(0, 1, 0, -5, 10), check(1, 0, 0, -6, 10)];
        let solved = solve_signal_delays(&mut checks);
        assert_eq!(solved.zeroed, vec![1, 0]);
        assert_eq!((checks[0].before, checks[1].before), (0, 0));
        assert!(solved.delays.values().all(|&delays| delays == (0, 0)));
        // One zeroed limit can be enough: B must not lag A (zero setup).
        let mut checks = [check(0, 1, 0, -5, 10), check(1, 0, 0, 0, 3)];
        let solved = solve_signal_delays(&mut checks);
        assert_eq!(solved.zeroed, vec![0]);
        assert!(solved.delays.values().all(|&delays| delays == (0, 0)));
    }

    #[test]
    fn non_negative_limits_need_no_delays() {
        let mut checks = [
            check(0, 1, 0, 3, 8),
            check(0, 2, 0, 0, 0),
            check(0, 3, 0, 1, 0),
        ];
        let solved = solve_signal_delays(&mut checks);
        assert!(solved.delays.values().all(|&delays| delays == (0, 0)));
        assert!(solved.zeroed.is_empty());
    }
}
