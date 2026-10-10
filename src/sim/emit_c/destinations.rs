//! Destination-passing packed assignments.
//!
//! Packed runtime operations return 32-byte (legacy) or 24-byte (compact)
//! descriptors. Returned aggregates of that size travel through a caller stack
//! temporary on every supported ABI, and GCC/Clang/MSVC give each call site its
//! own temporary, so `sv4_replace(&dst, op(a, b))` statements make a generated
//! function's frame grow with its body length. Every packed result the emitter
//! installs therefore goes through [`assign`], which renders the documented
//! destination-passing form from `value/destinations.h`:
//! `sv4_xor_to(&dst, &a, &b)`. Packed operands are passed by address, so the
//! AArch64/Win64 by-value argument copies disappear as well.
//!
//! Runtime calls returning `sv4_t` or `llg_string_t` (containers, references,
//! strings, VPI, randomness) have the same destination forms, declared at the
//! end of their runtime headers; string results go through [`assign_string`].
//!
//! The emitter builds each producer from a fixed runtime signature, so the
//! translation recognizes exactly one call of a registered operation whose
//! packed operands are addressable descriptors. Anything else (a user function
//! or a runtime call without a destination form) keeps the returning form.

/// How one argument of a registered operation is passed in its `_to` form.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Param {
    /// A borrowed packed descriptor, passed by address.
    Packed,
    /// Any other argument, passed unchanged.
    Scalar,
    /// A consumed string owner `llg_string_take(p)`, passed as `p`; the
    /// destination form takes the owner from that address itself.
    Taken,
}

use Param::{Packed as P, Scalar as S, Taken as T};

