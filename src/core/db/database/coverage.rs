//! Functional-coverage side tables (IEEE 1800-2009 19).
//!
//! Covergroup types, coverpoints, bins and crosses keep `NodeKind::Other` as
//! their node kind so the language server and lint see them as declarations;
//! these tables carry the typed structure the simulator lowers. Nothing here
//! is allocated for a design without covergroups.

use super::*;
use crate::ffi::slang::{
    COVERAGE_BIN_ARRAY, COVERAGE_BIN_DEFAULT, COVERAGE_BIN_DEFAULT_SEQUENCE,
    COVERAGE_BIN_KIND_MASK, COVERAGE_BIN_SELECT, COVERAGE_BIN_SET, COVERAGE_BIN_TRANSITION,
    COVERAGE_BIN_WILDCARD, COVERAGE_BIN_WITH, COVERGROUP_EMBEDDED, COVERGROUP_EXTENDS,
    COVERGROUP_WITH_SAMPLE, COVER_CROSS_OF_CROSS, COVER_REPEAT_CONSECUTIVE, COVER_REPEAT_GOTO,
    COVER_REPEAT_NONCONSECUTIVE, COVER_REPEAT_NONE, COVER_SELECT_AND, COVER_SELECT_CONDITION,
    COVER_SELECT_NOT, COVER_SELECT_OR, SEMANTIC_COVERAGE_BIN, SEMANTIC_COVERAGE_BODY,
    SEMANTIC_COVERAGE_CROSS, SEMANTIC_COVERAGE_GROUP, SEMANTIC_COVERAGE_POINT,
    SEMANTIC_COVERAGE_SELECT, SEMANTIC_COVERAGE_TRANS_RANGE, SEMANTIC_COVERAGE_TRANS_SET,
    SEMANTIC_TIMING_BLOCK_EVENT_LIST,
};

/// One covergroup type declaration (19.3).
#[derive(Debug, Clone, PartialEq)]
pub struct CovergroupInfo {
    /// The group's own type; handles declared with the group name carry it.
    pub type_id: Option<TypeId>,
    /// Construct formals, in declaration order (`covergroup cg(int a)`).
    pub formals: Vec<NodeId>,
    /// `with function sample(...)` formals, in declaration order (19.8.1).
    pub sample_formals: Vec<NodeId>,
    /// The coverage event, when one is declared.
    pub event: Option<CoverageEvent>,
    /// Group-level `option.x = e` / `type_option.x = e` setters, as
    /// assignment expressions in source order.
    pub options: Vec<NodeId>,
    /// The body scope that owns coverpoints and crosses.
    pub body: Option<NodeId>,
    /// Coverpoints in declaration order.
    pub coverpoints: Vec<NodeId>,
    /// Crosses in declaration order.
    pub crosses: Vec<NodeId>,
    /// Declared inside a class (19.4): one anonymous type per class.
    pub embedded: bool,
    /// Declared `with function sample(...)`.
    pub with_sample: bool,
    /// Declared `extends` another group (not 1800-2009 syntax; rejected).
    pub extends: bool,
}

/// The coverage event of a covergroup (19.3).
#[derive(Debug, Clone, PartialEq)]
pub enum CoverageEvent {
    /// A clocking event `@(...)`.
    Clocking {
        timing: NodeId,
        specs: Vec<EventSpec>,
        implicit: bool,
    },
    /// A block event list `@@(begin f or end g)`.
    Block {
        timing: NodeId,
        events: Vec<CoverageBlockEvent>,
    },
}

/// One `begin`/`end` entry of a block event list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoverageBlockEvent {
    /// `true` for `begin`, `false` for `end`.
    pub begin: bool,
    /// The named block, task, function or method.
    pub target: NodeId,
}

