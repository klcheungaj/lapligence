typedef struct {
    llg_file_slot_t* file;
    const unsigned char* bytes;
    size_t length;
    size_t position;
    int input_failure;
    // Owning scope of the call: `%m` text and the unit `%t` converts into.
    const char* scope;
    uint64_t time_unit_fs;
} llg_scan_input_t;

static int llg_scan_get(llg_scan_input_t* input) {
    int value;
    if (input->file) {
        value = llg_file_getc_slot(input->file);
    } else if (input->position >= input->length) {
        value = EOF;
    } else {
        value = input->bytes[input->position++];
    }
    return value;
}

static int llg_scan_unget(llg_scan_input_t* input, int value) {
    if (value == EOF) return 0;
    if (input->file) return llg_file_ungetc_slot(input->file, value) != EOF;
    if (input->position == 0) return 0;
    input->position--;
    return 1;
}

// IEEE 1800-2009 21.3.4.3 a): for $sscanf, null characters are also white
// space (a packed or byte-array source can carry them).
static int llg_scan_is_space(const llg_scan_input_t* input, int value) {
    return value != EOF && (isspace((unsigned char)value) || (!input->file && value == 0));
}

static int llg_scan_skip_space(llg_scan_input_t* input) {
    int value;
    do {
        value = llg_scan_get(input);
    } while (llg_scan_is_space(input, value));
    if (value != EOF) (void)llg_scan_unget(input, value);
    else input->input_failure = 1;
    return value != EOF;
}

static int llg_scan_token(llg_scan_input_t* input, size_t limit,
                          unsigned char** result, size_t* length) {
    size_t capacity = limit != SIZE_MAX && limit < 128u ? limit + 1u : 128u;
    if (capacity == 0) capacity = 1;
    unsigned char* bytes = (unsigned char*)llg_checked_malloc(capacity, 1, "file input token");
    size_t used = 0;
    for (;;) {
        int value = llg_scan_get(input);
        if (value == EOF || llg_scan_is_space(input, value)) {
            if (value != EOF) (void)llg_scan_unget(input, value);
            else if (used == 0) input->input_failure = 1;
            break;
        }
        if (limit != SIZE_MAX && used >= limit) {
            (void)llg_scan_unget(input, value);
            break;
        }
        if (used == capacity) {
            if (capacity > SIZE_MAX / 2u) {
                free(bytes);
                llg_fatal_allocation("file input token", capacity, 2u);
            }
            capacity *= 2u;
            unsigned char* replacement = (unsigned char*)realloc(bytes, capacity);
            if (!replacement) {
                free(bytes);
                llg_fatal_allocation("file input token", capacity, 1u);
            }
            bytes = replacement;
        }
        bytes[used++] = (unsigned char)value;
    }
    if (used == 0) {
        free(bytes);
        return 0;
    }
    *result = bytes;
    *length = used;
    return 1;
}

static int llg_scan_chars(llg_scan_input_t* input, size_t count,
                          unsigned char** result, size_t* length) {
    unsigned char* bytes = (unsigned char*)llg_checked_malloc(count ? count : 1u, 1,
                                                               "file input characters");
    size_t used = 0;
    while (used < count) {
        int value = llg_scan_get(input);
        if (value == EOF) {
            if (used == 0) input->input_failure = 1;
            break;
        }
        bytes[used++] = (unsigned char)value;
    }
    if (used == 0) {
        free(bytes);
        return 0;
    }
    *result = bytes;
    *length = used;
    return 1;
}

static int llg_scan_digit(unsigned char value, unsigned base) {
    if (value >= '0' && value <= '9') {
        int digit = (int)(value - '0');
        return digit < (int)base ? digit : -1;
    }
    if (value >= 'a' && value <= 'f') {
        int digit = (int)(value - 'a') + 10;
        return digit < (int)base ? digit : -1;
    }
    if (value >= 'A' && value <= 'F') {
        int digit = (int)(value - 'A') + 10;
        return digit < (int)base ? digit : -1;
    }
    return -1;
}

