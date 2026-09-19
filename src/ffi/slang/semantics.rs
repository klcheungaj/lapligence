//! Semantics.

use super::*;

pub(super) fn decode_semantic_edges(
    raw: &[RawSemanticEdge],
    nodes: &[RawSemanticNode],
) -> Result<Vec<SemanticEdge>, SlangError> {
    let node_ids: HashSet<_> = nodes.iter().map(|node| node.id).collect();
    raw.iter()
        .map(|edge| {
            if !node_ids.contains(&edge.target_id) {
                return Err(invalid_native("semantic edge target does not exist"));
            }
            if edge.sequence_delay_valid > 1
                || (edge.sequence_delay_valid != 0
                    && edge.sequence_delay_max != SEMANTIC_ASSERTION_RANGE_UNBOUNDED
                    && edge.sequence_delay_max < edge.sequence_delay_min)
            {
                return Err(invalid_native(
                    "semantic edge has an invalid sequence delay range",
                ));
            }
            let role = match edge.role {
                1 => SemanticEdgeRole::Child,
                2 => SemanticEdgeRole::HighConnection,
                3 => SemanticEdgeRole::LowConnection,
                4 => SemanticEdgeRole::Initializer,
                5 => SemanticEdgeRole::Lhs,
                6 => SemanticEdgeRole::Rhs,
                7 => SemanticEdgeRole::Condition,
                8 => SemanticEdgeRole::Then,
                9 => SemanticEdgeRole::Else,
                10 => SemanticEdgeRole::Body,
                11 => SemanticEdgeRole::Operand,
                12 => SemanticEdgeRole::Index,
                13 => SemanticEdgeRole::Left,
                14 => SemanticEdgeRole::Right,
                15 => SemanticEdgeRole::Base,
                16 => SemanticEdgeRole::Width,
                17 => SemanticEdgeRole::Delay,
                18 => SemanticEdgeRole::Event,
                19 => SemanticEdgeRole::Argument,
                20 => SemanticEdgeRole::Receiver,
                21 => SemanticEdgeRole::Callee,
                22 => SemanticEdgeRole::Actual,
                23 => SemanticEdgeRole::DefaultValue,
                24 => SemanticEdgeRole::CaseItem,
                25 => SemanticEdgeRole::CaseExpression,
                26 => SemanticEdgeRole::Branch,
                27 => SemanticEdgeRole::Increment,
                28 => SemanticEdgeRole::Declaration,
                29 => SemanticEdgeRole::Reference,
                30 => SemanticEdgeRole::SourceIdentity,
                31 => SemanticEdgeRole::ReturnOwner,
                32 => SemanticEdgeRole::AliasNet,
                33 => SemanticEdgeRole::PropertySpec,
                34 => SemanticEdgeRole::Clocking,
                35 => SemanticEdgeRole::AssertionFormal,
                36 => SemanticEdgeRole::AssertionActual,
                37 => SemanticEdgeRole::BaseConstructor,
                _ => return Err(invalid_native("semantic edge has an unknown role")),
            };
            Ok(SemanticEdge {
                role,
                index: edge.index,
                target_id: edge.target_id,
                sequence_delay: (edge.sequence_delay_valid != 0).then(|| SemanticSequenceRange {
                    min: edge.sequence_delay_min,
                    max: (edge.sequence_delay_max != u32::MAX).then_some(edge.sequence_delay_max),
                }),
            })
        })
        .collect()
}

