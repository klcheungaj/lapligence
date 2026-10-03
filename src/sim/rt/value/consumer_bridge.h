#ifndef LLG_VALUE_CONSUMER_BRIDGE_H
#define LLG_VALUE_CONSUMER_BRIDGE_H
static inline void llg_sv4_masked_merge(sv4_t* target, sv4_t source, sv4_t mask) {
    size_t count = llg_sv4_words(*target);
    if (count > llg_sv4_words(source)) count = llg_sv4_words(source);
    if (count > llg_sv4_words(mask)) count = llg_sv4_words(mask);
    for (size_t i = 0; i < count; ++i) {
        uint64_t selected = mask.bits[i];
        if (!selected) continue;
        target->bits[i] = (target->bits[i] & ~selected) | (source.bits[i] & selected);
        target->x[i] = (target->x[i] & ~selected) | (source.x[i] & selected);
        target->z[i] = (target->z[i] & ~selected) | (source.z[i] & selected);
    }
}

// Known-one masks and payload intervals are clipped to actual operand widths.
static inline int llg_sv4_masked_same(sv4_t a, sv4_t b, const sv4_t* mask) {
    uint32_t width = llg_sv4_width(a) < llg_sv4_width(b) ? llg_sv4_width(a) : llg_sv4_width(b);
    size_t words = ((size_t)width + 63u) / 64u;
    for (size_t i = 0; i < words; ++i) {
        uint64_t selected = mask ? llg_sv4_word(*mask, i, LLG_SV4_BITS) &
            ~(llg_sv4_word(*mask, i, LLG_SV4_X) | llg_sv4_word(*mask, i, LLG_SV4_Z)) : UINT64_MAX;
        if (i + 1u == words && width % 64u)
            selected &= UINT64_MAX >> (64u - width % 64u);
        if (((a.bits[i] ^ b.bits[i]) | (a.x[i] ^ b.x[i]) | (a.z[i] ^ b.z[i])) & selected)
            return 0;
    }
    return 1;
}
static inline void llg_sv4_masked_copy(sv4_t* target, sv4_t source, sv4_t mask) {
    uint32_t width = llg_sv4_width(*target) < llg_sv4_width(source) ? llg_sv4_width(*target) : llg_sv4_width(source);
    size_t words = ((size_t)width + 63u) / 64u;
    for (size_t i = 0; i < words; ++i) {
        uint64_t selected = llg_sv4_word(mask, i, LLG_SV4_BITS) &
            ~(llg_sv4_word(mask, i, LLG_SV4_X) | llg_sv4_word(mask, i, LLG_SV4_Z));
        if (i + 1u == words && width % 64u)
            selected &= UINT64_MAX >> (64u - width % 64u);
        if (!selected) continue;
        target->bits[i] = (target->bits[i] & ~selected) | (source.bits[i] & selected);
        target->x[i] = (target->x[i] & ~selected) | (source.x[i] & selected);
        target->z[i] = (target->z[i] & ~selected) | (source.z[i] & selected);
    }
}

// Internal slice load, padded with zero; count is in 1..64.
static inline uint64_t llg_sv4_plane_slice(sv4_t value, uint32_t low,
                                          unsigned count, unsigned plane) {
    size_t word = low / 64u;
    unsigned shift = low % 64u;
    uint64_t result = llg_sv4_word(value, word, plane) >> shift;
    if (shift && count > 64u - shift)
        result |= llg_sv4_word(value, word + 1u, plane) << (64u - shift);
    return result & (UINT64_MAX >> (64u - count));
}

