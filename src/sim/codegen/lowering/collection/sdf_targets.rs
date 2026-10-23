//! SDF annotation targets ([`SdfTargets`]): the instance tree with cell
//! types, ports and time precisions, and each instance's declared specify
//! paths and timing checks with the model rows lowering made of them. Built
//! before module paths are lowered to resolve `$sdf_annotate` files (rows
//! still empty), after timing checks are lowered to map the resolved
//! operations onto model rows, and on request; models are not affected.

use super::*;
use crate::core::db::{PathCondition, PathEdge, PrimitiveType, TimingCheckKind, TimingEvent};
use crate::sim::sdf::{
    parse_hdl_condition, CondExpr, SdfCheckEvent, SdfCheckRow, SdfNetLoad, SdfPathRow,
    SdfPortDirection, SdfScope, SdfScopeKind, SdfTargetCheck, SdfTargetCondition, SdfTargetEdge,
    SdfTargetNet, SdfTargetPath, SdfTargetPort, SdfTargets, SdfTerminal, SdfTimingCheckKind,
};

/// The primitive name a CELLTYPE uses for a gate instance.
fn primitive_name(primitive: PrimitiveType) -> &'static str {
    match primitive {
        PrimitiveType::And => "and",
        PrimitiveType::Nand => "nand",
        PrimitiveType::Nor => "nor",
        PrimitiveType::Or => "or",
        PrimitiveType::Xor => "xor",
        PrimitiveType::Xnor => "xnor",
        PrimitiveType::Buf => "buf",
        PrimitiveType::Not => "not",
        PrimitiveType::Bufif0 => "bufif0",
        PrimitiveType::Bufif1 => "bufif1",
        PrimitiveType::Notif0 => "notif0",
        PrimitiveType::Notif1 => "notif1",
        PrimitiveType::Nmos => "nmos",
        PrimitiveType::Pmos => "pmos",
        PrimitiveType::Cmos => "cmos",
        PrimitiveType::Rnmos => "rnmos",
        PrimitiveType::Rpmos => "rpmos",
        PrimitiveType::Rcmos => "rcmos",
        PrimitiveType::Rtran => "rtran",
        PrimitiveType::Rtranif0 => "rtranif0",
        PrimitiveType::Rtranif1 => "rtranif1",
        PrimitiveType::Tran => "tran",
        PrimitiveType::Tranif0 => "tranif0",
        PrimitiveType::Tranif1 => "tranif1",
        PrimitiveType::Pullup => "pullup",
        PrimitiveType::Pulldown => "pulldown",
        PrimitiveType::Sequential | PrimitiveType::Combinational | PrimitiveType::Unsupported => "",
    }
}

/// `name[index]` as an array name and element index.
fn split_element(name: &str) -> (String, Option<i64>) {
    if let Some((base, index)) = name
        .strip_suffix(']')
        .and_then(|rest| rest.rsplit_once('['))
    {
        if let Ok(index) = index.parse::<i64>() {
            if !base.is_empty() {
                return (base.to_owned(), Some(index));
            }
        }
    }
    (name.to_owned(), None)
}

