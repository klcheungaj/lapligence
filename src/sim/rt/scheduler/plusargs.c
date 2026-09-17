
// ── Command-line plusargs ───────────────────────────────────────────────────

typedef struct {
    char conversion;
    char* prefix;
    size_t prefix_len;
    char* suffix;
    size_t suffix_len;
} llg_plusarg_format_t;

static int llg_plusarg_conversion(int c) {
    c = tolower((unsigned char)c);
    return c == 'd' || c == 'h' || c == 'x' || c == 'o' || c == 'b' ||
           c == 'f' || c == 'e' || c == 'g' || c == 's';
}

static char llg_plusarg_normalize_conversion(char c) {
    c = (char)tolower((unsigned char)c);
    return c == 'x' ? 'h' : c;
}

static void llg_plusarg_format_free(llg_plusarg_format_t* format) {
    if (!format) return;
    free(format->prefix);
    free(format->suffix);
    memset(format, 0, sizeof(*format));
}

static int llg_plusarg_format_parse(const char* text,
                                    llg_plusarg_format_t* format) {
    if (!text || !format) return 0;
    memset(format, 0, sizeof(*format));
    size_t length = strlen(text);
    format->prefix = llg_checked_malloc(length + 1, 1, "plusarg format prefix");
    format->suffix = llg_checked_malloc(length + 1, 1, "plusarg format suffix");
    int after_conversion = 0;
    for (size_t i = 0; i < length;) {
        char c = text[i++];
        char* output = after_conversion ? format->suffix : format->prefix;
        size_t* output_len = after_conversion ? &format->suffix_len : &format->prefix_len;
        if (c != '%') {
            output[(*output_len)++] = c;
            continue;
        }
        if (i >= length) {
            llg_plusarg_format_free(format);
            return 0;
        }
        c = text[i++];
        if (c == '%') {
            output[(*output_len)++] = '%';
            continue;
        }
        if (c == '0') {
            if (i >= length) {
                llg_plusarg_format_free(format);
                return 0;
            }
            c = text[i++];
        }
        if (after_conversion || format->conversion || !llg_plusarg_conversion(c)) {
            llg_plusarg_format_free(format);
            return 0;
        }
        format->conversion = llg_plusarg_normalize_conversion(c);
        after_conversion = 1;
    }
    if (!format->conversion) {
        llg_plusarg_format_free(format);
        return 0;
    }
    format->prefix[format->prefix_len] = '\0';
    format->suffix[format->suffix_len] = '\0';
    return 1;
}

static int llg_plusarg_find(const llg_plusarg_format_t* format,
                            const char** value, size_t* value_len) {
    if (!format || !value || !value_len || !g.argv) return 0;
    for (int i = 1; i < g.argc; ++i) {
        const char* argument = g.argv[i];
        if (!argument || argument[0] != '+') continue;
        const char* body = argument + 1;
        size_t body_len = strlen(body);
        if (body_len < format->prefix_len ||
            memcmp(body, format->prefix, format->prefix_len) != 0) {
            continue;
        }
        const char* candidate = body + format->prefix_len;
        size_t candidate_len = body_len - format->prefix_len;
        if (candidate_len < format->suffix_len ||
            memcmp(candidate + candidate_len - format->suffix_len,
                   format->suffix, format->suffix_len) != 0) {
            continue;
        }
        *value = candidate;
        *value_len = candidate_len - format->suffix_len;
        return 1;
    }
    return 0;
}

static void llg_plusarg_set_bit(sv4_t* value, uint32_t index, int state) {
    if (!value || index >= value->width) return;
    int limb = (int)(index >> 6);
    uint64_t mask = 1ULL << (index & 63u);
    value->bits[limb] &= ~mask;
    value->x[limb] &= ~mask;
    value->z[limb] &= ~mask;
    if (state == 1) value->bits[limb] |= mask;
    else if (state == 2) value->x[limb] |= mask;
    else if (state == 3) value->z[limb] |= mask;
}

static void llg_plusarg_mask_top(sv4_t* value) {
    if (!value || value->width == 0 || value->width % 64u == 0) return;
    uint64_t mask = (1ULL << (value->width % 64u)) - 1ULL;
    int limb = (int)(value->width / 64u);
    value->bits[limb] &= mask;
    value->x[limb] &= mask;
    value->z[limb] &= mask;
}

static void llg_plusarg_unknown(sv4_t* value) {
    if (value) sv4_replace(value, sv4_x(value->width, value->is_signed));
}

