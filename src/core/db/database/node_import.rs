//! Node import.

use super::*;

pub(super) fn node_kind_from_slang(
    snapshot: &SlangSnapshot,
    type_projector: &SlangTypeProjector<'_>,
    node: &SemanticNode,
    edges: &[crate::ffi::slang::SemanticEdge],
    ids: &HashMap<u64, NodeId>,
    ty: TypeInfo,
) -> Result<NodeKind, DbError> {
    let first = |role| edge_target(ids, edges, role);
    Ok(match node.kind {
        SemanticKind::Instance if node.subkind == 193 => NodeKind::InstanceArray,
        SemanticKind::Instance | SemanticKind::Definition => NodeKind::ModuleInst {
            def_name: if node.definition_name.is_empty() {
                node.name.clone()
            } else {
                node.definition_name.clone()
            },
            is_top: node.is_top,
            is_interface: node.definition_kind == Some(SemanticDefinitionKind::Interface),
            timeunit: time_exponent(node.time_scale, false)?,
            timeprecision: time_exponent(node.time_scale, true)?,
        },
        SemanticKind::Package => NodeKind::Package,
        SemanticKind::Class => NodeKind::ClassDef,
        SemanticKind::GenerateScope if node.subkind == 196 => NodeKind::GenScopeArray,
        SemanticKind::GenerateScope => NodeKind::GenScope,
        SemanticKind::Port => {
            let direction = direction_from_slang(node);
            let high_expr = first(SemanticEdgeRole::HighConnection)?
                .map(|expression| {
                    connection_source_expression(snapshot, ids, expression, direction)
                })
                .transpose()?;
            let high =
                resolved_edge_target(snapshot, ids, edges, SemanticEdgeRole::HighConnection)?;
            NodeKind::Port {
                direction,
                ty,
                strength0: strength_from_slang(node.strength0),
                strength1: strength_from_slang(node.strength1),
                high,
                low: resolved_edge_target(snapshot, ids, edges, SemanticEdgeRole::LowConnection)?,
                high_expr,
                high_present: node.port_connection_present,
                high_open: node.port_connection_open,
            }
        }
        SemanticKind::Modport => NodeKind::ModPort,
        SemanticKind::InterfaceConnection => NodeKind::IfaceConn {
            actual: node
                .target_id
                .map(|id| semantic_id(ids, id))
                .transpose()?
                .ok_or_else(|| {
                    DbError::InvalidSnapshot("interface connection has no target".into())
                })?,
            modport: node.name.clone(),
        },
        SemanticKind::Net => NodeKind::Net {
            ty,
            net_type: net_type_from_subkind(node.subkind),
            strength0: strength_from_slang(node.strength0),
            strength1: strength_from_slang(node.strength1),
        },
        SemanticKind::NetAlias => NodeKind::NetAlias {
            nets: edge_targets(ids, edges, SemanticEdgeRole::AliasNet)?,
        },
        SemanticKind::Variable if node.subkind == 229 => NodeKind::Genvar { ty },
        SemanticKind::Variable => NodeKind::Var { ty },
        SemanticKind::Array => NodeKind::Array { ty },
        SemanticKind::NamedEvent => NodeKind::NamedEvent,
        SemanticKind::Parameter => NodeKind::Param {
            ty,
            value: node
                .constant_id
                .and_then(|id| snapshot.constants.get(id as usize))
                .and_then(|constant| val_from_slang(&constant.value)),
            local: node.is_local,
        },
        SemanticKind::Process => NodeKind::Process {
            kind: match node.subkind {
                1 => ProcessKind::Initial,
                2 => ProcessKind::Final,
                4 => ProcessKind::Always {
                    always_type: AlwaysKind::Comb,
                },
                5 => ProcessKind::Always {
                    always_type: AlwaysKind::Latch,
                },
                6 => ProcessKind::Always {
                    always_type: AlwaysKind::FlipFlop,
                },
                _ => ProcessKind::Always {
                    always_type: AlwaysKind::Always,
                },
            },
        },
        SemanticKind::ContinuousAssign => NodeKind::ContAssign {
            net_decl: node.subkind == 228,
            delay: driver_delay(snapshot, ids, edges)?,
            strength0: strength_from_slang(node.strength0),
            strength1: strength_from_slang(node.strength1),
        },
        SemanticKind::Primitive if node.is_primitive_instance => {
            let terms = edges
                .iter()
                .filter(|edge| edge.role == SemanticEdgeRole::Actual)
                .map(|edge| {
                    let expression = snapshot
                        .semantic_nodes
                        .get(edge.target_id as usize)
                        .ok_or_else(|| {
                            DbError::InvalidSnapshot(
                                "primitive terminal expression is missing".into(),
                            )
                        })?;
                    let direction = direction_from_slang(expression);
                    Ok(GateTerm {
                        direction,
                        term_index: i32::try_from(edge.index).map_err(|_| {
                            DbError::InvalidSnapshot("primitive terminal index is too large".into())
                        })?,
                        expr: peel_gate_terminal(
                            snapshot,
                            ids,
                            semantic_id(ids, edge.target_id)?,
                            direction,
                        )?,
                    })
                })
                .collect::<Result<Vec<_>, DbError>>()?;
            let prim_type = primitive_type_from_subkind(node.subkind);
            let is_array_element = node
                .parent_id
                .and_then(|parent| snapshot.semantic_nodes.get(parent as usize))
                .is_some_and(|parent| {
                    parent.kind == SemanticKind::Instance && parent.subkind == 193
                });
            NodeKind::Gate {
                class: if is_array_element {
                    PrimClass::Array
                } else if matches!(
                    prim_type,
                    PrimitiveType::Sequential | PrimitiveType::Combinational
                ) {
                    PrimClass::Udp
                } else if matches!(
                    prim_type,
                    PrimitiveType::Nmos
                        | PrimitiveType::Pmos
                        | PrimitiveType::Cmos
                        | PrimitiveType::Rnmos
                        | PrimitiveType::Rpmos
                        | PrimitiveType::Rcmos
                        | PrimitiveType::Rtran
                        | PrimitiveType::Rtranif0
                        | PrimitiveType::Rtranif1
                        | PrimitiveType::Tran
                        | PrimitiveType::Tranif0
                        | PrimitiveType::Tranif1
                ) {
                    PrimClass::Switch
                } else {
                    PrimClass::Gate
                },
                prim_type,
                strength0: strength_from_slang(node.strength0),
                strength1: strength_from_slang(node.strength1),
                delay: driver_delay(snapshot, ids, edges)?,
                terms,
            }
        }
        SemanticKind::Primitive => NodeKind::Other,
        SemanticKind::Subroutine => NodeKind::FuncTask {
            is_task: node.is_task,
            automatic: node.is_automatic,
            is_static: node.auxiliary & crate::ffi::slang::SUBROUTINE_STATIC != 0,
            is_virtual: node.auxiliary & crate::ffi::slang::SUBROUTINE_VIRTUAL != 0,
            is_pure: node.auxiliary & crate::ffi::slang::SUBROUTINE_PURE != 0,
            is_final: node.auxiliary & crate::ffi::slang::SUBROUTINE_FINAL != 0,
            is_constructor: node.auxiliary & crate::ffi::slang::SUBROUTINE_CONSTRUCTOR != 0,
            ret: (!node.is_task && ty.kind != "void").then_some(ty),
            body: first(SemanticEdgeRole::Body)?,
        },
        SemanticKind::Argument => NodeKind::FuncArg {
            direction: direction_from_slang(node),
            ty,
            default: first(SemanticEdgeRole::DefaultValue)?,
            const_ref: node.is_const_ref,
            ref_static: node.is_ref_static,
        },
        SemanticKind::Statement => statement_from_slang(snapshot, node, edges, ids)?,
        SemanticKind::Expression => {
            expression_from_slang(snapshot, type_projector, node, edges, ids, ty)?
        }
        SemanticKind::AssertionExpr => assertion_expr_from_slang(snapshot, node, edges, ids)?,
        SemanticKind::SystemCall => NodeKind::SysCall {
            name: node.name.clone(),
        },
        SemanticKind::MethodCall => NodeKind::MethodCall {
            name: node.name.clone(),
            receiver: first(SemanticEdgeRole::Receiver)?,
            callee: first(SemanticEdgeRole::Callee)?,
        },
        SemanticKind::FunctionCall => NodeKind::FuncCall {
            name: node.name.clone(),
            is_task: node.is_task,
            is_super: node.auxiliary & crate::ffi::slang::CALL_SUPER != 0,
            callee: first(SemanticEdgeRole::Callee)?,
        },
        SemanticKind::EnumConstant => NodeKind::EnumConst {
            value: node
                .constant_id
                .and_then(|id| snapshot.constants.get(id as usize))
                .and_then(|constant| val_from_slang(&constant.value)),
        },
        SemanticKind::Scope => NodeKind::Stmt(StmtKind::Begin),
        SemanticKind::TimingControl => NodeKind::Other,
        // Assertion-control hierarchy arguments are exported by vendored
        // Slang as an `ArbitrarySymbol` expression even though that node is
        // not assigned the ordinary expression semantic kind. Preserve its
        // resolved target as owned scope metadata instead of exposing a
        // borrowed/opaque executable node to simulator lowering.
        SemanticKind::Unsupported if node.detail == "ArbitrarySymbol" => {
            match first(SemanticEdgeRole::Reference)? {
                Some(target) => NodeKind::Expr(ExprKind::ScopeRef { target }),
                None => NodeKind::Other,
            }
        }
        // A `defparam` is an elaboration-time parameter assignment. Slang has
        // already applied its override when the snapshot is captured, so the
        // owned tree only needs to record that this declaration was consumed
        // during elaboration; classifying it as an executable `Other` node
        // would make every legal defparam a lowering failure.
        SemanticKind::Unsupported if node.detail == "DefParam" => {
            NodeKind::ParamAssign { overridden: true }
        }
        SemanticKind::Unsupported => NodeKind::Other,
    })
}
