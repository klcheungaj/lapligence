
// ── Memory file tasks ────────────────────────────────────────────────────────

enum {
    LLG_MEMORY_TOKEN_EOF = 0,
    LLG_MEMORY_TOKEN_DATA = 1,
    LLG_MEMORY_TOKEN_ADDRESS = 2,
    LLG_MEMORY_TOKEN_ERROR = -1,
};

typedef struct {
    sv4_t value;
    uint64_t digits;
    int too_wide;
    int64_t address;
    int invalid_index;
} llg_memory_value_t;

static void llg_memory_warning(const char* path, const char* format, ...) {
    va_list args;
    fprintf(stderr, "llg: memory file `%s`: ", path ? path : "");
    va_start(args, format);
    vfprintf(stderr, format, args);
    va_end(args);
    fputc('\n', stderr);
}

static char* llg_memory_path_copy(llg_string_t path) {
    char* copy = (char*)llg_checked_malloc(path.len + 1u, 1, "memory file path");
    if (path.len && path.data) memcpy(copy, path.data, path.len);
    copy[path.len] = 0;
    llg_string_destroy(&path);
    return copy;
}

static void llg_memory_shift_limbs(uint64_t* limbs, uint32_t count, unsigned shift) {
    uint64_t carry = 0;
    for (uint32_t i = 0; i < count; ++i) {
        uint64_t old = limbs[i];
        limbs[i] = (old << shift) | carry;
        carry = old >> (64u - shift);
    }
}

// Append one binary or hexadecimal digit, retaining the least significant
// token-capacity bits. The caller diagnoses an over-width token separately.
static void llg_memory_append_digit(llg_memory_value_t* value, unsigned bits,
                                    int state, unsigned numeric) {
    if (value->too_wide) return;
    if (value->digits >= (LLG_SUPPORTED_WIDTH_LIMIT - 1u) / bits) {
        value->too_wide = 1;
        return;
    }
    uint32_t required = (uint32_t)((value->digits + 1u) * bits);
    if (required > value->value.width) {
        uint32_t capacity = value->value.width ? value->value.width * 2u : 64u;
        if (capacity >= LLG_SUPPORTED_WIDTH_LIMIT) capacity = LLG_SUPPORTED_WIDTH_LIMIT - 1u;
        sv4_replace(&value->value, sv4_resize(value->value, capacity, 0));
    }
    uint32_t count = (value->value.width + 63u) / 64u;
    llg_memory_shift_limbs(value->value.bits, count, bits);
    llg_memory_shift_limbs(value->value.x, count, bits);
    llg_memory_shift_limbs(value->value.z, count, bits);
    uint64_t mask = bits == 1u ? 1u : 0xfu;
    if (state == 1) value->value.x[0] |= mask;
    else if (state == 2) value->value.z[0] |= mask;
    else value->value.bits[0] |= (uint64_t)numeric & mask;
    ++value->digits;
}

static int llg_memory_digit(int c, int radix, int* state, unsigned* numeric) {
    if (c == 'x' || c == 'X') {
        *state = 1;
        *numeric = 0;
        return 1;
    }
    if (c == 'z' || c == 'Z') {
        *state = 2;
        *numeric = 0;
        return 1;
    }
    if (c >= '0' && c <= '1') {
        *state = 0;
        *numeric = (unsigned)(c - '0');
        return 1;
    }
    if (radix == 16) {
        if (c >= '2' && c <= '9') {
            *state = 0;
            *numeric = (unsigned)(c - '0');
            return 1;
        }
        if (c >= 'a' && c <= 'f') {
            *state = 0;
            *numeric = (unsigned)(c - 'a' + 10);
            return 1;
        }
        if (c >= 'A' && c <= 'F') {
            *state = 0;
            *numeric = (unsigned)(c - 'A' + 10);
            return 1;
        }
    }
    return 0;
}

static int llg_memory_next_noncomment(FILE* stream) {
    for (;;) {
        int c = fgetc(stream);
        if (c == EOF || !isspace((unsigned char)c)) {
            if (c != '/') return c;
            int next = fgetc(stream);
            if (next == '/') {
                while ((c = fgetc(stream)) != EOF && c != '\n') {}
                continue;
            }
            if (next == '*') {
                int previous = 0;
                int closed = 0;
                while ((c = fgetc(stream)) != EOF) {
                    if (previous == '*' && c == '/') {
                        closed = 1;
                        break;
                    }
                    previous = c;
                }
                if (!closed) return -2;
                continue;
            }
            if (next != EOF) (void)ungetc(next, stream);
            return '/';
        }
    }
}

