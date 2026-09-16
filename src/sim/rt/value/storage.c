// Exact-width storage ownership. Private fragment of llg_value.c.
// No scheduler state, model-capacity dependency, or compiler extensions.

static size_t sv4_storage_limb_count(uint32_t width) {
    return (size_t)(width / 64u) + (width % 64u != 0);
}

sv4_storage_t sv4_storage_zero(uint32_t width, int8_t is_signed) {
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
    sv4_storage_t result = SV4_STORAGE_EMPTY;
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

sv4_storage_t sv4_storage_from_limbs(const uint64_t* bits, const uint64_t* x,
                                    const uint64_t* z, uint32_t width,
                                    int8_t is_signed) {
    sv4_storage_t result = sv4_storage_zero(width, is_signed);
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

sv4_storage_t sv4_storage_clone(const sv4_storage_t* source) {
    return sv4_storage_from_limbs(source->bits, source->x, source->z,
                                 source->width, source->is_signed);
}

void sv4_storage_copy(sv4_storage_t* destination, const sv4_storage_t* source) {
    if (destination == source) return;
    // Clone before replacement so allocation failure cannot partially update
    // an already initialized destination.
    sv4_storage_t copy = sv4_storage_clone(source);
    sv4_storage_move(destination, &copy);
}

void sv4_storage_move(sv4_storage_t* destination, sv4_storage_t* source) {
    if (destination == source) return;
    sv4_storage_destroy(destination);
    *destination = *source;
    *source = (sv4_storage_t)SV4_STORAGE_EMPTY;
}

void sv4_storage_destroy(sv4_storage_t* storage) {
    if (!storage) return;
    free(storage->bits);
    *storage = (sv4_storage_t)SV4_STORAGE_EMPTY;
}

size_t sv4_storage_bytes(const sv4_storage_t* storage) {
    return sv4_storage_limb_count(storage->width) * 3u * sizeof(uint64_t);
}