/// Value-returning operations with a destination form, their `_to` name and
/// parameter kinds. Keep synchronized with `rt/value/destinations.h` and the
/// runtime headers that declare the non-value `_to` forms.
const OPERATIONS: &[(&str, &str, &[Param])] = &[
    ("sv4_zero", "sv4_zero_to", &[S, S]),
    ("sv4_x", "sv4_x_to", &[S, S]),
    ("sv4_fill", "sv4_fill_to", &[S, S, S]),
    ("sv4_from_u64", "sv4_from_u64_to", &[S, S, S]),
    ("sv4_from_i64", "sv4_from_i64_to", &[S, S]),
    ("sv4_from_masks", "sv4_from_masks_to", &[S, S, S, S, S]),
    ("SV4_INIT", "sv4_from_masks_to", &[S, S, S, S, S]),
    ("sv4_from_limbs", "sv4_from_limbs_to", &[S, S, S, S, S]),
    ("sv4_from_real", "sv4_from_real_to", &[S, S, S]),
    ("sv4_rtoi", "sv4_rtoi_to", &[S]),
    ("sv4_realtobits", "sv4_realtobits_to", &[S]),
    ("sv4_shortrealtobits", "sv4_shortrealtobits_to", &[S]),
    ("sv4_udp_eval", "sv4_udp_eval_to", &[S, S, S, S]),
    ("sv4_cast", "sv4_cast_to", &[P, S, S]),
    ("sv4_resize", "sv4_resize_to", &[P, S, S]),
    ("sv4_to_two_state", "sv4_to_two_state_to", &[P]),
    ("sv4_neg", "sv4_neg_to", &[P]),
    ("sv4_bitneg", "sv4_bitneg_to", &[P]),
    ("sv4_lognot", "sv4_lognot_to", &[P]),
    ("sv4_reduce_and", "sv4_reduce_and_to", &[P]),
    ("sv4_reduce_nand", "sv4_reduce_nand_to", &[P]),
    ("sv4_reduce_or", "sv4_reduce_or_to", &[P]),
    ("sv4_reduce_nor", "sv4_reduce_nor_to", &[P]),
    ("sv4_reduce_xor", "sv4_reduce_xor_to", &[P]),
    ("sv4_reduce_xnor", "sv4_reduce_xnor_to", &[P]),
    ("sv4_clog2", "sv4_clog2_to", &[P]),
    ("sv4_countones", "sv4_countones_to", &[P]),
    ("sv4_onehot", "sv4_onehot_to", &[P, S]),
    ("sv4_repeat_count", "sv4_repeat_count_to", &[P]),
    ("sv4_repeat", "sv4_repeat_to", &[P, S]),
    ("sv4_stream", "sv4_stream_to", &[P, S, S]),
    ("sv4_unstream", "sv4_unstream_to", &[P, S, S]),
    ("sv4_part_select", "sv4_part_select_to", &[P, S, S]),
    ("sv4_bit_select", "sv4_bit_select_to", &[P, S]),
    (
        "sv4_idx_part_select",
        "sv4_idx_part_select_to",
        &[P, S, S, S],
    ),
    (
        "sv4_idx_part_select_value",
        "sv4_idx_part_select_value_to",
        &[P, P, S, S],
    ),
    ("sv4_select_plan_read", "sv4_select_plan_read_to", &[P, S]),
    (
        "sv4_select_plan_slice",
        "sv4_select_plan_slice_to",
        &[P, S, S],
    ),
    ("sv4_add", "sv4_add_to", &[P, P]),
    ("sv4_sub", "sv4_sub_to", &[P, P]),
    ("sv4_mul", "sv4_mul_to", &[P, P]),
    ("sv4_div", "sv4_div_to", &[P, P]),
    ("sv4_mod", "sv4_mod_to", &[P, P]),
    ("sv4_pow", "sv4_pow_to", &[P, P]),
    ("sv4_and", "sv4_and_to", &[P, P]),
    ("sv4_or", "sv4_or_to", &[P, P]),
    ("sv4_xor", "sv4_xor_to", &[P, P]),
    ("sv4_xnor", "sv4_xnor_to", &[P, P]),
    ("sv4_logand", "sv4_logand_to", &[P, P]),
    ("sv4_logor", "sv4_logor_to", &[P, P]),
    ("sv4_logimpl", "sv4_logimpl_to", &[P, P]),
    ("sv4_logequiv", "sv4_logequiv_to", &[P, P]),
    ("sv4_shl", "sv4_shl_to", &[P, P]),
    ("sv4_shr", "sv4_shr_to", &[P, P]),
    ("sv4_ashl", "sv4_ashl_to", &[P, P]),
    ("sv4_ashr", "sv4_ashr_to", &[P, P]),
    ("sv4_eq", "sv4_eq_to", &[P, P]),
    ("sv4_neq", "sv4_neq_to", &[P, P]),
    ("sv4_case_eq", "sv4_case_eq_to", &[P, P]),
    ("sv4_case_neq", "sv4_case_neq_to", &[P, P]),
    ("sv4_wild_eq", "sv4_wild_eq_to", &[P, P]),
    ("sv4_wild_neq", "sv4_wild_neq_to", &[P, P]),
    ("sv4_casez_eq", "sv4_casez_eq_to", &[P, P]),
    ("sv4_casex_eq", "sv4_casex_eq_to", &[P, P]),
    ("sv4_lt", "sv4_lt_to", &[P, P]),
    ("sv4_le", "sv4_le_to", &[P, P]),
    ("sv4_gt", "sv4_gt_to", &[P, P]),
    ("sv4_ge", "sv4_ge_to", &[P, P]),
    ("sv4_concat", "sv4_concat_to", &[P, P]),
    ("sv4_mux", "sv4_mux_to", &[P, P, P]),
    ("sv4_inside_range", "sv4_inside_range_to", &[P, P, P]),
    (
        "sv4_array_conditional_merge",
        "sv4_array_conditional_merge_to",
        &[P, P, P],
    ),
    (
        "sv4_enum_navigate",
        "sv4_enum_navigate_to",
        &[P, P, S, S, P, S],
    ),
    ("llg_ref_read", "llg_ref_read_to", &[S]),
    // Model-local helpers emitted beside their returning forms.
    (
        "llg_owned_assoc_get_string",
        "llg_owned_assoc_get_string_to",
        &[S, S],
    ),
    (
        "llg_dpi_sv4_from_logic",
        "llg_dpi_sv4_from_logic_to",
        &[S, S],
    ),
    (
        "llg_fixed_array_compare",
        "llg_fixed_array_compare_to",
        &[S, S, S, S],
    ),
    (
        "llg_fixed_array_stream_source",
        "llg_fixed_array_stream_source_to",
        &[S, S, S, S, P, S, P, P],
    ),
    ("llg_net_alias_read", "llg_net_alias_read_to", &[S]),
    ("llg_q_full", "llg_q_full_to", &[P, S]),
    ("llg_urandom", "llg_urandom_to", &[]),
    ("llg_urandom_seed", "llg_urandom_seed_to", &[P]),
    ("llg_urandom_range", "llg_urandom_range_to", &[P, P, S]),
    (
        "llg_sequence_local_read",
        "llg_sequence_local_read_to",
        &[S, S],
    ),
    ("llg_system", "llg_system_to", &[T, S]),
    ("llg_frame_read_value", "llg_frame_read_value_to", &[S, S]),
    (
        "llg_sampled_domain_past",
        "llg_sampled_domain_past_to",
        &[S, S],
    ),
    ("llg_rt_ref_read", "llg_rt_ref_read_to", &[S]),
    (
        "llg_dyn_value_get_nested",
        "llg_dyn_value_get_nested_to",
        &[S, S, S],
    ),
    (
        "llg_fixed_stream_source",
        "llg_fixed_stream_source_to",
        &[S, S, S, S, P, S, P, P],
    ),
    (
        "llg_stream_unpack_source",
        "llg_stream_unpack_source_to",
        &[P, S, S, S],
    ),
    (
        "llg_fixed_image_stream_source",
        "llg_fixed_image_stream_source_to",
        &[P, S, S, S, P, S, P, P],
    ),
    ("llg_stream_to_fixed", "llg_stream_to_fixed_to", &[P, S, S]),
    (
        "llg_stream_cast_fixed",
        "llg_stream_cast_fixed_to",
        &[P, S, S],
    ),
    ("llg_queue_value_get", "llg_queue_value_get_to", &[S, P]),
    (
        "llg_queue_value_get_nested",
        "llg_queue_value_get_nested_to",
        &[S, S, S],
    ),
    ("llg_dyn_stream", "llg_dyn_stream_to", &[S, S, S, S, P, P]),
    ("llg_dyn_get", "llg_dyn_get_to", &[S, P]),
    ("llg_dyn_reduce", "llg_dyn_reduce_to", &[S, S]),
    (
        "llg_dyn_reduce_with",
        "llg_dyn_reduce_with_to",
        &[S, S, S, S, S, S, S],
    ),
    (
        "llg_queue_stream",
        "llg_queue_stream_to",
        &[S, S, S, S, P, P],
    ),
    ("llg_queue_get", "llg_queue_get_to", &[S, P]),
    ("llg_queue_pop_front", "llg_queue_pop_front_to", &[S]),
    ("llg_queue_pop_back", "llg_queue_pop_back_to", &[S]),
    ("llg_queue_front", "llg_queue_front_to", &[S]),
    ("llg_queue_back", "llg_queue_back_to", &[S]),
    ("llg_queue_reduce", "llg_queue_reduce_to", &[S, S]),
    (
        "llg_queue_reduce_with",
        "llg_queue_reduce_with_to",
        &[S, S, S, S, S, S, S],
    ),
    ("llg_queue_cell_read", "llg_queue_cell_read_to", &[S]),
    ("llg_queue_ref_read", "llg_queue_ref_read_to", &[S, S]),
    (
        "llg_assoc_value_get_integral",
        "llg_assoc_value_get_integral_to",
        &[S, P],
    ),
    (
        "llg_assoc_value_get_nested_integral",
        "llg_assoc_value_get_nested_integral_to",
        &[S, S, S],
    ),
    ("llg_assoc_value_at", "llg_assoc_value_at_to", &[S, S]),
    ("llg_assoc_reduce", "llg_assoc_reduce_to", &[S, S]),
    (
        "llg_assoc_reduce_with",
        "llg_assoc_reduce_with_to",
        &[S, S, S, S, S, S, S],
    ),
    (
        "llg_assoc_get_integral",
        "llg_assoc_get_integral_to",
        &[S, P],
    ),
    (
        "llg_assoc_get_string",
        "llg_assoc_get_string_to",
        &[S, S, S],
    ),
    (
        "llg_string_to_packed",
        "llg_string_to_packed_to",
        &[T, S, S],
    ),
    ("llg_string_len", "llg_string_len_to", &[T]),
    ("llg_string_getc", "llg_string_getc_to", &[T, P]),
    ("llg_string_compare", "llg_string_compare_to", &[T, T, S]),
    ("llg_string_atoi", "llg_string_atoi_to", &[T, S]),
    (
        "llg_vpi_call_function",
        "llg_vpi_call_function_to",
        &[S, S, S, S, S],
    ),
    (
        "llg_vpi_call_function_site",
        "llg_vpi_call_function_site_to",
        &[S, S, S, S, S, S],
    ),
];

