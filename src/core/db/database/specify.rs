//! Specify blocks, specparams and module paths (IEEE 1364-2001 3.11.3 and
//! 14, IEEE 1800-2009 6.20.5 and 30).
//!
//! The specify nodes stay [`NodeKind::Other`] (their stable identity and
//! source range); these side tables hold their typed facts. The terminal,
//! delay and condition expressions are ordinary owned expression nodes.

use super::*;

/// How a module path connects source and destination bits (V 14.2.5,
/// SV 30.4.5): `*>` connects every source bit to every destination bit,
/// `=>` bit `i` to bit `i`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathConnection {
    Full,
    Parallel,
}

/// The polarity operator of a path (V 14.2.7, SV 30.4.7). It has no
/// simulation effect; timing analysis and SDF consumers read it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathPolarity {
    Unknown,
    Positive,
    Negative,
}

/// The edge identifier of an edge-sensitive path (V 14.2.3, SV 30.4.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathEdge {
    Posedge,
    Negedge,
    Edge,
}

/// The condition of a state-dependent path (V 14.2.4, SV 30.4.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathCondition {
    /// A simple or edge-sensitive path with no condition.
    None,
    /// `if (expression)`.
    If(NodeId),
    /// `ifnone`.
    IfNone,
}

/// One module path declaration, in source order within its specify block.
/// A full declaration with several terminals (`(a, b *> q, r)`) stays one
/// record, as the frontend keeps it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModulePath {
    /// The path declaration node: its stable source identity.
    pub node: NodeId,
    /// The enclosing specify block.
    pub block: NodeId,
    pub connection: PathConnection,
    pub polarity: PathPolarity,
    pub edge: Option<PathEdge>,
    /// Polarity of an edge-sensitive path's data source (`q +: d`).
    pub edge_polarity: PathPolarity,
    pub condition: PathCondition,
    /// Data source expression of an edge-sensitive path.
    pub data_source: Option<NodeId>,
    /// Input terminal expressions in source order.
    pub inputs: Vec<NodeId>,
    /// Output terminal expressions in source order.
    pub outputs: Vec<NodeId>,
    /// Path delay value expressions in source order (one, two, three, six
    /// or twelve in a valid design).
    pub delays: Vec<NodeId>,
}

/// A pulse-style or showcancelled declaration (V 14.6.4, SV 30.7.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PulseStyle {
    OnEvent,
    OnDetect,
    ShowCancelled,
    NoShowCancelled,
}

/// The kind of a system timing check (V 15, SV 31).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimingCheckKind {
    Setup,
    Hold,
    SetupHold,
    Recovery,
    Removal,
    RecRem,
    Skew,
    TimeSkew,
    FullSkew,
    Period,
    Width,
    NoChange,
    /// A check the frontend could not classify (it has reported an error).
    Unknown,
}

impl TimingCheckKind {
    /// The system task name, `$setup` through `$nochange`.
    pub fn name(self) -> &'static str {
        match self {
            Self::Setup => "$setup",
            Self::Hold => "$hold",
            Self::SetupHold => "$setuphold",
            Self::Recovery => "$recovery",
            Self::Removal => "$removal",
            Self::RecRem => "$recrem",
            Self::Skew => "$skew",
            Self::TimeSkew => "$timeskew",
            Self::FullSkew => "$fullskew",
            Self::Period => "$period",
            Self::Width => "$width",
            Self::NoChange => "$nochange",
            Self::Unknown => "system timing check",
        }
    }
}

/// The transitions a timing check event detects (V 15.4, SV 31.5), one bit
/// each in [`TimingEdges::T01`] .. [`TimingEdges::TX1`] order; z counts as
/// x. An empty set is an event without edge control: any transition.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TimingEdges(u8);

impl TimingEdges {
    pub const T01: u8 = 1 << 0;
    pub const T0X: u8 = 1 << 1;
    pub const T10: u8 = 1 << 2;
    pub const T1X: u8 = 1 << 3;
    pub const TX0: u8 = 1 << 4;
    pub const TX1: u8 = 1 << 5;
    /// `posedge`: `edge[01, 0x, x1]`.
    pub const RISING: u8 = Self::T01 | Self::T0X | Self::TX1;
    /// `negedge`: `edge[10, x0, 1x]`.
    pub const FALLING: u8 = Self::T10 | Self::T1X | Self::TX0;