// Read only this conversion's input item. In particular, a comma or colon
// belongs to the next directive, not to an all-or-nothing whitespace token.
static int llg_scan_numeric(llg_scan_input_t* input, char conversion, size_t limit,
                            unsigned char** result, size_t* length) {
    size_t capacity = 64u, used = 0;
    unsigned char* bytes = (unsigned char*)llg_checked_malloc(capacity, 1, "numeric input");
    int real = conversion == 'f' || conversion == 'e' || conversion == 'g' ||
               conversion == 't';
    unsigned base = conversion == 'b' ? 2u : conversion == 'o' ? 8u :
                    conversion == 'h' || conversion == 'x' ? 16u : 10u;
    int digits = 0, dot = 0, exponent = 0, exponent_digits = 0;
    int decimal_unknown = 0;
    while (used < limit) {
        int c = llg_scan_get(input);
        if (c == EOF) {
            if (!used) input->input_failure = 1;
            break;
        }
        int accept = 0;
        if (used == 0 && (c == '+' || c == '-') &&
            (real || base == 10u)) accept = 1;
        else if (real) {
            if (c >= '0' && c <= '9') {
                accept = 1;
                if (exponent) exponent_digits = 1; else digits = 1;
            } else if (c == '.' && !dot && !exponent) {
                accept = 1; dot = 1;
            } else if ((c == 'e' || c == 'E') && digits && !exponent) {
                accept = 1; exponent = 1;
            } else if ((c == '+' || c == '-') && used &&
                       (bytes[used - 1u] == 'e' || bytes[used - 1u] == 'E')) accept = 1;
        } else {
            size_t start = used && (bytes[0] == '+' || bytes[0] == '-') ? 1u : 0u;
            // Preserve the existing C-style auto-radix %i extension only.
            if (conversion == 'i' && used == start + 1u && bytes[start] == '0' &&
                (c == 'x' || c == 'X' || c == 'b' || c == 'B' || c == 'o' || c == 'O')) {
                base = c == 'x' || c == 'X' ? 16u : c == 'b' || c == 'B' ? 2u : 8u;
                digits = 0; accept = 1;
            } else {
                if (conversion == 'i' && used == start + 1u && bytes[start] == '0') base = 8u;
                if (c == '_' && digits) accept = 1;
                else if (!decimal_unknown && llg_scan_digit((unsigned char)c, base) >= 0) {
                    accept = 1; digits = 1;
                } else if (c == 'x' || c == 'X' || c == 'z' || c == 'Z' || c == '?') {
                    if (base != 10u || (!digits && used == start)) {
                        accept = 1; digits = 1;
                        if (base == 10u) decimal_unknown = 1;
                    }
                }
            }
        }
        if (!accept) { (void)llg_scan_unget(input, c); break; }
        if (used == capacity) {
            if (capacity > SIZE_MAX / 2u) llg_fatal_allocation("numeric input", capacity, 2u);
            capacity *= 2u;
            unsigned char* next = (unsigned char*)realloc(bytes, capacity);
            if (!next) llg_fatal_allocation("numeric input", capacity, 1u);
            bytes = next;
        }
        bytes[used++] = (unsigned char)c;
    }
    if (!digits || (real && exponent && !exponent_digits)) { free(bytes); return 0; }
    *result = bytes;
    *length = used;
    return 1;
}

static void llg_scan_set_bit(sv4_t* value, uint32_t bit, int state) {
    llg_sv4_set_state(value, bit, (unsigned)state);
}

static int llg_scan_unknown(unsigned char value) {
    return value == 'x' || value == 'X' ? 2 :
           value == 'z' || value == 'Z' || value == '?' ? 3 : 0;
}

static int llg_scan_integer(const unsigned char* bytes, size_t length,
                            char conversion, uint32_t width, int is_signed,
                            sv4_t* result) {
    size_t begin = 0;
    int negative = 0;
    unsigned base = conversion == 'b' ? 2u : conversion == 'o' ? 8u :
                    conversion == 'h' || conversion == 'x' ? 16u : 10u;
    if (length && (bytes[0] == '+' || bytes[0] == '-')) {
        negative = bytes[0] == '-';
        begin = 1;
    }
    if (conversion == 'i' && begin + 2u <= length && bytes[begin] == '0') {
        unsigned char prefix = bytes[begin + 1u];
        if (prefix == 'x' || prefix == 'X') { base = 16u; begin += 2u; }
        else if (prefix == 'b' || prefix == 'B') { base = 2u; begin += 2u; }
        else if (prefix == 'o' || prefix == 'O') { base = 8u; begin += 2u; }
        else base = 8u;
    }
    if (begin == length) return 0;
    int unknown = 0;
    for (size_t i = begin; i < length; i++) {
        if (bytes[i] == '_') continue;
        int state = llg_scan_unknown(bytes[i]);
        if (state) {
            unknown = unknown && unknown != state ? 2 : state;
            continue;
        }
        if (llg_scan_digit(bytes[i], base) < 0) return 0;
    }
    if (unknown && base == 10u) {
        sv4_replace(result, sv4_fill((uint8_t)(unknown == 3 ? 3 : 2), width, (int8_t)is_signed));
        return 1;
    }
    sv4_replace(result, sv4_zero(width, (int8_t)is_signed));
    if (base == 10u) {
        for (size_t i = begin; i < length; i++) {
            if (bytes[i] == '_') continue;
            unsigned digit = (unsigned)llg_scan_digit(bytes[i], base);
            llg_sv4_mul_add_known(result, 10, digit);
        }
    } else {
        uint32_t bits_per_digit = base == 16u ? 4u : base == 8u ? 3u : 1u;
        uint32_t bit = 0;
        for (size_t i = length; i > begin; i--) {
            unsigned char digit_char = bytes[i - 1u];
            if (digit_char == '_') continue;
            int state = llg_scan_unknown(digit_char);
            if (state) {
                for (uint32_t part = 0; part < bits_per_digit; part++)
                    llg_scan_set_bit(result, bit + part, state);
            } else {
                unsigned digit = (unsigned)llg_scan_digit(digit_char, base);
                for (uint32_t part = 0; part < bits_per_digit; part++)
                    llg_scan_set_bit(result, bit + part, (digit >> part) & 1u);
            }
            if (bit <= UINT32_MAX - bits_per_digit) bit += bits_per_digit;
        }
    }
    llg_plusarg_mask_top(result);
    if (negative) sv4_replace(result, sv4_neg(*result));
    return 1;
}