pub(super) fn decode_semantic_nodes(
    raw: &[RawSemanticNode],
    edges: &[SemanticEdge],
    files: &[File],
    types: &[Type],
    constant_len: usize,
) -> Result<Vec<SemanticNode>, SlangError> {
    let ids: HashSet<_> = raw.iter().map(|node| node.id).collect();
    if ids.len() != raw.len() || ids.contains(&INVALID_ID) {
        return Err(invalid_native(
            "snapshot contains duplicate or invalid semantic node ids",
        ));
    }
    if raw
        .iter()
        .enumerate()
        .any(|(index, node)| node.id != index as u64)
    {
        return Err(invalid_native(
            "semantic node ids are not contiguous arena indices",
        ));
    }
    let type_ids: HashSet<_> = types.iter().map(|ty| ty.id).collect();
    let mut claimed_edges = vec![false; edges.len()];
    for node in raw {
        let window = checked_window(
            node.edge_start,
            node.edge_count,
            edges.len(),
            "semantic node edges",
        )?;
        for index in window {
            if claimed_edges[index] {
                return Err(invalid_native("semantic node edge windows overlap"));
            }
            claimed_edges[index] = true;
        }
    }
    if claimed_edges.iter().any(|claimed| !claimed) {
        return Err(invalid_native("semantic edge is not owned by a node"));
    }
    raw.iter()
        .map(|node| {
            if (node.flags & ((1 << 8) | (1 << 9) | (1 << 10) | (1 << 11))).count_ones() > 1 {
                return Err(invalid_native(
                    "semantic node has conflicting direction flags",
                ));
            }
            if node.flags & (1 << 2) != 0 && node.flags & (1 << 3) != 0 {
                return Err(invalid_native("semantic node is both automatic and static"));
            }
            if (node.flags & ((1 << 13) | (1 << 14) | (1 << 15))).count_ones() > 1 {
                return Err(invalid_native(
                    "semantic node has conflicting definition-kind flags",
                ));
            }
            if node.flags & (1 << 29) != 0 && node.flags & (1 << 28) == 0 {
                return Err(invalid_native(
                    "semantic node has an open port connection without a connection",
                ));
            }
            if node.flags & (1 << 31) != 0 && node.kind != 21 {
                return Err(invalid_native(
                    "semantic with-clause flag is set on a non-method call",
                ));
            }
            validate_semantic_subkind(node.kind, node.subkind)?;
            validate_semantic_auxiliary(node)?;
            if (node.flags & ((1 << 16) | (1 << 17))).count_ones() > 1
                || (node.flags & ((1 << 18) | (1 << 19) | (1 << 20))).count_ones() > 1
                || (node.flags & ((1 << 21) | (1 << 22) | (1 << 23))).count_ones() > 1
            {
                return Err(invalid_native(
                    "semantic node has conflicting subtype flags",
                ));
            }
            if (node.flags & ((1 << 24) | (1 << 25) | (1 << 26))).count_ones() > 1 {
                return Err(invalid_native(
                    "semantic node has conflicting primitive-role flags",
                ));
            }
            let parent_id = (node.parent_id != INVALID_ID).then_some(node.parent_id);
            let target_id = (node.target_id != INVALID_ID).then_some(node.target_id);
            if parent_id.is_some_and(|id| !ids.contains(&id))
                || target_id.is_some_and(|id| !ids.contains(&id))
            {
                return Err(invalid_native(
                    "semantic node refers to an unknown semantic node",
                ));
            }
            let type_id = (node.type_id != INVALID_ID).then_some(node.type_id);
            if type_id.is_some_and(|id| !type_ids.contains(&id)) {
                return Err(invalid_native("semantic node type does not exist"));
            }
            let constant_id = (node.constant_id != INVALID_ID).then_some(node.constant_id);
            if constant_id
                .is_some_and(|id| usize::try_from(id).map_or(true, |id| id >= constant_len))
            {
                return Err(invalid_native("semantic node constant does not exist"));
            }
            let window = checked_window(
                node.edge_start,
                node.edge_count,
                edges.len(),
                "semantic node edges",
            )?;
            let mut edge_keys = HashSet::new();
            for edge in &edges[window] {
                if !edge_keys.insert((edge.role, edge.index)) {
                    return Err(invalid_native(
                        "semantic node has duplicate role/index edges",
                    ));
                }
            }
            let kind = decode_semantic_kind(node.kind)?;
            Ok(SemanticNode {
                id: node.id,
                parent_id,
                kind,
                subkind: node.subkind,
                operation: decode_semantic_operation(node.operation)?,
                is_bad: node.flags & 1 != 0,
                is_uninstantiated: node.flags & 2 != 0,
                is_automatic: node.flags & 4 != 0,
                is_static: node.flags & 8 != 0,
                is_top: node.flags & (1 << 4) != 0,
                is_implicit: node.flags & (1 << 5) != 0,
                is_local: node.flags & (1 << 6) != 0,
                is_nonblocking: node.flags & (1 << 7) != 0,
                is_input: node.flags & (1 << 8) != 0,
                is_output: node.flags & (1 << 9) != 0,
                is_inout: node.flags & (1 << 10) != 0,
                is_ref: node.flags & (1 << 11) != 0,
                is_const_ref: kind == SemanticKind::Argument
                    && node.auxiliary & ARGUMENT_CONST_REF != 0,
                is_ref_static: kind == SemanticKind::Argument
                    && node.auxiliary & ARGUMENT_REF_STATIC != 0,
                is_implicit_conversion: node.flags & (1 << 12) != 0,
                is_propagated_conversion: node.flags & (1 << 30) != 0,
                is_indexed_up: node.flags & (1 << 16) != 0,
                is_indexed_down: node.flags & (1 << 17) != 0,
                case_wildcard_x_or_z: node.flags & (1 << 18) != 0,
                case_wildcard_z: node.flags & (1 << 19) != 0,
                case_inside: node.flags & (1 << 20) != 0,
                is_posedge: node.flags & (1 << 21) != 0,
                is_negedge: node.flags & (1 << 22) != 0,
                is_both_edges: node.flags & (1 << 23) != 0,
                is_primitive_declaration: node.flags & (1 << 24) != 0,
                is_primitive_instance: node.flags & (1 << 25) != 0,
                is_primitive_port: node.flags & (1 << 26) != 0,
                is_task: node.flags & (1 << 27) != 0,
                port_connection_present: node.flags & (1 << 28) != 0,
                port_connection_open: node.flags & (1 << 29) != 0,
                method_with_clause: node.flags & (1 << 31) != 0,
                definition_kind: if node.flags & (1 << 13) != 0 {
                    Some(SemanticDefinitionKind::Module)
                } else if node.flags & (1 << 14) != 0 {
                    Some(SemanticDefinitionKind::Interface)
                } else if node.flags & (1 << 15) != 0 {
                    Some(SemanticDefinitionKind::Program)
                } else {
                    None
                },
                // SAFETY: native strings borrow from the live snapshot.
                name: unsafe { copy_string(node.name, "semantic node name")? },
                // SAFETY: native strings borrow from the live snapshot.
                detail: unsafe { copy_string(node.detail, "semantic node detail")? },
                // SAFETY: native strings borrow from the live snapshot.
                definition_name: unsafe {
                    copy_string(node.definition_name, "semantic node definition name")?
                },
                range: decode_range(node.range, files)?,
                type_id,
                constant_id,
                target_id,
                edge_start: node.edge_start,
                edge_count: node.edge_count,
                time_scale: decode_time_scale(node)?,
                strength0: decode_drive_strength(node.strength0)?,
                strength1: decode_drive_strength(node.strength1)?,
                auxiliary: node.auxiliary,
                assertion_range_min: node.assertion_range_min,
                assertion_range_max: (node.kind == 28 && node.assertion_range_max != u32::MAX)
                    .then_some(node.assertion_range_max),
                assertion_repetition_kind: node.assertion_repetition_kind,
            })
        })
        .collect()
}