    pub fn from_bits(bits: u8) -> Self {
        Self(bits & (Self::RISING | Self::FALLING))
    }

    pub fn bits(self) -> u8 {
        self.0
    }

    /// No edge control: every transition is an event.
    pub fn is_any(self) -> bool {
        self.0 == 0
    }
}

/// One event argument of a timing check: its terminal expression, edge
/// control and `&&&` condition (V 15.6, SV 31.7).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimingEvent {
    pub terminal: NodeId,
    pub edges: TimingEdges,
    pub condition: Option<NodeId>,
}

/// One system timing check with its arguments (V 15, SV 31). The check node
/// and its source range are the stable identity (back-annotation keys on it
/// within an instance).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TimingCheck {
    pub node: NodeId,
    /// The enclosing specify block.
    pub block: NodeId,
    pub kind: TimingCheckKind,
    /// The event arguments in signature order (`$setup` lists the data event
    /// first); `None` for one the frontend could not bind.
    pub events: [Option<TimingEvent>; 2],
    /// The limit arguments in signature order (limit, threshold or offsets);
    /// `None` for an empty position.
    pub limits: Vec<Option<NodeId>>,
    pub notifier: Option<NodeId>,
    /// Actual arguments in the source, empty positions included.
    pub argument_count: u8,
    /// Bit `i`: argument position `i` is empty (`$width(c, l, , n)`).
    pub empty_arguments: u16,
}

/// A specify item other than a path or specparam.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SpecifyItemKind {
    /// A pulse-style declaration and its output terminal expressions.
    PulseStyle(PulseStyle, Vec<NodeId>),
    TimingCheck(TimingCheckKind),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpecifyItem {
    pub node: NodeId,
    pub block: NodeId,
    pub kind: SpecifyItemKind,
}

/// Specparam facts kept beside the [`NodeKind::Param`] declaration, so
/// specify parameters stay distinct from elaboration parameters (V Table 8,
/// SV Table 6-11).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Specparam {
    /// Declared inside a specify block rather than the module body.
    pub in_specify: bool,
    /// A `PATHPULSE$` pulse-limit specparam (V 14.6.1, SV 30.7.1).
    pub pathpulse: bool,
}

/// The module paths a `PATHPULSE$` specparam applies to (V 14.6.1,
/// SV 30.7.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathPulseScope {
    /// `PATHPULSE$`: every path of the module without a path-specific one.
    Module,
    /// `PATHPULSE$in$out`: the input and output terminal declarations the
    /// name resolves to.
    Path { input: NodeId, output: NodeId },
    /// A name whose terminals do not resolve; it applies to no path.
    Unresolved,
}

/// One `PATHPULSE$` pulse-limit specparam with its evaluated limits (the
/// `min:typ:max` operand is the compilation's delay selection).
#[derive(Clone, Debug, PartialEq)]
pub struct PathPulse {
    /// The specparam declaration.
    pub node: NodeId,
    /// The enclosing specify block.
    pub block: NodeId,
    pub scope: PathPulseScope,
    /// The reject limit, `None` when it did not evaluate.
    pub reject: Option<Val>,
    /// The error limit; `None` when only the reject limit is given (it then
    /// applies to both) or it did not evaluate.
    pub error: Option<Val>,
}

/// Every specify fact of a design, in capture order.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct SpecifyTables {
    pub(super) blocks: Vec<NodeId>,
    pub(super) paths: Vec<ModulePath>,
    pub(super) items: Vec<SpecifyItem>,
    pub(super) specparams: HashMap<NodeId, Specparam>,
    pub(super) path_pulses: Vec<PathPulse>,
    pub(super) timing_checks: Vec<TimingCheck>,
}

/// Dense, unique `role[i]` targets in index order.
fn indexed_targets(
    ids: &SemanticIds,
    edges: &[crate::ffi::slang::SemanticEdge],
    role: SemanticEdgeRole,
) -> Result<Vec<NodeId>, DbError> {
    let mut indexed = edges
        .iter()
        .filter(|edge| edge.role == role)
        .collect::<Vec<_>>();
    indexed.sort_by_key(|edge| edge.index);
    indexed
        .into_iter()
        .enumerate()
        .map(|(expected, edge)| {
            if usize::try_from(edge.index).ok() != Some(expected) {
                return Err(DbError::InvalidSnapshot(format!(
                    "specify item {role:?} indices must be dense and unique"
                )));
            }
            semantic_id(ids, edge.target_id)
        })
        .collect()
}

