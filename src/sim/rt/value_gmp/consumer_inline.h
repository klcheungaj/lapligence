#ifndef LLG_SV4_COMPACT_CONSUMER_INLINE_H
#define LLG_SV4_COMPACT_CONSUMER_INLINE_H
/* Scanner mutations keep the common inline owner entirely in registers. */
static inline void llg_gmp_sv4_mask_top(g4_t* value) {
    if (value->width > 64) {
        llg_gmp_sv4_mask_top_wide(value);
        return;
    }
    uint64_t mask = g4_mask(value->width);
    value->data.small.a &= mask;
    value->data.small.b &= mask;
}
static inline void llg_gmp_sv4_mul_add_known(g4_t* value, uint32_t factor, uint32_t addend) {
    if (value->width > 64) {
        llg_gmp_sv4_mul_add_known_wide(value, factor, addend);
        return;
    }
    value->data.small.a = (value->data.small.a * factor + addend) & g4_mask(value->width);
}
static inline void llg_gmp_sv4_negate_known(g4_t* value) {
    if (value->width > 64) {
        llg_gmp_sv4_negate_known_wide(value);
        return;
    }
    value->data.small.a = (UINT64_C(0) - value->data.small.a) & g4_mask(value->width);
}
static inline void llg_gmp_sv4_two_state_inplace(g4_t* value) {
    if (value->width > 64) {
        llg_gmp_sv4_two_state_inplace_wide(value);
        return;
    }
    value->data.small.a &= ~value->data.small.b;
    value->data.small.b = 0;
}
static inline void llg_gmp_sv4_append_digit(g4_t* value, unsigned count, unsigned state,
                                            unsigned digit) {
    if (value->width > 64) {
        llg_gmp_sv4_append_digit_wide(value, count, state, digit);
        return;
    }
    uint64_t mask = g4_mask(count), top = g4_mask(value->width);
    uint64_t a = state == 0 ? digit & mask : state == 2 ? mask : 0;
    uint64_t b = state == 2 || state == 3 ? mask : 0;
    value->data.small.a = ((value->data.small.a << count) | a) & top;
    value->data.small.b = ((value->data.small.b << count) | b) & top;
}
#endif