fn decode_semantic_kind(raw: u32) -> Result<SemanticKind, SlangError> {
    Ok(match raw {
        1 => SemanticKind::Instance,
        2 => SemanticKind::Package,
        3 => SemanticKind::Class,
        4 => SemanticKind::GenerateScope,
        5 => SemanticKind::Port,
        6 => SemanticKind::Modport,
        7 => SemanticKind::InterfaceConnection,
        8 => SemanticKind::Net,
        9 => SemanticKind::Variable,
        10 => SemanticKind::Array,
        11 => SemanticKind::NamedEvent,
        12 => SemanticKind::Parameter,
        13 => SemanticKind::Process,
        14 => SemanticKind::ContinuousAssign,
        15 => SemanticKind::Primitive,
        16 => SemanticKind::Subroutine,
        17 => SemanticKind::Argument,
        18 => SemanticKind::Statement,
        19 => SemanticKind::Expression,
        20 => SemanticKind::SystemCall,
        21 => SemanticKind::MethodCall,
        22 => SemanticKind::FunctionCall,
        23 => SemanticKind::EnumConstant,
        24 => SemanticKind::Definition,
        25 => SemanticKind::Scope,
        26 => SemanticKind::TimingControl,
        27 => SemanticKind::NetAlias,
        28 => SemanticKind::AssertionExpr,
        255 => SemanticKind::Unsupported,
        _ => return Err(invalid_native("semantic node has an unknown kind")),
    })
}