fn path_polarity(field: u64) -> PathPolarity {
    match field {
        1 => PathPolarity::Positive,
        2 => PathPolarity::Negative,
        _ => PathPolarity::Unknown,
    }
}

fn path_edge(field: u64) -> Option<PathEdge> {
    match field {
        1 => Some(PathEdge::Posedge),
        2 => Some(PathEdge::Negedge),
        3 => Some(PathEdge::Edge),
        _ => None,
    }
}

fn timing_check_kind(code: u64) -> TimingCheckKind {
    use TimingCheckKind::*;
    const KINDS: [TimingCheckKind; 12] = [
        Setup, Hold, SetupHold, Recovery, Removal, RecRem, Skew, TimeSkew, FullSkew, Period, Width,
        NoChange,
    ];
    code.checked_sub(1)
        .and_then(|index| usize::try_from(index).ok())
        .and_then(|index| KINDS.get(index).copied())
        .unwrap_or(Unknown)
}

/// The arguments of one TIMING_CHECK node (ABI v38 fields; the edge roles
/// are documented with `LLG_SLANG_SPECIFY_TIMING_CHECK`).
fn timing_check_from_slang(
    ids: &SemanticIds,
    edges: &[crate::ffi::slang::SemanticEdge],
    semantic: &SemanticNode,
    node: NodeId,
    block: NodeId,
) -> Result<TimingCheck, DbError> {
    use crate::ffi::slang::{
        SPECIFY_CHECK_COUNT_SHIFT, SPECIFY_CHECK_EMPTY_SHIFT, SPECIFY_CHECK_EVENT_BITS,
        SPECIFY_CHECK_KIND_MASK, SPECIFY_EDGE_MASK,
    };
    let auxiliary = semantic.auxiliary;
    let mut events = [None, None];
    for (index, event) in events.iter_mut().enumerate() {
        let index = index as u32;
        let Some(terminal) = edge_target_at(ids, edges, SemanticEdgeRole::Operand, index)? else {
            continue;
        };
        let mask = (auxiliary >> (SPECIFY_CHECK_EVENT_BITS * (index + 1))) & SPECIFY_EDGE_MASK;
        *event = Some(TimingEvent {
            terminal,
            edges: TimingEdges::from_bits(mask as u8),
            condition: edge_target_at(ids, edges, SemanticEdgeRole::Condition, index)?,
        });
    }
    let slots = edges
        .iter()
        .filter(|edge| edge.role == SemanticEdgeRole::Delay)
        .map(|edge| edge.index as usize + 1)
        .max()
        .unwrap_or(0);
    let mut limits = vec![None; slots];
    for (index, limit) in limits.iter_mut().enumerate() {
        *limit = edge_target_at(ids, edges, SemanticEdgeRole::Delay, index as u32)?;
    }
    Ok(TimingCheck {
        node,
        block,
        kind: timing_check_kind(auxiliary & SPECIFY_CHECK_KIND_MASK),
        events,
        limits,
        notifier: edge_target(ids, edges, SemanticEdgeRole::Lhs)?,
        argument_count: ((auxiliary >> SPECIFY_CHECK_COUNT_SHIFT) & 0xff) as u8,
        empty_arguments: ((auxiliary >> SPECIFY_CHECK_EMPTY_SHIFT) & 0xffff) as u16,
    })
}

