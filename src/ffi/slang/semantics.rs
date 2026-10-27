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
        39 => SemanticEdgeRole::DefaultDisable,
        40 => SemanticEdgeRole::Weight,
        41 => SemanticEdgeRole::RandJoin,
        42 => SemanticEdgeRole::Typedef,
        _ => return Err(invalid_native("semantic edge has an unknown role")),
    };
    Ok(SemanticEdge {
        role,
        index: edge.index,
        target_id: edge.target_id,
        sequence_delay: (edge.sequence_delay_valid != 0).then(|| {
            Box::new(SemanticSequenceRange {
                min: edge.sequence_delay_min,
                max: (edge.sequence_delay_max != u32::MAX).then_some(edge.sequence_delay_max),
            })
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
    pub(super) texts: &'a mut TextInterner,
}

/// Interns node text for one stream; see [`SemanticText`].
#[derive(Default)]
pub(super) struct TextInterner {
    texts: HashSet<SemanticText>,
}

impl TextInterner {
    /// Validate `raw` as UTF-8 and return its shared spelling, allocating
    /// only for a spelling not seen before in this stream.
    ///
    /// # Safety
    /// For nonzero length, `raw.data` must reference that many initialized
    /// bytes that remain valid for this call.
    pub(super) unsafe fn intern(
        &mut self,
        raw: RawString,
        label: &str,
    ) -> Result<SemanticText, SlangError> {
        // SAFETY: the caller guarantees the borrowed bytes; foreign_slice
        // validates null and representable length.
        let bytes = unsafe { foreign_slice(raw.data, raw.len, label)? };
        let text = str::from_utf8(bytes)
            .map_err(|_| invalid_native(format!("{label} is not valid UTF-8")))?;
        if let Some(shared) = self.texts.get(text) {
            return Ok(shared.clone());
        }
        let shared = SemanticText::from(text);
        self.texts.insert(shared.clone());
        Ok(shared)
    }
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
    // Clearing a set costs its capacity, so after one wide node (a netlist
    // module's instance list) clearing it for every later node made decoding
    // nodes x widest window. Narrower nodes start from a set of their size.
    if context.edge_keys.capacity() > RETAINED_EDGE_KEYS.max(window.len().saturating_mul(4)) {
        *context.edge_keys = std::collections::HashSet::with_capacity(window.len());
    } else {
        context.edge_keys.clear();
    }
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
        parent_id: CompactId::pack(parent_id, "parent")?,
        kind,
        subkind: node.subkind,
        operation: decode_semantic_operation(node.operation)?,
        flags: SemanticFlags::from_raw(node.flags, kind == SemanticKind::Argument, node.auxiliary),
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
        name: unsafe { context.texts.intern(node.name, "semantic node name")? },
        // SAFETY: as above.
        detail: unsafe { context.texts.intern(node.detail, "semantic node detail")? },
        // SAFETY: as above.
        definition_name: unsafe {
            context
                .texts
                .intern(node.definition_name, "semantic node definition name")?
        },
        range: decode_range(node.range, context.files)?,
        type_id: CompactId::pack(type_id, "type")?,
        constant_id: CompactId::pack(constant_id, "constant")?,
        target_id: CompactId::pack(target_id, "target")?,
        edge_start: u32::try_from(node.edge_start)
            .map_err(|_| invalid_native("semantic node edge window exceeds the compact range"))?,
        edge_count: u32::try_from(node.edge_count)
            .map_err(|_| invalid_native("semantic node edge window exceeds the compact range"))?,
        time_scale: decode_time_scale(node)?.map(Box::new),
        strength0: decode_drive_strength(node.strength0)?,
        strength1: decode_drive_strength(node.strength1)?,
        auxiliary: node.auxiliary,
        assertion: assertion_metadata(node),
    })
}

/// Edge-key capacity a node's duplicate check keeps for the next node.
const RETAINED_EDGE_KEYS: usize = 64;

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
        29 => SemanticKind::ConstraintBlock,
        30 => SemanticKind::Constraint,
        31 => SemanticKind::Coverage,
        32 => SemanticKind::Production,
        33 => SemanticKind::Specify,
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
        18 => matches!(subkind, 0 | 32..=71 | SEMANTIC_STMT_PATTERN_CASE),
        19 => matches!(subkind, 0 | 64..=78 | 80..=95),
        25 => matches!(
            subkind,
            0 | 194 | SEMANTIC_SCOPE_CLOCKING_BLOCK | SEMANTIC_SCOPE_ANONYMOUS_PROGRAM
        ),
        26 => matches!(subkind, 0 | 112..=119),
        31 => matches!(
            subkind,
            super::SEMANTIC_COVERAGE_GROUP..=super::SEMANTIC_COVERAGE_SELECT
        ),
        32 => matches!(
            subkind,
            super::SEMANTIC_PRODUCTION_DECLARATION..=super::SEMANTIC_PRODUCTION_CASE
        ),
        33 => matches!(
            subkind,
            super::SEMANTIC_SPECIFY_BLOCK..=super::SEMANTIC_SPECIFY_TIMING_CHECK
        ),
        12 => matches!(
            subkind,
            0 | super::SEMANTIC_PARAMETER_SPECPARAM | super::SEMANTIC_PARAMETER_PATHPULSE
        ),
        28 => matches!(subkind, 0..=13),
        30 => matches!(
            subkind,
            super::SEMANTIC_CONSTRAINT_LIST..=super::SEMANTIC_CONSTRAINT_FOREACH
        ),
        20..=22 => matches!(subkind, 0 | 76),
        9 => matches!(
            subkind,
            0 | 229
                | SEMANTIC_VARIABLE_CLOCKING
                | SEMANTIC_VARIABLE_ASSERTION_LOCAL
                | SEMANTIC_VARIABLE_PATTERN_BINDING
        ),
        2 | 3 | 5..=7 | 10 | 11 | 16 | 17 | 23 | 24 | 27 | 29 => subkind == 0,
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
        // The low byte is the lifetime; `const` and one of `rand`/`randc`
        // are the qualifiers above it.
        (9 | 11, _, _) => {
            let qualifiers = super::VARIABLE_CONST | super::VARIABLE_RAND | super::VARIABLE_RANDC;
            matches!(node.auxiliary & super::VARIABLE_LIFETIME_MASK, 0..=2)
                && node.auxiliary & !(super::VARIABLE_LIFETIME_MASK | qualifiers) == 0
                && node.auxiliary & (super::VARIABLE_RAND | super::VARIABLE_RANDC)
                    != super::VARIABLE_RAND | super::VARIABLE_RANDC
        }
        (29, _, _) => {
            let allowed = super::CONSTRAINT_BLOCK_STATIC
                | super::CONSTRAINT_BLOCK_PURE
                | super::CONSTRAINT_BLOCK_EXTERN
                | super::CONSTRAINT_BLOCK_EXPLICIT_EXTERN
                | super::CONSTRAINT_BLOCK_INITIAL
                | super::CONSTRAINT_BLOCK_EXTENDS
                | super::CONSTRAINT_BLOCK_FINAL;
            node.auxiliary & !allowed == 0
        }
        (30, super::SEMANTIC_CONSTRAINT_EXPRESSION, _) => {
            node.auxiliary & !super::CONSTRAINT_SOFT == 0
        }
        (25, SEMANTIC_SCOPE_CLOCKING_BLOCK, _) => {
            let edge_mask = CLOCKING_EDGE_MASK << CLOCKING_INPUT_EDGE_SHIFT
                | CLOCKING_EDGE_MASK << CLOCKING_OUTPUT_EDGE_SHIFT;
            node.auxiliary & !(CLOCKING_BLOCK_DEFAULT | CLOCKING_BLOCK_GLOBAL | edge_mask) == 0
        }
        // A `PATHPULSE$` specparam carries its error-limit constant id plus
        // one; the Db checks that the constant exists.
        (12, super::SEMANTIC_PARAMETER_PATHPULSE, _) => true,
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
                | SUBROUTINE_DPI_PURE
                | SUBROUTINE_DPI_EXPORT;
            node.auxiliary & !allowed == 0
                && (node.auxiliary & (SUBROUTINE_DPI_CONTEXT | SUBROUTINE_DPI_PURE) == 0
                    || node.auxiliary & SUBROUTINE_DPI_IMPORT != 0)
        }
        // Argument qualifiers carry const-ref, ref-static and
        // coverage-sample bits.
        (17, _, _) => {
            let allowed = super::ARGUMENT_CONST_REF
                | super::ARGUMENT_REF_STATIC
                | super::ARGUMENT_COVERAGE_SAMPLE;
            node.auxiliary & !allowed == 0
        }
        (31, super::SEMANTIC_COVERAGE_GROUP, _) => {
            let allowed = super::COVERGROUP_EMBEDDED
                | super::COVERGROUP_WITH_SAMPLE
                | super::COVERGROUP_BLOCK_EVENT
                | super::COVERGROUP_EXTENDS;
            node.auxiliary & !allowed == 0
        }
        (31, super::SEMANTIC_COVERAGE_BIN, _) => {
            let allowed = super::COVERAGE_BIN_KIND_MASK
                | super::COVERAGE_BIN_ARRAY
                | super::COVERAGE_BIN_WILDCARD
                | super::COVERAGE_BIN_DEFAULT
                | super::COVERAGE_BIN_DEFAULT_SEQUENCE
                | super::COVERAGE_BIN_TRANSITION
                | super::COVERAGE_BIN_WITH
                | super::COVERAGE_BIN_SET
                | super::COVERAGE_BIN_SELECT;
            node.auxiliary & !allowed == 0 && node.auxiliary & super::COVERAGE_BIN_KIND_MASK != 3
        }
        // A production marks its non-input formals; a rule its rand join.
        (32, super::SEMANTIC_PRODUCTION_DECLARATION, _) => true,
        (32, super::SEMANTIC_PRODUCTION_RULE, _) => {
            node.auxiliary & !super::PRODUCTION_RULE_RAND_JOIN == 0
        }
        (32, _, _) => node.auxiliary == 0,
        // A module path's connection, polarity, edge and state-dependence
        // fields; a polarity field never holds 3.
        (33, super::SEMANTIC_SPECIFY_PATH, _) => {
            let field = |shift: u32| (node.auxiliary >> shift) & super::SPECIFY_PATH_FIELD_MASK;
            node.auxiliary
                & !(super::SPECIFY_PATH_FULL
                    | super::SPECIFY_PATH_FIELD_MASK << super::SPECIFY_PATH_POLARITY_SHIFT
                    | super::SPECIFY_PATH_FIELD_MASK << super::SPECIFY_PATH_EDGE_SHIFT
                    | super::SPECIFY_PATH_FIELD_MASK << super::SPECIFY_PATH_EDGE_POLARITY_SHIFT
                    | super::SPECIFY_PATH_STATE_DEPENDENT)
                == 0
                && field(super::SPECIFY_PATH_POLARITY_SHIFT) != 3
                && field(super::SPECIFY_PATH_EDGE_POLARITY_SHIFT) != 3
        }
        (33, super::SEMANTIC_SPECIFY_PULSE_STYLE, _) => (super::SPECIFY_PULSE_ONEVENT
            ..=super::SPECIFY_PULSE_NOSHOWCANCELLED)
            .contains(&node.auxiliary),
        // Zero is a timing check the frontend could not classify. Two event
        // edge masks, sixteen empty-argument bits and an argument count
        // follow the kind.
        (33, super::SEMANTIC_SPECIFY_TIMING_CHECK, _) => {
            let event =
                |k: u32| super::SPECIFY_EDGE_MASK << (super::SPECIFY_CHECK_EVENT_BITS * (k + 1));
            let allowed = super::SPECIFY_CHECK_KIND_MASK
                | event(0)
                | event(1)
                | 0xffff << super::SPECIFY_CHECK_EMPTY_SHIFT
                | 0xff << super::SPECIFY_CHECK_COUNT_SHIFT;
            node.auxiliary & !allowed == 0
                && node.auxiliary & super::SPECIFY_CHECK_KIND_MASK <= super::SPECIFY_CHECK_NOCHANGE
        }
        (33, _, _) => node.auxiliary == 0,
        (31, super::SEMANTIC_COVERAGE_CROSS, _) => {
            node.auxiliary & !super::COVER_CROSS_OF_CROSS == 0
        }
        (31, super::SEMANTIC_COVERAGE_TRANS_RANGE, _) => node.auxiliary <= super::COVER_REPEAT_GOTO,
        (31, super::SEMANTIC_COVERAGE_SELECT, _) => {
            (super::COVER_SELECT_CONDITION..=super::COVER_SELECT_OTHER).contains(&node.auxiliary)
        }
        // Bit i marks block event i as a `begin` event.
        (26, super::SEMANTIC_TIMING_BLOCK_EVENT_LIST, _) => true,
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
                    | SEMANTIC_ASSERTION_ABORT_SYNC
                    | SEMANTIC_ASSERTION_PARENTHESIZED)
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
        (19, super::SEMANTIC_EXPR_DIST, _) => node.auxiliary & !super::DIST_DEFAULT == 0,
        // A conversion marks a `const'` cast.
        (19, 72, _) => node.auxiliary & !super::CONVERSION_CONST == 0,
        // A foreach constraint counts its loop-variable slots like the
        // statement form.
        (30, super::SEMANTIC_CONSTRAINT_FOREACH, _) => node.auxiliary <= 4096,
        // A call qualified with `super` must bind directly to its declaring
        // base implementation instead of participating in virtual dispatch.
        // Randomize calls also record whether they are the class method or
        // the scope function; a call is at most one of the two.
        (20..=22, 76, _) => {
            let randomize = super::CALL_CLASS_RANDOMIZE | super::CALL_SCOPE_RANDOMIZE;
            node.auxiliary & !(CALL_SUPER | randomize) == 0
                && (node.auxiliary & randomize).count_ones() <= 1
        }
        // A primitive instance array element keeps its element count (bits
        // 32-63) and its least-significant-first slice (bits 0-31).
        (15, _, _) if node.flags & (1 << 25) != 0 => {
            let (count, slice) = (node.auxiliary >> 32, node.auxiliary & u64::from(u32::MAX));
            node.auxiliary == 0 || slice < count
        }
        // A typedef (an unsupported-kind node with detail `TypeAlias`) marks
        // that it renames a built-in type (IEEE 1800-2009 37.23 detail 6).
        (255, _, _) => node.auxiliary & !super::TYPEDEF_BUILTIN == 0,
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
        70 => SemanticOperation::OverloadUpdate,
        71 => SemanticOperation::OverloadPostUpdate,
        _ => return Err(invalid_native("semantic node has an unknown operation")),
    })
}

