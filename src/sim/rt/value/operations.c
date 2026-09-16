
// ── 4-state value ops ─────────────────────────────────────────────────────────

// Number of 64-bit limbs covering `w` bits.
static int sv4_nlimbs(uint32_t w) { return w == 0 ? 0 : (int)((w + 63u) / 64u); }

static void sv4_require_width(uint64_t width, const char* operation) {
    if (width < LLG_SUPPORTED_WIDTH_LIMIT) return;
    fprintf(stderr,
            "llg runtime fatal: %s width %llu reaches supported limit %u\n",
            operation, (unsigned long long)width,
            (unsigned)LLG_SUPPORTED_WIDTH_LIMIT);
    abort();
}

// Mask for limb `i` of a `w`-bit vector: full for interior limbs, partial for
// the top limb, zero beyond the width.
static uint64_t sv4_limb_mask(uint32_t w, int i) {
    int nl = sv4_nlimbs(w);
    if (i < 0 || i >= nl) return 0;
    if (i == nl - 1 && (w % 64) != 0) return LLG_MASK(w % 64);
    return ~0ULL;
}

sv4_t sv4_x(uint32_t width, int8_t is_signed) {
    sv4_t result = sv4_zero(width, is_signed);
    for (int i = 0; i < sv4_nlimbs(width); ++i)
        result.x[i] = sv4_limb_mask(width, i);
    return result;
}

sv4_t sv4_from_u64(uint64_t v, uint32_t width, int8_t is_signed) {
    sv4_t result = sv4_zero(width, is_signed);
    if (width) result.bits[0] = v & LLG_MASK(width);
    return result;
}

sv4_t sv4_from_i64(int64_t v, uint32_t width) {
    sv4_t result = sv4_from_u64((uint64_t)v, width, 1);
    if (v < 0) {
        for (int i = 1; i < sv4_nlimbs(width); ++i)
            result.bits[i] = sv4_limb_mask(width, i);
    }
    return result;
}

double sv4_to_real(sv4_t v) {
    int limbs = sv4_nlimbs(v.width);
    int negative = v.is_signed && v.width &&
        ((v.bits[(v.width - 1u) / 64u] &
          ~(v.x[(v.width - 1u) / 64u] | v.z[(v.width - 1u) / 64u])) >>
         ((v.width - 1u) % 64u) & 1u);
    // In ~magnitude + 1, carry stops at the first nonzero low limb. Locate it
    // first, then accumulate high-to-low to preserve the conversion's rounding
    // order without allocating or modifying a borrowed operand.
    int first_nonzero = 0;
    if (negative) {
        while (first_nonzero < limbs &&
               !(v.bits[first_nonzero] & ~(v.x[first_nonzero] | v.z[first_nonzero])))
            ++first_nonzero;
    }
    double result = 0.0;
    for (int i = limbs - 1; i >= 0; --i) {
        uint64_t word = v.bits[i] & ~(v.x[i] | v.z[i]);
        if (negative) word = ~word + (uint64_t)(i <= first_nonzero);
        word &= sv4_limb_mask(v.width, i);
        result = ldexp(result, 64) + (double)word;
    }
    return negative ? -result : result;
}

sv4_t sv4_from_real(double v, uint32_t width, int8_t is_signed) {
    if (!isfinite(v)) return sv4_x(width, is_signed);
    sv4_require_width(width, "real conversion");
    double rounded = round(v);
    const double modulus = 18446744073709551616.0;
    double magnitude = fabs(rounded);
    sv4_t result = sv4_zero(width, is_signed);
    result.width = width;
    result.is_signed = is_signed;
    for (int i = 0; i < sv4_nlimbs(width) && magnitude != 0.0; i++) {
        result.bits[i] = (uint64_t)fmod(magnitude, modulus);
        magnitude = floor(ldexp(magnitude, -64));
    }
    if (sv4_nlimbs(width) > 0)
        result.bits[sv4_nlimbs(width) - 1] &=
            sv4_limb_mask(width, sv4_nlimbs(width) - 1);
    if (signbit(rounded)) sv4_replace(&result, sv4_neg(result));
    return result;
}

sv4_t sv4_rtoi(double v) {
    if (!isfinite(v)) return sv4_x(32, 1);
    const double modulus = 4294967296.0;
    double magnitude = fmod(fabs(trunc(v)), modulus);
    uint64_t bits = (uint64_t)magnitude;
    if (signbit(v)) bits = 0ULL - bits;
    return sv4_from_u64(bits, 32, 1);
}

sv4_t sv4_realtobits(double v) {
    uint64_t bits;
    memcpy(&bits, &v, sizeof(bits));
    return sv4_from_u64(bits, 64, 0);
}

double sv4_bitstoreal(sv4_t v) {
    uint64_t bits = v.width ? v.bits[0] & ~(v.x[0] | v.z[0]) : 0;
    double result;
    memcpy(&result, &bits, sizeof(result));
    return result;
}

sv4_t sv4_shortrealtobits(double v) {
    float value = (float)v;
    uint32_t bits;
    memcpy(&bits, &value, sizeof(bits));
    return sv4_from_u64(bits, 32, 0);
}

double sv4_bitstoshortreal(sv4_t v) {
    uint32_t bits = v.width ? (uint32_t)(v.bits[0] & ~(v.x[0] | v.z[0])) : 0;
    float value;
    memcpy(&value, &bits, sizeof(value));
    return (double)value;
}

int llg_real_to_bool(double v) { return v != 0.0; }



int sv4_is_unknown(sv4_t v) {
    for (int i = 0; i < sv4_nlimbs(v.width); i++)
        if (v.x[i] | v.z[i]) return 1;
    return 0;
}

sv4_t sv4_countones(sv4_t v) {
    uint64_t count = 0;
    for (int i = 0; i < sv4_nlimbs(v.width); i++) {
        uint64_t ones = v.bits[i] & ~(v.x[i] | v.z[i]) & sv4_limb_mask(v.width, i);
        while (ones) {
            count++;
            ones &= ones - 1;
        }
    }
    return sv4_from_u64(count, 32, 1);
}

sv4_t sv4_onehot(sv4_t v, int allow_zero) {
    int found = 0;
    for (int i = 0; i < sv4_nlimbs(v.width); ++i) {
        uint64_t ones = v.bits[i] & ~(v.x[i] | v.z[i]) & sv4_limb_mask(v.width, i);
        if (!ones) continue;
        if (found || (ones & (ones - 1u))) return sv4_from_u64(0, 1, 0);
        found = 1;
    }
    return sv4_from_u64(found || allow_zero, 1, 0);
}

int sv4_to_bool(sv4_t v) {
    for (int i = 0; i < sv4_nlimbs(v.width); i++) {
        uint64_t known_ones = v.bits[i] & ~(v.x[i] | v.z[i]) &
                              sv4_limb_mask(v.width, i);
        if (known_ones) return 1;
    }
    return 0;
}

sv4_t sv4_repeat_count(sv4_t v) {
    sv4_require_width(v.width, "repeat count");
    if (sv4_is_unknown(v) || (v.is_signed && v.width &&
        ((v.bits[(v.width - 1) / 64] >> ((v.width - 1) % 64)) & 1ULL)))
        return sv4_from_u64(0, v.width, 0);
    v.is_signed = 0;
    return sv4_clone(&v);
}

uint64_t sv4_to_u64(sv4_t v) { return v.width ? v.bits[0] & LLG_MASK(v.width) : 0; }

static uint64_t checked_delay_product(uint64_t value, uint64_t scale) {
    if (!scale || value > UINT64_MAX / scale) {
        fprintf(stderr, "llg runtime fatal: delay exceeds the 64-bit tick range\n");
        abort();
    }
    return value * scale;
}

uint64_t sv4_delay_ticks(sv4_t value, uint64_t unit_ticks) {
    if (sv4_is_unknown(value)) return 0;
    int negative = value.is_signed && value.width &&
        ((value.bits[(value.width - 1) / 64] >> ((value.width - 1) % 64)) & 1);
    uint64_t raw = sv4_to_u64(value);
    if (negative && value.width < 64) raw |= ~LLG_MASK(value.width);
    if (!negative) {
        for (int i = 1; i < sv4_nlimbs(value.width); i++) {
            if (value.bits[i]) {
                fprintf(stderr, "llg runtime fatal: delay exceeds the 64-bit tick range\n");
                abort();
            }
        }
    }
    return checked_delay_product(raw, unit_ticks);
}

uint64_t sv4_real_delay_ticks(double value, uint64_t unit_ticks,
                              uint64_t precision_ticks) {
    if (!isfinite(value) || value < 0.0 || !precision_ticks || !unit_ticks) {
        fprintf(stderr, "llg runtime fatal: real delay must be finite and nonnegative\n");
        abort();
    }
    double rounded = round(value * ((double)unit_ticks / (double)precision_ticks));
    if (!isfinite(rounded) || rounded >= 18446744073709551616.0) {
        fprintf(stderr, "llg runtime fatal: delay exceeds the 64-bit tick range\n");
        abort();
    }
    return checked_delay_product((uint64_t)rounded, precision_ticks);
}

uint64_t sv4_to_index(sv4_t v) {
    if (sv4_is_unknown(v)) return UINT64_MAX;
    if (v.is_signed && v.width > 0 &&
        ((v.bits[(v.width - 1) / 64] >> ((v.width - 1) % 64)) & 1ULL)) {
        return UINT64_MAX;
    }
    for (int i = 1; i < sv4_nlimbs(v.width); i++) {
        if (v.bits[i]) return UINT64_MAX;
    }
    return sv4_to_u64(v);
}