/// The limits and terminals of one `PATHPULSE$` specparam node: its constant
/// is the reject limit, its auxiliary one more than the error limit's constant
/// id, and OPERAND[0]/LHS[0] the resolved terminals.
fn path_pulse_from_slang(
    snapshot: &SlangSnapshot,
    ids: &SemanticIds,
    semantic: &SemanticNode,
    node: NodeId,
) -> Result<PathPulse, DbError> {
    let constant = |id: u64| -> Result<Option<Val>, DbError> {
        let constant = snapshot.constants.get(id as usize).ok_or_else(|| {
            DbError::InvalidSnapshot("PATHPULSE$ limit names a missing constant".into())
        })?;
        Ok(val_from_slang(&constant.value))
    };
    let reject = match semantic.constant_id() {
        Some(id) => constant(id)?,
        None => None,
    };
    let error = match semantic.auxiliary.checked_sub(1) {
        Some(id) => constant(id)?,
        None => None,
    };
    let edges = semantic_edges(snapshot, semantic)?;
    let input = edge_target(ids, edges, SemanticEdgeRole::Operand)?;
    let output = edge_target(ids, edges, SemanticEdgeRole::Lhs)?;
    let scope = match (input, output) {
        (Some(input), Some(output)) => PathPulseScope::Path { input, output },
        _ if semantic.name == "PATHPULSE$" => PathPulseScope::Module,
        _ => PathPulseScope::Unresolved,
    };
    let block = semantic
        .parent_id()
        .and_then(|parent| snapshot.semantic_nodes.get(parent as usize))
        .filter(|parent| parent.kind == SemanticKind::Specify)
        .map(|parent| semantic_id(ids, parent.id))
        .transpose()?
        .ok_or_else(|| {
            DbError::InvalidSnapshot("PATHPULSE$ specparam is outside a specify block".into())
        })?;
    Ok(PathPulse {
        node,
        block,
        scope,
        reject,
        error,
    })
}

/// Import the specify side tables. The ABI layer has validated subkinds and
/// auxiliary fields; this checks the edge shapes. Language rules (terminal
/// directions, delay counts) are the frontend's and lowering's: an
/// erroneous design keeps its partial facts for navigation.
pub(super) fn specify_tables_from_slang(
    snapshot: &SlangSnapshot,
    ids: &SemanticIds,
) -> Result<SpecifyTables, DbError> {
    use crate::ffi::slang::{
        SEMANTIC_PARAMETER_PATHPULSE, SEMANTIC_PARAMETER_SPECPARAM, SEMANTIC_SPECIFY_BLOCK,
        SEMANTIC_SPECIFY_PATH, SEMANTIC_SPECIFY_PULSE_STYLE, SEMANTIC_SPECIFY_TIMING_CHECK,
        SPECIFY_PATH_EDGE_POLARITY_SHIFT, SPECIFY_PATH_EDGE_SHIFT, SPECIFY_PATH_FIELD_MASK,
        SPECIFY_PATH_FULL, SPECIFY_PATH_POLARITY_SHIFT, SPECIFY_PATH_STATE_DEPENDENT,
    };
    let mut tables = SpecifyTables::default();
    let block_of = |semantic: &SemanticNode| -> Result<NodeId, DbError> {
        let parent = semantic
            .parent_id()
            .and_then(|parent| snapshot.semantic_nodes.get(parent as usize))
            .filter(|parent| {
                parent.kind == SemanticKind::Specify && parent.subkind == SEMANTIC_SPECIFY_BLOCK
            })
            .ok_or_else(|| {
                DbError::InvalidSnapshot("specify item is not owned by a specify block".into())
            })?;
        semantic_id(ids, parent.id)
    };
    for semantic in &snapshot.semantic_nodes {
        let node = semantic_id(ids, semantic.id)?;
        if semantic.kind == SemanticKind::Parameter {
            let pathpulse = match semantic.subkind {
                SEMANTIC_PARAMETER_SPECPARAM => false,
                SEMANTIC_PARAMETER_PATHPULSE => true,
                _ => continue,
            };
            let in_specify = semantic
                .parent_id()
                .and_then(|parent| snapshot.semantic_nodes.get(parent as usize))
                .is_some_and(|parent| parent.kind == SemanticKind::Specify);
            tables.specparams.insert(
                node,
                Specparam {
                    in_specify,
                    pathpulse,
                },
            );
            if pathpulse {
                tables
                    .path_pulses
                    .push(path_pulse_from_slang(snapshot, ids, semantic, node)?);
            }
            continue;
        }
        if semantic.kind != SemanticKind::Specify {
            continue;
        }
        let edges = semantic_edges(snapshot, semantic)?;
        match semantic.subkind {
            SEMANTIC_SPECIFY_BLOCK => tables.blocks.push(node),
            SEMANTIC_SPECIFY_PATH => {
                let field = |shift: u32| (semantic.auxiliary >> shift) & SPECIFY_PATH_FIELD_MASK;
                let condition = edge_target(ids, edges, SemanticEdgeRole::Condition)?;
                let condition = match condition {
                    Some(condition) => PathCondition::If(condition),
                    None if semantic.auxiliary & SPECIFY_PATH_STATE_DEPENDENT != 0 => {
                        PathCondition::IfNone
                    }
                    None => PathCondition::None,
                };
                tables.paths.push(ModulePath {
                    node,
                    block: block_of(semantic)?,
                    connection: if semantic.auxiliary & SPECIFY_PATH_FULL != 0 {
                        PathConnection::Full
                    } else {
                        PathConnection::Parallel
                    },
                    polarity: path_polarity(field(SPECIFY_PATH_POLARITY_SHIFT)),
                    edge: path_edge(field(SPECIFY_PATH_EDGE_SHIFT)),
                    edge_polarity: path_polarity(field(SPECIFY_PATH_EDGE_POLARITY_SHIFT)),
                    condition,
                    data_source: edge_target(ids, edges, SemanticEdgeRole::Rhs)?,
                    inputs: indexed_targets(ids, edges, SemanticEdgeRole::Operand)?,
                    outputs: indexed_targets(ids, edges, SemanticEdgeRole::Lhs)?,
                    delays: indexed_targets(ids, edges, SemanticEdgeRole::Delay)?,
                });
            }
            SEMANTIC_SPECIFY_PULSE_STYLE => {
                let style = match semantic.auxiliary {
                    1 => PulseStyle::OnEvent,
                    2 => PulseStyle::OnDetect,
                    3 => PulseStyle::ShowCancelled,
                    _ => PulseStyle::NoShowCancelled,
                };
                tables.items.push(SpecifyItem {
                    node,
                    block: block_of(semantic)?,
                    kind: SpecifyItemKind::PulseStyle(
                        style,
                        indexed_targets(ids, edges, SemanticEdgeRole::Operand)?,
                    ),
                });
            }
            SEMANTIC_SPECIFY_TIMING_CHECK => {
                let block = block_of(semantic)?;
                let check = timing_check_from_slang(ids, edges, semantic, node, block)?;
                tables.items.push(SpecifyItem {
                    node,
                    block,
                    kind: SpecifyItemKind::TimingCheck(check.kind),
                });
                tables.timing_checks.push(check);
            }
            _ => {
                return Err(DbError::InvalidSnapshot(
                    "specify node has an unknown subkind".into(),
                ))
            }
        }
    }
    Ok(tables)
}