static void llg_memory_consume_bad_token(FILE* stream) {
    int c;
    while ((c = fgetc(stream)) != EOF) {
        if (isspace((unsigned char)c)) break;
        if (c == '/') {
            (void)ungetc(c, stream);
            break;
        }
    }
}

static int llg_memory_parse_digits(FILE* stream, int radix, int first,
                                   llg_memory_value_t* value) {
    const unsigned bits = radix == 2 ? 1u : 4u;
    int c = first;
    int saw_digit = 0;
    int malformed = 0;
    while (c != EOF) {
        int state;
        unsigned numeric;
        if (llg_memory_digit(c, radix, &state, &numeric)) {
            llg_memory_append_digit(value, bits, state, numeric);
            saw_digit = 1;
        } else if (c == '_') {
            // Underscores are separators inside a memory word.
        } else if (isspace((unsigned char)c)) {
            break;
        } else if (c == '/') {
            (void)ungetc(c, stream);
            break;
        } else {
            malformed = 1;
            llg_memory_consume_bad_token(stream);
            break;
        }
        c = fgetc(stream);
    }
    if (!saw_digit || malformed || value->too_wide) return LLG_MEMORY_TOKEN_ERROR;
    sv4_replace(&value->value, sv4_resize(value->value,
                (uint32_t)(value->digits * bits), 0));
    return LLG_MEMORY_TOKEN_DATA;
}

static int llg_memory_next_token(FILE* stream, int radix,
                                 llg_memory_value_t* value) {
    sv4_destroy(&value->value);
    memset(value, 0, sizeof(*value));
    int c = llg_memory_next_noncomment(stream);
    if (c == EOF) return LLG_MEMORY_TOKEN_EOF;
    if (c == -2) return LLG_MEMORY_TOKEN_ERROR;
    if (c == '@') {
        c = fgetc(stream);
        int negative = 0;
        uint64_t magnitude = 0;
        int saw_digit = 0;
        if (c == '-' || c == '+') {
            negative = c == '-';
            c = fgetc(stream);
        }
        int state;
        unsigned numeric;
        while (c != EOF) {
            if (llg_memory_digit(c, 16, &state, &numeric) && state == 0) {
                saw_digit = 1;
                if (magnitude > (UINT64_MAX - numeric) / 16u) {
                    value->invalid_index = 1;
                } else if (!value->invalid_index) {
                    magnitude = magnitude * 16u + numeric;
                }
            } else if (c == '_') {
                // Underscores are separators inside an address.
            } else if (isspace((unsigned char)c)) {
                break;
            } else if (c == '/') {
                (void)ungetc(c, stream);
                break;
            } else {
                llg_memory_consume_bad_token(stream);
                return LLG_MEMORY_TOKEN_ERROR;
            }
            c = fgetc(stream);
        }
        if (!saw_digit) return LLG_MEMORY_TOKEN_ERROR;
        uint64_t limit = negative ? (uint64_t)INT64_MAX + 1u : (uint64_t)INT64_MAX;
        if (magnitude > limit) value->invalid_index = 1;
        if (!value->invalid_index) {
            value->address = negative
                                 ? (magnitude == limit ? INT64_MIN : -(int64_t)magnitude)
                                 : (int64_t)magnitude;
        }
        return LLG_MEMORY_TOKEN_ADDRESS;
    }
    if (!llg_memory_digit(c, radix, &(int){0}, &(unsigned){0})) {
        llg_memory_consume_bad_token(stream);
        return LLG_MEMORY_TOKEN_ERROR;
    }
    return llg_memory_parse_digits(stream, radix, c, value);
}

static int llg_memory_extent(int32_t left_value, int32_t right_value,
                             uint64_t* extent) {
    int64_t left = left_value;
    int64_t right = right_value;
    uint64_t distance = left >= right ? (uint64_t)(left - right)
                                      : (uint64_t)(right - left);
    if (distance == UINT64_MAX) return 0;
    *extent = distance + 1u;
    return 1;
}

static int llg_memory_address_offset(int64_t address, int32_t left_value,
                                     int32_t right_value, uint64_t* offset) {
    int64_t left = left_value;
    int64_t right = right_value;
    if (address < (left < right ? left : right) ||
        address > (left > right ? left : right)) return 0;
    *offset = left >= right ? (uint64_t)(left - address)
                            : (uint64_t)(address - left);
    return 1;
}