int64_t sv4_to_i64(sv4_t v) {
    uint64_t b = sv4_to_u64(v);
    if (v.width == 0) return 0;
    if (v.width >= 64) return (int64_t)b;
    uint64_t sign = 1ULL << (v.width - 1);
    if (b & sign) return (int64_t)(b | ~(sign - 1));
    return (int64_t)b;
}

int sv4_to_index_i64(sv4_t v, int64_t* result) {
    if (!result || !sv4_fits_i64(v)) return 0;
    *result = v.is_signed ? sv4_to_i64(v) : (int64_t)sv4_to_u64(v);
    return 1;
}

uint32_t sv4_checked_width(sv4_t v) {
    if (sv4_is_unknown(v) ||
        (v.is_signed && v.width > 0 &&
         ((v.bits[(v.width - 1) / 64] >> ((v.width - 1) % 64)) & 1ULL))) {
        fprintf(stderr, "llg runtime fatal: invalid dynamic packed width\n");
        abort();
    }
    for (int i = 1; i < sv4_nlimbs(v.width); i++) {
        if (v.bits[i]) sv4_require_width(UINT64_MAX, "dynamic packed");
    }
    uint64_t width = sv4_to_u64(v);
    sv4_require_width(width, "dynamic packed");
    return (uint32_t)width;
}

int sv4_same(sv4_t a, sv4_t b) {
    int an = sv4_nlimbs(a.width), bn = sv4_nlimbs(b.width);
    int count = an > bn ? an : bn;
    for (int i = 0; i < count; ++i) {
        uint64_t ab = i < an ? a.bits[i] : 0;
        uint64_t ax = i < an ? a.x[i] : 0;
        uint64_t az = i < an ? a.z[i] : 0;
        uint64_t bb = i < bn ? b.bits[i] : 0;
        uint64_t bx = i < bn ? b.x[i] : 0;
        uint64_t bz = i < bn ? b.z[i] : 0;
        if (ab != bb || ax != bx || az != bz) return 0;
    }
    return 1;
}

sv4_t sv4_resolve(const sv4_t* const* drivers, int n_drivers,
                  uint32_t width, int8_t is_signed, int mode) {
    sv4_require_width(width, "net");
    sv4_t r = sv4_zero(width, is_signed);
    r.width = width;
    r.is_signed = is_signed;
    int nl = sv4_nlimbs(width);
    for (int i = 0; i < nl; i++) {
        uint64_t m = sv4_limb_mask(width, i);
        if (mode == LLG_RESOLVE_SUPPLY0 || mode == LLG_RESOLVE_SUPPLY1) {
            r.bits[i] = mode == LLG_RESOLVE_SUPPLY1 ? m : 0;
            continue;
        }
        uint64_t any0 = 0, any1 = 0, anyx = 0;
        for (int d = 0; d < n_drivers; d++) {
            const sv4_t* v = drivers[d];
            if (!v || i >= sv4_nlimbs(v->width)) continue;
            uint64_t bits = v->bits[i] & m;
            uint64_t x = v->x[i] & m;
            uint64_t z = (v->z[i] | ~sv4_limb_mask(v->width, i)) & m;
            any0 |= (~bits) & ~(x | z) & m;
            any1 |= bits & ~(x | z) & m;
            anyx |= x;
        }
        uint64_t driven = any0 | any1 | anyx;
        uint64_t known1;
        uint64_t unknown;
        if (mode == LLG_RESOLVE_WAND) {
            known1 = any1 & ~anyx & ~any0;
            unknown = anyx & ~any0;
        } else if (mode == LLG_RESOLVE_WOR) {
            known1 = any1;
            unknown = anyx & ~any1;
        } else {
            unknown = anyx | (any0 & any1);
            known1 = any1 & ~unknown;
        }
        r.bits[i] = known1 & m;
        r.x[i] = unknown & m;
        r.z[i] = ~driven & m;
        if (mode == LLG_RESOLVE_TRI0 || mode == LLG_RESOLVE_TRI1) {
            if (mode == LLG_RESOLVE_TRI1) r.bits[i] |= r.z[i];
            r.z[i] = 0;
        }
    }
    return r;
}

sv4_t sv4_resolve_strengths(const sv4_t* const* drivers,
                            const uint8_t* strength0,
                            const uint8_t* strength1, int n_drivers,
                            uint32_t width, int8_t is_signed, int mode) {
    if (!strength0 || !strength1)
        return sv4_resolve(drivers, n_drivers, width, is_signed, mode);

    sv4_require_width(width, "strength-aware net");
    sv4_t r = sv4_zero(width, is_signed);
    r.width = width;
    r.is_signed = is_signed;
    int nl = sv4_nlimbs(width);
    for (int i = 0; i < nl; i++) {
        uint64_t known0[8] = {0};
        uint64_t known1[8] = {0};
        uint64_t possible0[8] = {0};
        uint64_t possible1[8] = {0};
        uint64_t m = sv4_limb_mask(width, i);

        /* tri0/tri1 and supply0/supply1 contribute an implicit default
         * source.  It is a real strength-bearing source, rather than a
         * post-resolution fill, so an equal-strength opposing source can
         * produce X and a stronger source can override a pull default. */
        int default_strength = -1;
        int default_value = -1;
        if (mode == LLG_RESOLVE_TRI0) {
            default_strength = LLG_STRENGTH_PULL;
            default_value = 0;
        } else if (mode == LLG_RESOLVE_TRI1) {
            default_strength = LLG_STRENGTH_PULL;
            default_value = 1;
        } else if (mode == LLG_RESOLVE_SUPPLY0) {
            default_strength = LLG_STRENGTH_SUPPLY;
            default_value = 0;
        } else if (mode == LLG_RESOLVE_SUPPLY1) {
            default_strength = LLG_STRENGTH_SUPPLY;
            default_value = 1;
        }
        if (default_strength >= 0) {
            if (default_value == 0) {
                known0[default_strength] |= m;
                possible0[default_strength] |= m;
            } else {
                known1[default_strength] |= m;
                possible1[default_strength] |= m;
            }
        }

        for (int d = 0; d < n_drivers; d++) {
            const sv4_t* v = drivers[d];
            if (!v || i >= sv4_nlimbs(v->width)) continue;
            uint8_t s0 = strength0[d];
            uint8_t s1 = strength1[d];
            if (s0 > LLG_STRENGTH_SUPPLY || s1 > LLG_STRENGTH_SUPPLY) {
                fprintf(stderr, "llg runtime fatal: invalid net drive strength\n");
                abort();
            }
            uint64_t x = v->x[i] & m;
            uint64_t z = (v->z[i] | ~sv4_limb_mask(v->width, i)) & m;
            uint64_t k0 = (~v->bits[i]) & ~(x | z) & m;
            uint64_t k1 = v->bits[i] & ~(x | z) & m;
            if (s0 != LLG_STRENGTH_HIGHZ) {
                known0[s0] |= k0;
                possible0[s0] |= k0 | x;
            }
            if (s1 != LLG_STRENGTH_HIGHZ) {
                known1[s1] |= k1;
                possible1[s1] |= k1 | x;
            }
        }

        uint64_t out1 = 0;
        uint64_t outx = 0;
        uint64_t outz = 0;
        for (uint64_t bit = UINT64_C(1); bit != 0; bit <<= 1) {
            if (!(m & bit)) continue;
            int best_k0 = -1;
            int best_k1 = -1;
            int best_p0 = -1;
            int best_p1 = -1;
            for (int strength = LLG_STRENGTH_SUPPLY;
                 strength > LLG_STRENGTH_HIGHZ; strength--) {
                if (best_k0 < 0 && (known0[strength] & bit)) best_k0 = strength;
                if (best_k1 < 0 && (known1[strength] & bit)) best_k1 = strength;
                if (best_p0 < 0 && (possible0[strength] & bit)) best_p0 = strength;
                if (best_p1 < 0 && (possible1[strength] & bit)) best_p1 = strength;
            }

            if (best_p0 < 0 && best_p1 < 0) {
                outz |= bit;
            } else if (mode == LLG_RESOLVE_WAND) {
                /* A stronger known endpoint wins. At an equal strength,
                 * wired-AND gives a known 0 precedence over 1/X. */
                if (best_k0 > best_p1
                    || (best_k0 >= 0 && best_k0 == best_p1)) {
                    /* Known zero needs no bit in the cleared result. */
                } else if (best_k1 > best_p0) {
                    out1 |= bit;
                } else {
                    outx |= bit;
                }
            } else if (mode == LLG_RESOLVE_WOR) {
                /* Symmetric to WAND: a same-strength known 1 wins the
                 * wired-OR tie, while a stronger endpoint dominates. */
                if (best_k1 > best_p0
                    || (best_k1 >= 0 && best_k1 == best_p0)) {
                    out1 |= bit;
                } else if (best_k0 > best_p1) {
                    /* Known zero needs no bit in the cleared result. */
                } else {
                    outx |= bit;
                }
            } else if (best_k0 > best_p1) {
                /* Known zero needs no bit in the cleared result. */
            } else if (best_k1 > best_p0) {
                out1 |= bit;
            } else {
                outx |= bit;
            }
        }
        r.bits[i] = out1 & m;
        r.x[i] = outx & m;
        r.z[i] = outz & m;
    }
    return r;
}

