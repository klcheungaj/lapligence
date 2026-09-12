//! Frontend-independent semantic simulation model.
//!
//! [`SemanticModel`] validates and classifies the owned elaborated database
//! before executable lowering starts. It answers what declarations and
//! operations mean; runtime scheduling, coroutine state, and C spelling are
//! absent from this layer.

use crate::core::db::{
    AlwaysKind, ArrayKind, CapturedSemanticKind, CaseKind, ConstantType, Db, Direction, EventSpec,
    ExprKind, NetType, NodeId, NodeKind, Operation, PrimClass, PrimitiveType, ProcessKind,
    StmtKind, Strength,
};
use crate::core::model::TypeInfo;

/// Revision-local identity of a semantic source origin.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct OriginId(u32);

impl OriginId {
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

/// Physical source identity, or a truthful synthetic identity when the
/// frontend did not provide a source location.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Origin {
    Source {
        path: String,
        line: u32,
        column: u32,
        end_line: u32,
        end_column: u32,
    },
    Synthetic {
        reason: String,
    },
}

/// Opaque metadata owned by a future consumer. The simulator preserves the
/// namespace and key without defining tracing, coverage, or formal behavior.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExtensionRef {
    pub namespace: String,
    pub key: u64,
}

/// Semantic view over one validated, owned frontend snapshot.
#[derive(Debug)]
pub struct SemanticModel<'db> {
    db: &'db Db,
    origins: Vec<Origin>,
}

/// Coverage category for one node considered by simulation lowering.
///
/// The category is deliberately independent of the synthesis profile. A
/// declaration may be reachable through an elaborated instance while still
/// having no runtime obligation, whereas an unknown statement or expression
/// must never be silently treated as an empty node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SimulationNodeClass {
    Executable,
    DeclarationOnly,
    ElaborationConsumed,
    IntentionallyUnreachable,
    Unsupported,
}

/// One entry in the simulator's executable-node conformance ledger.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SimulationCoverageEntry {
    pub node: NodeId,
    pub class: SimulationNodeClass,
    pub origin: OriginId,
    pub detail: String,
}

/// A reachable node that has no simulator lowering contract.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SimulationIssue {
    pub node: NodeId,
    pub origin: OriginId,
    pub detail: String,
}

impl SimulationIssue {
    /// Render a source-located diagnostic while retaining truthful synthetic
    /// origins for elaborated objects without a physical source span.
    pub fn diagnostic(&self, model: &SemanticModel<'_>) -> String {
        let location = match model.origin(self.origin) {
            Some(Origin::Source {
                path, line, column, ..
            }) => format!("{path}:{line}:{column}"),
            Some(Origin::Synthetic { reason }) => format!("<synthetic: {reason}>"),
            None => "<unknown source>".to_owned(),
        };
        let path = model.db.node(self.node).full_name();
        if path.is_empty() {
            format!(
                "unsupported executable node `{}` at {location}",
                self.detail
            )
        } else {
            format!(
                "unsupported executable node `{}` at {location} in `{path}`",
                self.detail
            )
        }
    }
}

impl<'db> SemanticModel<'db> {
    pub fn from_db(db: &'db Db) -> Self {
        let origins = db
            .nodes()
            .iter()
            .map(|node| match node.file() {
                Some(path) => Origin::Source {
                    path: path.to_owned(),
                    line: node.line(),
                    column: node.column(),
                    end_line: node.end_line(),
                    end_column: node.end_column(),
                },
                None => Origin::Synthetic {
                    reason: format!("elaborated {}", node.full_name()),
                },
            })
            .collect();
        Self { db, origins }
    }

    pub fn design_name(&self) -> &str {
        self.db.design_name()
    }

    pub fn origins(&self) -> &[Origin] {
        &self.origins
    }

    pub fn origin_of(&self, node: NodeId) -> Option<OriginId> {
        (node.index() < self.origins.len()).then_some(OriginId(node.index() as u32))
    }

    pub fn origin(&self, id: OriginId) -> Option<&Origin> {
        self.origins.get(id.index())
    }