pub(super) fn validate_semantic_subkind(kind: u32, subkind: u32) -> Result<(), SlangError> {
    let valid = match kind {
        1 => matches!(subkind, 0 | 192 | 193),
        4 => matches!(subkind, 0 | 195 | 196),
        8 => matches!(subkind, 0 | 128..=141),
        13 => matches!(subkind, 0..=6),
        14 => matches!(subkind, 0 | 228),
        15 => matches!(subkind, 0 | 160..=164 | 200..=227),
        18 => matches!(subkind, 0 | 32..=67 | SEMANTIC_STMT_PATTERN_CASE),
        19 => matches!(subkind, 0 | 64..=78 | 80..=91),
        25 => matches!(subkind, 0 | 194 | SEMANTIC_SCOPE_CLOCKING_BLOCK),
        26 => matches!(subkind, 0 | 112..=118),
        28 => matches!(subkind, 0..=13),
        20..=22 => matches!(subkind, 0 | 76),
        9 => matches!(
            subkind,
            0 | 229 | SEMANTIC_VARIABLE_CLOCKING | SEMANTIC_VARIABLE_ASSERTION_LOCAL
        ),
        2 | 3 | 5..=7 | 10..=12 | 16 | 17 | 23 | 24 | 27 | 255 => subkind == 0,
        _ => true,
    };
    if !valid {
        return Err(invalid_native(
            "semantic node has a subkind incompatible with its kind",
        ));
    }
    Ok(())
}

fn validate_semantic_auxiliary(node: &RawSemanticNode) -> Result<(), SlangError> {
    let valid = match (node.kind, node.subkind, node.operation) {
        // An incomplete declaration placeholder may not carry its resolved
        // lifetime yet; complete variable nodes use static or automatic.
        (9 | 11, _, _) if node.subkind == SEMANTIC_VARIABLE_CLOCKING => {
            node.auxiliary
                & !(CLOCKING_EDGE_MASK | (CLOCKING_EDGE_MASK << CLOCKING_VAR_OUTPUT_EDGE_SHIFT))
                == 0
        }
        (9 | 11, _, _) => matches!(node.auxiliary, 0..=2),
        (25, SEMANTIC_SCOPE_CLOCKING_BLOCK, _) => {
            let edge_mask = CLOCKING_EDGE_MASK << CLOCKING_INPUT_EDGE_SHIFT
                | CLOCKING_EDGE_MASK << CLOCKING_OUTPUT_EDGE_SHIFT;
            node.auxiliary & !(CLOCKING_BLOCK_DEFAULT | CLOCKING_BLOCK_GLOBAL | edge_mask) == 0
        }
        // Parameter auxiliary metadata carries the frontend's override bit.
        (12, _, _) => node.auxiliary <= 1,
        // Subroutine qualifiers carry method and DPI-C metadata. Context and
        // DPI purity are meaningful only for imports.
        (16, _, _) => {
            let allowed = SUBROUTINE_STATIC
                | SUBROUTINE_VIRTUAL
                | SUBROUTINE_PURE
                | SUBROUTINE_FINAL
                | SUBROUTINE_CONSTRUCTOR
                | SUBROUTINE_DPI_IMPORT
                | SUBROUTINE_DPI_CONTEXT
                | SUBROUTINE_DPI_PURE;
            node.auxiliary & !allowed == 0
                && (node.auxiliary & (SUBROUTINE_DPI_CONTEXT | SUBROUTINE_DPI_PURE) == 0
                    || node.auxiliary & SUBROUTINE_DPI_IMPORT != 0)
        }
        // Argument qualifiers carry const-ref and ref-static bits.
        (17, _, _) => node.auxiliary <= 3,
        // Statement subkind 42 covers both `wait` and `wait_order`; the
        // auxiliary marker distinguishes the ordered form. Conditional and
        // case statements use the same scalar for their qualifier.
        (18, 42, _) => node.auxiliary <= 1,
        // Immediate assertions reserve two bits to preserve deferred/final
        // syntax until the simulator can either execute or reject it.
        (18, 61..=63, _) => node.auxiliary <= 3,
        // Concurrent assertion expressions use a small set of owned flags;
        // unknown flags would make the property shape ambiguous downstream.
        (28, 1..=13, _) => {
            let flags_valid = node.auxiliary
                & !(SEMANTIC_ASSERTION_REPETITION
                    | SEMANTIC_ASSERTION_RANGE
                    | SEMANTIC_ASSERTION_STRONG
                    | SEMANTIC_ASSERTION_ABORT_REJECT
                    | SEMANTIC_ASSERTION_ABORT_SYNC)
                == 0;
            let kind_valid = matches!(node.assertion_repetition_kind, 0..=3)
                && (node.assertion_range_max == SEMANTIC_ASSERTION_RANGE_UNBOUNDED
                    || node.assertion_range_max >= node.assertion_range_min)
                && ((node.auxiliary & SEMANTIC_ASSERTION_REPETITION != 0)
                    == (node.assertion_repetition_kind != 0));
            flags_valid && kind_valid
        }
        // Foreach uses the auxiliary field for the number of source iterator
        // slots so omitted trailing dimensions survive the owned snapshot.
        // Keep the count bounded independently of the later DB allocation.
        (18, 59, _) => node.auxiliary <= 4096,
        (18, 33 | 34, _) => node.auxiliary <= SEMANTIC_UNIQUE_PRIORITY_PRIORITY,
        // Class qualifiers and `new super` are repository-owned flags.
        (3, _, _) => node.auxiliary & !(CLASS_ABSTRACT | CLASS_FINAL | CLASS_INTERFACE) == 0,
        (19, 87, _) => node.auxiliary & !NEW_CLASS_SUPER == 0,
        // A call qualified with `super` must bind directly to its declaring
        // base implementation instead of participating in virtual dispatch.
        (22, 76, _) => node.auxiliary & !CALL_SUPER == 0,
        (19, 69, 40) => node.auxiliary == 0 || node.flags & 1 != 0,
        (19, 69, 41) => node.auxiliary > 0 || node.flags & 1 != 0,
        _ => node.auxiliary == 0,
    };
    if !valid {
        return Err(invalid_native(
            "semantic node has invalid kind-specific auxiliary metadata",
        ));
    }
    Ok(())
}

