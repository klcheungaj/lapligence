//! VPI timing objects and delays (IEEE 1800-2009 37.35, 37.36, 38.10,
//! 38.32): one catalog row per input and output pair of every module path
//! declaration and per timing check of every instance, and the delays of
//! primitives and continuous assignments, each with the model values
//! back-annotation writes. `vpi_put_delays`
//! applies new values through the same runtime operations as an SDF
//! annotation (`llg_sdf_apply`), so the two cannot disagree.

use super::*;
use crate::core::db::{
    DriverDelay, PathCondition, PathConnection, PathEdge, PathPolarity, TimingCheckKind,
    TimingEdges,
};
use crate::sim::ir::{expand_path_delays, IrSdfTarget, IrVpiConnRole, IrVpiTiming};
use crate::sim::sdf::{SdfCheckRow, SdfLimitSlot, SdfPathRow};

/// `vpiEdge` bits (Annex K).
const EDGE_01: u8 = 0x01;
const EDGE_10: u8 = 0x02;
const EDGE_0X: u8 = 0x04;
const EDGE_X1: u8 = 0x08;
const EDGE_1X: u8 = 0x10;
const EDGE_X0: u8 = 0x20;
const POSEDGE: u8 = EDGE_X1 | EDGE_01 | EDGE_0X;
const NEGEDGE: u8 = EDGE_X0 | EDGE_10 | EDGE_1X;

/// The delay expressions of a module path declaration or timing check and
/// the rows that have them (`vpiDelay`, 37.3.4): the pairs of a declaration
/// share theirs.
pub(super) struct VpiRowDelays {
    /// The keys of the rows.
    pub(super) rows: Vec<String>,
    pub(super) scope: String,
    pub(super) expressions: Vec<NodeId>,
}

/// The timing rows and delays [`Codegen::populate_vpi_metadata`] adds to
/// the catalog.
#[derive(Default)]
pub(in super::super::super) struct VpiTimingMetadata {
    /// Module path and timing check rows.
    pub(super) rows: Vec<IrVpiObject>,
    /// The delays of each primitive and continuous assignment.
    pub(super) delays: HashMap<NodeId, IrVpiTiming>,
    /// The delay expressions of each primitive, continuous assignment and
    /// net that declares delays (`vpiDelay`, 37.3.4).
    pub(super) delay_nodes: HashMap<NodeId, Vec<NodeId>>,
    /// The delay expressions of the module path and timing check rows.
    pub(super) row_delays: Vec<VpiRowDelays>,
    /// The SDF interconnect loads of each port: its bits with a load and
    /// the load (module input port delays, 37.14).
    pub(super) port_loads: HashMap<NodeId, Vec<(u32, u32)>>,
}

/// The expressions of a driver delay, in rise, fall, turn-off order.
fn driver_delay_nodes(delay: DriverDelay) -> Vec<NodeId> {
    match delay {
        DriverDelay::Single(value) => vec![value],
        DriverDelay::RiseFall(rise, fall) => vec![rise, fall],
        DriverDelay::RiseFallTurnOff(rise, fall, turn_off) => vec![rise, fall, turn_off],
    }
}

/// The `vpiEdge` bits of the transitions of a timing check event (z counts
/// as x, as the checks see it).
fn tchk_edge(edges: TimingEdges) -> u8 {
    let bits = edges.bits();
    [
        (TimingEdges::T01, EDGE_01),
        (TimingEdges::T0X, EDGE_0X),
        (TimingEdges::T10, EDGE_10),
        (TimingEdges::T1X, EDGE_1X),
        (TimingEdges::TX0, EDGE_X0),
        (TimingEdges::TX1, EDGE_X1),
    ]
    .iter()
    .filter(|(edge, _)| bits & edge != 0)
    .fold(0, |sum, (_, code)| sum | code)
}

fn polarity_code(polarity: PathPolarity) -> &'static str {
    match polarity {
        PathPolarity::Unknown => "vpiUnknown",
        PathPolarity::Positive => "vpiPositive",
        PathPolarity::Negative => "vpiNegative",
    }
}