static int llg_memory_descriptor(const char* path, uint64_t total,
                                 const int32_t* dims, int n_dims,
                                 const uint64_t* strides, uint64_t origin,
                                 uint64_t view_total, uint64_t* inner_total) {
    if (!dims || !strides || n_dims <= 0 || total == 0 || view_total == 0 ||
        origin >= total || view_total > total - origin) {
        llg_memory_warning(path, "memory descriptor is invalid");
        return 0;
    }
    uint64_t expected_total = 1;
    for (int dimension = 0; dimension < n_dims; ++dimension) {
        uint64_t extent;
        if (!llg_memory_extent(dims[2 * dimension], dims[2 * dimension + 1], &extent) ||
            expected_total > UINT64_MAX / extent) {
            llg_memory_warning(path, "memory descriptor dimension overflows");
            return 0;
        }
        expected_total *= extent;
    }
    if (expected_total != view_total) {
        llg_memory_warning(path, "memory descriptor size does not match its bounds");
        return 0;
    }
    uint64_t expected_stride = 1;
    for (int dimension = n_dims - 1; dimension >= 0; --dimension) {
        uint64_t extent;
        if (!llg_memory_extent(dims[2 * dimension], dims[2 * dimension + 1], &extent) ||
            strides[dimension] != expected_stride ||
            expected_stride > UINT64_MAX / extent) {
            llg_memory_warning(path, "memory descriptor stride is invalid");
            return 0;
        }
        expected_stride *= extent;
    }
    uint64_t max_offset = 0;
    for (int dimension = 0; dimension < n_dims; ++dimension) {
        uint64_t extent;
        if (!llg_memory_extent(dims[2 * dimension], dims[2 * dimension + 1], &extent) ||
            extent == 0 ||
            (extent - 1u) > UINT64_MAX / strides[dimension] ||
            max_offset > UINT64_MAX - (extent - 1u) * strides[dimension]) {
            llg_memory_warning(path, "memory descriptor range overflows");
            return 0;
        }
        max_offset += (extent - 1u) * strides[dimension];
    }
    if (max_offset >= total - origin) {
        llg_memory_warning(path, "memory descriptor exceeds its source array");
        return 0;
    }
    uint64_t outer_extent;
    if (!llg_memory_extent(dims[0], dims[1], &outer_extent) ||
        outer_extent == 0 || view_total % outer_extent != 0) {
        llg_memory_warning(path, "memory descriptor outer extent is invalid");
        return 0;
    }
    *inner_total = view_total / outer_extent;
    return 1;
}

static int llg_memory_view_index(int64_t address, uint64_t inner_ordinal,
                                 const int32_t* dims, int n_dims,
                                 const uint64_t* strides, uint64_t origin,
                                 uint64_t view_total, uint64_t total,
                                 uint64_t* index) {
    uint64_t outer_extent;
    if (!llg_memory_extent(dims[0], dims[1], &outer_extent) ||
        outer_extent == 0 || inner_ordinal >= view_total / outer_extent) {
        return 0;
    }
    uint64_t outer_offset;
    if (!llg_memory_address_offset(address, dims[0], dims[1], &outer_offset)) {
        return 0;
    }
    if (outer_offset > UINT64_MAX / strides[0]) return 0;
    uint64_t flat = origin + outer_offset * strides[0];
    if (flat < origin) return 0;
    uint64_t ordinal = inner_ordinal;
    for (int dimension = n_dims - 1; dimension >= 1; --dimension) {
        uint64_t extent;
        if (!llg_memory_extent(dims[2 * dimension], dims[2 * dimension + 1], &extent) ||
            extent == 0) {
            return 0;
        }
        uint64_t coordinate = ordinal % extent;
        ordinal /= extent;
        uint64_t offset = dims[2 * dimension] >= dims[2 * dimension + 1]
                              ? extent - 1u - coordinate
                              : coordinate;
        if (offset > UINT64_MAX / strides[dimension] ||
            flat > UINT64_MAX - offset * strides[dimension]) {
            return 0;
        }
        flat += offset * strides[dimension];
    }
    if (ordinal != 0 || flat >= total) return 0;
    *index = flat;
    return 1;
}

static uint64_t llg_memory_range_length(int64_t first, int64_t last) {
    uint64_t distance = first >= last ? (uint64_t)(first - last)
                                      : (uint64_t)(last - first);
    return distance == UINT64_MAX ? UINT64_MAX : distance + 1u;
}

