use super::stream::{receive, SINK_ABORT, SINK_CONTINUE};
use super::*;
use std::ffi::c_void;

/// Raw tables delivered through the real sink trampolines by [`stream`].
#[derive(Default)]
struct Tables {
    flags: Option<u32>,
    files: Vec<RawFile>,
    related: Vec<RawRelatedDiagnostic>,
    diagnostics: Vec<RawDiagnostic>,
    value_words: Vec<u64>,
    constants: Vec<RawConstant>,
    type_ranges: Vec<RawTypeRange>,
    type_members: Vec<RawTypeMember>,
    types: Vec<RawType>,
    parameters: Vec<RawParameter>,
    instances: Vec<RawInstance>,
    semantic_edges: Vec<RawSemanticEdge>,
    semantic_nodes: Vec<RawSemanticNode>,
    udp_rows: Vec<RawUdpRow>,
    udp_tables: Vec<RawUdpTable>,
    lexical_tokens: Vec<RawLexicalToken>,
    source_libraries: Vec<RawSourceLibrary>,
    line_directives: Vec<RawLineDirective>,
    edition_findings: Vec<RawEditionFinding>,
}

impl Tables {
    fn header(&self) -> RawStreamHeader {
        RawStreamHeader {
            abi_version: ABI_VERSION,
            flags: self.flags.unwrap_or(SNAPSHOT_EDITION_SYSTEMVERILOG_2009),
            file_count: self.files.len() as u64,
            related_diagnostic_count: self.related.len() as u64,
            diagnostic_count: self.diagnostics.len() as u64,
            value_word_count: self.value_words.len() as u64,
            constant_count: self.constants.len() as u64,
            type_range_count: self.type_ranges.len() as u64,
            type_member_count: self.type_members.len() as u64,
            type_count: self.types.len() as u64,
            parameter_count: self.parameters.len() as u64,
            instance_count: self.instances.len() as u64,
            semantic_edge_count: self.semantic_edges.len() as u64,
            semantic_node_count: self.semantic_nodes.len() as u64,
            udp_row_count: self.udp_rows.len() as u64,
            udp_table_count: self.udp_tables.len() as u64,
            lexical_token_count: self.lexical_tokens.len() as u64,
            source_library_count: self.source_libraries.len() as u64,
            line_directive_count: self.line_directives.len() as u64,
            edition_finding_count: self.edition_findings.len() as u64,
        }
    }
}

/// Deliver `records` in batches of at most `batch`, as the bridge does.
fn deliver<T>(callback: RawBatch<T>, context: *mut c_void, records: &[T], batch: usize) -> u32 {
    for chunk in records.chunks(batch) {
        // SAFETY: `context` is the sink's builder and `chunk` is a live,
        // aligned, initialized batch whose strings outlive the call.
        let status = unsafe { callback(context, chunk.as_ptr(), chunk.len() as u64) };
        if status != SINK_CONTINUE {
            return status;
        }
    }
    SINK_CONTINUE
}

/// Drive a complete stream with `header` through `sink`, stopping at the
/// first abort.
fn drive(sink: &RawSink, header: &RawStreamHeader, tables: &Tables, batch: usize) -> u32 {
    let context = sink.context;
    // SAFETY: `context` is the sink's builder; the header outlives the call.
    let status = unsafe { (sink.begin)(context, header) };
    if status != SINK_CONTINUE {
        return status;
    }
    macro_rules! table {
        ($($callback:ident: $field:ident),*) => {
            $(
                let status = deliver(sink.$callback, context, &tables.$field, batch);
                if status != SINK_CONTINUE {
                    return status;
                }
            )*
        };
    }
    table!(
        files: files,
        related_diagnostics: related,
        diagnostics: diagnostics,
        value_words: value_words,
        constants: constants,
        type_ranges: type_ranges,
        type_members: type_members,
        types: types,
        parameters: parameters,
        instances: instances,
        semantic_edges: semantic_edges,
        semantic_nodes: semantic_nodes,
        udp_rows: udp_rows,
        udp_tables: udp_tables,
        lexical_tokens: lexical_tokens,
        source_libraries: source_libraries,
        line_directives: line_directives,
        edition_findings: edition_findings
    );
    // SAFETY: `context` is the sink's builder.
    unsafe { (sink.end)(context) }
}

fn stream_batched(tables: &Tables, limits: Limits, batch: usize) -> Result<Snapshot, SlangError> {
    let mut builder = StreamBuilder::new(limits);
    let sink = sink_for(&mut builder);
    let status = drive(&sink, &tables.header(), tables, batch);
    if let Some(error) = builder.take_error() {
        assert_eq!(status, SINK_ABORT);
        return Err(error);
    }
    assert_eq!(status, SINK_CONTINUE);
    builder.finish().map(|(snapshot, _)| snapshot)
}

fn stream_with(tables: &Tables, limits: Limits) -> Result<Snapshot, SlangError> {
    stream_batched(tables, limits, 4096)
}

fn stream(tables: &Tables) -> Result<Snapshot, SlangError> {
    stream_with(tables, Limits::default())
}

fn nodes(nodes: Vec<RawSemanticNode>) -> Tables {
    Tables {
        semantic_nodes: nodes,
        ..Tables::default()
    }
}

fn edge(role: u32, index: u32, target_id: u64) -> RawSemanticEdge {
    RawSemanticEdge {
        role,
        index,
        target_id,
        sequence_delay_valid: 0,
        sequence_delay_min: 0,
        sequence_delay_max: 0,
    }
}

fn invalid_native_message(result: Result<Snapshot, SlangError>) -> String {
    let error = result.expect_err("malformed stream must fail");
    assert_eq!(error.kind(), SlangErrorKind::InvalidNativeData, "{error}");
    error.message().to_owned()
}

fn raw_integer(value_start: u64, unknown_start: u64) -> RawConstant {
    RawConstant {
        kind: 1,
        is_signed: 0,
        bit_width: 1,
        value_word_start: value_start,
        unknown_word_start: unknown_start,
        word_count: 1,
        real_bits: 0,
        text: empty_raw_string(),
    }
}

fn raw_type(id: u64) -> RawType {
    RawType {
        id,
        kind: 1,
        flags: 0b111,
        bit_width: 1,
        display_name: empty_raw_string(),
        element_type_id: INVALID_ID,
        index_type_id: INVALID_ID,
        range_start: 0,
        range_count: 0,
        member_start: 0,
        member_count: 0,
    }
}

