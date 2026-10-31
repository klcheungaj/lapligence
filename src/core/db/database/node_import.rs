//! Node import.

use super::*;
use std::sync::Arc;

pub(super) fn node_kind_from_slang(
    snapshot: &SlangSnapshot,
    type_projector: &SlangTypeProjector<'_>,
    node: &SemanticNode,
    edges: &[crate::ffi::slang::SemanticEdge],
    ids: &SemanticIds,
    ty: TypeInfo,
    udp_tables: &HashMap<u64, Arc<UdpTable>>,
) -> Result<NodeKind, DbError> {
    let first = |role| edge_target(ids, edges, role);
    Ok(match node.kind {
        SemanticKind::Instance if node.subkind == 193 => NodeKind::InstanceArray,
        SemanticKind::Instance | SemanticKind::Definition => NodeKind::ModuleInst {
            def_name: if node.definition_name.is_empty() {
                node.name.to_string()
            } else {
                node.definition_name.to_string()
            },
            is_top: node.is_top(),
            is_interface: node.definition_kind == Some(SemanticDefinitionKind::Interface),
            timeunit: time_exponent(node.time_scale(), false)?,
            timeprecision: time_exponent(node.time_scale(), true)?,
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
                // A multi-port's low connections are its component ports
                // (`Db::multi_port_components`), and a select or explicit ANSI
                // port expression is `Db::port_internal_expression`: neither
                // is one whole internal symbol.
                low: if node.detail == "MultiPort"
                    || port_internal_expression(snapshot, ids, edges)?.is_some()
                {
                    None
                } else {
                    resolved_edge_target(snapshot, ids, edges, SemanticEdgeRole::LowConnection)?
                },
                high_expr,
                high_present: node.port_connection_present(),
                high_open: node.port_connection_open(),
            }
        }
        SemanticKind::Modport => NodeKind::ModPort,
        SemanticKind::InterfaceConnection => NodeKind::IfaceConn {
            actual: node
                .target_id()
                .map(|id| semantic_id(ids, id))
                .transpose()?
                .ok_or_else(|| {
                    DbError::InvalidSnapshot("interface connection has no target".into())
                })?,
            modport: node.name.to_string(),
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
                .constant_id()
                .and_then(|id| snapshot.constants.get(id as usize))
                .and_then(|constant| val_from_slang(&constant.value)),
            local: node.is_local(),
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
        SemanticKind::Primitive if node.is_primitive_instance() => {
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
            let udp = if matches!(
                prim_type,
                PrimitiveType::Combinational | PrimitiveType::Sequential
            ) {
                let primitive_id = node.target_id().ok_or_else(|| {
                    DbError::InvalidSnapshot(format!(
                        "UDP instance `{}` has no declaration identity",
                        node.definition_name
                    ))
                })?;
                Some(Arc::clone(udp_tables.get(&primitive_id).ok_or_else(
                    || {
                        DbError::InvalidSnapshot(format!(
                            "UDP instance `{}` has no owned table for declaration {}",
                            node.definition_name, primitive_id
                        ))
                    },
                )?))
            } else {
                None
            };
            let is_array_element = node
                .parent_id()
                .and_then(|parent| snapshot.semantic_nodes.get(parent as usize))
                .is_some_and(|parent| {
                    parent.kind == SemanticKind::Instance && parent.subkind == 193
                });
            NodeKind::Gate {
                class: if matches!(
                    prim_type,
                    PrimitiveType::Sequential | PrimitiveType::Combinational
                ) {
                    PrimClass::Udp
                } else if is_array_element {
                    PrimClass::Array
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
                udp,
            }
        }
        SemanticKind::Primitive => NodeKind::Other,
        SemanticKind::Subroutine => NodeKind::FuncTask {
            is_task: node.is_task(),
            automatic: node.is_automatic(),
            is_static: node.auxiliary & crate::ffi::slang::SUBROUTINE_STATIC != 0,
            is_virtual: node.auxiliary & crate::ffi::slang::SUBROUTINE_VIRTUAL != 0,
            is_pure: node.auxiliary & crate::ffi::slang::SUBROUTINE_PURE != 0,
            is_final: node.auxiliary & crate::ffi::slang::SUBROUTINE_FINAL != 0,
            is_constructor: node.auxiliary & crate::ffi::slang::SUBROUTINE_CONSTRUCTOR != 0,
            ret: (!node.is_task() && ty.kind != "void").then_some(ty),
            body: first(SemanticEdgeRole::Body)?,
        },
        SemanticKind::Argument => NodeKind::FuncArg {
            direction: direction_from_slang(node),
            ty,
            default: first(SemanticEdgeRole::DefaultValue)?,
            const_ref: node.is_const_ref(),
            ref_static: node.is_ref_static(),
        },
        SemanticKind::Statement => {
            statement_from_slang(snapshot, type_projector, node, edges, ids)?
        }
        SemanticKind::Expression => {
            expression_from_slang(snapshot, type_projector, node, edges, ids, ty)?
        }
        SemanticKind::AssertionExpr => assertion_expr_from_slang(snapshot, node, edges, ids)?,
        SemanticKind::ConstraintBlock => constraint_block_from_slang(node, edges, ids)?,
        SemanticKind::Constraint => constraint_from_slang(node, edges, ids)?,
        SemanticKind::SystemCall => NodeKind::SysCall {
            name: node.name.to_string(),
        },
        SemanticKind::MethodCall => NodeKind::MethodCall {
            name: node.name.to_string(),
            receiver: first(SemanticEdgeRole::Receiver)?,
            callee: first(SemanticEdgeRole::Callee)?,
        },
        SemanticKind::FunctionCall => NodeKind::FuncCall {
            name: node.name.to_string(),
            is_task: node.is_task(),
            is_super: node.auxiliary & crate::ffi::slang::CALL_SUPER != 0,
            callee: first(SemanticEdgeRole::Callee)?,
        },
        SemanticKind::EnumConstant => NodeKind::EnumConst {
            value: node
                .constant_id()
                .and_then(|id| snapshot.constants.get(id as usize))
                .and_then(|constant| val_from_slang(&constant.value)),
        },
        SemanticKind::Scope => NodeKind::Stmt(StmtKind::Begin),
        SemanticKind::TimingControl => NodeKind::Other,
        // Coverage declarations keep their typed facts in the Db's coverage
        // side tables; the node itself is a declaration.
        SemanticKind::Coverage => NodeKind::Other,
        SemanticKind::Production => production_from_slang(node, edges, ids)?,
        // Specify blocks keep their typed facts in the Db's specify side
        // tables (`Db::module_paths`); the nodes give them stable identity.
        SemanticKind::Specify => NodeKind::Other,
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
        // owned tree records that this declaration was consumed during
        // elaboration, with the parameter and value expression the VPI object
        // model reports (IEEE 1800-2009 37.26); classifying it as an
        // executable `Other` node would make every legal defparam a lowering
        // failure.
        SemanticKind::Unsupported if node.detail == "DefParam" => NodeKind::ParamAssign {
            overridden: true,
            target: first(SemanticEdgeRole::Reference)?,
        },
        SemanticKind::Unsupported => NodeKind::Other,
    })
}

/// Import every UDP declaration's table once, with source positions, so its
/// instances share one immutable owned table.
pub(super) fn udp_tables_from_slang(
    snapshot: &SlangSnapshot,
    positions: &mut SourcePositions<'_>,
) -> Result<HashMap<u64, Arc<UdpTable>>, DbError> {
    let mut location = |range: Option<crate::ffi::slang::SourceRange>| match range {
        Some(range) => positions
            .range_position(range)
            .map(|(file, line, col, _, _)| UdpLocation { file, line, col }),
        None => Ok(UdpLocation::default()),
    };
    // The ports of every UDP declaration, collected in one pass: they are
    // the declaration's primitive-port children.
    let mut ports: HashMap<u64, Vec<(String, Direction)>> = snapshot
        .udp_tables
        .iter()
        .map(|table| (table.primitive_id, Vec::new()))
        .collect();
    for node in &snapshot.semantic_nodes {
        if !node.is_primitive_port() {
            continue;
        }
        let Some(list) = node.parent_id().and_then(|parent| ports.get_mut(&parent)) else {
            continue;
        };
        let direction = if node.is_inout() {
            Direction::Inout
        } else if node.is_output() {
            Direction::Output
        } else if node.is_input() {
            Direction::Input
        } else {
            Direction::None
        };
        list.push((node.name.to_string(), direction));
    }
    let mut tables = HashMap::with_capacity(snapshot.udp_tables.len());
    for table in &snapshot.udp_tables {
        let declaration = snapshot
            .semantic_nodes
            .get(table.primitive_id as usize)
            .ok_or_else(|| {
                DbError::InvalidSnapshot(format!(
                    "UDP table `{}` has no declaration node",
                    table.name
                ))
            })?;
        let rows = table
            .rows
            .iter()
            .map(|row| {
                Ok(UdpRow {
                    inputs: row.inputs.clone(),
                    state: row.state,
                    output: row.output,
                    location: location(row.range)?,
                })
            })
            .collect::<Result<Vec<_>, DbError>>()?;
        tables.insert(
            table.primitive_id,
            Arc::new(UdpTable {
                name: table.name.clone(),
                input_count: table.input_count,
                sequential: table.sequential,
                initial: table.initial,
                rows,
                ports: ports.remove(&table.primitive_id).unwrap_or_default(),
                location: location(declaration.range)?,
            }),
        );
    }
    Ok(tables)
}
