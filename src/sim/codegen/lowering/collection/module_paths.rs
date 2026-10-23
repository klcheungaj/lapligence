//! Module paths of specify blocks (IEEE 1364-2001 14.2-14.6,
//! IEEE 1800-2009 30.4-30.7).
//!
//! Every module instance with paths becomes one [`IrModulePathSet`]:
//! resolved source and destination storage ranges, the twelve-transition
//! delay table, edge and condition of each path, its `PATHPULSE$` limits and
//! the destination's pulse-style flags, plus one hidden process that records
//! source transitions. The destination is the instance's own net group, so
//! every reader of the net sees the path-delayed value (decision A7-D2).
//! Timing checks are lowered separately (`timing_checks.rs`).

use super::*;
use crate::core::db::{
    ModulePath, PathCondition, PathConnection, PathEdge, PathPolarity, PathPulse, PathPulseScope,
    PulseStyle, SpecifyItemKind,
};
use crate::sim::ir::{
    expand_path_delays, IrModulePath, IrModulePathDestination, IrModulePathSet, IrPathCondition,
    IrPathEdge, IrPathLimits, IrPathPolarity, PATH_STYLE_ON_DETECT, PATH_STYLE_SHOW_CANCELLED,
};

/// One path or timing check terminal: `width` storage bits of IR signal
/// `info.ir` from `lsb`.
pub(super) struct PathTerminal {
    /// The declaration the terminal names.
    pub(super) node: NodeId,
    pub(super) info: SignalInfo,
    pub(super) lsb: u32,
    pub(super) width: u32,
}

/// Distinct `(owner, source)` structural driver sites of each net group.
type GroupDrivers = HashMap<usize, BTreeSet<(usize, usize)>>;

/// The specify facts of one module instance.
#[derive(Default)]
struct InstanceSpecify<'d> {
    paths: Vec<&'d ModulePath>,
    styles: Vec<(NodeId, PulseStyle, &'d [NodeId])>,
    pulses: Vec<&'d PathPulse>,
}

/// Flags of a destination bit while its pulse-style declarations are read:
/// a style and a showcancelled choice, each fixed by the first declaration.
const STYLE_DECIDED: u8 = 4;
const CANCELLED_DECIDED: u8 = 8;