/// String-returning operations with a destination form (`llg_string_t*`
/// destination). Keep synchronized with the runtime string declarations.
const STRING_OPERATIONS: &[(&str, &str, &[Param])] = &[
    (
        "llg_process_get_randstate",
        "llg_process_get_randstate_to",
        &[],
    ),
    (
        "llg_process_handle_get_randstate",
        "llg_process_handle_get_randstate_to",
        &[S],
    ),
    (
        "llg_string_format_typed",
        "llg_string_format_typed_to",
        &[T, S, S, S],
    ),
    (
        "llg_dyn_value_get_string",
        "llg_dyn_value_get_string_to",
        &[S, P],
    ),
    (
        "llg_dyn_value_get_nested_string",
        "llg_dyn_value_get_nested_string_to",
        &[S, S, S],
    ),
    (
        "llg_queue_value_get_string",
        "llg_queue_value_get_string_to",
        &[S, P],
    ),
    (
        "llg_queue_value_get_nested_string",
        "llg_queue_value_get_nested_string_to",
        &[S, S, S],
    ),
    (
        "llg_assoc_value_get_integral_string",
        "llg_assoc_value_get_integral_string_to",
        &[S, P],
    ),
    (
        "llg_assoc_value_get_nested_integral_string",
        "llg_assoc_value_get_nested_integral_string_to",
        &[S, S, S],
    ),
    (
        "llg_assoc_value_get_string",
        "llg_assoc_value_get_string_to",
        &[S, S, S],
    ),
    (
        "llg_assoc_value_get_string_string",
        "llg_assoc_value_get_string_string_to",
        &[S, S, S],
    ),
    ("llg_string_bytes", "llg_string_bytes_to", &[S, S]),
    ("llg_string_clone", "llg_string_clone_to", &[S]),
    ("llg_string_concat", "llg_string_concat_to", &[T, T]),
    ("llg_string_repeat", "llg_string_repeat_to", &[T, P]),
    ("llg_string_case", "llg_string_case_to", &[T, S]),
    ("llg_string_substr", "llg_string_substr_to", &[T, P, P]),
    ("llg_string_from_packed", "llg_string_from_packed_to", &[P]),
];