/// One coverpoint (19.5).
#[derive(Debug, Clone, PartialEq)]
pub struct CoverpointInfo {
    /// The covergroup type that declares the coverpoint.
    pub group: NodeId,
    /// The coverage expression.
    pub expr: NodeId,
    /// The `iff` guard, when present.
    pub iff: Option<NodeId>,
    /// The coverpoint type.
    pub ty: TypeInfo,
    pub type_id: Option<TypeId>,
    /// Coverpoint-level option setters, in source order.
    pub options: Vec<NodeId>,
    /// Explicit bins declarations, in source order.
    pub bins: Vec<NodeId>,
}

/// The declared kind of a bins construct (19.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoverageBinKind {
    Bins,
    Illegal,
    Ignore,
}

/// One bins declaration (19.5).
#[derive(Debug, Clone, PartialEq)]
pub struct CoverageBinInfo {
    pub point: NodeId,
    pub kind: CoverageBinKind,
    /// `bins b[]` or `bins b[N]`.
    pub array: bool,
    pub wildcard: bool,
    pub default: bool,
    pub default_sequence: bool,
    pub transition: bool,
    pub with: bool,
    pub set: bool,
    pub select: bool,
    /// Open-range-list items (values or value ranges).
    pub values: Vec<NodeId>,
    /// Transition sets (19.5.1): each a sequence of range lists.
    pub transitions: Vec<Vec<CoverageTransRange>>,
    /// The select expression of a cross bin (19.6).
    pub select_expr: Option<CoverageSelect>,
    /// The `[N]` count expression.
    pub count: Option<NodeId>,
    pub iff: Option<NodeId>,
}

/// The repetition of one transition range list (19.5.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoverageRepeat {
    /// No repetition: exactly one sample.
    None,
    /// `[* n]` / `[* n:m]`.
    Consecutive,
    /// `[= n]` / `[= n:m]`.
    Nonconsecutive,
    /// `[-> n]` / `[-> n:m]`.
    GoTo,
}

/// One `trans_range_list` of a transition set.
#[derive(Debug, Clone, PartialEq)]
pub struct CoverageTransRange {
    /// The node that locates the range list in diagnostics.
    pub node: NodeId,
    /// Range-list items (values or value ranges).
    pub items: Vec<NodeId>,
    pub repeat: CoverageRepeat,
    /// The repeat bounds; `to` is absent for a single count.
    pub repeat_from: Option<NodeId>,
    pub repeat_to: Option<NodeId>,
}

/// A cross bin select expression (19.6).
#[derive(Debug, Clone, PartialEq)]
pub struct CoverageSelect {
    /// The node that locates the expression in diagnostics.
    pub node: NodeId,
    pub kind: CoverageSelectKind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CoverageSelectKind {
    /// `binsof(target) [intersect { items }]`: `target` is a coverpoint or
    /// a bin of one.
    Condition {
        target: NodeId,
        intersect: Vec<NodeId>,
    },
    Not(Box<CoverageSelect>),
    And(Box<CoverageSelect>, Box<CoverageSelect>),
    Or(Box<CoverageSelect>, Box<CoverageSelect>),
    /// A later-edition or invalid form (set expression, `with` filter,
    /// cross identifier).
    Other,
}

/// One cross declaration (19.6).
#[derive(Debug, Clone, PartialEq)]
pub struct CoverageCrossInfo {
    /// The covergroup type that declares the cross.
    pub group: NodeId,
    /// The crossed coverpoints, in source order (implicit coverpoints of
    /// crossed variables included).
    pub points: Vec<NodeId>,
    pub iff: Option<NodeId>,
    /// Cross-level option setters, in source order.
    pub options: Vec<NodeId>,
    /// Bins declarations, in source order.
    pub bins: Vec<NodeId>,
    /// A cross item named another cross (not 1800-2009 syntax).
    pub crosses_cross: bool,
}

impl CoverageBinInfo {
    /// `true` for a `bins name = { values }` declaration with no array,
    /// wildcard, default, transition, `with`, set or select shape.
    pub fn is_simple_value_bin(&self) -> bool {
        !(self.array
            || self.wildcard
            || self.default
            || self.default_sequence
            || self.transition
            || self.with
            || self.set
            || self.select)
    }
}

/// All coverage declarations of one design.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CoverageTables {
    pub(super) groups: HashMap<NodeId, CovergroupInfo>,
    pub(super) points: HashMap<NodeId, CoverpointInfo>,
    pub(super) bins: HashMap<NodeId, CoverageBinInfo>,
    pub(super) crosses: HashMap<NodeId, CoverageCrossInfo>,
    pub(super) group_types: HashMap<TypeId, NodeId>,
}