static int llg_plusarg_digit(int c, int base) {
    int digit = -1;
    if (c >= '0' && c <= '9') digit = c - '0';
    else if (c >= 'a' && c <= 'f') digit = c - 'a' + 10;
    else if (c >= 'A' && c <= 'F') digit = c - 'A' + 10;
    return digit >= 0 && digit < base ? digit : -1;
}

static int llg_plusarg_unknown_digit(int c) {
    return c == 'x' || c == 'X' || c == 'z' || c == 'Z';
}

static int llg_plusarg_decimal(const char* text, size_t length, sv4_t* output) {
    size_t start = 0;
    int negative = 0;
    if (start < length && (text[start] == '+' || text[start] == '-')) {
        negative = text[start++] == '-';
    }
    int digits = 0, unknown = 0;
    for (size_t i = start; i < length; ++i) {
        if (text[i] == '_') continue;
        if (llg_plusarg_unknown_digit(text[i])) unknown = 1;
        else if (llg_plusarg_digit((unsigned char)text[i], 10) < 0) return 0;
        digits = 1;
    }
    if (!digits) return 0;
    if (unknown) { llg_plusarg_unknown(output); return 1; }
    // The enclosing parser supplies a zeroed, exact-destination-width owner.
    uint32_t n = (output->width + 63u) / 64u;
    for (size_t i = start; i < length; ++i) {
        if (text[i] == '_') continue;
        uint64_t carry = (uint64_t)(text[i] - '0');
        for (uint32_t limb = 0; limb < n; ++limb) {
            uint64_t low = (output->bits[limb] & UINT32_MAX) * 10u + carry;
            uint64_t high = (output->bits[limb] >> 32) * 10u + (low >> 32);
            output->bits[limb] = (high << 32) | (low & UINT32_MAX);
            carry = high >> 32;
        }
    }
    if (negative) {
        uint64_t carry = 1;
        for (uint32_t limb = 0; limb < n; ++limb) {
            uint64_t inverted = ~output->bits[limb];
            output->bits[limb] = inverted + carry;
            carry = output->bits[limb] < inverted;
        }
    }
    llg_plusarg_mask_top(output);
    return 1;
}

static int llg_plusarg_based(const char* text, size_t length, int base,
                             int bits_per_digit, sv4_t* output) {
    size_t start = 0;
    int negative = 0;
    if (start < length && (text[start] == '+' || text[start] == '-')) {
        negative = text[start] == '-';
        ++start;
    }
    if (length - start >= 2 && text[start] == '0' &&
        ((base == 16 && (text[start + 1] == 'x' || text[start + 1] == 'X')) ||
         (base == 8 && (text[start + 1] == 'o' || text[start + 1] == 'O')) ||
         (base == 2 && (text[start + 1] == 'b' || text[start + 1] == 'B')))) {
        start += 2;
    }
    uint32_t bit = 0;
    int digits = 0;
    int unknown = 0;
    for (size_t i = length; i > start;) {
        char c = text[--i];
        if (c == '_') continue;
        int digit = llg_plusarg_digit((unsigned char)c, base);
        int state = 0;
        if (digit >= 0) state = 0;
        else if (llg_plusarg_unknown_digit((unsigned char)c)) {
            state = c == 'z' || c == 'Z' ? 3 : 2;
            unknown = 1;
        } else {
            return 0;
        }
        ++digits;
        for (int j = 0; j < bits_per_digit; ++j) {
            if (state != 0) {
                llg_plusarg_set_bit(output, bit + (uint32_t)j, state);
            } else {
                llg_plusarg_set_bit(output, bit + (uint32_t)j,
                                    (digit >> j) & 1);
            }
        }
        if (bit <= UINT32_MAX - (uint32_t)bits_per_digit) bit += (uint32_t)bits_per_digit;
    }
    if (!digits) return 0;
    if (negative && unknown) {
        llg_plusarg_unknown(output);
        return 1;
    }
    if (negative) {
        uint64_t carry = 1;
        for (int limb = 0; limb < (int)((output->width + 63u) / 64u); ++limb) {
            uint64_t inverted = ~output->bits[limb];
            uint64_t sum = inverted + carry;
            carry = sum < inverted;
            output->bits[limb] = sum;
        }
        llg_plusarg_mask_top(output);
    }
    return 1;
}

