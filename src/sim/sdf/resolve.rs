//! Resolution of SDF records against a model's [`SdfTargets`] (IEEE
//! 1800-2009 32.3-32.5, IEEE 1364-2001 16.2-16.4).
//!
//! The file is read cell by cell; each record is matched and appended to
//! the plan, so memory follows the plan, not the file. A malformed file
//! yields its first error and no plan. A record that matches nothing is a
//! warning and adds no op. Names are only compared with the in-memory
//! targets: an escaped identifier or a hierarchy divider never reaches the
//! file system.
//!
//! Matching rules:
//!
//! * A CELL's INSTANCE path walks down from the annotation scope; `[n]`
//!   selects an element of an instance array or loop generate block. The
//!   region's module (or primitive) name must equal CELLTYPE (OVI SDF 3.0
//!   3-9). `*` takes every instance of CELLTYPE at or below the scope.
//! * IOPATH (32.4.1): the input and output names (and bit specs, if given)
//!   select declared path terminals. An SDF edge must equal the path's
//!   edge; without one any edge matches. COND matches paths whose `if`
//!   condition is the same expression tree; CONDELSE matches `ifnone`
//!   paths; an IOPATH without condition matches every path between the
//!   ports. PATHPULSE matches paths regardless of edge and condition.
//! * Timing checks follow Table 32-2. An SDF edge must equal the declared
//!   event's edges and an SDF COND its `&&&` condition; what SDF leaves out
//!   matches anything (32.4.2). SCOND/CCOND compare with the stamp and check
//!   conditions and take precedence over COND (OVI SDF 3.0 3-27).
//! * PORT and INTERCONNECT name input/inout loads and output/inout sources
//!   (32.4.4); an INTERCONNECT whose source is not found still annotates
//!   its load, with a warning.

use super::expr::SdfBits;
use super::parser::SdfReader;
use super::plan::{
    SdfDelayOp, SdfLimitSlot, SdfMode, SdfOp, SdfOpKind, SdfPlan, SdfPortTarget, SdfPulseOp,
    SdfWarning,
};
use super::syntax::*;
use super::targets::*;
use super::{SdfError, SdfLimits, SdfPos};
use std::collections::HashMap;
use std::io::BufRead;

/// Read `input` and resolve it from scope `root` of `targets`.
pub fn resolve<R: BufRead>(
    input: R,
    targets: &SdfTargets,
    root: u32,
    limits: SdfLimits,
) -> Result<SdfPlan, SdfError> {
    let mut reader = SdfReader::new(input, limits)?;
    let header = reader.header().clone();
    let mut resolver = Resolver {
        targets,
        root,
        limits,
        children: HashMap::new(),
        by_cell_type: None,
        plan: SdfPlan {
            version: header.version,
            timescale: header.timescale,
            root,
            ops: Vec::new(),
            delay_lists: Vec::new(),
            warnings: Vec::new(),
        },
    };
    for (index, scope) in targets.scopes().iter().enumerate() {
        if let Some(parent) = scope.parent {
            resolver
                .children
                .entry((parent, scope.name.as_str(), scope.index))
                .or_insert(index as u32);
        }
    }
    while let Some(cell) = reader.next_cell()? {
        resolver.cell(&cell)?;
    }
    let mut warnings = reader.take_warnings();
    warnings.append(&mut resolver.plan.warnings);
    warnings.sort_by_key(|warning| warning.pos);
    resolver.plan.warnings = warnings;
    Ok(resolver.plan)
}

struct Resolver<'t> {
    targets: &'t SdfTargets,
    root: u32,
    limits: SdfLimits,
    /// `(parent, name, index)` to child scope.
    children: HashMap<(u32, &'t str, Option<i64>), u32>,
    /// Scopes at or below the root by cell type, built at the first
    /// wildcard cell.
    by_cell_type: Option<HashMap<&'t str, Vec<u32>>>,
    plan: SdfPlan,
}

/// The TimingEdges mask of an SDF edge (z counts as x).
fn edge_mask(edge: SdfEdge) -> u8 {
    const T01: u8 = 1;
    const T0X: u8 = 2;
    const T10: u8 = 4;
    const T1X: u8 = 8;
    const TX0: u8 = 16;
    const TX1: u8 = 32;
    match edge {
        SdfEdge::Posedge => T01 | T0X | TX1,
        SdfEdge::Negedge => T10 | T1X | TX0,
        SdfEdge::E01 => T01,
        SdfEdge::E10 => T10,
        SdfEdge::E0z => T0X,
        SdfEdge::Ez1 => TX1,
        SdfEdge::E1z => T1X,
        SdfEdge::Ez0 => TX0,
    }
}

fn path_text(components: &[SdfIdent]) -> String {
    components
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(".")
}

/// Whether an SDF port name and bit spec select a declared terminal: the
/// names are equal and the bit spec, when given, equals the terminal's
/// selected indices (or the whole port's range when the terminal is the
/// whole port).
fn terminal_matches(terminal: &SdfTerminal, port: &SdfIdent, whole: Option<(i64, i64)>) -> bool {
    if terminal.name != port.name {
        return false;
    }
    let Some(bits) = port.bits else {
        return true;
    };
    let (msb, lsb) = bits.bounds();
    let wanted = (msb as i64, lsb as i64);
    match terminal.range.or(whole) {
        Some(range) => range == wanted,
        None => false,
    }
}