impl Db {
    /// Every specify block node, in capture order.
    pub fn specify_blocks(&self) -> &[NodeId] {
        &self.specify.blocks
    }

    /// Every module path declaration, in capture order (each elaborated
    /// instance has its own records).
    pub fn module_paths(&self) -> &[ModulePath] {
        &self.specify.paths
    }

    /// Pulse-style declarations and timing checks, in capture order.
    pub fn specify_items(&self) -> &[SpecifyItem] {
        &self.specify.items
    }

    /// Specparam facts of a [`NodeKind::Param`] declaration, `None` for an
    /// elaboration parameter.
    pub fn specparam(&self, node: NodeId) -> Option<Specparam> {
        self.specify.specparams.get(&node).copied()
    }

    /// Every `PATHPULSE$` specparam, in capture order (each elaborated
    /// instance has its own records).
    pub fn path_pulses(&self) -> &[PathPulse] {
        &self.specify.path_pulses
    }

    /// Every system timing check with its arguments, in capture order (each
    /// elaborated instance has its own records).
    pub fn timing_checks(&self) -> &[TimingCheck] {
        &self.specify.timing_checks
    }

    /// Every specparam declaration, in node order.
    pub fn specparams(&self) -> Vec<(NodeId, Specparam)> {
        let mut specparams = self
            .specify
            .specparams
            .iter()
            .map(|(node, specparam)| (*node, *specparam))
            .collect::<Vec<_>>();
        specparams.sort_by_key(|(node, _)| node.index());
        specparams
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::db::ExprKind;

    fn compile(source: &str) -> Db {
        let output = crate::core::compile::compile_sources_checked(
            &[crate::core::compile::OwnedSource::compilation_unit(
                "specify.sv",
                source,
            )],
            &crate::core::compile::CompileOpts {
                top: Some("tb".into()),
                ..Default::default()
            },
        )
        .unwrap();
        Db::from_slang(&output.snapshot).unwrap()
    }

    fn target_name(db: &Db, expression: NodeId) -> String {
        match db.node_kind(expression) {
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) => db.node(*target).name.clone(),
            other => panic!("terminal reference expected, found {other:?}"),
        }
    }