fn raw_semantic_node(edge_count: u64) -> RawSemanticNode {
    RawSemanticNode {
        id: 0,
        parent_id: INVALID_ID,
        kind: 25,
        subkind: 0,
        operation: 0,
        flags: 0,
        name: empty_raw_string(),
        detail: empty_raw_string(),
        definition_name: empty_raw_string(),
        range: RawRange {
            file_id: INVALID_ID,
            start: 0,
            end: 0,
        },
        type_id: INVALID_ID,
        constant_id: INVALID_ID,
        target_id: INVALID_ID,
        edge_start: 0,
        edge_count,
        time_unit: 0,
        time_unit_magnitude: 0,
        time_precision_unit: 0,
        time_precision_magnitude: 0,
        strength0: 0,
        strength1: 0,
        auxiliary: 0,
        assertion_range_min: 0,
        assertion_range_max: 0,
        assertion_repetition_kind: 0,
    }
}

fn raw_udp_node(id: u64, name: &str) -> RawSemanticNode {
    let mut node = raw_semantic_node(0);
    node.id = id;
    node.kind = 15;
    node.subkind = 227;
    node.flags = 1 << 24;
    node.name = raw_string(name);
    node
}

fn raw_udp_table(
    primitive_id: u64,
    row_start: u64,
    row_count: u64,
    input_count: u32,
    name: &str,
) -> RawUdpTable {
    RawUdpTable {
        primitive_id,
        input_count,
        reserved: 0,
        row_start,
        row_count,
        name: raw_string(name),
    }
}

fn raw_udp_row(inputs: &str, output: u8) -> RawUdpRow {
    RawUdpRow {
        inputs: raw_string(inputs),
        state: 0,
        output: u32::from(output),
        flags: 0,
        reserved: 0,
    }
}

#[test]
fn narrow_constants_charge_bits_without_double_charging_word_padding() {
    let raw = [raw_integer(0, 1), raw_integer(2, 3)];
    let words = [1, 0, 0, 0];
    let limits = Limits {
        max_value_bits: 2,
        ..Limits::default()
    };

    let tables = Tables {
        value_words: words.to_vec(),
        constants: raw.to_vec(),
        ..Tables::default()
    };
    let snapshot = stream_with(&tables, limits).expect("two one-bit values");
    assert_eq!(snapshot.constants.len(), 2);
}

#[test]
fn malformed_native_tags_and_related_windows_are_rejected() {
    let unknown_constant = RawConstant {
        kind: 99,
        is_signed: 0,
        bit_width: 0,
        value_word_start: INVALID_ID,
        unknown_word_start: INVALID_ID,
        word_count: 0,
        real_bits: 0,
        text: empty_raw_string(),
    };
    let tables = Tables {
        constants: vec![unknown_constant],
        ..Tables::default()
    };
    invalid_native_message(stream(&tables));

    let diagnostic = RawDiagnostic {
        provider: 1,
        severity: 2,
        subsystem: 0,
        code: 0,
        name: empty_raw_string(),
        option_name: empty_raw_string(),
        message: empty_raw_string(),
        primary: RawRange {
            file_id: INVALID_ID,
            start: 0,
            end: 0,
        },
        related_start: 1,
        related_count: 1,
    };
    let related = RawRelatedDiagnostic {
        range: RawRange {
            file_id: INVALID_ID,
            start: 0,
            end: 0,
        },
        message: empty_raw_string(),
    };
    let tables = Tables {
        related: vec![related],
        diagnostics: vec![diagnostic],
        ..Tables::default()
    };
    let message = invalid_native_message(stream(&tables));
    assert!(message.contains("diagnostic related records"), "{message}");
}

#[test]
fn malformed_type_component_and_windows_are_rejected() {
    let mut ty = raw_type(0);
    ty.element_type_id = 17;
    let types = |types: Vec<RawType>, members: Vec<RawTypeMember>| Tables {
        types,
        type_members: members,
        ..Tables::default()
    };
    invalid_native_message(stream(&types(vec![ty], vec![])));

    let mut ty = raw_type(0);
    ty.range_count = 1;
    invalid_native_message(stream(&types(vec![ty], vec![])));

    let mut ty = raw_type(0);
    ty.member_count = 1;
    let member = RawTypeMember {
        initializer_constant_id: INVALID_ID,
        name: empty_raw_string(),
        type_id: 9,
        bit_offset: 0,
        bit_width: 1,
    };
    invalid_native_message(stream(&types(vec![ty], vec![member])));
    let member = RawTypeMember {
        type_id: 0,
        initializer_constant_id: 0,
        ..member
    };
    invalid_native_message(stream(&types(vec![ty], vec![member])));
    let mut second = raw_type(1);
    second.id = 0;
    let message = invalid_native_message(stream(&types(vec![raw_type(0), second], vec![])));
    assert!(message.contains("dense table index"), "{message}");
}

#[test]
fn duplicate_semantic_role_index_is_rejected() {
    let node = raw_semantic_node(2);
    let raw_edges = [
        RawSemanticEdge {
            role: 1,
            index: 0,
            target_id: 0,
            sequence_delay_valid: 0,
            sequence_delay_min: 0,
            sequence_delay_max: 0,
        },
        RawSemanticEdge {
            role: 1,
            index: 0,
            target_id: 0,
            sequence_delay_valid: 0,
            sequence_delay_min: 0,
            sequence_delay_max: 0,
        },
    ];
    let tables = Tables {
        semantic_edges: raw_edges.to_vec(),
        semantic_nodes: vec![node],
        ..Tables::default()
    };
    let message = invalid_native_message(stream(&tables));
    assert!(message.contains("duplicate role/index"), "{message}");
}

#[test]
fn invalid_semantic_strength_and_port_state_are_rejected() {
    let mut node = raw_semantic_node(0);
    node.strength0 = 6;
    let error = stream(&nodes(vec![node])).expect_err("unknown drive strength must fail");
    assert_eq!(error.kind(), SlangErrorKind::InvalidNativeData);

    let mut node = raw_semantic_node(0);
    node.flags = 1 << 29;
    let error = stream(&nodes(vec![node]))
        .expect_err("open port state without a present connection must fail");
    assert_eq!(error.kind(), SlangErrorKind::InvalidNativeData);

    let mut node = raw_semantic_node(0);
    node.flags = 1 << 31;
    let error =
        stream(&nodes(vec![node])).expect_err("method with-clause flag on a scope must fail");
    assert_eq!(error.kind(), SlangErrorKind::InvalidNativeData);

    let mut node = raw_semantic_node(0);
    node.subkind = 32;
    let error = stream(&nodes(vec![node])).expect_err("statement subkind on a scope must fail");
    assert_eq!(error.kind(), SlangErrorKind::InvalidNativeData);

    let mut node = raw_semantic_node(0);
    node.time_unit = 4;
    node.time_unit_magnitude = 1;
    node.time_precision_unit = 2;
    node.time_precision_magnitude = 1;
    let error = stream(&nodes(vec![node])).expect_err("coarser time precision must fail");
    assert_eq!(error.kind(), SlangErrorKind::InvalidNativeData);

    let mut node = raw_semantic_node(0);
    node.auxiliary = 1;
    let error =
        stream(&nodes(vec![node])).expect_err("variable lifetime metadata on a scope must fail");
    assert_eq!(error.kind(), SlangErrorKind::InvalidNativeData);
}