impl<'t> Resolver<'t> {
    fn warn(&mut self, pos: SdfPos, message: String) {
        self.plan.warnings.push(SdfWarning::new(pos, message));
    }

    fn push(&mut self, pos: SdfPos, kind: SdfOpKind) -> Result<(), SdfError> {
        if self.plan.ops.len() >= self.limits.max_plan_ops {
            return Err(SdfError::new(
                pos,
                format!(
                    "the annotation has more than {} operations",
                    self.limits.max_plan_ops
                ),
            ));
        }
        self.plan.ops.push(SdfOp { pos, kind });
        Ok(())
    }

    fn delay_op(&mut self, mode: SdfMode, list: &SdfDelvalList) -> SdfDelayOp {
        let index = self.plan.delay_lists.len() as u32;
        self.plan.delay_lists.push(list.clone());
        SdfDelayOp { mode, list: index }
    }

    fn child(&self, scope: u32, component: &SdfIdent) -> Option<u32> {
        let index = match component.bits {
            None => None,
            Some(SdfBits::Bit(bit)) => Some(i64::try_from(bit).ok()?),
            Some(SdfBits::Range(..)) => return None,
        };
        self.children
            .get(&(scope, component.name.as_str(), index))
            .copied()
    }

    fn walk(&self, from: u32, components: &[SdfIdent]) -> Option<u32> {
        components
            .iter()
            .try_fold(from, |scope, component| self.child(scope, component))
    }

    fn scope_name(&self, scope: u32) -> String {
        self.targets.path_of(scope)
    }

    fn cell(&mut self, cell: &SdfCell) -> Result<(), SdfError> {
        let regions = match &cell.instance {
            SdfCellInstance::Wildcard => {
                let map = self.by_cell_type.get_or_insert_with(|| {
                    let mut map: HashMap<&'t str, Vec<u32>> = HashMap::new();
                    for scope in self.targets.subtree(self.root) {
                        let entry = self.targets.scope(scope);
                        if entry.kind != SdfScopeKind::Generate {
                            map.entry(entry.cell_type.as_str()).or_default().push(scope);
                        }
                    }
                    map
                });
                let regions = map
                    .get(cell.cell_type.as_str())
                    .cloned()
                    .unwrap_or_default();
                if regions.is_empty() {
                    let message = format!(
                        "no instance of cell type \"{}\" in `{}`; the CELL is not annotated",
                        cell.cell_type,
                        self.scope_name(self.root)
                    );
                    self.warn(cell.pos, message);
                }
                regions
            }
            SdfCellInstance::Path(components) => {
                let Some(region) = self.walk(self.root, components) else {
                    let message = format!(
                        "instance `{}` not found in `{}`; the CELL is not annotated",
                        path_text(components),
                        self.scope_name(self.root)
                    );
                    self.warn(cell.pos, message);
                    return Ok(());
                };
                let found = self.targets.scope(region);
                // A generate block is a level of the hierarchy, not a cell.
                if found.kind == SdfScopeKind::Generate || found.cell_type != cell.cell_type {
                    let message = format!(
                        "`{}` is {}, not cell type \"{}\"; the CELL is not annotated",
                        self.scope_name(region),
                        match found.kind {
                            SdfScopeKind::Generate => "a generate block".to_owned(),
                            _ => format!("an instance of `{}`", found.cell_type),
                        },
                        cell.cell_type
                    );
                    self.warn(cell.pos, message);
                    return Ok(());
                }
                vec![region]
            }
        };
        for region in regions {
            for spec in &cell.specs {
                match spec {
                    SdfTimingSpec::Delay(deltypes) => {
                        for deltype in deltypes {
                            self.deltype(region, deltype)?;
                        }
                    }
                    SdfTimingSpec::TimingCheck(checks) => {
                        for check in checks {
                            self.timing_check(region, check)?;
                        }
                    }
                    SdfTimingSpec::TimingEnv => {}
                }
            }
        }
        Ok(())
    }

    // ----- delays --------------------------------------------------------

    fn deltype(&mut self, region: u32, deltype: &SdfDelType) -> Result<(), SdfError> {
        match deltype {
            SdfDelType::Absolute(defs) => {
                for def in defs {
                    self.del_def(region, SdfMode::Absolute, def)?;
                }
            }
            SdfDelType::Increment(defs) => {
                for def in defs {
                    self.del_def(region, SdfMode::Increment, def)?;
                }
            }
            SdfDelType::PathPulse {
                pos,
                percent,
                path,
                reject,
                error,
            } => {
                if *percent {
                    for value in [reject, error].into_iter().flatten() {
                        for set in 0..3 {
                            if let Some(number) = value.member(set) {
                                if number.cmp_exact(&super::number::SdfNumber::new(100, 0))
                                    == std::cmp::Ordering::Greater
                                {
                                    let message =
                                        format!("the PATHPULSEPERCENT limit {number} is above 100");
                                    self.warn(*pos, message);
                                }
                            }
                        }
                    }
                }
                let pulse = SdfPulseOp {
                    percent: *percent,
                    reject: *reject,
                    error: *error,
                };
                let matches = match path {
                    Some(pair) => {
                        let (input, output) = &**pair;
                        let Some(scope) = self.port_scope(region, input, output, *pos, "PATHPULSE")
                        else {
                            return Ok(());
                        };
                        let found = self.path_matches(scope, None, &input.port, &output.port, None);
                        if found.is_empty() {
                            let message = format!(
                                "no specify path from `{}` to `{}` in `{}`; the PATHPULSE is not \
                                 annotated",
                                input,
                                output,
                                self.scope_name(scope)
                            );
                            self.warn(*pos, message);
                        }
                        found.into_iter().map(|row| (scope, row)).collect()
                    }
                    None => self.all_paths(region, None),
                };
                if matches.is_empty() && path.is_none() {
                    let message = format!(
                        "`{}` has no specify paths; the PATHPULSE is not annotated",
                        self.scope_name(region)
                    );
                    self.warn(*pos, message);
                }
                for (scope, (path, input, output)) in matches {
                    self.push(
                        *pos,
                        SdfOpKind::PathPulse {
                            scope,
                            path,
                            input,
                            output,
                            pulse,
                        },
                    )?;
                }
            }
        }
        Ok(())
    }

