#include "internal.h"

gmp4_t g4_new(uint32_t width, int8_t sign, int with_b) {
    g4_require_width(width);
    gmp4_t result = GMP4_EMPTY;
    result.width = width;
    result.is_signed = sign != 0;
    if (width <= 64) return result;
    size_t n = g4_count(width), planes = with_b ? 2u : 1u;
    result.data.wide.a = g4_alloc(n * planes);
    memset(result.data.wide.a, 0, n * planes * sizeof(mp_limb_t));
    result.data.wide.b = with_b ? result.data.wide.a + n : NULL;
    return result;
}
gmp4_t gmp4_zero(uint32_t width, int8_t sign) { return g4_new(width, sign, 0); }

void gmp4_destroy(gmp4_t *value) {
    if (!value) return;
    if (value->width > 64) GMP4_FREE(value->data.wide.a);
    *value = (gmp4_t)GMP4_EMPTY;
}
void gmp4_destroy_array(gmp4_t *values, size_t count) {
    for (size_t i = 0; i < count; ++i) gmp4_destroy(&values[i]);
}
size_t gmp4_bytes(const gmp4_t *value) {
    return value->width <= 64 ? 0 : g4_count(value->width) * sizeof(mp_limb_t) *
        (value->data.wide.b ? 2u : 1u);
}
void gmp4_replace(gmp4_t *destination, gmp4_t owned) {
    if (destination->width > 64 && owned.width > 64 &&
        destination->data.wide.a == owned.data.wide.a) {
        if (destination->width != owned.width) g4_fail("self-borrow changed width");
        destination->is_signed = owned.is_signed;
        return;
    }
    gmp4_destroy(destination);
    *destination = owned;
}
void gmp4_move(gmp4_t *destination, gmp4_t *source) {
    if (destination == source) return;
    gmp4_replace(destination, *source);
    *source = (gmp4_t)GMP4_EMPTY;
}
gmp4_t gmp4_clone(const gmp4_t *source) {
    if (source->width <= 64) return *source;
    gmp4_t result = g4_new(source->width, source->is_signed, g4_has_b(*source));
    memcpy(result.data.wide.a, source->data.wide.a, gmp4_bytes(source));
    return result;
}
void gmp4_copy(gmp4_t *destination, const gmp4_t *source) {
    if (destination == source) return;
    if (source->width <= 64) {
        gmp4_replace(destination, *source);
        return;
    }
    if (destination->width == source->width &&
        (destination->data.wide.b || !source->data.wide.b)) {
        if (destination->data.wide.a == source->data.wide.a) {
            destination->is_signed = source->is_signed;
            return;
        }
        size_t bytes = g4_count(source->width) * sizeof(mp_limb_t);
        memcpy(destination->data.wide.a, source->data.wide.a, bytes);
        if (destination->data.wide.b) {
            if (source->data.wide.b)
                memcpy(destination->data.wide.b, source->data.wide.b, bytes);
            else memset(destination->data.wide.b, 0, bytes);
        }
        destination->is_signed = source->is_signed;
        return;
    }
    gmp4_replace(destination, gmp4_clone(source));
}
void gmp4_assign(gmp4_t *destination, gmp4_t source) { gmp4_copy(destination, &source); }