static int llg_scan_bytes_to_packed(const unsigned char* bytes, size_t length,
                                    uint32_t width, int is_signed, sv4_t* result) {
    sv4_replace(result, sv4_zero(width, (int8_t)is_signed));
    size_t capacity = ((size_t)width + 7u) / 8u;
    size_t used = length < capacity ? length : capacity;
    for (size_t i = 0; i < used; i++) {
        unsigned char value = bytes[length - 1u - i];
        for (unsigned bit = 0; bit < 8u; bit++)
            llg_scan_set_bit(result, (uint32_t)(i * 8u + bit), (value >> bit) & 1u);
    }
    return 1;
}

// Integral conversions into a real destination take the token's integer
// value (unknown digits read as zero, as in any integral-to-real conversion).
static int llg_scan_integer_to_real(const unsigned char* bytes, size_t length,
                                    char conversion, double* result) {
    // Four bits per digit hold any base; past 2^20 bits a double has long
    // saturated, so longer tokens keep their low-order digits only.
    uint64_t width64 = (uint64_t)length * 4u + 2u;
    uint32_t width = width64 > (UINT64_C(1) << 20) ? UINT32_C(1) << 20 : (uint32_t)width64;
    sv4_t value = SV4_EMPTY;
    if (!llg_scan_integer(bytes, length, conversion, width, 1, &value)) {
        sv4_destroy(&value);
        return 0;
    }
    *result = sv4_to_real(value);
    sv4_destroy(&value);
    return 1;
}

static void llg_scan_store_real(const llg_file_input_target_t* target, double value) {
    llg_ba_d(target->real, target->shortreal ? (double)(float)value : value);
}

static int llg_scan_assign(const unsigned char* bytes, size_t length, char conversion,
                           const llg_file_input_target_t* target, uint32_t width,
                           int is_signed) {
    if (!target) return 0;
    if (conversion == 's' || conversion == 'c' || conversion == 'm') {
        if (target->kind == LLG_FILE_INPUT_STRING && target->string) {
            llg_string_move(target->string, llg_string_bytes((const char*)bytes, length));
            return 1;
        }
        if (target->kind == LLG_FILE_INPUT_PACKED && target->packed) {
            sv4_t value = SV4_EMPTY;
            llg_scan_bytes_to_packed(bytes, length, width, is_signed, &value);
            llg_ref_write_owned(target->packed, value);
            return 1;
        }
        return 0;
    }
    if (conversion == 'f' || conversion == 'e' || conversion == 'g') {
        char* text = (char*)llg_checked_malloc(length + 1u, 1, "file input real token");
        memcpy(text, bytes, length);
        text[length] = 0;
        char* end = NULL;
        double value = strtod(text, &end);
        int valid = end == text + length;
        free(text);
        if (!valid) return 0;
        if (target->kind == LLG_FILE_INPUT_REAL && target->real) {
            llg_scan_store_real(target, value);
            return 1;
        }
        if (target->kind == LLG_FILE_INPUT_PACKED && target->packed) {
            sv4_t packed = sv4_from_real(value, width, (int8_t)is_signed);
            llg_ref_write_owned(target->packed, packed);
            return 1;
        }
        return 0;
    }
    if (target->kind == LLG_FILE_INPUT_REAL && target->real) {
        double value;
        if (!llg_scan_integer_to_real(bytes, length, conversion, &value)) return 0;
        llg_scan_store_real(target, value);
        return 1;
    }
    if (target->kind != LLG_FILE_INPUT_PACKED || !target->packed) return 0;
    sv4_t value = SV4_EMPTY;
    if (!llg_scan_integer(bytes, length, conversion, width, is_signed, &value)) {
        sv4_destroy(&value);
        return 0;
    }
    llg_ref_write_owned(target->packed, value);
    return 1;
}

static void llg_scan_token_destroy(void* object) {
    free(*(unsigned char**)object);
}