#[test]
fn current_statement_and_expression_subkinds_are_admitted() {
    assert!(validate_semantic_subkind(18, 49).is_ok());
    assert!(validate_semantic_subkind(18, 59).is_ok());
    assert!(validate_semantic_subkind(18, 60).is_ok());
    assert!(validate_semantic_subkind(18, SEMANTIC_STMT_IMMEDIATE_ASSERT).is_ok());
    assert!(validate_semantic_subkind(18, SEMANTIC_STMT_IMMEDIATE_ASSUME).is_ok());
    assert!(validate_semantic_subkind(18, SEMANTIC_STMT_IMMEDIATE_COVER).is_ok());
    assert!(validate_semantic_subkind(18, SEMANTIC_STMT_CONCURRENT_ASSERT).is_ok());
    assert!(validate_semantic_subkind(18, SEMANTIC_STMT_CONCURRENT_ASSUME).is_ok());
    assert!(validate_semantic_subkind(18, SEMANTIC_STMT_CONCURRENT_COVER).is_ok());
    assert!(validate_semantic_subkind(18, SEMANTIC_STMT_CONCURRENT_EXPECT).is_ok());
    assert!(validate_semantic_subkind(18, SEMANTIC_STMT_PATTERN_CASE).is_ok());
    assert!(validate_semantic_subkind(19, 86).is_ok());
    assert!(validate_semantic_subkind(19, 89).is_ok());
    assert!(validate_semantic_subkind(19, SEMANTIC_EXPR_ASSERTION_INSTANCE).is_ok());
    assert!(validate_semantic_subkind(19, SEMANTIC_EXPR_CLOCKING_EVENT).is_ok());
    assert!(validate_semantic_subkind(19, SEMANTIC_EXPR_TAGGED_UNION).is_ok());
    assert!(validate_semantic_subkind(9, 229).is_ok());
    assert!(validate_semantic_subkind(25, SEMANTIC_SCOPE_CLOCKING_BLOCK).is_ok());
    assert!(validate_semantic_subkind(9, SEMANTIC_VARIABLE_CLOCKING).is_ok());
    assert!(validate_semantic_subkind(26, SEMANTIC_TIMING_ONE_STEP_DELAY).is_ok());
    assert!(validate_semantic_subkind(18, 68).is_err());
    assert!(validate_semantic_subkind(19, 79).is_err());
    assert!(validate_semantic_subkind(28, SEMANTIC_ASSERTION_EXPR_SIMPLE).is_ok());
    assert!(validate_semantic_subkind(28, SEMANTIC_ASSERTION_EXPR_DISABLE_IFF).is_ok());
    assert!(validate_semantic_subkind(28, 14).is_err());
    assert_eq!(
        decode_semantic_operation(47).expect("list operation must decode"),
        SemanticOperation::List
    );

    let mut method = raw_semantic_node(0);
    method.kind = 21;
    method.subkind = 76;
    method.flags = 1 << 31;
    let decoded =
        stream(&nodes(vec![method])).expect("method with-clause flag must decode on a method call");
    assert!(decoded.semantic_nodes[0].method_with_clause());

    let mut qualified = raw_semantic_node(0);
    qualified.kind = 18;
    qualified.subkind = 33;
    qualified.auxiliary = SEMANTIC_UNIQUE_PRIORITY_PRIORITY;
    let decoded = stream(&nodes(vec![qualified]))
        .expect("all repository-owned conditional qualifier tags must decode");
    assert_eq!(
        decoded.semantic_nodes[0].auxiliary,
        SEMANTIC_UNIQUE_PRIORITY_PRIORITY
    );
    qualified.auxiliary = SEMANTIC_UNIQUE_PRIORITY_PRIORITY + 1;
    let error =
        stream(&nodes(vec![qualified])).expect_err("unknown conditional qualifier must fail");
    assert_eq!(error.kind(), SlangErrorKind::InvalidNativeData);

    let mut variable = raw_semantic_node(0);
    variable.kind = 9;
    variable.auxiliary = 2;
    let decoded = stream(&nodes(vec![variable])).expect("resolved variable lifetime must decode");
    assert_eq!(decoded.semantic_nodes[0].auxiliary, 2);

    variable.kind = 11;
    let decoded =
        stream(&nodes(vec![variable])).expect("named-event variable lifetime must decode");
    assert_eq!(decoded.semantic_nodes[0].auxiliary, 2);

    let mut clocking_block = raw_semantic_node(0);
    clocking_block.kind = 25;
    clocking_block.subkind = SEMANTIC_SCOPE_CLOCKING_BLOCK;
    clocking_block.auxiliary = CLOCKING_BLOCK_DEFAULT
        | CLOCKING_BLOCK_GLOBAL
        | (3 << CLOCKING_INPUT_EDGE_SHIFT)
        | (2 << CLOCKING_OUTPUT_EDGE_SHIFT);
    let decoded =
        stream(&nodes(vec![clocking_block])).expect("clocking block metadata must decode");
    assert_eq!(
        decoded.semantic_nodes[0].auxiliary,
        clocking_block.auxiliary
    );

    let mut clocking_var = raw_semantic_node(0);
    clocking_var.kind = 9;
    clocking_var.subkind = SEMANTIC_VARIABLE_CLOCKING;
    clocking_var.auxiliary = 1 | (3 << CLOCKING_VAR_OUTPUT_EDGE_SHIFT);
    let decoded =
        stream(&nodes(vec![clocking_var])).expect("clocking variable metadata must decode");
    assert_eq!(decoded.semantic_nodes[0].auxiliary, clocking_var.auxiliary);

    variable.auxiliary = 3;
    let error =
        stream(&nodes(vec![variable])).expect_err("unknown resolved variable lifetime must fail");
    assert_eq!(error.kind(), SlangErrorKind::InvalidNativeData);

    let tables = Tables {
        semantic_edges: vec![edge(30, 0, 0), edge(31, 0, 0)],
        semantic_nodes: vec![raw_semantic_node(2)],
        ..Tables::default()
    };
    let edges = stream(&tables)
        .expect("semantic identity roles must decode")
        .semantic_edges;
    assert_eq!(edges[0].role, SemanticEdgeRole::SourceIdentity);
    assert_eq!(edges[1].role, SemanticEdgeRole::ReturnOwner);
}