    pub(crate) fn db(&self) -> &'db Db {
        self.db
    }

    /// Classify every captured node at the simulation lowering boundary.
    ///
    /// Reachability follows the same owned child/reference graph consumed by
    /// lowering, including initializer and embedded semantic references. Nodes
    /// omitted by elaboration (for example an inactive generate branch) remain
    /// visible in the ledger as intentionally unreachable rather than becoming
    /// false unsupported diagnostics.
    pub fn simulation_coverage(&self) -> Vec<SimulationCoverageEntry> {
        let reachable = self.simulation_reachability();
        let elaboration_placeholders = self.simulation_elaboration_placeholders();
        self.db
            .node_ids()
            .map(|node| {
                let class = if reachable[node.index()] {
                    classify_simulation_node(self.db, node, elaboration_placeholders[node.index()])
                } else {
                    SimulationNodeClass::IntentionallyUnreachable
                };
                SimulationCoverageEntry {
                    node,
                    class,
                    origin: OriginId(node.index() as u32),
                    detail: simulation_node_detail(self.db, node),
                }
            })
            .collect()
    }

    /// Reject every reachable executable node that has no simulation
    /// lowering contract before IR construction starts.
    pub fn validate_simulation(&self) -> Result<(), Vec<SimulationIssue>> {
        let issues = self
            .simulation_coverage()
            .into_iter()
            .filter(|entry| entry.class == SimulationNodeClass::Unsupported)
            .map(|entry| SimulationIssue {
                node: entry.node,
                origin: entry.origin,
                detail: entry.detail,
            })
            .collect::<Vec<_>>();
        if issues.is_empty() {
            Ok(())
        } else {
            Err(issues)
        }
    }

    fn simulation_elaboration_placeholders(&self) -> Vec<bool> {
        let mut placeholders = vec![false; self.db.nodes().len()];
        for owner in self.db.node_ids() {
            if !matches!(self.db.node_kind(owner), NodeKind::GenScope) {
                continue;
            }
            for child in self.db.node(owner).children() {
                if !matches!(self.db.node_kind(*child), NodeKind::Other)
                    || self.db.semantic_kind(*child) != Some(CapturedSemanticKind::Unsupported)
                    || self.db.semantic_detail(*child) != Some("")
                {
                    continue;
                }
                let node = self.db.node(*child);
                if node.parent().is_none()
                    && node.name().is_empty()
                    && node.full_name().is_empty()
                    && node.file().is_none()
                {
                    placeholders[child.index()] = true;
                }
            }
        }
        for id in self.db.node_ids() {
            if assignment_pattern_metadata_node(self.db, id) {
                placeholders[id.index()] = true;
            }
        }
        // A captured ArbitrarySymbol is not generally executable. Admit only
        // typed interface actuals and $dumpvars scope/storage arguments, and
        // require every use of a shared expression node to be a metadata use.
        let mut metadata_use = vec![false; self.db.nodes().len()];
        let mut value_use = vec![false; self.db.nodes().len()];
        for owner in self.db.node_ids() {
            let node = self.db.node(owner);
            let mut references = node.children().to_vec();
            node.kind().append_references(&mut references);
            for reference in references {
                let NodeKind::Expr(ExprKind::ScopeRef { target }) = self.db.node_kind(reference)
                else {
                    continue;
                };
                if scope_reference_is_metadata(self.db, owner, reference, *target) {
                    metadata_use[reference.index()] = true;
                } else {
                    value_use[reference.index()] = true;
                }
            }
        }
        for index in 0..placeholders.len() {
            placeholders[index] |= metadata_use[index] && !value_use[index];
        }
        placeholders
    }

    fn simulation_reachability(&self) -> Vec<bool> {
        let mut reachable = vec![false; self.db.nodes().len()];
        let mut pending = self.db.tops().to_vec();
        while let Some(id) = pending.pop() {
            if reachable[id.index()] {
                continue;
            }
            reachable[id.index()] = true;
            let node = self.db.node(id);
            let declaration_only_other = matches!(node.kind(), NodeKind::Other)
                && classify_simulation_node(self.db, id, false)
                    == SimulationNodeClass::DeclarationOnly;
            if !matches!(node.kind(), NodeKind::ClassDef | NodeKind::Package)
                && !declaration_only_other
            {
                pending.extend_from_slice(node.children());
            }
            // A metadata reference does not make a scope's contents executable.
            // The target itself has already passed database bounds validation.
            if !matches!(node.kind(), NodeKind::Expr(ExprKind::ScopeRef { .. })) {
                node.kind().append_references(&mut pending);
            }
            pending.extend(self.db.var_initializer(id));
            pending.extend(
                self.db
                    .array_meta(id)
                    .and_then(|metadata| metadata.initializer()),
            );
        }
        reachable
    }

    /// Validate every captured construct against an explicit synthesis
    /// profile. The returned view is proof that no reachable construct was
    /// silently discarded by classification.
    pub fn validate_synthesizable(
        &self,
        profile: SynthesisProfile,
    ) -> Result<SynthDesignView<'_, 'db>, Vec<SynthesisIssue>> {
        let mut issues = Vec::new();
        let mut reachable = vec![false; self.db.nodes().len()];
        let mut pending = self.db.tops().to_vec();
        while let Some(id) = pending.pop() {
            if reachable[id.index()] {
                continue;
            }
            reachable[id.index()] = true;
            let node = self.db.node(id);
            pending.extend_from_slice(node.children());
            // A metadata reference does not make a scope's contents executable.
            // The target itself has already passed database bounds validation.
            if !matches!(node.kind(), NodeKind::Expr(ExprKind::ScopeRef { .. })) {
                node.kind().append_references(&mut pending);
            }
            pending.extend(self.db.var_initializer(id));
            pending.extend(
                self.db
                    .array_meta(id)
                    .and_then(|metadata| metadata.initializer()),
            );
        }
        for id in self.db.node_ids().filter(|id| reachable[id.index()]) {
            let node = self.db.node(id);
            let kind = match node.kind() {
                NodeKind::ClassDef => Some(SynthesisIssueKind::RuntimeObject),
                NodeKind::NamedEvent => Some(SynthesisIssueKind::EventOperation),
                NodeKind::Array { ty } => match self.db.array_meta(id) {
                    Some(meta) if matches!(meta.kind(), ArrayKind::Static) => classify_type(ty)
                        .or_else(|| {
                            (meta.dimensions().is_empty()
                                || meta.dimensions().iter().any(Option::is_none))
                            .then_some(SynthesisIssueKind::UnknownType)
                        })
                        .or_else(|| {
                            meta.initializer()
                                .map(|_| SynthesisIssueKind::StorageInitialization)
                        }),
                    Some(_) => Some(SynthesisIssueKind::DynamicContainer),
                    None => Some(SynthesisIssueKind::UnknownType),
                },
                NodeKind::Net { ty, net_type } => classify_type(ty).or_else(|| {
                    (!matches!(
                        net_type,
                        NetType::Wire | NetType::Uwire | NetType::Logic | NetType::Reg
                    ))
                    .then_some(SynthesisIssueKind::ResolvedNet)
                }),
                NodeKind::Var { ty } => classify_type(ty).or_else(|| {
                    self.db
                        .var_initializer(id)
                        .map(|_| SynthesisIssueKind::StorageInitialization)
                }),
                NodeKind::Param {
                    ty, value: None, ..
                } => classify_type(ty).or(Some(SynthesisIssueKind::UnresolvedExpression)),
                NodeKind::Param {
                    ty, value: Some(_), ..
                } => classify_type(ty),
                NodeKind::FuncTask { ret: Some(ty), .. } => classify_type(ty),
                NodeKind::Process {
                    kind: ProcessKind::Initial | ProcessKind::Final,
                } => Some(SynthesisIssueKind::SimulationProcess),
                NodeKind::Process {
                    kind:
                        ProcessKind::Always {
                            always_type: AlwaysKind::Unsupported,
                        },
                } => Some(SynthesisIssueKind::UnknownConstruct),
                NodeKind::Process {
                    kind:
                        ProcessKind::Always {
                            always_type: AlwaysKind::Always,
                        },
                } if !node.children().iter().any(|child| {
                    matches!(
                        self.db.node_kind(*child),
                        NodeKind::Stmt(StmtKind::EventControl { .. })
                    )
                }) =>
                {
                    Some(SynthesisIssueKind::TimingControl)
                }
                NodeKind::Port { direction, .. } | NodeKind::IoDecl { direction, .. }
                    if !matches!(
                        direction,
                        Direction::Input | Direction::Output | Direction::Inout
                    ) =>
                {
                    Some(SynthesisIssueKind::UnknownConstruct)
                }
                NodeKind::Port { ty, .. } => classify_type(ty),
                NodeKind::Genvar { ty } => classify_type(ty),
                NodeKind::FuncArg {
                    direction: Direction::Mixed | Direction::None | Direction::Unsupported,
                    ..
                } => Some(SynthesisIssueKind::UnknownConstruct),
                NodeKind::FuncArg { ty, .. } => classify_type(ty),
                NodeKind::ContAssign { delay: Some(_), .. } => {
                    Some(SynthesisIssueKind::TimingControl)
                }
                NodeKind::ContAssign {
                    strength0,
                    strength1,
                    ..
                } if !matches!(strength0, Strength::Unspecified)
                    || !matches!(strength1, Strength::Unspecified) =>
                {
                    Some(SynthesisIssueKind::DriveStrength)
                }
                NodeKind::Stmt(statement) => classify_statement(self.db, id, statement),
                NodeKind::Expr(expression) => classify_expression(expression),
                NodeKind::SysCall { name } if is_synthesis_system_call(name) => None,
                NodeKind::SysCall { .. } | NodeKind::MethodCall { .. } => {
                    Some(SynthesisIssueKind::RuntimeService)
                }
                NodeKind::FuncCall { callee: None, .. } | NodeKind::EnumConst { value: None } => {
                    Some(SynthesisIssueKind::UnresolvedExpression)
                }
                NodeKind::FuncCall { .. } => Some(SynthesisIssueKind::UnprovenCall),
                NodeKind::Gate {
                    class: PrimClass::Gate,
                    prim_type,
                    delay: None,
                    strength0: Strength::Unspecified,
                    strength1: Strength::Unspecified,
                    ..
                } if is_synthesis_primitive(*prim_type) => None,
                NodeKind::Gate { .. } => Some(SynthesisIssueKind::UnsupportedPrimitive),
                NodeKind::Other => Some(SynthesisIssueKind::UnknownConstruct),
                _ => None,
            };
            if let Some(kind) = kind {
                issues.push(SynthesisIssue {
                    kind,
                    origin: OriginId(id.index() as u32),
                });
            }
        }
        if issues.is_empty() {
            Ok(SynthDesignView {
                model: self,
                profile,
            })
        } else {
            Err(issues)
        }
    }
}

