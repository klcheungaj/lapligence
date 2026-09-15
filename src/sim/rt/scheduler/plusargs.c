
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
    if (!value) return;
    memset(value->bits, 0, sizeof(value->bits));
    memset(value->x, 0xff, sizeof(value->x));
    memset(value->z, 0, sizeof(value->z));
    llg_plusarg_mask_top(value);
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
        negative = text[start] == '-';
        ++start;
    }
    uint64_t limbs[LLG_LIMBS] = {0};
    int digits = 0;
    int unknown = 0;
    for (size_t i = start; i < length; ++i) {
        char c = text[i];
        if (c == '_') continue;
        if (llg_plusarg_unknown_digit(c)) {
            unknown = 1;
            digits = 1;
            continue;
        }
        int digit = llg_plusarg_digit((unsigned char)c, 10);
        if (digit < 0) return 0;
        digits = 1;
        uint64_t carry = (uint64_t)digit;
        for (int limb = 0; limb < (int)LLG_LIMBS; ++limb) {
            uint64_t low = (limbs[limb] & UINT32_MAX) * 10ULL + carry;
            uint64_t high = (limbs[limb] >> 32) * 10ULL + (low >> 32);
            limbs[limb] = (high << 32) | (low & UINT32_MAX);
            carry = high >> 32;
        }
    }
    if (!digits) return 0;
    if (unknown) {
        llg_plusarg_unknown(output);
        return 1;
    }
    memcpy(output->bits, limbs, sizeof(limbs));
    memset(output->x, 0, sizeof(output->x));
    memset(output->z, 0, sizeof(output->z));
    if (negative) {
        uint64_t carry = 1;
        for (int limb = 0; limb < (int)LLG_LIMBS; ++limb) {
            uint64_t inverted = ~output->bits[limb];
            uint64_t sum = inverted + carry;
            carry = sum < inverted;
            output->bits[limb] = sum;
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
        for (int limb = 0; limb < (int)LLG_LIMBS; ++limb) {
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
    *output = sv4_from_u64(0, output->width, output->is_signed);
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
            *output = sv4_from_real(real, output->width, output->is_signed);
            return 1;
        }
        default: return 0;
    }
}

static void llg_plusarg_to_two_state(sv4_t* value) {
    if (!value) return;
    for (int limb = 0; limb < (int)LLG_LIMBS; ++limb) {
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
    if (!out || width == 0 || width > LLG_MAX_WIDTH) return 0;
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
        *out = parsed;
    }
    llg_plusarg_format_free(&format);
    return converted;
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
            sv4_t parsed = sv4_from_u64(
                0, LLG_MAX_WIDTH, value_len > 0 && value[0] == '-');
            converted = llg_plusarg_packed_value(
                value, value_len, format.conversion, &parsed);
            if (converted) *out = sv4_to_real(parsed);
            else *out = 0.0;
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
    if (converted) llg_string_move(out, llg_string_bytes(value, value_len));
    llg_plusarg_format_free(&format);
    return converted;
}