static int llg_plusarg_real_value(const char* text, size_t length,
                                  double* output) {
    if (length == 0) {
        *output = 0.0;
        return 1;
    }
    char* copy = llg_checked_malloc(length + 1, 1, "plusarg real value");
    memcpy(copy, text, length);
    copy[length] = '\0';
    char* end = NULL;
    double parsed = strtod(copy, &end);
    int converted = end != copy && *end == '\0' && isfinite(parsed);
    if (converted) *output = parsed;
    free(copy);
    return converted;
}

static int llg_plusarg_packed_value(const char* text, size_t length,
                                     char conversion, sv4_t* output) {
    sv4_replace(output, sv4_zero(output->width, output->is_signed));
    if (length == 0) return 1;
    switch (conversion) {
        case 'd': return llg_plusarg_decimal(text, length, output);
        case 'h': return llg_plusarg_based(text, length, 16, 4, output);
        case 'o': return llg_plusarg_based(text, length, 8, 3, output);
        case 'b': return llg_plusarg_based(text, length, 2, 1, output);
        case 's': {
            // A packed string destination receives the rightmost bytes of
            // the argument, with zero extension on the left, just like an
            // integral assignment from a SystemVerilog string value.
            uint32_t bit = 0;
            for (size_t i = length; i > 0 && bit < output->width; --i) {
                unsigned char byte = (unsigned char)text[i - 1];
                for (int j = 0; j < 8 && bit + (uint32_t)j < output->width; ++j) {
                    llg_plusarg_set_bit(output, bit + (uint32_t)j,
                                        (byte >> j) & 1);
                }
                if (bit <= UINT32_MAX - 8u) bit += 8u;
            }
            llg_plusarg_mask_top(output);
            return 1;
        }
        case 'f':
        case 'e':
        case 'g': {
            double real = 0.0;
            if (!llg_plusarg_real_value(text, length, &real)) return 0;
            sv4_replace(output, sv4_from_real(real, output->width, output->is_signed));
            return 1;
        }
        default: return 0;
    }
}

static void llg_plusarg_to_two_state(sv4_t* value) {
    if (!value) return;
    for (int limb = 0; limb < (int)((value->width + 63u) / 64u); ++limb) {
        value->bits[limb] &= ~(value->x[limb] | value->z[limb]);
        value->x[limb] = 0;
        value->z[limb] = 0;
    }
}

int llg_test_plusargs(const char* pattern) {
    if (!pattern || !g.argv) return 0;
    size_t length = strlen(pattern);
    for (int i = 1; i < g.argc; ++i) {
        const char* argument = g.argv[i];
        if (!argument || argument[0] != '+') continue;
        if (strncmp(argument + 1, pattern, length) == 0) return 1;
    }
    return 0;
}

int llg_value_plusargs_packed(const char* format_text, sv4_t* out,
                              uint32_t width, int is_signed, int two_state) {
    if (!out || width == 0 || width >= LLG_SUPPORTED_WIDTH_LIMIT) return 0;
    llg_plusarg_format_t format;
    if (!llg_plusarg_format_parse(format_text, &format)) return 0;
    const char* value = NULL;
    size_t value_len = 0;
    int matched = llg_plusarg_find(&format, &value, &value_len);
    sv4_t parsed = sv4_from_u64(0, width, (int8_t)is_signed);
    int converted = 0;
    if (matched) {
        converted = llg_plusarg_packed_value(
            value, value_len, format.conversion, &parsed);
        if (!converted) {
            // A matching plusarg with an illegal value is still a successful
            // query; the LRM specifies an all-X packed result for this case.
            llg_plusarg_unknown(&parsed);
            converted = 1;
        }
        if (two_state) llg_plusarg_to_two_state(&parsed);
        sv4_move(out, &parsed);
    }
    sv4_destroy(&parsed);
    llg_plusarg_format_free(&format);
    return converted;
}