/// `vpiTchkType` of a check and how many limits `vpi_get_delays` reports:
/// the timing check limits, offsets and second limits the clause names
/// (31.3, 31.4), not a `$width` threshold or the skew flags.
fn tchk_type(kind: TimingCheckKind) -> Option<(&'static str, u32)> {
    Some(match kind {
        TimingCheckKind::Setup => ("vpiSetup", 1),
        TimingCheckKind::Hold => ("vpiHold", 1),
        TimingCheckKind::SetupHold => ("vpiSetupHold", 2),
        TimingCheckKind::Recovery => ("vpiRecovery", 1),
        TimingCheckKind::Removal => ("vpiRemoval", 1),
        TimingCheckKind::RecRem => ("vpiRecrem", 2),
        TimingCheckKind::Skew => ("vpiSkew", 1),
        TimingCheckKind::TimeSkew => ("vpiTimeskew", 1),
        TimingCheckKind::FullSkew => ("vpiFullskew", 2),
        TimingCheckKind::Period => ("vpiPeriod", 1),
        TimingCheckKind::Width => ("vpiWidth", 1),
        TimingCheckKind::NoChange => ("vpiNoChange", 2),
        TimingCheckKind::Unknown => return None,
    })
}

/// The model values of limit `limit` (0 the first, 1 the second) of a
/// check with rows `rows`, as an SDF annotation of that limit targets them
/// (`map_sdf_annotations`).
fn check_targets(kind: TimingCheckKind, rows: &[SdfCheckRow], limit: u32) -> Vec<IrSdfTarget> {
    if let Some(pair) = rows.iter().find_map(|row| row.pair) {
        // `$setuphold` holds setup before and hold after the reference
        // event, `$recrem` recovery after and removal before.
        let after = (kind == TimingCheckKind::SetupHold) == (limit == 1);
        return vec![IrSdfTarget::Pair {
            set: rows[0].set,
            pair,
            after,
        }];
    }
    rows.iter()
        .filter_map(|row| {
            let second = match (limit, row.slot) {
                (0, SdfLimitSlot::First | SdfLimitSlot::Both) | (1, SdfLimitSlot::Second) => false,
                (1, SdfLimitSlot::Both) => true,
                _ => return None,
            };
            Some(IrSdfTarget::Check {
                set: row.set,
                check: row.check,
                second,
            })
        })
        .collect()
}