// Bit `i` counted from the LSB; out-of-range -> 2 (X), X -> 2, Z -> 3,
// else 0/1.
static int sv4_lsb_bit(sv4_t v, int i) {
    if (i < 0 || i >= (int)v.width) return 2;
    int l = i >> 6, b = i & 63;
    if ((v.x[l] >> b) & 1) return 2;
    if ((v.z[l] >> b) & 1) return 3;
    return (v.bits[l] >> b) & 1;
}

int sv4_fits_i64(sv4_t v) {
    if (sv4_is_unknown(v)) return 0;
    if (v.width == 0) return 1;
    if (!v.is_signed) {
        if (v.width >= 64 && sv4_lsb_bit(v, 63) != 0) return 0;
        for (int i = 64; i < (int)v.width; i++)
            if (sv4_lsb_bit(v, i) != 0) return 0;
        return 1;
    }
    if (v.width <= 64) return 1;
    int sign = sv4_lsb_bit(v, 63);
    for (int i = 64; i < (int)v.width; i++)
        if (sv4_lsb_bit(v, i) != sign) return 0;
    return 1;
}

// Set bit `i` (LSB-indexed) of `v` to `val` (0, 1, 2 = X or 3 = Z).
static void sv4_lsb_bit_set(sv4_t* v, int i, int val) {
    if (i < 0 || i >= (int)v->width) return;
    int l = i >> 6;
    uint64_t m = 1ULL << (i & 63);
    if (val == 2) {
        v->x[l] |= m;
        v->z[l] &= ~m;
        v->bits[l] &= ~m;
    } else if (val == 3) {
        v->z[l] |= m;
        v->x[l] &= ~m;
        v->bits[l] &= ~m;
    } else if (val) {
        v->bits[l] |= m;
        v->x[l] &= ~m;
        v->z[l] &= ~m;
    } else {
        v->bits[l] &= ~m;
        v->x[l] &= ~m;
        v->z[l] &= ~m;
    }
}

// Shared resize core: pad/truncate `v` to `width` bits, tagging the result
// `is_signed`.  Widening fills the new MSBs per `ext_signed`; a signed X/Z
// sign bit fills with that same literal state (IEEE 1800-2009 §11.8.4).
static sv4_t sv4_resize_ext(sv4_t v, uint32_t width, int8_t is_signed, int8_t ext_signed) {
    sv4_require_width(width, "resize");
    if (v.width == width) {
        v.is_signed = is_signed;
        return sv4_clone(&v);
    }
    sv4_t r = sv4_zero(width, is_signed);
    r.width = width;
    r.is_signed = is_signed;
    int oln = sv4_nlimbs(v.width), nln = sv4_nlimbs(width);
    int copy = oln < nln ? oln : nln;
    for (int i = 0; i < copy; i++) {
        r.bits[i] = v.bits[i];
        r.x[i] = v.x[i];
        r.z[i] = v.z[i];
    }
    if (width > v.width && ext_signed && v.width > 0) {
        int sign = sv4_lsb_bit(v, (int)v.width - 1);
        for (int i = (int)v.width; i < (int)width; i++)
            sv4_lsb_bit_set(&r, i, sign);
    }
    if (nln > 0) {
        uint64_t m = sv4_limb_mask(width, nln - 1);
        r.bits[nln - 1] &= m;
        r.x[nln - 1] &= m;
        r.z[nln - 1] &= m;
    }
    return r;
}

sv4_t sv4_resize(sv4_t v, uint32_t width, int8_t is_signed) {
    return sv4_resize_ext(v, width, is_signed, is_signed);
}

// Value-preserving conversion (LRM 1800-2009 §6.24.1 / §10.7): widening
// extends by the SOURCE's signedness (`v.is_signed`) — an unsigned source
// zero-extends even into a signed target and vice versa — narrowing
// truncates; the result carries `is_signed`.
sv4_t sv4_cast(sv4_t v, uint32_t width, int8_t is_signed) {
    return sv4_resize_ext(v, width, is_signed, v.is_signed);
}

sv4_t sv4_to_two_state(sv4_t v) {
    v = sv4_clone(&v);
    for (int i = 0; i < sv4_nlimbs(v.width); i++) {
        v.bits[i] &= ~(v.x[i] | v.z[i]);
        v.x[i] = 0;
        v.z[i] = 0;
    }
    return v;
}

sv4_t sv4_fill(uint8_t bit, uint32_t width, int8_t is_signed) {
    sv4_require_width(width, "fill");
    sv4_t r = sv4_zero(width, is_signed);
    for (int i = 0; i < sv4_nlimbs(width); i++) {
        uint64_t m = sv4_limb_mask(width, i);
        r.bits[i] = (bit == 1) ? m : 0;
        r.x[i] = (bit == 2) ? m : 0;
        r.z[i] = (bit == 3) ? m : 0;
    }
    r.width = width;
    r.is_signed = is_signed;
    return r;
}

// Highest set known bit, or -1 when the value is zero.
static int sv4_msb(sv4_t v) {
    for (int i = sv4_nlimbs(v.width) - 1; i >= 0; --i) {
        uint64_t word = v.bits[i];
        if (!word) continue;
        int bit = 0;
        while (word >>= 1u) ++bit;
        return i * 64 + bit;
    }
    return -1;
}

sv4_t sv4_clog2(sv4_t v) {
    if (sv4_is_unknown(v)) return sv4_x(32, 0);
    int top = sv4_msb(v);
    if (top <= 0) return sv4_from_u64(0, 32, 0);
    int is_power_of_two = 1;
    for (int i = 0; i < sv4_nlimbs(v.width); ++i) {
        uint64_t word = v.bits[i];
        if (i == top / 64) word &= ~(UINT64_C(1) << (top % 64));
        if (word) { is_power_of_two = 0; break; }
    }
    return sv4_from_u64((uint64_t)(top + !is_power_of_two), 32, 0);
}

// ── sv4 arithmetic ────────────────────────────────────────────────────────────

static uint32_t sv4_maxw(sv4_t a, sv4_t b) {
    return a.width > b.width ? a.width : b.width;
}

static uint64_t sv4_extended_limb(sv4_t v, uint32_t width, int is_signed,
                                  int limb, int plane) {
    uint64_t mask = sv4_limb_mask(v.width, limb);
    uint64_t result = 0;
    if (mask) {
        const uint64_t* data = plane == 0 ? v.bits : plane == 1 ? v.x : v.z;
        result = data[limb] & mask;
    }
    if (is_signed && v.width && width > v.width) {
        int sign = sv4_lsb_bit(v, (int)v.width - 1);
        int fill = plane == 0 ? sign == 1 : plane == 1 ? sign == 2 : sign == 3;
        if (fill) result |= ~mask;
    }
    return result & sv4_limb_mask(width, limb);
}

static int sv4_extended_bit(sv4_t v, int bit, int is_signed) {
    if ((uint32_t)bit < v.width) return sv4_lsb_bit(v, bit);
    return is_signed && v.width ? sv4_lsb_bit(v, (int)v.width - 1) : 0;
}

// Portable 64x64 -> 128-bit multiplication using base-2^32 partial products.
static void sv4_multiply_words(uint64_t a, uint64_t b,
                               uint64_t* low, uint64_t* high) {
    uint64_t a0 = (uint32_t)a, a1 = a >> 32;
    uint64_t b0 = (uint32_t)b, b1 = b >> 32;
    uint64_t w0 = a0 * b0;
    uint64_t t = a1 * b0 + (w0 >> 32);
    uint64_t w1 = (uint32_t)t;
    uint64_t w2 = t >> 32;
    w1 += a0 * b1;
    *high = a1 * b1 + w2 + (w1 >> 32);
    *low = (w1 << 32) | (uint32_t)w0;
}

static void* sv4_scratch_alloc(size_t count);

sv4_t sv4_add(sv4_t a, sv4_t b) {
    uint32_t w = sv4_maxw(a, b);
    int8_t s = a.is_signed && b.is_signed;
    if (sv4_is_unknown(a) || sv4_is_unknown(b)) return sv4_x(w, s);
    sv4_t r = sv4_zero(w, s);
    int nl = sv4_nlimbs(w);
    uint64_t carry = 0;
    for (int i = 0; i < nl; i++) {
        uint64_t t = sv4_extended_limb(a, w, s, i, 0) + sv4_extended_limb(b, w, s, i, 0);
        uint64_t c1 = t < sv4_extended_limb(a, w, s, i, 0) ? 1 : 0;
        uint64_t u = t + carry;
        carry = c1 | (u < t ? 1 : 0);
        r.bits[i] = u;
    }
    if (nl > 0) r.bits[nl - 1] &= sv4_limb_mask(w, nl - 1);
    r.width = w;
    r.is_signed = s;
    return r;
}

sv4_t sv4_sub(sv4_t a, sv4_t b) {
    uint32_t w = sv4_maxw(a, b);
    int8_t s = a.is_signed && b.is_signed;
    if (sv4_is_unknown(a) || sv4_is_unknown(b)) return sv4_x(w, s);
    sv4_t r = sv4_zero(w, s);
    int nl = sv4_nlimbs(w);
    uint64_t borrow = 0;
    for (int i = 0; i < nl; i++) {
        uint64_t t = sv4_extended_limb(b, w, s, i, 0) + borrow; // mod 2^64
        uint64_t f = t < sv4_extended_limb(b, w, s, i, 0);      // b + borrow overflowed
        r.bits[i] = sv4_extended_limb(a, w, s, i, 0) - t;       // mod 2^64
        borrow = f || (sv4_extended_limb(a, w, s, i, 0) < t);
    }
    if (nl > 0) r.bits[nl - 1] &= sv4_limb_mask(w, nl - 1);
    r.width = w;
    r.is_signed = s;
    return r;
}