#[test]
fn semantic_and_type_table_limits_are_exact() {
    assert!(enforce_count(4, 4, "semantic nodes").is_ok());
    assert!(enforce_count(5, 4, "semantic nodes").is_err());
    assert!(enforce_count(4, 4, "semantic edges").is_ok());
    assert!(enforce_count(5, 4, "semantic edges").is_err());
    assert!(enforce_count(4, 4, "lexical tokens").is_ok());
    assert!(enforce_count(5, 4, "lexical tokens").is_err());
    assert!(enforce_count(4, 4, "type ranges").is_ok());
    assert!(enforce_count(5, 4, "type ranges").is_err());
    assert!(enforce_count(4, 4, "type members").is_ok());
    assert!(enforce_count(5, 4, "type members").is_err());
    assert!(enforce_count(4, 4, "constants").is_ok());
    assert!(enforce_count(5, 4, "constants").is_err());
}

fn udp_stream(
    names: &[&str],
    tables: Vec<RawUdpTable>,
    rows: Vec<RawUdpRow>,
    max_overlap: u64,
) -> Result<Snapshot, SlangError> {
    let tables = Tables {
        semantic_nodes: names
            .iter()
            .enumerate()
            .map(|(id, name)| raw_udp_node(id as u64, name))
            .collect(),
        udp_rows: rows,
        udp_tables: tables,
        ..Tables::default()
    };
    stream_with(
        &tables,
        Limits {
            max_semantic_edges: max_overlap,
            ..Limits::default()
        },
    )
}

#[test]
fn udp_overlap_validation_rejects_conflicts_and_bounds_expansion() {
    let table = raw_udp_table(0, 0, 2, 2, "udp");
    let rows = vec![raw_udp_row("0?", b'0'), raw_udp_row("?0", b'1')];
    let error = udp_stream(&["udp"], vec![table], rows, 16)
        .expect_err("overlapping UDP rows with different outputs must fail");
    assert_eq!(error.kind(), SlangErrorKind::InvalidNativeData);
    assert!(error.message().contains("overlapping rows"));

    let table = raw_udp_table(0, 0, 1, 2, "udp");
    let rows = vec![raw_udp_row("??", b'x')];
    let error = udp_stream(&["udp"], vec![table], rows, 8)
        .expect_err("wildcard expansion must honor the selected validation capacity");
    assert_eq!(error.kind(), SlangErrorKind::LimitExceeded);
    assert!(error.message().contains("overlap validation"));
}

#[test]
fn udp_overlap_budget_is_shared_across_tables() {
    let tables = || {
        vec![
            raw_udp_table(0, 0, 1, 2, "first"),
            raw_udp_table(1, 1, 1, 2, "second"),
        ]
    };
    let rows = || vec![raw_udp_row("0?", b'0'), raw_udp_row("1?", b'1')];

    let decoded = udp_stream(&["first", "second"], tables(), rows(), 6)
        .expect("exactly the shared assignment budget should be accepted");
    assert_eq!(decoded.udp_tables.len(), 2);
    assert_eq!(decoded.udp_tables[1].rows[0].inputs, "1?");

    let error = udp_stream(&["first", "second"], tables(), rows(), 5)
        .expect_err("multiple UDP tables must charge one shared overlap budget");
    assert_eq!(error.kind(), SlangErrorKind::LimitExceeded);
    assert!(error.message().contains("bounded assignment capacity"));
}

#[test]
fn udp_wide_wildcard_rejects_before_large_assignment_growth() {
    let table = raw_udp_table(0, 0, 1, 15, "udp");
    let rows = vec![raw_udp_row("???????????????", b'x')];
    let error = udp_stream(&["udp"], vec![table], rows, NATIVE_MAX_RECORDS)
        .expect_err("wide wildcard expansion must stay below the memory boundary");
    assert_eq!(error.kind(), SlangErrorKind::LimitExceeded);
    assert!(error.message().contains("bounded assignment capacity"));
}

#[test]
fn udp_rows_must_be_claimed_by_a_table_window() {
    let table = raw_udp_table(0, 0, 1, 2, "udp");
    let rows = vec![raw_udp_row("00", b'0'), raw_udp_row("01", b'1')];
    let error = udp_stream(&["udp"], vec![table], rows, 8)
        .expect_err("unclaimed UDP rows must fail checked decoding");
    assert_eq!(error.kind(), SlangErrorKind::InvalidNativeData);
    assert!(error.message().contains("not owned by a table"));
}

#[test]
fn language_edition_parser_and_default_are_explicit() {
    assert_eq!(
        LanguageEdition::default(),
        LanguageEdition::SystemVerilog2009
    );
    assert_eq!(LanguageEdition::Verilog2001.to_string(), "2001");
    assert_eq!(LanguageEdition::SystemVerilog2009.to_string(), "2009");
    assert_eq!(
        "1364-2001".parse::<LanguageEdition>(),
        Ok(LanguageEdition::Verilog2001)
    );
    assert_eq!(
        "1800-2009".parse::<LanguageEdition>(),
        Ok(LanguageEdition::SystemVerilog2009)
    );
    assert!("2017".parse::<LanguageEdition>().is_err());
    assert_eq!(
        LanguageEdition::from_snapshot_flags(SNAPSHOT_EDITION_VERILOG_2001),
        Ok(LanguageEdition::Verilog2001)
    );
    assert!(LanguageEdition::from_snapshot_flags(0).is_err());
    assert!(LanguageEdition::from_snapshot_flags(SNAPSHOT_EDITION_MASK).is_err());
}

#[test]
fn sequential_predicate_pattern_edge_decodes_and_unknown_roles_still_fail() {
    let tables = |edge| Tables {
        semantic_edges: vec![edge],
        semantic_nodes: vec![raw_semantic_node(1)],
        ..Tables::default()
    };
    let decoded = stream(&tables(edge(38, 2, 0)))
        .expect("paired predicate pattern role")
        .semantic_edges;
    assert_eq!(decoded[0].role, SemanticEdgeRole::ConditionPattern);
    assert_eq!(decoded[0].index, 2);
    let message = invalid_native_message(stream(&tables(edge(39, 2, 0))));
    assert!(message.contains("unknown role"), "{message}");
}

#[test]
fn semantic_decode_reserves_exact_validated_record_counts() {
    for count in [33, 513] {
        let nodes = (0..count)
            .map(|index| {
                let mut node = raw_semantic_node(1);
                node.id = index as u64;
                node.edge_start = index as u64;
                node
            })
            .collect::<Vec<_>>();
        let edges = (0..count)
            .map(|index| edge(1, 0, index as u64))
            .collect::<Vec<_>>();
        let tables = Tables {
            semantic_edges: edges,
            semantic_nodes: nodes,
            ..Tables::default()
        };
        // Odd batches split both tables across several callbacks.
        let snapshot = stream_batched(&tables, Limits::default(), 7).unwrap();
        let (edges, decoded) = (snapshot.semantic_edges, snapshot.semantic_nodes);
        assert_eq!(edges.capacity(), count);
        assert_eq!(decoded.capacity(), count);
        assert_eq!(decoded.len(), count);
        for (index, node) in decoded.iter().enumerate() {
            assert_eq!(node.id, index as u64);
            assert_eq!(edges[index].target_id, node.id);
        }
    }
}

