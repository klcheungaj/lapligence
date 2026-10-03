#ifndef LLG_PROTOTYPE_GMP4_H
#define LLG_PROTOTYPE_GMP4_H

#include <gmp.h>
#include <stddef.h>
#include <stdint.h>

#define GMP4_WIDTH_LIMIT (UINT32_C(1) << 20)
#define GMP4_PROTOTYPE_ABI 1u

/* Private payload. Width/sign are readable metadata, never mutate width.
 * A/B: 0=00, 1=10, X=11, Z=01. No pointers refer to inline storage.
 * By-value arguments borrow. Returned values own. Start destinations EMPTY.
 * Never destroy or retain a wide struct-copy borrow across source mutation.
 */
typedef struct {
    union {
        struct { uint64_t a, b; } small;
        struct { mp_limb_t *a, *b; } wide;
    } data;
    uint32_t width;
    int8_t is_signed;
} gmp4_t;
#define GMP4_EMPTY { { { 0, 0 } }, 0, 0 }

typedef struct {
    mp_limb_t *data;
    size_t capacity;
} gmp4_workspace_t;
#define GMP4_WORKSPACE_EMPTY { NULL, 0 }

gmp4_t gmp4_zero(uint32_t width, int8_t is_signed);
gmp4_t gmp4_fill(uint8_t state, uint32_t width, int8_t is_signed);
gmp4_t gmp4_x(uint32_t width, int8_t is_signed);
gmp4_t gmp4_from_u64(uint64_t bits, uint32_t width, int8_t is_signed);
gmp4_t gmp4_from_i64(int64_t bits, uint32_t width);
/* Each non-NULL input covers ceil(width/64) words, LSW first. Disjoint X/Z
 * required. Irrelevant bits under X/Z are canonicalized; top bits masked. */
gmp4_t gmp4_from_limbs(const uint64_t *bits, const uint64_t *x,
                     const uint64_t *z, uint32_t width, int8_t is_signed);
gmp4_t gmp4_from_masks(uint64_t bits, uint64_t x, uint64_t z,
                     uint32_t width, int8_t is_signed);
gmp4_t gmp4_clone(const gmp4_t *source);
void gmp4_copy(gmp4_t *destination, const gmp4_t *source);
void gmp4_assign(gmp4_t *destination, gmp4_t source);
void gmp4_move(gmp4_t *destination, gmp4_t *source);
/* Consumes a fresh result; use move for a named owner. */
void gmp4_replace(gmp4_t *destination, gmp4_t owned);
void gmp4_destroy(gmp4_t *value);
void gmp4_destroy_array(gmp4_t *values, size_t count);
/* Owned heap bytes only, including retained B; excludes sizeof and workspace. */
size_t gmp4_bytes(const gmp4_t *value);
void gmp4_compact(gmp4_t *value);
void gmp4_workspace_destroy(gmp4_workspace_t *workspace);
size_t gmp4_workspace_bytes(const gmp4_workspace_t *workspace);

/* Read-only old-plane adapter: 0=known bits, 1=X, 2=Z; out-of-range -> 0.
 * Invalid plane is a contract violation. No pointer view escapes. */
uint64_t gmp4_word(gmp4_t value, size_t word, unsigned plane);
/* Scalar codes match legacy sv4_fill: 0,1,2=X,3=Z, NOT DPI scalar codes. */
unsigned gmp4_get_bit(gmp4_t value, uint64_t bit);
void gmp4_set_bit(gmp4_t *value, uint64_t bit, unsigned state);
uint64_t gmp4_to_u64(gmp4_t value);
int gmp4_is_unknown(gmp4_t value);
int gmp4_to_bool(gmp4_t value);
int gmp4_same(gmp4_t a, gmp4_t b);
gmp4_t gmp4_resize(gmp4_t value, uint32_t width, int8_t result_signed);
gmp4_t gmp4_cast(gmp4_t value, uint32_t width, int8_t result_signed);
gmp4_t gmp4_to_two_state(gmp4_t value);

gmp4_t gmp4_add(gmp4_t a, gmp4_t b);
gmp4_t gmp4_sub(gmp4_t a, gmp4_t b);
gmp4_t gmp4_mul(gmp4_t a, gmp4_t b);
gmp4_t gmp4_neg(gmp4_t value);
/* Reuse initialized output storage; exact operand/output aliases supported.
 * Workspace must not be shared concurrently or aliased to value payloads. */
void gmp4_add_into(gmp4_t *destination, gmp4_t a, gmp4_t b);
void gmp4_mul_into(gmp4_t *destination, gmp4_t a, gmp4_t b,
                  gmp4_workspace_t *workspace);
gmp4_t gmp4_bitneg(gmp4_t value);
gmp4_t gmp4_and(gmp4_t a, gmp4_t b);
gmp4_t gmp4_or(gmp4_t a, gmp4_t b);
gmp4_t gmp4_xor(gmp4_t a, gmp4_t b);
gmp4_t gmp4_xnor(gmp4_t a, gmp4_t b);
gmp4_t gmp4_eq(gmp4_t a, gmp4_t b);
gmp4_t gmp4_case_eq(gmp4_t a, gmp4_t b);
gmp4_t gmp4_lognot(gmp4_t value);
gmp4_t gmp4_logand(gmp4_t a, gmp4_t b);
gmp4_t gmp4_logor(gmp4_t a, gmp4_t b);
gmp4_t gmp4_mux(gmp4_t condition, gmp4_t a, gmp4_t b);
/* Equal-strength wire/tri ONLY. Every non-NULL driver has exactly width bits.
 * This is not llg_net_t: no strengths, delays, charge, aliases or scheduling. */
gmp4_t gmp4_resolve_wire(const gmp4_t *const *drivers, size_t count,
                         uint32_t width, int8_t is_signed);

#endif