/// Raw coverage records collected while importing nodes; owners are linked
/// once every node's parent is known.
#[derive(Default)]
pub(super) struct CoverageImport {
    groups: Vec<(NodeId, CovergroupInfo)>,
    points: Vec<(NodeId, CoverpointInfo)>,
    bins: Vec<(NodeId, CoverageBinInfo)>,
    crosses: Vec<(NodeId, CoverageCrossInfo)>,
}

fn ordered_targets(
    ids: &SemanticIds,
    edges: &[crate::ffi::slang::SemanticEdge],
    role: SemanticEdgeRole,
) -> Result<Vec<NodeId>, DbError> {
    let mut ordered = edges
        .iter()
        .filter(|edge| edge.role == role)
        .collect::<Vec<_>>();
    ordered.sort_by_key(|edge| edge.index);
    ordered
        .into_iter()
        .map(|edge| semantic_id(ids, edge.target_id))
        .collect()
}

impl CoverageImport {
    /// Record one `SemanticKind::Coverage` node.
    pub(super) fn import(
        &mut self,
        snapshot: &SlangSnapshot,
        ids: &SemanticIds,
        id: NodeId,
        semantic: &SemanticNode,
        edges: &[crate::ffi::slang::SemanticEdge],
        ty: &TypeInfo,
    ) -> Result<(), DbError> {
        match semantic.subkind {
            SEMANTIC_COVERAGE_GROUP => {
                let event = edge_target(ids, edges, SemanticEdgeRole::Event)?
                    .map(|timing| coverage_event(snapshot, ids, timing))
                    .transpose()?;
                self.groups.push((
                    id,
                    CovergroupInfo {
                        type_id: semantic.type_id().map(TypeId),
                        formals: ordered_targets(ids, edges, SemanticEdgeRole::Argument)?,
                        sample_formals: ordered_targets(ids, edges, SemanticEdgeRole::Declaration)?,
                        event,
                        options: ordered_targets(ids, edges, SemanticEdgeRole::Operand)?,
                        body: edge_target(ids, edges, SemanticEdgeRole::Body)?,
                        coverpoints: Vec::new(),
                        crosses: Vec::new(),
                        embedded: semantic.auxiliary & COVERGROUP_EMBEDDED != 0,
                        with_sample: semantic.auxiliary & COVERGROUP_WITH_SAMPLE != 0,
                        extends: semantic.auxiliary & COVERGROUP_EXTENDS != 0,
                    },
                ));
            }
            SEMANTIC_COVERAGE_POINT => {
                // A declaration-only capture (the language server's) omits
                // coverage roles; such a coverpoint has no expression edge.
                let Some(expr) = edge_target(ids, edges, SemanticEdgeRole::Initializer)? else {
                    return Ok(());
                };
                self.points.push((
                    id,
                    CoverpointInfo {
                        group: id,
                        expr,
                        iff: edge_target(ids, edges, SemanticEdgeRole::Condition)?,
                        ty: ty.clone(),
                        type_id: semantic.type_id().map(TypeId),
                        options: ordered_targets(ids, edges, SemanticEdgeRole::Operand)?,
                        bins: Vec::new(),
                    },
                ));
            }
            SEMANTIC_COVERAGE_BIN => {
                let flags = semantic.auxiliary;
                let kind = match flags & COVERAGE_BIN_KIND_MASK {
                    0 => CoverageBinKind::Bins,
                    1 => CoverageBinKind::Illegal,
                    2 => CoverageBinKind::Ignore,
                    other => {
                        return Err(DbError::InvalidSnapshot(format!(
                            "unknown coverage bin kind {other}"
                        )))
                    }
                };
                let transition = flags & COVERAGE_BIN_TRANSITION != 0;
                let operands = ordered_targets(ids, edges, SemanticEdgeRole::Operand)?;
                let (values, transitions) = if transition {
                    let sets = operands
                        .iter()
                        .map(|set| transition_set(snapshot, ids, *set))
                        .collect::<Result<Vec<_>, _>>()?;
                    (Vec::new(), sets)
                } else {
                    (operands, Vec::new())
                };
                let select_expr = edge_target(ids, edges, SemanticEdgeRole::Body)?
                    .map(|node| select_expression(snapshot, ids, node))
                    .transpose()?;
                self.bins.push((
                    id,
                    CoverageBinInfo {
                        point: id,
                        kind,
                        array: flags & COVERAGE_BIN_ARRAY != 0,
                        wildcard: flags & COVERAGE_BIN_WILDCARD != 0,
                        default: flags & COVERAGE_BIN_DEFAULT != 0,
                        default_sequence: flags & COVERAGE_BIN_DEFAULT_SEQUENCE != 0,
                        transition,
                        with: flags & COVERAGE_BIN_WITH != 0,
                        set: flags & COVERAGE_BIN_SET != 0,
                        select: flags & COVERAGE_BIN_SELECT != 0,
                        values,
                        transitions,
                        select_expr,
                        count: edge_target(ids, edges, SemanticEdgeRole::Width)?,
                        iff: edge_target(ids, edges, SemanticEdgeRole::Condition)?,
                    },
                ));
            }
            SEMANTIC_COVERAGE_CROSS => self.crosses.push((
                id,
                CoverageCrossInfo {
                    group: id,
                    points: ordered_targets(ids, edges, SemanticEdgeRole::Argument)?,
                    iff: edge_target(ids, edges, SemanticEdgeRole::Condition)?,
                    options: ordered_targets(ids, edges, SemanticEdgeRole::Operand)?,
                    bins: Vec::new(),
                    crosses_cross: semantic.auxiliary & COVER_CROSS_OF_CROSS != 0,
                },
            )),
            // Read through their bin.
            SEMANTIC_COVERAGE_BODY
            | SEMANTIC_COVERAGE_TRANS_SET
            | SEMANTIC_COVERAGE_TRANS_RANGE
            | SEMANTIC_COVERAGE_SELECT => {}
            other => {
                return Err(DbError::InvalidSnapshot(format!(
                    "unknown coverage subkind {other}"
                )))
            }
        }
        Ok(())
    }