// Parse only the magnitude required by this argument. Leading zeroes consume no
// limb storage, and base-2 input does not reserve four bits for every digit.
// Accumulate integers exactly before applying the packed-to-real rounding order.
static int llg_plusarg_integral_real(const char* text, size_t length,
                                     char conversion, double* output) {
    unsigned base = conversion == 'b' ? 2u : conversion == 'o' ? 8u :
                    conversion == 'h' ? 16u : 10u;
    size_t start = 0;
    int negative = length && text[0] == '-';
    if (length && (text[0] == '-' || text[0] == '+')) ++start;
    if (base != 10u && length - start >= 2 && text[start] == '0' &&
        ((base == 2u && (text[start + 1] == 'b' || text[start + 1] == 'B')) ||
         (base == 8u && (text[start + 1] == 'o' || text[start + 1] == 'O')) ||
         (base == 16u && (text[start + 1] == 'x' || text[start + 1] == 'X'))))
        start += 2;
    int digits = 0, unknown = 0;
    for (size_t i = start; i < length; ++i) {
        if (text[i] == '_') continue;
        if (llg_plusarg_unknown_digit((unsigned char)text[i])) unknown = 1;
        else if (llg_plusarg_digit((unsigned char)text[i], (int)base) < 0) return 0;
        digits = 1;
    }
    if (!digits) return 0;
    if (unknown && (negative || base == 10u)) { *output = 0.0; return 1; }
    uint64_t* limbs = NULL;
    size_t used = 0, capacity = 0;
    const size_t maximum = (LLG_SUPPORTED_WIDTH_LIMIT - 1u + 63u) / 64u;
    for (size_t i = start; i < length; ++i) {
        if (text[i] == '_') continue;
        int digit = llg_plusarg_digit((unsigned char)text[i], (int)base);
        uint64_t carry = digit < 0 ? 0u : (unsigned)digit;
        for (size_t limb = 0; limb < used; ++limb) {
            uint64_t low = (limbs[limb] & UINT32_MAX) * base + carry;
            uint64_t high = (limbs[limb] >> 32) * base + (low >> 32);
            limbs[limb] = (high << 32) | (low & UINT32_MAX);
            carry = high >> 32;
        }
        if (carry) {
            if (used == maximum) {
                free(limbs);
                llg_fatal_allocation("integral plusarg width", used + 1u, 64u);
            }
            if (used == capacity) {
                size_t next = capacity ? capacity * 2u : 1u;
                if (next > maximum) next = maximum;
                uint64_t* replacement = llg_checked_malloc(next, sizeof(*limbs), "integral plusarg limbs");
                if (used) memcpy(replacement, limbs, used * sizeof(*limbs));
                free(limbs);
                limbs = replacement;
                capacity = next;
            }
            limbs[used++] = carry;
        }
        if (used == maximum && (limbs[used - 1u] >> 63u)) {
            free(limbs);
            llg_fatal_allocation("integral plusarg width", LLG_SUPPORTED_WIDTH_LIMIT, 1u);
        }
    }
    double result = 0.0;
    for (size_t i = used; i > 0; --i) result = ldexp(result, 64) + (double)limbs[i - 1u];
    free(limbs);
    *output = negative && result != 0.0 ? -result : result;
    return 1;
}

int llg_value_plusargs_real(const char* format_text, double* out) {
    if (!out) return 0;
    llg_plusarg_format_t format = {0};
    if (!llg_plusarg_format_parse(format_text, &format) ||
        format.conversion == 's') {
        llg_plusarg_format_free(&format);
        return 0;
    }
    const char* value = NULL;
    size_t value_len = 0;
    int converted = 0;
    if (llg_plusarg_find(&format, &value, &value_len)) {
        if (format.conversion == 'f' || format.conversion == 'e' ||
            format.conversion == 'g') {
            converted = llg_plusarg_real_value(value, value_len, out);
            if (!converted) {
                // A matching malformed real conversion has no four-state
                // representation; keep the successful query and deterministic
                // zero result used by the real assignment path.
                *out = 0.0;
                converted = 1;
            }
        } else {
            converted = llg_plusarg_integral_real(
                value, value_len, format.conversion, out);
            if (!converted) *out = 0.0;
            // A matching malformed integral conversion is represented as X
            // before assignment to real storage, which yields zero here.
            converted = 1;
        }
    }
    llg_plusarg_format_free(&format);
    return converted;
}

int llg_value_plusargs_string(const char* format_text, llg_string_t* out) {
    if (!out) return 0;
    llg_plusarg_format_t format = {0};
    if (!llg_plusarg_format_parse(format_text, &format) ||
        format.conversion != 's') {
        llg_plusarg_format_free(&format);
        return 0;
    }
    const char* value = NULL;
    size_t value_len = 0;
    int converted = llg_plusarg_find(&format, &value, &value_len);
    llg_string_t result = {0};
    if (converted) result = llg_string_bytes(value, value_len);
    llg_plusarg_format_free(&format);
    /* No parser-owned buffers may remain across a notifying string write. */
    if (converted) llg_string_move(out, result);
    return converted;
}