    const CELL: &str = "module c(input a, input [1:0] b, output y, output [1:0] z);
  specparam tpd = 3;
  assign y = a;
  assign z = b;
  specify
    specparam t2 = 5;
    showcancelled z;
    (a => y) = (tpd, t2);
    (b[0] *> y) = 1;
    (b -=> z) = (1, 2, 3, 4, 5, 6);
    $setup(a, posedge b[1], 2);
  endspecify
endmodule
module tb;
  reg a; reg [1:0] b; wire y; wire [1:0] z;
  c u(.a(a), .b(b), .y(y), .z(z));
endmodule
";

    #[test]
    fn module_paths_keep_terminals_connection_polarity_and_delays() {
        let db = compile(CELL);
        assert_eq!(db.specify_blocks().len(), 1);
        let block = db.specify_blocks()[0];
        let paths = db.module_paths();
        assert_eq!(paths.len(), 3);
        assert!(paths.iter().all(|path| path.block == block));
        assert!(paths
            .iter()
            .all(|path| path.condition == PathCondition::None
                && path.edge.is_none()
                && path.data_source.is_none()));

        assert_eq!(paths[0].connection, PathConnection::Parallel);
        assert_eq!(paths[0].polarity, PathPolarity::Unknown);
        assert_eq!(paths[0].inputs.len(), 1);
        assert_eq!(target_name(&db, paths[0].inputs[0]), "a");
        assert_eq!(target_name(&db, paths[0].outputs[0]), "y");
        assert_eq!(paths[0].delays.len(), 2);

        assert_eq!(paths[1].connection, PathConnection::Full);
        assert_eq!(paths[1].inputs.len(), 1);
        assert!(matches!(
            db.node_kind(paths[1].inputs[0]),
            NodeKind::Expr(ExprKind::BitSelect { .. })
        ));
        assert_eq!(paths[1].delays.len(), 1);

        assert_eq!(paths[2].polarity, PathPolarity::Negative);
        assert_eq!(paths[2].delays.len(), 6);

        // The path node is the stable source identity of the declaration.
        assert_eq!(db.node(paths[0].node).line, 8);
        assert_eq!(db.node(paths[2].node).line, 10);
    }