pub(super) fn decode_semantic_operation(raw: u32) -> Result<SemanticOperation, SlangError> {
    Ok(match raw {
        0 => SemanticOperation::None,
        1 => SemanticOperation::Plus,
        2 => SemanticOperation::Minus,
        3 => SemanticOperation::Multiply,
        4 => SemanticOperation::Divide,
        5 => SemanticOperation::Modulo,
        6 => SemanticOperation::Power,
        7 => SemanticOperation::BitNot,
        8 => SemanticOperation::BitAnd,
        9 => SemanticOperation::BitOr,
        10 => SemanticOperation::BitXor,
        11 => SemanticOperation::BitNand,
        12 => SemanticOperation::BitNor,
        13 => SemanticOperation::BitXnor,
        14 => SemanticOperation::LogicalNot,
        15 => SemanticOperation::LogicalAnd,
        16 => SemanticOperation::LogicalOr,
        17 => SemanticOperation::LogicalImplication,
        18 => SemanticOperation::LogicalEquivalence,
        19 => SemanticOperation::Equal,
        20 => SemanticOperation::NotEqual,
        21 => SemanticOperation::CaseEqual,
        22 => SemanticOperation::CaseNotEqual,
        23 => SemanticOperation::WildcardEqual,
        24 => SemanticOperation::WildcardNotEqual,
        25 => SemanticOperation::Greater,
        26 => SemanticOperation::GreaterEqual,
        27 => SemanticOperation::Less,
        28 => SemanticOperation::LessEqual,
        29 => SemanticOperation::ShiftLeft,
        30 => SemanticOperation::ShiftRight,
        31 => SemanticOperation::ArithmeticShiftLeft,
        32 => SemanticOperation::ArithmeticShiftRight,
        33 => SemanticOperation::PreIncrement,
        34 => SemanticOperation::PreDecrement,
        35 => SemanticOperation::PostIncrement,
        36 => SemanticOperation::PostDecrement,
        37 => SemanticOperation::Concat,
        38 => SemanticOperation::Replicate,
        39 => SemanticOperation::Conditional,
        40 => SemanticOperation::StreamLeft,
        41 => SemanticOperation::StreamRight,
        42 => SemanticOperation::Assign,
        43 => SemanticOperation::Inside,
        44 => SemanticOperation::AssignmentPattern,
        45 => SemanticOperation::MinTypMax,
        46 => SemanticOperation::MultiAssignmentPattern,
        47 => SemanticOperation::List,
        48 => SemanticOperation::AssertionAnd,
        49 => SemanticOperation::AssertionOr,
        50 => SemanticOperation::AssertionIntersect,
        51 => SemanticOperation::AssertionThroughout,
        52 => SemanticOperation::AssertionWithin,
        53 => SemanticOperation::AssertionIff,
        54 => SemanticOperation::AssertionUntil,
        55 => SemanticOperation::AssertionSUntil,
        56 => SemanticOperation::AssertionUntilWith,
        57 => SemanticOperation::AssertionSUntilWith,
        58 => SemanticOperation::AssertionImplies,
        59 => SemanticOperation::AssertionOverlappedImplies,
        60 => SemanticOperation::AssertionNonOverlappedImplies,
        61 => SemanticOperation::AssertionOverlappedFollowedBy,
        62 => SemanticOperation::AssertionNonOverlappedFollowedBy,
        63 => SemanticOperation::AssertionNot,
        64 => SemanticOperation::AssertionNextTime,
        65 => SemanticOperation::AssertionSNextTime,
        66 => SemanticOperation::AssertionAlways,
        67 => SemanticOperation::AssertionSAlways,
        68 => SemanticOperation::AssertionEventually,
        69 => SemanticOperation::AssertionSEventually,
        _ => return Err(invalid_native("semantic node has an unknown operation")),
    })
}