    /// Link coverpoints, bins and crosses to their owners through the node
    /// parent chain and build the lookup tables.
    pub(super) fn finish(self, nodes: &[Node]) -> Result<CoverageTables, DbError> {
        let mut tables = CoverageTables::default();
        for (id, group) in self.groups {
            if let Some(type_id) = group.type_id {
                tables.group_types.insert(type_id, id);
            }
            tables.groups.insert(id, group);
        }
        let owner = |start: NodeId, wanted: &dyn Fn(NodeId) -> bool| -> Option<NodeId> {
            let mut current = nodes.get(start.index())?.parent;
            while let Some(id) = current {
                if wanted(id) {
                    return Some(id);
                }
                current = nodes.get(id.index())?.parent;
            }
            None
        };
        let group_ids = tables.groups.keys().copied().collect::<HashSet<_>>();
        for (id, mut point) in self.points {
            let Some(group) = owner(id, &|candidate| group_ids.contains(&candidate)) else {
                return Err(DbError::InvalidSnapshot(format!(
                    "coverpoint {} has no covergroup",
                    id.index()
                )));
            };
            point.group = group;
            if let Some(info) = tables.groups.get_mut(&group) {
                info.coverpoints.push(id);
            }
            tables.points.insert(id, point);
        }
        for (id, mut cross) in self.crosses {
            let Some(group) = owner(id, &|candidate| group_ids.contains(&candidate)) else {
                return Err(DbError::InvalidSnapshot(format!(
                    "cross {} has no covergroup",
                    id.index()
                )));
            };
            if let Some(info) = tables.groups.get_mut(&group) {
                info.crosses.push(id);
            }
            cross.group = group;
            tables.crosses.insert(id, cross);
        }
        for (id, mut bin) in self.bins {
            let Some(point) = owner(id, &|candidate| {
                tables.points.contains_key(&candidate) || tables.crosses.contains_key(&candidate)
            }) else {
                return Err(DbError::InvalidSnapshot(format!(
                    "coverage bin {} has no coverpoint or cross",
                    id.index()
                )));
            };
            bin.point = point;
            if let Some(info) = tables.points.get_mut(&point) {
                info.bins.push(id);
            } else if let Some(info) = tables.crosses.get_mut(&point) {
                info.bins.push(id);
            }
            tables.bins.insert(id, bin);
        }
        // Declaration order is node-id order within one scope.
        for group in tables.groups.values_mut() {
            group.coverpoints.sort_by_key(|id| id.index());
            group.crosses.sort_by_key(|id| id.index());
        }
        for point in tables.points.values_mut() {
            point.bins.sort_by_key(|id| id.index());
        }
        for cross in tables.crosses.values_mut() {
            cross.bins.sort_by_key(|id| id.index());
        }
        Ok(tables)
    }
}