/// A UDP row copied on receipt. Rows precede the tables whose windows own
/// them; the owning table checks the width and moves the text out.
pub(super) struct PendingUdpRow {
    inputs: String,
    state: Option<u8>,
    output: u8,
    edge_sensitive: bool,
    range: Option<SourceRange>,
}

/// Copy one UDP row, checking only its record structure. Symbol legality is
/// a frontend rule: Slang keeps rows whose symbols it diagnosed, and failing
/// here would replace those located diagnostics with an unlocated startup
/// error, so consumers check symbols after the frontend has reported.
pub(super) fn decode_udp_row(
    raw_row: &RawUdpRow,
    files: &[File],
) -> Result<PendingUdpRow, SlangError> {
    const EDGE_SENSITIVE: u32 = 1;
    if raw_row.reserved != 0
        || raw_row.flags & !EDGE_SENSITIVE != 0
        || raw_row.state > u32::from(u8::MAX)
        || raw_row.output == 0
        || raw_row.output > u32::from(u8::MAX)
    {
        return Err(invalid_native("UDP row has invalid metadata"));
    }
    Ok(PendingUdpRow {
        // SAFETY: stream records and their strings are valid for the callback
        // that delivered them.
        inputs: unsafe { copy_string(raw_row.inputs, "UDP row inputs")? },
        state: (raw_row.state != 0).then_some(raw_row.state as u8),
        output: raw_row.output as u8,
        edge_sensitive: raw_row.flags & EDGE_SENSITIVE != 0,
        range: decode_range(raw_row.range, files)?,
    })
}