impl Codegen<'_> {
    /// Build the targets of the instance trees under `tops`. Rows are those
    /// [`Self::lower_module_paths`] and [`Self::lower_timing_checks`] made so
    /// far.
    pub(in super::super) fn build_sdf_targets(&self, tops: &[NodeId]) -> SdfTargets {
        self.build_sdf_catalog(tops).0
    }

    /// [`Self::build_sdf_targets`] and the design node of each scope.
    pub(in super::super) fn build_sdf_catalog(&self, tops: &[NodeId]) -> (SdfTargets, Vec<NodeId>) {
        let db = self.db;
        let mut paths: HashMap<NodeId, Vec<&crate::core::db::ModulePath>> = HashMap::new();
        for path in db.module_paths() {
            if let Some(instance) = self.owning_inst(path.node) {
                paths.entry(instance).or_default().push(path);
            }
        }
        let mut checks: HashMap<NodeId, Vec<&crate::core::db::TimingCheck>> = HashMap::new();
        for check in db.timing_checks() {
            if let Some(instance) = self.owning_inst(check.node) {
                checks.entry(instance).or_default().push(check);
            }
        }
        let mut path_rows: HashMap<NodeId, Vec<SdfPathRow>> = HashMap::new();
        for (node, row) in &self.sdf_path_rows {
            path_rows.entry(*node).or_default().push(*row);
        }
        let mut check_rows: HashMap<NodeId, Vec<SdfCheckRow>> = HashMap::new();
        for (node, row) in &self.sdf_check_rows {
            check_rows.entry(*node).or_default().push(*row);
        }
        let mut tables = Tables {
            paths,
            checks,
            path_rows,
            check_rows,
            conditions: HashMap::new(),
            line_starts: HashMap::new(),
            nodes: Vec::new(),
        };
        let mut targets = SdfTargets::new();
        for top in tops {
            self.add_sdf_scope(&mut targets, &mut tables, None, *top);
        }
        self.add_sdf_nets(&mut targets, &tables.nodes);
        (targets, tables.nodes)
    }

    /// The nets that input and inout ports of module instances connect to
    /// (NETDELAY, 32.4.4), recorded in the scope declaring the net. A port
    /// whose actual is not a net or a constant select of one (a
    /// concatenation, an expression) is no NETDELAY load.
    fn add_sdf_nets(&self, targets: &mut SdfTargets, nodes: &[NodeId]) {
        let index_of = nodes
            .iter()
            .enumerate()
            .map(|(index, node)| (*node, index as u32))
            .collect::<HashMap<_, _>>();
        let mut nets: Vec<(u32, NodeId, SdfNetLoad)> = Vec::new();
        for (scope, node) in nodes.iter().enumerate() {
            if targets.scope(scope as u32).kind != SdfScopeKind::Module {
                continue;
            }
            let ports = self
                .node(*node)
                .children
                .iter()
                .filter(|child| matches!(self.kind(**child), NodeKind::Port { .. }));
            for (port_index, port) in ports.enumerate() {
                let NodeKind::Port {
                    direction: crate::core::db::Direction::Input | crate::core::db::Direction::Inout,
                    high,
                    high_expr,
                    ..
                } = self.kind(*port)
                else {
                    continue;
                };
                let Some(actual) = high_expr.or(*high) else {
                    continue;
                };
                let Some(net) = self.sdf_root_target(actual) else {
                    continue;
                };
                if !matches!(self.kind(net), NodeKind::Net { .. }) {
                    continue;
                }
                let Some(net_bits) = self.sdf_actual_bits(actual, net) else {
                    continue;
                };
                // The innermost catalog scope above the declaration.
                let mut owner = self.node(net).parent;
                let owner = loop {
                    match owner {
                        Some(candidate) => match index_of.get(&candidate) {
                            Some(index) => break Some(*index),
                            None => owner = self.node(candidate).parent,
                        },
                        None => break None,
                    }
                };
                if let Some(owner) = owner {
                    nets.push((
                        owner,
                        net,
                        SdfNetLoad {
                            scope: scope as u32,
                            port: port_index as u32,
                            net_bits,
                        },
                    ));
                }
            }
        }
        for (owner, net, load) in nets {
            let name = self.node(net).name.clone();
            let entry = targets.scope_mut(owner);
            match entry.nets.iter_mut().find(|known| known.name == name) {
                Some(known) => known.loads.push(load),
                None => {
                    let range = self
                        .db
                        .packed_dimensions(net)
                        .and_then(|ranges| (ranges.len() == 1).then(|| ranges[0]))
                        .map(|range| (range.left as i64, range.right as i64));
                    entry.nets.push(SdfTargetNet {
                        name,
                        range,
                        loads: vec![load],
                    });
                }
            }
        }
    }

    /// The net indices, left to right, a port actual naming `net` selects:
    /// the declared range for the whole net (`Some(None)` for a scalar),
    /// or a constant select; `None` when they are not constant.
    fn sdf_actual_bits(&self, actual: NodeId, net: NodeId) -> Option<Option<(i64, i64)>> {
        let bound = |node: NodeId| self.eval_bound_i128(node).ok().map(|value| value as i64);
        match self.kind(actual) {
            NodeKind::Expr(ExprKind::BitSelect { base, index }) if self.is_net_ref(*base, net) => {
                bound(*index).map(|bit| Some((bit, bit)))
            }
            NodeKind::Expr(ExprKind::PartSelect {
                base, left, right, ..
            }) if self.is_net_ref(*base, net) => bound(*left).zip(bound(*right)).map(Some),
            NodeKind::Expr(ExprKind::IndexedPartSelect {
                base,
                base_expr,
                width_expr,
                neg,
                ..
            }) if self.is_net_ref(*base, net) => {
                let (start, width) = bound(*base_expr).zip(bound(*width_expr))?;
                Some(Some(if *neg {
                    (start, start - width + 1)
                } else {
                    (start + width - 1, start)
                }))
            }
            NodeKind::Expr(ExprKind::Ref { .. }) | NodeKind::Net { .. } => Some(
                self.db
                    .packed_dimensions(net)
                    .and_then(|ranges| (ranges.len() == 1).then(|| ranges[0]))
                    .map(|range| (range.left as i64, range.right as i64)),
            ),
            _ => None,
        }
    }

    /// `expression` names `net` itself (not an element of an array of nets).
    fn is_net_ref(&self, expression: NodeId, net: NodeId) -> bool {
        match self.kind(expression) {
            NodeKind::Expr(ExprKind::Ref { target }) => *target == Some(net),
            NodeKind::Net { .. } => expression == net,
            _ => false,
        }
    }

    fn add_sdf_scope(
        &self,
        targets: &mut SdfTargets,
        tables: &mut Tables<'_>,
        parent: Option<u32>,
        node: NodeId,
    ) {
        let entry = self.node(node);
        let scope = match &entry.kind {
            NodeKind::ModuleInst {
                def_name, is_top, ..
            } => {
                let name = if *is_top {
                    strip_lib(&entry.name)
                } else {
                    entry.name.clone()
                };
                let (name, index) = split_element(&name);
                let mut scope =
                    SdfScope::new(name, index, SdfScopeKind::Module, strip_lib(def_name));
                scope.precision_fs = self.timescale_of_node(node).precision_fs;
                scope.ports = self.sdf_ports(node);
                scope.paths = self.sdf_paths(node, tables);
                scope.checks = self.sdf_checks(node, tables);
                Some(scope)
            }
            NodeKind::GenScope if !entry.name.is_empty() => {
                let (name, index) = split_element(&entry.name);
                Some(SdfScope::new(
                    name,
                    index,
                    SdfScopeKind::Generate,
                    String::new(),
                ))
            }
            NodeKind::Gate { prim_type, udp, .. } if !entry.name.is_empty() => {
                let cell_type = match udp {
                    Some(_) => self.gate_definition_name(node),
                    None => primitive_name(*prim_type).to_owned(),
                };
                let (name, index) = split_element(&entry.name);
                let mut scope = SdfScope::new(name, index, SdfScopeKind::Primitive, cell_type);
                scope.precision_fs = self.timescale_of_node(node).precision_fs;
                scope.drives = self.sdf_primitive_drives(node);
                Some(scope)
            }
            _ => None,
        };
        let parent = match scope {
            Some(scope) => {
                tables.nodes.push(node);
                Some(targets.add_scope(parent, scope))
            }
            None => parent,
        };
        if matches!(entry.kind, NodeKind::Gate { .. }) {
            return;
        }
        for child in &entry.children {
            if matches!(
                self.kind(*child),
                NodeKind::ModuleInst { .. }
                    | NodeKind::GenScope
                    | NodeKind::GenScopeArray
                    | NodeKind::InstanceArray
                    | NodeKind::Gate { .. }
            ) {
                self.add_sdf_scope(targets, tables, parent, *child);
            }
        }
    }

    /// The ports of the module instance owning gate `gate` that one of its
    /// output terminals names, by port order: the outputs a DEVICE entry on
    /// a module without paths annotates through the gate (decision A12-D11).
    fn sdf_primitive_drives(&self, gate: NodeId) -> Vec<u32> {
        let NodeKind::Gate { terms, .. } = self.kind(gate) else {
            return Vec::new();
        };
        let Some(instance) = self.owning_inst(gate) else {
            return Vec::new();
        };
        let outputs = terms
            .iter()
            .filter(|term| term.direction != crate::core::db::Direction::Input)
            .filter_map(|term| self.sdf_root_target(term.expr))
            .collect::<Vec<_>>();
        if outputs.is_empty() {
            return Vec::new();
        }
        let mut drives = Vec::new();
        let ports = self
            .node(instance)
            .children
            .iter()
            .filter(|child| matches!(self.kind(**child), NodeKind::Port { .. }));
        for (index, port) in ports.enumerate() {
            let NodeKind::Port { low, .. } = self.kind(*port) else {
                continue;
            };
            let target = self
                .port_child_target(*port, *low)
                .and_then(|expression| self.sdf_root_target(expression));
            if target.is_some_and(|target| outputs.contains(&target)) {
                drives.push(index as u32);
            }
        }
        drives
    }

    /// The declaration an expression names, through selects.
    fn sdf_root_target(&self, expression: NodeId) -> Option<NodeId> {
        match self.kind(expression) {
            NodeKind::Expr(ExprKind::Ref { target }) => *target,
            NodeKind::Expr(
                ExprKind::BitSelect { base, .. }
                | ExprKind::PartSelect { base, .. }
                | ExprKind::IndexedPartSelect { base, .. },
            ) => self.sdf_root_target(*base),
            NodeKind::Var { .. } | NodeKind::Net { .. } => Some(expression),
            _ => None,
        }
    }

    /// A UDP instance's cell type: the UDP's declared name.
    fn gate_definition_name(&self, node: NodeId) -> String {
        match &self.node(node).kind {
            NodeKind::Gate {
                udp: Some(table), ..
            } => table.name.clone(),
            _ => String::new(),
        }
    }

    fn sdf_ports(&self, instance: NodeId) -> Vec<SdfTargetPort> {
        let mut ports = Vec::new();
        for child in &self.node(instance).children {
            let NodeKind::Port { direction, .. } = self.kind(*child) else {
                continue;
            };
            let direction = match direction {
                crate::core::db::Direction::Input => SdfPortDirection::Input,
                crate::core::db::Direction::Output => SdfPortDirection::Output,
                crate::core::db::Direction::Inout => SdfPortDirection::Inout,
                _ => SdfPortDirection::Other,
            };
            let range = self
                .db
                .packed_dimensions(*child)
                .and_then(|ranges| (ranges.len() == 1).then(|| ranges[0]))
                .map(|range| (range.left as i64, range.right as i64));
            ports.push(SdfTargetPort {
                name: self.node(*child).name.clone(),
                direction,
                range,
            });
        }
        ports
    }

    /// A path or timing check terminal: its signal name and the declared
    /// indices it selects.
    fn sdf_terminal(&self, expression: NodeId) -> SdfTerminal {
        let name = |base: NodeId| match self.kind(base) {
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) => self.node(*target).name.clone(),
            _ => self.node(base).name.clone(),
        };
        let bound = |node: NodeId| self.eval_bound_i128(node).ok().map(|value| value as i64);
        let (name, range) = match self.kind(expression) {
            NodeKind::Expr(ExprKind::BitSelect { base, index }) => {
                (name(*base), bound(*index).map(|bit| (bit, bit)))
            }
            NodeKind::Expr(ExprKind::PartSelect {
                base, left, right, ..
            }) => (name(*base), bound(*left).zip(bound(*right))),
            NodeKind::Expr(ExprKind::IndexedPartSelect {
                base,
                base_expr,
                width_expr,
                neg,
                ..
            }) => {
                let range = bound(*base_expr)
                    .zip(bound(*width_expr))
                    .map(|(start, width)| {
                        if *neg {
                            (start, start - width + 1)
                        } else {
                            (start + width - 1, start)
                        }
                    });
                (name(*base), range)
            }
            _ => (name(expression), None),
        };
        SdfTerminal { name, range }
    }

    /// The condition a node's source text spells, in SDF form. Every
    /// instance of a module shares the text of its declarations, so the
    /// result is cached by source span.
    fn sdf_condition(
        &self,
        condition: Option<NodeId>,
        tables: &mut Tables<'_>,
    ) -> SdfTargetCondition {
        let Some(node) = condition else {
            return SdfTargetCondition::None;
        };
        let entry = self.node(node);
        let Some(file) = entry.file.as_deref() else {
            return SdfTargetCondition::If(None);
        };
        let key = (
            file.to_owned(),
            entry.line,
            entry.col,
            entry.end_line,
            entry.end_col,
        );
        if let Some(cached) = tables.conditions.get(&key) {
            return SdfTargetCondition::If(cached.clone());
        }
        let parsed = self
            .sdf_node_text(node, tables)
            .and_then(|text| parse_hdl_condition(&text));
        tables.conditions.insert(key, parsed.clone());
        SdfTargetCondition::If(parsed)
    }

    /// The source text a node spans, if it lies on known lines (columns
    /// are 1-based; the end column is one past the last character).
    fn sdf_node_text(&self, node: NodeId, tables: &mut Tables<'_>) -> Option<String> {
        let entry = self.node(node);
        let file = entry.file.as_deref()?;
        let source = self.db.source_text(file)?;
        let starts = tables
            .line_starts
            .entry(file.to_owned())
            .or_insert_with(|| {
                std::iter::once(0)
                    .chain(source.match_indices('\n').map(|(at, _)| at + 1))
                    .collect()
            });
        let (line, col, end_line, end_col) = (
            entry.line as usize,
            entry.col as usize,
            entry.end_line as usize,
            entry.end_col as usize,
        );
        if line == 0 || col == 0 || end_line < line || end_line > starts.len() {
            return None;
        }
        let line_text = |number: usize| {
            let start = starts[number - 1];
            let end = starts.get(number).map_or(source.len(), |next| next - 1);
            source[start..end].trim_end_matches('\r')
        };
        let mut text = String::new();
        for number in line..=end_line {
            let current = line_text(number);
            let start = if number == line { col - 1 } else { 0 };
            let end = if number == end_line {
                end_col.saturating_sub(1).min(current.len())
            } else {
                current.len()
            };
            text.push_str(current.get(start..end)?);
            text.push(' ');
        }
        Some(text)
    }

    fn sdf_paths(&self, instance: NodeId, tables: &mut Tables<'_>) -> Vec<SdfTargetPath> {
        let Some(paths) = tables.paths.get(&instance).cloned() else {
            return Vec::new();
        };
        paths
            .iter()
            .map(|path| SdfTargetPath {
                origin: self.specify_location(path.node),
                inputs: path
                    .inputs
                    .iter()
                    .map(|input| self.sdf_terminal(*input))
                    .collect(),
                outputs: path
                    .outputs
                    .iter()
                    .map(|output| self.sdf_terminal(*output))
                    .collect(),
                edge: match path.edge {
                    None => SdfTargetEdge::None,
                    Some(PathEdge::Posedge) => SdfTargetEdge::Posedge,
                    Some(PathEdge::Negedge) => SdfTargetEdge::Negedge,
                    Some(PathEdge::Edge) => SdfTargetEdge::Any,
                },
                condition: match path.condition {
                    PathCondition::None => SdfTargetCondition::None,
                    PathCondition::IfNone => SdfTargetCondition::IfNone,
                    PathCondition::If(expression) => self.sdf_condition(Some(expression), tables),
                },
                rows: tables
                    .path_rows
                    .get(&path.node)
                    .cloned()
                    .unwrap_or_default(),
            })
            .collect()
    }

    fn sdf_event(&self, event: &TimingEvent, tables: &mut Tables<'_>) -> SdfCheckEvent {
        SdfCheckEvent {
            terminal: self.sdf_terminal(event.terminal),
            edges: event.edges.bits(),
            condition: self.sdf_condition(event.condition, tables),
        }
    }

    fn sdf_checks(&self, instance: NodeId, tables: &mut Tables<'_>) -> Vec<SdfTargetCheck> {
        let Some(checks) = tables.checks.get(&instance).cloned() else {
            return Vec::new();
        };
        let mut out = Vec::with_capacity(checks.len());
        for check in checks {
            let kind = match check.kind {
                TimingCheckKind::Setup => SdfTimingCheckKind::Setup,
                TimingCheckKind::Hold => SdfTimingCheckKind::Hold,
                TimingCheckKind::SetupHold => SdfTimingCheckKind::SetupHold,
                TimingCheckKind::Recovery => SdfTimingCheckKind::Recovery,
                TimingCheckKind::Removal => SdfTimingCheckKind::Removal,
                TimingCheckKind::RecRem => SdfTimingCheckKind::RecRem,
                TimingCheckKind::Skew => SdfTimingCheckKind::Skew,
                TimingCheckKind::TimeSkew => SdfTimingCheckKind::TimeSkew,
                TimingCheckKind::FullSkew => SdfTimingCheckKind::FullSkew,
                TimingCheckKind::Period => SdfTimingCheckKind::Period,
                TimingCheckKind::Width => SdfTimingCheckKind::Width,
                TimingCheckKind::NoChange => SdfTimingCheckKind::NoChange,
                TimingCheckKind::Unknown => continue,
            };
            // `$setup` lists its data event first (31.3.1); `$period` and
            // `$width` have only a reference event.
            let (reference, data) = match kind {
                SdfTimingCheckKind::Setup => (check.events[1], check.events[0]),
                SdfTimingCheckKind::Period | SdfTimingCheckKind::Width => (check.events[0], None),
                _ => (check.events[0], check.events[1]),
            };
            let Some(reference) = reference else {
                continue;
            };
            out.push(SdfTargetCheck {
                origin: self.specify_location(check.node),
                kind,
                reference: self.sdf_event(&reference, tables),
                data: data.map(|event| self.sdf_event(&event, tables)),
                stamp_condition: self.sdf_condition(check.stamp_condition, tables),
                check_condition: self.sdf_condition(check.check_condition, tables),
                rows: tables
                    .check_rows
                    .get(&check.node)
                    .cloned()
                    .unwrap_or_default(),
                delayed: self.sdf_delayed_checks.contains(&check.node),
            });
        }
        out
    }
}

/// Specify facts and model rows grouped by instance and declaration, and
/// the parsed conditions and line starts of the source files.
struct Tables<'d> {
    paths: HashMap<NodeId, Vec<&'d crate::core::db::ModulePath>>,
    checks: HashMap<NodeId, Vec<&'d crate::core::db::TimingCheck>>,
    path_rows: HashMap<NodeId, Vec<SdfPathRow>>,
    check_rows: HashMap<NodeId, Vec<SdfCheckRow>>,
    /// Keyed by file and span: (line, col, end line, end col).
    conditions: HashMap<(String, u32, u32, u32, u32), Option<CondExpr>>,
    line_starts: HashMap<String, Vec<usize>>,
    /// The design node of each scope, by scope index.
    nodes: Vec<NodeId>,
}
