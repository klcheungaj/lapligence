#include "internal.h"

int llg_gmp_sv4_truth_wide(g4_t v) {
    size_t n = llg_gmp_sv4_words(v);
    const uint64_t *a = v.data.wide.a, *b = v.data.wide.b;
    if (!b) {
        for (size_t i = 0; i < n; ++i)
            if (a[i])
                return 1;
        return 0;
    }
    for (size_t i = 0; i < n; ++i)
        if (a[i] & ~b[i])
            return 1;
    return 2;
}
int llg_gmp_sv4_has_plane_wide(g4_t v, int z) {
    if (!v.data.wide.b)
        return 0;
    size_t n = llg_gmp_sv4_words(v);
    const uint64_t *a = v.data.wide.a, *b = v.data.wide.b;
    if (z) {
        for (size_t i = 0; i < n; ++i)
            if (~a[i] & b[i])
                return 1;
    } else {
        for (size_t i = 0; i < n; ++i)
            if (a[i] & b[i])
                return 1;
    }
    return 0;
}
int llg_gmp_sv4_same_wide(g4_t a, g4_t b) {
    size_t an = llg_gmp_sv4_words(a), bn = llg_gmp_sv4_words(b), n = an < bn ? an : bn;
    const uint64_t *aa = g4_a(&a), *ab = g4_b(&a), *ba = g4_a(&b), *bb = g4_b(&b);
    if (n && memcmp(aa, ba, n * 8u))
        return 0;
    if (ab && bb) {
        if (n && memcmp(ab, bb, n * 8u))
            return 0;
    } else if (ab || bb) {
        const uint64_t* p = ab ? ab : bb;
        for (size_t i = 0; i < n; ++i)
            if (p[i])
                return 0;
    }
    for (size_t i = n; i < an; ++i)
        if (aa[i] || (ab && ab[i]))
            return 0;
    for (size_t i = n; i < bn; ++i)
        if (ba[i] || (bb && bb[i]))
            return 0;
    return 1;
}
g4_t llg_gmp_sv4_compare_wide(g4_t a, g4_t b, unsigned op) {
    uint32_t w = g4_maxw(a, b);
    int s = a.is_signed && b.is_signed;
    if (op >= 2 && (llg_gmp_sv4_is_unknown(a) || llg_gmp_sv4_is_unknown(b)))
        return g4_predicate(2);
    if (a.width == b.width &&
        (op == 1 || (!llg_gmp_sv4_is_unknown(a) && !llg_gmp_sv4_is_unknown(b) && op == 0))) {
        size_t bytes = llg_gmp_sv4_words(a) * sizeof(uint64_t);
        if (memcmp(a.data.wide.a, b.data.wide.a, bytes))
            return g4_predicate(0);
        const uint64_t *ab = a.data.wide.b, *bb = b.data.wide.b;
        return g4_predicate(ab && bb ? !memcmp(ab, bb, bytes) : ab == bb);
    }
    g4_span x = g4_span_of(&a, s), y = g4_span_of(&b, s);
    size_t n = ((size_t)w + 63) / 64;
    int unknown = 0, c = 0;
    for (size_t i = n; i-- > 0;) {
        uint64_t mask = i + 1 == n ? g4_topmask(w) : UINT64_MAX;
        uint64_t aa = G4_SPAN_A(x, i) & mask, ab = G4_SPAN_B(x, i) & mask;
        uint64_t ba = G4_SPAN_A(y, i) & mask, bb = G4_SPAN_B(y, i) & mask;
        if (op == 0) {
            if ((aa ^ ba) & ~(ab | bb))
                return g4_predicate(0);
            unknown |= (ab | bb) != 0;
        } else if (op == 1) {
            if (aa != ba || ab != bb)
                return g4_predicate(0);
        } else if (!c && aa != ba)
            c = aa < ba ? -1 : 1;
    }
    if (op <= 1)
        return g4_predicate(unknown ? 2 : 1);
    if (s && x.fill_a != y.fill_a)
        c = x.fill_a ? -1 : 1;
    return g4_predicate(op == 2 ? c < 0 : op == 3 ? c <= 0 : op == 4 ? c > 0 : c >= 0);
}
g4_t llg_gmp_sv4_mux_wide(g4_t a, g4_t b) {
    uint32_t w = g4_maxw(a, b);
    int8_t s = a.is_signed && b.is_signed;
    g4_t out = llg_gmp_sv4_new(w, s, 1);
    llg_gmp_sv4_extend_value(&out, a, s);
    size_t n = llg_gmp_sv4_words(out);
    g4_span rhs = g4_span_of(&b, s);
    uint64_t *aa = out.data.wide.a, *ab = out.data.wide.b;
    for (size_t i = 0; i < n; ++i) {
        uint64_t ba = G4_SPAN_A(rhs, i), bb = G4_SPAN_B(rhs, i);
        uint64_t u = (aa[i] ^ ba) | ab[i] | bb;
        aa[i] = (aa[i] & ~u) | u;
        ab[i] = u;
    }
    llg_gmp_sv4_finish(&out);
    return out;
}