// `%t` (Table 21-8): the token is a real number in the `$timeformat` unit,
// rounded to the `$timeformat` precision and converted to the calling
// scope's unit. Both steps run on the decimal digits so that the standard's
// example (10.345 ms at precision 2 into a 1ns unit gives 10350000.0) is
// exact rather than subject to binary rounding.
static int llg_scan_time_value(const unsigned char* bytes, size_t length,
                               const llg_scan_input_t* input, double* result) {
    size_t at = 0;
    int negative = 0;
    if (at < length && (bytes[at] == '+' || bytes[at] == '-')) negative = bytes[at++] == '-';
    char* digits = (char*)llg_checked_malloc(length + 2u, 1, "time input digits");
    size_t count = 0;
    long point = 0;
    int seen_dot = 0;
    for (; at < length && bytes[at] != 'e' && bytes[at] != 'E'; ++at) {
        if (bytes[at] == '.') { seen_dot = 1; continue; }
        digits[count++] = (char)bytes[at];
        if (!seen_dot) ++point;
    }
    if (at < length) {
        long exponent = 0;
        int exponent_negative = 0;
        ++at;
        if (at < length && (bytes[at] == '+' || bytes[at] == '-'))
            exponent_negative = bytes[at++] == '-';
        for (; at < length; ++at)
            if (exponent < 100000) exponent = exponent * 10 + (bytes[at] - '0');
        point += exponent_negative ? -exponent : exponent;
    }
    int display = llg_time_unit_exponent(g.time_format.unit_fs);
    int scope = llg_time_unit_exponent(input->time_unit_fs);
    if (scope == INT_MIN) scope = llg_time_unit_exponent(g.design_precision_fs);
    if (display == INT_MIN || scope == INT_MIN) display = scope = 0;
    // Round half away from zero at the display precision.
    long keep = point + (long)g.time_format.precision;
    if (keep < (long)count) {
        int up = keep >= 0 && digits[keep] >= '5';
        if (keep < 0) keep = 0;
        count = (size_t)keep;
        if (up) {
            size_t index = count;
            while (index > 0 && digits[index - 1u] == '9') digits[--index] = '0';
            if (index == 0) {
                memmove(digits + 1, digits, count);
                digits[0] = '1';
                ++count;
                ++point;
            } else {
                digits[index - 1u]++;
            }
        }
    }
    point += (long)display - (long)scope;
    double value = 0.0;
    if (count > 0 && point > 400) value = HUGE_VAL;
    else if (count > 0 && point > -400) {
        // Render "0.<zeros><digits>" or "<digits><zeros>.<rest>" for strtod.
        size_t lead = point < 0 ? (size_t)(-point) : 0;
        size_t trail = point > (long)count ? (size_t)point - count : 0;
        size_t size = count + lead + trail + 4u;
        char* text = (char*)llg_checked_malloc(size, 1, "time input text");
        size_t used = 0;
        if (point <= 0) {
            text[used++] = '0';
            text[used++] = '.';
            memset(text + used, '0', lead);
            used += lead;
            memcpy(text + used, digits, count);
            used += count;
        } else if ((size_t)point >= count) {
            memcpy(text + used, digits, count);
            used += count;
            memset(text + used, '0', trail);
            used += trail;
        } else {
            memcpy(text + used, digits, (size_t)point);
            used += (size_t)point;
            text[used++] = '.';
            memcpy(text + used, digits + point, count - (size_t)point);
            used += count - (size_t)point;
        }
        text[used] = 0;
        value = strtod(text, NULL);
        free(text);
    }
    free(digits);
    *result = negative ? -value : value;
    return 1;
}

static int llg_scan_store_time(const unsigned char* bytes, size_t length,
                               const llg_scan_input_t* input,
                               const llg_file_input_target_t* target) {
    double value;
    if (!target || !llg_scan_time_value(bytes, length, input, &value)) return 0;
    if (target->kind == LLG_FILE_INPUT_REAL && target->real) {
        llg_scan_store_real(target, value);
        return 1;
    }
    if (target->kind == LLG_FILE_INPUT_PACKED && target->packed) {
        llg_ref_write_owned(target->packed, sv4_from_real(value, target->packed->width,
                                                          target->packed->is_signed));
        return 1;
    }
    return 0;
}

// `%u` / `%z` (Table 21-8): unformatted data in the layout `$fwrite` gives
// them, 32-bit little-endian words with the least significant word first;
// `%z` writes each word as its VPI aval/bval pair (0:00 1:10 Z:01 X:11).
// Exactly enough data to fill the destination is read; no white space is
// skipped. A short read assigns nothing.
static int llg_scan_binary(llg_scan_input_t* input, char conversion,
                           const llg_file_input_target_t* target) {
    if (!target || target->kind != LLG_FILE_INPUT_PACKED || !target->packed ||
        target->packed->width == 0)
        return 0;
    uint32_t width = target->packed->width;
    size_t planes = conversion == 'z' ? 2u : 1u;
    sv4_t value = sv4_zero(width, target->packed->is_signed);
    uint64_t words = ((uint64_t)width + 31u) / 32u;
    size_t consumed = 0;
    for (uint64_t word = 0; word < words; ++word) {
        uint32_t plane[2] = {0, 0};
        for (size_t p = 0; p < planes; ++p) {
            for (unsigned byte = 0; byte < 4u; ++byte) {
                int c = llg_scan_get(input);
                if (c == EOF) {
                    sv4_destroy(&value);
                    if (!consumed) {
                        input->input_failure = 1;
                        return -1;
                    }
                    return 0;
                }
                ++consumed;
                plane[p] |= (uint32_t)(unsigned char)c << (byte * 8u);
            }
        }
        uint32_t aval = plane[0];
        uint32_t bval = conversion == 'z' ? plane[1] : 0u;
        uint64_t bits = aval & ~bval, x = aval & bval, z = ~aval & bval;
        size_t limb = (size_t)(word / 2u);
        unsigned shift = (unsigned)(word % 2u) * 32u;
        llg_sv4_set_word(&value, limb,
                         llg_sv4_word(value, limb, LLG_SV4_BITS) | (bits << shift),
                         llg_sv4_word(value, limb, LLG_SV4_X) | (x << shift),
                         llg_sv4_word(value, limb, LLG_SV4_Z) | (z << shift));
    }
    llg_ref_write_owned(target->packed, value);
    return 1;
}