static int llg_memory_bounds(const char* path, uint64_t total,
                             const int32_t* dims, int n_dims,
                             const uint64_t* strides, uint64_t origin,
                             uint64_t view_total, sv4_t start, sv4_t finish,
                             int has_start, int has_finish, int addressing_policy,
                             int64_t* first, int64_t* last,
                             uint64_t* inner_total) {
    if (!llg_memory_descriptor(path, total, dims, n_dims, strides, origin,
                               view_total, inner_total)) {
        return 0;
    }
    int32_t left_value = dims[0], right_value = dims[1];
    int64_t left = left_value, right = right_value;
    if (has_start && !sv4_to_index_i64(start, first)) {
        llg_memory_warning(path, "start address is unknown, negative-width, or out of range");
        return 0;
    }
    if (has_finish && !sv4_to_index_i64(finish, last)) {
        llg_memory_warning(path, "finish address is unknown, negative-width, or out of range");
        return 0;
    }
    if (!has_start) {
        *first = addressing_policy == LLG_MEMORY_ADDRESSING_SYSTEMVERILOG_2009
                     ? (left < right ? left : right)
                     : left;
    }
    if (!has_finish) {
        *last = addressing_policy == LLG_MEMORY_ADDRESSING_SYSTEMVERILOG_2009
                    ? (left > right ? left : right)
                    : right;
    }
    uint64_t ignored_offset;
    if (!llg_memory_address_offset(*first, left_value, right_value, &ignored_offset) ||
        !llg_memory_address_offset(*last, left_value, right_value, &ignored_offset)) {
        llg_memory_warning(path, "selected range includes an address outside the destination memory");
        return 0;
    }
    return 1;
}

static int llg_memory_in_requested_range(int64_t address, int64_t first,
                                         int64_t last) {
    return first <= last ? address >= first && address <= last
                         : address <= first && address >= last;
}

static int llg_memory_enum_value_allowed(sv4_t value,
                                         const sv4_t* enum_values,
                                         uint32_t enum_count) {
    if (!enum_values || enum_count == 0) return 1;
    for (uint32_t index = 0; index < enum_count; ++index) {
        sv4_t match = sv4_case_eq(value, enum_values[index]);
        int allowed = sv4_to_bool(match);
        sv4_destroy(&match);
        if (allowed) return 1;
    }
    return 0;
}

// Memory-file words carry no signed marker. When a known word is wider than an
// enum base, its discarded bits must be redundant for that base: zeroes for an
// unsigned base, or copies of the retained sign bit for a signed base.
static int llg_memory_enum_word_fits_width(sv4_t value, uint32_t width,
                                            int8_t is_signed) {
    if (sv4_is_unknown(value) || value.width <= width) return 1;
    int sign = is_signed && width != 0
                   ? (int)((value.bits[(width - 1u) / 64u] >> ((width - 1u) % 64u)) & 1u)
                   : 0;
    for (uint32_t bit = width; bit < value.width; ++bit) {
        int high = (int)((value.bits[bit / 64u] >> (bit % 64u)) & 1u);
        if (high != sign) return 0;
    }
    return 1;
}

// Memory words are unsigned based digits. Only a leading X/Z digit pads with
// its state; a known leading one still zero-extends, even into signed storage.
static sv4_t llg_memory_word_cast(sv4_t word, uint32_t width, int8_t is_signed) {
    int8_t extend_unknown = 0;
    if (word.width != 0 && width > word.width) {
        uint32_t bit = word.width - 1u;
        uint64_t mask = UINT64_C(1) << (bit % 64u);
        extend_unknown = ((word.x[bit / 64u] | word.z[bit / 64u]) & mask) != 0;
    }
    sv4_t result = sv4_resize(word, width, extend_unknown);
    result.is_signed = is_signed;
    return result;
}