/// Number of input fields in Slang's row normalization, where `(vw)` is one
/// field and every other character is one field; `None` for unbalanced or
/// non-two-symbol transitions, which Slang never keeps.
fn udp_field_count(inputs: &str) -> Option<usize> {
    let bytes = inputs.as_bytes();
    let (mut count, mut index) = (0, 0);
    while index < bytes.len() {
        match bytes[index] {
            b'(' if bytes.get(index + 3) == Some(&b')') => index += 4,
            b'(' | b')' => return None,
            _ => index += 1,
        }
        count += 1;
    }
    Some(count)
}

/// Receiver state shared by every UDP table of one stream.
pub(super) struct UdpTableContext<'a> {
    pub(super) rows: &'a mut [PendingUdpRow],
    pub(super) claimed_rows: &'a mut [bool],
    pub(super) table_ids: &'a mut HashSet<u64>,
    pub(super) overlap: &'a mut UdpOverlapValidator,
    pub(super) semantic_nodes: &'a [SemanticNode],
}

/// Semantic subkinds of sequential and combinational UDP declarations.
const UDP_SEQUENTIAL_SUBKIND: u32 = 226;
const UDP_COMBINATIONAL_SUBKIND: u32 = 227;

/// Decode one UDP table into owned rows and check the frontend-independent
/// structure needed by downstream consumers: the declaration identity, the
/// row windows and each row's field count. A well-formed combinational table
/// also keeps the bounded overlap check at the ABI boundary, so a malformed
/// or stale native export cannot make the simulator depend on borrowed
/// frontend state; sequential overlap (including `-` against explicit
/// outputs) is left to Slang's `UdpDupDiffOutput`.
pub(super) fn decode_udp_table(
    raw_table: &RawUdpTable,
    context: UdpTableContext<'_>,
) -> Result<UdpTable, SlangError> {
    if !context.table_ids.insert(raw_table.primitive_id) {
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
    let sequential = match declaration.subkind {
        UDP_SEQUENTIAL_SUBKIND => true,
        UDP_COMBINATIONAL_SUBKIND => false,
        _ => {
            return Err(invalid_native(
                "UDP table is not attached to a user-defined primitive declaration",
            ))
        }
    };
    if declaration.kind != SemanticKind::Primitive || !declaration.is_primitive_declaration() {
        return Err(invalid_native(
            "UDP table is not attached to a user-defined primitive declaration",
        ));
    }
    let initial = match raw_table.initial_value {
        0 => None,
        value if sequential && b"01x".map(u32::from).contains(&value) => Some(value as u8),
        _ => return Err(invalid_native("UDP table has an invalid initial value")),
    };
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
        if udp_field_count(&inputs) != Some(input_count) {
            return Err(invalid_native(
                "UDP row has the wrong number of input fields",
            ));
        }
        rows.push(UdpRow {
            inputs,
            state: pending.state,
            output: pending.output,
            edge_sensitive: pending.edge_sensitive,
            range: pending.range,
        });
    }
    let well_formed_combinational = !sequential
        && rows.iter().all(|row| {
            row.state.is_none()
                && !row.edge_sensitive
                && matches!(row.output, b'0' | b'1' | b'x')
                && row
                    .inputs
                    .bytes()
                    .all(|symbol| udp_symbol_mask(symbol) != 0)
        });
    if well_formed_combinational {
        context.overlap.validate_rows(&rows)?;
    }
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
        sequential,
        initial,
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

/// Assertion metadata, kept only when it differs from the defaults every
/// other node reads back (`0`, unbounded and `0`).
fn assertion_metadata(node: &RawSemanticNode) -> Option<Box<AssertionMetadata>> {
    let meta = AssertionMetadata {
        range_min: node.assertion_range_min,
        range_max: (node.kind == 28 && node.assertion_range_max != u32::MAX)
            .then_some(node.assertion_range_max),
        repetition_kind: node.assertion_repetition_kind,
    };
    let default = AssertionMetadata {
        range_min: 0,
        range_max: None,
        repetition_kind: 0,
    };
    (meta != default).then(|| Box::new(meta))
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