// `%v` (Table 21-8, 21.2.1.5): one three-character strength token, assigned
// as its four-state value. A mnemonic pair or two strength digits precede
// the value character; `HiZ` is high impedance. L and H read as X.
static int llg_scan_strength(llg_scan_input_t* input, const llg_file_input_target_t* target,
                             int suppressed) {
    if (!llg_scan_skip_space(input)) return -1;
    char token[3];
    for (unsigned i = 0; i < 3u; ++i) {
        int c = llg_scan_get(input);
        if (c == EOF || llg_scan_is_space(input, c)) {
            if (c != EOF) (void)llg_scan_unget(input, c);
            return 0;
        }
        token[i] = (char)c;
    }
    static const char* const mnemonics[] = {"Su", "St", "Pu", "La", "We", "Me", "Sm", "Hi"};
    unsigned state;
    if (memcmp(token, "HiZ", 3) == 0) {
        state = 3;
    } else {
        int known = isdigit((unsigned char)token[0]) && token[0] <= '7' &&
                    isdigit((unsigned char)token[1]) && token[1] <= '7';
        for (size_t i = 0; !known && i < sizeof(mnemonics) / sizeof(mnemonics[0]); ++i)
            known = memcmp(token, mnemonics[i], 2) == 0;
        if (!known) return 0;
        switch (token[2]) {
        case '0': state = 0; break;
        case '1': state = 1; break;
        case 'X': case 'x': case 'L': case 'H': state = 2; break;
        case 'Z': case 'z': state = 3; break;
        default: return 0;
        }
    }
    if (suppressed) return 2;
    if (!target || target->kind != LLG_FILE_INPUT_PACKED || !target->packed ||
        target->packed->width == 0)
        return 0;
    sv4_t value = sv4_zero(target->packed->width, target->packed->is_signed);
    llg_scan_set_bit(&value, 0, (int)state);
    llg_ref_write_owned(target->packed, value);
    return 1;
}

static int llg_scan_conversion(llg_scan_input_t* input, char conversion,
                               size_t width, int suppressed,
                               const llg_file_input_target_t* target) {
    unsigned char* bytes = NULL;
    size_t length = 0;
    int ok;
    if (conversion == 'u' || conversion == 'z') {
        // Unformatted data is sized by its destination; a suppressed field
        // has none, so the directive fails.
        return suppressed ? 0 : llg_scan_binary(input, conversion, target);
    }
    if (conversion == 'v') return llg_scan_strength(input, target, suppressed);
    if (conversion == 'm') {
        // `%m` reads no input; it assigns the calling scope's name.
        if (suppressed) return 2;
        const char* scope = input->scope ? input->scope : "";
        return target && llg_scan_assign((const unsigned char*)scope, strlen(scope), 'm',
                                         target,
                                         target->packed ? target->packed->width : 32u,
                                         target->packed ? target->packed->is_signed : 0)
            ? 1 : 0;
    }
    if (conversion == 'c') {
        ok = llg_scan_chars(input, width ? width : 1u, &bytes, &length);
    } else {
        if (!llg_scan_skip_space(input)) return -1;
        ok = conversion == 's'
            ? llg_scan_token(input, width ? width : SIZE_MAX, &bytes, &length)
            : llg_scan_numeric(input, conversion, width ? width : SIZE_MAX, &bytes, &length);
    }
    if (!ok) return input->input_failure ? -1 : 0;
    if (suppressed) {
        // The lexical conversion above still runs; only assignment is suppressed.
        free(bytes);
        return 2;
    }
    if (!target) {
        free(bytes);
        return 0;
    }
    uint32_t target_width = target->packed ? target->packed->width : 32u;
    int target_signed = target->packed ? target->packed->is_signed : 1;
    llg_value_scope_t* token_scope = llg_value_scope_begin_object(
        sizeof(unsigned char*), llg_scan_token_destroy);
    *(unsigned char**)llg_value_scope_object(token_scope) = bytes;
    ok = conversion == 't'
        ? llg_scan_store_time(bytes, length, input, target)
        : llg_scan_assign(bytes, length, conversion, target, target_width, target_signed);
    llg_value_scope_end(token_scope);
    return ok ? 1 : 0;
}

