#ifndef LLG_VALUE_DESTINATIONS_H
#define LLG_VALUE_DESTINATIONS_H
/* Destination-passing forms of the value-returning packed operations.
 *
 * `sv4_<op>_to(dst, operands..., scalars...)` has exactly the semantics of
 * `sv4_replace(dst, sv4_<op>(*operands..., scalars...))`: dst must be an
 * initialized owner (SV4_EMPTY or a live value), operands are borrowed for the
 * call, and dst may alias any operand. Results are independent owners exactly
 * as for the returning form; an implementation may reuse dst's payload when
 * that cannot be observed through the borrowed operands.
 *
 * Generated code uses only these forms. A returned or by-value 32/24-byte
 * descriptor needs a caller stack temporary at every call site (returned
 * aggregates are passed in memory on SysV x86-64, AArch64 and Win64, and
 * by-value aggregates are copied to the caller's frame on AArch64 and Win64);
 * compilers do not reuse those temporaries, so a generated function's frame
 * would grow with its body length. Pointer operands and a pointer destination
 * keep every generated frame independent of the number of operations.
 *
 * These are additive declarations over the selected backend's public names;
 * they do not change the value descriptor ABI. */
/* Compact builds keep backend-prefixed link names, like the operations they
 * wrap, so a differential executable can link both backends. */
#if LLG_SV4_USE_GMP
#define sv4_zero_to llg_gmp_sv4_zero_to
#define sv4_x_to llg_gmp_sv4_x_to
#define sv4_fill_to llg_gmp_sv4_fill_to
#define sv4_from_u64_to llg_gmp_sv4_from_u64_to
#define sv4_from_i64_to llg_gmp_sv4_from_i64_to
#define sv4_from_masks_to llg_gmp_sv4_from_masks_to
#define sv4_from_limbs_to llg_gmp_sv4_from_limbs_to
#define sv4_from_real_to llg_gmp_sv4_from_real_to
#define sv4_rtoi_to llg_gmp_sv4_rtoi_to
#define sv4_realtobits_to llg_gmp_sv4_realtobits_to
#define sv4_shortrealtobits_to llg_gmp_sv4_shortrealtobits_to
#define sv4_udp_eval_to llg_gmp_sv4_udp_eval_to
#define sv4_cast_to llg_gmp_sv4_cast_to
#define sv4_resize_to llg_gmp_sv4_resize_to
#define sv4_to_two_state_to llg_gmp_sv4_to_two_state_to
#define sv4_neg_to llg_gmp_sv4_neg_to
#define sv4_bitneg_to llg_gmp_sv4_bitneg_to
#define sv4_lognot_to llg_gmp_sv4_lognot_to
#define sv4_reduce_and_to llg_gmp_sv4_reduce_and_to
#define sv4_reduce_nand_to llg_gmp_sv4_reduce_nand_to
#define sv4_reduce_or_to llg_gmp_sv4_reduce_or_to
#define sv4_reduce_nor_to llg_gmp_sv4_reduce_nor_to
#define sv4_reduce_xor_to llg_gmp_sv4_reduce_xor_to
#define sv4_reduce_xnor_to llg_gmp_sv4_reduce_xnor_to
#define sv4_clog2_to llg_gmp_sv4_clog2_to
#define sv4_countones_to llg_gmp_sv4_countones_to
#define sv4_onehot_to llg_gmp_sv4_onehot_to
#define sv4_repeat_count_to llg_gmp_sv4_repeat_count_to
#define sv4_repeat_to llg_gmp_sv4_repeat_to
#define sv4_stream_to llg_gmp_sv4_stream_to
#define sv4_unstream_to llg_gmp_sv4_unstream_to
#define sv4_part_select_to llg_gmp_sv4_part_select_to
#define sv4_bit_select_to llg_gmp_sv4_bit_select_to
#define sv4_idx_part_select_to llg_gmp_sv4_idx_part_select_to
#define sv4_idx_part_select_value_to llg_gmp_sv4_idx_part_select_value_to
#define sv4_select_plan_read_to llg_gmp_sv4_select_plan_read_to
#define sv4_select_plan_slice_to llg_gmp_sv4_select_plan_slice_to
#define sv4_add_to llg_gmp_sv4_add_to
#define sv4_sub_to llg_gmp_sv4_sub_to
#define sv4_mul_to llg_gmp_sv4_mul_to
#define sv4_div_to llg_gmp_sv4_div_to
#define sv4_mod_to llg_gmp_sv4_mod_to
#define sv4_pow_to llg_gmp_sv4_pow_to
#define sv4_and_to llg_gmp_sv4_and_to
#define sv4_or_to llg_gmp_sv4_or_to
#define sv4_xor_to llg_gmp_sv4_xor_to
#define sv4_xnor_to llg_gmp_sv4_xnor_to
#define sv4_logand_to llg_gmp_sv4_logand_to
#define sv4_logor_to llg_gmp_sv4_logor_to
#define sv4_logimpl_to llg_gmp_sv4_logimpl_to
#define sv4_logequiv_to llg_gmp_sv4_logequiv_to
#define sv4_shl_to llg_gmp_sv4_shl_to
#define sv4_shr_to llg_gmp_sv4_shr_to
#define sv4_ashl_to llg_gmp_sv4_ashl_to
#define sv4_ashr_to llg_gmp_sv4_ashr_to
#define sv4_eq_to llg_gmp_sv4_eq_to
#define sv4_neq_to llg_gmp_sv4_neq_to
#define sv4_case_eq_to llg_gmp_sv4_case_eq_to
#define sv4_case_neq_to llg_gmp_sv4_case_neq_to
#define sv4_wild_eq_to llg_gmp_sv4_wild_eq_to
#define sv4_wild_neq_to llg_gmp_sv4_wild_neq_to
#define sv4_casez_eq_to llg_gmp_sv4_casez_eq_to
#define sv4_casex_eq_to llg_gmp_sv4_casex_eq_to
#define sv4_lt_to llg_gmp_sv4_lt_to
#define sv4_le_to llg_gmp_sv4_le_to
#define sv4_gt_to llg_gmp_sv4_gt_to
#define sv4_ge_to llg_gmp_sv4_ge_to
#define sv4_concat_to llg_gmp_sv4_concat_to
#define sv4_mux_to llg_gmp_sv4_mux_to
#define sv4_inside_range_to llg_gmp_sv4_inside_range_to
#define sv4_array_conditional_merge_to llg_gmp_sv4_array_conditional_merge_to
#define sv4_enum_navigate_to llg_gmp_sv4_enum_navigate_to
#define llg_ref_read_to llg_gmp_ref_read_to
#endif