    /// The scope of an input/output port pair, which must be the same.
    fn port_scope(
        &mut self,
        region: u32,
        input: &SdfPortRef,
        output: &SdfPortRef,
        pos: SdfPos,
        what: &str,
    ) -> Option<u32> {
        let Some(scope) = self.walk(region, &input.path) else {
            let message = format!(
                "instance `{}` not found in `{}`; the {what} is not annotated",
                path_text(&input.path),
                self.scope_name(region)
            );
            self.warn(pos, message);
            return None;
        };
        if input.path != output.path {
            let message = format!(
                "the {what} ports `{input}` and `{output}` are in different instances; it is not \
                 annotated"
            );
            self.warn(pos, message);
            return None;
        }
        Some(scope)
    }

    /// Declared paths of `scope` from `input` to `output`, as
    /// `(path, input terminal, output terminal)`. `edge` and `condition`
    /// restrict them when given (`None` matches any).
    fn path_matches(
        &self,
        scope: u32,
        edge: Option<SdfEdge>,
        input: &SdfIdent,
        output: &SdfIdent,
        condition: Option<&SdfPathCondition>,
    ) -> Vec<(u32, u32, u32)> {
        let entry = self.targets.scope(scope);
        let whole = |name: &str| {
            entry
                .ports
                .iter()
                .find(|port| port.name == name)
                .and_then(|port| port.range)
        };
        let mut found = Vec::new();
        for (index, path) in entry.paths.iter().enumerate() {
            let edge_ok = match edge {
                None => true,
                Some(SdfEdge::Posedge) => path.edge == SdfTargetEdge::Posedge,
                Some(SdfEdge::Negedge) => path.edge == SdfTargetEdge::Negedge,
                Some(_) => false,
            };
            let condition_ok = match condition {
                None | Some(SdfPathCondition::None) => true,
                Some(SdfPathCondition::Cond(condition)) => matches!(
                    &path.condition,
                    SdfTargetCondition::If(Some(expr)) if *expr == condition.expr
                ),
                Some(SdfPathCondition::CondElse) => path.condition == SdfTargetCondition::IfNone,
            };
            if !edge_ok || !condition_ok {
                continue;
            }
            for (i, terminal) in path.inputs.iter().enumerate() {
                if !terminal_matches(terminal, input, whole(&terminal.name)) {
                    continue;
                }
                for (o, out) in path.outputs.iter().enumerate() {
                    if terminal_matches(out, output, whole(&out.name)) {
                        found.push((index as u32, i as u32, o as u32));
                    }
                }
            }
        }
        found
    }

    /// Every declared path of `scope` (all terminal pairs), optionally only
    /// those into output `output`.
    fn all_paths(&self, scope: u32, output: Option<&str>) -> Vec<(u32, (u32, u32, u32))> {
        let mut found = Vec::new();
        for (index, path) in self.targets.scope(scope).paths.iter().enumerate() {
            for i in 0..path.inputs.len() {
                for (o, terminal) in path.outputs.iter().enumerate() {
                    if output.is_none_or(|name| terminal.name == name) {
                        found.push((scope, (index as u32, i as u32, o as u32)));
                    }
                }
            }
        }
        found
    }

    /// A port of an instance below `region`, with its direction checked.
    /// The load ports a NETDELAY naming a net annotates (32.4.4: every
    /// load port connected to the net, or to its selected bits); `None`
    /// when the name is a port of its scope or no net, which
    /// [`Self::port_target`] then resolves.
    fn net_loads(
        &mut self,
        region: u32,
        net: &SdfPortRef,
        pos: SdfPos,
    ) -> Option<Vec<SdfPortTarget>> {
        let scope = self.walk(region, &net.path)?;
        let entry = self.targets.scope(scope);
        if entry.ports.iter().any(|port| port.name == net.port.name) {
            return None;
        }
        let declared = entry
            .nets
            .iter()
            .find(|known| known.name == net.port.name)?;
        let selected = match net.port.bits {
            None => None,
            Some(bits) => {
                let (msb, lsb) = bits.bounds();
                let (msb, lsb) = (msb as i64, lsb as i64);
                let inside = |bit: i64| match declared.range {
                    Some((a, b)) => bit >= a.min(b) && bit <= a.max(b),
                    None => false,
                };
                if !inside(msb) || !inside(lsb) {
                    let message = format!(
                        "NETDELAY `{net}` selects bits outside the net's declared range; the \
                         NETDELAY is not annotated"
                    );
                    self.warn(pos, message);
                    return Some(Vec::new());
                }
                Some((msb.min(lsb), msb.max(lsb)))
            }
        };
        let mut loads = Vec::new();
        for load in &declared.loads {
            let port = &self.targets.scope(load.scope).ports[load.port as usize];
            let bits = match (selected, load.net_bits) {
                (None, _) => None,
                (Some(_), None) => None,
                (Some((low, high)), Some((left, right))) => {
                    let first = low.max(left.min(right));
                    let last = high.min(left.max(right));
                    if first > last {
                        continue;
                    }
                    // Net index to port index: the actual's bits connect
                    // to the port's bits from left to right.
                    let port_bit = |bit: i64| match port.range {
                        Some((port_left, port_right)) => {
                            let distance = (bit - left).abs();
                            if port_left >= port_right {
                                port_left - distance
                            } else {
                                port_left + distance
                            }
                        }
                        None => 0,
                    };
                    port.range.map(|_| (port_bit(last), port_bit(first)))
                }
            };
            loads.push(SdfPortTarget {
                scope: load.scope,
                port: load.port,
                bits,
            });
        }
        if loads.is_empty() {
            let message = format!("net `{net}` has no load ports; the NETDELAY is not annotated");
            self.warn(pos, message);
        }
        Some(loads)
    }