#[test]
fn dense_semantic_decode_rejects_malformed_ids_and_references() {
    for ids in [[0, 0], [1, 0], [0, INVALID_ID], [0, 2]] {
        let raw = ids.map(|id| {
            let mut node = raw_semantic_node(0);
            node.id = id;
            node
        });
        let message = invalid_native_message(stream(&nodes(raw.to_vec())));
        assert!(message.contains("contiguous arena indices"), "{message}");
    }
    for dangling in [1, u64::MAX - 1] {
        let mut node = raw_semantic_node(0);
        node.parent_id = dangling;
        assert!(stream(&nodes(vec![node])).is_err());
        node.parent_id = INVALID_ID;
        node.target_id = dangling;
        assert!(stream(&nodes(vec![node])).is_err());
        node.target_id = INVALID_ID;
        let mut owner = node;
        owner.edge_count = 1;
        let tables = Tables {
            semantic_edges: vec![edge(1, 0, dangling)],
            semantic_nodes: vec![owner],
            ..Tables::default()
        };
        assert!(stream(&tables).is_err());
    }
}

#[test]
fn source_library_records_require_known_unique_nodes_and_names() {
    let name = b"rtl";
    let library = |semantic_id| RawSourceLibrary {
        semantic_id,
        library: RawString {
            data: name.as_ptr(),
            len: name.len() as u64,
        },
    };
    let tables = |libraries: Vec<RawSourceLibrary>, node_count: u64| Tables {
        semantic_nodes: (0..node_count)
            .map(|id| {
                let mut node = raw_semantic_node(0);
                node.id = id;
                node
            })
            .collect(),
        source_libraries: libraries,
        ..Tables::default()
    };
    let decoded = stream(&tables(vec![library(1), library(0)], 2))
        .expect("valid records")
        .source_libraries;
    assert_eq!(
        decoded,
        vec![
            SourceLibraryBinding {
                semantic_id: 1,
                library: "rtl".to_owned(),
            },
            SourceLibraryBinding {
                semantic_id: 0,
                library: "rtl".to_owned(),
            },
        ]
    );
    for records in [vec![library(2)], vec![library(0), library(0)]] {
        let message = invalid_native_message(stream(&tables(records, 2)));
        assert!(message.contains("invalid or repeated semantic node"));
    }
    let empty = RawSourceLibrary {
        semantic_id: 0,
        library: empty_raw_string(),
    };
    let message = invalid_native_message(stream(&tables(vec![empty], 1)));
    assert!(message.contains("empty name"));
}

#[test]
fn line_directive_records_are_bounded_sorted_and_deduplicated() {
    let names = [b"a.sv".as_slice(), b"b.svh".as_slice()];
    let files = vec![
        RawFile {
            id: 0,
            name: RawString {
                data: names[0].as_ptr(),
                len: names[0].len() as u64,
            },
            byte_len: 40,
        },
        RawFile {
            id: 1,
            name: RawString {
                data: names[1].as_ptr(),
                len: names[1].len() as u64,
            },
            byte_len: 10,
        },
    ];
    let tables = |line_directives: Vec<RawLineDirective>| Tables {
        files: files.clone(),
        line_directives,
        ..Tables::default()
    };
    let mapped = b"orig.sv";
    let other = b"other.sv";
    let record = |file_id, physical_offset, logical_line, name: &'static [u8]| RawLineDirective {
        file_id,
        physical_offset,
        logical_line,
        logical_file: RawString {
            data: name.as_ptr(),
            len: name.len() as u64,
        },
    };
    let decoded = stream(&tables(vec![
        record(1, 4, 9, mapped),
        record(0, 30, 70, other),
        record(0, 12, 40, mapped),
        // An include reached twice reports the same mapping twice.
        record(0, 12, 40, mapped),
    ]))
    .expect("valid records")
    .line_directives;
    let summary: Vec<_> = decoded
        .iter()
        .map(|d| {
            (
                d.file_id,
                d.physical_offset,
                d.logical_line,
                d.logical_file.as_str(),
            )
        })
        .collect();
    assert_eq!(
        summary,
        vec![
            (0, 12, 40, "orig.sv"),
            (0, 30, 70, "other.sv"),
            (1, 4, 9, "orig.sv")
        ]
    );
    for (records, message) in [
        (vec![record(2, 0, 1, mapped)], "unknown file"),
        (vec![record(1, 10, 1, mapped)], "outside its file"),
        (
            vec![record(0, 12, 40, mapped), record(0, 12, 41, mapped)],
            "disagree",
        ),
    ] {
        let error = invalid_native_message(stream(&tables(records)));
        assert!(error.contains(message), "{error}");
    }
}

#[test]
fn edition_finding_records_are_bounded_and_keep_capture_order() {
    let name = b"a.v";
    let files = vec![RawFile {
        id: 0,
        name: RawString {
            data: name.as_ptr(),
            len: name.len() as u64,
        },
        byte_len: 20,
    }];
    let tables = |edition_findings: Vec<RawEditionFinding>| Tables {
        files: files.clone(),
        edition_findings,
        ..Tables::default()
    };
    let record = |file_id, start, end, label: &'static [u8]| RawEditionFinding {
        range: RawRange {
            file_id,
            start,
            end,
        },
        label: RawString {
            data: label.as_ptr(),
            len: label.len() as u64,
        },
    };
    let decoded = stream(&tables(vec![
        record(0, 9, 12, b"end label"),
        record(0, 2, 4, b"cast"),
    ]))
    .expect("valid records")
    .edition_findings;
    let summary: Vec<_> = decoded
        .iter()
        .map(|f| (f.range.start, f.range.end, f.label.as_str()))
        .collect();
    assert_eq!(summary, vec![(9, 12, "end label"), (2, 4, "cast")]);
    for (records, message) in [
        (vec![record(1, 0, 1, b"cast")], "unknown file"),
        (vec![record(0, 4, 21, b"cast")], "outside its file"),
        (vec![record(INVALID_ID, 0, 0, b"cast")], "no source range"),
        (vec![record(0, 1, 2, b"")], "empty label"),
    ] {
        let error = invalid_native_message(stream(&tables(records)));
        assert!(error.contains(message), "{error}");
    }
}

fn stream_error(builder: &mut StreamBuilder) -> String {
    let error = builder.take_error().expect("receiver must record an error");
    assert_eq!(error.kind(), SlangErrorKind::InvalidNativeData, "{error}");
    error.message().to_owned()
}