fn decode_time_scale(node: &RawSemanticNode) -> Result<Option<SemanticTimeScale>, SlangError> {
    let values = [
        node.time_unit,
        node.time_unit_magnitude,
        node.time_precision_unit,
        node.time_precision_magnitude,
    ];
    if values.iter().all(|value| *value == 0) {
        return Ok(None);
    }
    if values.contains(&0)
        || !matches!(node.time_unit_magnitude, 1 | 10 | 100)
        || !matches!(node.time_precision_magnitude, 1 | 10 | 100)
    {
        return Err(invalid_native("semantic node has an invalid time scale"));
    }
    let scale = SemanticTimeScale {
        unit: decode_time_unit(node.time_unit)?,
        magnitude: node.time_unit_magnitude,
        precision_unit: decode_time_unit(node.time_precision_unit)?,
        precision_magnitude: node.time_precision_magnitude,
    };
    if semantic_time_exponent(scale.precision_unit, scale.precision_magnitude)
        > semantic_time_exponent(scale.unit, scale.magnitude)
    {
        return Err(invalid_native(
            "semantic node time precision is coarser than its time unit",
        ));
    }
    Ok(Some(scale))
}

fn semantic_time_exponent(unit: SemanticTimeUnit, magnitude: u32) -> i32 {
    let base = match unit {
        SemanticTimeUnit::Seconds => 0,
        SemanticTimeUnit::Milliseconds => -3,
        SemanticTimeUnit::Microseconds => -6,
        SemanticTimeUnit::Nanoseconds => -9,
        SemanticTimeUnit::Picoseconds => -12,
        SemanticTimeUnit::Femtoseconds => -15,
    };
    base + match magnitude {
        1 => 0,
        10 => 1,
        100 => 2,
        _ => unreachable!("magnitude validated by decode_time_scale"),
    }
}

fn decode_drive_strength(value: u32) -> Result<SemanticDriveStrength, SlangError> {
    Ok(match value {
        0 => SemanticDriveStrength::Unspecified,
        1 => SemanticDriveStrength::Supply,
        2 => SemanticDriveStrength::Strong,
        3 => SemanticDriveStrength::Pull,
        4 => SemanticDriveStrength::Weak,
        5 => SemanticDriveStrength::HighZ,
        _ => {
            return Err(invalid_native(
                "semantic node has an unknown drive strength",
            ))
        }
    })
}

fn decode_time_unit(value: u32) -> Result<SemanticTimeUnit, SlangError> {
    Ok(match value {
        1 => SemanticTimeUnit::Seconds,
        2 => SemanticTimeUnit::Milliseconds,
        3 => SemanticTimeUnit::Microseconds,
        4 => SemanticTimeUnit::Nanoseconds,
        5 => SemanticTimeUnit::Picoseconds,
        6 => SemanticTimeUnit::Femtoseconds,
        _ => return Err(invalid_native("semantic node has an unknown time unit")),
    })
}