    fn port_target(
        &mut self,
        region: u32,
        port: &SdfPortRef,
        allowed: &[SdfPortDirection],
        role: &str,
        warn: bool,
    ) -> Option<SdfPortTarget> {
        let Some(scope) = self.walk(region, &port.path) else {
            if warn {
                let message = format!(
                    "instance `{}` of {role} `{port}` not found in `{}`",
                    path_text(&port.path),
                    self.scope_name(region)
                );
                self.warn(port.pos, message);
            }
            return None;
        };
        let entry = self.targets.scope(scope);
        let Some(index) = entry.ports.iter().position(|p| p.name == port.port.name) else {
            if warn {
                let message = format!(
                    "{role} `{port}`: `{}` has no port `{}`",
                    self.scope_name(scope),
                    port.port.name
                );
                self.warn(port.pos, message);
            }
            return None;
        };
        let declared = &entry.ports[index];
        if !allowed.contains(&declared.direction) {
            if warn {
                let message = format!(
                    "{role} `{port}` must be {} port",
                    if allowed.contains(&SdfPortDirection::Input) {
                        "an input or inout"
                    } else {
                        "an output or inout"
                    }
                );
                self.warn(port.pos, message);
            }
            return None;
        }
        let bits = match port.port.bits {
            None => None,
            Some(bits) => {
                let (msb, lsb) = bits.bounds();
                let (msb, lsb) = (msb as i64, lsb as i64);
                let inside = |bit: i64| match declared.range {
                    Some((a, b)) => bit >= a.min(b) && bit <= a.max(b),
                    None => false,
                };
                if !inside(msb) || !inside(lsb) {
                    if warn {
                        let message = format!(
                            "{role} `{port}` selects bits outside the port's declared range"
                        );
                        self.warn(port.pos, message);
                    }
                    return None;
                }
                Some((msb, lsb))
            }
        };
        Some(SdfPortTarget {
            scope,
            port: index as u32,
            bits,
        })
    }

    fn del_def(&mut self, region: u32, mode: SdfMode, def: &SdfDelDef) -> Result<(), SdfError> {
        const LOADS: &[SdfPortDirection] = &[SdfPortDirection::Input, SdfPortDirection::Inout];
        const SOURCES: &[SdfPortDirection] = &[SdfPortDirection::Output, SdfPortDirection::Inout];
        // The reader warned about a list of more than 12 delay values.
        let delays = match def {
            SdfDelDef::IoPath { delays, .. }
            | SdfDelDef::Port { delays, .. }
            | SdfDelDef::Interconnect { delays, .. }
            | SdfDelDef::NetDelay { delays, .. }
            | SdfDelDef::Device { delays, .. } => delays,
        };
        if delays.values.len() > 12 {
            return Ok(());
        }
        match def {
            SdfDelDef::IoPath {
                pos,
                condition,
                input,
                output,
                retain,
                delays,
            } => {
                let Some(scope) = self.port_scope(region, &input.port, output, *pos, "IOPATH")
                else {
                    return Ok(());
                };
                let found = self.path_matches(
                    scope,
                    input.edge,
                    &input.port.port,
                    &output.port,
                    Some(condition),
                );
                if found.is_empty() {
                    let restriction = match condition {
                        SdfPathCondition::None => String::new(),
                        SdfPathCondition::Cond(condition) => {
                            format!(" with condition `{}`", condition.expr)
                        }
                        SdfPathCondition::CondElse => " that is `ifnone`".to_owned(),
                    };
                    let message = format!(
                        "no specify path from `{input}` to `{output}`{restriction} in `{}`; the \
                         IOPATH is not annotated",
                        self.scope_name(scope)
                    );
                    self.warn(*pos, message);
                    return Ok(());
                }
                if !retain.is_empty() {
                    self.warn(
                        *pos,
                        "RETAIN delays are not simulated and are ignored (IEEE 1800-2009 Table \
                         32-1)"
                            .to_owned(),
                    );
                }
                let delay = self.delay_op(mode, delays);
                for (path, input, output) in found {
                    self.push(
                        *pos,
                        SdfOpKind::PathDelay {
                            scope,
                            path,
                            input,
                            output,
                            delay,
                        },
                    )?;
                }
            }
            SdfDelDef::Port { pos, port, delays } => {
                let Some(load) = self.port_target(region, port, LOADS, "PORT", true) else {
                    return Ok(());
                };
                let delay = self.delay_op(mode, delays);
                self.push(*pos, SdfOpKind::PortDelay { load, delay })?;
            }
            SdfDelDef::Interconnect {
                pos,
                source,
                load,
                delays,
            } => {
                let Some(load) =
                    self.port_target(region, load, LOADS, "the INTERCONNECT load", true)
                else {
                    return Ok(());
                };
                let source_target =
                    self.port_target(region, source, SOURCES, "the INTERCONNECT source", true);
                if source_target.is_none() {
                    self.warn(
                        *pos,
                        "the INTERCONNECT source was not found; the delay applies from every \
                         source of the load (IEEE 1800-2009 32.4.4)"
                            .to_owned(),
                    );
                }
                let delay = self.delay_op(mode, delays);
                self.push(
                    *pos,
                    SdfOpKind::Interconnect {
                        source: source_target,
                        load,
                        delay,
                    },
                )?;
            }
            SdfDelDef::NetDelay { pos, net, delays } => {
                let loads = match self.net_loads(region, net, *pos) {
                    Some(loads) => loads,
                    None => match self.port_target(region, net, LOADS, "NETDELAY", true) {
                        Some(load) => vec![load],
                        None => return Ok(()),
                    },
                };
                let delay = self.delay_op(mode, delays);
                for load in loads {
                    self.push(*pos, SdfOpKind::NetDelay { load, delay })?;
                }
            }
            SdfDelDef::Device { pos, port, delays } => {
                self.device(region, mode, *pos, port, delays)?
            }
        }
        Ok(())
    }

