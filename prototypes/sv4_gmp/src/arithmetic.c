#include "internal.h"

static gmp4_t add_sub(gmp4_t a, gmp4_t b, int subtract) {
    uint32_t width = g4_max_width(a, b);
    int8_t sign = a.is_signed && b.is_signed;
    if (gmp4_is_unknown(a) || gmp4_is_unknown(b)) return gmp4_x(width, sign);
    if (width <= 64) {
        uint64_t x = g4_small_extended(a, width, sign);
        uint64_t y = g4_small_extended(b, width, sign);
        return gmp4_from_u64(subtract ? x - y : x + y, width, sign);
    }
    gmp4_t result = gmp4_zero(width, sign);
    size_t n = g4_count(width);
    mp_limb_t *scratch = NULL;
    const mp_limb_t *left, *right;
    if (a.width == width && b.width == width) {
        left = a.data.wide.a; right = b.data.wide.a;
    } else {
        scratch = g4_alloc(2u * n);
        for (size_t i = 0; i < n; ++i) {
            scratch[i] = g4_extended(a, width, sign, i, 0);
            scratch[n + i] = g4_extended(b, width, sign, i, 0);
        }
        left = scratch; right = scratch + n;
    }
    if (subtract) (void)mpn_sub_n(result.data.wide.a, left, right, (mp_size_t)n);
    else (void)mpn_add_n(result.data.wide.a, left, right, (mp_size_t)n);
    result.data.wide.a[n - 1u] &= g4_mask(width, n - 1u);
    GMP4_FREE(scratch);
    return result;
}
gmp4_t gmp4_add(gmp4_t a, gmp4_t b) { return add_sub(a, b, 0); }
gmp4_t gmp4_sub(gmp4_t a, gmp4_t b) { return add_sub(a, b, 1); }

void gmp4_add_into(gmp4_t *destination, gmp4_t a, gmp4_t b) {
    if (a.width > 64 && a.width == b.width && destination->width == a.width &&
        !gmp4_is_unknown(a) && !gmp4_is_unknown(b)) {
        size_t n = g4_count(a.width);
        /* mpn_add_n permits EXACT source/destination aliasing. */
        (void)mpn_add_n(destination->data.wide.a, a.data.wide.a,
                        b.data.wide.a, (mp_size_t)n);
        destination->data.wide.a[n - 1u] &= g4_mask(a.width, n - 1u);
        if (destination->data.wide.b)
            memset(destination->data.wide.b, 0, n * sizeof(mp_limb_t));
        destination->is_signed = a.is_signed && b.is_signed;
        return;
    }
    gmp4_replace(destination, gmp4_add(a, b));
}
void gmp4_mul_into(gmp4_t *destination, gmp4_t a, gmp4_t b,
                  gmp4_workspace_t *workspace) {
    uint32_t width = g4_max_width(a, b);
    int8_t sign = a.is_signed && b.is_signed;
    if (gmp4_is_unknown(a) || gmp4_is_unknown(b)) {
        gmp4_replace(destination, gmp4_x(width, sign)); return;
    }
    if (width <= 64) {
        uint64_t x = g4_small_extended(a, width, sign);
        uint64_t y = g4_small_extended(b, width, sign);
        gmp4_replace(destination, gmp4_from_u64(x * y, width, sign)); return;
    }
    size_t n = g4_count(width);
    int direct = a.width == width && b.width == width;
    g4_workspace_reserve(workspace, (direct ? 2u : 4u) * n);
    const mp_limb_t *left, *right;
    if (direct) { left = a.data.wide.a; right = b.data.wide.a; }
    else {
        for (size_t i = 0; i < n; ++i) {
            workspace->data[2u * n + i] = g4_extended(a, width, sign, i, 0);
            workspace->data[3u * n + i] = g4_extended(b, width, sign, i, 0);
        }
        left = workspace->data + 2u * n;
        right = workspace->data + 3u * n;
    }
    /* Product area has 2n limbs and NEVER overlaps either input. The result
     * is truncated only after GMP has written the full product. */
    if (left == right) mpn_sqr(workspace->data, left, (mp_size_t)n);
    else mpn_mul_n(workspace->data, left, right, (mp_size_t)n);
    workspace->data[n - 1u] &= g4_mask(width, n - 1u);
    /* All borrowed inputs have been consumed before destination replacement. */
    if (destination->width != width)
        gmp4_replace(destination, gmp4_zero(width, sign));
    memcpy(destination->data.wide.a, workspace->data, n * sizeof(mp_limb_t));
    if (destination->data.wide.b)
        memset(destination->data.wide.b, 0, n * sizeof(mp_limb_t));
    destination->is_signed = sign;
}
gmp4_t gmp4_mul(gmp4_t a, gmp4_t b) {
    uint32_t width = g4_max_width(a, b);
    if (width <= 64) {
        int8_t sign = a.is_signed && b.is_signed;
        if (gmp4_is_unknown(a) || gmp4_is_unknown(b)) return gmp4_x(width, sign);
        return gmp4_from_u64(g4_small_extended(a, width, sign) *
                            g4_small_extended(b, width, sign), width, sign);
    }
    gmp4_t result = GMP4_EMPTY;
    gmp4_workspace_t scratch = GMP4_WORKSPACE_EMPTY;
    gmp4_mul_into(&result, a, b, &scratch);
    gmp4_workspace_destroy(&scratch);
    return result;
}
gmp4_t gmp4_neg(gmp4_t value) {
    if (gmp4_is_unknown(value)) return gmp4_x(value.width, value.is_signed);
    if (value.width <= 64)
        return gmp4_from_u64(UINT64_C(0) - value.data.small.a,
                            value.width, value.is_signed);
    gmp4_t result = gmp4_zero(value.width, value.is_signed);
    size_t n = g4_count(value.width);
    (void)mpn_neg(result.data.wide.a, value.data.wide.a, (mp_size_t)n);
    result.data.wide.a[n - 1u] &= g4_mask(value.width, n - 1u);
    return result;
}
