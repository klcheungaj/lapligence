#include "internal.h"

g4_t llg_gmp_sv4_match_wide(g4_t a, g4_t b, unsigned mode) {
    uint32_t width = g4_maxw(a, b);
    int sign = mode == 2 && a.is_signed && b.is_signed;
    g4_span lhs = g4_span_of(&a, sign), rhs = g4_span_of(&b, sign);
    size_t n = ((size_t)width + 63u) / 64u;
    int unknown = 0;
    for (size_t i = 0; i < n; ++i) {
        uint64_t mask = i + 1 == n ? g4_topmask(width) : UINT64_MAX;
        uint64_t aa = G4_SPAN_A(lhs, i), ab = G4_SPAN_B(lhs, i);
        uint64_t ba = G4_SPAN_A(rhs, i), bb = G4_SPAN_B(rhs, i);
        if (mode == 0) {
            if ((aa ^ ba) & ~(ab | bb) & mask)
                return g4_predicate(0);
        } else if (mode == 1) {
            uint64_t care = ~((~aa & ab) | (~ba & bb)) & mask;
            if (((aa ^ ba) | (ab ^ bb)) & care)
                return g4_predicate(0);
        } else {
            uint64_t care = ~bb & mask;
            if ((aa ^ ba) & ~ab & care)
                return g4_predicate(0);
            unknown |= (ab & care) != 0;
        }
    }
    return g4_predicate(unknown ? 2 : 1);
}