    /// DEVICE (Table 32-1): all paths to the region's outputs, those into
    /// one output port, or all paths of a named child instance; on a
    /// primitive, the primitive's delay.
    fn device(
        &mut self,
        region: u32,
        mode: SdfMode,
        pos: SdfPos,
        port: &Option<SdfPortRef>,
        delays: &SdfDelvalList,
    ) -> Result<(), SdfError> {
        let entry = self.targets.scope(region);
        if entry.kind == SdfScopeKind::Primitive {
            if let Some(port) = port {
                let message = format!(
                    "DEVICE on primitive `{}` cannot name a port (`{port}`)",
                    self.scope_name(region)
                );
                self.warn(pos, message);
                return Ok(());
            }
            let delay = self.delay_op(mode, delays);
            return self.push(
                pos,
                SdfOpKind::PrimitiveDelay {
                    scope: region,
                    delay,
                },
            );
        }
        let (scope, output) = match port {
            None => (region, None),
            Some(port) => {
                let Some(scope) = self.walk(region, &port.path) else {
                    let message = format!(
                        "instance `{}` not found in `{}`; the DEVICE is not annotated",
                        path_text(&port.path),
                        self.scope_name(region)
                    );
                    self.warn(pos, message);
                    return Ok(());
                };
                let entry = self.targets.scope(scope);
                if let Some(declared) = entry.ports.iter().find(|p| p.name == port.port.name) {
                    if !matches!(
                        declared.direction,
                        SdfPortDirection::Output | SdfPortDirection::Inout
                    ) {
                        let message = format!("the DEVICE port `{port}` is not an output");
                        self.warn(pos, message);
                        return Ok(());
                    }
                    (scope, Some(port.port.name.as_str()))
                } else if let Some(child) = self.child(scope, &port.port) {
                    let kind = self.targets.scope(child).kind;
                    if kind == SdfScopeKind::Primitive {
                        let delay = self.delay_op(mode, delays);
                        return self.push(
                            pos,
                            SdfOpKind::PrimitiveDelay {
                                scope: child,
                                delay,
                            },
                        );
                    }
                    (child, None)
                } else {
                    let message = format!(
                        "`{}` has no output port or instance `{}`; the DEVICE is not annotated",
                        self.scope_name(scope),
                        port.port
                    );
                    self.warn(pos, message);
                    return Ok(());
                }
            }
        };
        let found = self.all_paths(scope, output);
        if found.is_empty() {
            // Without specify paths, the primitives driving the outputs
            // (Table 32-1).
            let entry = self.targets.scope(scope);
            let port = output.and_then(|name| entry.ports.iter().position(|p| p.name == name));
            let primitives = entry
                .children
                .iter()
                .copied()
                .filter(|child| {
                    let child = self.targets.scope(*child);
                    child.kind == SdfScopeKind::Primitive
                        && !child.drives.is_empty()
                        && port.is_none_or(|port| child.drives.contains(&(port as u32)))
                })
                .collect::<Vec<_>>();
            if primitives.is_empty() {
                let message = format!(
                    "`{}` has no specify paths{} and no primitive driving {}; the DEVICE is \
                     not annotated",
                    self.scope_name(scope),
                    output
                        .map(|name| format!(" to `{name}`"))
                        .unwrap_or_default(),
                    output.map_or_else(|| "its outputs".to_owned(), |name| format!("`{name}`"))
                );
                self.warn(pos, message);
                return Ok(());
            }
            let delay = self.delay_op(mode, delays);
            for primitive in primitives {
                self.push(
                    pos,
                    SdfOpKind::PrimitiveDelay {
                        scope: primitive,
                        delay,
                    },
                )?;
            }
            return Ok(());
        }
        let delay = self.delay_op(mode, delays);
        for (scope, (path, input, output)) in found {
            self.push(
                pos,
                SdfOpKind::PathDelay {
                    scope,
                    path,
                    input,
                    output,
                    delay,
                },
            )?;
        }
        Ok(())
    }

