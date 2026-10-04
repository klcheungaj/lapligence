//! Instance, parameter, type and constant record receivers.
//!
//! Type and instance IDs are their dense table indices, so references to
//! records that arrive later are checked against the announced table count.

use super::*;

/// Decode one instance at table position `index` and claim its parameter
/// window; parameters precede instances in the stream.
pub(super) fn decode_instance(
    item: &RawInstance,
    index: u64,
    instance_count: u64,
    files: &[File],
    parameters: &[Parameter],
    claimed_parameters: &mut [bool],
) -> Result<Instance, SlangError> {
    if item.id != index {
        return Err(invalid_native("instance id is not its dense table index"));
    }
    if item.reserved != 0 {
        return Err(invalid_native("instance reserved field is nonzero"));
    }
    let parent_id = (item.parent_id != INVALID_ID).then_some(item.parent_id);
    if parent_id.is_some_and(|id| id >= instance_count) {
        return Err(invalid_native("instance parent does not exist"));
    }
    let window = checked_window(
        item.parameter_start,
        item.parameter_count,
        parameters.len(),
        "instance parameters",
    )?;
    for index in window {
        if claimed_parameters[index] || parameters[index].owner_instance_id != item.id {
            return Err(invalid_native(
                "instance parameter windows overlap or contain the wrong owner",
            ));
        }
        claimed_parameters[index] = true;
    }
    Ok(Instance {
        id: item.id,
        parent_id,
        kind: match item.kind {
            1 => InstanceKind::Module,
            2 => InstanceKind::Interface,
            3 => InstanceKind::Program,
            255 => InstanceKind::Unknown,
            _ => return Err(invalid_native("instance has an unknown kind")),
        },
        // SAFETY: stream records and their strings are valid for the callback
        // that delivered them.
        name: unsafe { copy_string(item.name, "instance name")? },
        // SAFETY: as above.
        definition_name: unsafe { copy_string(item.definition_name, "instance definition name")? },
        declaration: decode_range(item.declaration, files)?,
        parameter_start: item.parameter_start,
        parameter_count: item.parameter_count,
    })
}

pub(super) fn decode_type_range(range: &RawTypeRange) -> Result<TypeRange, SlangError> {
    if range.reserved != 0 {
        return Err(invalid_native("type range reserved field is nonzero"));
    }
    let kind = match range.kind {
        1 => TypeRangeKind::Packed,
        2 => TypeRangeKind::Unpacked,
        3 => TypeRangeKind::QueueBound,
        _ => return Err(invalid_native("type range has an unknown kind")),
    };
    Ok(TypeRange {
        left: range.left,
        right: range.right,
        kind,
    })
}

pub(super) fn decode_type_member(
    member: &RawTypeMember,
    type_count: u64,
    constant_len: usize,
) -> Result<TypeMember, SlangError> {
    if member.type_id >= type_count {
        return Err(invalid_native("type member refers to an unknown type"));
    }
    let initializer_constant_id =
        (member.initializer_constant_id != INVALID_ID).then_some(member.initializer_constant_id);
    if initializer_constant_id
        .is_some_and(|id| usize::try_from(id).map_or(true, |id| id >= constant_len))
    {
        return Err(invalid_native(
            "type member initializer refers to an unknown constant",
        ));
    }
    Ok(TypeMember {
        initializer_constant_id,
        // SAFETY: stream records and their strings are valid for the callback
        // that delivered them.
        name: unsafe { copy_string(member.name, "type member name")? },
        type_id: member.type_id,
        bit_offset: member.bit_offset,
        bit_width: member.bit_width,
    })
}

/// Decode one type at table position `index`, claiming its range and member
/// windows; ranges and members precede types in the stream.
pub(super) fn decode_type(
    item: &RawType,
    index: u64,
    type_count: u64,
    claimed_ranges: &mut [bool],
    claimed_members: &mut [bool],
) -> Result<Type, SlangError> {
    if item.id != index {
        return Err(invalid_native("type id is not its dense table index"));
    }
    if item.flags & !0b1111 != 0 {
        return Err(invalid_native("type contains unknown flags"));
    }
    for index in checked_window(
        item.range_start,
        item.range_count,
        claimed_ranges.len(),
        "type ranges",
    )? {
        if claimed_ranges[index] {
            return Err(invalid_native("type range windows overlap"));
        }
        claimed_ranges[index] = true;
    }
    for index in checked_window(
        item.member_start,
        item.member_count,
        claimed_members.len(),
        "type members",
    )? {
        if claimed_members[index] {
            return Err(invalid_native("type member windows overlap"));
        }
        claimed_members[index] = true;
    }
    let element_type_id = (item.element_type_id != INVALID_ID).then_some(item.element_type_id);
    let index_type_id = (item.index_type_id != INVALID_ID).then_some(item.index_type_id);
    if element_type_id.is_some_and(|id| id >= type_count)
        || index_type_id.is_some_and(|id| id >= type_count)
    {
        return Err(invalid_native("type refers to an unknown component type"));
    }
    Ok(Type {
        id: item.id,
        kind: match item.kind {
            1 => TypeKind::Integral,
            2 => TypeKind::Floating,
            3 => TypeKind::String,
            4 => TypeKind::Aggregate,
            5 => TypeKind::Enum,
            6 => TypeKind::PackedArray,
            7 => TypeKind::FixedUnpackedArray,
            8 => TypeKind::DynamicArray,
            9 => TypeKind::AssociativeArray,
            10 => TypeKind::Queue,
            11 => TypeKind::PackedStruct,
            12 => TypeKind::PackedUnion,
            13 => TypeKind::UnpackedStruct,
            14 => TypeKind::UnpackedUnion,
            15 => TypeKind::Class,
            16 => TypeKind::Chandle,
            17 => TypeKind::Event,
            18 => TypeKind::Void,
            19 => TypeKind::VirtualInterface,
            255 => TypeKind::Other,
            _ => return Err(invalid_native("type has an unknown kind")),
        },
        is_signed: item.flags & 1 != 0,
        is_four_state: item.flags & 2 != 0,
        is_fixed_size: item.flags & 4 != 0,
        is_tagged: item.flags & 8 != 0,
        bit_width: item.bit_width,
        // SAFETY: stream records and their strings are valid for the callback
        // that delivered them.
        display_name: unsafe { copy_string(item.display_name, "type display name")? },
        element_type_id,
        index_type_id,
        range_start: item.range_start,
        range_count: item.range_count,
        member_start: item.member_start,
        member_count: item.member_count,
    })
}