#[test]
fn stream_tables_follow_header_order_and_counts() {
    let names = b"top.sv";
    let file = RawFile {
        id: 0,
        name: RawString {
            data: names.as_ptr(),
            len: names.len() as u64,
        },
        byte_len: 1,
    };
    let tables = Tables {
        files: vec![file],
        semantic_nodes: vec![raw_semantic_node(0)],
        ..Tables::default()
    };
    let header = tables.header();

    // A table after a later one has started is out of order.
    let mut builder = StreamBuilder::new(Limits::default());
    let sink = sink_for(&mut builder);
    let context = sink.context;
    // SAFETY: `context` is the sink's builder and the header is live.
    assert_eq!(unsafe { (sink.begin)(context, &header) }, SINK_CONTINUE);
    let status = deliver(sink.semantic_nodes, context, &tables.semantic_nodes, 1);
    assert_eq!(status, SINK_ABORT);
    assert!(stream_error(&mut builder).contains("ended files early"));

    let mut builder = StreamBuilder::new(Limits::default());
    let sink = sink_for(&mut builder);
    let context = sink.context;
    // SAFETY: as above.
    assert_eq!(unsafe { (sink.begin)(context, &header) }, SINK_CONTINUE);
    assert_eq!(
        deliver(sink.files, context, &tables.files, 1),
        SINK_CONTINUE
    );
    let status = deliver(sink.semantic_nodes, context, &tables.semantic_nodes, 1);
    assert_eq!(status, SINK_CONTINUE);
    assert_eq!(deliver(sink.files, context, &tables.files, 1), SINK_ABORT);
    assert!(stream_error(&mut builder).contains("delivered files out of order"));

    // More records than announced.
    let mut builder = StreamBuilder::new(Limits::default());
    let sink = sink_for(&mut builder);
    let context = sink.context;
    // SAFETY: as above.
    assert_eq!(unsafe { (sink.begin)(context, &header) }, SINK_CONTINUE);
    let twice = [file, file];
    assert_eq!(deliver(sink.files, context, &twice, 2), SINK_ABORT);
    assert!(stream_error(&mut builder).contains("more files than announced"));

    // Fewer records than announced, detected when the stream ends.
    let short = RawStreamHeader {
        semantic_node_count: 2,
        ..tables.header()
    };
    let mut builder = StreamBuilder::new(Limits::default());
    let sink = sink_for(&mut builder);
    assert_eq!(drive(&sink, &short, &tables, 4), SINK_ABORT);
    assert!(stream_error(&mut builder).contains("ended semantic nodes early"));

    // Records before the header, a repeated header, records after the end and
    // a missing end are all malformed.
    let mut builder = StreamBuilder::new(Limits::default());
    let sink = sink_for(&mut builder);
    assert_eq!(
        deliver(sink.files, sink.context, &tables.files, 1),
        SINK_ABORT
    );
    assert!(stream_error(&mut builder).contains("before its header"));

    let mut builder = StreamBuilder::new(Limits::default());
    let sink = sink_for(&mut builder);
    let context = sink.context;
    // SAFETY: as above.
    assert_eq!(unsafe { (sink.begin)(context, &header) }, SINK_CONTINUE);
    // SAFETY: as above.
    assert_eq!(unsafe { (sink.begin)(context, &header) }, SINK_ABORT);
    assert!(stream_error(&mut builder).contains("repeated its header"));

    let mut builder = StreamBuilder::new(Limits::default());
    let sink = sink_for(&mut builder);
    assert_eq!(drive(&sink, &header, &tables, 4), SINK_CONTINUE);
    assert_eq!(
        deliver(sink.files, sink.context, &tables.files, 1),
        SINK_ABORT
    );
    assert!(stream_error(&mut builder).contains("after its end"));

    let mut builder = StreamBuilder::new(Limits::default());
    let sink = sink_for(&mut builder);
    // SAFETY: as above.
    let status = unsafe { (sink.begin)(sink.context, &header) };
    assert_eq!(status, SINK_CONTINUE);
    let error = builder.finish().map(|_| ()).expect_err("missing end");
    assert!(error.message().contains("without its end record"));

    let mut stale = tables.header();
    stale.abi_version = ABI_VERSION - 1;
    let mut builder = StreamBuilder::new(Limits::default());
    let sink = sink_for(&mut builder);
    assert_eq!(drive(&sink, &stale, &tables, 4), SINK_ABORT);
    assert!(stream_error(&mut builder).contains("ABI version mismatch"));
}

#[test]
fn stream_header_enforces_record_limits_and_export_budget() {
    let tables = nodes(vec![raw_semantic_node(0)]);
    let limits = Limits {
        max_semantic_nodes: 0,
        ..Limits::default()
    };
    let message = invalid_native_message(stream_with(&tables, limits));
    assert!(
        message.contains("native semantic nodes exceed"),
        "{message}"
    );

    let record_bytes = std::mem::size_of::<RawSemanticNode>() as u64;
    let exact = Limits {
        max_output_bytes: record_bytes,
        ..Limits::default()
    };
    stream_with(&tables, exact).expect("records exactly fill the budget");
    let short = Limits {
        max_output_bytes: record_bytes - 1,
        ..Limits::default()
    };
    let message = invalid_native_message(stream_with(&tables, short));
    assert!(message.contains("output records exceed"), "{message}");

    // Strings are charged as they arrive, on top of the record bytes.
    let mut named = raw_semantic_node(0);
    named.name = raw_string("name");
    let named = nodes(vec![named]);
    let fits = Limits {
        max_output_bytes: record_bytes + 4,
        ..Limits::default()
    };
    let snapshot = stream_with(&named, fits).expect("name fits the budget");
    assert_eq!(snapshot.semantic_nodes[0].name, "name");
    let short = Limits {
        max_output_bytes: record_bytes + 3,
        ..Limits::default()
    };
    let message = invalid_native_message(stream_with(&named, short));
    assert!(message.contains("output strings exceed"), "{message}");

    let mut invalid = raw_semantic_node(0);
    let bytes = [0xff_u8];
    invalid.detail = RawString {
        data: bytes.as_ptr(),
        len: 1,
    };
    let message = invalid_native_message(stream(&nodes(vec![invalid])));
    assert!(message.contains("not valid UTF-8"), "{message}");
}