impl Codegen<'_> {
    /// The timing rows of every module path and timing check of the design
    /// and the delays of its primitives and continuous assignments. Runs
    /// after lowering, when the model rows exist.
    pub(in super::super::super) fn vpi_timing_metadata(
        &mut self,
        tops: &[NodeId],
    ) -> VpiTimingMetadata {
        let mut metadata = VpiTimingMetadata::default();
        // A node of an untaken generate block keeps its parent chain but
        // does not exist (27.3).
        let reachable = |cg: &Self, node: NodeId| {
            if cg.db.is_uninstantiated(node) {
                return false;
            }
            let mut current = Some(node);
            while let Some(id) = current {
                if tops.contains(&id) {
                    return true;
                }
                current = cg.node(id).parent;
            }
            false
        };
        let path_rows: HashMap<PathPair, SdfPathRow> = self
            .sdf_path_rows
            .iter()
            .map(|(node, row)| ((*node, row.input, row.output), *row))
            .collect();
        let mut check_rows: HashMap<NodeId, Vec<SdfCheckRow>> = HashMap::new();
        for (node, row) in &self.sdf_check_rows {
            check_rows.entry(*node).or_default().push(*row);
        }
        let db = self.db;
        // The first pair of every declaration comes before the other pairs
        // (decision A19-D9).
        let mut other_pairs = Vec::new();
        for path in db.module_paths() {
            if !reachable(self, path.node) {
                continue;
            }
            let Some(instance) = self.owning_inst(path.node) else {
                continue;
            };
            let mut rows = Vec::new();
            for (pair, row) in self.vpi_path_rows(path, instance, &path_rows) {
                rows.push(row.full_name.clone());
                if pair == (0, 0) {
                    metadata.rows.push(row);
                } else {
                    other_pairs.push(row);
                }
            }
            if !rows.is_empty() {
                metadata.row_delays.push(VpiRowDelays {
                    rows,
                    scope: self.vpi_full_name(instance),
                    expressions: path.delays.clone(),
                });
            }
        }
        metadata.rows.append(&mut other_pairs);
        for check in db.timing_checks() {
            if !reachable(self, check.node) {
                continue;
            }
            let Some(instance) = self.owning_inst(check.node) else {
                continue;
            };
            let rows = check_rows.remove(&check.node).unwrap_or_default();
            if let Some(row) = self.vpi_tchk_row(check, instance, &rows) {
                // The limits are the check's delays (37.36, 37.3.4); a
                // `$width` threshold is not one.
                let limits = row.timing.as_ref().map_or(0, |timing| timing.count) as usize;
                metadata.row_delays.push(VpiRowDelays {
                    rows: vec![row.full_name.clone()],
                    scope: self.vpi_full_name(instance),
                    expressions: check
                        .limits
                        .iter()
                        .take(limits)
                        .flatten()
                        .copied()
                        .collect(),
                });
                metadata.rows.push(row);
            }
        }
        for id in db.node_ids() {
            let delay = match self.kind(id) {
                NodeKind::Gate { delay, .. } => *delay,
                NodeKind::ContAssign { delay, .. } => *delay,
                _ => continue,
            };
            if !reachable(self, id) {
                continue;
            }
            let mut timing = self.vpi_driver_delays(id, delay);
            if let Some(slot) = self
                .sdf_gate_slot(id)
                .or_else(|| self.vpi_runtime_gate_slot(id))
            {
                timing.targets.push((0, IrSdfTarget::Gate { slot }));
            }
            if let Some(delay) = delay {
                metadata.delay_nodes.insert(id, driver_delay_nodes(delay));
            }
            metadata.delays.insert(id, timing);
        }
        for (port, bit, load) in self.sdf_port_loads() {
            metadata
                .port_loads
                .entry(port)
                .or_default()
                .push((bit, load));
        }
        for (net, delay) in db.net_delays() {
            if reachable(self, *net) {
                metadata
                    .delay_nodes
                    .insert(*net, driver_delay_nodes(*delay));
            }
        }
        metadata
    }

    /// The declared rise, fall and turn-off delays of a gate or continuous
    /// assignment; none for a delay computed at run time.
    fn vpi_driver_delays(&mut self, node: NodeId, delay: Option<DriverDelay>) -> IrVpiTiming {
        let Some(delay) = delay else {
            return IrVpiTiming {
                declared: vec![0; 3],
                ..IrVpiTiming::default()
            };
        };
        let count = match delay {
            DriverDelay::Single(_) => 1,
            DriverDelay::RiseFall(..) => 2,
            DriverDelay::RiseFallTurnOff(..) => 3,
        };
        let declared = if self.driver_delay_is_runtime(delay) {
            Vec::new()
        } else {
            self.driver_delay_ticks(node, delay)
                .map(|delay| {
                    [delay.rise, delay.fall, delay.turn_off]
                        .iter()
                        .map(|ticks| i64::try_from(*ticks).unwrap_or(i64::MAX))
                        .collect()
                })
                .unwrap_or_default()
        };
        IrVpiTiming {
            declared,
            count,
            ..IrVpiTiming::default()
        }
    }

    /// The rows of one module path declaration of `instance` (37.35), one
    /// per input and output terminal pair, inputs first, each with its pair:
    /// its terminals, the declaration's condition and delays, and the model
    /// path it became. A pair an `ifnone` declaration of the same path
    /// replaces is no path (decision A8-D3) and has no row.
    /// `vpiModPathHasIfNone` holds for an `ifnone` path only, not for the
    /// state-dependent paths sharing its terminals (decision A19-D9).
    fn vpi_path_rows(
        &mut self,
        path: &crate::core::db::ModulePath,
        instance: NodeId,
        rows: &HashMap<PathPair, SdfPathRow>,
    ) -> Vec<((u32, u32), IrVpiObject)> {
        let values = path
            .delays
            .iter()
            .map(|delay| self.path_delay_ticks(path.node, *delay))
            .collect::<Result<Vec<_>, _>>()
            .ok();
        let declared: Vec<i64> = values
            .as_deref()
            .and_then(expand_path_delays)
            .map(|table| {
                table
                    .iter()
                    .map(|ticks| i64::try_from(*ticks).unwrap_or(i64::MAX))
                    .collect()
            })
            .unwrap_or_default();
        let edge = |input: usize| match path.input_edge(input) {
            None => 0,
            Some(PathEdge::Posedge) => POSEDGE,
            Some(PathEdge::Negedge) => NEGEDGE,
            Some(PathEdge::Edge) => POSEDGE | NEGEDGE,
        };
        let term = |cg: &Self, node: NodeId, direction, role, edge| IrVpiConn {
            role,
            edge,
            ..cg.vpi_conn(node, direction)
        };
        let terminals =
            |cg: &Self, nodes: &[NodeId], direction, role, edge: &dyn Fn(usize) -> u8| {
                nodes
                    .iter()
                    .enumerate()
                    .map(|(index, node)| {
                        (
                            term(cg, *node, direction, role, edge(index)),
                            cg.vpi_terminal_text(*node),
                        )
                    })
                    .collect::<Vec<_>>()
            };
        let inputs = terminals(
            self,
            &path.inputs,
            IrVpiDirection::Input,
            IrVpiConnRole::PathIn,
            &edge,
        );
        let outputs = terminals(
            self,
            &path.outputs,
            IrVpiDirection::Output,
            IrVpiConnRole::PathOut,
            &|_| 0,
        );
        // The terminals every pair has.
        let mut shared = Vec::new();
        if let Some(data) = path.data_source {
            shared.push(term(
                self,
                data,
                IrVpiDirection::Input,
                IrVpiConnRole::PathData,
                0,
            ));
        }
        if let PathCondition::If(condition) = path.condition {
            shared.push(term(
                self,
                condition,
                IrVpiDirection::None,
                IrVpiConnRole::Condition,
                0,
            ));
        }
        let scope = self.vpi_full_name(instance);
        let shown = scope.replace(SEPARATOR, ".");
        let key = self.vpi_unnamed_key(&scope, 'm', path.node);
        let node = self.node(path.node);
        let time_unit_fs = self.timescale_of_node(path.node).unit_fs;
        let mut pairs = Vec::with_capacity(inputs.len() * outputs.len());
        for (input, (input_term, input_text)) in (0u32..).zip(&inputs) {
            for (output, (output_term, output_text)) in (0u32..).zip(&outputs) {
                let identity = (path.node, input, output);
                if self.dropped_path_pairs.contains(&identity) {
                    continue;
                }
                let mut conns = Vec::with_capacity(2 + shared.len());
                conns.extend([input_term.clone(), output_term.clone()]);
                conns.extend(shared.iter().cloned());
                let row = IrVpiObject {
                    full_name: format!("{key}.{input}.{output}"),
                    // A path is never found by name, but reports
                    // `<scope>.<in>$<out>` as both vpiName and vpiFullName
                    // (decision A19-D9).
                    name: input_text
                        .as_ref()
                        .zip(output_text.as_ref())
                        .map(|(input, output)| format!("{shown}.{input}${output}"))
                        .unwrap_or_default(),
                    kind: IrVpiObjectKind::ModPath,
                    unnamed: true,
                    file: node.file().map(str::to_owned),
                    line: node.line,
                    time_unit_fs,
                    detail: Some(match path.connection {
                        PathConnection::Full => "vpiPathFull",
                        _ => "vpiPathParallel",
                    }),
                    conns,
                    timing: Some(IrVpiTiming {
                        targets: rows
                            .get(&identity)
                            .map(|row| {
                                (
                                    0,
                                    IrSdfTarget::Path {
                                        set: row.set,
                                        destination: row.destination,
                                        path: row.path,
                                    },
                                )
                            })
                            .into_iter()
                            .collect(),
                        declared: declared.clone(),
                        count: path.delays.len() as u32,
                        polarity: polarity_code(path.polarity),
                        // Without a data source the data path is the path
                        // itself, so its polarity is the path's (decision
                        // A19-D9).
                        data_polarity: if path.edge.is_some() {
                            polarity_code(path.edge_polarity)
                        } else {
                            polarity_code(path.polarity)
                        },
                        has_ifnone: path.condition == PathCondition::IfNone,
                        delay: None,
                    }),
                    ..IrVpiObject::default()
                };
                pairs.push(((input, output), row));
            }
        }
        pairs
    }

    /// The row of one timing check of `instance` (37.36): its terminals, its
    /// notifier, then its other arguments in source order, the conditions of its terminals, and its limits with
    /// the model rows that hold them.
    fn vpi_tchk_row(
        &mut self,
        check: &crate::core::db::TimingCheck,
        instance: NodeId,
        rows: &[SdfCheckRow],
    ) -> Option<IrVpiObject> {
        let (type_code, count) = tchk_type(check.kind)?;
        // `$setup` alone lists its data event first (31.3.1).
        let roles = if check.kind == TimingCheckKind::Setup {
            [IrVpiConnRole::TchkData, IrVpiConnRole::TchkRef]
        } else {
            [IrVpiConnRole::TchkRef, IrVpiConnRole::TchkData]
        };
        let mut conns = Vec::new();
        let mut conditions = Vec::new();
        for (event, role) in check.events.iter().zip(roles) {
            let Some(event) = event else { continue };
            conns.push(IrVpiConn {
                role,
                edge: tchk_edge(event.edges),
                ..self.vpi_conn(event.terminal, IrVpiDirection::None)
            });
            if let Some(condition) = event.condition {
                conditions.push((conns.len() - 1, condition));
            }
        }
        let argument = |cg: &Self, node: NodeId, role| IrVpiConn {
            role,
            ..cg.vpi_conn(node, IrVpiDirection::None)
        };
        // The notifier follows the terminals, ahead of every limit
        // (industry-standard order; decision A19-D10).
        if let Some(notifier) = check.notifier {
            conns.push(argument(self, notifier, IrVpiConnRole::Notifier));
        }
        let mut after = check.limits.iter().flatten().copied().collect::<Vec<_>>();
        after.extend(check.stamp_condition);
        after.extend(check.check_condition);
        after.extend(check.delayed.iter().flatten());
        for node in after {
            conns.push(argument(self, node, IrVpiConnRole::Argument));
        }
        for (term, condition) in conditions {
            conns[term].condition = Some(conns.len() as u32);
            conns.push(argument(self, condition, IrVpiConnRole::TermCondition));
        }
        let declared = (0..count as usize)
            .map(|index| match check.limits.get(index).copied().flatten() {
                Some(limit) => self
                    .timing_limit_ticks_signed(check.node, limit)
                    .unwrap_or(0),
                None => 0,
            })
            .collect();
        let targets = (0..count)
            .flat_map(|limit| {
                check_targets(check.kind, rows, limit)
                    .into_iter()
                    .map(move |target| (limit, target))
            })
            .collect();
        let scope = self.vpi_full_name(instance);
        let node = self.node(check.node);
        Some(IrVpiObject {
            full_name: self.vpi_unnamed_key(&scope, 't', check.node),
            kind: IrVpiObjectKind::Tchk,
            unnamed: true,
            file: node.file().map(str::to_owned),
            line: node.line,
            time_unit_fs: self.timescale_of_node(check.node).unit_fs,
            detail: Some(type_code),
            conns,
            timing: Some(IrVpiTiming {
                targets,
                declared,
                count,
                polarity: "vpiUnknown",
                data_polarity: "vpiUnknown",
                has_ifnone: false,
                delay: None,
            }),
            ..IrVpiObject::default()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timing_check_edges_map_to_vpi_edge_bits() {
        assert_eq!(
            tchk_edge(TimingEdges::from_bits(TimingEdges::RISING)),
            POSEDGE
        );
        assert_eq!(
            tchk_edge(TimingEdges::from_bits(TimingEdges::FALLING)),
            NEGEDGE
        );
        assert_eq!(tchk_edge(TimingEdges::from_bits(TimingEdges::T01)), EDGE_01);
        assert_eq!(tchk_edge(TimingEdges::from_bits(0)), 0);
    }

    #[test]
    fn combined_limits_target_their_pair_or_rows() {
        let row = |check, slot| SdfCheckRow {
            set: 3,
            check,
            slot,
            pair: None,
        };
        let rows = [row(0, SdfLimitSlot::First), row(1, SdfLimitSlot::Second)];
        assert_eq!(
            check_targets(TimingCheckKind::SetupHold, &rows, 1),
            vec![IrSdfTarget::Check {
                set: 3,
                check: 1,
                second: false
            }]
        );
        let both = [row(4, SdfLimitSlot::Both)];
        assert_eq!(
            check_targets(TimingCheckKind::NoChange, &both, 1),
            vec![IrSdfTarget::Check {
                set: 3,
                check: 4,
                second: true
            }]
        );
        let paired = [SdfCheckRow {
            pair: Some(2),
            ..row(0, SdfLimitSlot::First)
        }];
        assert_eq!(
            check_targets(TimingCheckKind::RecRem, &paired, 0),
            vec![IrSdfTarget::Pair {
                set: 3,
                pair: 2,
                after: true
            }]
        );
        assert_eq!(tchk_type(TimingCheckKind::Width), Some(("vpiWidth", 1)));
    }
}
