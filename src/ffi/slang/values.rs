//! Values.

use super::*;

pub(super) fn decode_instances(
    raw: &[RawInstance],
    files: &[File],
    parameter_len: usize,
) -> Result<Vec<Instance>, SlangError> {
    let ids: HashSet<_> = raw.iter().map(|item| item.id).collect();
    if ids.len() != raw.len() || ids.contains(&INVALID_ID) {
        return Err(invalid_native(
            "snapshot contains duplicate or invalid instance ids",
        ));
    }
    raw.iter()
        .map(|item| {
            if item.reserved != 0 {
                return Err(invalid_native("instance reserved field is nonzero"));
            }
            let parent_id = (item.parent_id != INVALID_ID).then_some(item.parent_id);
            if parent_id.is_some_and(|id| !ids.contains(&id)) {
                return Err(invalid_native("instance parent does not exist"));
            }
            checked_window(
                item.parameter_start,
                item.parameter_count,
                parameter_len,
                "instance parameters",
            )?;
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
                // SAFETY: native strings borrow from the live snapshot.
                name: unsafe { copy_string(item.name, "instance name")? },
                // SAFETY: native strings borrow from the live snapshot.
                definition_name: unsafe {
                    copy_string(item.definition_name, "instance definition name")?
                },
                declaration: decode_range(item.declaration, files)?,
                parameter_start: item.parameter_start,
                parameter_count: item.parameter_count,
            })
        })
        .collect()
}

pub(super) type DecodedTypes = (Vec<Type>, Vec<TypeRange>, Vec<TypeMember>);