#[test]
fn instance_and_parameter_windows_are_dense_and_owned() {
    let instance = |id, start, count| RawInstance {
        id,
        parent_id: INVALID_ID,
        kind: 1,
        reserved: 0,
        name: empty_raw_string(),
        definition_name: empty_raw_string(),
        declaration: RawRange {
            file_id: INVALID_ID,
            start: 0,
            end: 0,
        },
        parameter_start: start,
        parameter_count: count,
    };
    let parameter = |owner| RawParameter {
        owner_instance_id: owner,
        kind: 1,
        flags: 0,
        name: empty_raw_string(),
        declaration: RawRange {
            file_id: INVALID_ID,
            start: 0,
            end: 0,
        },
        type_id: INVALID_ID,
        constant_id: INVALID_ID,
    };
    let tables = |instances, parameters| Tables {
        instances,
        parameters,
        ..Tables::default()
    };
    let snapshot = stream(&tables(
        vec![instance(0, 0, 1), instance(1, 1, 1)],
        vec![parameter(0), parameter(1)],
    ))
    .expect("dense instances own their parameter windows");
    assert_eq!(snapshot.instances.len(), 2);
    assert_eq!(snapshot.parameters[1].owner_instance_id, 1);

    for (instances, parameters, expected) in [
        (vec![instance(1, 0, 0)], vec![], "dense table index"),
        (
            vec![instance(0, 0, 1), instance(1, 0, 1)],
            vec![parameter(0)],
            "overlap or contain the wrong owner",
        ),
        (
            vec![instance(0, 0, 1)],
            vec![parameter(0), parameter(0)],
            "not covered by its owner instance window",
        ),
        (
            vec![instance(0, 0, 0)],
            vec![parameter(1)],
            "owner instance",
        ),
    ] {
        let message = invalid_native_message(stream(&tables(instances, parameters)));
        assert!(message.contains(expected), "{message}");
    }
}

#[test]
fn receiver_panic_becomes_an_internal_error_and_stops_the_stream() {
    let mut builder = StreamBuilder::new(Limits::default());
    let context = (&mut builder as *mut StreamBuilder).cast::<c_void>();
    // SAFETY: `context` is the only live reference to `builder`.
    let status = unsafe { receive(context, |_| panic!("receiver failure")) };
    assert_eq!(status, SINK_ABORT);
    // SAFETY: as above; a recorded error stops every later callback.
    let status = unsafe { receive(context, |_| Ok(())) };
    assert_eq!(status, SINK_ABORT);
    let error = builder.take_error().expect("panic is recorded");
    assert_eq!(error.kind(), SlangErrorKind::Internal);
    assert!(error.message().contains("panicked"));
}

const BRIDGE_SOURCE: &str = "module top; logic a; assign a = 1'b1; endmodule\n";

unsafe extern "C" fn reject_header(_: *mut c_void, _: *const RawStreamHeader) -> u32 {
    SINK_ABORT
}

#[test]
fn native_bridge_stops_on_sink_abort_and_keeps_the_receiver_error() {
    let sources = [Source::compilation_unit("top.sv", BRIDGE_SOURCE)];
    let options = CompileOptions::default();
    let request = CompileRequest {
        sources: &sources,
        library_sources: &[],
        options: &options,
    };

    // A sink abort ends the native call with its own status and message.
    let mut builder = StreamBuilder::new(Limits::default());
    let mut sink = sink_for(&mut builder);
    sink.begin = reject_header;
    let (status, error) = call_native(&request, &sink);
    assert_eq!(status, STATUS_SINK_ABORTED);
    let error = take_native_error(status, error);
    assert_eq!(error.kind(), SlangErrorKind::Internal);
    assert!(error.message().contains("sink rejected"), "{error}");

    // A receiver rejection aborts the real stream; the receiver's error is
    // what `compile` reports.
    let strict = Limits {
        max_semantic_nodes: 0,
        ..Limits::default()
    };
    let mut builder = StreamBuilder::new(strict);
    let sink = sink_for(&mut builder);
    let (status, _error) = call_native(&request, &sink);
    assert_eq!(status, STATUS_SINK_ABORTED);
    let message = stream_error(&mut builder);
    assert!(
        message.contains("native semantic nodes exceed"),
        "{message}"
    );

    // The full stream of a real compilation is complete and owned.
    let snapshot = compile(&request).expect("valid source compiles");
    assert_eq!(snapshot.files.len(), 1);
    assert_eq!(snapshot.files[0].text, BRIDGE_SOURCE);
    assert!(!snapshot.has_errors());
    assert!(snapshot
        .semantic_nodes
        .iter()
        .any(|node| node.kind == SemanticKind::ContinuousAssign));
}

#[test]
fn node_text_is_interned_per_stream_and_compares_as_str() {
    let mut first = raw_semantic_node(0);
    first.name = raw_string("clk");
    first.detail = raw_string("NamedValue");
    let mut second = first;
    second.id = 1;
    second.name = raw_string("rst");
    let snapshot = stream(&nodes(vec![first, second])).expect("valid nodes");
    let [a, b] = &snapshot.semantic_nodes[..] else {
        panic!("two nodes");
    };
    assert!(Arc::ptr_eq(&a.detail.0, &b.detail.0));
    assert!(!Arc::ptr_eq(&a.name.0, &b.name.0));
    assert_eq!(a.name, "clk");
    assert_eq!(b.name.as_str(), "rst");
    assert_eq!(a.detail, String::from("NamedValue"));
    assert!(a.definition_name.is_empty());
    assert_eq!(format!("{} {:?}", a.name, b.name), "clk \"rst\"");

    let mut boxed = edge(1, 0, 0);
    boxed.sequence_delay_valid = 1;
    boxed.sequence_delay_min = 2;
    boxed.sequence_delay_max = u32::MAX;
    let mut owner = raw_semantic_node(1);
    owner.kind = 28;
    let tables = Tables {
        semantic_edges: vec![boxed],
        semantic_nodes: vec![owner],
        ..Tables::default()
    };
    let delay = stream(&tables).expect("sequence delay").semantic_edges[0]
        .sequence_delay
        .clone();
    assert_eq!(
        delay.as_deref(),
        Some(&SemanticSequenceRange { min: 2, max: None })
    );
}

fn decoded_flags_node(raw_flags: u32, kind: u32, auxiliary: u64) -> SemanticNode {
    let mut raw = raw_semantic_node(0);
    raw.kind = kind;
    raw.flags = raw_flags;
    raw.auxiliary = auxiliary;
    if kind == 21 {
        raw.subkind = 76;
    }
    stream(&nodes(vec![raw]))
        .expect("a single valid flag must decode")
        .semantic_nodes
        .remove(0)
}

#[test]
fn every_raw_node_flag_sets_exactly_its_named_accessor() {
    let derived = [SemanticFlags::IS_CONST_REF, SemanticFlags::IS_REF_STATIC];
    let mut seen = 0u64;
    for (name, flag, accessor) in SemanticFlags::ALL {
        if derived.contains(flag) {
            continue;
        }
        assert_eq!(flag.bits().count_ones(), 1, "{name}");
        assert_eq!(seen & flag.bits(), 0, "{name} shares a bit");
        seen |= flag.bits();
        let bit = flag.bits().trailing_zeros();
        // Open connections require a connection; with-clause needs a method call.
        let raw = 1u32 << bit
            | if *flag == SemanticFlags::PORT_CONNECTION_OPEN {
                SemanticFlags::PORT_CONNECTION_PRESENT.bits() as u32
            } else {
                0
            };
        let kind = if *flag == SemanticFlags::METHOD_WITH_CLAUSE {
            21
        } else {
            25
        };
        let node = decoded_flags_node(raw, kind, 0);
        assert!(accessor(&node), "{name}");
        for (other, other_flag, other_accessor) in SemanticFlags::ALL {
            let expected = other_flag == flag
                || (*flag == SemanticFlags::PORT_CONNECTION_OPEN
                    && *other_flag == SemanticFlags::PORT_CONNECTION_PRESENT);
            assert_eq!(other_accessor(&node), expected, "{name} vs {other}");
        }
    }
    assert_eq!(
        seen.count_ones() as usize,
        SemanticFlags::ALL.len() - derived.len()
    );
    assert_eq!(SemanticFlags::default(), SemanticFlags::EMPTY);
}