fn assignment_pattern_metadata_node(db: &Db, id: NodeId) -> bool {
    let node = db.node(id);
    if node.file().is_some()
        || db.semantic_kind(id) != Some(CapturedSemanticKind::Unsupported)
        || db.semantic_detail(id) != Some("")
    {
        return false;
    }
    let mut current = node.parent();
    while let Some(parent) = current {
        if matches!(
            db.node_kind(parent),
            NodeKind::Expr(ExprKind::Operation {
                op: Operation::AssignmentPattern | Operation::MultiAssignmentPattern,
                ..
            })
        ) {
            return true;
        }
        current = db.node(parent).parent();
    }
    false
}

fn scope_reference_is_metadata(db: &Db, owner: NodeId, reference: NodeId, target: NodeId) -> bool {
    let owner_node = db.node(owner);
    match owner_node.kind() {
        NodeKind::SysCall { name } if name == "$dumpvars" => {
            // The first argument is a depth expression, not a selection.
            owner_node.children().first() != Some(&reference)
                && owner_node
                    .children()
                    .iter()
                    .skip(1)
                    .any(|arg| *arg == reference)
                && matches!(
                    db.node_kind(target),
                    NodeKind::ModuleInst { .. }
                        | NodeKind::GenScope
                        | NodeKind::GenScopeArray
                        | NodeKind::Stmt(StmtKind::Begin)
                        | NodeKind::Port { .. }
                        | NodeKind::IoDecl { .. }
                        | NodeKind::Net { .. }
                        | NodeKind::Var { .. }
                        | NodeKind::Array { .. }
                )
        }
        NodeKind::Port {
            high_expr: Some(actual_expr),
            ..
        } if *actual_expr == reference => owner_node.children().iter().any(|child| {
            let NodeKind::IfaceConn { actual, .. } = db.node_kind(*child) else {
                return false;
            };
            matches!(
                db.node_kind(*actual),
                NodeKind::ModuleInst {
                    is_interface: true,
                    ..
                }
            ) && (target == *actual
                || (matches!(db.node_kind(target), NodeKind::ModPort)
                    && db.node(target).parent() == Some(*actual)))
        }),
        _ => false,
    }
}

fn classify_simulation_node(
    db: &Db,
    id: NodeId,
    elaboration_placeholder: bool,
) -> SimulationNodeClass {
    let node = db.node(id);
    match node.kind() {
        NodeKind::Other => match db.semantic_kind(id) {
            Some(CapturedSemanticKind::TimingControl) => SimulationNodeClass::ElaborationConsumed,
            Some(CapturedSemanticKind::Unsupported) if elaboration_placeholder => {
                SimulationNodeClass::ElaborationConsumed
            }
            Some(CapturedSemanticKind::Primitive)
            | Some(CapturedSemanticKind::Definition)
            | Some(CapturedSemanticKind::Package)
            | Some(CapturedSemanticKind::Class) => SimulationNodeClass::DeclarationOnly,
            Some(CapturedSemanticKind::Unsupported)
                if is_declaration_only_unknown(db.semantic_detail(id)) =>
            {
                SimulationNodeClass::DeclarationOnly
            }
            _ => SimulationNodeClass::Unsupported,
        },
        NodeKind::Stmt(StmtKind::Begin)
            if db.semantic_kind(id) == Some(CapturedSemanticKind::Scope) =>
        {
            SimulationNodeClass::ElaborationConsumed
        }
        NodeKind::Stmt(StmtKind::Unsupported { .. }) => SimulationNodeClass::Unsupported,
        NodeKind::Expr(ExprKind::ScopeRef { .. }) => {
            if elaboration_placeholder {
                SimulationNodeClass::ElaborationConsumed
            } else {
                SimulationNodeClass::Unsupported
            }
        }
        NodeKind::Expr(ExprKind::Other) => {
            if db.semantic_detail(id) == Some("EmptyArgument") {
                SimulationNodeClass::ElaborationConsumed
            } else {
                SimulationNodeClass::Unsupported
            }
        }
        NodeKind::Gate {
            class,
            prim_type,
            strength0,
            strength1,
            ..
        } if !is_simulation_gate(*class, *prim_type, *strength0, *strength1) => {
            SimulationNodeClass::Unsupported
        }
        NodeKind::Package | NodeKind::ClassDef => SimulationNodeClass::DeclarationOnly,
        NodeKind::Param { .. }
        | NodeKind::ParamAssign { .. }
        | NodeKind::Genvar { .. }
        | NodeKind::InstanceArray
        | NodeKind::GenScopeArray
        | NodeKind::GenScope
        | NodeKind::ModPort
        | NodeKind::IfaceConn { .. }
        | NodeKind::EnumConst { .. } => SimulationNodeClass::ElaborationConsumed,
        NodeKind::ModuleInst { .. }
        | NodeKind::Port { .. }
        | NodeKind::IoDecl { .. }
        | NodeKind::Net { .. }
        | NodeKind::Var { .. }
        | NodeKind::Array { .. }
        | NodeKind::NamedEvent
        | NodeKind::FuncArg { .. } => SimulationNodeClass::DeclarationOnly,
        NodeKind::Process { .. }
        | NodeKind::ContAssign { .. }
        | NodeKind::Gate { .. }
        | NodeKind::Stmt(_)
        | NodeKind::Expr(_)
        | NodeKind::SysCall { .. }
        | NodeKind::MethodCall { .. }
        | NodeKind::FuncCall { .. }
        | NodeKind::FuncTask { .. } => SimulationNodeClass::Executable,
    }
}