sv4_t sv4_mul(sv4_t a, sv4_t b) {
    uint32_t w = sv4_maxw(a, b);
    int8_t s = a.is_signed && b.is_signed;
    if (sv4_is_unknown(a) || sv4_is_unknown(b)) return sv4_x(w, s);
    int nl = sv4_nlimbs(w);
    // Schoolbook product modulo 2^w.  Terms at limb nl and above cannot affect
    // the truncated result, so avoid allocating or computing them.
    sv4_t r = sv4_zero(w, s);
    uint64_t* acc = r.bits;
    for (int i = 0; i < nl; i++) {
        for (int j = 0; j < nl - i; j++) {
            uint64_t plo, phi;
            sv4_multiply_words(sv4_extended_limb(a, w, s, i, 0),
                               sv4_extended_limb(b, w, s, j, 0), &plo, &phi);
            // add plo to acc[i+j], propagating the carry upward
            uint64_t t = acc[i + j] + plo;
            uint64_t carry = t < acc[i + j] ? 1 : 0;
            acc[i + j] = t;
            for (int k = i + j + 1; carry && k < nl; k++) {
                uint64_t t2 = acc[k] + 1;
                carry = t2 < acc[k] ? 1 : 0;
                acc[k] = t2;
            }
            // add phi to acc[i+j+1], propagating the carry upward
            if (i + j + 1 >= nl) continue;
            t = acc[i + j + 1] + phi;
            carry = t < acc[i + j + 1] ? 1 : 0;
            acc[i + j + 1] = t;
            for (int k = i + j + 2; carry && k < nl; k++) {
                uint64_t t2 = acc[k] + 1;
                carry = t2 < acc[k] ? 1 : 0;
                acc[k] = t2;
            }
        }
    }
    if (nl > 0) r.bits[nl - 1] &= sv4_limb_mask(w, nl - 1);
    r.width = w;
    r.is_signed = s;
    return r;
}

static int sv4_raw_nlimbs(const sv4_t* v) {
    int limbs = sv4_nlimbs(v->width);
    while (limbs > 0 && v->bits[limbs - 1] == 0) limbs--;
    return limbs;
}

static int sv4_raw_ucmp(const sv4_t* a, const sv4_t* b) {
    int a_limbs = sv4_raw_nlimbs(a);
    int b_limbs = sv4_raw_nlimbs(b);
    if (a_limbs != b_limbs) return a_limbs < b_limbs ? -1 : 1;
    for (int i = a_limbs - 1; i >= 0; i--) {
        if (a->bits[i] != b->bits[i])
            return a->bits[i] < b->bits[i] ? -1 : 1;
    }
    return 0;
}

static void* sv4_scratch_alloc(size_t count) {
    if (count > SIZE_MAX / sizeof(uint64_t)) {
        fputs("llg runtime fatal: value scratch size overflow\n", stderr);
        abort();
    }
    if (count == 0) count = 1;
    void* allocation = calloc(count, sizeof(uint64_t));
    if (allocation) return allocation;
    fprintf(stderr, "llg runtime fatal: value-operation allocation failed\n");
    abort();
}

// Divide known unsigned equal-width vectors in base 2^32. Quotient estimates
// use standard uint64_t intermediates, including on platforms without __int128.
// Scratch size depends on populated operands, never on a model-wide capacity.
static uint32_t sv4_word32(const sv4_t* value, int index) {
    return (uint32_t)(value->bits[index / 2] >> ((index % 2) * 32));
}

static void sv4_set_word32(sv4_t* value, int index, uint32_t word) {
    int shift = (index % 2) * 32;
    uint64_t mask = (uint64_t)UINT32_MAX << shift;
    value->bits[index / 2] = (value->bits[index / 2] & ~mask) |
                            ((uint64_t)word << shift);
}

static void sv4_unsigned_divmod(const sv4_t* dividend, const sv4_t* divisor,
                                int want_remainder, sv4_t* result) {
    if (sv4_raw_ucmp(dividend, divisor) < 0) {
        if (want_remainder) {
            size_t bytes = (size_t)sv4_nlimbs(dividend->width) * sizeof(uint64_t);
            if (bytes) memcpy(result->bits, dividend->bits, bytes);
        }
        return;
    }
    int n = 2 * sv4_raw_nlimbs(dividend);
    int m = 2 * sv4_raw_nlimbs(divisor);
    while (n && !sv4_word32(dividend, n - 1)) --n;
    while (m && !sv4_word32(divisor, m - 1)) --m;
    if (m == 1) {
        uint64_t remainder = 0;
        uint32_t denominator = sv4_word32(divisor, 0);
        for (int i = n - 1; i >= 0; --i) {
            uint64_t partial = (remainder << 32) | sv4_word32(dividend, i);
            if (!want_remainder)
                sv4_set_word32(result, i, (uint32_t)(partial / denominator));
            remainder = partial % denominator;
        }
        if (want_remainder) result->bits[0] = remainder;
        return;
    }

    size_t count = (size_t)n + 1u + (size_t)m;
    // Both lengths are bounded by the supported bit width. Check again here
    // so the allocation contract remains local if that boundary ever changes.
    if (count > SIZE_MAX / sizeof(uint32_t)) {
        fputs("llg runtime fatal: division scratch size overflow\n", stderr);
        abort();
    }
    uint32_t* scratch = (uint32_t*)calloc(count, sizeof(uint32_t));
    if (!scratch) {
        fputs("llg runtime fatal: division scratch allocation failed\n", stderr);
        abort();
    }
    uint32_t* u = scratch;
    uint32_t* v = u + n + 1;
    unsigned shift = 0;
    uint32_t top = sv4_word32(divisor, m - 1);
    while (!(top & UINT32_C(0x80000000))) { top <<= 1; ++shift; }
    for (int i = 0; i < m; ++i) {
        v[i] = sv4_word32(divisor, i) << shift;
        if (shift && i) v[i] |= sv4_word32(divisor, i - 1) >> (32u - shift);
    }
    for (int i = 0; i < n; ++i) {
        u[i] = sv4_word32(dividend, i) << shift;
        if (shift && i) u[i] |= sv4_word32(dividend, i - 1) >> (32u - shift);
    }
    if (shift) u[n] = sv4_word32(dividend, n - 1) >> (32u - shift);
    const uint64_t radix = UINT64_C(1) << 32;
    for (int j = n - m; j >= 0; --j) {
        uint64_t q, rem;
        if (u[j + m] == v[m - 1]) {
            q = UINT32_MAX;
            rem = (uint64_t)u[j + m - 1] + v[m - 1];
        } else {
            uint64_t numerator = ((uint64_t)u[j + m] << 32) | u[j + m - 1];
            q = numerator / v[m - 1];
            rem = numerator % v[m - 1];
        }
        while (rem < radix && q * v[m - 2] >
                                  (rem << 32) + u[j + m - 2]) {
            --q;
            rem += v[m - 1];
        }
        uint64_t borrow = 0;
        for (int i = 0; i < m; ++i) {
            uint64_t product = q * v[i] + borrow;
            uint32_t low = (uint32_t)product;
            uint32_t before = u[j + i];
            u[j + i] = before - low;
            borrow = (product >> 32) + (before < low);
        }
        uint32_t before = u[j + m];
        u[j + m] = before - (uint32_t)borrow;
        if ((uint64_t)before < borrow) {
            --q;
            uint64_t carry = 0;
            for (int i = 0; i < m; ++i) {
                uint64_t sum = (uint64_t)u[j + i] + v[i] + carry;
                u[j + i] = (uint32_t)sum;
                carry = sum >> 32;
            }
            u[j + m] += (uint32_t)carry;
        }
        if (!want_remainder) sv4_set_word32(result, j, (uint32_t)q);
    }
    if (want_remainder) {
        for (int i = 0; i < m; ++i) {
            uint32_t word = u[i] >> shift;
            if (shift) word |= u[i + 1] << (32u - shift);
            sv4_set_word32(result, i, word);
        }
    }
    free(scratch);
}

static int sv4_is_negative(sv4_t v) {
    return v.is_signed && v.width > 0 &&
           sv4_lsb_bit(v, (int)v.width - 1) == 1;
}

static sv4_t sv4_divmod(sv4_t a, sv4_t b, int want_remainder) {
    uint32_t width = sv4_maxw(a, b);
    int8_t is_signed = a.is_signed && b.is_signed;
    if (sv4_is_unknown(a) || sv4_is_unknown(b))
        return sv4_x(width, is_signed);
    a = sv4_resize(a, width, is_signed);
    b = sv4_resize(b, width, is_signed);
    if (sv4_raw_nlimbs(&b) == 0) {
        sv4_destroy(&a);
        sv4_destroy(&b);
        return sv4_x(width, is_signed);
    }

    int dividend_negative = is_signed && sv4_is_negative(a);
    int divisor_negative = is_signed && sv4_is_negative(b);
    if (dividend_negative) sv4_replace(&a, sv4_neg(a));
    if (divisor_negative) sv4_replace(&b, sv4_neg(b));

    sv4_t result = sv4_zero(width, is_signed);
    result.width = width;
    result.is_signed = is_signed;
    sv4_unsigned_divmod(&a, &b, want_remainder, &result);
    if ((want_remainder && dividend_negative) ||
        (!want_remainder && dividend_negative != divisor_negative)) {
        sv4_replace(&result, sv4_neg(result));
    }
    sv4_destroy(&a);
    sv4_destroy(&b);
    return result;
}

