//! Semantic node, edge and UDP record receivers.

use super::*;
use std::collections::HashMap;

/// Decode one semantic edge. Edges precede the nodes that own them, so the
/// target is checked against the announced node count.
pub(super) fn decode_semantic_edge(
    edge: &RawSemanticEdge,
    node_count: u64,
) -> Result<SemanticEdge, SlangError> {
    if edge.target_id >= node_count {
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
        38 => SemanticEdgeRole::ConditionPattern,
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
}

/// Tables a semantic node is validated against while it is received.
pub(super) struct SemanticNodeContext<'a> {
    pub(super) node_count: u64,
    pub(super) edges: &'a [SemanticEdge],
    pub(super) claimed_edges: &'a mut [bool],
    /// Reused per-node scratch set for the role/index uniqueness check.
    pub(super) edge_keys: &'a mut HashSet<(SemanticEdgeRole, u32)>,
    pub(super) files: &'a [File],
    pub(super) type_count: usize,
    pub(super) constant_count: usize,
}

/// Decode the semantic node at table position `index`, claiming its edge
/// window. Node IDs are dense arena indices.
pub(super) fn decode_semantic_node(
    node: &RawSemanticNode,
    index: u64,
    context: SemanticNodeContext<'_>,
) -> Result<SemanticNode, SlangError> {
    if node.id != index {
        return Err(invalid_native(
            "semantic node ids are not contiguous arena indices",
        ));
    }
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
    if parent_id.is_some_and(|id| id >= context.node_count)
        || target_id.is_some_and(|id| id >= context.node_count)
    {
        return Err(invalid_native(
            "semantic node refers to an unknown semantic node",
        ));
    }
    let type_id = (node.type_id != INVALID_ID).then_some(node.type_id);
    if type_id.is_some_and(|id| usize::try_from(id).map_or(true, |id| id >= context.type_count)) {
        return Err(invalid_native("semantic node type does not exist"));
    }
    let constant_id = (node.constant_id != INVALID_ID).then_some(node.constant_id);
    if constant_id
        .is_some_and(|id| usize::try_from(id).map_or(true, |id| id >= context.constant_count))
    {
        return Err(invalid_native("semantic node constant does not exist"));
    }
    let window = checked_window(
        node.edge_start,
        node.edge_count,
        context.edges.len(),
        "semantic node edges",
    )?;
    context.edge_keys.clear();
    for index in window {
        if context.claimed_edges[index] {
            return Err(invalid_native("semantic node edge windows overlap"));
        }
        context.claimed_edges[index] = true;
        let edge = &context.edges[index];
        if !context.edge_keys.insert((edge.role, edge.index)) {
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
        is_const_ref: kind == SemanticKind::Argument && node.auxiliary & ARGUMENT_CONST_REF != 0,
        is_ref_static: kind == SemanticKind::Argument && node.auxiliary & ARGUMENT_REF_STATIC != 0,
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
        // SAFETY: stream records and their strings are valid for the
        // callback that delivered them.
        name: unsafe { copy_string(node.name, "semantic node name")? },
        // SAFETY: stream records and their strings are valid for the
        // callback that delivered them.
        detail: unsafe { copy_string(node.detail, "semantic node detail")? },
        // SAFETY: stream records and their strings are valid for the
        // callback that delivered them.
        definition_name: unsafe {
            copy_string(node.definition_name, "semantic node definition name")?
        },
        range: decode_range(node.range, context.files)?,
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
        19 => matches!(subkind, 0 | 64..=78 | 80..=92),
        25 => matches!(subkind, 0 | 194 | SEMANTIC_SCOPE_CLOCKING_BLOCK),
        26 => matches!(subkind, 0 | 112..=118),
        28 => matches!(subkind, 0..=13),
        20..=22 => matches!(subkind, 0 | 76),
        9 => matches!(
            subkind,
            0 | 229
                | SEMANTIC_VARIABLE_CLOCKING
                | SEMANTIC_VARIABLE_ASSERTION_LOCAL
                | SEMANTIC_VARIABLE_PATTERN_BINDING
        ),
        2 | 3 | 5..=7 | 10..=12 | 16 | 17 | 23 | 24 | 27 => subkind == 0,
        255 => matches!(
            subkind,
            0 | SEMANTIC_PATTERN_INVALID
                | SEMANTIC_PATTERN_WILDCARD
                | SEMANTIC_PATTERN_CONSTANT
                | SEMANTIC_PATTERN_VARIABLE
                | SEMANTIC_PATTERN_TAGGED
                | SEMANTIC_PATTERN_STRUCTURE
        ),
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
        // Module definitions and elaborated instances carry the resolved
        // `unconnected_drive` value for omitted input links.
        (1 | 24, _, _) => {
            let allowed = UNCONNECTED_PULL0 | UNCONNECTED_PULL1;
            node.auxiliary & !allowed == 0 && node.auxiliary.count_ones() <= 1
        }
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
        (18, 33 | 34 | SEMANTIC_STMT_PATTERN_CASE, _) => {
            node.auxiliary <= SEMANTIC_UNIQUE_PRIORITY_PRIORITY
        }
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

/// A UDP row copied on receipt. Rows precede the tables whose windows own
/// them; the owning table checks the width and moves the text out.
pub(super) struct PendingUdpRow {
    inputs: String,
    output: u8,
}

/// Validate and copy one combinational UDP row.
pub(super) fn decode_udp_row(raw_row: &RawUdpRow) -> Result<PendingUdpRow, SlangError> {
    if raw_row.reserved != 0 || raw_row.flags != 0 || raw_row.state != 0 {
        return Err(invalid_native(
            "combinational UDP row contains state or edge metadata",
        ));
    }
    if raw_row.output > u8::MAX as u32 || !matches!(raw_row.output as u8, b'0' | b'1' | b'x') {
        return Err(invalid_native(
            "combinational UDP row has an invalid output",
        ));
    }
    Ok(PendingUdpRow {
        // SAFETY: stream records and their strings are valid for the callback
        // that delivered them.
        inputs: unsafe { copy_string(raw_row.inputs, "UDP row inputs")? },
        output: raw_row.output as u8,
    })
}

/// Receiver state shared by every UDP table of one stream.
pub(super) struct UdpTableContext<'a> {
    pub(super) rows: &'a mut [PendingUdpRow],
    pub(super) claimed_rows: &'a mut [bool],
    pub(super) table_ids: &'a mut HashSet<u64>,
    pub(super) overlap: &'a mut UdpOverlapValidator,
    pub(super) semantic_nodes: &'a [SemanticNode],
}

/// Decode one UDP table into owned rows and check the frontend-independent
/// consistency rules needed by downstream lowering. Slang has already
/// diagnosed conflicting rows, but retaining this check at the ABI boundary
/// prevents a malformed or stale native export from making the simulator
/// depend on borrowed frontend state.
pub(super) fn decode_udp_table(
    raw_table: &RawUdpTable,
    context: UdpTableContext<'_>,
) -> Result<UdpTable, SlangError> {
    if raw_table.reserved != 0 || !context.table_ids.insert(raw_table.primitive_id) {
        return Err(invalid_native(
            "UDP table has invalid or duplicate metadata",
        ));
    }
    let declaration_index = usize::try_from(raw_table.primitive_id)
        .map_err(|_| invalid_native("UDP table primitive ID does not fit usize"))?;
    let declaration = context
        .semantic_nodes
        .get(declaration_index)
        .filter(|node| node.id == raw_table.primitive_id)
        .ok_or_else(|| invalid_native("UDP table refers to an unknown primitive"))?;
    if declaration.kind != SemanticKind::Primitive
        || !declaration.is_primitive_declaration
        || declaration.subkind != 227
    {
        return Err(invalid_native(
            "UDP table is not attached to a combinational primitive declaration",
        ));
    }
    let input_count = usize::try_from(raw_table.input_count)
        .map_err(|_| invalid_native("UDP input count does not fit usize"))?;
    let window = checked_window(
        raw_table.row_start,
        raw_table.row_count,
        context.rows.len(),
        "UDP table rows",
    )?;
    if window.clone().any(|index| context.claimed_rows[index]) {
        return Err(invalid_native("UDP table row windows overlap"));
    }
    let mut rows = Vec::with_capacity(window.len());
    for index in window {
        context.claimed_rows[index] = true;
        // Each row is claimed by exactly one window, so its text moves.
        let pending = &mut context.rows[index];
        let inputs = std::mem::take(&mut pending.inputs);
        if inputs.chars().count() != input_count
            || inputs
                .bytes()
                .any(|symbol| !matches!(symbol, b'0' | b'1' | b'x' | b'?' | b'b'))
        {
            return Err(invalid_native(
                "combinational UDP row has invalid input symbols or width",
            ));
        }
        rows.push(UdpRow {
            inputs,
            state: None,
            output: pending.output,
            edge_sensitive: false,
        });
    }
    context.overlap.validate_rows(&rows)?;
    // SAFETY: stream records and their strings are valid for the callback
    // that delivered them.
    let name = unsafe { copy_string(raw_table.name, "UDP table name")? };
    if name != declaration.name {
        return Err(invalid_native(
            "UDP table name does not match its primitive declaration",
        ));
    }
    Ok(UdpTable {
        primitive_id: raw_table.primitive_id,
        name,
        input_count: raw_table.input_count,
        rows,
    })
}

/// Keep overlap validation small enough for the concrete assignment index to
/// have a predictable process-memory cost. This is deliberately below the
/// native semantic-edge ceiling: a 16-million-entry `HashMap<Vec<u8>, u8>`
/// can consume hundreds of MiB once hash buckets and per-key allocations are
/// included, even though the native row count is within its hard limit.
const MAX_UDP_OVERLAP_ASSIGNMENTS: u64 = 65_536;
const MAX_UDP_OVERLAP_INDEX_BYTES: u64 = 8 * 1024 * 1024;

/// Validate row overlap by indexing concrete input assignments.
///
/// A combinational UDP row describes a Cartesian product over the three
/// four-state input values represented by the table (`0`, `1`, and `x`).
/// Expanding each row into those assignments makes overlap validation a
/// linear hash-table insertion pass rather than a quadratic row pair scan.
/// The validator owns one assignment index, reuses it for each table, and
/// charges one global expansion budget across the complete snapshot. Its
/// fixed 65,536-assignment cap bounds the retained hash buckets and key
/// allocations even when a caller supplies the native 16-million edge limit;
/// an additional 8 MiB key-byte budget covers wide rows whose individual
/// assignments would otherwise make each `Vec<u8>` expensive.
pub(super) struct UdpOverlapValidator {
    assignments: HashMap<Vec<u8>, u8>,
    generated: u64,
    index_bytes: u64,
    max_assignments: u64,
}

impl UdpOverlapValidator {
    pub(super) fn new(requested_max: u64) -> Self {
        Self {
            assignments: HashMap::new(),
            generated: 0,
            index_bytes: 0,
            max_assignments: requested_max.min(MAX_UDP_OVERLAP_ASSIGNMENTS),
        }
    }

    fn validate_rows(&mut self, rows: &[UdpRow]) -> Result<(), SlangError> {
        // Tables are independent overlap domains. Retain the allocated bucket
        // storage for reuse, while keeping entries from one table out of the
        // next table's conflict checks.
        self.assignments.clear();

        for row in rows {
            // Check the Cartesian-product size before allocating the masks or
            // assignment index entries. A wide wildcard row therefore fails
            // at the resource boundary without starting a large expansion.
            let mut row_assignments = 1_u64;
            let remaining_assignments = self
                .max_assignments
                .checked_sub(self.generated)
                .ok_or_else(|| {
                    limit_exceeded("UDP overlap validation assignment count overflowed")
                })?;
            for symbol in row.inputs.bytes() {
                let mask = udp_symbol_mask(symbol);
                if mask == 0 {
                    return Err(invalid_native(
                        "combinational UDP row has invalid input symbols or width",
                    ));
                }
                row_assignments = row_assignments
                    .checked_mul(u64::from(mask.count_ones()))
                    .ok_or_else(|| {
                        limit_exceeded("UDP overlap validation assignment count overflowed")
                    })?;
                if row_assignments > remaining_assignments {
                    return Err(limit_exceeded(
                        "UDP overlap validation exceeds its bounded assignment capacity",
                    ));
                }
            }
            if row_assignments > remaining_assignments {
                return Err(limit_exceeded(
                    "UDP overlap validation exceeds its bounded assignment capacity",
                ));
            }
            self.generated = self.generated.checked_add(row_assignments).ok_or_else(|| {
                limit_exceeded("UDP overlap validation assignment count overflowed")
            })?;
            let row_index_bytes = row_assignments
                .checked_mul(u64::try_from(row.inputs.len()).map_err(|_| {
                    limit_exceeded("UDP overlap validation row width does not fit u64")
                })?)
                .ok_or_else(|| {
                    limit_exceeded("UDP overlap validation index byte count overflowed")
                })?;
            if row_index_bytes > MAX_UDP_OVERLAP_INDEX_BYTES
                || row_index_bytes > MAX_UDP_OVERLAP_INDEX_BYTES - self.index_bytes
            {
                return Err(limit_exceeded(
                    "UDP overlap validation exceeds its bounded index memory capacity",
                ));
            }
            self.index_bytes = self
                .index_bytes
                .checked_add(row_index_bytes)
                .ok_or_else(|| {
                    limit_exceeded("UDP overlap validation index byte count overflowed")
                })?;

            let masks: Vec<u8> = row.inputs.bytes().map(udp_symbol_mask).collect();
            let mut assignment = masks
                .iter()
                .map(|mask| udp_first_value(*mask))
                .collect::<Vec<_>>();
            loop {
                match self.assignments.entry(assignment.clone()) {
                    std::collections::hash_map::Entry::Vacant(entry) => {
                        entry.insert(row.output);
                    }
                    std::collections::hash_map::Entry::Occupied(entry)
                        if *entry.get() != row.output =>
                    {
                        return Err(invalid_native(
                            "combinational UDP table has overlapping rows with different outputs",
                        ));
                    }
                    std::collections::hash_map::Entry::Occupied(_) => {}
                }

                let mut advanced = false;
                for position in (0..masks.len()).rev() {
                    if let Some(value) = udp_next_value(masks[position], assignment[position]) {
                        assignment[position] = value;
                        for reset in (position + 1)..masks.len() {
                            assignment[reset] = udp_first_value(masks[reset]);
                        }
                        advanced = true;
                        break;
                    }
                }
                if !advanced {
                    break;
                }
            }
        }
        Ok(())
    }
}

fn udp_symbol_mask(symbol: u8) -> u8 {
    match symbol {
        b'0' => 1,
        b'1' => 2,
        b'x' => 4,
        b'b' => 3,
        b'?' => 7,
        _ => 0,
    }
}

fn udp_first_value(mask: u8) -> u8 {
    (0..=2)
        .find(|value| mask & (1 << value) != 0)
        .expect("validated UDP symbol has at least one value")
}

fn udp_next_value(mask: u8, current: u8) -> Option<u8> {
    ((current + 1)..=2).find(|value| mask & (1 << value) != 0)
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