fn simulation_node_detail(db: &Db, id: NodeId) -> String {
    if let NodeKind::Gate {
        class,
        prim_type,
        strength0,
        strength1,
        ..
    } = db.node_kind(id)
    {
        if !is_simulation_gate(*class, *prim_type, *strength0, *strength1) {
            return match class {
                PrimClass::Switch => "switch/transistor primitive is not supported".to_owned(),
                PrimClass::Udp => "user-defined primitive instance is not supported".to_owned(),
                PrimClass::Array => "primitive array is not supported".to_owned(),
                PrimClass::Gate
                    if *strength0 != Strength::Unspecified
                        || *strength1 != Strength::Unspecified =>
                {
                    "drive-strength specification".to_owned()
                }
                PrimClass::Gate => format!("primitive type {prim_type:?}"),
            };
        }
    }
    db.semantic_detail(id)
        .filter(|detail| !detail.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| match db.node_kind(id) {
            NodeKind::Stmt(StmtKind::Unsupported { object_type }) => {
                format!("statement {object_type:?}")
            }
            NodeKind::Expr(ExprKind::Other) => "expression".to_owned(),
            NodeKind::Other => "owned node kind Other".to_owned(),
            kind => format!("{kind:?}"),
        })
}

fn is_declaration_only_unknown(detail: Option<&str>) -> bool {
    matches!(
        detail,
        Some(
            "NetType"
                | "CompilationUnit"
                | "Checker"
                | "Covergroup"
                | "CovergroupType"
                | "Property"
                | "Sequence"
                | "ClockingBlock"
                | "ClockingBlockPort"
                | "Constraint"
                | "ExplicitImport"
                | "Export"
                | "Import"
                | "LetDeclaration"
                | "TypeAlias"
                | "TransparentMember"
                | "WildcardImport"
        )
    )
}

fn is_simulation_gate(
    class: PrimClass,
    prim_type: PrimitiveType,
    strength0: Strength,
    strength1: Strength,
) -> bool {
    class == PrimClass::Gate
        && supported_gate_strength(strength0)
        && supported_gate_strength(strength1)
        && matches!(
            prim_type,
            PrimitiveType::And
                | PrimitiveType::Nand
                | PrimitiveType::Nor
                | PrimitiveType::Or
                | PrimitiveType::Xor
                | PrimitiveType::Xnor
                | PrimitiveType::Buf
                | PrimitiveType::Not
                | PrimitiveType::Bufif0
                | PrimitiveType::Bufif1
                | PrimitiveType::Notif0
                | PrimitiveType::Notif1
                | PrimitiveType::Pullup
                | PrimitiveType::Pulldown
        )
}

fn supported_gate_strength(strength: Strength) -> bool {
    matches!(
        strength,
        Strength::Unspecified
            | Strength::Supply
            | Strength::Strong
            | Strength::Pull
            | Strength::Weak
            | Strength::HighZ
    )
}

fn classify_type(ty: &TypeInfo) -> Option<SynthesisIssueKind> {
    if matches!(
        ty.kind.as_str(),
        "real" | "shortreal" | "string" | "chandle" | "class"
    ) {
        Some(SynthesisIssueKind::RuntimeType)
    } else if !ty.width.is_some_and(|width| width > 0)
        || !matches!(
            ty.kind.as_str(),
            "bit"
                | "logic"
                | "reg"
                | "byte"
                | "shortint"
                | "int"
                | "integer"
                | "longint"
                | "time"
                | "enum"
                | "struct"
                | "union"
        )
    {
        Some(SynthesisIssueKind::UnknownType)
    } else {
        None
    }
}

fn classify_statement(db: &Db, id: NodeId, statement: &StmtKind) -> Option<SynthesisIssueKind> {
    match statement {
        StmtKind::Assign { delay: Some(_), .. } => Some(SynthesisIssueKind::TimingControl),
        StmtKind::Assign { op, .. }
            if !matches!(op, Operation::Assignment) && !is_synthesis_operation(*op) =>
        {
            Some(SynthesisIssueKind::UnsupportedExpression)
        }
        StmtKind::Case {
            case_type: CaseKind::Unsupported,
            ..
        } => Some(SynthesisIssueKind::UnknownConstruct),
        StmtKind::EventControl {
            specs,
            implicit,
            body: Some(_),
        } if db
            .node(id)
            .parent()
            .is_some_and(|parent| matches!(db.node_kind(parent), NodeKind::Process { .. }))
            && synthesis_event_control(db, specs, *implicit) =>
        {
            None
        }
        StmtKind::EventControl { .. } | StmtKind::DelayControl { .. } | StmtKind::Wait { .. } => {
            Some(SynthesisIssueKind::TimingControl)
        }
        StmtKind::EventTrigger { .. } => Some(SynthesisIssueKind::EventOperation),
        StmtKind::Force { .. }
        | StmtKind::Release { .. }
        | StmtKind::Deassign { .. }
        | StmtKind::ProcContAssign { .. } => Some(SynthesisIssueKind::ForceOrProceduralDriver),
        StmtKind::Fork { .. }
        | StmtKind::WaitFork
        | StmtKind::DisableFork
        | StmtKind::Disable { .. } => Some(SynthesisIssueKind::DynamicProcess),
        StmtKind::For { .. }
        | StmtKind::While { .. }
        | StmtKind::DoWhile { .. }
        | StmtKind::Repeat { .. }
        | StmtKind::Forever { .. }
        | StmtKind::Foreach { .. } => Some(SynthesisIssueKind::UnprovenLoop),
        StmtKind::Unsupported { .. } => Some(SynthesisIssueKind::UnknownConstruct),
        _ => None,
    }
}

fn synthesis_event_control(db: &Db, specs: &[EventSpec], implicit: bool) -> bool {
    if implicit {
        return specs.is_empty();
    }
    let simple_signal = |id: NodeId, scalar: bool| {
        let target = match db.node_kind(id) {
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) => *target,
            NodeKind::Net { .. } | NodeKind::Var { .. } => id,
            _ => return false,
        };
        match db.node_kind(target) {
            NodeKind::Net { ty, .. } | NodeKind::Var { ty } => {
                classify_type(ty).is_none() && (!scalar || ty.width == Some(1))
            }
            _ => false,
        }
    };
    match specs {
        [EventSpec::Edge { sig, .. }] => simple_signal(*sig, true),
        [] => false,
        _ => specs.iter().all(|spec| {
            matches!(
                spec, EventSpec::AnyChange { sig } if simple_signal(*sig, false)
            )
        }),
    }
}