sv4_t sv4_div(sv4_t a, sv4_t b) { return sv4_divmod(a, b, 0); }

sv4_t sv4_mod(sv4_t a, sv4_t b) { return sv4_divmod(a, b, 1); }

static int sv4_is_all_ones(sv4_t v) {
    for (unsigned i = 0; i < (unsigned)sv4_nlimbs(v.width); i++) {
        uint64_t mask = sv4_limb_mask(v.width, (int)i);
        if ((v.bits[i] & mask) != mask) {
            return 0;
        }
    }
    return v.width != 0;
}

sv4_t sv4_pow(sv4_t a, sv4_t b) {
    uint32_t width = a.width;
    int8_t is_signed = a.is_signed;
    if (sv4_is_unknown(a) || sv4_is_unknown(b))
        return sv4_x(width, is_signed);
    if (sv4_is_negative(b)) {
        if (sv4_raw_nlimbs(&a) == 0) return sv4_x(width, is_signed);
        if (sv4_is_negative(a) && sv4_is_all_ones(a)) {
            return (b.bits[0] & 1ULL)
                ? sv4_clone(&a)
                : sv4_from_u64(1, width, is_signed);
        }
        if (sv4_raw_nlimbs(&a) == 1 && a.bits[0] == 1)
            return sv4_from_u64(1, width, is_signed);
        return sv4_from_u64(0, width, is_signed);
    }

    sv4_t result = sv4_from_u64(1, width, is_signed);
    sv4_t base = sv4_clone(&a);
    int exponent_msb = sv4_msb(b);
    for (int bit = 0; bit <= exponent_msb; bit++) {
        if (sv4_lsb_bit(b, bit) == 1) sv4_replace(&result, sv4_mul(result, base));
        if (bit != exponent_msb) sv4_replace(&base, sv4_mul(base, base));
    }
    sv4_destroy(&base);
    return result;
}

sv4_t sv4_neg(sv4_t a) {
    uint32_t w = a.width;
    if (sv4_is_unknown(a)) return sv4_x(w, a.is_signed);
    sv4_t r = sv4_zero(w, a.is_signed);
    int nl = sv4_nlimbs(w);
    uint64_t carry = 1; // two's complement: ~a + 1
    for (int i = 0; i < nl; i++) {
        uint64_t t = ~a.bits[i] + carry;
        carry = t < (~a.bits[i]) ? 1 : 0;
        r.bits[i] = t;
    }
    if (nl > 0) r.bits[nl - 1] &= sv4_limb_mask(w, nl - 1);
    r.width = w;
    r.is_signed = a.is_signed;
    return r;
}

sv4_t sv4_bitneg(sv4_t a) {
    sv4_t r = sv4_zero(a.width, a.is_signed);
    for (int i = 0; i < sv4_nlimbs(a.width); i++) {
        uint64_t m = sv4_limb_mask(a.width, i);
        // X stays X; Z degrades to X (IEEE 4-state NOT table: NOT z = x).
        uint64_t unk = (a.x[i] | a.z[i]) & m;
        r.bits[i] = (~a.bits[i]) & ~unk & m;
        r.x[i] = unk;
        r.z[i] = 0;
    }
    r.width = a.width;
    r.is_signed = a.is_signed;
    return r;
}

// Logical truth is determined by a known one even when other bits are X/Z;
// without a known one, an unknown bit leaves the truth value indeterminate.
static int sv4_logical_truth(sv4_t v) {
    int unknown = 0;
    for (int i = 0; i < (int)v.width; i++) {
        int bit = sv4_lsb_bit(v, i);
        if (bit == 1) return 1;
        if (bit >= 2) unknown = 1;
    }
    return unknown ? 2 : 0;
}

sv4_t sv4_lognot(sv4_t a) {
    int truth = sv4_logical_truth(a);
    if (truth == 2) return SV4_X(1);
    return sv4_from_u64(truth ? 0 : 1, 1, 0);
}

sv4_t sv4_logand(sv4_t a, sv4_t b) {
    int ta = sv4_logical_truth(a), tb = sv4_logical_truth(b);
    if (ta == 0 || tb == 0) return SV4_C(0, 1);
    if (ta == 1 && tb == 1) return SV4_C(1, 1);
    return SV4_X(1);
}

sv4_t sv4_logor(sv4_t a, sv4_t b) {
    int ta = sv4_logical_truth(a), tb = sv4_logical_truth(b);
    if (ta == 1 || tb == 1) return SV4_C(1, 1);
    if (ta == 0 && tb == 0) return SV4_C(0, 1);
    return SV4_X(1);
}

sv4_t sv4_logimpl(sv4_t a, sv4_t b) {
    int ta = sv4_logical_truth(a), tb = sv4_logical_truth(b);
    if (ta == 0 || tb == 1) return SV4_C(1, 1);
    if (ta == 1 && tb == 0) return SV4_C(0, 1);
    return SV4_X(1);
}

sv4_t sv4_logequiv(sv4_t a, sv4_t b) {
    int ta = sv4_logical_truth(a), tb = sv4_logical_truth(b);
    if (ta == 2 || tb == 2) return SV4_X(1);
    return SV4_C(ta == tb, 1);
}

// Per-bit AND/OR/XOR/XNOR with operands resized to the result width.
static sv4_t sv4_bitwise(sv4_t a, sv4_t b, int op) {
    uint32_t w = sv4_maxw(a, b);
    int8_t s = a.is_signed && b.is_signed;
    sv4_t r = sv4_zero(w, s);
    r.width = w;
    for (int i = 0; i < (int)w; i++) {
        int ab = sv4_extended_bit(a, i, s), ax = ab >= 2; // Z behaves as X here (LRM 11.4.5)
        int bb = sv4_extended_bit(b, i, s), bx = bb >= 2;
        int o_b = 0, o_x = 0;
        if (op == 0) { // AND: 0 dominates
            if ((!ax && !ab) || (!bx && !bb)) o_b = 0;
            else if (!ax && ab && !bx && bb) o_b = 1;
            else o_x = 1;
        } else if (op == 1) { // OR: 1 dominates
            if ((!ax && ab) || (!bx && bb)) o_b = 1;
            else if (!ax && !ab && !bx && !bb) o_b = 0;
            else o_x = 1;
        } else if (op == 2) { // XOR
            if (ax || bx) o_x = 1;
            else o_b = ab ^ bb;
        } else { // XNOR
            if (ax || bx) o_x = 1;
            else o_b = !(ab ^ bb);
        }
        sv4_lsb_bit_set(&r, i, o_x ? 2 : o_b);
    }
    r.width = w;
    r.is_signed = s;
    return r;
}

sv4_t sv4_and(sv4_t a, sv4_t b) { return sv4_bitwise(a, b, 0); }
sv4_t sv4_or(sv4_t a, sv4_t b) { return sv4_bitwise(a, b, 1); }
sv4_t sv4_xor(sv4_t a, sv4_t b) { return sv4_bitwise(a, b, 2); }
sv4_t sv4_xnor(sv4_t a, sv4_t b) { return sv4_bitwise(a, b, 3); }

static sv4_t sv4_reduce(sv4_t a, int op) {
    int ones = 0, zeros = 0, unknown = 0;
    for (int i = 0; i < (int)a.width; i++) {
        int bit = sv4_lsb_bit(a, i);
        if (bit == 1) ones++;
        else if (bit == 0) zeros++;
        else unknown = 1;
    }
    int r;
    switch (op) {
        case 0: // AND: a known zero dominates; otherwise X beats all-one.
            if (zeros != 0) return SV4_C(0, 1);
            if (unknown) return SV4_X(1);
            r = 1;
            break;
        case 1: // OR: a known one dominates; otherwise X beats all-zero.
            if (ones != 0) return SV4_C(1, 1);
            if (unknown) return SV4_X(1);
            r = 0;
            break;
        case 2: // XOR: any X/Z makes the parity unknowable.
            if (unknown) return SV4_X(1);
            r = ones & 1;
            break;
        case 3:
            if (zeros != 0) r = 1;
            else if (unknown) return SV4_X(1);
            else r = 0;
            break;
        case 4:
            if (ones != 0) r = 0;
            else if (unknown) return SV4_X(1);
            else r = 1;
            break;
        default:
            if (unknown) return SV4_X(1);
            r = !(ones & 1);
            break;
    }
    return sv4_from_u64(r, 1, 0);
}

sv4_t sv4_reduce_and(sv4_t a) { return sv4_reduce(a, 0); }
sv4_t sv4_reduce_or(sv4_t a) { return sv4_reduce(a, 1); }
sv4_t sv4_reduce_xor(sv4_t a) { return sv4_reduce(a, 2); }
sv4_t sv4_reduce_nand(sv4_t a) { return sv4_reduce(a, 3); }
sv4_t sv4_reduce_nor(sv4_t a) { return sv4_reduce(a, 4); }
sv4_t sv4_reduce_xnor(sv4_t a) { return sv4_reduce(a, 5); }