fn coverage_node<'a>(
    snapshot: &'a SlangSnapshot,
    node: NodeId,
    subkind: u32,
) -> Result<(&'a SemanticNode, &'a [crate::ffi::slang::SemanticEdge]), DbError> {
    let semantic = snapshot
        .semantic_nodes
        .get(node.index())
        .filter(|semantic| semantic.kind == SemanticKind::Coverage && semantic.subkind == subkind)
        .ok_or_else(|| {
            DbError::InvalidSnapshot(format!(
                "coverage node {} is not of subkind {subkind}",
                node.index()
            ))
        })?;
    Ok((semantic, semantic_edges(snapshot, semantic)?))
}

/// The range lists of one transition set (19.5.1).
fn transition_set(
    snapshot: &SlangSnapshot,
    ids: &SemanticIds,
    set: NodeId,
) -> Result<Vec<CoverageTransRange>, DbError> {
    let (_, edges) = coverage_node(snapshot, set, SEMANTIC_COVERAGE_TRANS_SET)?;
    ordered_targets(ids, edges, SemanticEdgeRole::Operand)?
        .into_iter()
        .map(|range| {
            let (semantic, edges) = coverage_node(snapshot, range, SEMANTIC_COVERAGE_TRANS_RANGE)?;
            let repeat = match semantic.auxiliary {
                COVER_REPEAT_NONE => CoverageRepeat::None,
                COVER_REPEAT_CONSECUTIVE => CoverageRepeat::Consecutive,
                COVER_REPEAT_NONCONSECUTIVE => CoverageRepeat::Nonconsecutive,
                COVER_REPEAT_GOTO => CoverageRepeat::GoTo,
                other => {
                    return Err(DbError::InvalidSnapshot(format!(
                        "unknown transition repetition {other}"
                    )))
                }
            };
            Ok(CoverageTransRange {
                node: range,
                items: ordered_targets(ids, edges, SemanticEdgeRole::Operand)?,
                repeat,
                repeat_from: edge_target(ids, edges, SemanticEdgeRole::Left)?,
                repeat_to: edge_target(ids, edges, SemanticEdgeRole::Right)?,
            })
        })
        .collect()
}