pub(super) fn decode_types(
    raw: &[RawType],
    raw_ranges: &[RawTypeRange],
    raw_members: &[RawTypeMember],
    constant_len: usize,
) -> Result<DecodedTypes, SlangError> {
    let ids: HashSet<_> = raw.iter().map(|item| item.id).collect();
    if ids.len() != raw.len() || ids.contains(&INVALID_ID) {
        return Err(invalid_native(
            "snapshot contains duplicate or invalid type ids",
        ));
    }

    let ranges = raw_ranges
        .iter()
        .map(|range| {
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
        })
        .collect::<Result<Vec<_>, _>>()?;
    let members = raw_members
        .iter()
        .map(|member| {
            if !ids.contains(&member.type_id) {
                return Err(invalid_native("type member refers to an unknown type"));
            }
            let initializer_constant_id = (member.initializer_constant_id != INVALID_ID)
                .then_some(member.initializer_constant_id);
            if initializer_constant_id
                .is_some_and(|id| usize::try_from(id).map_or(true, |id| id >= constant_len))
            {
                return Err(invalid_native(
                    "type member initializer refers to an unknown constant",
                ));
            }
            Ok(TypeMember {
                initializer_constant_id,
                // SAFETY: native strings borrow from the live snapshot.
                name: unsafe { copy_string(member.name, "type member name")? },
                type_id: member.type_id,
                bit_offset: member.bit_offset,
                bit_width: member.bit_width,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut claimed_ranges = vec![false; ranges.len()];
    let mut claimed_members = vec![false; members.len()];
    let types = raw
        .iter()
        .map(|item| {
            if item.flags & !0b111 != 0 {
                return Err(invalid_native("type contains unknown flags"));
            }
            for index in checked_window(
                item.range_start,
                item.range_count,
                ranges.len(),
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
                members.len(),
                "type members",
            )? {
                if claimed_members[index] {
                    return Err(invalid_native("type member windows overlap"));
                }
                claimed_members[index] = true;
            }
            let element_type_id =
                (item.element_type_id != INVALID_ID).then_some(item.element_type_id);
            let index_type_id = (item.index_type_id != INVALID_ID).then_some(item.index_type_id);
            if element_type_id.is_some_and(|id| !ids.contains(&id))
                || index_type_id.is_some_and(|id| !ids.contains(&id))
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
                bit_width: item.bit_width,
                // SAFETY: native strings borrow from the live snapshot.
                display_name: unsafe { copy_string(item.display_name, "type display name")? },
                element_type_id,
                index_type_id,
                range_start: item.range_start,
                range_count: item.range_count,
                member_start: item.member_start,
                member_count: item.member_count,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if claimed_ranges.iter().any(|claimed| !claimed) {
        return Err(invalid_native("type range is not owned by a type"));
    }
    if claimed_members.iter().any(|claimed| !claimed) {
        return Err(invalid_native("type member is not owned by a type"));
    }
    Ok((types, ranges, members))
}

pub(super) fn decode_constants(
    raw: &[RawConstant],
    words: &[u64],
    limits: &Limits,
) -> Result<Vec<Constant>, SlangError> {
    let mut total_value_bits = 0_u64;
    raw.iter()
        .map(|item| {
            let value = match item.kind {
                0 => ConstantValue::None,
                1 => {
                    if item.bit_width > limits.max_value_bits {
                        return Err(invalid_native("constant width exceeds max_value_bits"));
                    }
                    total_value_bits = total_value_bits
                        .checked_add(item.bit_width)
                        .ok_or_else(|| invalid_native("constant bit total overflowed"))?;
                    if total_value_bits > limits.max_value_bits {
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
                    if item.bit_width % 64 != 0 && !value_words.is_empty() {
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
                    // SAFETY: native bytes borrow from the live snapshot.
                    unsafe { copy_bytes(item.text, "string constant")? },
                ),
                255 => ConstantValue::Other(
                    // SAFETY: native strings borrow from the live snapshot.
                    unsafe { copy_string(item.text, "constant display text")? },
                ),
                _ => return Err(invalid_native("constant has an unknown kind")),
            };
            Ok(Constant { value })
        })
        .collect()
}

pub(super) fn decode_parameters(
    raw: &[RawParameter],
    files: &[File],
    instances: &[Instance],
    types: &[Type],
    constant_len: usize,
) -> Result<Vec<Parameter>, SlangError> {
    let instance_ids: HashSet<_> = instances.iter().map(|item| item.id).collect();
    let type_ids: HashSet<_> = types.iter().map(|item| item.id).collect();
    raw.iter()
        .map(|item| {
            if !instance_ids.contains(&item.owner_instance_id) {
                return Err(invalid_native("parameter owner instance does not exist"));
            }
            if item.flags & !0b11 != 0 {
                return Err(invalid_native("parameter contains unknown flags"));
            }
            let type_id = (item.type_id != INVALID_ID).then_some(item.type_id);
            if type_id.is_some_and(|id| !type_ids.contains(&id)) {
                return Err(invalid_native("parameter type does not exist"));
            }
            let constant_id = (item.constant_id != INVALID_ID).then_some(item.constant_id);
            if constant_id
                .is_some_and(|id| usize::try_from(id).map_or(true, |id| id >= constant_len))
            {
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
                // SAFETY: native strings borrow from the live snapshot.
                name: unsafe { copy_string(item.name, "parameter name")? },
                declaration: decode_range(item.declaration, files)?,
                type_id,
                constant_id,
            })
        })
        .collect()
}

pub(super) fn validate_parameter_windows(
    instances: &[Instance],
    parameters: &[Parameter],
) -> Result<(), SlangError> {
    let mut claimed = vec![false; parameters.len()];
    for instance in instances {
        let window = checked_window(
            instance.parameter_start,
            instance.parameter_count,
            parameters.len(),
            "instance parameters",
        )?;
        for index in window {
            if claimed[index] || parameters[index].owner_instance_id != instance.id {
                return Err(invalid_native(
                    "instance parameter windows overlap or contain the wrong owner",
                ));
            }
            claimed[index] = true;
        }
    }
    if claimed.iter().any(|claimed| !claimed) {
        return Err(invalid_native(
            "parameter record is not covered by its owner instance window",
        ));
    }
    Ok(())
}
