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
        typedef: (member.typedef_id != INVALID_ID)
            .then(|| {
                u32::try_from(member.typedef_depth)
                    .map(|depth| (member.typedef_id, depth))
                    .map_err(|_| invalid_native("type member typedef depth is too large"))
            })
            .transpose()?,
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
            20 => TypeKind::DpiOpenUnpacked,
            21 => TypeKind::DpiOpenPacked,
            22 => TypeKind::Covergroup,
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

fn charge_constant_bits(
    bits: u64,
    limits: &Limits,
    total_value_bits: &mut u64,
) -> Result<(), SlangError> {
    *total_value_bits = total_value_bits
        .checked_add(bits)
        .ok_or_else(|| invalid_native("constant bit total overflowed"))?;
    if *total_value_bits > limits.max_value_bits {
        return Err(invalid_native(
            "constant bits exceed the configured max_value_bits",
        ));
    }
    Ok(())
}

fn integer_constant(
    is_signed: u64,
    bit_width: u64,
    value_words: &[u64],
    unknown_words: &[u64],
) -> Result<ConstantValue, SlangError> {
    if !bit_width.is_multiple_of(64) {
        let tail_mask = !0_u64 << (bit_width % 64);
        if [value_words, unknown_words]
            .iter()
            .any(|words| words.last().is_some_and(|word| word & tail_mask != 0))
        {
            return Err(invalid_native("integer constant has nonzero tail bits"));
        }
    }
    Ok(ConstantValue::Integer {
        is_signed: match is_signed {
            0 => false,
            1 => true,
            _ => return Err(invalid_native("constant signedness is not boolean")),
        },
        bit_width,
        value_words: value_words.to_vec(),
        unknown_words: unknown_words.to_vec(),
    })
}

fn words_kind(value: &ConstantValue) -> u32 {
    match value {
        ConstantValue::Elements(_) => 5,
        ConstantValue::Union { .. } => 6,
        _ => 255,
    }
}

/// Take one item of an `LLG_SLANG_VALUE_ELEMENTS` value off `words`.
fn decode_element_item(words: &mut &[u64]) -> Result<ConstantValue, SlangError> {
    fn take<'w>(words: &mut &'w [u64], count: u64) -> Result<&'w [u64], SlangError> {
        let (taken, rest) = usize::try_from(count)
            .ok()
            .and_then(|count| words.split_at_checked(count))
            .ok_or_else(|| invalid_native("constant element words are truncated"))?;
        *words = rest;
        Ok(taken)
    }
    fn word(words: &mut &[u64]) -> Result<u64, SlangError> {
        Ok(take(words, 1)?[0])
    }
    Ok(match word(words)? {
        1 => {
            let is_signed = word(words)?;
            let bit_width = word(words)?;
            let word_count = bit_width.div_ceil(64);
            let value_words = take(words, word_count)?;
            let unknown_words = take(words, word_count)?;
            integer_constant(is_signed, bit_width, value_words, unknown_words)?
        }
        2 => ConstantValue::Real(f64::from_bits(word(words)?)),
        3 => ConstantValue::ShortReal(f32::from_bits(
            u32::try_from(word(words)?)
                .map_err(|_| invalid_native("shortreal element exceeds 32 bits"))?,
        )),
        4 => {
            let length = word(words)?;
            let packed = take(words, length.div_ceil(8))?;
            ConstantValue::String(
                packed
                    .iter()
                    .flat_map(|word| word.to_le_bytes())
                    .take(usize::try_from(length).unwrap_or(usize::MAX))
                    .collect(),
            )
        }
        5 => {
            let count = word(words)?;
            // Every item takes at least one word.
            let capacity = usize::try_from(count)
                .ok()
                .filter(|count| *count <= words.len())
                .ok_or_else(|| invalid_native("constant element count exceeds its words"))?;
            let mut elements = Vec::with_capacity(capacity);
            for _ in 0..capacity {
                elements.push(decode_element_item(words)?);
            }
            ConstantValue::Elements(elements)
        }
        6 => ConstantValue::Union {
            member: usize::try_from(word(words)?)
                .map_err(|_| invalid_native("union member index exceeds usize"))?,
            value: Box::new(decode_element_item(words)?),
        },
        _ => return Err(invalid_native("constant element has an unknown kind")),
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
            charge_constant_bits(item.bit_width, limits, total_value_bits)?;
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
            integer_constant(
                u64::from(item.is_signed),
                item.bit_width,
                &words[value_range],
                &words[unknown_range],
            )?
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
        5 | 6 => {
            let range = checked_window(
                item.value_word_start,
                item.word_count,
                words.len(),
                "constant element words",
            )?;
            charge_constant_bits(item.word_count.saturating_mul(64), limits, total_value_bits)?;
            let mut words = &words[range];
            let value = decode_element_item(&mut words)?;
            if !words.is_empty() || words_kind(&value) != item.kind {
                return Err(invalid_native("constant elements are not one item"));
            }
            value
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn decode(words: &[u64]) -> Result<ConstantValue, SlangError> {
        let item = RawConstant {
            kind: words[0] as u32,
            is_signed: 0,
            bit_width: 0,
            value_word_start: 0,
            unknown_word_start: INVALID_ID,
            word_count: words.len() as u64,
            real_bits: 0,
            text: RawString {
                data: std::ptr::null(),
                len: 0,
            },
        };
        decode_constant(&item, words, &Limits::default(), &mut 0).map(|constant| constant.value)
    }

    #[test]
    fn element_constants_decode_every_kind_of_item() {
        let value = decode(&[
            5,
            4, // four members
            1,
            1,
            8,
            0x5a,
            0x02, // 8-bit signed, one unknown bit
            2,
            0x3ff8_0000_0000_0000, // 1.5
            4,
            9,
            0x6867_6665_6463_6261,
            0x69, // "abcdefghi"
            6,
            1,
            5,
            1,
            3,
            0x3f00_0000, // union member 1: one shortreal 0.5
        ])
        .expect("one item");
        assert_eq!(
            value,
            ConstantValue::Elements(vec![
                ConstantValue::Integer {
                    is_signed: true,
                    bit_width: 8,
                    value_words: vec![0x5a],
                    unknown_words: vec![0x02],
                },
                ConstantValue::Real(1.5),
                ConstantValue::String(b"abcdefghi".to_vec()),
                ConstantValue::Union {
                    member: 1,
                    value: Box::new(ConstantValue::Elements(vec![ConstantValue::ShortReal(0.5)])),
                },
            ])
        );
    }

    #[test]
    fn malformed_element_constants_are_rejected() {
        for words in [
            &[5u64, 1][..],             // a missing element
            &[5, u64::MAX, 2, 0],       // more elements than words
            &[5, 1, 2, 0, 0],           // words after the item
            &[5, 1, 1, 0, 4, 0xff, 0],  // bits beyond the width
            &[5, 1, 1, 2, 4, 0, 0],     // a signedness that is no flag
            &[5, 1, 1, 0, u64::MAX, 0], // a width beyond the words
            &[5, 1, 3, u64::MAX],       // a wide shortreal
            &[5, 1, 4, 9, 0],           // truncated text
            &[5, 1, 7, 0],              // an unknown kind
            &[6, 0],                    // a union without a value
        ] {
            assert!(decode(words).is_err(), "{words:?}");
        }
    }
}
