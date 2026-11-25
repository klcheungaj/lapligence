
// ── Command-line plusargs ───────────────────────────────────────────────────

typedef struct {
    char conversion;
    char* prefix;
    size_t prefix_len;
    char* suffix;
    size_t suffix_len;
} llg_plusarg_format_t;

static int32_t llg_plusarg_conversion(int32_t c) {
    c = tolower((uint8_t)c);
    return c == 'd' || c == 'h' || c == 'x' || c == 'o' || c == 'b' ||
           c == 'f' || c == 'e' || c == 'g' || c == 's';
}

static char llg_plusarg_normalize_conversion(char c) {
    c = (char)tolower((uint8_t)c);
    return c == 'x' ? 'h' : c;
}

static void llg_plusarg_format_free(llg_plusarg_format_t* format) {
    if (!format) return;
    free(format->prefix);
    free(format->suffix);
    memset(format, 0, sizeof(*format));
}

static int32_t llg_plusarg_format_parse(const char* text, size_t length,
                                    llg_plusarg_format_t* format) {
    if (!text || !format) return 0;
    memset(format, 0, sizeof(*format));
    format->prefix = llg_checked_malloc(length + 1, 1, "plusarg format prefix");
    format->suffix = llg_checked_malloc(length + 1, 1, "plusarg format suffix");
    int32_t after_conversion = 0;
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

static int32_t llg_plusarg_find(const llg_plusarg_format_t* format,
                            const char** value, size_t* value_len) {
    if (!format || !value || !value_len || !g.argv) return 0;
    // An empty plusarg_string names no plusarg (decision S26-D24).
    if (!format->prefix_len) return 0;
    for (int32_t i = 1; i < g.argc; ++i) {
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

static void llg_plusarg_set_bit(sv4_t* value, uint32_t index, int32_t state) {
    if (value) llg_sv4_set_state(value, index, (uint32_t)state);
}

static void llg_plusarg_mask_top(sv4_t* value) {
    if (value) llg_sv4_mask_top(value);
}

static void llg_plusarg_unknown(sv4_t* value) {
    if (value) sv4_replace(value, sv4_x(llg_sv4_width(*value), llg_sv4_signed(*value)));
}

static int32_t llg_plusarg_digit(int32_t c, int32_t base) {
    int32_t digit = -1;
    if (c >= '0' && c <= '9') digit = c - '0';
    else if (c >= 'a' && c <= 'f') digit = c - 'a' + 10;
    else if (c >= 'A' && c <= 'F') digit = c - 'A' + 10;
    return digit >= 0 && digit < base ? digit : -1;
}

// The four-state value of an x or z digit (5.7.1), else 0.
static int32_t llg_plusarg_unknown_digit(int32_t c) {
    return c == 'x' || c == 'X' ? 2 : c == 'z' || c == 'Z' || c == '?' ? 3 : 0;
}

static int32_t llg_plusarg_decimal(const char* text, size_t length, sv4_t* output) {
    size_t start = 0;
    int32_t negative = 0;
    if (start < length && (text[start] == '+' || text[start] == '-')) {
        negative = text[start++] == '-';
    }
    int32_t unknown = 0;
    size_t digits = 0;
    for (size_t i = start; i < length; ++i) {
        if (text[i] == '_') continue;
        ++digits;
        if (llg_plusarg_digit((uint8_t)text[i], 10) >= 0) continue;
        unknown = llg_plusarg_unknown_digit((uint8_t)text[i]);
        if (!unknown) return 0;
    }
    if (!digits) return 0;
    if (unknown) {
        // An x or z digit stands alone in a decimal number (5.7.1).
        if (digits != 1 || start) return 0;
        sv4_replace(output, sv4_fill((uint8_t)unknown, llg_sv4_width(*output),
                                     llg_sv4_signed(*output)));
        return 1;
    }
    // The enclosing parser supplies a zeroed, exact-destination-width owner.
    for (size_t i = start; i < length; ++i) {
        if (text[i] == '_') continue;
        llg_sv4_mul_add_known(output, 10, (uint32_t)(text[i] - '0'));
    }
    if (negative) llg_sv4_negate_known(output);
    llg_plusarg_mask_top(output);
    return 1;
}

static int32_t llg_plusarg_based(const char* text, size_t length, int32_t base,
                             int32_t bits_per_digit, sv4_t* output) {
    size_t start = 0;
    int32_t negative = 0;
    if (start < length && (text[start] == '+' || text[start] == '-')) {
        negative = text[start] == '-';
        ++start;
    }
    uint32_t bit = 0;
    int32_t digits = 0;
    int32_t unknown = 0;
    for (size_t i = length; i > start;) {
        char c = text[--i];
        if (c == '_') continue;
        int32_t digit = llg_plusarg_digit((uint8_t)c, base);
        int32_t state = digit >= 0 ? 0 : llg_plusarg_unknown_digit((uint8_t)c);
        if (digit < 0 && !state) return 0;
        if (state) unknown = 1;
        ++digits;
        // The enclosing parser supplies a zeroed, exact-destination-width owner.
        for (int32_t j = 0; j < bits_per_digit; ++j) {
            int32_t digit_bit = state ? state : (digit >> j) & 1;
            if (digit_bit) llg_plusarg_set_bit(output, bit + (uint32_t)j, digit_bit);
        }
        if (bit <= UINT32_MAX - (uint32_t)bits_per_digit) bit += (uint32_t)bits_per_digit;
    }
    if (!digits) return 0;
    if (negative && unknown) {
        llg_plusarg_unknown(output);
        return 1;
    }
    if (negative) {
        llg_sv4_negate_known(output);
        llg_plusarg_mask_top(output);
    }
    return 1;
}

static int32_t llg_plusarg_real_value(const char* text, size_t length,
                                  double* output) {
    if (length == 0) {
        *output = 0.0;
        return 1;
    }
    // strtod also accepts hexadecimal numbers, infinities and NaNs.
    for (size_t i = 0; i < length; ++i) {
        char c = text[i];
        if (!((c >= '0' && c <= '9') || c == '.' || c == 'e' || c == 'E' || c == '+' || c == '-'))
            return 0;
    }
    char* copy = llg_checked_malloc(length + 1, 1, "plusarg real value");
    memcpy(copy, text, length);
    copy[length] = '\0';
    char* end = NULL;
    double parsed = strtod(copy, &end);
    int32_t converted = end != copy && *end == '\0' && isfinite(parsed);
    if (converted) *output = parsed;
    free(copy);
    return converted;
}

static int32_t llg_plusarg_packed_value(const char* text, size_t length,
                                     char conversion, sv4_t* output) {
    sv4_replace(output, sv4_zero(llg_sv4_width(*output), llg_sv4_signed(*output)));
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
            for (size_t i = length; i > 0 && bit < llg_sv4_width(*output); --i) {
                uint8_t byte = (uint8_t)text[i - 1];
                for (int32_t j = 0; j < 8 && bit + (uint32_t)j < llg_sv4_width(*output); ++j) {
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
            sv4_replace(output, sv4_from_real(real, llg_sv4_width(*output), llg_sv4_signed(*output)));
            return 1;
        }
        default: return 0;
    }
}

static void llg_plusarg_to_two_state(sv4_t* value) {
    if (!value) return;
    llg_sv4_two_state_inplace(value);
}

int32_t llg_test_plusargs(const char* pattern, size_t length) {
    // An empty string names no plusarg (decision S26-D24).
    if (!pattern || !length || !g.argv) return 0;
    for (int32_t i = 1; i < g.argc; ++i) {
        const char* argument = g.argv[i];
        if (!argument || argument[0] != '+') continue;
        if (strlen(argument + 1) >= length && memcmp(argument + 1, pattern, length) == 0)
            return 1;
    }
    return 0;
}

int32_t llg_value_plusargs_packed(const char* format_text, size_t format_len, sv4_t* out,
                              uint32_t width, int32_t is_signed, int32_t two_state) {
    if (!out || width == 0 || width >= LLG_SUPPORTED_WIDTH_LIMIT) return 0;
    llg_plusarg_format_t format;
    if (!llg_plusarg_format_parse(format_text, format_len, &format)) return 0;
    const char* value = NULL;
    size_t value_len = 0;
    int32_t matched = llg_plusarg_find(&format, &value, &value_len);
    sv4_t parsed = sv4_from_u64(0, width, (int8_t)is_signed);
    int32_t converted = 0;
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
static int32_t llg_plusarg_integral_real(const char* text, size_t length,
                                     char conversion, double* output) {
    uint32_t base = conversion == 'b' ? 2u : conversion == 'o' ? 8u :
                    conversion == 'h' ? 16u : 10u;
    size_t start = 0;
    int32_t negative = length && text[0] == '-';
    if (length && (text[0] == '-' || text[0] == '+')) ++start;
    int32_t digits = 0, unknown = 0;
    for (size_t i = start; i < length; ++i) {
        if (text[i] == '_') continue;
        if (llg_plusarg_unknown_digit((uint8_t)text[i])) unknown = 1;
        else if (llg_plusarg_digit((uint8_t)text[i], (int32_t)base) < 0) return 0;
        digits = 1;
    }
    if (!digits) return 0;
    if (unknown && (negative || base == 10u)) { *output = 0.0; return 1; }
    uint64_t* limbs = NULL;
    size_t used = 0, capacity = 0;
    const size_t maximum = (LLG_SUPPORTED_WIDTH_LIMIT - 1u + 63u) / 64u;
    for (size_t i = start; i < length; ++i) {
        if (text[i] == '_') continue;
        int32_t digit = llg_plusarg_digit((uint8_t)text[i], (int32_t)base);
        uint64_t carry = digit < 0 ? 0u : (uint32_t)digit;
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

int32_t llg_value_plusargs_real(const char* format_text, size_t format_len, double* out) {
    if (!out) return 0;
    llg_plusarg_format_t format = {0};
    if (!llg_plusarg_format_parse(format_text, format_len, &format)) return 0;
    const char* value = NULL;
    size_t value_len = 0;
    int32_t converted = 0;
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
        } else if (format.conversion == 's') {
            // The characters as an integral value (6.16).
            sv4_t bytes = sv4_zero(value_len ? (uint32_t)value_len * 8u : 1u, 0);
            converted = llg_plusarg_packed_value(value, value_len, 's', &bytes);
            *out = sv4_to_real(bytes);
            sv4_destroy(&bytes);
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

int32_t llg_value_plusargs_string(const char* format_text, size_t format_len, llg_string_t* out) {
    if (!out) return 0;
    llg_plusarg_format_t format = {0};
    if (!llg_plusarg_format_parse(format_text, format_len, &format)) return 0;
    const char* value = NULL;
    size_t value_len = 0;
    int32_t found = llg_plusarg_find(&format, &value, &value_len);
    // Another conversion has no value for a string (decision S26-D26).
    int32_t converted = found && format.conversion == 's';
    llg_string_t result = {0};
    if (converted) result = llg_string_bytes(value, value_len);
    llg_plusarg_format_free(&format);
    /* No parser-owned buffers may remain across a notifying string write. */
    if (converted) llg_string_move(out, result);
    return found;
}

// Command line of the run, borrowed for vpi_get_vlog_info (38.30). The
// strings belong to the model's main() and outlive the run.
int32_t llg_rt_command_line(char*** argv) {
    if (argv) *argv = g.argv;
    return g.argv ? g.argc : 0;
}