#[test]
fn definition_kind_bits_stay_out_of_the_boolean_flags() {
    for (raw, expected) in [
        (1u32 << 13, SemanticDefinitionKind::Module),
        (1 << 14, SemanticDefinitionKind::Interface),
        (1 << 15, SemanticDefinitionKind::Program),
    ] {
        let node = decoded_flags_node(raw, 24, 0);
        assert_eq!(node.definition_kind, Some(expected));
        assert_eq!(node.flags, SemanticFlags::EMPTY);
    }
}

#[test]
fn argument_qualifiers_come_from_auxiliary_for_arguments_only() {
    let both = ARGUMENT_CONST_REF | ARGUMENT_REF_STATIC;
    let argument = decoded_flags_node(0, 17, both);
    assert!(argument.is_const_ref() && argument.is_ref_static());
    let only_const = decoded_flags_node(0, 17, ARGUMENT_CONST_REF);
    assert!(only_const.is_const_ref() && !only_const.is_ref_static());
    // A parameter reuses auxiliary bit 0 for its override marker.
    let parameter = decoded_flags_node(0, 12, ARGUMENT_CONST_REF);
    assert!(!parameter.is_const_ref() && !parameter.is_ref_static());
    assert_eq!(parameter.auxiliary, ARGUMENT_CONST_REF);
}

#[test]
fn flags_debug_lists_the_set_accessors() {
    let flags = SemanticFlags::IS_BAD | SemanticFlags::IS_TOP;
    assert_eq!(format!("{flags:?}"), "{is_bad, is_top}");
    assert_eq!(format!("{:?}", SemanticFlags::EMPTY), "{}");
}

#[test]
fn semantic_node_stays_compact() {
    // The capture holds one node per semantic construct (600k at 10k
    // processes). Growing this bound needs a measured reason.
    assert!(
        std::mem::size_of::<SemanticNode>() <= 160,
        "SemanticNode is {} bytes",
        std::mem::size_of::<SemanticNode>()
    );
}

#[test]
fn compact_ids_round_trip_and_reject_the_absent_sentinel() {
    assert_eq!(CompactId::pack(None, "x").unwrap().get(), None);
    assert_eq!(CompactId::pack(Some(0), "x").unwrap().get(), Some(0));
    let largest = u64::from(u32::MAX - 1);
    assert_eq!(
        CompactId::pack(Some(largest), "x").unwrap().get(),
        Some(largest)
    );
    for too_large in [u64::from(u32::MAX), u64::from(u32::MAX) + 1, u64::MAX] {
        let error = CompactId::pack(Some(too_large), "type").unwrap_err();
        assert_eq!(error.kind(), SlangErrorKind::InvalidNativeData);
        assert!(error.message().contains("type exceeds"), "{error}");
    }
    assert_eq!(CompactId::saturating(None).get(), None);
    assert_eq!(CompactId::saturating(Some(7)).get(), Some(7));
    assert_eq!(CompactId::saturating(Some(u64::MAX)).get(), Some(largest));
    assert_eq!(
        CompactId::saturating(Some(u64::from(u32::MAX))).get(),
        Some(largest)
    );
}

#[test]
fn compact_node_fields_read_back_as_the_logical_values() {
    let mut child = raw_semantic_node(1);
    child.id = 1;
    child.parent_id = 0;
    child.target_id = 0;
    child.edge_start = 0;
    child.time_unit = 4;
    child.time_unit_magnitude = 10;
    child.time_precision_unit = 5;
    child.time_precision_magnitude = 1;
    let mut tables = nodes(vec![raw_semantic_node(0), child]);
    tables.semantic_edges = vec![edge(1, 0, 0)];
    let snapshot = stream(&tables).expect("a small node graph must decode");
    let root = &snapshot.semantic_nodes[0];
    let child = &snapshot.semantic_nodes[1];
    assert_eq!(
        (root.parent_id(), root.target_id(), root.type_id()),
        (None, None, None)
    );
    assert_eq!(root.constant_id(), None);
    assert_eq!((root.edge_start(), root.edge_count()), (0, 0));
    assert_eq!(root.time_scale(), None);
    assert_eq!((child.parent_id(), child.target_id()), (Some(0), Some(0)));
    assert_eq!((child.edge_start(), child.edge_count()), (0, 1));
    let scale = child.time_scale().expect("time scale must survive boxing");
    assert_eq!((scale.magnitude, scale.precision_magnitude), (10, 1));
}

#[test]
fn assertion_metadata_is_boxed_only_when_it_differs_from_the_defaults() {
    let plain = decoded_flags_node(0, 25, 0);
    assert!(plain.assertion.is_none());
    assert_eq!(
        (
            plain.assertion_range_min(),
            plain.assertion_range_max(),
            plain.assertion_repetition_kind()
        ),
        (0, None, 0)
    );

    let mut raw = raw_semantic_node(0);
    raw.kind = 28;
    raw.subkind = SEMANTIC_ASSERTION_EXPR_SIMPLE;
    raw.auxiliary = SEMANTIC_ASSERTION_REPETITION | SEMANTIC_ASSERTION_RANGE;
    raw.assertion_range_min = 2;
    raw.assertion_range_max = 5;
    raw.assertion_repetition_kind = 1;
    let ranged = stream(&nodes(vec![raw]))
        .expect("a bounded repetition range must decode")
        .semantic_nodes
        .remove(0);
    assert_eq!(
        (
            ranged.assertion_range_min(),
            ranged.assertion_range_max(),
            ranged.assertion_repetition_kind()
        ),
        (2, Some(5), 1)
    );

    let mut unbounded = raw_semantic_node(0);
    unbounded.kind = 28;
    unbounded.subkind = SEMANTIC_ASSERTION_EXPR_SIMPLE;
    unbounded.assertion_range_max = SEMANTIC_ASSERTION_RANGE_UNBOUNDED;
    let unbounded = stream(&nodes(vec![unbounded]))
        .expect("an unbounded default must decode")
        .semantic_nodes
        .remove(0);
    assert!(unbounded.assertion.is_none());
    assert_eq!(unbounded.assertion_range_max(), None);
}
