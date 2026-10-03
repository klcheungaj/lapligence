#ifndef LLG_COMPACT_PENDING_H
#define LLG_COMPACT_PENDING_H
/* Missing S2-S5 have declarations for runtime compilation, never definitions. */
#define sv4_select_plan_init llg_gmp_sv4_select_plan_init
sv4_select_plan_t sv4_select_plan_init(uint32_t storage_width);
#define sv4_select_plan_bit llg_gmp_sv4_select_plan_bit
sv4_select_plan_t sv4_select_plan_bit(uint32_t storage_width, uint64_t index);
#define sv4_select_plan_part llg_gmp_sv4_select_plan_part
sv4_select_plan_t sv4_select_plan_part(uint32_t storage_width,
                                       int64_t left, int64_t right);
#define sv4_select_plan_indexed llg_gmp_sv4_select_plan_indexed
sv4_select_plan_t sv4_select_plan_indexed(uint32_t storage_width, sv4_t base,
                                          uint32_t width, int negative);
#define sv4_select_plan_step llg_gmp_sv4_select_plan_step
void sv4_select_plan_step(sv4_select_plan_t* plan, sv4_t base, uint32_t width);
#define sv4_select_plan_read llg_gmp_sv4_select_plan_read
sv4_t sv4_select_plan_read(sv4_t source, const sv4_select_plan_t* plan);
#define sv4_select_plan_slice llg_gmp_sv4_select_plan_slice
sv4_t sv4_select_plan_slice(sv4_t source, const sv4_select_plan_t* plan,
                            int reverse);
#define sv4_select_plan_set llg_gmp_sv4_select_plan_set
void sv4_select_plan_set(sv4_t* destination, const sv4_select_plan_t* plan, sv4_t source);
#define sv4_countones llg_gmp_sv4_countones
sv4_t sv4_countones(sv4_t v);
#define sv4_onehot llg_gmp_sv4_onehot
sv4_t sv4_onehot(sv4_t v, int allow_zero);
#define sv4_repeat_count llg_gmp_sv4_repeat_count
sv4_t sv4_repeat_count(sv4_t v);
#define sv4_logimpl llg_gmp_sv4_logimpl
sv4_t sv4_logimpl(sv4_t a, sv4_t b);
#define sv4_logequiv llg_gmp_sv4_logequiv
sv4_t sv4_logequiv(sv4_t a, sv4_t b);
#define sv4_reduce_and llg_gmp_sv4_reduce_and
sv4_t sv4_reduce_and(sv4_t a);
#define sv4_reduce_nand llg_gmp_sv4_reduce_nand
sv4_t sv4_reduce_nand(sv4_t a);
#define sv4_reduce_or llg_gmp_sv4_reduce_or
sv4_t sv4_reduce_or(sv4_t a);
#define sv4_reduce_nor llg_gmp_sv4_reduce_nor
sv4_t sv4_reduce_nor(sv4_t a);
#define sv4_reduce_xor llg_gmp_sv4_reduce_xor
sv4_t sv4_reduce_xor(sv4_t a);
#define sv4_reduce_xnor llg_gmp_sv4_reduce_xnor
sv4_t sv4_reduce_xnor(sv4_t a);
#define sv4_shl llg_gmp_sv4_shl
sv4_t sv4_shl(sv4_t a, sv4_t b);
#define sv4_shr llg_gmp_sv4_shr
sv4_t sv4_shr(sv4_t a, sv4_t b);
#define sv4_ashl llg_gmp_sv4_ashl
sv4_t sv4_ashl(sv4_t a, sv4_t b);
#define sv4_ashr llg_gmp_sv4_ashr
sv4_t sv4_ashr(sv4_t a, sv4_t b);
#define sv4_wild_eq llg_gmp_sv4_wild_eq
sv4_t sv4_wild_eq(sv4_t lhs, sv4_t rhs);
#define sv4_wild_neq llg_gmp_sv4_wild_neq
sv4_t sv4_wild_neq(sv4_t lhs, sv4_t rhs);
#define sv4_casez_eq llg_gmp_sv4_casez_eq
sv4_t sv4_casez_eq(sv4_t sel, sv4_t item);
#define sv4_casex_eq llg_gmp_sv4_casex_eq
sv4_t sv4_casex_eq(sv4_t sel, sv4_t item);
#define sv4_inside_range llg_gmp_sv4_inside_range
sv4_t sv4_inside_range(sv4_t value, sv4_t low, sv4_t high);
#define sv4_array_conditional_merge llg_gmp_sv4_array_conditional_merge
sv4_t sv4_array_conditional_merge(sv4_t a, sv4_t b, sv4_t element_default);
#define sv4_concat llg_gmp_sv4_concat
sv4_t sv4_concat(sv4_t hi, sv4_t lo);
#define sv4_repeat llg_gmp_sv4_repeat
sv4_t sv4_repeat(sv4_t pat, uint64_t n);
#define sv4_stream llg_gmp_sv4_stream
sv4_t sv4_stream(sv4_t value, uint32_t slice, int right_to_left);
#define sv4_unstream llg_gmp_sv4_unstream
sv4_t sv4_unstream(sv4_t value, uint32_t slice, int right_to_left);
#define sv4_part_select llg_gmp_sv4_part_select
sv4_t sv4_part_select(sv4_t v, int64_t left, int64_t right);
#define sv4_part_select_set llg_gmp_sv4_part_select_set
void sv4_part_select_set(sv4_t* tgt, int64_t left, int64_t right, sv4_t value);
#define sv4_bit_select llg_gmp_sv4_bit_select
sv4_t sv4_bit_select(sv4_t v, uint64_t i);
#define sv4_bit_select_set llg_gmp_sv4_bit_select_set
void sv4_bit_select_set(sv4_t* tgt, uint64_t i, sv4_t value);
#define sv4_idx_part_select llg_gmp_sv4_idx_part_select
sv4_t sv4_idx_part_select(sv4_t v, uint64_t base, uint32_t width, int neg);
#define sv4_idx_part_select_set llg_gmp_sv4_idx_part_select_set
void sv4_idx_part_select_set(sv4_t* tgt, uint64_t base, uint32_t width, int neg,
                             sv4_t value);
#define sv4_idx_part_select_value llg_gmp_sv4_idx_part_select_value
sv4_t sv4_idx_part_select_value(sv4_t v, sv4_t base, uint32_t width, int neg);
#define sv4_idx_part_select_set_value llg_gmp_sv4_idx_part_select_set_value
void sv4_idx_part_select_set_value(sv4_t* tgt, sv4_t base, uint32_t width,
                                   int neg, sv4_t value);
#endif
