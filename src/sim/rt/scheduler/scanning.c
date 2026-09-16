
typedef struct {
    llg_file_slot_t* file;
    const unsigned char* bytes;
    size_t length;
    size_t position;
    int input_failure;
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

static int llg_scan_skip_space(llg_scan_input_t* input) {
    int value;
    do {
        value = llg_scan_get(input);
    } while (value != EOF && isspace((unsigned char)value));
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
        if (value == EOF || isspace((unsigned char)value)) {
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
    int real = conversion == 'f' || conversion == 'e' || conversion == 'g';
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
    if (bit >= value->width) return;
    uint32_t limb = bit / 64u;
    uint64_t mask = 1ULL << (bit % 64u);
    value->bits[limb] &= ~mask;
    value->x[limb] &= ~mask;
    value->z[limb] &= ~mask;
    if (state == 1) value->bits[limb] |= mask;
    else if (state == 2) value->x[limb] |= mask;
    else if (state == 3) value->z[limb] |= mask;
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
            uint64_t carry = digit;
            uint32_t limbs = (width + 63u) / 64u;
            for (uint32_t limb = 0; limb < limbs; limb++) {
                uint64_t low = (result->bits[limb] & UINT32_MAX) * 10u + carry;
                uint64_t high = (result->bits[limb] >> 32) * 10u + (low >> 32);
                result->bits[limb] = (high << 32) | (low & UINT32_MAX);
                carry = high >> 32;
            }
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

static int llg_scan_assign(const unsigned char* bytes, size_t length, char conversion,
                           const llg_file_input_target_t* target, uint32_t width,
                           int is_signed) {
    if (!target) return 0;
    if (conversion == 's' || conversion == 'c') {
        if (target->kind == LLG_FILE_INPUT_STRING && target->string) {
            llg_string_move(target->string, llg_string_bytes((const char*)bytes, length));
            return 1;
        }
        if (target->kind == LLG_FILE_INPUT_PACKED && target->packed) {
            sv4_t value = SV4_EMPTY;
            llg_scan_bytes_to_packed(bytes, length, width, is_signed, &value);
            llg_ref_write(target->packed, value);
            sv4_destroy(&value);
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
            llg_ba_d(target->real, target->shortreal ? (double)(float)value : value);
            return 1;
        }
        if (target->kind == LLG_FILE_INPUT_PACKED && target->packed) {
            sv4_t packed = sv4_from_real(value, width, (int8_t)is_signed);
            llg_ref_write(target->packed, packed);
            sv4_destroy(&packed);
            return 1;
        }
        return 0;
    }
    if (target->kind != LLG_FILE_INPUT_PACKED || !target->packed) return 0;
    sv4_t value = SV4_EMPTY;
    if (!llg_scan_integer(bytes, length, conversion, width, is_signed, &value)) return 0;
    llg_ref_write(target->packed, value);
            sv4_destroy(&value);
    return 1;
}

static int llg_scan_conversion(llg_scan_input_t* input, char conversion,
                               size_t width, int suppressed,
                               const llg_file_input_target_t* target) {
    unsigned char* bytes = NULL;
    size_t length = 0;
    int ok;
    if (conversion == 'c') {
        ok = llg_scan_chars(input, width ? width : 1u, &bytes, &length);
    } else {
        if (!llg_scan_skip_space(input)) return 0;
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
    ok = llg_scan_assign(bytes, length, conversion, target, target_width, target_signed);
    free(bytes);
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
        while (i < length && (format[i] == 'l' || format[i] == 'L' ||
                              format[i] == 'j' ||
                              format[i] == 'z' || format[i] == 't')) i++;
        if (i >= length) break;
        char conversion = format[i++];
        if (conversion >= 'A' && conversion <= 'Z') conversion = (char)(conversion - 'A' + 'a');
        if (conversion != 'd' && conversion != 'i' && conversion != 'u' &&
            conversion != 'o' && conversion != 'x' && conversion != 'h' &&
            conversion != 'b' && conversion != 'c' && conversion != 's' &&
            conversion != 'f' && conversion != 'e' && conversion != 'g') break;
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

int llg_file_scanf(uint32_t descriptor, const char* format,
                   const llg_file_input_target_t* targets, int target_count) {
    llg_file_slot_t* slot;
    if (!llg_file_single_ordinary(descriptor, &slot)) return 0;
    llg_scan_input_t input = {slot, NULL, 0, 0, 0};
    return llg_scan_format(&input, format, targets, target_count);
}

int llg_string_scanf(const char* source, size_t source_length,
                     const char* format,
                     const llg_file_input_target_t* targets, int target_count) {
    llg_scan_input_t input = {NULL, (const unsigned char*)source, source_length, 0, 0};
    return llg_scan_format(&input, format, targets, target_count);
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
    if (read) llg_ref_write(target, value);
    sv4_destroy(&value);
    return read;
}

int llg_file_read_array(uint32_t descriptor, sv4_t* values, uint32_t elem_width,
                        int elem_signed, int elem_two_state, uint64_t total,
                        const int32_t* dimensions, int dimension_count,
                        int has_start, sv4_t start, int has_count, sv4_t count) {
    llg_file_slot_t* slot;
    if (!values || total == 0 || elem_width == 0 || !dimensions || dimension_count <= 0 ||
        !llg_file_single_ordinary(descriptor, &slot)) return 0;
    uint64_t offset = 0;
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
    uint64_t requested = total - offset;
    if (has_count) {
        int64_t value;
        if (!sv4_to_index_i64(count, &value) || value < 0) {
            llg_file_slot_failure(slot, "file read count is out of bounds");
            return 0;
        }
        requested = (uint64_t)value;
        if (requested > total - offset) requested = total - offset;
    }
    size_t bytes_per_element = ((size_t)elem_width + 7u) / 8u;
    int result = 0;
    for (uint64_t element = 0; element < requested; element++) {
        sv4_t value = sv4_clone(&values[offset + element]);
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
        value.is_signed = (int8_t)elem_signed;
        if (elem_two_state) sv4_replace(&value, sv4_to_two_state(value));
        llg_ba(&values[offset + element], value);
        sv4_destroy(&value);
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
        if (args[i].kind == LLG_FMT_PACKED) {
            if (args[i].value.packed.width > (SIZE_MAX - extra) / 8u)
                llg_fatal_allocation("typed formatted line", 1, SIZE_MAX);
            extra += (size_t)args[i].value.packed.width * 8u;
        }
        if (args[i].kind == LLG_FMT_STRING) {
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