static int llg_scan_format(llg_scan_input_t* input, const char* format,
                           const llg_file_input_target_t* targets, int target_count) {
    if (!format || target_count < 0) return 0;
    int assigned = 0;
    int matched = 0;
    int target_index = 0;
    size_t length = strlen(format);
    for (size_t i = 0; i < length;) {
        unsigned char format_char = (unsigned char)format[i++];
        if (isspace(format_char)) {
            while (i < length && isspace((unsigned char)format[i])) i++;
            (void)llg_scan_skip_space(input);
            continue;
        }
        if (format_char != '%') {
            int value = llg_scan_get(input);
            if (value != format_char) {
                if (value == EOF) input->input_failure = 1;
                (void)llg_scan_unget(input, value);
                break;
            }
            continue;
        }
        if (i >= length) break;
        if (format[i] == '%') {
            i++;
            int value = llg_scan_get(input);
            if (value != '%') {
                if (value == EOF) input->input_failure = 1;
                (void)llg_scan_unget(input, value);
                break;
            }
            continue;
        }
        int suppressed = 0;
        if (format[i] == '*') { suppressed = 1; i++; }
        size_t width = 0;
        while (i < length && isdigit((unsigned char)format[i])) {
            unsigned digit = (unsigned)(format[i++] - '0');
            if (width > (SIZE_MAX - digit) / 10u) width = SIZE_MAX;
            else width = width * 10u + digit;
        }
        // C length modifiers are tolerated; `z` and `t` are conversions here.
        while (i < length && (format[i] == 'l' || format[i] == 'L' || format[i] == 'j')) i++;
        if (i >= length) break;
        char conversion = format[i++];
        if (conversion >= 'A' && conversion <= 'Z') conversion = (char)(conversion - 'A' + 'a');
        if (!strchr("dioxhbcsfegtuvzm", conversion)) break;
        const llg_file_input_target_t* target = NULL;
        if (!suppressed) {
            if (target_index >= target_count) break;
            target = &targets[target_index++];
        }
        int converted = llg_scan_conversion(input, conversion, width, suppressed, target);
        if (converted < 0) break;
        if (converted == 0) break;
        matched = 1;
        if (converted == 1) assigned++;
    }
    return assigned || matched || !input->input_failure ? assigned : -1;
}

int llg_file_scanf_scoped(uint32_t descriptor, const char* format,
                          const llg_file_input_target_t* targets, int target_count,
                          const char* scope, uint64_t time_unit_fs) {
    llg_file_slot_t* slot;
    // No input can be read from an invalid, closed or multichannel descriptor,
    // so the call ends before its first conversion: EOF (21.3.4.3).
    if (!llg_file_single_ordinary(descriptor, &slot)) return EOF;
    llg_scan_input_t input = {slot, NULL, 0, 0, 0, scope, time_unit_fs};
    return llg_scan_format(&input, format, targets, target_count);
}

int llg_file_scanf(uint32_t descriptor, const char* format,
                   const llg_file_input_target_t* targets, int target_count) {
    return llg_file_scanf_scoped(descriptor, format, targets, target_count, NULL, 0);
}

int llg_string_scanf_scoped(const char* source, size_t source_length,
                            const char* format,
                            const llg_file_input_target_t* targets, int target_count,
                            const char* scope, uint64_t time_unit_fs) {
    llg_scan_input_t input = {NULL, (const unsigned char*)source, source_length, 0, 0,
                              scope, time_unit_fs};
    return llg_scan_format(&input, format, targets, target_count);
}

int llg_string_scanf(const char* source, size_t source_length,
                     const char* format,
                     const llg_file_input_target_t* targets, int target_count) {
    return llg_string_scanf_scoped(source, source_length, format, targets, target_count,
                                   NULL, 0);
}

// The text of a packed `$sscanf` source or format: its bytes from the most
// significant end with leading zero bytes dropped (they only pad the value);
// interior zero bytes stay and read as white space. Unknown bits make the
// call return EOF (21.3.4.3), reported through `unknown`.
llg_string_t llg_scan_text_from_packed(sv4_t value, int* unknown) {
    if (unknown) *unknown = sv4_is_unknown(value);
    uint32_t width = llg_sv4_width(value);
    size_t count = ((size_t)width + 7u) / 8u;
    unsigned char* bytes = (unsigned char*)llg_checked_malloc(count ? count : 1u, 1,
                                                              "scan source text");
    size_t used = 0;
    for (size_t index = count; index > 0; --index) {
        size_t bit = (index - 1u) * 8u;
        unsigned char byte = 0;
        for (unsigned part = 0; part < 8u; ++part)
            if (llg_sv4_state(value, (uint64_t)(bit + part)) == 1) byte |= (unsigned char)(1u << part);
        if (!used && !byte) continue;
        bytes[used++] = byte;
    }
    llg_string_t text = llg_string_bytes((const char*)bytes, used);
    free(bytes);
    return text;
}

static int llg_file_read_byte(llg_file_slot_t* slot, unsigned char* output) {
    int value = llg_file_getc_slot(slot);
    if (value == EOF) return 0;
    *output = (unsigned char)value;
    return 1;
}

int llg_file_read_packed(uint32_t descriptor, llg_ref_t* target) {
    llg_file_slot_t* slot;
    if (!target || !llg_file_single_ordinary(descriptor, &slot) || target->width == 0)
        return 0;
    sv4_t value = llg_ref_read(target);
    size_t bytes = ((size_t)target->width + 7u) / 8u;
    int read = 0;
    for (size_t index = 0; index < bytes; index++) {
        unsigned char byte;
        if (!llg_file_read_byte(slot, &byte)) break;
        size_t bit_base = (bytes - 1u - index) * 8u;
        for (unsigned bit = 0; bit < 8u; bit++)
            llg_scan_set_bit(&value, (uint32_t)(bit_base + bit), (byte >> bit) & 1u);
        read++;
    }
    if (read) llg_ref_write_owned(target, value);
    else sv4_destroy(&value);
    return read;
}