/// Render the C statement that stores `producer`, a C expression yielding a
/// fresh `llg_string_t` owner, into the expression owner at `destination`
/// (an address of an empty or expression-owned string, never persistent
/// storage with a change callback).
pub(in crate::sim::emit_c) fn assign_string(destination: &str, producer: &str) -> String {
    if let Some(statement) = table_form(STRING_OPERATIONS, destination, producer) {
        statement
    } else {
        format!("*({destination}) = {producer};")
    }
}

/// Render a notifying storage write of `producer` to the string at `target`
/// (`llg_string_move`), avoiding by-value string temporaries for owners that
/// are taken or copied from an address.
pub(in crate::sim::emit_c) fn move_string(target: &str, producer: &str) -> String {
    if let Some((name, arguments)) = single_call(producer.trim()) {
        if let [source] = arguments.as_slice() {
            match name {
                "llg_string_take" => return format!("llg_string_move_take({target}, {source});"),
                "llg_string_clone" => return format!("llg_string_assign({target}, {source});"),
                _ => {}
            }
        }
    }
    format!("llg_string_move({target}, {producer});")
}

/// Render the C statement that replaces the initialized packed owner at
/// `destination` (an address expression) with the independent result of
/// `producer`, a C expression yielding a fresh `sv4_t` owner.
pub(in crate::sim::emit_c) fn assign(destination: &str, producer: &str) -> String {
    if let Some(statement) = destination_form(destination, producer) {
        statement
    } else {
        format!("sv4_replace({destination}, {producer});")
    }
}