    #[test]
    fn specify_items_and_specparams_are_distinct_from_parameters() {
        let db = compile(CELL);
        let kinds = db
            .specify_items()
            .iter()
            .map(|item| match &item.kind {
                SpecifyItemKind::TimingCheck(kind) => kind.name().to_owned(),
                SpecifyItemKind::PulseStyle(style, terminals) => {
                    format!("{style:?}/{}", terminals.len())
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(kinds, ["ShowCancelled/1", "$setup"]);
        let specparams = db
            .specparams()
            .into_iter()
            .map(|(node, specparam)| (db.node(node).name.clone(), specparam))
            .collect::<Vec<_>>();
        assert_eq!(
            specparams,
            [
                (
                    "tpd".to_owned(),
                    Specparam {
                        in_specify: false,
                        pathpulse: false
                    }
                ),
                (
                    "t2".to_owned(),
                    Specparam {
                        in_specify: true,
                        pathpulse: false
                    }
                ),
            ]
        );
    }

    #[test]
    fn path_pulses_keep_limits_and_resolved_terminals() {
        let db = compile(
            "module c(input a, b, output y);
  assign y = a & b;
  specify
    (a => y) = 4;
    (b => y) = 5;
    specparam PATHPULSE$a$y = (1, 3);
    specparam PATHPULSE$ = 2;
    specparam PATHPULSE$q$y = 1;
  endspecify
endmodule
module tb;
  reg a, b; wire y;
  c u(.a(a), .b(b), .y(y));
endmodule
",
        );
        let pulses = db.path_pulses();
        assert_eq!(pulses.len(), 3);
        assert!(pulses
            .iter()
            .all(|pulse| pulse.block == db.specify_blocks()[0]));
        let names = |pulse: &PathPulse| match pulse.scope {
            PathPulseScope::Path { input, output } => {
                format!("{}->{}", db.node(input).name, db.node(output).name)
            }
            other => format!("{other:?}"),
        };
        let int = |value: &Option<Val>| match value {
            Some(Val::Bits(bits)) => bits.to_i128(),
            _ => None,
        };
        assert_eq!(names(&pulses[0]), "a->y");
        assert_eq!(
            (int(&pulses[0].reject), int(&pulses[0].error)),
            (Some(1), Some(3))
        );
        assert_eq!(names(&pulses[1]), "Module");
        assert_eq!(
            (int(&pulses[1].reject), pulses[1].error.clone()),
            (Some(2), None)
        );
        // A name whose input does not exist applies to no path.
        assert_eq!(names(&pulses[2]), "Unresolved");
        assert!(
            db.specparams()
                .iter()
                .filter(|(_, specparam)| specparam.pathpulse)
                .count()
                == 3
        );
    }

    #[test]
    fn timing_checks_keep_events_edges_conditions_limits_and_notifiers() {
        let db = compile(
            "module c(input d, input [1:0] c, input en);
  reg n;
  specify
    specparam t = 4;
    $setup(d, posedge c[1] &&& en, t, n);
    $width(edge[01, x1, 0z] c[0], 6, , n);
    $nochange(negedge c[0], d, 1, 2);
    $hold(posedge c[0], d, 3);
  endspecify
endmodule
module tb;
  reg d, en; reg [1:0] c;
  c u(.d(d), .c(c), .en(en));
endmodule
",
        );
        let checks = db.timing_checks();
        assert_eq!(checks.len(), 4);
        assert_eq!(db.specify_items().len(), 4);
        let setup = &checks[0];
        assert_eq!(setup.kind, TimingCheckKind::Setup);
        assert_eq!(setup.block, db.specify_blocks()[0]);
        assert_eq!((setup.argument_count, setup.empty_arguments), (4, 0));
        let data = setup.events[0].expect("data event");
        assert!(data.edges.is_any() && data.condition.is_none());
        assert_eq!(target_name(&db, data.terminal), "d");
        let reference = setup.events[1].expect("reference event");
        assert_eq!(reference.edges.bits(), TimingEdges::RISING);
        assert!(matches!(
            db.node_kind(reference.terminal),
            NodeKind::Expr(ExprKind::BitSelect { .. })
        ));
        assert_eq!(
            target_name(&db, reference.condition.expect("&&& condition")),
            "en"
        );
        assert_eq!(setup.limits.len(), 1);
        assert_eq!(target_name(&db, setup.notifier.expect("notifier")), "n");

        // The empty threshold leaves a gap that the argument fields record.
        let width = &checks[1];
        assert_eq!(width.kind, TimingCheckKind::Width);
        assert_eq!(
            width.events[0].expect("reference").edges.bits(),
            TimingEdges::T01 | TimingEdges::TX1 | TimingEdges::T0X
        );
        assert!(width.events[1].is_none());
        assert_eq!(width.limits.len(), 1);
        assert_eq!((width.argument_count, width.empty_arguments), (4, 1 << 2));
        assert!(width.notifier.is_some());

        let nochange = &checks[2];
        assert_eq!(
            nochange.events[0].unwrap().edges.bits(),
            TimingEdges::FALLING
        );
        assert_eq!(nochange.limits.len(), 2);
        assert!(nochange.limits.iter().all(Option::is_some));
        assert!(nochange.notifier.is_none());
        assert_eq!(checks[3].kind, TimingCheckKind::Hold);
        // The check node is the stable source identity.
        assert_eq!(db.node(setup.node).line, 5);
    }

    #[test]
    fn designs_without_specify_blocks_have_empty_tables() {
        let db = compile("module tb; parameter p = 1; wire w = p; endmodule\n");
        assert!(db.specify_blocks().is_empty());
        assert!(db.module_paths().is_empty());
        assert!(db.specify_items().is_empty());
        assert!(db.specparams().is_empty());
        assert!(db.path_pulses().is_empty());
        assert!(db.timing_checks().is_empty());
    }
}