static int fixed_file_read_array(uint32_t descriptor, sv4_t* values, llg_fixed_array_t* fixed, uint32_t elem_width,
                        int elem_signed, int elem_two_state, uint64_t total,
                        const int32_t* dimensions, int dimension_count,
                        int has_start, sv4_t start, int has_count, sv4_t count) {
    llg_file_slot_t* slot;
    if ((!values && !fixed) || total == 0 || elem_width == 0 || !dimensions || dimension_count <= 0 ||
        !llg_file_single_ordinary(descriptor, &slot)) return 0;
    /* IEEE 1364-2001 17.2.4.4 / 1800-2009 21.3.4.4: a memory
       is read from its lowest address toward its highest, not in declaration
       order. Rank-one descending storage therefore walks backward. */
    int reverse_storage = dimension_count == 1 && dimensions[0] > dimensions[1];
    uint64_t offset = reverse_storage ? total - 1u : 0;
    if (has_start) {
        int64_t index;
        int64_t low = dimensions[0] < dimensions[1] ? dimensions[0] : dimensions[1];
        int64_t high = dimensions[0] > dimensions[1] ? dimensions[0] : dimensions[1];
        if (!sv4_to_index_i64(start, &index) || index < low || index > high) {
            llg_file_slot_failure(slot, "file read start index is out of bounds");
            return 0;
        }
        offset = dimensions[0] >= dimensions[1]
                     ? (uint64_t)((int64_t)dimensions[0] - index)
                     : (uint64_t)(index - (int64_t)dimensions[0]);
    }
    if (offset >= total) {
        llg_file_slot_failure(slot, "file read start index is out of bounds");
        return 0;
    }
    uint64_t available = reverse_storage ? offset + 1u : total - offset;
    uint64_t requested = available;
    if (has_count) {
        int64_t value;
        if (!sv4_to_index_i64(count, &value) || value < 0) {
            llg_file_slot_failure(slot, "file read count is out of bounds");
            return 0;
        }
        requested = (uint64_t)value;
        if (requested > available) requested = available;
    }
    size_t bytes_per_element = ((size_t)elem_width + 7u) / 8u;
    int result = 0;
    for (uint64_t element = 0; element < requested; element++) {
        uint64_t position = reverse_storage ? offset - element : offset + element;
        sv4_t value = sv4_clone(fixed ? llg_fixed_array_peek(fixed, position) : &values[position]);
        int read = 0;
        for (size_t index = 0; index < bytes_per_element; index++) {
            unsigned char byte;
            if (!llg_file_read_byte(slot, &byte)) break;
            size_t bit_base = (bytes_per_element - 1u - index) * 8u;
            for (unsigned bit = 0; bit < 8u; bit++)
                llg_scan_set_bit(&value, (uint32_t)(bit_base + bit), (byte >> bit) & 1u);
            read++;
        }
        if (!read) { sv4_destroy(&value); break; }
        llg_sv4_set_signed(&value, (int8_t)elem_signed);
        if (elem_two_state) sv4_replace(&value, sv4_to_two_state(value));
        llg_value_scope_t* value_scope = llg_value_scope_begin(1);
        sv4_t* owned = llg_value_scope_values(value_scope);
        owned[0] = value;
        llg_ba(fixed ? llg_fixed_array_cell(fixed, position) : &values[position], owned[0]);
        llg_value_scope_end(value_scope);
        result += read;
        if ((size_t)read < bytes_per_element) break;
    }
    return result;
}

static void llg_file_write_typed(uint32_t descriptor, const char* output,
                                 size_t length, int newline) {
    if (!llg_file_mask_valid(descriptor)) return;
    for (unsigned i = 0; i < LLG_FILE_SLOTS; i++) {
        if (!llg_file_selected(descriptor, i)) continue;
        llg_file_slot_t* slot = &llg_file_slots[i];
        if (fwrite(output, 1, length, slot->stream) != length ||
            (newline && fputc('\n', slot->stream) == EOF) ||
            fflush(slot->stream) != 0) {
            llg_file_slot_failure(slot, "file output failed");
        }
    }
}

static char* llg_typed_line_alloc(const char* fmt, llg_fmt_arg_t* args, int n,
                                  const char* scope, size_t* length) {
    size_t cap = strlen(fmt) + (scope ? strlen(scope) : 0) + 64u;
    for (int i = 0; i < n; i++) {
        size_t extra = 64u;
        if (args[i].kind == LLG_FMT_PACKED || args[i].kind == LLG_FMT_STRENGTH) {
            if (llg_sv4_width(args[i].value.packed) > (SIZE_MAX - extra) / 8u)
                llg_fatal_allocation("typed formatted line", 1, SIZE_MAX);
            extra += (size_t)llg_sv4_width(args[i].value.packed) * 8u;
        }
        if (args[i].kind == LLG_FMT_STRING || args[i].kind == LLG_FMT_TEXT) {
            if (args[i].value.string.len > SIZE_MAX - extra)
                llg_fatal_allocation("typed formatted line", 1, SIZE_MAX);
            if (args[i].value.string.len > (SIZE_MAX - extra) / 8u)
                llg_fatal_allocation("formatted string", args[i].value.string.len, 8u);
            extra += args[i].value.string.len * 8u;
        }
        if ((size_t)g.time_format.minimum_field_width > SIZE_MAX - extra)
            llg_fatal_allocation("typed formatted line", 1, SIZE_MAX);
        extra += (size_t)g.time_format.minimum_field_width;
        if ((size_t)g.time_format.precision > SIZE_MAX - extra)
            llg_fatal_allocation("typed formatted line", 1, SIZE_MAX);
        extra += (size_t)g.time_format.precision;
        if (g.time_format.suffix.len > SIZE_MAX - extra)
            llg_fatal_allocation("typed formatted line", 1, SIZE_MAX);
        extra += g.time_format.suffix.len;
        if (extra > SIZE_MAX - cap) llg_fatal_allocation("typed formatted line", cap, extra);
        cap += extra;
    }
    // Include explicit field widths/precisions and repeated scope conversions.
    for (const char* p = fmt; *p;) {
        if (*p++ != '%') continue;
        const char* start = p - 1;
        llg_fmt_spec_t spec;
        p = llg_parse_typed_spec(start, p, &spec);
        cap = llg_format_size_add(cap, (size_t)spec.width);
        cap = llg_format_size_add(cap, (size_t)spec.precision);
        cap = llg_format_size_add(cap, scope ? strlen(scope) : 0);
        if (*p) ++p;
    }
    char* out = llg_checked_malloc(cap, 1, "typed formatted line");
    *length = llg_format_typed(out, cap, fmt, args, n, scope);
    return out;
}

