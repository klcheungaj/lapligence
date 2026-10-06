// Destination-passing packed operations. Private fragment of llg_value.c for
// both backends; it uses only public facade names, so each `_to` form keeps
// the selected backend's exact result semantics. Every form computes the
// complete independent result before replacing `dst`, so `dst` may alias any
// operand. The returned descriptor is a local of this out-of-line function,
// which keeps it out of generated frames (see value/destinations.h).

#define LLG_SV4_TO_UNARY(op)                                  \
    void sv4_##op##_to(sv4_t* dst, const sv4_t* a) {          \
        sv4_replace(dst, sv4_##op(*a));                       \
    }
#define LLG_SV4_TO_BINARY(op)                                          \
    void sv4_##op##_to(sv4_t* dst, const sv4_t* a, const sv4_t* b) {   \
        sv4_replace(dst, sv4_##op(*a, *b));                            \
    }

void sv4_zero_to(sv4_t* dst, uint32_t width, int8_t is_signed) {
    sv4_replace(dst, sv4_zero(width, is_signed));
}
void sv4_x_to(sv4_t* dst, uint32_t width, int8_t is_signed) {
    sv4_replace(dst, sv4_x(width, is_signed));
}
void sv4_fill_to(sv4_t* dst, uint8_t bit, uint32_t width, int8_t is_signed) {
    sv4_replace(dst, sv4_fill(bit, width, is_signed));
}
void sv4_from_u64_to(sv4_t* dst, uint64_t v, uint32_t width, int8_t is_signed) {
    sv4_replace(dst, sv4_from_u64(v, width, is_signed));
}
void sv4_from_i64_to(sv4_t* dst, int64_t v, uint32_t width) {
    sv4_replace(dst, sv4_from_i64(v, width));
}
void sv4_from_masks_to(sv4_t* dst, uint64_t bits, uint64_t x, uint64_t z,
                       uint32_t width, int8_t is_signed) {
    sv4_replace(dst, sv4_from_masks(bits, x, z, width, is_signed));
}
void sv4_from_limbs_to(sv4_t* dst, const uint64_t* bits, const uint64_t* x,
                       const uint64_t* z, uint32_t width, int8_t is_signed) {
    sv4_replace(dst, sv4_from_limbs(bits, x, z, width, is_signed));
}
void sv4_from_real_to(sv4_t* dst, double v, uint32_t width, int8_t is_signed) {
    sv4_replace(dst, sv4_from_real(v, width, is_signed));
}
void sv4_rtoi_to(sv4_t* dst, double v) { sv4_replace(dst, sv4_rtoi(v)); }
void sv4_realtobits_to(sv4_t* dst, double v) {
    sv4_replace(dst, sv4_realtobits(v));
}
void sv4_shortrealtobits_to(sv4_t* dst, double v) {
    sv4_replace(dst, sv4_shortrealtobits(v));
}
void sv4_udp_eval_to(sv4_t* dst, const uint8_t* rows, size_t row_count,
                     size_t input_count, const sv4_t* const* inputs) {
    sv4_replace(dst, sv4_udp_eval(rows, row_count, input_count, inputs));
}

void sv4_cast_to(sv4_t* dst, const sv4_t* v, uint32_t width, int8_t is_signed) {
    sv4_replace(dst, sv4_cast(*v, width, is_signed));
}
void sv4_resize_to(sv4_t* dst, const sv4_t* v, uint32_t width, int8_t is_signed) {
    sv4_replace(dst, sv4_resize(*v, width, is_signed));
}
LLG_SV4_TO_UNARY(to_two_state)
LLG_SV4_TO_UNARY(neg)
LLG_SV4_TO_UNARY(bitneg)
LLG_SV4_TO_UNARY(lognot)
LLG_SV4_TO_UNARY(reduce_and)
LLG_SV4_TO_UNARY(reduce_nand)
LLG_SV4_TO_UNARY(reduce_or)
LLG_SV4_TO_UNARY(reduce_nor)
LLG_SV4_TO_UNARY(reduce_xor)
LLG_SV4_TO_UNARY(reduce_xnor)
LLG_SV4_TO_UNARY(clog2)
LLG_SV4_TO_UNARY(countones)
LLG_SV4_TO_UNARY(repeat_count)
void sv4_onehot_to(sv4_t* dst, const sv4_t* v, int allow_zero) {
    sv4_replace(dst, sv4_onehot(*v, allow_zero));
}
void sv4_repeat_to(sv4_t* dst, const sv4_t* pattern, uint64_t n) {
    sv4_replace(dst, sv4_repeat(*pattern, n));
}
void sv4_stream_to(sv4_t* dst, const sv4_t* v, uint32_t slice, int right_to_left) {
    sv4_replace(dst, sv4_stream(*v, slice, right_to_left));
}
void sv4_unstream_to(sv4_t* dst, const sv4_t* v, uint32_t slice, int right_to_left) {
    sv4_replace(dst, sv4_unstream(*v, slice, right_to_left));
}