static sv4_t sv4_shift(sv4_t a, sv4_t b, int right, int arith) {
    uint32_t w = a.width;
    if (sv4_is_unknown(b)) return sv4_x(w, a.is_signed);
    int oversized = 0;
    if (b.width > 64) {
        for (int i = 1; i < sv4_nlimbs(b.width); i++)
            if (b.bits[i]) {
                oversized = 1;
                break;
            }
    }
    uint64_t sh = sv4_to_u64(b);
    if (oversized || sh >= w) {
        if (right && arith && a.is_signed && w > 0)
            return sv4_fill((uint8_t)sv4_lsb_bit(a, (int)w - 1), w, a.is_signed);
        return sv4_from_u64(0, w, a.is_signed);
    }
    sv4_t r = sv4_zero(w, a.is_signed);
    r.width = w;
    r.is_signed = a.is_signed;
    int nsh = (int)sh;
    if (!right) {
        for (int i = nsh; i < (int)w; i++)
            sv4_lsb_bit_set(&r, i, sv4_lsb_bit(a, i - nsh));
    } else {
        for (int i = 0; i + nsh < (int)w; i++)
            sv4_lsb_bit_set(&r, i, sv4_lsb_bit(a, i + nsh));
        if (arith && a.is_signed && nsh > 0) {
            int msb = sv4_lsb_bit(a, (int)w - 1); // 2 = X fills X
            for (int i = (int)w - nsh; i < (int)w; i++)
                sv4_lsb_bit_set(&r, i, msb);
        }
    }
    return r;
}

sv4_t sv4_shl(sv4_t a, sv4_t b) { return sv4_shift(a, b, 0, 0); }
sv4_t sv4_shr(sv4_t a, sv4_t b) { return sv4_shift(a, b, 1, 0); }
sv4_t sv4_ashl(sv4_t a, sv4_t b) { return sv4_shift(a, b, 0, 1); }
sv4_t sv4_ashr(sv4_t a, sv4_t b) { return sv4_shift(a, b, 1, 1); }

static sv4_t sv4_cmp_bit(int known, int val) {
    if (!known) return SV4_X(1);
    return sv4_from_u64(val, 1, 0);
}

// Unsigned MSB-first limb compare of equal-width values: -1, 0 or +1.
static int sv4_ucmp(sv4_t a, sv4_t b) {
    uint32_t width = sv4_maxw(a, b);
    int is_signed = a.is_signed && b.is_signed;
    for (int i = sv4_nlimbs(width) - 1; i >= 0; --i) {
        uint64_t left = sv4_extended_limb(a, width, is_signed, i, 0);
        uint64_t right = sv4_extended_limb(b, width, is_signed, i, 0);
        if (left != right) return left < right ? -1 : 1;
    }
    return 0;
}

sv4_t sv4_eq(sv4_t a, sv4_t b) {
    uint32_t w = sv4_maxw(a, b);
    int8_t s = a.is_signed && b.is_signed;
    int unknown = 0;
    for (int i = 0; i < sv4_nlimbs(w); i++) {
        uint64_t mask = sv4_limb_mask(w, i);
        uint64_t either_unknown = (sv4_extended_limb(a, w, s, i, 1) | sv4_extended_limb(a, w, s, i, 2) | sv4_extended_limb(b, w, s, i, 1) | sv4_extended_limb(b, w, s, i, 2)) & mask;
        uint64_t both_known = mask & ~either_unknown;
        if (((sv4_extended_limb(a, w, s, i, 0) ^ sv4_extended_limb(b, w, s, i, 0)) & both_known) != 0) return SV4_C(0, 1);
        unknown |= either_unknown != 0;
    }
    return unknown ? SV4_X(1) : SV4_C(1, 1);
}

sv4_t sv4_neq(sv4_t a, sv4_t b) {
    sv4_t result = sv4_eq(a, b);
    if (sv4_is_unknown(result)) return result;
    result.bits[0] ^= 1u;
    return result;
}

sv4_t sv4_case_eq(sv4_t a, sv4_t b) {
    uint32_t width = sv4_maxw(a, b);
    int is_signed = a.is_signed && b.is_signed;
    for (int i = 0; i < sv4_nlimbs(width); ++i) {
        for (int plane = 0; plane < 3; ++plane) {
            if (sv4_extended_limb(a, width, is_signed, i, plane) !=
                sv4_extended_limb(b, width, is_signed, i, plane))
                return sv4_from_u64(0, 1, 0);
        }
    }
    return sv4_from_u64(1, 1, 0);
}

sv4_t sv4_enum_navigate(sv4_t current, sv4_t step, const sv4_t* values,
                        uint32_t count, sv4_t default_value, int direction) {
    if (!values || count == 0) return sv4_clone(&default_value);
    uint32_t found = count;
    for (uint32_t index = 0; index < count; ++index) {
        // Keep scanning after a match so aliases have one deterministic,
        // declaration-order policy: the last matching member wins.
        sv4_t match = sv4_case_eq(current, values[index]);
        if (sv4_to_bool(match)) found = index;
        sv4_destroy(&match);
    }
    if (found == count) return sv4_clone(&default_value);
    // The lowering converts the optional int unsigned step to a 2-state
    // 32-bit value. Keep this defensive normalization for direct callers.
    uint64_t distance = sv4_is_unknown(step) ? 0 : sv4_to_u64(step);
    uint32_t offset = (uint32_t)(distance % (uint64_t)count);
    uint32_t target;
    if (direction < 0)
        target = (found + count - offset) % count;
    else
        target = (found + offset) % count;
    return sv4_clone(&values[target]);
}

sv4_t sv4_case_neq(sv4_t a, sv4_t b) {
    sv4_t r = sv4_case_eq(a, b);
    r.bits[0] ^= 1u;
    return r;
}

sv4_t sv4_wild_eq(sv4_t lhs, sv4_t rhs) {
    uint32_t w = sv4_maxw(lhs, rhs);
    int8_t s = lhs.is_signed && rhs.is_signed;
    int unknown = 0;
    for (int i = 0; i < sv4_nlimbs(w); i++) {
        uint64_t mask = sv4_limb_mask(w, i);
        uint64_t wildcard = (sv4_extended_limb(rhs, w, s, i, 1) | sv4_extended_limb(rhs, w, s, i, 2)) & mask;
        uint64_t care = mask & ~wildcard;
        uint64_t left_unknown = (sv4_extended_limb(lhs, w, s, i, 1) | sv4_extended_limb(lhs, w, s, i, 2)) & care;
        uint64_t known = care & ~left_unknown;
        if (((sv4_extended_limb(lhs, w, s, i, 0) ^ sv4_extended_limb(rhs, w, s, i, 0)) & known) != 0) return SV4_C(0, 1);
        unknown |= left_unknown != 0;
    }
    return unknown ? SV4_X(1) : SV4_C(1, 1);
}

sv4_t sv4_wild_neq(sv4_t lhs, sv4_t rhs) {
    sv4_t result = sv4_wild_eq(lhs, rhs);
    if (sv4_is_unknown(result)) return result;
    result.bits[0] ^= 1u;
    return result;
}

// casez per LRM 12.5.1: a z (or ?) bit in the case ITEM is a don't-care; an x
// in the item matches an x selector bit only; a known item bit must equal the
// selector bit (a selector x/z never matches a known item bit).  Operands are
// zero-extended to max width before comparing, like `case`.
sv4_t sv4_casez_eq(sv4_t sel, sv4_t item) {
    uint32_t w = sv4_maxw(sel, item);
    for (int i = 0; i < (int)w; i++) {
        int ib = sv4_extended_bit(item, i, 0); // 0/1/2(x)/3(z)
        if (ib == 3) continue;       // item z/? -> don't-care
        int sb = sv4_extended_bit(sel, i, 0);
        if (ib == 2) {               // item x matches selector x only
            if (sb != 2) return SV4_C(0, 1);
        } else if (sb != ib) {       // known item: selector must equal it
            return SV4_C(0, 1);
        }
    }
    return SV4_C(1, 1);
}

// casex per LRM 12.5.1: x/z (or ?) bits in the ITEM are don't-cares, and a
// selector x/z is a don't-care against a known item bit too — only an
// opposite known bit fails the match.
sv4_t sv4_casex_eq(sv4_t sel, sv4_t item) {
    uint32_t w = sv4_maxw(sel, item);
    for (int i = 0; i < (int)w; i++) {
        int ib = sv4_extended_bit(item, i, 0);
        if (ib >= 2) continue;       // item x/z -> don't-care
        int sb = sv4_extended_bit(sel, i, 0);
        if (sb == ib) continue;      // equal known bits match
        if (sb >= 2) continue;       // selector x/z is a don't-care in casex
        return SV4_C(0, 1);          // opposite known bit -> no match
    }
    return SV4_C(1, 1);
}

static sv4_t sv4_cmp(sv4_t a, sv4_t b, int op) {
    if (sv4_is_unknown(a) || sv4_is_unknown(b)) return sv4_x(1, 0);
    int c = sv4_ucmp(a, b);
    if (a.is_signed && b.is_signed) {
        int an = sv4_is_negative(a), bn = sv4_is_negative(b);
        if (an != bn) c = an ? -1 : 1;
    }
    int result = op == 0 ? c < 0 : op == 1 ? c <= 0 : op == 2 ? c > 0 : c >= 0;
    return sv4_cmp_bit(1, result);
}

