use super::*;

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

#[test]
fn narrow_constants_charge_bits_without_double_charging_word_padding() {
    let raw = [raw_integer(0, 1), raw_integer(2, 3)];
    let words = [1, 0, 0, 0];
    let limits = Limits {
        max_value_bits: 2,
        ..Limits::default()
    };

    let constants = decode_constants(&raw, &words, &limits).expect("two one-bit values");
    assert_eq!(constants.len(), 2);
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
    let error = decode_constants(&[unknown_constant], &[], &Limits::default())
        .expect_err("unknown constant tag must fail");
    assert_eq!(error.kind(), SlangErrorKind::InvalidNativeData);

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
    let related = [RelatedDiagnostic {
        range: None,
        message: String::new(),
    }];
    let error = decode_diagnostics(&[diagnostic], &related, &[])
        .expect_err("out-of-bounds related window must fail");
    assert_eq!(error.kind(), SlangErrorKind::InvalidNativeData);
}

#[test]
fn malformed_type_component_and_windows_are_rejected() {
    let mut ty = raw_type(0);
    ty.element_type_id = 17;
    let error = decode_types(&[ty], &[], &[], 0).expect_err("unknown element type must fail");
    assert_eq!(error.kind(), SlangErrorKind::InvalidNativeData);

    let mut ty = raw_type(0);
    ty.range_count = 1;
    let error =
        decode_types(&[ty], &[], &[], 0).expect_err("out-of-bounds type range window must fail");
    assert_eq!(error.kind(), SlangErrorKind::InvalidNativeData);

    let mut ty = raw_type(0);
    ty.member_count = 1;
    let member = RawTypeMember {
        initializer_constant_id: INVALID_ID,
        name: empty_raw_string(),
        type_id: 9,
        bit_offset: 0,
        bit_width: 1,
    };
    let error = decode_types(&[ty], &[], &[member], 0).expect_err("unknown member type must fail");
    assert_eq!(error.kind(), SlangErrorKind::InvalidNativeData);
    let member = RawTypeMember {
        type_id: 0,
        initializer_constant_id: 0,
        ..member
    };
    let error = decode_types(&[ty], &[], &[member], 0)
        .expect_err("unknown member initializer constant must fail");
    assert_eq!(error.kind(), SlangErrorKind::InvalidNativeData);
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
    let edges = decode_semantic_edges(&raw_edges, std::slice::from_ref(&node))
        .expect("edge records decode before per-owner validation");
    let error = decode_semantic_nodes(&[node], &edges, &[], &[], 0)
        .expect_err("duplicate role/index pair must fail");
    assert_eq!(error.kind(), SlangErrorKind::InvalidNativeData);
}