void g4_promote(gmp4_t *value) {
    if (value->width <= 64 || value->data.wide.b) return;
    size_t n = g4_count(value->width);
    mp_limb_t *data = g4_alloc(2u * n);
    memcpy(data, value->data.wide.a, n * sizeof(mp_limb_t));
    memset(data + n, 0, n * sizeof(mp_limb_t));
    GMP4_FREE(value->data.wide.a);
    value->data.wide.a = data;
    value->data.wide.b = data + n;
}
void gmp4_compact(gmp4_t *value) {
    if (value->width <= 64 || !value->data.wide.b || gmp4_is_unknown(*value)) return;
    size_t n = g4_count(value->width);
    mp_limb_t *data = g4_alloc(n);
    memcpy(data, value->data.wide.a, n * sizeof(mp_limb_t));
    GMP4_FREE(value->data.wide.a);
    value->data.wide.a = data;
    value->data.wide.b = NULL;
}
void g4_workspace_reserve(gmp4_workspace_t *workspace, size_t limbs) {
    if (!workspace) g4_fail("NULL workspace");
    if (limbs <= workspace->capacity) return;
    mp_limb_t *data = g4_alloc(limbs);
    GMP4_FREE(workspace->data);
    workspace->data = data;
    workspace->capacity = limbs;
}
void gmp4_workspace_destroy(gmp4_workspace_t *workspace) {
    if (!workspace) return;
    GMP4_FREE(workspace->data);
    *workspace = (gmp4_workspace_t)GMP4_WORKSPACE_EMPTY;
}
size_t gmp4_workspace_bytes(const gmp4_workspace_t *workspace) {
    return workspace->capacity * sizeof(mp_limb_t);
}

gmp4_t gmp4_fill(uint8_t state, uint32_t width, int8_t sign) {
    if (state > 3) g4_fail("invalid scalar state");
    gmp4_t result = g4_new(width, sign, state >= 2);
    for (size_t i = 0; i < g4_count(width); ++i) {
        g4_put(&result, i, 0, state == 1 || state == 2 ? GMP_NUMB_MASK : 0);
        g4_put(&result, i, 1, state >= 2 ? GMP_NUMB_MASK : 0);
    }
    return result;
}
gmp4_t gmp4_x(uint32_t width, int8_t sign) { return gmp4_fill(2, width, sign); }

gmp4_t gmp4_from_u64(uint64_t bits, uint32_t width, int8_t sign) {
    gmp4_t result = gmp4_zero(width, sign);
    if (width <= 64) result.data.small.a = bits & g4_mask64(width);
    else {
        result.data.wide.a[0] = (mp_limb_t)bits;
#if GMP_NUMB_BITS == 32
        result.data.wide.a[1] = (mp_limb_t)(bits >> 32);
#endif
    }
    return result;
}
gmp4_t gmp4_from_i64(int64_t bits, uint32_t width) {
    gmp4_t small = gmp4_from_u64((uint64_t)bits, 64, 1);
    return gmp4_cast(small, width, 1);
}

static mp_limb_t input_limb(const uint64_t *input, size_t limb) {
    if (!input) return 0;
#if GMP_NUMB_BITS == 64
    return (mp_limb_t)input[limb];
#else
    return (mp_limb_t)(input[limb / 2u] >> ((limb % 2u) * 32u));
#endif
}
gmp4_t gmp4_from_limbs(const uint64_t *bits, const uint64_t *x,
                     const uint64_t *z, uint32_t width, int8_t sign) {
    g4_require_width(width);
    int unknown = 0;
    for (size_t i = 0; i < g4_count(width); ++i) {
        mp_limb_t xx = input_limb(x, i), zz = input_limb(z, i);
        if ((xx & zz) & g4_mask(width, i)) g4_fail("overlapping X/Z masks");
        unknown |= ((xx | zz) & g4_mask(width, i)) != 0;
    }
    gmp4_t result = g4_new(width, sign, unknown);
    for (size_t i = 0; i < g4_count(width); ++i) {
        mp_limb_t xx = input_limb(x, i), zz = input_limb(z, i);
        mp_limb_t b = xx | zz;
        g4_put(&result, i, 0, (input_limb(bits, i) & ~b) | xx);
        g4_put(&result, i, 1, b);
    }
    return result;
}
gmp4_t gmp4_from_masks(uint64_t bits, uint64_t x, uint64_t z,
                     uint32_t width, int8_t sign) {
    if (width > 64) g4_fail("mask constructor requires width <= 64");
    return gmp4_from_limbs(&bits, &x, &z, width, sign);
}
