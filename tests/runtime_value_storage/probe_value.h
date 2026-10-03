#ifndef LLG_PROBE_VALUE_H
#define LLG_PROBE_VALUE_H
#include "llg_value.h"
/* Oracle plane access through the neutral bridge, so every probe builds with
 * either selected backend. Reads return legacy-format bits/X/Z words. */
#define PROBE_BITS(value, word) llg_sv4_word((value), (word), LLG_SV4_BITS)
#define PROBE_X(value, word) llg_sv4_word((value), (word), LLG_SV4_X)
#define PROBE_Z(value, word) llg_sv4_word((value), (word), LLG_SV4_Z)
static inline unsigned probe_state(sv4_t value, uint64_t bit) {
    uint64_t mask = UINT64_C(1) << (bit % 64u);
    size_t word = (size_t)(bit / 64u);
    if (PROBE_X(value, word) & mask) return 2;
    if (PROBE_Z(value, word) & mask) return 3;
    return (PROBE_BITS(value, word) & mask) != 0;
}
static inline void probe_put_state(sv4_t* value, uint64_t bit, unsigned state) {
    llg_sv4_set_state(value, bit, state);
}
/* Replace one known word, keeping that word's X/Z positions. */
static inline void probe_set_bits(sv4_t* value, size_t word, uint64_t bits) {
    llg_sv4_set_word(value, word, bits, PROBE_X(*value, word), PROBE_Z(*value, word));
}
static inline int probe_is_empty(sv4_t value) { return llg_sv4_width(value) == 0; }
/* Independent owners: changing word 0 of `owner` leaves `other` unchanged.
 * The owner is restored; empty owners are trivially independent. */
static inline int probe_distinct(sv4_t* owner, sv4_t other) {
    if (probe_is_empty(*owner) || probe_is_empty(other)) return 1;
    uint64_t bits = PROBE_BITS(*owner, 0), x = PROBE_X(*owner, 0), z = PROBE_Z(*owner, 0);
    uint64_t other_bits = PROBE_BITS(other, 0), other_x = PROBE_X(other, 0);
    uint64_t other_z = PROBE_Z(other, 0);
    llg_sv4_set_word(owner, 0, ~bits, 0, 0);
    int unchanged = PROBE_BITS(other, 0) == other_bits && PROBE_X(other, 0) == other_x &&
                    PROBE_Z(other, 0) == other_z;
    llg_sv4_set_word(owner, 0, bits, x, z);
    return unchanged;
}
/* Exact payload bytes and allocations of one owner in the selected backend:
 * legacy keeps three planes for every nonzero width; compact keeps widths
 * through 64 inline and adds a B plane only while X/Z exists. */
static inline size_t probe_payload_bytes(uint32_t width, int unknown) {
#if LLG_SV4_USE_GMP
    if (width <= 64u) return 0;
    return ((size_t)width + 63u) / 64u * (unknown ? 16u : 8u);
#else
    (void)unknown;
    return ((size_t)width + 63u) / 64u * 24u;
#endif
}
static inline size_t probe_owner_allocations(uint32_t width) {
#if LLG_SV4_USE_GMP
    return width > 64u;
#else
    return width != 0;
#endif
}
/* Same width and stored states, independently of the payload layout. */
static inline int probe_same(sv4_t left, sv4_t right) {
    return llg_sv4_width(left) == llg_sv4_width(right) && sv4_same(left, right);
}
#endif