/// Whether [`assign`] renders `producer` without a returned descriptor.
#[cfg(test)]
pub(super) fn has_destination_form(producer: &str) -> bool {
    destination_form("&_llg_probe", producer).is_some()
}

fn destination_form(destination: &str, producer: &str) -> Option<String> {
    let (name, arguments) = single_call(producer.trim())?;
    if name == "sv4_clone" {
        // `sv4_clone` takes the source address already; its destination form
        // is the deep, self-copy-safe `sv4_copy`.
        let [source] = arguments.as_slice() else {
            return None;
        };
        return Some(format!("sv4_copy({destination}, {source});"));
    }
    table_form(OPERATIONS, destination, producer)
}

fn table_form(
    table: &[(&str, &str, &[Param])],
    destination: &str,
    producer: &str,
) -> Option<String> {
    let (name, arguments) = single_call(producer.trim())?;
    let (_, target, params) = table.iter().find(|(known, _, _)| *known == name)?;
    if params.len() != arguments.len() {
        return None;
    }
    let mut rendered = Vec::with_capacity(arguments.len() + 1);
    rendered.push(destination.to_owned());
    for (param, argument) in params.iter().zip(&arguments) {
        rendered.push(match param {
            Param::Scalar => (*argument).to_owned(),
            Param::Packed => address_of(argument)?,
            Param::Taken => taken_string(argument)?,
        });
    }
    Some(format!("{target}({});", rendered.join(", ")))
}

/// Split `name(arg, ...)` when the whole text is exactly one call, honoring
/// nested parentheses, brackets, braces and quoted literals.
fn single_call(text: &str) -> Option<(&str, Vec<&str>)> {
    let open = text.find('(')?;
    let name = &text[..open];
    if name.is_empty()
        || !name
            .bytes()
            .all(|byte| byte == b'_' || byte.is_ascii_alphanumeric())
        || name.as_bytes()[0].is_ascii_digit()
    {
        return None;
    }
    let bytes = text.as_bytes();
    let mut depth = 0usize;
    let mut quoted = None;
    let mut escaped = false;
    let mut arguments = Vec::new();
    let mut start = open + 1;
    for (index, &byte) in bytes.iter().enumerate().skip(open) {
        if let Some(quote) = quoted {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == quote {
                quoted = None;
            }
            continue;
        }
        match byte {
            b'\'' | b'"' => quoted = Some(byte),
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    // The closing parenthesis of the call must end the text.
                    if byte != b')' || index + 1 != bytes.len() {
                        return None;
                    }
                    let last = text[start..index].trim();
                    if !last.is_empty() || !arguments.is_empty() {
                        arguments.push(last);
                    }
                    return Some((name, arguments));
                }
            }
            b',' if depth == 1 => {
                arguments.push(text[start..index].trim());
                start = index + 1;
            }
            _ => {}
        }
    }
    None
}

/// Address of a packed operand that names a descriptor. `*(p)` yields `p`;
/// identifiers with member/index suffixes take `&`. Calls and other rvalues
/// are rejected so the caller keeps the returning form.
fn address_of(operand: &str) -> Option<String> {
    let operand = operand.trim();
    if let Some(inner) = operand
        .strip_prefix("*(")
        .and_then(|rest| rest.strip_suffix(')'))
    {
        if balanced(inner) {
            return Some(inner.to_owned());
        }
    }
    let operand = strip_parentheses(operand);
    let first = *operand.as_bytes().first()?;
    if !(first == b'_' || first.is_ascii_alphabetic()) || !designator(operand) {
        return None;
    }
    Some(format!("&{operand}"))
}