    // ----- timing checks -------------------------------------------------

    fn timing_check(&mut self, region: u32, check: &SdfTimingCheck) -> Result<(), SdfError> {
        use SdfLimitSlot::{First, Second};
        use SdfTimingCheckKind as K;
        // Table 32-2: the declared kinds an entry annotates and, per value,
        // the declared limit it replaces.
        let table: &[(K, &[(usize, SdfLimitSlot)])] = match check.kind {
            SdfCheckKind::Setup => &[(K::Setup, &[(0, First)]), (K::SetupHold, &[(0, First)])],
            SdfCheckKind::Hold => &[(K::Hold, &[(0, First)]), (K::SetupHold, &[(0, Second)])],
            SdfCheckKind::SetupHold => &[
                (K::Setup, &[(0, First)]),
                (K::Hold, &[(1, First)]),
                (K::SetupHold, &[(0, First), (1, Second)]),
            ],
            SdfCheckKind::Recovery => &[(K::Recovery, &[(0, First)]), (K::RecRem, &[(0, First)])],
            SdfCheckKind::Removal => &[(K::Removal, &[(0, First)]), (K::RecRem, &[(0, Second)])],
            SdfCheckKind::RecRem => &[
                (K::Recovery, &[(0, First)]),
                (K::Removal, &[(1, First)]),
                (K::RecRem, &[(0, First), (1, Second)]),
            ],
            SdfCheckKind::Skew => &[(K::Skew, &[(0, First)]), (K::TimeSkew, &[(0, First)])],
            SdfCheckKind::Width => &[(K::Width, &[(0, First)])],
            SdfCheckKind::Period => &[(K::Period, &[(0, First)])],
            SdfCheckKind::NoChange => &[(K::NoChange, &[(0, First), (1, Second)])],
        };
        let first = &check.ports[0];
        let Some(scope) = self.walk(region, &first.spec.port.path) else {
            let message = format!(
                "instance `{}` not found in `{}`; the {} is not annotated",
                path_text(&first.spec.port.path),
                self.scope_name(region),
                check.kind.keyword()
            );
            self.warn(check.pos, message);
            return Ok(());
        };
        if check
            .ports
            .iter()
            .any(|port| port.spec.port.path != first.spec.port.path)
        {
            let message = format!(
                "the {} ports are in different instances; it is not annotated",
                check.kind.keyword()
            );
            self.warn(check.pos, message);
            return Ok(());
        }
        // SDF port order: data then clock for SETUP, HOLD and SETUPHOLD;
        // otherwise the reference (control, stamp) port first.
        let (reference, data) = match check.kind {
            SdfCheckKind::Setup | SdfCheckKind::Hold | SdfCheckKind::SetupHold => {
                (&check.ports[1], Some(&check.ports[0]))
            }
            SdfCheckKind::Width | SdfCheckKind::Period => (&check.ports[0], None),
            _ => (&check.ports[0], check.ports.get(1)),
        };
        let entry = self.targets.scope(scope);
        let whole = |name: &str| {
            entry
                .ports
                .iter()
                .find(|port| port.name == name)
                .and_then(|port| port.range)
        };
        let event_matches =
            |sdf: &SdfPortTchk, declared: &SdfCheckEvent, stamp: Option<&SdfCondition>| {
                if !terminal_matches(
                    &declared.terminal,
                    &sdf.spec.port.port,
                    whole(&declared.terminal.name),
                ) {
                    return false;
                }
                if let Some(edge) = sdf.spec.edge {
                    if declared.edges != edge_mask(edge) {
                        return false;
                    }
                }
                let condition = stamp.or(sdf.condition.as_ref());
                match condition {
                    None => true,
                    Some(condition) => matches!(
                        &declared.condition,
                        SdfTargetCondition::If(Some(expr)) if *expr == condition.expr
                    ),
                }
            };
        let declared_condition_matches =
            |sdf: Option<&SdfCondition>, declared: &SdfTargetCondition| match sdf {
                None => true,
                Some(condition) => matches!(
                    declared,
                    SdfTargetCondition::If(Some(expr)) if *expr == condition.expr
                ),
            };
        let mut ops = Vec::new();
        let mut matched = false;
        for (index, declared) in entry.checks.iter().enumerate() {
            let Some((_, slots)) = table.iter().find(|(kind, _)| *kind == declared.kind) else {
                continue;
            };
            let combined = matches!(declared.kind, K::SetupHold | K::RecRem);
            // SCOND/CCOND: on a combined declaration they match its stamp
            // and check conditions; on a separate $setup the stamp event is
            // the data event, on a separate $hold (and $recovery,
            // $removal) the reference event (OVI SDF 3.0 3-27).
            let (reference_extra, data_extra) = if combined {
                (None, None)
            } else if declared.kind == K::Setup {
                (check.ccond.as_ref(), check.scond.as_ref())
            } else {
                (check.scond.as_ref(), check.ccond.as_ref())
            };
            if combined
                && (!declared_condition_matches(check.scond.as_ref(), &declared.stamp_condition)
                    || !declared_condition_matches(check.ccond.as_ref(), &declared.check_condition))
            {
                continue;
            }
            if !event_matches(reference, &declared.reference, reference_extra) {
                continue;
            }
            match (data, &declared.data) {
                (Some(data), Some(declared_data)) => {
                    if !event_matches(data, declared_data, data_extra) {
                        continue;
                    }
                }
                (None, None) => {}
                _ => continue,
            }
            matched = true;
            // An empty `()` value leaves its limit unchanged.
            for (value, slot) in slots.iter() {
                if let Some(value) = check.values[*value] {
                    ops.push((index as u32, *slot, value));
                }
            }
        }
        if !matched {
            let mut ports = check
                .ports
                .iter()
                .map(|port| match &port.condition {
                    Some(condition) => format!("(COND {} {})", condition.expr, port.spec),
                    None => port.spec.to_string(),
                })
                .collect::<Vec<_>>()
                .join(" ");
            for (keyword, condition) in [("SCOND", &check.scond), ("CCOND", &check.ccond)] {
                if let Some(condition) = condition {
                    ports.push_str(&format!(" ({keyword} {})", condition.expr));
                }
            }
            let message = format!(
                "no timing check in `{}` matches ({} {ports}); it is not annotated",
                self.scope_name(scope),
                check.kind.keyword()
            );
            self.warn(check.pos, message);
            return Ok(());
        }
        for (index, slot, value) in ops {
            self.push(
                check.pos,
                SdfOpKind::CheckLimit {
                    scope,
                    check: index,
                    slot,
                    value,
                },
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn terminal(name: &str) -> SdfTerminal {
        SdfTerminal {
            name: name.to_owned(),
            range: None,
        }
    }

    fn event(name: &str, edges: u8) -> SdfCheckEvent {
        SdfCheckEvent {
            terminal: terminal(name),
            edges,
            condition: SdfTargetCondition::None,
        }
    }

    fn check(
        kind: SdfTimingCheckKind,
        reference: SdfCheckEvent,
        data: Option<SdfCheckEvent>,
    ) -> SdfTargetCheck {
        SdfTargetCheck {
            origin: String::new(),
            kind,
            reference,
            data,
            stamp_condition: SdfTargetCondition::None,
            check_condition: SdfTargetCondition::None,
            rows: Vec::new(),
            delayed: false,
        }
    }

    const POSEDGE: u8 = 1 | 2 | 32;

    /// `top` with child `u` (cell type "C") that declares one check of
    /// every kind between `clk` (reference) and `d` (data).
    fn targets() -> SdfTargets {
        use SdfTimingCheckKind as K;
        let mut targets = SdfTargets::new();
        let top = targets.add_scope(
            None,
            SdfScope::new("top".into(), None, SdfScopeKind::Module, "top".into()),
        );
        let mut cell = SdfScope::new("u".into(), None, SdfScopeKind::Module, "C".into());
        for name in ["clk", "d"] {
            cell.ports.push(SdfTargetPort {
                name: name.into(),
                direction: SdfPortDirection::Input,
                range: None,
            });
        }
        for kind in [
            K::Setup,
            K::Hold,
            K::SetupHold,
            K::Recovery,
            K::Removal,
            K::RecRem,
            K::Skew,
            K::TimeSkew,
            K::FullSkew,
            K::NoChange,
        ] {
            cell.checks
                .push(check(kind, event("clk", POSEDGE), Some(event("d", 0))));
        }
        cell.checks
            .push(check(K::Width, event("clk", POSEDGE), None));
        cell.checks
            .push(check(K::Period, event("clk", POSEDGE), None));
        targets.add_scope(Some(top), cell);
        targets
    }

    fn ops(body: &str) -> Vec<String> {
        let targets = targets();
        let text = format!(
            "(DELAYFILE (SDFVERSION \"3.0\") (CELL (CELLTYPE \"C\") (INSTANCE u) (TIMINGCHECK {body})))"
        );
        let plan = resolve(text.as_bytes(), &targets, 0, SdfLimits::default()).expect("resolves");
        plan.ops
            .iter()
            .map(|op| match &op.kind {
                SdfOpKind::CheckLimit {
                    check, slot, value, ..
                } => format!(
                    "{} {slot:?} {value}",
                    targets.scope(1).checks[*check as usize].kind.name()
                ),
                other => format!("{other:?}"),
            })
            .chain(
                plan.warnings
                    .iter()
                    .map(|warning| format!("warning {}", warning.message)),
            )
            .collect()
    }

    /// IEEE 1800-2009 Table 32-2.
    #[test]
    fn timing_checks_follow_the_mapping_table() {
        assert_eq!(
            ops("(SETUP d (posedge clk) (1))"),
            ["$setup First 1", "$setuphold First 1"]
        );
        assert_eq!(
            ops("(HOLD d (posedge clk) (2))"),
            ["$hold First 2", "$setuphold Second 2"]
        );
        assert_eq!(
            ops("(SETUPHOLD d (posedge clk) (1) (2))"),
            [
                "$setup First 1",
                "$hold First 2",
                "$setuphold First 1",
                "$setuphold Second 2"
            ]
        );
        assert_eq!(
            ops("(RECOVERY (posedge clk) d (3))"),
            ["$recovery First 3", "$recrem First 3"]
        );
        assert_eq!(
            ops("(REMOVAL (posedge clk) d (4))"),
            ["$removal First 4", "$recrem Second 4"]
        );
        assert_eq!(
            ops("(RECREM (posedge clk) d (3) (4))"),
            [
                "$recovery First 3",
                "$removal First 4",
                "$recrem First 3",
                "$recrem Second 4"
            ]
        );
        assert_eq!(
            ops("(SKEW (posedge clk) d (5))"),
            ["$skew First 5", "$timeskew First 5"]
        );
        assert_eq!(ops("(WIDTH (posedge clk) (6))"), ["$width First 6"]);
        assert_eq!(ops("(PERIOD (posedge clk) (7))"), ["$period First 7"]);
        assert_eq!(
            ops("(NOCHANGE (posedge clk) d (8) (9))"),
            ["$nochange First 8", "$nochange Second 9"]
        );
    }

    #[test]
    fn edges_conditions_and_empty_values() {
        // No SDF edge matches any declared edge; another edge matches none.
        assert_eq!(ops("(WIDTH clk (1))"), ["$width First 1"]);
        assert_eq!(
            ops("(WIDTH (negedge clk) (1))"),
            ["warning no timing check in `top.u` matches (WIDTH (negedge clk)); it is not annotated"]
        );
        // The declared events have no `&&&` condition.
        assert_eq!(
            ops("(WIDTH (COND en (posedge clk)) (1))"),
            ["warning no timing check in `top.u` matches (WIDTH (COND en (posedge clk))); it is not \
              annotated"]
        );
        // `()` keeps a limit: the entry matches but adds no operation.
        assert_eq!(
            ops("(SETUPHOLD d (posedge clk) () (2))"),
            ["$hold First 2", "$setuphold Second 2"]
        );
    }

    #[test]
    fn the_plan_size_is_bounded() {
        let targets = targets();
        let text = "(DELAYFILE (SDFVERSION \"3.0\") (CELL (CELLTYPE \"C\") (INSTANCE *) \
                    (TIMINGCHECK (RECREM (posedge clk) d (3) (4)))))";
        let limits = SdfLimits {
            max_plan_ops: 3,
            ..SdfLimits::default()
        };
        assert_eq!(
            resolve(text.as_bytes(), &targets, 0, limits)
                .expect_err("too many operations")
                .to_string(),
            "1:79: the annotation has more than 3 operations"
        );
    }

    /// NETDELAY naming a net (32.4.4): every load port connected to the
    /// net, or to its selected bits, mapped to port indices left to right.
    #[test]
    fn netdelay_to_a_net_annotates_its_load_ports() {
        use super::super::{SdfNetLoad, SdfTargetNet};
        let mut targets = SdfTargets::new();
        let mut top = SdfScope::new("top".into(), None, SdfScopeKind::Module, "top".into());
        // `wire [7:0] n`: `a` (port [0:3]) takes n[7:4], `b` (port [1:0])
        // takes n[1:0], `c` (scalar) takes n[5].
        top.nets.push(SdfTargetNet {
            name: "n".into(),
            range: Some((7, 0)),
            loads: vec![
                SdfNetLoad {
                    scope: 1,
                    port: 0,
                    net_bits: Some((7, 4)),
                },
                SdfNetLoad {
                    scope: 2,
                    port: 0,
                    net_bits: Some((1, 0)),
                },
                SdfNetLoad {
                    scope: 3,
                    port: 0,
                    net_bits: Some((5, 5)),
                },
            ],
        });
        let top = targets.add_scope(None, top);
        for (name, range) in [("a", Some((0, 3))), ("b", Some((1, 0))), ("c", None)] {
            let mut cell = SdfScope::new(name.into(), None, SdfScopeKind::Module, "C".into());
            cell.ports.push(SdfTargetPort {
                name: "i".into(),
                direction: SdfPortDirection::Input,
                range,
            });
            targets.add_scope(Some(top), cell);
        }
        let loads = |net: &str| {
            let text = format!(
                "(DELAYFILE (SDFVERSION \"2.1\") (CELL (CELLTYPE \"top\") (INSTANCE) \
                 (DELAY (ABSOLUTE (NETDELAY {net} (1))))))"
            );
            let plan =
                resolve(text.as_bytes(), &targets, 0, SdfLimits::default()).expect("resolves");
            plan.ops
                .iter()
                .map(|op| match &op.kind {
                    SdfOpKind::NetDelay { load, .. } => {
                        format!("{} {:?}", targets.scope(load.scope).name, load.bits)
                    }
                    other => format!("{other:?}"),
                })
                .chain(
                    plan.warnings
                        .iter()
                        .map(|w| format!("warning {}", w.message)),
                )
                .collect::<Vec<_>>()
        };
        assert_eq!(loads("n"), ["a None", "b None", "c None"]);
        assert_eq!(loads("n[6]"), ["a Some((1, 1))"]);
        assert_eq!(
            loads("n[5:0]"),
            ["a Some((2, 3))", "b Some((1, 0))", "c None"]
        );
        assert_eq!(
            loads("n[3:2]"),
            ["warning net `n[3:2]` has no load ports; the NETDELAY is not annotated"]
        );
        assert_eq!(
            loads("n[8]"),
            [
                "warning NETDELAY `n[8]` selects bits outside the net's declared range; the \
              NETDELAY is not annotated"
            ]
        );
    }
}