fn classify_expression(expression: &ExprKind) -> Option<SynthesisIssueKind> {
    match expression {
        ExprKind::ScopeRef { .. } => Some(SynthesisIssueKind::UnsupportedExpression),
        ExprKind::NewArray { .. } => Some(SynthesisIssueKind::DynamicContainer),
        ExprKind::Streaming { .. } => Some(SynthesisIssueKind::UnsupportedExpression),
        ExprKind::Constant {
            const_type: ConstantType::Unsupported | ConstantType::Null,
            ..
        }
        | ExprKind::TaggedPattern { value: None, .. } => {
            Some(SynthesisIssueKind::UnresolvedExpression)
        }
        ExprKind::HierPath { parts, refs }
            if parts.len() != refs.len() || refs.iter().any(Option::is_none) =>
        {
            Some(SynthesisIssueKind::UnresolvedExpression)
        }
        ExprKind::Constant {
            const_type: ConstantType::Real | ConstantType::String,
            ..
        } => Some(SynthesisIssueKind::RuntimeType),
        ExprKind::Other | ExprKind::Ref { target: None } => {
            Some(SynthesisIssueKind::UnresolvedExpression)
        }
        ExprKind::Cast {
            cast_kind_known, ..
        } if !*cast_kind_known => Some(SynthesisIssueKind::UnresolvedExpression),
        ExprKind::Cast { ty, .. } => classify_type(ty),
        ExprKind::Operation { op, operands, .. }
            if !operation_arity_is_valid(*op, operands.len()) =>
        {
            Some(SynthesisIssueKind::UnresolvedExpression)
        }
        ExprKind::Operation { op, .. } if !is_synthesis_operation(*op) => {
            Some(SynthesisIssueKind::UnsupportedExpression)
        }
        _ => None,
    }
}

pub(crate) fn operation_arity_requirement(
    operation: Operation,
) -> Option<(usize, Option<usize>, &'static str)> {
    Some(match operation {
        Operation::UnaryMinus
        | Operation::UnaryPlus
        | Operation::LogicalNot
        | Operation::BitwiseNot
        | Operation::ReductionAnd
        | Operation::ReductionNand
        | Operation::ReductionOr
        | Operation::ReductionNor
        | Operation::ReductionXor
        | Operation::ReductionXnor => (1, Some(1), "exactly one"),
        Operation::Subtract
        | Operation::Divide
        | Operation::Modulo
        | Operation::Equal
        | Operation::NotEqual
        | Operation::CaseEqual
        | Operation::CaseNotEqual
        | Operation::Greater
        | Operation::GreaterEqual
        | Operation::Less
        | Operation::LessEqual
        | Operation::ShiftLeft
        | Operation::ShiftRight
        | Operation::Add
        | Operation::Multiply
        | Operation::LogicalAnd
        | Operation::LogicalOr
        | Operation::Imply
        | Operation::LogicalEquivalence
        | Operation::BitwiseAnd
        | Operation::BitwiseOr
        | Operation::BitwiseXor
        | Operation::BitwiseXnor
        | Operation::ArithmeticShiftLeft
        | Operation::ArithmeticShiftRight
        | Operation::Power
        | Operation::WildEqual
        | Operation::WildNotEqual => (2, Some(2), "exactly two"),
        Operation::Conditional => (3, Some(3), "exactly three"),
        Operation::Concat | Operation::MinTypMax => (1, None, "at least one"),
        Operation::MultiConcat | Operation::Inside => (2, None, "at least two"),
        Operation::StreamLeftToRight | Operation::StreamRightToLeft => (1, Some(2), "one or two"),
        _ => return None,
    })
}

fn operation_arity_is_valid(operation: Operation, actual: usize) -> bool {
    operation_arity_requirement(operation).is_none_or(|(minimum, maximum, _)| {
        actual >= minimum && maximum.is_none_or(|maximum| actual <= maximum)
    })
}

fn is_synthesis_operation(op: Operation) -> bool {
    matches!(
        op,
        Operation::UnaryMinus
            | Operation::UnaryPlus
            | Operation::LogicalNot
            | Operation::BitwiseNot
            | Operation::ReductionAnd
            | Operation::ReductionNand
            | Operation::ReductionOr
            | Operation::ReductionNor
            | Operation::ReductionXor
            | Operation::ReductionXnor
            | Operation::Subtract
            | Operation::Divide
            | Operation::Modulo
            | Operation::Equal
            | Operation::NotEqual
            | Operation::CaseEqual
            | Operation::CaseNotEqual
            | Operation::Greater
            | Operation::GreaterEqual
            | Operation::Less
            | Operation::LessEqual
            | Operation::ShiftLeft
            | Operation::ShiftRight
            | Operation::ArithmeticShiftLeft
            | Operation::ArithmeticShiftRight
            | Operation::Add
            | Operation::Multiply
            | Operation::Power
            | Operation::LogicalAnd
            | Operation::LogicalOr
            | Operation::Imply
            | Operation::LogicalEquivalence
            | Operation::BitwiseAnd
            | Operation::BitwiseOr
            | Operation::BitwiseXor
            | Operation::BitwiseXnor
            | Operation::Conditional
            | Operation::Concat
            | Operation::MultiConcat
            | Operation::AssignmentPattern
            | Operation::MultiAssignmentPattern
            | Operation::MinTypMax
            | Operation::Cast
    )
}

fn is_synthesis_system_call(name: &str) -> bool {
    matches!(name, "$bits" | "$clog2" | "$signed" | "$unsigned")
}