/// The owner address of `llg_string_take(p)`.
fn taken_string(argument: &str) -> Option<String> {
    let inner = argument
        .trim()
        .strip_prefix("llg_string_take(")?
        .strip_suffix(')')?;
    balanced(inner).then(|| inner.trim().to_owned())
}

fn strip_parentheses(mut text: &str) -> &str {
    while let Some(inner) = text
        .strip_prefix('(')
        .and_then(|rest| rest.strip_suffix(')'))
    {
        if !balanced(inner) {
            break;
        }
        text = inner.trim();
    }
    text
}

fn balanced(text: &str) -> bool {
    let mut depth = 0usize;
    for byte in text.bytes() {
        match byte {
            b'(' | b'[' => depth += 1,
            b')' | b']' => match depth.checked_sub(1) {
                Some(next) => depth = next,
                None => return false,
            },
            _ => {}
        }
    }
    depth == 0
}

/// An identifier followed only by `.member`, `->member` and `[index]`
/// suffixes; index expressions may be arbitrary balanced C.
fn designator(text: &str) -> bool {
    let bytes = text.as_bytes();
    let mut index = 0;
    let identifier = |index: &mut usize| {
        let start = *index;
        while *index < bytes.len()
            && (bytes[*index] == b'_' || bytes[*index].is_ascii_alphanumeric())
        {
            *index += 1;
        }
        *index > start
    };
    if !identifier(&mut index) {
        return false;
    }
    while index < bytes.len() {
        match bytes[index] {
            b'.' => {
                index += 1;
                if !identifier(&mut index) {
                    return false;
                }
            }
            b'-' if bytes.get(index + 1) == Some(&b'>') => {
                index += 2;
                if !identifier(&mut index) {
                    return false;
                }
            }
            b'[' => {
                let mut depth = 0usize;
                loop {
                    match bytes.get(index) {
                        Some(b'[') => depth += 1,
                        Some(b']') => {
                            depth -= 1;
                            if depth == 0 {
                                index += 1;
                                break;
                            }
                        }
                        Some(_) => {}
                        None => return false,
                    }
                    index += 1;
                }
            }
            _ => return false,
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binary_operands_pass_by_address() {
        assert_eq!(
            assign("&_llg_t[0]", "sv4_xor(_llg_t[0], _llg_t[1])"),
            "sv4_xor_to(&_llg_t[0], &_llg_t[0], &_llg_t[1]);"
        );
        assert_eq!(
            assign("&_llg_t[2]", "sv4_add(*(_llg_local_3), G_count)"),
            "sv4_add_to(&_llg_t[2], _llg_local_3, &G_count);"
        );
    }

    #[test]
    fn scalars_and_literal_arrays_stay_in_place() {
        assert_eq!(
            assign("&x", "sv4_cast(F->v.w[1], 4096, 0)"),
            "sv4_cast_to(&x, &F->v.w[1], 4096, 0);"
        );
        assert_eq!(
            assign("&x", "SV4_INIT(1ULL, 0ULL, 0ULL, 32, 1)"),
            "sv4_from_masks_to(&x, 1ULL, 0ULL, 0ULL, 32, 1);"
        );
        assert_eq!(
            assign(
                "&x",
                "sv4_from_limbs((uint64_t[]){1ULL, 2ULL}, (uint64_t[]){0ULL, 0ULL}, (uint64_t[]){0ULL, 0ULL}, 65, 0)"
            ),
            "sv4_from_limbs_to(&x, (uint64_t[]){1ULL, 2ULL}, (uint64_t[]){0ULL, 0ULL}, (uint64_t[]){0ULL, 0ULL}, 65, 0);"
        );
        assert_eq!(
            assign("&x", "sv4_part_select(a, (f(1, 2)), 0)"),
            "sv4_part_select_to(&x, &a, (f(1, 2)), 0);"
        );
    }

    #[test]
    fn clone_becomes_copy() {
        assert_eq!(
            assign("_llg_local_1", "sv4_clone(&G_a)"),
            "sv4_copy(_llg_local_1, &G_a);"
        );
    }

    #[test]
    fn unrecognized_producers_keep_the_returning_form() {
        for producer in [
            "fn_tb_f(_llg_t[0], depth + 1)",
            "sv4_xor(sv4_x(1, 0), a)",
            "sv4_xor(a)",
            "sv4_xor(a, b)[0]",
            "sv4_cast(\"a,b\", 1, 0)",
        ] {
            let rendered = assign("&d", producer);
            assert_eq!(
                rendered,
                format!("sv4_replace(&d, {producer});"),
                "{producer}"
            );
        }
        assert!(has_destination_form("sv4_xor(a, b)"));
    }

    #[test]
    fn consumed_strings_pass_their_owner_address() {
        assert_eq!(
            assign(
                "&_llg_t[0]",
                "llg_string_compare(llg_string_take(_llg_native_1), llg_string_take(F->s), 0)"
            ),
            "llg_string_compare_to(&_llg_t[0], _llg_native_1, F->s, 0);"
        );
        // A cloned operand is not consumed through its address.
        let cloned = "llg_string_compare(llg_string_clone(&s), llg_string_take(p), 0)";
        assert_eq!(assign("&d", cloned), format!("sv4_replace(&d, {cloned});"));
    }

    #[test]
    fn string_producers_and_storage_moves_avoid_returned_strings() {
        assert_eq!(
            assign_string(
                "_llg_native_3",
                "llg_string_concat(llg_string_take(_llg_native_1), llg_string_take(_llg_native_2))"
            ),
            "llg_string_concat_to(_llg_native_3, _llg_native_1, _llg_native_2);"
        );
        assert_eq!(
            assign_string("p", "llg_string_bytes(\"\\101,\", 2)"),
            "llg_string_bytes_to(p, \"\\101,\", 2);"
        );
        assert_eq!(
            assign_string("p", "user_string(1)"),
            "*(p) = user_string(1);"
        );
        assert_eq!(
            move_string("&G_s", "llg_string_take(_llg_native_1)"),
            "llg_string_move_take(&G_s, _llg_native_1);"
        );
        assert_eq!(
            move_string("o1", "llg_string_clone(_llg_local_2)"),
            "llg_string_assign(o1, _llg_local_2);"
        );
        assert_eq!(move_string("o1", "value"), "llg_string_move(o1, value);");
    }

    #[test]
    fn every_registered_destination_form_is_declared_by_the_runtime() {
        let headers = [
            include_str!("../rt/value/destinations.h"),
            include_str!("../rt/llg_rt.h"),
            include_str!("../rt/llg_container.h"),
            include_str!("../rt/llg_string.h"),
            include_str!("../rt/llg_vpi.h"),
        ]
        .concat();
        // Model-local helpers are emitted with the model.
        let local = ["llg_owned_assoc_get_string_to", "llg_dpi_sv4_from_logic_to"];
        for (_, target, params) in OPERATIONS.iter().chain(STRING_OPERATIONS) {
            if local.contains(target) {
                continue;
            }
            let declaration = headers
                .find(&format!("void {target}("))
                .unwrap_or_else(|| panic!("{target} is not declared by the runtime"));
            let declared = &headers[declaration..];
            let declared = &declared[..declared.find(';').expect("declaration end")];
            // Destination plus one runtime parameter per registered argument.
            let arity = declared.matches(',').count();
            assert_eq!(arity, params.len(), "{target}: {declared}");
        }
    }

    #[test]
    fn quoted_commas_do_not_split_scalar_arguments() {
        assert_eq!(
            assign("&d", "llg_ref_read(f(\"a,b\", ')'))"),
            "llg_ref_read_to(&d, f(\"a,b\", ')'));"
        );
    }
}