sv4_t sv4_lt(sv4_t a, sv4_t b) { return sv4_cmp(a, b, 0); }
sv4_t sv4_le(sv4_t a, sv4_t b) { return sv4_cmp(a, b, 1); }
sv4_t sv4_gt(sv4_t a, sv4_t b) { return sv4_cmp(a, b, 2); }
sv4_t sv4_ge(sv4_t a, sv4_t b) { return sv4_cmp(a, b, 3); }

sv4_t sv4_inside_range(sv4_t value, sv4_t low, sv4_t high) {
    sv4_t ge = sv4_ge(value, low);
    sv4_t le = sv4_le(value, high);
    sv4_t result = sv4_logand(ge, le);
    sv4_destroy(&ge);
    sv4_destroy(&le);
    return result;
}

sv4_t sv4_mux(sv4_t sel, sv4_t a, sv4_t b) {
    uint32_t w = sv4_maxw(a, b);
    int8_t s = a.is_signed && b.is_signed;
    int truth = sv4_logical_truth(sel);
    if (truth == 1) return sv4_resize(a, w, s);
    if (truth == 0) return sv4_resize(b, w, s);
    sv4_t r = sv4_zero(w, s);
    r.width = w;
    r.is_signed = s;
    for (int i = 0; i < (int)w; i++) {
        int ab = sv4_extended_bit(a, i, s), bb = sv4_extended_bit(b, i, s);
        sv4_lsb_bit_set(&r, i, ab == bb ? ab : 2);
    }
    return r;
}

sv4_t sv4_concat(sv4_t hi, sv4_t lo) {
    uint64_t total = (uint64_t)hi.width + (uint64_t)lo.width;
    sv4_require_width(total, "concatenation");
    sv4_t r = sv4_zero((uint32_t)total, 0);
    r.width = (uint32_t)total;
    r.is_signed = 0;
    for (int i = 0; i < (int)lo.width; i++)
        sv4_lsb_bit_set(&r, i, sv4_lsb_bit(lo, i));
    for (int i = 0; i < (int)hi.width && (int)lo.width + i < (int)total; i++)
        sv4_lsb_bit_set(&r, (int)lo.width + i, sv4_lsb_bit(hi, i));
    return r;
}

sv4_t sv4_repeat(sv4_t pat, uint64_t n) {
    if (pat.width != 0 && n > UINT64_MAX / pat.width) {
        sv4_require_width(UINT64_MAX, "replication");
    }
    uint64_t w_total64 = (uint64_t)pat.width * n;
    sv4_require_width(w_total64, "replication");
    uint32_t w_total = (uint32_t)w_total64;
    sv4_t r = sv4_zero(w_total, 0);
    r.width = w_total;
    r.is_signed = 0;
    for (uint64_t rep = 0; rep < n; rep++) {
        uint64_t dst64 = rep * (uint64_t)pat.width;
        if (dst64 >= w_total64) break;
        int dst = (int)dst64;
        for (int i = 0; i < (int)pat.width && dst + i < (int)w_total; i++)
            sv4_lsb_bit_set(&r, dst + i, sv4_lsb_bit(pat, i));
    }
    return r;
}

sv4_t sv4_stream(sv4_t value, uint32_t slice, int right_to_left) {
    if (slice == 0) {
        fprintf(stderr, "llg runtime fatal: zero streaming slice size\n");
        abort();
    }
    sv4_t r = sv4_zero(value.width, 0);
    r.width = value.width;
    r.is_signed = 0;
    if (!right_to_left || value.width == 0 || slice >= value.width) {
        int limbs = sv4_nlimbs(value.width);
        for (int i = 0; i < limbs; i++) {
            r.bits[i] = value.bits[i];
            r.x[i] = value.x[i];
            r.z[i] = value.z[i];
        }
        return r;
    }
    uint64_t width = value.width;
    for (uint64_t src = 0; src < width; src++) {
        uint64_t block = src / slice;
        uint64_t offset = src % slice;
        uint64_t consumed = (block + 1) * (uint64_t)slice;
        if (consumed > width) consumed = width;
        uint64_t dst = width - consumed + offset;
        sv4_lsb_bit_set(&r, (int)dst, sv4_lsb_bit(value, (int)src));
    }
    return r;
}

sv4_t sv4_unstream(sv4_t value, uint32_t slice, int right_to_left) {
    if (slice == 0) {
        fprintf(stderr, "llg runtime fatal: zero streaming slice size\n");
        abort();
    }
    sv4_t r = sv4_zero(value.width, 0);
    r.width = value.width;
    r.is_signed = 0;
    if (!right_to_left || value.width == 0 || slice >= value.width) {
        int limbs = sv4_nlimbs(value.width);
        for (int i = 0; i < limbs; i++) {
            r.bits[i] = value.bits[i];
            r.x[i] = value.x[i];
            r.z[i] = value.z[i];
        }
        return r;
    }
    uint64_t width = value.width;
    for (uint64_t dst = 0; dst < width; dst++) {
        uint64_t block = dst / slice;
        uint64_t offset = dst % slice;
        uint64_t consumed = (block + 1) * (uint64_t)slice;
        if (consumed > width) consumed = width;
        uint64_t src = width - consumed + offset;
        sv4_lsb_bit_set(&r, (int)dst, sv4_lsb_bit(value, (int)src));
    }
    return r;
}

sv4_t sv4_bit_select(sv4_t v, uint64_t i) {
    if (i >= v.width) return SV4_X(1);
    int b = sv4_lsb_bit(v, (int)i);
    if (b == 2) return SV4_X(1);
    if (b == 3) return SV4_Z(1);
    return sv4_from_u64(b, 1, 0);
}

void sv4_bit_select_set(sv4_t* tgt, uint64_t i, sv4_t value) {
    if (i >= tgt->width) return;
    sv4_lsb_bit_set(tgt, (int)i, sv4_lsb_bit(value, 0));
}

static uint32_t llg_part_select_width(int64_t left, int64_t right) {
    uint64_t delta = left >= right
        ? (uint64_t)left - (uint64_t)right
        : (uint64_t)right - (uint64_t)left;
    if (delta == UINT64_MAX) sv4_require_width(delta, "part-select");
    sv4_require_width(delta + 1, "part-select");
    return (uint32_t)(delta + 1);
}

sv4_t sv4_part_select(sv4_t v, int64_t left, int64_t right) {
    uint32_t w = llg_part_select_width(left, right);
    sv4_t r = sv4_zero(w, 0);
    r.width = w;
    int64_t step = left > right ? -1 : 1;
    int out = 0;
    for (int64_t i = left; ; i += step) {
        int b = i < 0 || i >= (int64_t)v.width ? 2 : sv4_lsb_bit(v, (int)i);
        int pos = w - 1 - out; // first index (left) is the MSB
        sv4_lsb_bit_set(&r, pos, b);
        out++;
        if (i == right) break;
    }
    return r;
}

void sv4_part_select_set(sv4_t* tgt, int64_t left, int64_t right, sv4_t value) {
    (void)llg_part_select_width(left, right);
    sv4_t snapshot = SV4_EMPTY;
    if (tgt->bits && tgt->bits == value.bits) {
        snapshot = sv4_clone(&value);
        value = snapshot;
    }
    int64_t step = left > right ? -1 : 1;
    int in = (int)value.width - 1; // value MSB maps to the first target index
    for (int64_t i = left; ; i += step) {
        if (i < 0 || i >= (int64_t)tgt->width) {
            in--;
            if (i == right) break;
            continue;
        }
        sv4_lsb_bit_set(tgt, (int)i, sv4_lsb_bit(value, in));
        in--;
        if (i == right) break;
    }
    sv4_destroy(&snapshot);
}

sv4_t sv4_idx_part_select(sv4_t v, uint64_t base, uint32_t width, int neg) {
    sv4_require_width(width, "indexed part-select");
    sv4_t r = sv4_zero(width, 0);
    r.width = width;
    for (uint32_t output_bit = 0; output_bit < width; output_bit++) {
        uint64_t source_bit;
        int in_range;
        if (!neg) {
            source_bit = base + output_bit;
            in_range = source_bit >= base && source_bit < v.width;
        } else {
            uint64_t distance = (uint64_t)width - 1 - output_bit;
            in_range = base >= distance;
            source_bit = in_range ? base - distance : 0;
            in_range = in_range && source_bit < v.width;
        }
        sv4_lsb_bit_set(&r, (int)output_bit,
                        in_range ? sv4_lsb_bit(v, (int)source_bit) : 2);
    }
    return r;
}

void sv4_idx_part_select_set(sv4_t* tgt, uint64_t base, uint32_t width, int neg,
                             sv4_t value) {
    sv4_require_width(width, "indexed part-select");
    sv4_t snapshot = SV4_EMPTY;
    if (tgt->bits && tgt->bits == value.bits) {
        snapshot = sv4_clone(&value);
        value = snapshot;
    }
    for (uint32_t value_bit = 0; value_bit < width; value_bit++) {
        uint64_t target_bit;
        int in_range;
        if (!neg) {
            target_bit = base + value_bit;
            in_range = target_bit >= base && target_bit < tgt->width;
        } else {
            uint64_t distance = (uint64_t)width - 1 - value_bit;
            in_range = base >= distance;
            target_bit = in_range ? base - distance : 0;
            in_range = in_range && target_bit < tgt->width;
        }
        if (in_range)
            sv4_lsb_bit_set(tgt, (int)target_bit,
                            sv4_lsb_bit(value, (int)value_bit));
    }
    sv4_destroy(&snapshot);
}