void llg_memory_read_view(llg_string_t path, sv4_t* memory, uint64_t total,
                          uint32_t elem_width, int8_t elem_signed, int8_t two_state,
                          const int32_t* dims, int n_dims,
                          const uint64_t* strides, uint64_t origin,
                          uint64_t view_total, sv4_t start, sv4_t finish,
                          int has_start, int has_finish, int addressing_policy,
                          const sv4_t* enum_values, uint32_t enum_count, int radix) {
    char* filename = llg_memory_path_copy(path);
    FILE* stream = fopen(filename, "r");
    if (!stream) {
        llg_memory_warning(filename, "open for reading failed: %s", strerror(errno));
        free(filename);
        return;
    }
    int64_t first, last;
    uint64_t inner_total;
    if (!llg_memory_bounds(filename, total, dims, n_dims, strides, origin,
                           view_total, start, finish, has_start, has_finish,
                           addressing_policy, &first, &last, &inner_total)) {
        fclose(stream);
        free(filename);
        return;
    }
    uint64_t range = llg_memory_range_length(first, last);
    uint64_t expected = range > UINT64_MAX / inner_total
                            ? UINT64_MAX
                            : range * inner_total;
    uint64_t written = 0;
    int64_t current = first;
    int64_t step = first <= last ? 1 : -1;
    uint64_t inner = 0;
    int warned_extra = 0;
    int warned_unknown = 0;
    int saw_address = 0;
    llg_memory_value_t token = {0};
    for (;;) {
        int kind = llg_memory_next_token(stream, radix, &token);
        if (kind == LLG_MEMORY_TOKEN_EOF) break;
        if (kind == LLG_MEMORY_TOKEN_ERROR) {
            llg_memory_warning(filename, "malformed or over-width memory token");
            continue;
        }
        if (kind == LLG_MEMORY_TOKEN_ADDRESS) {
            saw_address = 1;
            int64_t address = token.address;
            if (token.invalid_index) {
                llg_memory_warning(filename, "address jump is not a known index");
                sv4_destroy(&token.value);
                fclose(stream);
                free(filename);
                return;
            } else {
                current = address;
                inner = 0;
                if (!llg_memory_address_offset(address, dims[0], dims[1], &(uint64_t){0}) ||
                    !llg_memory_in_requested_range(address, first, last)) {
                    llg_memory_warning(filename,
                        "address jump is outside the destination memory or selected range; load terminated");
                    sv4_destroy(&token.value);
                    fclose(stream);
                    free(filename);
                    return;
                }
            }
            continue;
        }
        if (!llg_memory_in_requested_range(current, first, last)) {
            if (!warned_extra) {
                llg_memory_warning(filename, "memory file contains more words than the selected range");
                warned_extra = 1;
            }
        } else {
            uint64_t index;
            if (inner >= inner_total ||
                !llg_memory_view_index(current, inner, dims, n_dims, strides,
                                       origin, view_total, total, &index)) {
                if (!warned_extra) {
                    llg_memory_warning(filename, "selected address is outside the destination memory");
                    warned_extra = 1;
                }
            } else {
                // Normalize two-state data before a narrowing conversion so a
                // low X/Z digit cannot hide a known out-of-range enum high bit.
                // Four-state enum membership remains an exact state comparison.
                int had_unknown = sv4_is_unknown(token.value);
                if (two_state && had_unknown) {
                    sv4_replace(&token.value, sv4_to_two_state(token.value));
                }
                if (enum_count != 0 &&
                    !llg_memory_enum_word_fits_width(token.value, elem_width,
                                                     elem_signed)) {
                    llg_memory_warning(filename,
                        "numeric memory data does not fit the enum base type; load terminated");
                    sv4_destroy(&token.value);
                    fclose(stream);
                    free(filename);
                    return;
                }
                sv4_t converted = llg_memory_word_cast(token.value, elem_width, elem_signed);
                if (two_state && had_unknown) {
                    if (!warned_unknown) {
                        llg_memory_warning(filename, "X/Z memory data converted to a two-state element");
                        warned_unknown = 1;
                    }
                }
                if (!llg_memory_enum_value_allowed(converted, enum_values, enum_count)) {
                    llg_memory_warning(filename,
                        "memory data value is not a member of the enum; load terminated");
                    sv4_destroy(&converted);
                    sv4_destroy(&token.value);
                    fclose(stream);
                    free(filename);
                    return;
                }
                llg_ba(&memory[index], converted);
                sv4_destroy(&converted);
                written++;
            }
        }
        if (inner + 1u >= inner_total) {
            inner = 0;
            if (current == last) {
                current = step > 0 ? INT64_MAX : INT64_MIN;
            } else if ((step > 0 && current < INT64_MAX) ||
                       (step < 0 && current > INT64_MIN)) {
                current += step;
            }
        } else {
            ++inner;
        }
    }
    sv4_destroy(&token.value);
    if (written < expected &&
        (!saw_address || addressing_policy == LLG_MEMORY_ADDRESSING_VERILOG_2001)) {
        llg_memory_warning(filename, "memory file contains too few words for the selected range");
    } else if (written > expected && !warned_extra &&
               addressing_policy == LLG_MEMORY_ADDRESSING_VERILOG_2001) {
        llg_memory_warning(filename, "memory file contains more words than the selected range");
    }
    if (ferror(stream)) llg_memory_warning(filename, "read failed");
    fclose(stream);
    free(filename);
}