fn is_synthesis_primitive(primitive: PrimitiveType) -> bool {
    matches!(
        primitive,
        PrimitiveType::And
            | PrimitiveType::Nand
            | PrimitiveType::Nor
            | PrimitiveType::Or
            | PrimitiveType::Xor
            | PrimitiveType::Xnor
            | PrimitiveType::Buf
            | PrimitiveType::Not
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SynthesisProfile {
    /// Conservative portable RTL: static packed storage, structural logic,
    /// combinational processes and single-scalar-edge processes. Procedural
    /// loops, subprogram calls, and storage initialization require future proofs
    /// or target-specific profiles and are rejected here.
    PortableRtl,
}

/// Borrowed proof of successful synthesis classification.
#[derive(Clone, Copy, Debug)]
pub struct SynthDesignView<'model, 'db> {
    model: &'model SemanticModel<'db>,
    profile: SynthesisProfile,
}

impl<'db> SynthDesignView<'_, 'db> {
    pub fn model(&self) -> &SemanticModel<'db> {
        self.model
    }

    pub fn profile(&self) -> SynthesisProfile {
        self.profile
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SynthesisIssueKind {
    TimingControl,
    EventOperation,
    DynamicProcess,
    ForceOrProceduralDriver,
    RuntimeService,
    RuntimeType,
    DynamicContainer,
    RuntimeObject,
    SimulationProcess,
    StorageInitialization,
    UnprovenLoop,
    UnprovenCall,
    ResolvedNet,
    DriveStrength,
    UnknownType,
    UnresolvedExpression,
    UnsupportedExpression,
    UnsupportedPrimitive,
    UnknownConstruct,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SynthesisIssue {
    pub kind: SynthesisIssueKind,
    pub origin: OriginId,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::db::AlwaysKind;
    use crate::core::db::Node;
    use std::collections::HashMap;

    fn node(kind: NodeKind, parent: Option<NodeId>, children: Vec<NodeId>) -> Node {
        Node {
            kind,
            children,
            parent,
            name: String::new(),
            full_name: String::new(),
            file: Some("test.sv".into()),
            line: 1,
            col: 1,
            end_line: 1,
            end_col: 2,
        }
    }

    fn top(children: Vec<NodeId>) -> Node {
        node(
            NodeKind::ModuleInst {
                def_name: "top".into(),
                is_top: true,
                is_interface: false,
                timeunit: 0,
                timeprecision: 0,
            },
            None,
            children,
        )
    }

    #[test]
    fn synthesis_classification_fails_closed_for_unresolved_references() {
        assert_eq!(
            classify_expression(&ExprKind::Ref { target: None }),
            Some(SynthesisIssueKind::UnresolvedExpression)
        );
    }

    #[test]
    fn synthesis_classification_accepts_resolved_arithmetic() {
        assert!(is_synthesis_operation(Operation::Add));
        assert!(!is_synthesis_operation(Operation::Inside));
    }

    #[test]
    fn synthesis_type_requires_known_fixed_width_storage() {
        for kind in ["int", "reg", "logic", "enum"] {
            assert_eq!(
                classify_type(&TypeInfo {
                    kind: kind.into(),
                    width: Some(32),
                    signed: false,
                    type_name: None,
                }),
                None
            );
        }
        for (kind, width) in [
            ("future_type", Some(32)),
            ("logic", Some(0)),
            ("logic", None),
        ] {
            assert_eq!(
                classify_type(&TypeInfo {
                    kind: kind.into(),
                    width,
                    signed: false,
                    type_name: None,
                }),
                Some(SynthesisIssueKind::UnknownType)
            );
        }
    }

    #[test]
    fn portable_rtl_view_accepts_empty_structural_top() {
        let db =
            Db::from_test_nodes("top", vec![top(vec![])], vec![NodeId(0)], HashMap::new()).unwrap();
        let model = SemanticModel::from_db(&db);
        assert!(model
            .validate_synthesizable(SynthesisProfile::PortableRtl)
            .is_ok());
    }

    #[test]
    fn simulation_coverage_rejects_reachable_unknown_nodes_with_origin() {
        let db = Db::from_test_nodes(
            "top",
            vec![
                top(vec![NodeId(1)]),
                node(NodeKind::Other, Some(NodeId(0)), vec![]),
            ],
            vec![NodeId(0)],
            HashMap::new(),
        )
        .unwrap();
        let model = SemanticModel::from_db(&db);
        let coverage = model.simulation_coverage();
        assert_eq!(coverage[1].class, SimulationNodeClass::Unsupported);
        let issues = model
            .validate_simulation()
            .expect_err("reachable unknown nodes must fail closed");
        assert_eq!(issues[0].origin.index(), 1);
        assert_eq!(
            issues[0].diagnostic(&model),
            "unsupported executable node `owned node kind Other` at test.sv:1:1"
        );
    }

    #[test]
    fn simulation_coverage_distinguishes_declarations_and_unreachable_nodes() {
        let db = Db::from_test_nodes(
            "top",
            vec![
                top(vec![NodeId(1)]),
                node(
                    NodeKind::Param {
                        ty: TypeInfo {
                            kind: "int".into(),
                            width: Some(32),
                            signed: true,
                            type_name: None,
                        },
                        value: None,
                        local: false,
                    },
                    Some(NodeId(0)),
                    vec![],
                ),
                node(NodeKind::Other, None, vec![]),
            ],
            vec![NodeId(0)],
            HashMap::new(),
        )
        .unwrap();
        let model = SemanticModel::from_db(&db);
        let coverage = model.simulation_coverage();
        assert_eq!(coverage[1].class, SimulationNodeClass::ElaborationConsumed);
        assert_eq!(
            coverage[2].class,
            SimulationNodeClass::IntentionallyUnreachable
        );
        assert!(model.validate_simulation().is_ok());
    }

    #[test]
    fn simulation_coverage_does_not_walk_declaration_only_class_bodies() {
        let db = Db::from_test_nodes(
            "top",
            vec![
                top(vec![NodeId(1)]),
                node(NodeKind::ClassDef, Some(NodeId(0)), vec![NodeId(2)]),
                node(NodeKind::Other, Some(NodeId(1)), vec![]),
            ],
            vec![NodeId(0)],
            HashMap::new(),
        )
        .unwrap();
        let model = SemanticModel::from_db(&db);
        let coverage = model.simulation_coverage();
        assert_eq!(
            coverage[2].class,
            SimulationNodeClass::IntentionallyUnreachable
        );
        assert!(model.validate_simulation().is_ok());
    }

    #[test]
    fn simulation_coverage_rejects_unknown_expression_but_accepts_typed_nodes() {
        let db = Db::from_test_nodes(
            "top",
            vec![
                top(vec![NodeId(1), NodeId(2)]),
                node(NodeKind::Expr(ExprKind::Other), Some(NodeId(0)), vec![]),
                node(NodeKind::Stmt(StmtKind::Empty), Some(NodeId(0)), vec![]),
            ],
            vec![NodeId(0)],
            HashMap::new(),
        )
        .unwrap();
        let model = SemanticModel::from_db(&db);
        let issues = model
            .validate_simulation()
            .expect_err("unknown expressions must fail closed");
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].detail, "expression");
        assert_eq!(
            model.simulation_coverage()[2].class,
            SimulationNodeClass::Executable
        );
    }

    #[test]
    fn synthesis_keeps_elaboration_genvars_distinct_from_runtime_storage() {
        let nodes = vec![
            top(vec![NodeId(1)]),
            node(
                NodeKind::Genvar {
                    ty: TypeInfo {
                        kind: "integer".into(),
                        width: Some(32),
                        signed: true,
                        type_name: None,
                    },
                },
                Some(NodeId(0)),
                vec![],
            ),
        ];
        let db = Db::from_test_nodes("top", nodes, vec![NodeId(0)], HashMap::new()).unwrap();
        assert!(SemanticModel::from_db(&db)
            .validate_synthesizable(SynthesisProfile::PortableRtl)
            .is_ok());
    }

    #[test]
    fn synthesis_rejects_non_integral_or_unresolved_genvar_types() {
        for (kind, width) in [("real", Some(64)), ("integer", None)] {
            let nodes = vec![
                top(vec![NodeId(1)]),
                node(
                    NodeKind::Genvar {
                        ty: TypeInfo {
                            kind: kind.into(),
                            width,
                            signed: true,
                            type_name: None,
                        },
                    },
                    Some(NodeId(0)),
                    vec![],
                ),
            ];
            let db = Db::from_test_nodes("top", nodes, vec![NodeId(0)], HashMap::new()).unwrap();
            let issues = SemanticModel::from_db(&db)
                .validate_synthesizable(SynthesisProfile::PortableRtl)
                .expect_err("genvars require resolved integral metadata");
            assert!(issues.iter().any(|issue| issue.origin.index() == 1));
        }
    }

    #[test]
    fn synthesis_checks_unconnected_port_types() {
        for width in [None, Some(8)] {
            let nodes = vec![
                top(vec![NodeId(1)]),
                node(
                    NodeKind::Port {
                        direction: Direction::Input,
                        ty: TypeInfo {
                            kind: "logic".into(),
                            width,
                            signed: false,
                            type_name: None,
                        },
                        high: None,
                        low: None,
                        high_expr: None,
                        high_present: false,
                        high_open: false,
                    },
                    Some(NodeId(0)),
                    vec![],
                ),
            ];
            let db = Db::from_test_nodes("top", nodes, vec![NodeId(0)], HashMap::new()).unwrap();
            assert_eq!(
                SemanticModel::from_db(&db)
                    .validate_synthesizable(SynthesisProfile::PortableRtl)
                    .is_ok(),
                width.is_some()
            );
        }
    }

    #[test]
    fn synthesis_requires_resolved_static_array_dimensions() {
        for dimensions in [vec![], vec![None], vec![Some((3, 0))]] {
            let resolved = dimensions == vec![Some((3, 0))];
            let nodes = vec![
                top(vec![NodeId(1)]),
                node(
                    NodeKind::Array {
                        ty: TypeInfo {
                            kind: "logic".into(),
                            width: Some(8),
                            signed: false,
                            type_name: None,
                        },
                    },
                    Some(NodeId(0)),
                    vec![],
                ),
            ];
            let arrays = HashMap::from([(
                NodeId(1),
                crate::core::db::ArrayMeta {
                    kind: ArrayKind::Static,
                    dims: dimensions,
                    init: None,
                    net_type: None,
                },
            )]);
            let db = Db::from_test_nodes("top", nodes, vec![NodeId(0)], arrays).unwrap();
            assert_eq!(
                SemanticModel::from_db(&db)
                    .validate_synthesizable(SynthesisProfile::PortableRtl)
                    .is_ok(),
                resolved
            );
        }
    }

    #[test]
    fn synthesis_follows_expression_references_without_structural_children() {
        let nodes = vec![
            top(vec![NodeId(1)]),
            node(
                NodeKind::Stmt(StmtKind::IfElse { cond: NodeId(2) }),
                Some(NodeId(0)),
                vec![],
            ),
            node(NodeKind::Expr(ExprKind::Other), None, vec![]),
        ];
        let db = Db::from_test_nodes("top", nodes, vec![NodeId(0)], HashMap::new()).unwrap();
        let model = SemanticModel::from_db(&db);
        let issues = model
            .validate_synthesizable(SynthesisProfile::PortableRtl)
            .unwrap_err();
        assert!(issues.iter().any(|issue| issue.origin.index() == 2
            && issue.kind == SynthesisIssueKind::UnresolvedExpression));
    }

    #[test]
    fn synthesis_rejects_loops_without_a_static_bound_proof() {
        let nodes = vec![
            top(vec![NodeId(1)]),
            node(
                NodeKind::Stmt(StmtKind::Forever { body: NodeId(2) }),
                Some(NodeId(0)),
                vec![],
            ),
            node(NodeKind::Stmt(StmtKind::Empty), Some(NodeId(1)), vec![]),
        ];
        let db = Db::from_test_nodes("top", nodes, vec![NodeId(0)], HashMap::new()).unwrap();
        let model = SemanticModel::from_db(&db);
        let issues = model
            .validate_synthesizable(SynthesisProfile::PortableRtl)
            .unwrap_err();
        assert!(issues
            .iter()
            .any(|issue| issue.kind == SynthesisIssueKind::UnprovenLoop));
    }

    #[test]
    fn synthesis_rejects_recursive_calls_without_looping_during_reachability() {
        let nodes = vec![
            top(vec![NodeId(1)]),
            node(
                NodeKind::FuncCall {
                    name: "f".into(),
                    is_task: false,
                    callee: Some(NodeId(2)),
                },
                Some(NodeId(0)),
                vec![],
            ),
            node(
                NodeKind::FuncTask {
                    is_task: false,
                    automatic: true,
                    ret: None,
                    body: Some(NodeId(3)),
                },
                None,
                vec![],
            ),
            node(
                NodeKind::Stmt(StmtKind::Begin),
                Some(NodeId(2)),
                vec![NodeId(4)],
            ),
            node(
                NodeKind::FuncCall {
                    name: "f".into(),
                    is_task: false,
                    callee: Some(NodeId(2)),
                },
                Some(NodeId(3)),
                vec![],
            ),
        ];
        let db = Db::from_test_nodes("top", nodes, vec![NodeId(0)], HashMap::new()).unwrap();
        let model = SemanticModel::from_db(&db);
        let issues = model
            .validate_synthesizable(SynthesisProfile::PortableRtl)
            .unwrap_err();
        assert!(issues.iter().any(
            |issue| issue.origin.index() == 4 && issue.kind == SynthesisIssueKind::UnprovenCall
        ));
    }

    #[test]
    fn synthesis_event_profile_requires_one_scalar_clock_or_plain_change_signals() {
        let db = Db::from_test_nodes(
            "top",
            vec![
                top(vec![NodeId(1), NodeId(2)]),
                node(
                    NodeKind::Var {
                        ty: TypeInfo {
                            kind: "logic".into(),
                            width: Some(1),
                            signed: false,
                            type_name: None,
                        },
                    },
                    Some(NodeId(0)),
                    vec![],
                ),
                node(
                    NodeKind::Var {
                        ty: TypeInfo {
                            kind: "logic".into(),
                            width: Some(8),
                            signed: false,
                            type_name: None,
                        },
                    },
                    Some(NodeId(0)),
                    vec![],
                ),
            ],
            vec![NodeId(0)],
            HashMap::new(),
        )
        .unwrap();
        assert!(synthesis_event_control(&db, &[], true));
        assert!(!synthesis_event_control(&db, &[], false));
        assert!(synthesis_event_control(
            &db,
            &[EventSpec::Edge {
                sig: NodeId(1),
                posedge: true
            }],
            false
        ));
        assert!(!synthesis_event_control(
            &db,
            &[EventSpec::Edge {
                sig: NodeId(2),
                posedge: true
            }],
            false
        ));
        assert!(!synthesis_event_control(
            &db,
            &[
                EventSpec::Edge {
                    sig: NodeId(1),
                    posedge: true
                },
                EventSpec::Edge {
                    sig: NodeId(1),
                    posedge: false
                },
            ],
            false
        ));
        assert!(synthesis_event_control(
            &db,
            &[EventSpec::AnyChange { sig: NodeId(2) }],
            false
        ));
    }

    #[test]
    fn portable_rtl_view_rejects_nested_delay_with_source_origin() {
        let nodes = vec![
            top(vec![NodeId(1)]),
            node(
                NodeKind::Process {
                    kind: ProcessKind::Always {
                        always_type: AlwaysKind::Always,
                    },
                },
                Some(NodeId(0)),
                vec![NodeId(2)],
            ),
            node(
                NodeKind::Stmt(StmtKind::DelayControl { delay: NodeId(3) }),
                Some(NodeId(1)),
                vec![],
            ),
            node(
                NodeKind::Expr(ExprKind::Constant {
                    value: crate::core::value::ValueData::UInt(1),
                    size: 32,
                    const_type: ConstantType::Integer,
                    source: crate::core::db::ConstantSource::Exact("1".into()),
                    time_scale: None,
                }),
                Some(NodeId(2)),
                vec![],
            ),
        ];
        let db = Db::from_test_nodes("top", nodes, vec![NodeId(0)], HashMap::new()).unwrap();
        let model = SemanticModel::from_db(&db);
        let issues = model
            .validate_synthesizable(SynthesisProfile::PortableRtl)
            .unwrap_err();
        assert_eq!(issues[0].kind, SynthesisIssueKind::TimingControl);
        assert!(matches!(
            model.origins()[issues[0].origin.index()],
            Origin::Source { .. }
        ));
    }

    fn dump_scope_nodes(name: &str, arguments: Vec<NodeId>, extra_use: bool) -> Vec<Node> {
        let mut children = vec![NodeId(1)];
        if extra_use {
            children.push(NodeId(4));
        }
        let mut nodes = vec![
            top(children),
            node(
                NodeKind::SysCall { name: name.into() },
                Some(NodeId(0)),
                arguments,
            ),
            node(
                NodeKind::Expr(ExprKind::Constant {
                    value: crate::core::value::ValueData::UInt(0),
                    size: 32,
                    const_type: ConstantType::Integer,
                    source: crate::core::db::ConstantSource::Exact("0".into()),
                    time_scale: None,
                }),
                Some(NodeId(1)),
                vec![],
            ),
            node(
                NodeKind::Expr(ExprKind::ScopeRef { target: NodeId(0) }),
                Some(NodeId(1)),
                vec![],
            ),
        ];
        if extra_use {
            nodes.push(node(
                NodeKind::SysCall {
                    name: "$display".into(),
                },
                Some(NodeId(0)),
                vec![NodeId(3)],
            ));
        }
        nodes
    }

    fn dump_scope_database(name: &str, arguments: Vec<NodeId>, extra_use: bool) -> Db {
        Db::from_test_nodes(
            "top",
            dump_scope_nodes(name, arguments, extra_use),
            vec![NodeId(0)],
            HashMap::new(),
        )
        .unwrap()
    }

    #[test]
    fn scope_names_are_metadata_only_at_supported_argument_positions() {
        for (name, arguments, extra_use, accepted) in [
            ("$dumpvars", vec![NodeId(2), NodeId(3)], false, true),
            ("$display", vec![NodeId(2), NodeId(3)], false, false),
            ("$dumpvars", vec![NodeId(3), NodeId(2)], false, false),
            // A shared node in the depth slot is still a value use.
            ("$dumpvars", vec![NodeId(3), NodeId(3)], false, false),
            // Another executable use must not be hidden by the metadata use.
            ("$dumpvars", vec![NodeId(2), NodeId(3)], true, false),
        ] {
            let db = dump_scope_database(name, arguments, extra_use);
            let model = SemanticModel::from_db(&db);
            let expected = if accepted {
                SimulationNodeClass::ElaborationConsumed
            } else {
                SimulationNodeClass::Unsupported
            };
            assert_eq!(model.simulation_coverage()[3].class, expected, "{name}");
            assert_eq!(model.validate_simulation().is_ok(), accepted, "{name}");
        }
    }

    #[test]
    fn interface_scope_metadata_requires_the_connected_interface_identity() {
        for (target, actual, accepted) in [
            (NodeId(2), NodeId(2), true),  // bare interface actual
            (NodeId(4), NodeId(2), true),  // modport of that interface
            (NodeId(0), NodeId(2), false), // unrelated module
            (NodeId(4), NodeId(0), false), // a module is not an interface
            (NodeId(4), NodeId(5), false), // another interface instance
        ] {
            let interface = |children| {
                node(
                    NodeKind::ModuleInst {
                        def_name: "bus".into(),
                        is_top: false,
                        is_interface: true,
                        timeunit: 0,
                        timeprecision: 0,
                    },
                    Some(NodeId(0)),
                    children,
                )
            };
            let nodes = vec![
                top(vec![NodeId(1), NodeId(2), NodeId(5)]),
                node(
                    NodeKind::Port {
                        direction: Direction::Input,
                        ty: TypeInfo {
                            kind: "interface".into(),
                            width: None,
                            signed: false,
                            type_name: None,
                        },
                        high: None,
                        low: None,
                        high_expr: Some(NodeId(3)),
                        high_present: true,
                        high_open: false,
                    },
                    Some(NodeId(0)),
                    vec![NodeId(6), NodeId(3)],
                ),
                interface(vec![NodeId(4)]),
                node(
                    NodeKind::Expr(ExprKind::ScopeRef { target }),
                    Some(NodeId(1)),
                    vec![],
                ),
                node(NodeKind::ModPort, Some(NodeId(2)), vec![]),
                interface(vec![]),
                node(
                    NodeKind::IfaceConn {
                        actual,
                        modport: String::new(),
                    },
                    Some(NodeId(1)),
                    vec![],
                ),
            ];
            let db = Db::from_test_nodes("top", nodes, vec![NodeId(0)], HashMap::new()).unwrap();
            assert_eq!(
                SemanticModel::from_db(&db).validate_simulation().is_ok(),
                accepted,
            );
        }
    }

    #[test]
    fn a_scope_reference_does_not_make_its_target_body_executable() {
        let mut nodes = dump_scope_nodes("$dumpvars", vec![NodeId(2), NodeId(3)], false);
        // Build a separate scope with an unsupported child, reached by a name
        // rather than through the selected design's structural children.
        nodes[3].kind = NodeKind::Expr(ExprKind::ScopeRef { target: NodeId(4) });
        nodes.push(top(vec![NodeId(5)]));
        nodes.push(node(NodeKind::Other, Some(NodeId(4)), vec![]));
        let db = Db::from_test_nodes("top", nodes, vec![NodeId(0)], HashMap::new()).unwrap();
        let coverage = SemanticModel::from_db(&db).simulation_coverage();
        assert_eq!(coverage[3].class, SimulationNodeClass::ElaborationConsumed);
        assert_eq!(
            coverage[4].class,
            SimulationNodeClass::IntentionallyUnreachable
        );
        assert_eq!(
            coverage[5].class,
            SimulationNodeClass::IntentionallyUnreachable
        );
    }
}