impl Codegen<'_> {
    pub(super) fn specify_location(&self, node: NodeId) -> String {
        format!(
            "{}:{}:{}",
            self.node(node).file.as_deref().unwrap_or("<unknown>"),
            self.node(node).line,
            self.node(node).col,
        )
    }

    pub(super) fn specify_unsupported(&self, node: NodeId, what: &str) -> String {
        format!(
            "unsupported {what} at {} in `{}`",
            self.specify_location(node),
            self.owning_inst(node)
                .map(|instance| self.source_path(&self.instance_path_of(instance)))
                .unwrap_or_default(),
        )
    }

    /// Lower the module paths of every elaborated instance. Runs after the
    /// net groups and their structural driver sites exist and before any
    /// driver process is lowered. A design without specify blocks costs one
    /// emptiness test.
    pub(in super::super) fn lower_module_paths(&mut self) -> Result<(), String> {
        let db = self.db;
        if db.specify_blocks().is_empty() {
            return Ok(());
        }
        let design = self.design_nodes().into_iter().collect::<HashSet<_>>();
        // Specify facts grouped by instance, in capture order.
        let mut instances: Vec<(NodeId, InstanceSpecify<'_>)> = Vec::new();
        let mut instance_index = HashMap::new();
        let mut slot_of = |codegen: &Self,
                           instances: &mut Vec<(NodeId, InstanceSpecify<'_>)>,
                           node: NodeId|
         -> Result<usize, String> {
            let instance = codegen.owning_inst(node).ok_or_else(|| {
                codegen.specify_unsupported(node, "specify item outside a module")
            })?;
            Ok(*instance_index.entry(instance).or_insert_with(|| {
                instances.push((instance, InstanceSpecify::default()));
                instances.len() - 1
            }))
        };
        for item in db.specify_items() {
            if !design.contains(&item.node) {
                continue;
            }
            match &item.kind {
                SpecifyItemKind::PulseStyle(style, terminals) => {
                    let slot = slot_of(self, &mut instances, item.node)?;
                    instances[slot]
                        .1
                        .styles
                        .push((item.node, *style, terminals));
                }
                // Lowered by `lower_timing_checks` (timing_checks.rs).
                SpecifyItemKind::TimingCheck(_) => {}
            }
        }
        for pulse in db.path_pulses() {
            if design.contains(&pulse.node) {
                let slot = slot_of(self, &mut instances, pulse.node)?;
                instances[slot].1.pulses.push(pulse);
            }
        }
        for path in db.module_paths() {
            if design.contains(&path.node) {
                let slot = slot_of(self, &mut instances, path.node)?;
                instances[slot].1.paths.push(path);
            }
        }
        instances.retain(|(_, specify)| !specify.paths.is_empty());
        if instances.is_empty() {
            return Ok(());
        }
        let mut drivers = GroupDrivers::new();
        for (owner, source, group) in self.structural_driver_sites.keys() {
            // A collapsed inout member keeps a slot of its own storage; the
            // assignments, gates and port connections that drive it are
            // separate sites.
            if matches!(self.kind(*source), NodeKind::Net { .. }) {
                continue;
            }
            drivers
                .entry(*group)
                .or_default()
                .insert((owner.index(), source.index()));
        }
        for (instance, specify) in instances {
            self.lower_instance_paths(instance, &specify, &drivers)?;
        }
        Ok(())
    }

    fn lower_instance_paths(
        &mut self,
        instance: NodeId,
        specify: &InstanceSpecify<'_>,
        drivers: &GroupDrivers,
    ) -> Result<(), String> {
        let previous_inst = self.inst;
        self.inst = instance;
        let result = self.lower_instance_paths_in(instance, specify, drivers);
        self.inst = previous_inst;
        result
    }

    fn lower_instance_paths_in(
        &mut self,
        instance: NodeId,
        specify: &InstanceSpecify<'_>,
        drivers: &GroupDrivers,
    ) -> Result<(), String> {
        let paths = &specify.paths;
        let scope = self.instance_path_of(instance);
        let mut sources: Vec<usize> = Vec::new();
        let mut source_index: HashMap<usize, usize> = HashMap::new();
        let mut reads = Vec::new();
        let mut destinations: Vec<IrModulePathDestination> = Vec::new();
        let mut destination_index: HashMap<usize, usize> = HashMap::new();
        let mut conditions = Vec::new();
        for path in paths {
            let location = self.specify_location(path.node);
            let error = |message: String| format!("module path at {location}: {message}");
            let values = path
                .delays
                .iter()
                .map(|delay| self.path_delay_ticks(path.node, *delay))
                .collect::<Result<Vec<_>, _>>()
                .map_err(error)?;
            let delays = expand_path_delays(&values).ok_or_else(|| {
                error(format!(
                    "a path delay has 1, 2, 3, 6 or 12 values, not {}",
                    values.len()
                ))
            })?;
            let full = path.connection == PathConnection::Full;
            if !full && (path.inputs.len() != 1 || path.outputs.len() != 1) {
                return Err(error(
                    "a parallel connection has one source and one destination".to_owned(),
                ));
            }
            let polarity = match path.polarity {
                PathPolarity::Unknown => IrPathPolarity::Unknown,
                PathPolarity::Positive => IrPathPolarity::Positive,
                PathPolarity::Negative => IrPathPolarity::Negative,
            };
            // The data source expression and the polarity describe the data
            // flow for timing analysis only (30.4.3, 30.4.7).
            let edge = match path.edge {
                None => IrPathEdge::None,
                Some(PathEdge::Posedge) => IrPathEdge::Posedge,
                Some(PathEdge::Negedge) => IrPathEdge::Negedge,
                Some(PathEdge::Edge) => IrPathEdge::Any,
            };
            let condition = match path.condition {
                PathCondition::None => IrPathCondition::None,
                PathCondition::IfNone => IrPathCondition::IfNone,
                PathCondition::If(expression) => {
                    let value = self.lower_expr(&scope, expression).map_err(|message| {
                        error(format!("the path condition is not simulated: {message}"))
                    })?;
                    if value.is_real() {
                        return Err(error("the path condition is not an integral value".into()));
                    }
                    conditions.push(value);
                    IrPathCondition::If(conditions.len() - 1)
                }
            };
            let inputs = path
                .inputs
                .iter()
                .map(|input| self.path_terminal(*input))
                .collect::<Result<Vec<_>, _>>()
                .map_err(error)?;
            let limits = self
                .path_pulse_limits(path, &specify.pulses)
                .map_err(error)?;
            let mut outputs = Vec::with_capacity(path.outputs.len());
            for output in &path.outputs {
                let terminal = self.path_terminal(*output).map_err(error)?;
                let Some(group) =
                    self.path_destination_group(instance, &terminal, drivers, &location)?
                else {
                    // An undriven destination never transitions, so its
                    // paths have no effect.
                    continue;
                };
                outputs.push((terminal, group));
            }
            for input in &inputs {
                let signal = input.info.ir;
                let index = match source_index.get(&signal) {
                    Some(index) => *index,
                    None => {
                        source_index.insert(signal, sources.len());
                        sources.push(signal);
                        reads.push(self.signal_dependency(&input.info));
                        sources.len() - 1
                    }
                };
                for (output, group) in &outputs {
                    if !full && input.width != output.width {
                        return Err(error(format!(
                            "parallel connection terminals differ in width ({} and {})",
                            input.width, output.width
                        )));
                    }
                    let slot = *destination_index.entry(*group).or_insert_with(|| {
                        destinations.push(IrModulePathDestination::new(*group, Vec::new()));
                        destinations.len() - 1
                    });
                    let mut module_path = IrModulePath::new(
                        index,
                        input.lsb,
                        input.width,
                        output.lsb,
                        output.width,
                        full,
                        polarity,
                        delays,
                        location.clone(),
                    )
                    .with_trigger(edge, condition);
                    if let Some(limits) = &limits {
                        module_path = module_path.with_limits(limits.clone());
                    }
                    destinations[slot].paths.push(module_path);
                }
            }
        }
        if destinations.is_empty() {
            return Ok(());
        }
        for destination in &mut destinations {
            group_ifnone_paths(&mut destination.paths);
        }
        self.apply_pulse_styles(&specify.styles, &mut destinations, &destination_index)?;
        let set = self.model.module_paths.len();
        let label = self.source_path(&scope);
        self.model.module_paths.push(
            IrModulePathSet::new(label.clone(), sources, destinations).with_conditions(conditions),
        );
        reads.sort_by_key(|dependency| self.dependency_sort_key(dependency));
        reads.dedup();
        let block = paths[0].block;
        let name = self.new_fn_name(&scope, "path_stamp");
        let origin = self.origin(block);
        self.model.processes.push(IrProcess::new_with_origin(
            name,
            format!("{label}.specify"),
            IrShape::SensLoop { reads },
            Vec::new(),
            vec![IrStmt::ModulePathStamp { set }],
            origin,
        ));
        Ok(())
    }

    /// The `PATHPULSE$` limits of a path declaration (30.7.1): the specparam
    /// that names the declaration's first input and first output terminal,
    /// else the module-wide one, else none (the global percentages apply).
    /// A missing error limit equals the reject limit; an error limit below
    /// the reject limit lowers the reject limit to it.
    fn path_pulse_limits(
        &mut self,
        path: &ModulePath,
        pulses: &[&PathPulse],
    ) -> Result<Option<IrPathLimits>, String> {
        if pulses.is_empty() {
            return Ok(None);
        }
        let (Some(input), Some(output)) = (path.inputs.first(), path.outputs.first()) else {
            return Ok(None);
        };
        let input = self.path_terminal(*input)?.node;
        let output = self.path_terminal(*output)?.node;
        let specific = pulses
            .iter()
            .find(|pulse| pulse.scope == PathPulseScope::Path { input, output });
        let Some(pulse) = specific.or_else(|| {
            pulses
                .iter()
                .find(|pulse| pulse.scope == PathPulseScope::Module)
        }) else {
            return Ok(None);
        };
        let Some(reject) = &pulse.reject else {
            return Ok(None);
        };
        let reject = self.path_limit_ticks(pulse.node, reject)?;
        let error = match &pulse.error {
            Some(error) => self.path_limit_ticks(pulse.node, error)?,
            None => reject,
        };
        Ok(Some(IrPathLimits::uniform(reject.min(error), error)))
    }

    /// Set the destination bits' style flags from the instance's
    /// `pulsestyle_*` and `showcancelled` declarations (30.7.4). For each bit
    /// the first declaration of each kind wins; a terminal that is not a path
    /// destination has nothing to style.
    fn apply_pulse_styles(
        &self,
        styles: &[(NodeId, PulseStyle, &[NodeId])],
        destinations: &mut [IrModulePathDestination],
        destination_index: &HashMap<usize, usize>,
    ) -> Result<(), String> {
        for (node, style, terminals) in styles {
            let location = self.specify_location(*node);
            for terminal in *terminals {
                let terminal = self
                    .path_terminal(*terminal)
                    .map_err(|message| format!("pulse style at {location}: {message}"))?;
                let Some(&slot) = self
                    .model
                    .signal(terminal.info.ir)
                    .net_driver
                    .and_then(|(group, _)| destination_index.get(&group))
                else {
                    continue;
                };
                let width = self.model.net_groups[destinations[slot].group].width as usize;
                let flags = destinations[slot]
                    .styles
                    .get_or_insert_with(|| vec![0; width]);
                let (decided, set) = match style {
                    PulseStyle::OnEvent => (STYLE_DECIDED, 0),
                    PulseStyle::OnDetect => (STYLE_DECIDED, PATH_STYLE_ON_DETECT),
                    PulseStyle::ShowCancelled => (CANCELLED_DECIDED, PATH_STYLE_SHOW_CANCELLED),
                    PulseStyle::NoShowCancelled => (CANCELLED_DECIDED, 0),
                };
                let low = terminal.lsb as usize;
                for bit in flags.iter_mut().skip(low).take(terminal.width as usize) {
                    if *bit & decided == 0 {
                        *bit |= decided | set;
                    }
                }
            }
        }
        for destination in destinations {
            if let Some(flags) = &mut destination.styles {
                for bit in flags.iter_mut() {
                    *bit &= PATH_STYLE_ON_DETECT | PATH_STYLE_SHOW_CANCELLED;
                }
                if flags.iter().all(|bit| *bit == 0) {
                    destination.styles = None;
                }
            }
        }
        Ok(())
    }

    /// Resolve a terminal expression to storage bits: a whole signal or a
    /// constant bit, part or indexed part select of a one-dimensional packed
    /// signal.
    pub(super) fn path_terminal(&self, expression: NodeId) -> Result<PathTerminal, String> {
        let target = |base: NodeId| -> Result<NodeId, String> {
            match self.kind(base) {
                NodeKind::Net { .. } | NodeKind::Var { .. } => Ok(base),
                NodeKind::Expr(ExprKind::Ref {
                    target: Some(target),
                }) => Ok(*target),
                _ => Err(
                    "the terminal is not a whole port or a constant bit or part select of one \
                     (not simulated yet)"
                        .to_owned(),
                ),
            }
        };
        let (base, signal_node) = match self.kind(expression) {
            NodeKind::Expr(
                ExprKind::BitSelect { base, .. }
                | ExprKind::PartSelect { base, .. }
                | ExprKind::IndexedPartSelect { base, .. },
            ) => (*base, target(*base)?),
            _ => (expression, target(expression)?),
        };
        let info = self.signal_of(signal_node).cloned().ok_or_else(|| {
            format!(
                "terminal `{}` has no simulated storage",
                self.node(signal_node).name
            )
        })?;
        if info.real || info.shortreal {
            return Err(format!(
                "terminal `{}` is not a packed signal",
                self.node(signal_node).name
            ));
        }
        let whole = || PathTerminal {
            node: signal_node,
            info: info.clone(),
            lsb: 0,
            width: info.width,
        };
        let selected = |left: i128, right: i128| -> Result<PathTerminal, String> {
            if self
                .packed_ranges_for_base(base)
                .is_some_and(|ranges| ranges.len() > 1)
            {
                return Err(format!(
                    "a select of the multidimensional packed terminal `{}` is not supported yet",
                    self.node(signal_node).name
                ));
            }
            let left = self.packed_relative_bound(base, left)?;
            let right = self.packed_relative_bound(base, right)?;
            let (low, high) = (left.min(right), left.max(right));
            if low < 0 || high >= i128::from(info.width) {
                return Err(format!(
                    "terminal select of `{}` is outside its range",
                    self.node(signal_node).name
                ));
            }
            Ok(PathTerminal {
                node: signal_node,
                info: info.clone(),
                lsb: u32::try_from(low).map_err(|_| "terminal select overflows".to_owned())?,
                width: u32::try_from(high - low + 1)
                    .map_err(|_| "terminal select overflows".to_owned())?,
            })
        };
        match self.kind(expression) {
            NodeKind::Expr(ExprKind::BitSelect { index, .. }) => {
                let label = self.eval_bound_i128(*index)?;
                selected(label, label)
            }
            NodeKind::Expr(ExprKind::PartSelect { left, right, .. }) => {
                selected(self.eval_bound_i128(*left)?, self.eval_bound_i128(*right)?)
            }
            NodeKind::Expr(ExprKind::IndexedPartSelect {
                base_expr,
                width_expr,
                neg,
                ..
            }) => {
                let start = self.eval_bound_i128(*base_expr)?;
                let width = self.eval_bound_i128(*width_expr)?;
                if width <= 0 {
                    return Err("indexed part-select terminal width must be positive".to_owned());
                }
                let end = if *neg {
                    start.checked_sub(width - 1)
                } else {
                    start.checked_add(width - 1)
                }
                .ok_or_else(|| "terminal select overflows".to_owned())?;
                selected(start, end)
            }
            _ => Ok(whole()),
        }
    }

    /// The net group a destination terminal delays, after the destination
    /// rules: a net (variables are not simulated as destinations yet) whose
    /// whole storage is one group, driven only inside this instance, and in
    /// IEEE 1364-2001 by one driver at most (14.2.1). `None` for an
    /// undriven net.
    fn path_destination_group(
        &self,
        instance: NodeId,
        terminal: &PathTerminal,
        drivers: &GroupDrivers,
        location: &str,
    ) -> Result<Option<usize>, String> {
        let signal = self.model.signal(terminal.info.ir);
        let name = self.display_name(terminal.node);
        let shown = self.source_path(&self.instance_path_of(instance));
        let unsupported = |message: &str| {
            format!("unsupported module path destination `{name}` at {location} in `{shown}`: {message}")
        };
        let Some((group, _)) = signal.net_driver else {
            if matches!(self.kind(terminal.node), NodeKind::Net { .. }) {
                return Ok(None);
            }
            return Err(unsupported(
                "a variable destination is not simulated yet; declare the output as a net",
            ));
        };
        if !signal.net_alias.is_empty()
            || signal.alias.is_some()
            || self.model.net_groups[group].width != terminal.info.width
        {
            return Err(unsupported(
                "the destination net shares storage with an alias or a wider network",
            ));
        }
        let sites = drivers.get(&group);
        for (owner, _) in sites.into_iter().flatten() {
            if !self.node_within(NodeId::from_index(*owner), instance) {
                return Err(unsupported(
                    "the destination is also driven outside the module, so the path would delay \
                     that driver too",
                ));
            }
        }
        if self.db.edition() == LanguageEdition::Verilog2001 && sites.is_some_and(|s| s.len() > 1) {
            return Err(format!(
                "module path destination `{name}` at {location} in `{shown}` has more than one \
                 driver inside the module (IEEE 1364-2001 14.2.1: the module path destination \
                 shall have only one driver inside the module)"
            ));
        }
        Ok(Some(group))
    }

    /// Whether `node` is `ancestor` or below it.
    fn node_within(&self, node: NodeId, ancestor: NodeId) -> bool {
        let mut current = Some(node);
        while let Some(id) = current {
            if id == ancestor {
                return true;
            }
            current = self.node(id).parent;
        }
        false
    }
}

/// Number the paths whose terminals equal those of an `ifnone` path, so the
/// runtime can let the `ifnone` path apply only when no other path of its
/// group is active (30.4.4.4). An unconditional simple path with the same
/// terminals as an `ifnone` path is dropped: the language makes the pair
/// illegal and the `ifnone` path is the one kept; of several `ifnone`
/// declarations of one path the last one is kept (decision A8-D3).
fn group_ifnone_paths(paths: &mut Vec<IrModulePath>) {
    type Terminals = (usize, u32, u32, u32, u32);
    let terminals = |path: &IrModulePath| -> Terminals {
        (
            path.source,
            path.source_lsb,
            path.source_width,
            path.dest_lsb,
            path.dest_width,
        )
    };
    let mut groups: Vec<Terminals> = Vec::new();
    for path in paths.iter() {
        if path.condition == IrPathCondition::IfNone && !groups.contains(&terminals(path)) {
            groups.push(terminals(path));
        }
    }
    if groups.is_empty() {
        return;
    }
    // The last `ifnone` declaration of a path replaces earlier ones.
    let mut last_ifnone = vec![0usize; groups.len()];
    for (index, path) in paths.iter().enumerate() {
        if path.condition == IrPathCondition::IfNone {
            if let Some(group) = groups.iter().position(|group| *group == terminals(path)) {
                last_ifnone[group] = index;
            }
        }
    }
    let mut index = 0;
    paths.retain(|path| {
        let position = index;
        index += 1;
        let group = groups.iter().position(|group| *group == terminals(path));
        match (path.condition, group) {
            (IrPathCondition::IfNone, Some(group)) => last_ifnone[group] == position,
            (IrPathCondition::None, Some(_)) => path.edge != IrPathEdge::None,
            _ => true,
        }
    });
    for path in paths.iter_mut() {
        if let Some(index) = groups.iter().position(|group| *group == terminals(path)) {
            path.group = u32::try_from(index + 1).unwrap_or(u32::MAX);
        }
    }
}