void llg_memory_write_view(llg_string_t path, sv4_t* memory, uint64_t total,
                           uint32_t elem_width, int8_t elem_signed, int8_t two_state,
                           const int32_t* dims, int n_dims,
                           const uint64_t* strides, uint64_t origin,
                           uint64_t view_total, sv4_t start, sv4_t finish,
                           int has_start, int has_finish, int addressing_policy,
                           const sv4_t* enum_values, uint32_t enum_count, int radix) {
    (void)two_state;
    (void)enum_values;
    (void)enum_count;
    char* requested = llg_memory_path_copy(path);
    char* filename = llg_output_path(requested);
    free(requested);
    FILE* stream = fopen(filename, "w");
    if (!stream) {
        llg_memory_warning(filename, "open for writing failed: %s", strerror(errno));
        free(filename);
        return;
    }
    int64_t first, last;
    uint64_t inner_total;
    if (!llg_memory_bounds(filename, total, dims, n_dims, strides, origin,
                           view_total, start, finish, has_start, has_finish,
                           addressing_policy, &first, &last, &inner_total)) {
        fclose(stream);
        free(filename);
        return;
    }
    size_t capacity = (size_t)elem_width + 2u;
    char* digits = (char*)llg_checked_malloc(capacity, 1, "memory file word");
    int64_t current = first;
    int64_t step = first <= last ? 1 : -1;
    int warned_extra = 0;
    uint64_t inner = 0;
    for (;;) {
        uint64_t index;
        if (!llg_memory_view_index(current, inner, dims, n_dims, strides,
                                   origin, view_total, total, &index)) {
            if (!warned_extra) {
                llg_memory_warning(filename, "selected address is outside the source memory");
                warned_extra = 1;
            }
        } else {
            sv4_format(radix == 2 ? 'b' : 'h', memory[index], digits, capacity);
            if (fputs(digits, stream) == EOF || fputc('\n', stream) == EOF) {
                llg_memory_warning(filename, "write failed");
                break;
            }
        }
        if (inner + 1u >= inner_total) {
            inner = 0;
            if (current == last) break;
            if ((step > 0 && current == INT64_MAX) ||
                (step < 0 && current == INT64_MIN)) break;
            current += step;
        } else {
            ++inner;
        }
    }
    free(digits);
    if (fclose(stream) != 0) llg_memory_warning(filename, "close after writing failed");
    (void)elem_width;
    (void)elem_signed;
    free(filename);
}

void llg_memory_read(llg_string_t path, sv4_t* memory, uint64_t total,
                     uint32_t elem_width, int8_t elem_signed, int8_t two_state,
                     const int32_t* dims, int n_dims, sv4_t start, sv4_t finish,
                     int has_start, int has_finish, int addressing_policy,
                     const sv4_t* enum_values, uint32_t enum_count, int radix) {
    uint64_t stride = 1;
    llg_memory_read_view(path, memory, total, elem_width, elem_signed, two_state,
                         dims, n_dims, n_dims == 1 ? &stride : NULL, 0, total,
                         start, finish, has_start, has_finish, addressing_policy,
                         enum_values, enum_count, radix);
}

void llg_memory_write(llg_string_t path, sv4_t* memory, uint64_t total,
                      uint32_t elem_width, int8_t elem_signed, int8_t two_state,
                      const int32_t* dims, int n_dims, sv4_t start, sv4_t finish,
                      int has_start, int has_finish, int addressing_policy,
                      const sv4_t* enum_values, uint32_t enum_count, int radix) {
    uint64_t stride = 1;
    llg_memory_write_view(path, memory, total, elem_width, elem_signed, two_state,
                          dims, n_dims, n_dims == 1 ? &stride : NULL, 0, total,
                          start, finish, has_start, has_finish, addressing_policy,
                          enum_values, enum_count, radix);
}

static void llg_file_cleanup(void) {
    if (!llg_files_initialized) return;
    for (unsigned i = 0; i < LLG_FILE_SLOTS; i++) {
        if (llg_file_slots[i].owned && llg_file_slots[i].open && llg_file_slots[i].stream)
            fclose(llg_file_slots[i].stream);
    }
    memset(llg_file_slots, 0, sizeof(llg_file_slots));
    llg_files_initialized = 0;
    llg_file_global_error = 0;
    llg_file_global_message[0] = 0;
}
