//! Snapshot.

use super::*;

pub(super) fn decode_snapshot(
    owner: &SnapshotOwner,
    limits: &Limits,
) -> Result<Snapshot, SlangError> {
    let mut view = RawSnapshotView {
        abi_version: 0,
        flags: 0,
        files: ptr::null(),
        file_count: 0,
        diagnostics: ptr::null(),
        diagnostic_count: 0,
        related_diagnostics: ptr::null(),
        related_diagnostic_count: 0,
        instances: ptr::null(),
        instance_count: 0,
        parameters: ptr::null(),
        parameter_count: 0,
        types: ptr::null(),
        type_count: 0,
        constants: ptr::null(),
        constant_count: 0,
        value_words: ptr::null(),
        value_word_count: 0,
        semantic_nodes: ptr::null(),
        semantic_node_count: 0,
        semantic_edges: ptr::null(),
        semantic_edge_count: 0,
        lexical_tokens: ptr::null(),
        lexical_token_count: 0,
        type_ranges: ptr::null(),
        type_range_count: 0,
        type_members: ptr::null(),
        type_member_count: 0,
    };
    let mut error = ptr::null_mut();
    // SAFETY: owner contains a live snapshot and output pointers are writable.
    let status = unsafe { llg_slang_snapshot_view(owner.0, &mut view, &mut error) };
    if status != STATUS_OK {
        return Err(take_native_error(status, error));
    }
    let unexpected_error = ErrorOwner(error);
    if !unexpected_error.0.is_null() {
        return Err(invalid_native(
            "successful snapshot view returned an unexpected error owner",
        ));
    }
    if view.abi_version != ABI_VERSION {
        return Err(invalid_native(format!(
            "Slang ABI version mismatch: expected {ABI_VERSION}, received {}",
            view.abi_version
        )));
    }
    if view.flags & !SNAPSHOT_KNOWN_FLAGS != 0 {
        return Err(invalid_native("snapshot contains unknown flags"));
    }
    enforce_count(view.file_count, limits.max_sources, "files")?;
    enforce_count(view.diagnostic_count, limits.max_diagnostics, "diagnostics")?;
    enforce_count(view.instance_count, limits.max_instances, "instances")?;
    enforce_count(view.parameter_count, limits.max_parameters, "parameters")?;
    enforce_count(view.type_count, limits.max_types, "types")?;
    enforce_count(
        view.semantic_node_count,
        limits.max_semantic_nodes,
        "semantic nodes",
    )?;
    enforce_count(
        view.semantic_edge_count,
        limits.max_semantic_edges,
        "semantic edges",
    )?;
    enforce_count(
        view.lexical_token_count,
        limits.max_lexical_tokens,
        "lexical tokens",
    )?;
    enforce_count(view.type_range_count, limits.max_type_ranges, "type ranges")?;
    enforce_count(
        view.type_member_count,
        limits.max_type_members,
        "type members",
    )?;
    enforce_count(
        view.related_diagnostic_count,
        limits.max_related_diagnostics,
        "related diagnostics",
    )?;
    // Each integer constant can require a partially filled word, so the
    // padded-word bound includes one extra word per constant and per plane.
    let max_words = limits
        .max_value_bits
        .div_ceil(64)
        .saturating_add(view.constant_count)
        .saturating_mul(2);
    enforce_count(view.value_word_count, max_words, "constant value words")?;
    enforce_count(view.constant_count, limits.max_constants, "constants")?;

    let mut output_bytes = 0_u64;
    for (count, size) in [
        (view.file_count, std::mem::size_of::<RawFile>()),
        (view.diagnostic_count, std::mem::size_of::<RawDiagnostic>()),
        (
            view.related_diagnostic_count,
            std::mem::size_of::<RawRelatedDiagnostic>(),
        ),
        (view.instance_count, std::mem::size_of::<RawInstance>()),
        (view.parameter_count, std::mem::size_of::<RawParameter>()),
        (view.type_count, std::mem::size_of::<RawType>()),
        (view.constant_count, std::mem::size_of::<RawConstant>()),
        (view.value_word_count, std::mem::size_of::<u64>()),
        (
            view.semantic_node_count,
            std::mem::size_of::<RawSemanticNode>(),
        ),
        (
            view.semantic_edge_count,
            std::mem::size_of::<RawSemanticEdge>(),
        ),
        (
            view.lexical_token_count,
            std::mem::size_of::<RawLexicalToken>(),
        ),
        (view.type_range_count, std::mem::size_of::<RawTypeRange>()),
        (view.type_member_count, std::mem::size_of::<RawTypeMember>()),
    ] {
        let bytes = count
            .checked_mul(size as u64)
            .ok_or_else(|| invalid_native("native output record byte count overflowed"))?;
        output_bytes = output_bytes
            .checked_add(bytes)
            .ok_or_else(|| invalid_native("native output byte count overflowed"))?;
    }
    if output_bytes > limits.max_output_bytes {
        return Err(invalid_native(
            "native output records exceed the configured max_output_bytes",
        ));
    }

    // SAFETY: the shim guarantees every nonempty view pointer references a
    // properly aligned initialized array owned by the live snapshot.
    let raw_files = unsafe { foreign_slice(view.files, view.file_count, "files")? };
    // SAFETY: same snapshot-view contract as above.
    let raw_related = unsafe {
        foreign_slice(
            view.related_diagnostics,
            view.related_diagnostic_count,
            "related diagnostics",
        )?
    };
    // SAFETY: same snapshot-view contract as above.
    let raw_diagnostics =
        unsafe { foreign_slice(view.diagnostics, view.diagnostic_count, "diagnostics")? };
    // SAFETY: same snapshot-view contract as above.
    let raw_instances = unsafe { foreign_slice(view.instances, view.instance_count, "instances")? };
    // SAFETY: same snapshot-view contract as above.
    let raw_parameters =
        unsafe { foreign_slice(view.parameters, view.parameter_count, "parameters")? };
    // SAFETY: same snapshot-view contract as above.
    let raw_types = unsafe { foreign_slice(view.types, view.type_count, "types")? };
    // SAFETY: same snapshot-view contract as above.
    let raw_constants = unsafe { foreign_slice(view.constants, view.constant_count, "constants")? };
    // SAFETY: same snapshot-view contract as above.
    let value_words = unsafe {
        foreign_slice(
            view.value_words,
            view.value_word_count,
            "constant value words",
        )?
    };
    // SAFETY: same snapshot-view contract as above.
    let raw_semantic_nodes = unsafe {
        foreign_slice(
            view.semantic_nodes,
            view.semantic_node_count,
            "semantic nodes",
        )?
    };
    // SAFETY: same snapshot-view contract as above.
    let raw_semantic_edges = unsafe {
        foreign_slice(
            view.semantic_edges,
            view.semantic_edge_count,
            "semantic edges",
        )?
    };
    // SAFETY: same snapshot-view contract as above.
    let raw_lexical_tokens = unsafe {
        foreign_slice(
            view.lexical_tokens,
            view.lexical_token_count,
            "lexical tokens",
        )?
    };
    // SAFETY: same snapshot-view contract as above.
    let raw_type_ranges =
        unsafe { foreign_slice(view.type_ranges, view.type_range_count, "type ranges")? };
    // SAFETY: same snapshot-view contract as above.
    let raw_type_members =
        unsafe { foreign_slice(view.type_members, view.type_member_count, "type members")? };

    for item in raw_files {
        charge_output_string(&mut output_bytes, item.name, limits.max_output_bytes)?;
    }
    for item in raw_diagnostics {
        for value in [item.name, item.option_name, item.message] {
            charge_output_string(&mut output_bytes, value, limits.max_output_bytes)?;
        }
    }
    for item in raw_related {
        charge_output_string(&mut output_bytes, item.message, limits.max_output_bytes)?;
    }
    for item in raw_instances {
        for value in [item.name, item.definition_name] {
            charge_output_string(&mut output_bytes, value, limits.max_output_bytes)?;
        }
    }
    for item in raw_parameters {
        charge_output_string(&mut output_bytes, item.name, limits.max_output_bytes)?;
    }
    for item in raw_types {
        charge_output_string(
            &mut output_bytes,
            item.display_name,
            limits.max_output_bytes,
        )?;
    }
    for item in raw_constants {
        charge_output_string(&mut output_bytes, item.text, limits.max_output_bytes)?;
    }
    for item in raw_semantic_nodes {
        for value in [item.name, item.detail, item.definition_name] {
            charge_output_string(&mut output_bytes, value, limits.max_output_bytes)?;
        }
    }
    for item in raw_lexical_tokens {
        charge_output_string(&mut output_bytes, item.text, limits.max_output_bytes)?;
    }
    for item in raw_type_members {
        charge_output_string(&mut output_bytes, item.name, limits.max_output_bytes)?;
    }

    let mut file_ids = HashSet::with_capacity(raw_files.len());
    let mut files = Vec::with_capacity(raw_files.len());
    for raw in raw_files {
        if raw.id == INVALID_ID || !file_ids.insert(raw.id) {
            return Err(invalid_native(
                "snapshot contains an invalid or duplicate file id",
            ));
        }
        if raw.byte_len > limits.max_source_bytes {
            return Err(invalid_native(
                "file byte length exceeds the configured limit",
            ));
        }
        files.push(File {
            id: raw.id,
            // SAFETY: native strings borrow from the live snapshot.
            name: unsafe { copy_string(raw.name, "file name")? },
            byte_len: raw.byte_len,
            text: String::new(),
        });
    }

    let related = decode_related(raw_related, &files)?;
    let diagnostics = decode_diagnostics(raw_diagnostics, &related, &files)?;
    let (types, type_ranges, type_members) =
        decode_types(raw_types, raw_type_ranges, raw_type_members)?;
    let constants = decode_constants(raw_constants, value_words, limits)?;
    let instances = decode_instances(raw_instances, &files, raw_parameters.len())?;
    let parameters =
        decode_parameters(raw_parameters, &files, &instances, &types, constants.len())?;
    validate_parameter_windows(&instances, &parameters)?;
    let semantic_edges = decode_semantic_edges(raw_semantic_edges, raw_semantic_nodes)?;
    let semantic_nodes = decode_semantic_nodes(
        raw_semantic_nodes,
        &semantic_edges,
        &files,
        &types,
        constants.len(),
    )?;
    let lexical_tokens = decode_lexical_tokens(raw_lexical_tokens, &files, &semantic_nodes)?;

    drop(unexpected_error);
    Ok(Snapshot {
        flags: view.flags,
        edition: LanguageEdition::from_snapshot_flags(view.flags)?,
        compilation_unit_mode: CompilationUnitMode::from_snapshot_flags(view.flags),
        files,
        diagnostics,
        instances,
        parameters,
        types,
        constants,
        semantic_nodes,
        semantic_edges,
        lexical_tokens,
        type_ranges,
        type_members,
    })
}
