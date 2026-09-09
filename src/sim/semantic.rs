//! Frontend-independent semantic simulation model.
//!
//! [`SemanticModel`] validates and classifies the owned elaborated database
//! before executable lowering starts. It answers what declarations and
//! operations mean; runtime scheduling, coroutine state, and C spelling are
//! absent from this layer.

use crate::core::db::{
    AlwaysKind, ArrayKind, CaseKind, ConstantType, Db, Direction, EventSpec, ExprKind, NetType,
    NodeId, NodeKind, Operation, PrimClass, PrimitiveType, ProcessKind, StmtKind, Strength,
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
            node.kind().append_references(&mut pending);
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
}