#ifdef __cplusplus
extern "C" {
#endif

/* Constructors. */
void sv4_zero_to(sv4_t* dst, uint32_t width, int8_t is_signed);
void sv4_x_to(sv4_t* dst, uint32_t width, int8_t is_signed);
void sv4_fill_to(sv4_t* dst, uint8_t bit, uint32_t width, int8_t is_signed);
void sv4_from_u64_to(sv4_t* dst, uint64_t v, uint32_t width, int8_t is_signed);
void sv4_from_i64_to(sv4_t* dst, int64_t v, uint32_t width);
void sv4_from_masks_to(sv4_t* dst, uint64_t bits, uint64_t x, uint64_t z,
                       uint32_t width, int8_t is_signed);
void sv4_from_limbs_to(sv4_t* dst, const uint64_t* bits, const uint64_t* x,
                       const uint64_t* z, uint32_t width, int8_t is_signed);
void sv4_from_real_to(sv4_t* dst, double v, uint32_t width, int8_t is_signed);
void sv4_rtoi_to(sv4_t* dst, double v);
void sv4_realtobits_to(sv4_t* dst, double v);
void sv4_shortrealtobits_to(sv4_t* dst, double v);
void sv4_udp_eval_to(sv4_t* dst, const uint8_t* rows, size_t row_count,
                     size_t input_count, const sv4_t* const* inputs);

/* Unary operations and conversions. */
void sv4_cast_to(sv4_t* dst, const sv4_t* v, uint32_t width, int8_t is_signed);
void sv4_resize_to(sv4_t* dst, const sv4_t* v, uint32_t width, int8_t is_signed);
void sv4_to_two_state_to(sv4_t* dst, const sv4_t* v);
void sv4_neg_to(sv4_t* dst, const sv4_t* a);
void sv4_bitneg_to(sv4_t* dst, const sv4_t* a);
void sv4_lognot_to(sv4_t* dst, const sv4_t* a);
void sv4_reduce_and_to(sv4_t* dst, const sv4_t* a);
void sv4_reduce_nand_to(sv4_t* dst, const sv4_t* a);
void sv4_reduce_or_to(sv4_t* dst, const sv4_t* a);
void sv4_reduce_nor_to(sv4_t* dst, const sv4_t* a);
void sv4_reduce_xor_to(sv4_t* dst, const sv4_t* a);
void sv4_reduce_xnor_to(sv4_t* dst, const sv4_t* a);
void sv4_clog2_to(sv4_t* dst, const sv4_t* v);
void sv4_countones_to(sv4_t* dst, const sv4_t* v);
void sv4_onehot_to(sv4_t* dst, const sv4_t* v, int allow_zero);
void sv4_repeat_count_to(sv4_t* dst, const sv4_t* v);
void sv4_repeat_to(sv4_t* dst, const sv4_t* pattern, uint64_t n);
void sv4_stream_to(sv4_t* dst, const sv4_t* v, uint32_t slice, int right_to_left);
void sv4_unstream_to(sv4_t* dst, const sv4_t* v, uint32_t slice, int right_to_left);

/* Selections. */
void sv4_part_select_to(sv4_t* dst, const sv4_t* v, int64_t left, int64_t right);
void sv4_bit_select_to(sv4_t* dst, const sv4_t* v, uint64_t index);
void sv4_idx_part_select_to(sv4_t* dst, const sv4_t* v, uint64_t base,
                            uint32_t width, int neg);
void sv4_idx_part_select_value_to(sv4_t* dst, const sv4_t* v, const sv4_t* base,
                                  uint32_t width, int neg);
void sv4_select_plan_read_to(sv4_t* dst, const sv4_t* source,
                             const sv4_select_plan_t* plan);
void sv4_select_plan_slice_to(sv4_t* dst, const sv4_t* source,
                              const sv4_select_plan_t* plan, int reverse);

/* Binary operations. */
void sv4_add_to(sv4_t* dst, const sv4_t* a, const sv4_t* b);
void sv4_sub_to(sv4_t* dst, const sv4_t* a, const sv4_t* b);
void sv4_mul_to(sv4_t* dst, const sv4_t* a, const sv4_t* b);
void sv4_div_to(sv4_t* dst, const sv4_t* a, const sv4_t* b);
void sv4_mod_to(sv4_t* dst, const sv4_t* a, const sv4_t* b);
void sv4_pow_to(sv4_t* dst, const sv4_t* a, const sv4_t* b);
void sv4_and_to(sv4_t* dst, const sv4_t* a, const sv4_t* b);
void sv4_or_to(sv4_t* dst, const sv4_t* a, const sv4_t* b);
void sv4_xor_to(sv4_t* dst, const sv4_t* a, const sv4_t* b);
void sv4_xnor_to(sv4_t* dst, const sv4_t* a, const sv4_t* b);
void sv4_logand_to(sv4_t* dst, const sv4_t* a, const sv4_t* b);
void sv4_logor_to(sv4_t* dst, const sv4_t* a, const sv4_t* b);
void sv4_logimpl_to(sv4_t* dst, const sv4_t* a, const sv4_t* b);
void sv4_logequiv_to(sv4_t* dst, const sv4_t* a, const sv4_t* b);
void sv4_shl_to(sv4_t* dst, const sv4_t* a, const sv4_t* b);
void sv4_shr_to(sv4_t* dst, const sv4_t* a, const sv4_t* b);
void sv4_ashl_to(sv4_t* dst, const sv4_t* a, const sv4_t* b);
void sv4_ashr_to(sv4_t* dst, const sv4_t* a, const sv4_t* b);
void sv4_eq_to(sv4_t* dst, const sv4_t* a, const sv4_t* b);
void sv4_neq_to(sv4_t* dst, const sv4_t* a, const sv4_t* b);
void sv4_case_eq_to(sv4_t* dst, const sv4_t* a, const sv4_t* b);
void sv4_case_neq_to(sv4_t* dst, const sv4_t* a, const sv4_t* b);
void sv4_wild_eq_to(sv4_t* dst, const sv4_t* a, const sv4_t* b);
void sv4_wild_neq_to(sv4_t* dst, const sv4_t* a, const sv4_t* b);
void sv4_casez_eq_to(sv4_t* dst, const sv4_t* a, const sv4_t* b);
void sv4_casex_eq_to(sv4_t* dst, const sv4_t* a, const sv4_t* b);
void sv4_lt_to(sv4_t* dst, const sv4_t* a, const sv4_t* b);
void sv4_le_to(sv4_t* dst, const sv4_t* a, const sv4_t* b);
void sv4_gt_to(sv4_t* dst, const sv4_t* a, const sv4_t* b);
void sv4_ge_to(sv4_t* dst, const sv4_t* a, const sv4_t* b);
void sv4_concat_to(sv4_t* dst, const sv4_t* hi, const sv4_t* lo);

/* Ternary and table operations. */
void sv4_mux_to(sv4_t* dst, const sv4_t* sel, const sv4_t* a, const sv4_t* b);
void sv4_inside_range_to(sv4_t* dst, const sv4_t* v, const sv4_t* low,
                         const sv4_t* high);
void sv4_array_conditional_merge_to(sv4_t* dst, const sv4_t* a, const sv4_t* b,
                                    const sv4_t* element_default);
void sv4_enum_navigate_to(sv4_t* dst, const sv4_t* current, const sv4_t* step,
                          const sv4_t* values, uint32_t count,
                          const sv4_t* default_value, int direction);

/* Reference read through a canonical descriptor (value/references.c). */
void llg_ref_read_to(sv4_t* dst, const llg_ref_t* ref);

#ifdef __cplusplus
}
#endif
#endif