static int sv4_indexed_source(int64_t base, uint32_t width,
                              uint32_t output_bit, int neg,
                              uint64_t* source_bit) {
    if (!neg) {
        if (base >= 0) {
            *source_bit = (uint64_t)base + output_bit;
            return 1;
        }
        uint64_t magnitude = 0 - (uint64_t)base;
        if ((uint64_t)output_bit < magnitude) return 0;
        *source_bit = (uint64_t)output_bit - magnitude;
        return 1;
    }
    uint64_t distance = (uint64_t)width - 1 - output_bit;
    if (base < 0 || (uint64_t)base < distance) return 0;
    *source_bit = (uint64_t)base - distance;
    return 1;
}

sv4_t sv4_idx_part_select_value(sv4_t v, sv4_t base, uint32_t width, int neg) {
    sv4_require_width(width, "indexed part-select");
    int64_t signed_base;
    if (!sv4_to_index_i64(base, &signed_base)) return sv4_x(width, 0);
    sv4_t result = sv4_x(width, 0);
    for (uint32_t output_bit = 0; output_bit < width; output_bit++) {
        uint64_t source_bit;
        if (sv4_indexed_source(signed_base, width, output_bit, neg, &source_bit) &&
            source_bit < v.width) {
            sv4_lsb_bit_set(&result, (int)output_bit,
                            sv4_lsb_bit(v, (int)source_bit));
        }
    }
    return result;
}

void sv4_idx_part_select_set_value(sv4_t* tgt, sv4_t base, uint32_t width,
                                   int neg, sv4_t value) {
    sv4_require_width(width, "indexed part-select");
    int64_t signed_base;
    if (!sv4_to_index_i64(base, &signed_base)) return;
    sv4_t snapshot = SV4_EMPTY;
    if (tgt->bits && tgt->bits == value.bits) {
        snapshot = sv4_clone(&value);
        value = snapshot;
    }
    for (uint32_t value_bit = 0; value_bit < width; value_bit++) {
        uint64_t target_bit;
        if (sv4_indexed_source(signed_base, width, value_bit, neg, &target_bit) &&
            target_bit < tgt->width) {
            sv4_lsb_bit_set(tgt, (int)target_bit,
                            sv4_lsb_bit(value, (int)value_bit));
        }
    }
    sv4_destroy(&snapshot);
}

sv4_t llg_ref_read(const llg_ref_t* ref) {
    if (!ref) return sv4_x(1, 0);
    if ((llg_ref_kind_t)ref->kind == LLG_REF_QUEUE) {
        if (ref->retained_read) return ref->retained_read(ref->retained);
        if (!ref->queue_read)
            return ref->two_state
                       ? sv4_from_u64(0, ref->width, ref->is_signed)
                       : sv4_x(ref->width ? ref->width : 1, ref->is_signed);
        sv4_t value = ref->queue_read(ref->queue, ref->queue_identity);
        sv4_replace(&value, sv4_cast(value, ref->width, ref->is_signed));
        if (ref->two_state) sv4_replace(&value, sv4_to_two_state(value));
        return value;
    }
    if (!ref->base) return sv4_x(1, 0);
    sv4_t value;
    switch ((llg_ref_kind_t)ref->kind) {
    case LLG_REF_WHOLE:
        value = sv4_clone(ref->base);
        break;
    case LLG_REF_BIT:
        value = sv4_bit_select(*ref->base, ref->index);
        break;
    case LLG_REF_PART:
        value = sv4_part_select(*ref->base, ref->left, ref->right);
        break;
    case LLG_REF_INDEXED:
        value = sv4_idx_part_select(*ref->base, ref->index,
                                    ref->indexed_width,
                                    ref->indexed_negative);
        break;
    case LLG_REF_ARRAY:
        if (ref->index == UINT64_MAX || ref->index >= ref->array_size)
            return ref->two_state ? sv4_from_u64(0, ref->width, ref->is_signed)
                                  : sv4_x(ref->width ? ref->width : 1, ref->is_signed);
        value = sv4_clone(&ref->base[ref->index]);
        break;
    default:
        return sv4_x(ref->width ? ref->width : 1, ref->is_signed);
    }
    sv4_replace(&value, sv4_cast(value, ref->width, ref->is_signed));
    if (ref->two_state) sv4_replace(&value, sv4_to_two_state(value));
    return value;
}

// ── Formatting ────────────────────────────────────────────────────────────────

static void llg_append(char* buf, size_t cap, size_t* len, char c) {
    if (*len + 1 < cap) buf[(*len)++] = c;
}

// Unsigned decimal via repeated long division by 10 across the limbs.  A
// signed value (`is_signed`) with the sign bit set prints '-' followed by its
// two's-complement magnitude (~v + 1 within the value's width).
void sv4_to_dec_string(sv4_t v, char* buf, size_t cap) {
    if (cap == 0) return;
    if (sv4_is_unknown(v)) {
        if (cap > 1) {
            buf[0] = 'x';
            buf[1] = 0;
        } else {
            buf[0] = 0;
        }
        return;
    }
    int negative = 0;
    int nl = sv4_nlimbs(v.width);
    uint64_t* tmp = sv4_scratch_alloc((size_t)(nl > 0 ? nl : 1));
    if (v.is_signed && v.width > 0 && sv4_lsb_bit(v, (int)v.width - 1) == 1) {
        negative = 1;
        sv4_t mag = sv4_neg(v);
        for (int i = 0; i < nl; i++) tmp[i] = mag.bits[i];
        sv4_destroy(&mag);
    } else {
        for (int i = 0; i < nl; i++) tmp[i] = v.bits[i];
    }
    if (nl > 0) tmp[nl - 1] &= sv4_limb_mask(v.width, nl - 1);
    size_t digit_capacity = ((size_t)v.width * 30103u) / 100000u + 2u;
    char* digits = malloc(digit_capacity);
    if (!digits) {
        free(tmp);
        fprintf(stderr, "llg runtime fatal: value-format allocation failed\n");
        abort();
    }
    size_t n = 0;
    for (;;) {
        int nonzero = 0;
        for (int i = 0; i < nl; i++)
            if (tmp[i]) { nonzero = 1; break; }
        if (!nonzero) break;
        uint64_t rem = 0;
        for (int i = nl - 1; i >= 0; i--) {
            uint64_t high = (rem << 32) | (tmp[i] >> 32);
            uint64_t low = ((high % 10u) << 32) | (uint32_t)tmp[i];
            tmp[i] = ((high / 10u) << 32) | (low / 10u);
            rem = low % 10u;
        }
        if (n < digit_capacity) digits[n++] = (char)('0' + (int)rem);
    }
    size_t len = 0;
    if (negative) llg_append(buf, cap, &len, '-');
    if (n == 0) {
        llg_append(buf, cap, &len, '0');
    } else {
        while (n > 0) llg_append(buf, cap, &len, digits[--n]);
    }
    buf[len] = 0;
    free(digits);
    free(tmp);
}

void sv4_format(char fmt, sv4_t v, char* buf, size_t cap) {
    if (cap == 0) return;
    size_t len = 0;
    buf[0] = 0;
    switch (fmt) {
        case 'd':
            sv4_to_dec_string(v, buf, cap);
            return;
        case 'b':
            for (int i = (int)v.width - 1; i >= 0; i--) {
                int b = sv4_lsb_bit(v, i);
                llg_append(buf, cap, &len, b == 2 ? 'x' : b == 3 ? 'z' : (b ? '1' : '0'));
            }
            break;
        case 'h': {
            int digits = ((int)v.width + 3) / 4;
            for (int d = digits - 1; d >= 0; d--) {
                int has_x = 0, has_z = 0, val = 0;
                for (int k = 0; k < 4; k++) {
                    int idx = d * 4 + k;
                    int b = idx < (int)v.width ? sv4_lsb_bit(v, idx) : 0;
                    if (b == 2) { has_x = 1; break; } // X wins over Z
                    if (b == 3) { has_z = 1; }
                    else val |= b << k;
                }
                if (has_x) {
                    llg_append(buf, cap, &len, 'x');
                } else if (has_z) {
                    llg_append(buf, cap, &len, 'z');
                } else {
                    llg_append(buf, cap, &len,
                                val < 10 ? (char)('0' + val) : (char)('a' + val - 10));
                }
            }
            break;
        }
        case 'o': {
            int digits = ((int)v.width + 2) / 3;
            for (int d = digits - 1; d >= 0; d--) {
                int has_x = 0, has_z = 0, val = 0;
                for (int k = 0; k < 3; k++) {
                    int idx = d * 3 + k;
                    int b = idx < (int)v.width ? sv4_lsb_bit(v, idx) : 0;
                    if (b == 2) { has_x = 1; break; } // X wins over Z
                    if (b == 3) { has_z = 1; }
                    else val |= b << k;
                }
                if (has_x) {
                    llg_append(buf, cap, &len, 'x');
                } else if (has_z) {
                    llg_append(buf, cap, &len, 'z');
                } else {
                    llg_append(buf, cap, &len, (char)('0' + val));
                }
            }
            break;
        }
        default:
            llg_append(buf, cap, &len, '?');
            break;
    }
    buf[len] = 0;
}