void sv4_part_select_to(sv4_t* dst, const sv4_t* v, int64_t left, int64_t right) {
    sv4_replace(dst, sv4_part_select(*v, left, right));
}
void sv4_bit_select_to(sv4_t* dst, const sv4_t* v, uint64_t index) {
    sv4_replace(dst, sv4_bit_select(*v, index));
}
void sv4_idx_part_select_to(sv4_t* dst, const sv4_t* v, uint64_t base,
                            uint32_t width, int neg) {
    sv4_replace(dst, sv4_idx_part_select(*v, base, width, neg));
}
void sv4_idx_part_select_value_to(sv4_t* dst, const sv4_t* v, const sv4_t* base,
                                  uint32_t width, int neg) {
    sv4_replace(dst, sv4_idx_part_select_value(*v, *base, width, neg));
}
void sv4_select_plan_read_to(sv4_t* dst, const sv4_t* source,
                             const sv4_select_plan_t* plan) {
    sv4_replace(dst, sv4_select_plan_read(*source, plan));
}
void sv4_select_plan_slice_to(sv4_t* dst, const sv4_t* source,
                              const sv4_select_plan_t* plan, int reverse) {
    sv4_replace(dst, sv4_select_plan_slice(*source, plan, reverse));
}

/* Add/sub/mul and the bitwise forms keep their same-width payload reuse
 * through the `_into` forms, which support exact destination/operand aliases. */
void sv4_add_to(sv4_t* dst, const sv4_t* a, const sv4_t* b) {
    sv4_add_into(dst, *a, *b);
}
void sv4_sub_to(sv4_t* dst, const sv4_t* a, const sv4_t* b) {
    sv4_sub_into(dst, *a, *b);
}
void sv4_mul_to(sv4_t* dst, const sv4_t* a, const sv4_t* b) {
    sv4_mul_into(dst, *a, *b);
}
LLG_SV4_TO_BINARY(div)
LLG_SV4_TO_BINARY(mod)
LLG_SV4_TO_BINARY(pow)
void sv4_and_to(sv4_t* dst, const sv4_t* a, const sv4_t* b) {
    sv4_and_into(dst, *a, *b);
}
void sv4_or_to(sv4_t* dst, const sv4_t* a, const sv4_t* b) {
    sv4_or_into(dst, *a, *b);
}
void sv4_xor_to(sv4_t* dst, const sv4_t* a, const sv4_t* b) {
    sv4_xor_into(dst, *a, *b);
}
void sv4_xnor_to(sv4_t* dst, const sv4_t* a, const sv4_t* b) {
    sv4_xnor_into(dst, *a, *b);
}
LLG_SV4_TO_BINARY(logand)
LLG_SV4_TO_BINARY(logor)
LLG_SV4_TO_BINARY(logimpl)
LLG_SV4_TO_BINARY(logequiv)
LLG_SV4_TO_BINARY(shl)
LLG_SV4_TO_BINARY(shr)
LLG_SV4_TO_BINARY(ashl)
LLG_SV4_TO_BINARY(ashr)
LLG_SV4_TO_BINARY(eq)
LLG_SV4_TO_BINARY(neq)
LLG_SV4_TO_BINARY(case_eq)
LLG_SV4_TO_BINARY(case_neq)
LLG_SV4_TO_BINARY(wild_eq)
LLG_SV4_TO_BINARY(wild_neq)
LLG_SV4_TO_BINARY(casez_eq)
LLG_SV4_TO_BINARY(casex_eq)
LLG_SV4_TO_BINARY(lt)
LLG_SV4_TO_BINARY(le)
LLG_SV4_TO_BINARY(gt)
LLG_SV4_TO_BINARY(ge)
LLG_SV4_TO_BINARY(concat)

void sv4_mux_to(sv4_t* dst, const sv4_t* sel, const sv4_t* a, const sv4_t* b) {
    sv4_replace(dst, sv4_mux(*sel, *a, *b));
}
void sv4_inside_range_to(sv4_t* dst, const sv4_t* v, const sv4_t* low,
                         const sv4_t* high) {
    sv4_replace(dst, sv4_inside_range(*v, *low, *high));
}
void sv4_array_conditional_merge_to(sv4_t* dst, const sv4_t* a, const sv4_t* b,
                                    const sv4_t* element_default) {
    sv4_replace(dst, sv4_array_conditional_merge(*a, *b, *element_default));
}
void sv4_enum_navigate_to(sv4_t* dst, const sv4_t* current, const sv4_t* step,
                          const sv4_t* values, uint32_t count,
                          const sv4_t* default_value, int direction) {
    sv4_replace(dst, sv4_enum_navigate(*current, *step, values, count,
                                       *default_value, direction));
}

void llg_ref_read_to(sv4_t* dst, const llg_ref_t* ref) {
    sv4_replace(dst, llg_ref_read(ref));
}

#undef LLG_SV4_TO_UNARY
#undef LLG_SV4_TO_BINARY
