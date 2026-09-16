
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
        int state;
        unsigned numeric;
        while (c != EOF) {
            if (llg_memory_digit(c, 16, &state, &numeric) && state == 0) {
                llg_memory_append_digit(value, 4u, 0, numeric);
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
        if (value->digits == 0 || value->too_wide) return LLG_MEMORY_TOKEN_ERROR;
        sv4_replace(&value->value, sv4_resize(value->value,
                    (uint32_t)(value->digits * 4u), 0));
        return LLG_MEMORY_TOKEN_ADDRESS;
    }
    if (!llg_memory_digit(c, radix, &(int){0}, &(unsigned){0})) {
        llg_memory_consume_bad_token(stream);
        return LLG_MEMORY_TOKEN_ERROR;
    }
    return llg_memory_parse_digits(stream, radix, c, value);
}

static int llg_memory_index(int64_t address, const int32_t* dims,
                            uint64_t total, uint64_t* index) {
    int64_t left = dims[0];
    int64_t right = dims[1];
    if (address < (left < right ? left : right) ||
        address > (left > right ? left : right)) return 0;
    uint64_t offset = left >= right ? (uint64_t)(left - address)
                                    : (uint64_t)(address - left);
    if (offset >= total) return 0;
    *index = offset;
    return 1;
}

static uint64_t llg_memory_range_length(int64_t first, int64_t last) {
    uint64_t distance = first >= last ? (uint64_t)first - (uint64_t)last
                                      : (uint64_t)last - (uint64_t)first;
    return distance == UINT64_MAX ? UINT64_MAX : distance + 1u;
}

static int llg_memory_bounds(const char* path, uint64_t total,
                             const int32_t* dims, int n_dims,
                             sv4_t start, sv4_t finish, int has_start,
                             int has_finish, int64_t* first, int64_t* last) {
    if (!dims || n_dims != 1 || total == 0) {
        llg_memory_warning(path, "memory descriptor is invalid");
        return 0;
    }
    int64_t left = dims[0], right = dims[1];
    uint64_t extent = left >= right ? (uint64_t)(left - right) + 1u
                                    : (uint64_t)(right - left) + 1u;
    if (extent != total) {
        llg_memory_warning(path, "memory descriptor size does not match its bounds");
        return 0;
    }
    if (has_start && !sv4_to_index_i64(start, first)) {
        llg_memory_warning(path, "start address is unknown, negative-width, or out of range");
        return 0;
    }
    if (has_finish && !sv4_to_index_i64(finish, last)) {
        llg_memory_warning(path, "finish address is unknown, negative-width, or out of range");
        return 0;
    }
    if (!has_start) *first = left;
    if (!has_finish) *last = right;
    uint64_t ignored_index;
    if (!llg_memory_index(*first, dims, total, &ignored_index) ||
        !llg_memory_index(*last, dims, total, &ignored_index)) {
        llg_memory_warning(path, "selected range includes an address outside the destination memory");
    }
    return 1;
}

static int llg_memory_in_requested_range(int64_t address, int64_t first,
                                         int64_t last) {
    return first <= last ? address >= first && address <= last
                         : address <= first && address >= last;
}

void llg_memory_read(llg_string_t path, sv4_t* memory, uint64_t total,
                     uint32_t elem_width, int8_t elem_signed, int8_t two_state,
                     const int32_t* dims, int n_dims, sv4_t start, sv4_t finish,
                     int has_start, int has_finish, int radix) {
    char* filename = llg_memory_path_copy(path);
    FILE* stream = fopen(filename, "r");
    if (!stream) {
        llg_memory_warning(filename, "open for reading failed: %s", strerror(errno));
        free(filename);
        return;
    }
    int64_t first, last;
    if (!llg_memory_bounds(filename, total, dims, n_dims, start, finish,
                           has_start, has_finish, &first, &last)) {
        fclose(stream);
        free(filename);
        return;
    }
    uint64_t expected = llg_memory_range_length(first, last);
    uint64_t written = 0;
    int64_t current = first;
    int64_t step = first <= last ? 1 : -1;
    int warned_extra = 0;
    int warned_unknown = 0;
    llg_memory_value_t token = {0};
    for (;;) {
        int kind = llg_memory_next_token(stream, radix, &token);
        if (kind == LLG_MEMORY_TOKEN_EOF) break;
        if (kind == LLG_MEMORY_TOKEN_ERROR) {
            llg_memory_warning(filename, "malformed or over-width memory token");
            continue;
        }
        if (kind == LLG_MEMORY_TOKEN_ADDRESS) {
            int64_t address;
            if (!sv4_to_index_i64(token.value, &address)) {
                llg_memory_warning(filename, "address jump is not a known non-negative index");
            } else {
                current = address;
                if (!llg_memory_index(address, dims, total, &(uint64_t){0}) && !warned_extra) {
                    llg_memory_warning(filename, "address jump is outside the destination memory");
                    warned_extra = 1;
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
            if (!llg_memory_index(current, dims, total, &index)) {
                if (!warned_extra) {
                    llg_memory_warning(filename, "selected address is outside the destination memory");
                    warned_extra = 1;
                }
            } else {
                sv4_t converted = sv4_cast(token.value, elem_width, elem_signed);
                if (two_state && sv4_is_unknown(token.value)) {
                    if (!warned_unknown) {
                        llg_memory_warning(filename, "X/Z memory data converted to a two-state element");
                        warned_unknown = 1;
                    }
                    sv4_replace(&converted, sv4_to_two_state(converted));
                }
                llg_ba(&memory[index], converted);
                sv4_destroy(&converted);
                written++;
            }
        }
        if (current == last) {
            current = step > 0 ? INT64_MAX : INT64_MIN;
        } else if ((step > 0 && current < INT64_MAX) ||
                   (step < 0 && current > INT64_MIN)) {
            current += step;
        }
    }
    sv4_destroy(&token.value);
    if (written < expected) {
        llg_memory_warning(filename, "memory file contains too few words for the selected range");
    }
    if (ferror(stream)) llg_memory_warning(filename, "read failed");
    fclose(stream);
    free(filename);
}

void llg_memory_write(llg_string_t path, sv4_t* memory, uint64_t total,
                      uint32_t elem_width, int8_t elem_signed, int8_t two_state,
                      const int32_t* dims, int n_dims, sv4_t start, sv4_t finish,
                      int has_start, int has_finish, int radix) {
    (void)two_state;
    char* filename = llg_memory_path_copy(path);
    FILE* stream = fopen(filename, "w");
    if (!stream) {
        llg_memory_warning(filename, "open for writing failed: %s", strerror(errno));
        free(filename);
        return;
    }
    int64_t first, last;
    if (!llg_memory_bounds(filename, total, dims, n_dims, start, finish,
                           has_start, has_finish, &first, &last)) {
        fclose(stream);
        free(filename);
        return;
    }
    size_t capacity = (size_t)elem_width + 2u;
    char* digits = (char*)llg_checked_malloc(capacity, 1, "memory file word");
    int64_t current = first;
    int64_t step = first <= last ? 1 : -1;
    int warned_extra = 0;
    for (;;) {
        uint64_t index;
        if (!llg_memory_index(current, dims, total, &index)) {
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
        if (current == last) break;
        if ((step > 0 && current == INT64_MAX) ||
            (step < 0 && current == INT64_MIN)) break;
        current += step;
    }
    free(digits);
    if (fclose(stream) != 0) llg_memory_warning(filename, "close after writing failed");
    (void)elem_width;
    (void)elem_signed;
    free(filename);
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