llg_string_t llg_string_format_typed(llg_string_t format, llg_fmt_arg_t* args,
                                     int n, const char* scope) {
    const char* text = format.data ? format.data : "";
    size_t length = 0;
    char* output = llg_typed_line_alloc(text, args, n, scope, &length);
    llg_string_t result = llg_string_bytes(output, length);
    free(output);
    llg_string_destroy(&format);
    llg_fmt_args_destroy(args, n);
    return result;
}

static void llg_print_typed_to(uint32_t descriptor, const char* fmt,
                               llg_fmt_arg_t* args, int n, const char* scope,
                               int newline) {
    size_t len = 0;
    char* out = llg_typed_line_alloc(fmt, args, n, scope, &len);
    llg_file_write_typed(descriptor, out, len, newline);
    free(out);
}

void llg_file_display_typed(uint32_t descriptor, const char* fmt,
                            llg_fmt_arg_t* args, int n, const char* scope,
                            int newline) {
    llg_print_typed_to(descriptor, fmt, args, n, scope, newline);
    llg_fmt_args_destroy(args, n);
}

int llg_file_read_array(uint32_t descriptor, sv4_t* values, uint32_t elem_width,
                        int elem_signed, int elem_two_state, uint64_t total,
                        const int32_t* dimensions, int dimension_count,
                        int has_start, sv4_t start, int has_count, sv4_t count) {
    return fixed_file_read_array(descriptor, values, NULL, elem_width, elem_signed, elem_two_state, total, dimensions, dimension_count, has_start, start, has_count, count);
}

int llg_fixed_file_read_array(uint32_t descriptor, llg_fixed_array_t* values, uint32_t elem_width,
                        int elem_signed, int elem_two_state, uint64_t total,
                        const int32_t* dimensions, int dimension_count,
                        int has_start, sv4_t start, int has_count, sv4_t count) {
    return fixed_file_read_array(descriptor, NULL, values, elem_width, elem_signed, elem_two_state, total, dimensions, dimension_count, has_start, start, has_count, count);
}

// `$fread` into a whole dynamic array or queue (SIM-026): the elements are
// addresses 0..size-1 of a memory, read like a fixed memory (21.3.4.4); one
// contents notification follows when any byte was read.
static int llg_file_read_resizable(uint32_t descriptor, sv4_t* values, size_t size,
                                   uint32_t elem_width, int elem_signed, int elem_two_state,
                                   int has_start, sv4_t start, int has_count, sv4_t count,
                                   llg_container_notify_fn notify, sv4_t* contents,
                                   sv4_t* shape) {
    if (size > (size_t)INT32_MAX) {
        llg_file_global_failure("file read destination is too large");
        return 0;
    }
    if (size == 0) return 0;
    const int32_t dimensions[2] = {0, (int32_t)(size - 1u)};
    int result = fixed_file_read_array(descriptor, values, NULL, elem_width, elem_signed,
                                       elem_two_state, size, dimensions, 1, has_start,
                                       start, has_count, count);
    if (result > 0 && notify) notify(contents, shape, LLG_CONTAINER_CHANGED_CONTENTS);
    return result;
}

int llg_dyn_file_read(uint32_t descriptor, struct llg_dyn_array_t* array,
                      int has_start, sv4_t start, int has_count, sv4_t count) {
    if (!array) return 0;
    return llg_file_read_resizable(descriptor, array->data, array->size,
                                   array->element_width, array->element_signed,
                                   array->element_two_state, has_start, start, has_count,
                                   count, array->notify, array->contents_dependency,
                                   array->shape_dependency);
}

int llg_queue_file_read(uint32_t descriptor, struct llg_queue_t* queue,
                        int has_start, sv4_t start, int has_count, sv4_t count) {
    if (!queue) return 0;
    return llg_file_read_resizable(descriptor, queue->data, queue->size,
                                   queue->element_width, queue->element_signed,
                                   queue->element_two_state, has_start, start, has_count,
                                   count, queue->notify, queue->contents_dependency,
                                   queue->shape_dependency);
}