/// Decode one constant, copying its words out of the value-word table and
/// charging its width to the snapshot-wide `total_value_bits`.
pub(super) fn decode_constant(
    item: &RawConstant,
    words: &[u64],
    limits: &Limits,
    total_value_bits: &mut u64,
) -> Result<Constant, SlangError> {
    let value = match item.kind {
        0 => ConstantValue::None,
        1 => {
            if item.bit_width > limits.max_value_bits {
                return Err(invalid_native("constant width exceeds max_value_bits"));
            }
            *total_value_bits = total_value_bits
                .checked_add(item.bit_width)
                .ok_or_else(|| invalid_native("constant bit total overflowed"))?;
            if *total_value_bits > limits.max_value_bits {
                return Err(invalid_native(
                    "constant bits exceed the configured max_value_bits",
                ));
            }
            let expected_words = item.bit_width.div_ceil(64);
            if item.word_count != expected_words {
                return Err(invalid_native(
                    "integer constant word count does not match its width",
                ));
            }
            let value_range = checked_window(
                item.value_word_start,
                item.word_count,
                words.len(),
                "constant value words",
            )?;
            let unknown_range = checked_window(
                item.unknown_word_start,
                item.word_count,
                words.len(),
                "constant unknown words",
            )?;
            let value_words = words[value_range].to_vec();
            let unknown_words = words[unknown_range].to_vec();
            if !item.bit_width.is_multiple_of(64) && !value_words.is_empty() {
                let used = item.bit_width % 64;
                let tail_mask = !0_u64 << used;
                if value_words.last().is_some_and(|word| word & tail_mask != 0)
                    || unknown_words
                        .last()
                        .is_some_and(|word| word & tail_mask != 0)
                {
                    return Err(invalid_native("integer constant has nonzero tail bits"));
                }
            }
            ConstantValue::Integer {
                is_signed: match item.is_signed {
                    0 => false,
                    1 => true,
                    _ => return Err(invalid_native("constant signedness is not boolean")),
                },
                bit_width: item.bit_width,
                value_words,
                unknown_words,
            }
        }
        2 => {
            if item.bit_width != 64 {
                return Err(invalid_native("real constant does not have a 64-bit width"));
            }
            ConstantValue::Real(f64::from_bits(item.real_bits))
        }
        3 => {
            if item.bit_width != 32 || item.real_bits >> 32 != 0 {
                return Err(invalid_native(
                    "shortreal constant does not have a canonical 32-bit payload",
                ));
            }
            ConstantValue::ShortReal(f32::from_bits(item.real_bits as u32))
        }
        4 => ConstantValue::String(
            // SAFETY: stream records and their bytes are valid for the callback
            // that delivered them.
            unsafe { copy_bytes(item.text, "string constant")? },
        ),
        255 => ConstantValue::Other(
            // SAFETY: as above.
            unsafe { copy_string(item.text, "constant display text")? },
        ),
        _ => return Err(invalid_native("constant has an unknown kind")),
    };
    Ok(Constant { value })
}

pub(super) fn decode_parameter(
    item: &RawParameter,
    files: &[File],
    instance_count: u64,
    type_count: usize,
    constant_len: usize,
) -> Result<Parameter, SlangError> {
    if item.owner_instance_id >= instance_count {
        return Err(invalid_native("parameter owner instance does not exist"));
    }
    if item.flags & !0b11 != 0 {
        return Err(invalid_native("parameter contains unknown flags"));
    }
    let type_id = (item.type_id != INVALID_ID).then_some(item.type_id);
    if type_id.is_some_and(|id| usize::try_from(id).map_or(true, |id| id >= type_count)) {
        return Err(invalid_native("parameter type does not exist"));
    }
    let constant_id = (item.constant_id != INVALID_ID).then_some(item.constant_id);
    if constant_id.is_some_and(|id| usize::try_from(id).map_or(true, |id| id >= constant_len)) {
        return Err(invalid_native("parameter constant does not exist"));
    }
    Ok(Parameter {
        owner_instance_id: item.owner_instance_id,
        kind: match item.kind {
            1 => ParameterKind::Value,
            2 => ParameterKind::Type,
            _ => return Err(invalid_native("parameter has an unknown kind")),
        },
        is_local: item.flags & 1 != 0,
        is_port: item.flags & 2 != 0,
        // SAFETY: stream records and their strings are valid for the callback
        // that delivered them.
        name: unsafe { copy_string(item.name, "parameter name")? },
        declaration: decode_range(item.declaration, files)?,
        type_id,
        constant_id,
    })
}