// Ranges must fit. Inputs are independent owners, or an exact self-copy at offset zero.
static inline int llg_sv4_range_same(sv4_t target, uint32_t offset, sv4_t source) {
    for (uint32_t bit = 0; bit < llg_sv4_width(source);) {
        unsigned count = llg_sv4_width(source) - bit < 64u ? llg_sv4_width(source) - bit : 64u;
        for (unsigned plane = LLG_SV4_BITS; plane <= LLG_SV4_Z; ++plane)
            if (llg_sv4_plane_slice(target, offset + bit, count, plane) !=
                llg_sv4_word(source, bit / 64u, plane)) return 0;
        bit += count;
    }
    return 1;
}
static inline void llg_sv4_range_copy(sv4_t* target, uint32_t offset, sv4_t source) {
    for (uint32_t bit = 0; bit < llg_sv4_width(source);) {
        uint32_t low = offset + bit;
        unsigned count = 64u - low % 64u;
        if (count > llg_sv4_width(source) - bit) count = llg_sv4_width(source) - bit;
        uint64_t selected = (UINT64_MAX >> (64u - count)) << (low % 64u);
        size_t word = low / 64u;
        uint64_t bits = llg_sv4_plane_slice(source, bit, count, LLG_SV4_BITS);
        uint64_t x = llg_sv4_plane_slice(source, bit, count, LLG_SV4_X);
        uint64_t z = llg_sv4_plane_slice(source, bit, count, LLG_SV4_Z);
        target->bits[word] = (target->bits[word] & ~selected) | (bits << (low % 64u));
        target->x[word] = (target->x[word] & ~selected) | (x << (low % 64u));
        target->z[word] = (target->z[word] & ~selected) | (z << (low % 64u));
        bit += count;
    }
}
static inline void llg_sv4_range_fill(sv4_t* target, uint32_t low,
                                     uint32_t count, unsigned state) {
    if (low >= llg_sv4_width(*target)) return;
    if (count > llg_sv4_width(*target) - low) count = llg_sv4_width(*target) - low;
    while (count) {
        unsigned chunk = 64u - low % 64u;
        if (chunk > count) chunk = count;
        uint64_t mask = (UINT64_MAX >> (64u - chunk)) << (low % 64u);
        size_t word = low / 64u;
        target->bits[word] = (target->bits[word] & ~mask) | (state == 1 ? mask : 0);
        target->x[word] = (target->x[word] & ~mask) | (state == 2 ? mask : 0);
        target->z[word] = (target->z[word] & ~mask) | (state == 3 ? mask : 0);
        low += chunk;
        count -= chunk;
    }
}
static inline void llg_sv4_mask_remove(sv4_t* target, sv4_t removed) {
    for (size_t i = 0; i < llg_sv4_words(*target); ++i)
        target->bits[i] &= ~llg_sv4_word(removed, i, LLG_SV4_BITS);
}
static inline void llg_sv4_mask_top(sv4_t* value) {
    if (!llg_sv4_width(*value) || !(llg_sv4_width(*value) % 64u)) return;
    size_t word = llg_sv4_width(*value) / 64u;
    uint64_t mask = UINT64_MAX >> (64u - llg_sv4_width(*value) % 64u);
    value->bits[word] &= mask;
    value->x[word] &= mask;
    value->z[word] &= mask;
}
static inline void llg_sv4_mul_add_known(sv4_t* value, uint32_t factor, uint32_t addend) {
    uint64_t carry = addend;
    for (size_t i = 0; i < llg_sv4_words(*value); ++i) {
        uint64_t low = (value->bits[i] & UINT32_MAX) * factor + carry;
        uint64_t high = (value->bits[i] >> 32) * factor + (low >> 32);
        value->bits[i] = (high << 32) | (low & UINT32_MAX);
        carry = high >> 32;
    }
    llg_sv4_mask_top(value);
}
static inline void llg_sv4_negate_known(sv4_t* value) {
    uint64_t carry = 1;
    for (size_t i = 0; i < llg_sv4_words(*value); ++i) {
        uint64_t inverted = ~value->bits[i];
        value->bits[i] = inverted + carry;
        carry = value->bits[i] < inverted;
    }
    llg_sv4_mask_top(value);
}
static inline void llg_sv4_two_state_inplace(sv4_t* value) {
    for (size_t i = 0; i < llg_sv4_words(*value); ++i) {
        value->bits[i] &= ~(value->x[i] | value->z[i]);
        value->x[i] = value->z[i] = 0;
    }
}
// Append 1..4 low bits, with X/Z states 2/3, modulo the existing width.
static inline void llg_sv4_append_digit(sv4_t* value, unsigned count,
                                       unsigned state, unsigned digit) {
    uint64_t mask = (UINT64_C(1) << count) - 1u;
    uint64_t bits = state == 0 ? digit & mask : 0;
    uint64_t x = state == 2 ? mask : 0, z = state == 3 ? mask : 0;
    for (size_t i = 0; i < llg_sv4_words(*value); ++i) {
        uint64_t old_bits = value->bits[i], old_x = value->x[i], old_z = value->z[i];
        value->bits[i] = (old_bits << count) | bits;
        value->x[i] = (old_x << count) | x;
        value->z[i] = (old_z << count) | z;
        bits = old_bits >> (64u - count);
        x = old_x >> (64u - count);
        z = old_z >> (64u - count);
    }
    llg_sv4_mask_top(value);
}

// External interleaved 32-bit aval/bval records; stride >= 8, first two fields
// are four-byte integers. memcpy avoids aliasing foreign signed/unsigned types.
#endif