#[test]
fn invalid_semantic_strength_and_port_state_are_rejected() {
    let mut node = raw_semantic_node(0);
    node.strength0 = 6;
    let error = decode_semantic_nodes(&[node], &[], &[], &[], 0)
        .expect_err("unknown drive strength must fail");
    assert_eq!(error.kind(), SlangErrorKind::InvalidNativeData);

    let mut node = raw_semantic_node(0);
    node.flags = 1 << 29;
    let error = decode_semantic_nodes(&[node], &[], &[], &[], 0)
        .expect_err("open port state without a present connection must fail");
    assert_eq!(error.kind(), SlangErrorKind::InvalidNativeData);

    let mut node = raw_semantic_node(0);
    node.flags = 1 << 31;
    let error = decode_semantic_nodes(&[node], &[], &[], &[], 0)
        .expect_err("method with-clause flag on a scope must fail");
    assert_eq!(error.kind(), SlangErrorKind::InvalidNativeData);

    let mut node = raw_semantic_node(0);
    node.subkind = 32;
    let error = decode_semantic_nodes(&[node], &[], &[], &[], 0)
        .expect_err("statement subkind on a scope must fail");
    assert_eq!(error.kind(), SlangErrorKind::InvalidNativeData);

    let mut node = raw_semantic_node(0);
    node.time_unit = 4;
    node.time_unit_magnitude = 1;
    node.time_precision_unit = 2;
    node.time_precision_magnitude = 1;
    let error = decode_semantic_nodes(&[node], &[], &[], &[], 0)
        .expect_err("coarser time precision must fail");
    assert_eq!(error.kind(), SlangErrorKind::InvalidNativeData);

    let mut node = raw_semantic_node(0);
    node.auxiliary = 1;
    let error = decode_semantic_nodes(&[node], &[], &[], &[], 0)
        .expect_err("variable lifetime metadata on a scope must fail");
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
    let decoded = decode_semantic_nodes(&[method], &[], &[], &[], 0)
        .expect("method with-clause flag must decode on a method call");
    assert!(decoded[0].method_with_clause);

    let mut qualified = raw_semantic_node(0);
    qualified.kind = 18;
    qualified.subkind = 33;
    qualified.auxiliary = SEMANTIC_UNIQUE_PRIORITY_PRIORITY;
    let decoded = decode_semantic_nodes(&[qualified], &[], &[], &[], 0)
        .expect("all repository-owned conditional qualifier tags must decode");
    assert_eq!(decoded[0].auxiliary, SEMANTIC_UNIQUE_PRIORITY_PRIORITY);
    qualified.auxiliary = SEMANTIC_UNIQUE_PRIORITY_PRIORITY + 1;
    let error = decode_semantic_nodes(&[qualified], &[], &[], &[], 0)
        .expect_err("unknown conditional qualifier must fail");
    assert_eq!(error.kind(), SlangErrorKind::InvalidNativeData);

    let mut variable = raw_semantic_node(0);
    variable.kind = 9;
    variable.auxiliary = 2;
    let decoded = decode_semantic_nodes(&[variable], &[], &[], &[], 0)
        .expect("resolved variable lifetime must decode");
    assert_eq!(decoded[0].auxiliary, 2);

    variable.kind = 11;
    let decoded = decode_semantic_nodes(&[variable], &[], &[], &[], 0)
        .expect("named-event variable lifetime must decode");
    assert_eq!(decoded[0].auxiliary, 2);

    let mut clocking_block = raw_semantic_node(0);
    clocking_block.kind = 25;
    clocking_block.subkind = SEMANTIC_SCOPE_CLOCKING_BLOCK;
    clocking_block.auxiliary = CLOCKING_BLOCK_DEFAULT
        | CLOCKING_BLOCK_GLOBAL
        | (3 << CLOCKING_INPUT_EDGE_SHIFT)
        | (2 << CLOCKING_OUTPUT_EDGE_SHIFT);
    let decoded = decode_semantic_nodes(&[clocking_block], &[], &[], &[], 0)
        .expect("clocking block metadata must decode");
    assert_eq!(decoded[0].auxiliary, clocking_block.auxiliary);

    let mut clocking_var = raw_semantic_node(0);
    clocking_var.kind = 9;
    clocking_var.subkind = SEMANTIC_VARIABLE_CLOCKING;
    clocking_var.auxiliary = 1 | (3 << CLOCKING_VAR_OUTPUT_EDGE_SHIFT);
    let decoded = decode_semantic_nodes(&[clocking_var], &[], &[], &[], 0)
        .expect("clocking variable metadata must decode");
    assert_eq!(decoded[0].auxiliary, clocking_var.auxiliary);

    variable.auxiliary = 3;
    let error = decode_semantic_nodes(&[variable], &[], &[], &[], 0)
        .expect_err("unknown resolved variable lifetime must fail");
    assert_eq!(error.kind(), SlangErrorKind::InvalidNativeData);

    let owner = raw_semantic_node(2);
    let edges = decode_semantic_edges(
        &[
            RawSemanticEdge {
                role: 30,
                index: 0,
                target_id: 0,
                sequence_delay_valid: 0,
                sequence_delay_min: 0,
                sequence_delay_max: 0,
            },
            RawSemanticEdge {
                role: 31,
                index: 0,
                target_id: 0,
                sequence_delay_valid: 0,
                sequence_delay_min: 0,
                sequence_delay_max: 0,
            },
        ],
        std::slice::from_ref(&owner),
    )
    .expect("semantic identity roles must decode");
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
