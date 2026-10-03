#include "internal.h"

gmp4_t gmp4_resolve_wire(const gmp4_t *const *drivers, size_t count,
                         uint32_t width, int8_t sign) {
    if (count && !drivers) g4_fail("NULL driver table");
    for (size_t d = 0; d < count; ++d)
        if (drivers[d] && drivers[d]->width != width)
            g4_fail("wire driver width mismatch");
    gmp4_t result = g4_new(width, sign, 1);
    for (size_t i = 0; i < g4_count(width); ++i) {
        mp_limb_t zero = 0, one = 0, x = 0;
        for (size_t d = 0; d < count; ++d) {
            if (!drivers[d]) continue;
            mp_limb_t a = g4_limb(*drivers[d], i, 0);
            mp_limb_t b = g4_limb(*drivers[d], i, 1);
            zero |= ~a & ~b;
            one |= a & ~b;
            x |= a & b;
        }
        mp_limb_t unknown = x | (zero & one);
        mp_limb_t z = ~(zero | one | x);
        g4_put(&result, i, 0, one | unknown);
        g4_put(&result, i, 1, unknown | z);
    }
    return result;
}