/// One cross bin select expression (19.6).
fn select_expression(
    snapshot: &SlangSnapshot,
    ids: &SemanticIds,
    node: NodeId,
) -> Result<CoverageSelect, DbError> {
    let (semantic, edges) = coverage_node(snapshot, node, SEMANTIC_COVERAGE_SELECT)?;
    let operand = |index: u32| -> Result<Box<CoverageSelect>, DbError> {
        let child = edge_target_at(ids, edges, SemanticEdgeRole::Operand, index)?
            .ok_or_else(|| DbError::InvalidSnapshot("select operand is missing".to_owned()))?;
        Ok(Box::new(select_expression(snapshot, ids, child)?))
    };
    let kind = match semantic.auxiliary {
        COVER_SELECT_CONDITION => CoverageSelectKind::Condition {
            target: edge_target(ids, edges, SemanticEdgeRole::Reference)?.ok_or_else(|| {
                DbError::InvalidSnapshot("binsof select has no target".to_owned())
            })?,
            intersect: ordered_targets(ids, edges, SemanticEdgeRole::Operand)?,
        },
        COVER_SELECT_NOT => CoverageSelectKind::Not(operand(0)?),
        COVER_SELECT_AND => CoverageSelectKind::And(operand(0)?, operand(1)?),
        COVER_SELECT_OR => CoverageSelectKind::Or(operand(0)?, operand(1)?),
        _ => CoverageSelectKind::Other,
    };
    Ok(CoverageSelect { node, kind })
}

