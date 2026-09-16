// Exact-width storage ownership. Private fragment of llg_value.c.
// No scheduler state, model-capacity dependency, or compiler extensions.

static size_t sv4_storage_limb_count(uint32_t width) {
    return (size_t)(width / 64u) + (width % 64u != 0);
}

sv4_t sv4_zero(uint32_t width, int8_t is_signed) {
    if (width >= LLG_SUPPORTED_WIDTH_LIMIT) {
        fprintf(stderr,
                "llg runtime fatal: packed storage width %llu reaches or exceeds supported limit %u\n",
                (unsigned long long)width, (unsigned)LLG_SUPPORTED_WIDTH_LIMIT);
        abort();
    }
    size_t limbs = sv4_storage_limb_count(width);
    if (limbs > SIZE_MAX / sizeof(uint64_t) / 3u) {
        fputs("llg runtime fatal: packed storage allocation size overflow\n", stderr);
        abort();
    }
    sv4_t result = SV4_EMPTY;
    result.width = width;
    result.is_signed = (int8_t)(is_signed != 0);
    if (!limbs) return result;

    size_t bytes = limbs * 3u * sizeof(uint64_t);
    result.bits = (uint64_t*)malloc(bytes);
    if (!result.bits) {
        fputs("llg runtime fatal: packed storage allocation failed\n", stderr);
        abort();
    }
    result.x = result.bits + limbs;
    result.z = result.x + limbs;
    memset(result.bits, 0, bytes);
    return result;
}

sv4_t sv4_from_limbs(const uint64_t* bits, const uint64_t* x,
                                    const uint64_t* z, uint32_t width,
                                    int8_t is_signed) {
    sv4_t result = sv4_zero(width, is_signed);
    size_t limbs = sv4_storage_limb_count(width);
    if (!limbs) return result;
    size_t bytes = limbs * sizeof(uint64_t);
    if (bits) memcpy(result.bits, bits, bytes);
    if (x) memcpy(result.x, x, bytes);
    if (z) memcpy(result.z, z, bytes);
    if (width % 64u) {
        uint64_t mask = UINT64_MAX >> (64u - width % 64u);
        result.bits[limbs - 1u] &= mask;
        result.x[limbs - 1u] &= mask;
        result.z[limbs - 1u] &= mask;
    }
    return result;
}

sv4_t sv4_clone(const sv4_t* source) {
    return sv4_from_limbs(source->bits, source->x, source->z,
                                 source->width, source->is_signed);
}

void sv4_copy(sv4_t* destination, const sv4_t* source) {
    if (destination == source) return;
    // Clone before replacement so allocation failure cannot partially update
    // an already initialized destination.
    sv4_t copy = sv4_clone(source);
    sv4_move(destination, &copy);
}

void sv4_move(sv4_t* destination, sv4_t* source) {
    if (destination == source) return;
    sv4_destroy(destination);
    *destination = *source;
    *source = (sv4_t)SV4_EMPTY;
}

void sv4_destroy(sv4_t* storage) {
    if (!storage) return;
    free(storage->bits);
    *storage = (sv4_t)SV4_EMPTY;
}

size_t sv4_bytes(const sv4_t* storage) {
    return sv4_storage_limb_count(storage->width) * 3u * sizeof(uint64_t);
}

void sv4_replace(sv4_t* destination, sv4_t owned) {
    // Equal allocation is possible only for an explicit self-borrow. Do not
    // destroy it; well-formed independent owners otherwise never alias.
    if (destination->bits && destination->bits == owned.bits) {
        destination->width = owned.width;
        destination->is_signed = owned.is_signed;
        return;
    }
    sv4_destroy(destination);
    *destination = owned;
}

void sv4_assign(sv4_t* destination, sv4_t source) {
    sv4_copy(destination, &source);
}

void sv4_destroy_array(sv4_t* values, size_t count) {
    for (size_t i = 0; i < count; ++i) sv4_destroy(&values[i]);
}

sv4_t sv4_from_masks(uint64_t bits, uint64_t x, uint64_t z,
                     uint32_t width, int8_t is_signed) {
    if (width > 64u) {
        fputs("llg runtime fatal: mask constructor requires width <= 64\n", stderr);
        abort();
    }
    return sv4_from_limbs(&bits, &x, &z, width, is_signed);
}