fn coverage_event(
    snapshot: &SlangSnapshot,
    ids: &SemanticIds,
    timing: NodeId,
) -> Result<CoverageEvent, DbError> {
    let node = snapshot
        .semantic_nodes
        .get(timing.index())
        .ok_or_else(|| DbError::InvalidSnapshot("coverage event is missing".into()))?;
    if node.kind == SemanticKind::TimingControl && node.subkind == SEMANTIC_TIMING_BLOCK_EVENT_LIST
    {
        let edges = semantic_edges(snapshot, node)?;
        let targets = ordered_targets(ids, edges, SemanticEdgeRole::Reference)?;
        let events = targets
            .into_iter()
            .enumerate()
            .map(|(index, target)| CoverageBlockEvent {
                begin: index < 64 && node.auxiliary & (1u64 << index) != 0,
                target,
            })
            .collect();
        return Ok(CoverageEvent::Block { timing, events });
    }
    let (specs, implicit) = event_specs(snapshot, node, ids)?;
    Ok(CoverageEvent::Clocking {
        timing,
        specs,
        implicit,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn compile(source: &str) -> Db {
        let output = crate::core::compile::compile_sources_checked(
            &[crate::core::compile::OwnedSource::compilation_unit(
                "coverage.sv",
                source,
            )],
            &crate::core::compile::CompileOpts {
                top: Some("top".into()),
                ..Default::default()
            },
        )
        .unwrap();
        Db::from_slang(&output.snapshot).unwrap()
    }

    fn group<'a>(db: &'a Db, name: &str) -> (NodeId, &'a CovergroupInfo) {
        db.covergroups()
            .iter()
            .find(|(id, _)| db.node(**id).name == name)
            .map(|(id, info)| (*id, info))
            .unwrap_or_else(|| {
                panic!(
                    "covergroup `{name}` in {:?}",
                    db.covergroups()
                        .keys()
                        .map(|id| db.node(*id).name.clone())
                        .collect::<Vec<_>>()
                )
            })
    }

    #[test]
    fn covergroup_structure_is_typed() {
        let db = compile(
            "module top; logic clk; logic [3:0] a, b; int lim; \
             covergroup cg(int w, ref logic [3:0] r) @(posedge clk); \
               option.per_instance = 1; type_option.strobe = 0; \
               ca: coverpoint a iff (w > 0) { option.weight = 2; \
                 bins lo = {[0:3]}; bins one = {5, 7}; illegal_bins bad = {15}; \
                 bins arr[] = {[8:9]}; bins d = default; } \
               coverpoint r; \
               x: cross ca, r; \
             endgroup \
             covergroup sg with function sample(logic [3:0] v); coverpoint v; endgroup \
             task t; endtask \
             covergroup bg @@(begin t or end t); coverpoint b; endgroup \
             class C; logic [3:0] f; covergroup ec; coverpoint f; endgroup \
               function new; ec = new; endfunction endclass \
             cg c1 = new(3, b); sg s1 = new; bg b1 = new; C cobj = new; \
             endmodule",
        );
        let (cg, info) = group(&db, "cg");
        assert_eq!(info.formals.len(), 2);
        assert_eq!(db.node(info.formals[0]).name, "w");
        assert_eq!(db.node(info.formals[1]).name, "r");
        assert!(info.sample_formals.is_empty());
        assert!(!info.embedded && !info.with_sample && !info.extends);
        assert_eq!(info.options.len(), 2);
        assert!(matches!(
            info.event,
            Some(CoverageEvent::Clocking { ref specs, .. })
                if matches!(specs.as_slice(), [EventSpec::Edge { posedge: true, .. }])
        ));
        assert_eq!(info.coverpoints.len(), 2);
        assert_eq!(info.crosses.len(), 1);
        assert_eq!(db.coverage_cross_group(info.crosses[0]), Some(cg));
        assert_eq!(db.covergroup_for_type(info.type_id.unwrap()), Some(cg));

        let ca = db.coverpoint(info.coverpoints[0]).unwrap();
        assert_eq!(db.node(info.coverpoints[0]).name, "ca");
        assert_eq!(ca.group, cg);
        assert!(ca.iff.is_some());
        assert_eq!(ca.options.len(), 1);
        for option in ca.options.iter().chain(&info.options) {
            assert!(
                matches!(
                    db.node_kind(*option),
                    NodeKind::Expr(ExprKind::Operation {
                        op: Operation::Assignment,
                        ..
                    })
                ),
                "{:?}",
                db.node_kind(*option)
            );
        }
        assert_eq!(ca.ty.width, Some(4));
        let bins = ca
            .bins
            .iter()
            .map(|id| (db.node(*id).name.clone(), db.coverage_bin(*id).unwrap()))
            .collect::<Vec<_>>();
        assert_eq!(bins.len(), 5);
        assert_eq!(bins[0].0, "lo");
        assert!(bins[0].1.is_simple_value_bin());
        assert_eq!(bins[0].1.values.len(), 1);
        assert_eq!(bins[1].1.values.len(), 2);
        assert_eq!(bins[2].1.kind, CoverageBinKind::Illegal);
        assert!(bins[3].1.array && !bins[3].1.is_simple_value_bin());
        assert!(bins[4].1.default);

        let (_, sg) = group(&db, "sg");
        assert!(sg.with_sample);
        assert_eq!(sg.sample_formals.len(), 1);
        assert!(sg.event.is_none());

        let (_, bg) = group(&db, "bg");
        let Some(CoverageEvent::Block { events, .. }) = &bg.event else {
            panic!("block event expected");
        };
        assert_eq!(events.len(), 2);
        assert!(events[0].begin && !events[1].begin);
        assert_eq!(db.node(events[0].target).name, "t");

        // An embedded group's type is anonymous; the class property of the
        // same name holds its instance (19.4).
        let property = db
            .node_ids()
            .find(|id| db.node(*id).name == "ec" && db.type_descriptor(*id).is_some())
            .unwrap();
        let embedded = db
            .covergroup_for_type(db.type_descriptor(property).unwrap().id)
            .unwrap();
        let ec = db.covergroup(embedded).unwrap();
        assert!(ec.embedded);
        assert_eq!(ec.coverpoints.len(), 1);

        let news = db
            .node_ids()
            .filter_map(|id| match db.node_kind(id) {
                NodeKind::Expr(ExprKind::NewCovergroup { group_type, args }) => {
                    Some((*group_type, args.len()))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(news.contains(&(info.type_id, 2)));
        assert_eq!(news.len(), 4);
    }

    #[test]
    fn transitions_crosses_and_selects_are_typed() {
        let db = compile(
            "module top; bit [3:0] a, b, v; \
             covergroup cg; \
               ca: coverpoint a { bins t = (1, 2 => [3:4] => 5 [* 2:3]), (6 => 7 [-> 2]); \
                 bins u[] = (8 => 9 [= 1:2] => 10); bins lo = {[0:3]}; bins hi = {[4:15]}; } \
               cb: coverpoint b; \
               x: cross ca, cb, v iff (a != 0) { option.weight = 3; \
                 bins s = binsof(ca.lo) && !binsof(cb) intersect {[1:2], 9}; \
                 ignore_bins i = binsof(v) || binsof(ca) intersect {7}; } \
             endgroup \
             cg c = new; \
             endmodule",
        );
        let (cg, info) = group(&db, "cg");
        let ca = db.coverpoint(info.coverpoints[0]).unwrap();
        let bin = |id: NodeId| db.coverage_bin(id).unwrap();
        let t = bin(ca.bins[0]);
        assert!(t.transition && t.values.is_empty());
        assert_eq!(t.transitions.len(), 2);
        let first = &t.transitions[0];
        assert_eq!(first.len(), 3);
        assert_eq!(first[0].items.len(), 2);
        assert_eq!(first[0].repeat, CoverageRepeat::None);
        assert!(first[0].repeat_from.is_none());
        assert_eq!(first[2].repeat, CoverageRepeat::Consecutive);
        assert!(first[2].repeat_from.is_some() && first[2].repeat_to.is_some());
        assert_eq!(t.transitions[1][1].repeat, CoverageRepeat::GoTo);
        assert!(t.transitions[1][1].repeat_to.is_none());
        let u = bin(ca.bins[1]);
        assert!(u.array && u.transition);
        assert_eq!(u.transitions[0][1].repeat, CoverageRepeat::Nonconsecutive);

        // `v` is crossed as a variable: the frontend creates an implicit
        // coverpoint for it (19.6).
        assert_eq!(info.crosses.len(), 1);
        let cross = db.coverage_cross(info.crosses[0]).unwrap();
        assert_eq!(cross.group, cg);
        assert_eq!(cross.points.len(), 3);
        assert_eq!(cross.points[0], info.coverpoints[0]);
        assert_eq!(db.node(cross.points[2]).name, "v");
        assert!(db.coverpoint(cross.points[2]).is_some());
        assert!(info.coverpoints.contains(&cross.points[2]));
        assert!(cross.iff.is_some() && !cross.crosses_cross);
        assert_eq!(cross.options.len(), 1);
        assert_eq!(cross.bins.len(), 2);
        let s = bin(cross.bins[0]);
        assert_eq!(s.point, info.crosses[0]);
        let Some(CoverageSelect {
            kind: CoverageSelectKind::And(left, right),
            ..
        }) = &s.select_expr
        else {
            panic!("and expected: {:?}", s.select_expr);
        };
        assert!(matches!(
            &left.kind,
            CoverageSelectKind::Condition { target, intersect } if *target == ca.bins[2] && intersect.is_empty()
        ));
        let CoverageSelectKind::Not(inner) = &right.kind else {
            panic!("not expected: {right:?}");
        };
        assert!(matches!(
            &inner.kind,
            CoverageSelectKind::Condition { target, intersect }
                if *target == info.coverpoints[1] && intersect.len() == 2
        ));
        let i = bin(cross.bins[1]);
        assert_eq!(i.kind, CoverageBinKind::Ignore);
        assert!(matches!(
            &i.select_expr,
            Some(CoverageSelect {
                kind: CoverageSelectKind::Or(..),
                ..
            })
        ));
    }
}
