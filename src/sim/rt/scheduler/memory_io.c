
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

// The words of a load come from a file or, for `$sreadmemb/$sreadmemh`
// (IEEE 1800-2009 D.14), from text in the model. Both reach the one parser
// through this stream, so values, `@` addresses, comments, X/Z and range
// rules cannot differ between them.
typedef struct {
    FILE* file;
    const char* text;
    size_t length;
    size_t offset;
} llg_memory_stream_t;

// The text the generated code supplies for the load it is about to call,
// which `llg_memory_open_read` serves instead of opening the file. The load
// runs to completion within the calling process, so one slot is enough.
static struct {
    const char* data;
    size_t length;
    int active;
} llg_memory_text;

void llg_memory_text_begin(const char* data, size_t length) {
    llg_memory_text.data = data;
    llg_memory_text.length = length;
    llg_memory_text.active = 1;
}

void llg_memory_text_end(void) {
    memset(&llg_memory_text, 0, sizeof(llg_memory_text));
}

static int llg_memory_getc(llg_memory_stream_t* stream) {
    if (stream->file) return fgetc(stream->file);
    return stream->offset < stream->length ? (unsigned char)stream->text[stream->offset++] : EOF;
}

// Pushes back one character that `llg_memory_getc` just returned.
static void llg_memory_ungetc(int c, llg_memory_stream_t* stream) {
    if (stream->file) {
        (void)ungetc(c, stream->file);
    } else if (stream->offset != 0) {
        --stream->offset;
    }
}

static void llg_memory_close(llg_memory_stream_t* stream) {
    if (stream->file) fclose(stream->file);
}

static int llg_memory_stream_failed(const llg_memory_stream_t* stream) {
    return stream->file && ferror(stream->file);
}

// What the diagnostics call the source: the label names the file, or the
// `$sreadmem*` call.
static const char* llg_memory_noun(void) {
    return llg_memory_text.active ? "memory string" : "memory file";
}

// Diagnostics carry the category IEEE 1800-2009 21.4/21.4.2 and IEEE
// 1364-2001 17.2.8 prescribe: a word count that differs from the range is a
// warning; an out-of-range file address or enum value is an error that
// terminates the load. Neither ends the simulation or changes its status.
static void llg_memory_report(const char* category, const char* path,
                              const char* format, va_list args) {
    llg_report_t report;
    llg_report_init(&report);
    llg_report_printf(&report, "llg: %s: %s `%s`: ", category, llg_memory_noun(),
                      path ? path : "");
    llg_report_vprintf(&report, format, args);
    llg_report_bytes(&report, "\n", 1);
    llg_report_emit(&report, stderr);
}

static void llg_memory_warning(const char* path, const char* format, ...) {
    va_list args;
    va_start(args, format);
    llg_memory_report("warning", path, format, args);
    va_end(args);
}

static void llg_memory_error(const char* path, const char* format, ...) {
    va_list args;
    va_start(args, format);
    llg_memory_report("error", path, format, args);
    va_end(args);
}

// Opens the source of a load; returns 0, with the warning reported, when a
// file cannot be opened. Text sources always open.
static int llg_memory_open_read(const char* label, llg_memory_stream_t* stream) {
    memset(stream, 0, sizeof(*stream));
    if (llg_memory_text.active) {
        stream->text = llg_memory_text.data;
        stream->length = llg_memory_text.length;
        return 1;
    }
    stream->file = fopen(label, "r");
    if (!stream->file) {
        llg_memory_warning(label, "open for reading failed: %s", strerror(errno));
        return 0;
    }
    return 1;
}

static char* llg_memory_path_copy(llg_string_t path) {
    char* copy = (char*)llg_checked_malloc(path.len + 1u, 1, "memory file path");
    if (path.len && path.data) memcpy(copy, path.data, path.len);
    copy[path.len] = 0;
    llg_string_destroy(&path);
    return copy;
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
    if (required > llg_sv4_width(value->value)) {
        uint32_t capacity = llg_sv4_width(value->value) ? llg_sv4_width(value->value) * 2u : 64u;
        if (capacity >= LLG_SUPPORTED_WIDTH_LIMIT) capacity = LLG_SUPPORTED_WIDTH_LIMIT - 1u;
        sv4_replace(&value->value, sv4_resize(value->value, capacity, 0));
    }
    llg_sv4_append_digit(&value->value, bits, state == 1 ? 2u : state == 2 ? 3u : 0u, numeric);
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

static int llg_memory_next_noncomment(llg_memory_stream_t* stream) {
    for (;;) {
        int c = llg_memory_getc(stream);
        if (c == EOF || !isspace((unsigned char)c)) {
            if (c != '/') return c;
            int next = llg_memory_getc(stream);
            if (next == '/') {
                while ((c = llg_memory_getc(stream)) != EOF && c != '\n') {}
                continue;
            }
            if (next == '*') {
                int previous = 0;
                int closed = 0;
                while ((c = llg_memory_getc(stream)) != EOF) {
                    if (previous == '*' && c == '/') {
                        closed = 1;
                        break;
                    }
                    previous = c;
                }
                if (!closed) return -2;
                continue;
            }
            if (next != EOF) llg_memory_ungetc(next, stream);
            return '/';
        }
    }
}

static void llg_memory_consume_bad_token(llg_memory_stream_t* stream) {
    int c;
    while ((c = llg_memory_getc(stream)) != EOF) {
        if (isspace((unsigned char)c)) break;
        if (c == '/') {
            llg_memory_ungetc(c, stream);
            break;
        }
    }
}

static int llg_memory_parse_digits(llg_memory_stream_t* stream, int radix, int first,
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
            llg_memory_ungetc(c, stream);
            break;
        } else {
            malformed = 1;
            llg_memory_consume_bad_token(stream);
            break;
        }
        c = llg_memory_getc(stream);
    }
    if (!saw_digit || malformed || value->too_wide) return LLG_MEMORY_TOKEN_ERROR;
    sv4_replace(&value->value, sv4_resize(value->value,
                (uint32_t)(value->digits * bits), 0));
    return LLG_MEMORY_TOKEN_DATA;
}

// With `wide_address`, an `@` address keeps all its hexadecimal digits in
// `value->value` (an unsigned number without sign or X/Z) instead of a host
// index, for associative-array keys of any width.
static int llg_memory_next_token(llg_memory_stream_t* stream, int radix, int wide_address,
                                 llg_memory_value_t* value) {
    sv4_destroy(&value->value);
    memset(value, 0, sizeof(*value));
    int c = llg_memory_next_noncomment(stream);
    if (c == EOF) return LLG_MEMORY_TOKEN_EOF;
    if (c == -2) return LLG_MEMORY_TOKEN_ERROR;
    if (c == '@' && wide_address) {
        c = llg_memory_getc(stream);
        int state = 0;
        if (!llg_memory_digit(c, 16, &state, &(unsigned){0}) || state != 0) {
            if (c != EOF && !isspace((unsigned char)c)) llg_memory_consume_bad_token(stream);
            return LLG_MEMORY_TOKEN_ERROR;
        }
        if (llg_memory_parse_digits(stream, 16, c, value) != LLG_MEMORY_TOKEN_DATA ||
            sv4_is_unknown(value->value)) {
            return LLG_MEMORY_TOKEN_ERROR;
        }
        return LLG_MEMORY_TOKEN_ADDRESS;
    }
    if (c == '@') {
        c = llg_memory_getc(stream);
        int negative = 0;
        uint64_t magnitude = 0;
        int saw_digit = 0;
        if (c == '-' || c == '+') {
            negative = c == '-';
            c = llg_memory_getc(stream);
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
                llg_memory_ungetc(c, stream);
                break;
            } else {
                llg_memory_consume_bad_token(stream);
                return LLG_MEMORY_TOKEN_ERROR;
            }
            c = llg_memory_getc(stream);
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
    if (origin == UINT64_MAX) {
        // The generated call marks a runtime index or slice position that is
        // unknown or outside its dimension (IEEE 1800-2009 7.4.6).
        llg_memory_error(path, "selected index or slice is outside the memory");
        return 0;
    }
    if (!dims || !strides || n_dims <= 0 || total == 0 || view_total == 0 ||
        origin >= total || view_total > total - origin) {
        llg_memory_error(path, "memory descriptor is invalid");
        return 0;
    }
    uint64_t expected_total = 1;
    for (int dimension = 0; dimension < n_dims; ++dimension) {
        uint64_t extent;
        if (!llg_memory_extent(dims[2 * dimension], dims[2 * dimension + 1], &extent) ||
            expected_total > UINT64_MAX / extent) {
            llg_memory_error(path, "memory descriptor dimension overflows");
            return 0;
        }
        expected_total *= extent;
    }
    if (expected_total != view_total) {
        llg_memory_error(path, "memory descriptor size does not match its bounds");
        return 0;
    }
    uint64_t expected_stride = 1;
    for (int dimension = n_dims - 1; dimension >= 0; --dimension) {
        uint64_t extent;
        if (!llg_memory_extent(dims[2 * dimension], dims[2 * dimension + 1], &extent) ||
            strides[dimension] != expected_stride ||
            expected_stride > UINT64_MAX / extent) {
            llg_memory_error(path, "memory descriptor stride is invalid");
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
            llg_memory_error(path, "memory descriptor range overflows");
            return 0;
        }
        max_offset += (extent - 1u) * strides[dimension];
    }
    if (max_offset >= total - origin) {
        llg_memory_error(path, "memory descriptor exceeds its source array");
        return 0;
    }
    uint64_t outer_extent;
    if (!llg_memory_extent(dims[0], dims[1], &outer_extent) ||
        outer_extent == 0 || view_total % outer_extent != 0) {
        llg_memory_error(path, "memory descriptor outer extent is invalid");
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
                             int reject_outside, int64_t* first, int64_t* last,
                             uint64_t* inner_total) {
    if (!llg_memory_descriptor(path, total, dims, n_dims, strides, origin,
                               view_total, inner_total)) {
        return 0;
    }
    int32_t left_value = dims[0], right_value = dims[1];
    int64_t left = left_value, right = right_value;
    if (has_start && !sv4_to_index_i64(start, first)) {
        llg_memory_error(path, "start address is unknown or outside the index range");
        return 0;
    }
    if (has_finish && !sv4_to_index_i64(finish, last)) {
        llg_memory_error(path, "finish address is unknown or outside the index range");
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
    // A load checks each word's address as it reaches it (S27-D7). A dump
    // rejects a range outside the memory before it opens the file, and so
    // does a load that would create an associative element (S27-D23).
    if (!reject_outside) return 1;
    uint64_t ignored_offset;
    if (!llg_memory_address_offset(*first, left_value, right_value, &ignored_offset) ||
        !llg_memory_address_offset(*last, left_value, right_value, &ignored_offset)) {
        llg_memory_error(path, "selected range includes an address outside the memory");
        return 0;
    }
    return 1;
}

static int llg_memory_in_requested_range(int64_t address, int64_t first,
                                         int64_t last) {
    return first <= last ? address >= first && address <= last
                         : address <= first && address >= last;
}

// Memory words are unsigned based digits. Only a leading X/Z digit pads with
// its state; a known leading one still zero-extends, even into signed storage.
static sv4_t llg_memory_word_cast(sv4_t word, uint32_t width, int8_t is_signed) {
    int8_t extend_unknown = 0;
    if (llg_sv4_width(word) != 0 && width > llg_sv4_width(word)) {
        uint32_t bit = llg_sv4_width(word) - 1u;
        extend_unknown = llg_sv4_state(word, bit) >= 2u;
    }
    sv4_t result = sv4_resize(word, width, extend_unknown);
    llg_sv4_set_signed(&result, is_signed);
    return result;
}

// Destination or source cells of one memory file task: dense or descriptor
// model cells, or the packed image of a fixed array without model storage
// (see `llg_memory_image_t`). An image holds `total` cells of the element
// width from bit `image_offset`, cell 0 in the most significant bits.
typedef struct {
    sv4_t* memory;
    llg_fixed_array_t* fixed;
    sv4_t* image;
    uint32_t image_offset;
    // Published image: 0 = no store allowed, 1 = private store, 2 = store and
    // publish once at the end (see `llg_memory_image_begin`).
    int publish;
    int image_mode;
    int image_changed;
    sv4_t image_old_lsb;
    // Rows of a container whose elements are fixed arrays of packed words:
    // cell `i` is item `i % row_cells` of row `i / row_cells`.
    llg_value_t* rows;
    uint64_t row_cells;
    // One element of an associative array of fixed arrays (`aa[key]`) that a
    // load creates, with its default value, once its range is valid
    // (S27-D23); `rows` then points at it.
    llg_assoc_value_t* assoc;
    sv4_t assoc_key;
} llg_memory_storage_t;

static sv4_t* llg_memory_row_cell(const llg_memory_storage_t* storage, uint64_t index) {
    return &storage->rows[index / storage->row_cells]
                .value.items[index % storage->row_cells]
                .value.packed;
}

static uint64_t llg_memory_image_low(const llg_memory_storage_t* storage,
                                     uint64_t index, uint64_t total,
                                     uint32_t elem_width) {
    return (uint64_t)storage->image_offset + (total - 1u - index) * (uint64_t)elem_width;
}

// Decide, as `llg_ba` does for a whole store, whether a published image may
// change: never while it is forced, and in a read-only region only as a
// private store.
static void llg_memory_image_begin(llg_memory_storage_t* storage) {
    if (!storage->image || !storage->publish) return;
    if (llg_is_forced(storage->image)) {
        storage->image_mode = 0;
    } else if (region_is_read_only_now(g.current_region) && !clocking_sample_publication) {
        storage->image_mode = region_private_store("signal write") ? 1 : 0;
    } else {
        storage->image_mode = 2;
    }
}

// Publish a changed image once, after its last word. Like
// `sig_write_ranges`, the edge snapshots are bit 0 before the first change
// and after the last, the only bit directional packed edges consume.
static void llg_memory_image_end(llg_memory_storage_t* storage) {
    if (!storage->image || !storage->publish || !storage->image_changed) return;
    if (storage->image_mode != 2) {
        sv4_destroy(&storage->image_old_lsb);
        return;
    }
    sv4_t* target = storage->image;
    llg_value_scope_t* target_pin = value_target_pin(target);
    llg_value_scope_t* snapshots = llg_value_scope_begin(2);
    sv4_t* edges = llg_value_scope_values(snapshots);
    sv4_replace(&edges[0], storage->image_old_lsb);
    storage->image_old_lsb = (sv4_t)SV4_EMPTY;
    sv4_replace(&edges[1], sv4_bit_select(*target, 0));
    sig_publish_changed(target, edges[0], edges[1]);
    llg_value_scope_end(snapshots);
    if (target_pin) llg_value_scope_end(target_pin);
}

static void llg_memory_store(llg_memory_storage_t* storage, uint64_t index,
                             uint64_t total, uint32_t elem_width, sv4_t value) {
    if (storage->image) {
        uint64_t low = llg_memory_image_low(storage, index, total, elem_width);
        if (storage->publish) {
            if (storage->image_mode == 0 ||
                llg_sv4_range_same(*storage->image, (uint32_t)low, value)) return;
            if (!storage->image_changed) {
                storage->image_old_lsb = sv4_bit_select(*storage->image, 0);
                storage->image_changed = 1;
            }
            llg_sv4_range_copy(storage->image, (uint32_t)low, value);
            return;
        }
        sv4_part_select_set(storage->image, (int64_t)(low + elem_width - 1u),
                            (int64_t)low, value);
        return;
    }
    if (storage->rows) {
        llg_ba(llg_memory_row_cell(storage, index), value);
        return;
    }
    llg_ba(storage->fixed ? llg_fixed_array_cell(storage->fixed, index)
                          : &storage->memory[index],
           value);
}

// Format one cell into `digits`.
static void llg_memory_format_cell(const llg_memory_storage_t* storage,
                                   uint64_t index, uint64_t total,
                                   uint32_t elem_width, int radix,
                                   char* digits, size_t capacity) {
    char conversion = radix == 2 ? 'b' : 'h';
    if (storage->image) {
        uint64_t low = llg_memory_image_low(storage, index, total, elem_width);
        sv4_t cell = sv4_part_select(*storage->image,
                                     (int64_t)(low + elem_width - 1u), (int64_t)low);
        sv4_format(conversion, cell, digits, capacity);
        sv4_destroy(&cell);
        return;
    }
    if (storage->rows) {
        sv4_format(conversion, *llg_memory_row_cell(storage, index), digits, capacity);
        return;
    }
    sv4_format(conversion,
               *(storage->fixed ? llg_fixed_array_peek(storage->fixed, index)
                                : &storage->memory[index]),
               digits, capacity);
}

// Create the associative element a load targets. Returns 0, with the error
// reported, when the element cannot be created.
static int llg_memory_storage_resolve(const char* path, llg_memory_storage_t* storage) {
    if (!storage->assoc || storage->rows) return 1;
    storage->rows = llg_assoc_value_element_integral(storage->assoc, &storage->assoc_key, 1, 1);
    if (!storage->rows) {
        llg_memory_error(path, "selected associative array element cannot be created");
        return 0;
    }
    return 1;
}

// An image must lie within its storage: `total` cells of the element width
// from `image_offset`.
static int llg_memory_storage_valid(const char* path,
                                    const llg_memory_storage_t* storage,
                                    uint64_t total, uint32_t elem_width) {
    if (!storage->image) return 1;
    if (elem_width == 0 || total > UINT32_MAX / elem_width ||
        (uint64_t)llg_sv4_width(*storage->image) < (uint64_t)storage->image_offset ||
        (uint64_t)llg_sv4_width(*storage->image) - storage->image_offset <
            total * (uint64_t)elem_width) {
        llg_memory_error(path, "memory image size does not match its cells");
        return 0;
    }
    return 1;
}

// Convert one file word to an element: it may not have more digits than the
// element holds, two-state data read X/Z as 0 (one warning per load), and the
// word is an unsized unsigned literal at the element width. An enum element
// takes any such word, member or not (S27-D4, industry-standard behaviour).
// Returns 0, with the error reported, when the load must terminate.
static int llg_memory_convert_word(const char* filename, llg_memory_value_t* token,
                                   uint32_t elem_width, int8_t elem_signed,
                                   int8_t two_state, int* warned_unknown,
                                   sv4_t* converted) {
    // A number with a whole digit more than the element holds, leading zeros
    // included, is an error that ends the load (S27-D2, industry-standard
    // behaviour); the unused bits of a partly used leading digit are dropped.
    uint64_t digit_bits = token->digits ? llg_sv4_width(token->value) / token->digits : 0u;
    if (digit_bits && token->digits * digit_bits >= (uint64_t)elem_width + digit_bits) {
        llg_memory_error(filename,
            "memory word has more digits than the element holds; load terminated");
        return 0;
    }
    int had_unknown = sv4_is_unknown(token->value);
    if (two_state && had_unknown) {
        sv4_replace(&token->value, sv4_to_two_state(token->value));
    }
    *converted = llg_memory_word_cast(token->value, elem_width, elem_signed);
    if (two_state && had_unknown && !*warned_unknown) {
        llg_memory_warning(filename, "X/Z memory data converted to a two-state element");
        *warned_unknown = 1;
    }
    return 1;
}

static uint64_t fixed_memory_read_view(llg_string_t path, llg_memory_storage_t* storage, uint64_t total,
                          uint32_t elem_width, int8_t elem_signed, int8_t two_state,
                          const int32_t* dims, int n_dims,
                          const uint64_t* strides, uint64_t origin,
                          uint64_t view_total, sv4_t start, sv4_t finish,
                          int has_start, int has_finish, int addressing_policy,
                          int radix) {
    char* filename = llg_memory_path_copy(path);
    llg_memory_stream_t stream_storage;
    llg_memory_stream_t* stream = &stream_storage;
    if (!llg_memory_open_read(filename, stream)) {
        free(filename);
        return 0;
    }
    int64_t first, last;
    uint64_t inner_total;
    if (!llg_memory_storage_valid(filename, storage, total, elem_width) ||
        !llg_memory_bounds(filename, total, dims, n_dims, strides, origin,
                           view_total, start, finish, has_start, has_finish,
                           addressing_policy, storage->assoc != NULL, &first,
                           &last, &inner_total) ||
        !llg_memory_storage_resolve(filename, storage)) {
        llg_memory_close(stream);
        free(filename);
        return 0;
    }
    // Only the part of the range inside the memory can take words, so the
    // word-count warnings compare against that part.
    int64_t lowest = dims[0] < dims[1] ? dims[0] : dims[1];
    int64_t highest = dims[0] < dims[1] ? dims[1] : dims[0];
    int64_t range_low = first < last ? first : last;
    int64_t range_high = first < last ? last : first;
    if (range_low < lowest) range_low = lowest;
    if (range_high > highest) range_high = highest;
    uint64_t range = range_low > range_high ? 0u : llg_memory_range_length(range_low, range_high);
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
        int kind = llg_memory_next_token(stream, radix, 0, &token);
        if (kind == LLG_MEMORY_TOKEN_EOF) break;
        if (kind == LLG_MEMORY_TOKEN_ERROR) {
            // The text admits only white space, comments, numbers and `@`
            // addresses; the words after a malformed token have no reliable
            // address, so the load stops and keeps the words already placed.
            llg_memory_error(filename, "malformed or over-width memory token; load terminated");
            sv4_destroy(&token.value);
            llg_memory_close(stream);
            free(filename);
            return written;
        }
        if (kind == LLG_MEMORY_TOKEN_ADDRESS) {
            saw_address = 1;
            int64_t address = token.address;
            if (token.invalid_index) {
                llg_memory_error(filename, "address jump is not a known index; load terminated");
                sv4_destroy(&token.value);
                llg_memory_close(stream);
                free(filename);
                return written;
            } else {
                current = address;
                inner = 0;
                if (!llg_memory_address_offset(address, dims[0], dims[1], &(uint64_t){0}) ||
                    !llg_memory_in_requested_range(address, first, last)) {
                    llg_memory_error(filename,
                        "address jump is outside the destination memory or selected range; load terminated");
                    sv4_destroy(&token.value);
                    llg_memory_close(stream);
                    free(filename);
                    return written;
                }
            }
            continue;
        }
        if (!llg_memory_address_offset(current, dims[0], dims[1], &(uint64_t){0})) {
            // A word whose address has no cell ends the load, also past the
            // highest address of a file without a range (S27-D7, S27-D8,
            // industry-standard behaviour).
            llg_memory_error(filename, "memory word address is outside the destination memory; load terminated");
            sv4_destroy(&token.value);
            llg_memory_close(stream);
            free(filename);
            return written;
        }
        if (!llg_memory_in_requested_range(current, first, last)) {
            if (!warned_extra) {
                llg_memory_warning(filename, "%s contains more words than the selected range", llg_memory_noun());
                warned_extra = 1;
            }
        } else {
            uint64_t index;
            if (inner >= inner_total ||
                !llg_memory_view_index(current, inner, dims, n_dims, strides,
                                       origin, view_total, total, &index)) {
                if (!warned_extra) {
                    llg_memory_error(filename, "selected address is outside the destination memory");
                    warned_extra = 1;
                }
            } else {
                sv4_t converted;
                if (!llg_memory_convert_word(filename, &token, elem_width, elem_signed,
                                             two_state, &warned_unknown, &converted)) {
                    sv4_destroy(&token.value);
                    llg_memory_close(stream);
                    free(filename);
                    return written;
                }
                llg_memory_store(storage, index, total, elem_width, converted);
                sv4_destroy(&converted);
                written++;
            }
        }
        if (inner + 1u >= inner_total) {
            // `current` is inside the memory's 32-bit bounds here, so the
            // step cannot overflow.
            inner = 0;
            current += step;
        } else {
            ++inner;
        }
    }
    sv4_destroy(&token.value);
    if (written < expected &&
        (!saw_address || addressing_policy == LLG_MEMORY_ADDRESSING_VERILOG_2001)) {
        llg_memory_warning(filename, "%s contains too few words for the selected range", llg_memory_noun());
    } else if (written > expected && !warned_extra &&
               addressing_policy == LLG_MEMORY_ADDRESSING_VERILOG_2001) {
        llg_memory_warning(filename, "%s contains more words than the selected range", llg_memory_noun());
    }
    if (llg_memory_stream_failed(stream)) llg_memory_error(filename, "read failed");
    llg_memory_close(stream);
    free(filename);
    return written;
}

static void fixed_memory_write_view(llg_string_t path, const llg_memory_storage_t* storage, uint64_t total,
                           uint32_t elem_width, int8_t elem_signed, int8_t two_state,
                           const int32_t* dims, int n_dims,
                           const uint64_t* strides, uint64_t origin,
                           uint64_t view_total, sv4_t start, sv4_t finish,
                           int has_start, int has_finish, int addressing_policy,
                           int radix) {
    (void)two_state;
    char* requested = llg_memory_path_copy(path);
    char* filename = llg_output_path(requested);
    free(requested);
    // Check the range before opening, so a rejected call leaves an existing
    // file untouched rather than truncating it.
    int64_t first, last;
    uint64_t inner_total;
    if (!llg_memory_storage_valid(filename, storage, total, elem_width) ||
        !llg_memory_bounds(filename, total, dims, n_dims, strides, origin,
                           view_total, start, finish, has_start, has_finish,
                           addressing_policy, 1, &first, &last, &inner_total)) {
        free(filename);
        return;
    }
    FILE* stream = fopen(filename, "w");
    if (!stream) {
        llg_memory_warning(filename, "open for writing failed: %s", strerror(errno));
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
                llg_memory_error(filename, "selected address is outside the source memory");
                warned_extra = 1;
            }
        } else {
            llg_memory_format_cell(storage, index, total, elem_width, radix,
                                   digits, capacity);
            if (fputs(digits, stream) == EOF || fputc('\n', stream) == EOF) {
                llg_memory_error(filename, "write failed");
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
    if (fclose(stream) != 0) llg_memory_error(filename, "close after writing failed");
    (void)elem_signed;
    free(filename);
}

void llg_memory_read(llg_string_t path, sv4_t* memory, uint64_t total,
                     uint32_t elem_width, int8_t elem_signed, int8_t two_state,
                     const int32_t* dims, int n_dims, sv4_t start, sv4_t finish,
                     int has_start, int has_finish, int addressing_policy,
                     int radix) {
    uint64_t stride = 1;
    llg_memory_read_view(path, memory, total, elem_width, elem_signed, two_state,
                         dims, n_dims, n_dims == 1 ? &stride : NULL, 0, total,
                         start, finish, has_start, has_finish, addressing_policy,
                         radix);
}

void llg_memory_write(llg_string_t path, sv4_t* memory, uint64_t total,
                      uint32_t elem_width, int8_t elem_signed, int8_t two_state,
                      const int32_t* dims, int n_dims, sv4_t start, sv4_t finish,
                      int has_start, int has_finish, int addressing_policy,
                      int radix) {
    uint64_t stride = 1;
    llg_memory_write_view(path, memory, total, elem_width, elem_signed, two_state,
                          dims, n_dims, n_dims == 1 ? &stride : NULL, 0, total,
                          start, finish, has_start, has_finish, addressing_policy,
                          radix);
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

void llg_memory_read_view(llg_string_t path, sv4_t* memory, uint64_t total,
                          uint32_t elem_width, int8_t elem_signed, int8_t two_state,
                          const int32_t* dims, int n_dims,
                          const uint64_t* strides, uint64_t origin,
                          uint64_t view_total, sv4_t start, sv4_t finish,
                          int has_start, int has_finish, int addressing_policy,
                          int radix) {
    llg_memory_storage_t storage = {0};
    storage.memory = memory;
    fixed_memory_read_view(path, &storage, total, elem_width, elem_signed, two_state, dims, n_dims, strides, origin, view_total, start, finish, has_start, has_finish, addressing_policy, radix);
}
void llg_fixed_memory_read_view(llg_string_t path, llg_fixed_array_t* memory, uint64_t total,
                          uint32_t elem_width, int8_t elem_signed, int8_t two_state,
                          const int32_t* dims, int n_dims,
                          const uint64_t* strides, uint64_t origin,
                          uint64_t view_total, sv4_t start, sv4_t finish,
                          int has_start, int has_finish, int addressing_policy,
                          int radix) {
    llg_memory_storage_t storage = {0};
    storage.fixed = memory;
    fixed_memory_read_view(path, &storage, total, elem_width, elem_signed, two_state, dims, n_dims, strides, origin, view_total, start, finish, has_start, has_finish, addressing_policy, radix);
}
void llg_image_memory_read_view(llg_string_t path, const llg_memory_image_t* memory,
                          uint64_t total,
                          uint32_t elem_width, int8_t elem_signed, int8_t two_state,
                          const int32_t* dims, int n_dims,
                          const uint64_t* strides, uint64_t origin,
                          uint64_t view_total, sv4_t start, sv4_t finish,
                          int has_start, int has_finish, int addressing_policy,
                          int radix) {
    llg_memory_storage_t storage = {0};
    storage.image = memory->storage;
    storage.image_offset = memory->offset;
    storage.publish = memory->publish != 0;
    llg_memory_image_begin(&storage);
    fixed_memory_read_view(path, &storage, total, elem_width, elem_signed, two_state, dims, n_dims, strides, origin, view_total, start, finish, has_start, has_finish, addressing_policy, radix);
    llg_memory_image_end(&storage);
}

void llg_memory_write_view(llg_string_t path, sv4_t* memory, uint64_t total,
                          uint32_t elem_width, int8_t elem_signed, int8_t two_state,
                          const int32_t* dims, int n_dims,
                          const uint64_t* strides, uint64_t origin,
                          uint64_t view_total, sv4_t start, sv4_t finish,
                          int has_start, int has_finish, int addressing_policy,
                          int radix) {
    llg_memory_storage_t storage = {0};
    storage.memory = memory;
    fixed_memory_write_view(path, &storage, total, elem_width, elem_signed, two_state, dims, n_dims, strides, origin, view_total, start, finish, has_start, has_finish, addressing_policy, radix);
}
void llg_fixed_memory_write_view(llg_string_t path, llg_fixed_array_t* memory, uint64_t total,
                          uint32_t elem_width, int8_t elem_signed, int8_t two_state,
                          const int32_t* dims, int n_dims,
                          const uint64_t* strides, uint64_t origin,
                          uint64_t view_total, sv4_t start, sv4_t finish,
                          int has_start, int has_finish, int addressing_policy,
                          int radix) {
    llg_memory_storage_t storage = {0};
    storage.fixed = memory;
    fixed_memory_write_view(path, &storage, total, elem_width, elem_signed, two_state, dims, n_dims, strides, origin, view_total, start, finish, has_start, has_finish, addressing_policy, radix);
}
void llg_image_memory_write_view(llg_string_t path, const llg_memory_image_t* memory,
                          uint64_t total,
                          uint32_t elem_width, int8_t elem_signed, int8_t two_state,
                          const int32_t* dims, int n_dims,
                          const uint64_t* strides, uint64_t origin,
                          uint64_t view_total, sv4_t start, sv4_t finish,
                          int has_start, int has_finish, int addressing_policy,
                          int radix) {
    llg_memory_storage_t storage = {0};
    storage.image = memory->storage;
    storage.image_offset = memory->offset;
    fixed_memory_write_view(path, &storage, total, elem_width, elem_signed, two_state, dims, n_dims, strides, origin, view_total, start, finish, has_start, has_finish, addressing_policy, radix);
}

// Whether a runtime-sized container of `size` elements can be addressed;
// otherwise report it and return 0. An empty array loads nothing with a
// warning and dumps as an empty file (decision S27-D10).
static int llg_container_memory_addressable(int write, llg_string_t path, size_t size,
                                            int has_start, int has_finish) {
    if (size != 0 && size - 1u <= (size_t)INT32_MAX) return 1;
    char* filename = llg_memory_path_copy(path);
    if (size != 0) {
        llg_memory_error(filename, "memory has too many elements to address");
    } else if (write && !has_start && !has_finish) {
        // An empty array dumps as an empty file.
        char* output = llg_output_path(filename);
        FILE* stream = fopen(output, "w");
        if (!stream || fclose(stream) != 0)
            llg_memory_warning(output, "open for writing failed: %s", strerror(errno));
        free(output);
    } else if (!write && !has_start && !has_finish) {
        llg_memory_warning(filename, "destination memory has no elements; nothing loaded");
    } else {
        llg_memory_error(filename, "selected range includes an address outside the memory");
    }
    free(filename);
    return 0;
}

// A slice `[dims[0]:dims[1]]` of a runtime-sized array must hold ascending
// indices within `[0:size-1]` of the current size (decision S27-D14);
// reversed queue bounds select no element (S27-D20). Otherwise report it and
// return 0.
static int llg_container_memory_slice_valid(llg_string_t path, const int32_t* dims,
                                            size_t size) {
    const char* problem = NULL;
    if (dims[0] >= 0 && dims[0] > dims[1]) {
        problem = "slice left bound exceeds its right bound; the slice selects no element";
    } else if (dims[0] < 0 || dims[0] > dims[1] || (uint64_t)dims[1] >= (uint64_t)size) {
        problem = "slice is outside the current size of the array";
    }
    if (!problem) return 1;
    char* filename = llg_memory_path_copy(path);
    llg_memory_error(filename, "%s", problem);
    free(filename);
    return 0;
}

// Dynamic arrays, queues and fixed-size class properties keep their elements
// as dense cells in declaration order. A runtime-sized view (`n_dims` 0)
// addresses `[0:size-1]` of the current size, which a load never changes
// (IEEE 1800-2009 21.4.1); a slice of one (`total` 0, one ascending dimension)
// must lie within that size; a fixed-size view must match the size. A load that
// placed any word sends one contents notification, like `$fread`.
static void llg_container_memory_view(int write, llg_string_t path, sv4_t* data,
                                      size_t size, llg_container_notify_fn notify,
                                      sv4_t* contents, sv4_t* shape, uint64_t total,
                                      uint32_t elem_width, int8_t elem_signed,
                                      int8_t two_state, const int32_t* dims, int n_dims,
                                      const uint64_t* strides, uint64_t origin,
                                      uint64_t view_total, sv4_t start, sv4_t finish,
                                      int has_start, int has_finish, int addressing_policy,
                                      int radix) {
    int32_t runtime_dims[2];
    uint64_t runtime_stride = 1;
    if (n_dims == 0) {
        if (!llg_container_memory_addressable(write, path, size, has_start, has_finish))
            return;
        runtime_dims[0] = 0;
        runtime_dims[1] = (int32_t)(size - 1u);
        dims = runtime_dims;
        strides = &runtime_stride;
        n_dims = 1;
        origin = 0;
        total = (uint64_t)size;
        view_total = total;
    } else if (total == 0 && n_dims == 1) {
        if (!llg_container_memory_slice_valid(path, dims, size)) return;
        total = (uint64_t)size;
    } else if (total != (uint64_t)size) {
        char* filename = llg_memory_path_copy(path);
        llg_memory_error(filename, "memory descriptor size does not match its container");
        free(filename);
        return;
    }
    llg_memory_storage_t storage = {0};
    storage.memory = data;
    if (write) {
        fixed_memory_write_view(path, &storage, total, elem_width, elem_signed, two_state, dims, n_dims, strides, origin, view_total, start, finish, has_start, has_finish, addressing_policy, radix);
    } else if (fixed_memory_read_view(path, &storage, total, elem_width, elem_signed, two_state, dims, n_dims, strides, origin, view_total, start, finish, has_start, has_finish, addressing_policy, radix) != 0 &&
               notify) {
        notify(contents, shape, LLG_CONTAINER_CHANGED_CONTENTS);
    }
}

#define LLG_CONTAINER_MEMORY_ENTRY(name, type, write)                              \
    void name(llg_string_t path, type* memory, uint64_t total, uint32_t elem_width,  \
              int8_t elem_signed, int8_t two_state, const int32_t* dims, int n_dims, \
              const uint64_t* strides, uint64_t origin, uint64_t view_total,         \
              sv4_t start, sv4_t finish, int has_start, int has_finish,              \
              int addressing_policy, \
              int radix) {                                                           \
        llg_container_memory_view(write, path, memory->data, memory->size,           \
                                  memory->notify, memory->contents_dependency,       \
                                  memory->shape_dependency, total, elem_width,       \
                                  elem_signed, two_state, dims, n_dims, strides,     \
                                  origin, view_total, start, finish, has_start,      \
                                  has_finish, addressing_policy, radix);                                \
    }
LLG_CONTAINER_MEMORY_ENTRY(llg_dyn_memory_read_view, struct llg_dyn_array_t, 0)
LLG_CONTAINER_MEMORY_ENTRY(llg_dyn_memory_write_view, struct llg_dyn_array_t, 1)
LLG_CONTAINER_MEMORY_ENTRY(llg_queue_memory_read_view, struct llg_queue_t, 0)
LLG_CONTAINER_MEMORY_ENTRY(llg_queue_memory_write_view, struct llg_queue_t, 1)
#undef LLG_CONTAINER_MEMORY_ENTRY

// Whether `element` describes fixed arrays of `row_cells` packed words of
// `elem_width` bits, viewed through `dims`.
static int llg_rows_descriptor_valid(const llg_value_desc_t* element, uint64_t row_cells,
                                     uint32_t elem_width, const int32_t* dims, int n_dims) {
    return element && element->kind == LLG_VALUE_FIXED_ARRAY &&
           element->item_count == row_cells && row_cells != 0 && element->element &&
           element->element->kind == LLG_VALUE_PACKED &&
           element->element->packed_width == elem_width && dims && n_dims > 0;
}

// Element `row` of a dynamic array or queue whose elements are fixed arrays
// of `row_cells` packed words (`llg_value_t` rows), addressed in place in
// declaration order through the view `dims`, `strides`, `origin` and
// `view_total` inside it; an unknown or missing element is an error. The
// container is never resized (IEEE 1800-2009 21.4.1). A load that placed any
// word sends one contents notification.
static void llg_rows_memory_view(int write, llg_string_t path, llg_value_t* data, size_t size,
                                 const llg_value_desc_t* element,
                                 llg_container_notify_fn notify, sv4_t* contents,
                                 sv4_t* shape, sv4_t row, uint64_t row_cells,
                                 uint32_t elem_width, int8_t elem_signed, int8_t two_state,
                                 const int32_t* dims, int n_dims, const uint64_t* strides,
                                 uint64_t origin, uint64_t view_total, sv4_t start,
                                 sv4_t finish, int has_start, int has_finish,
                                 int addressing_policy, int radix) {
    if (!llg_rows_descriptor_valid(element, row_cells, elem_width, dims, n_dims)) {
        char* filename = llg_memory_path_copy(path);
        llg_memory_error(filename, "memory descriptor does not match its container");
        free(filename);
        return;
    }
    int64_t index;
    if (!sv4_to_index_i64(row, &index) || index < 0 || (uint64_t)index >= (uint64_t)size) {
        char* filename = llg_memory_path_copy(path);
        llg_memory_error(filename, "selected index or slice is outside the memory");
        free(filename);
        return;
    }
    llg_memory_storage_t storage = {0};
    storage.row_cells = row_cells;
    storage.rows = &data[index];
    if (write) {
        fixed_memory_write_view(path, &storage, row_cells, elem_width, elem_signed, two_state, dims, n_dims, strides, origin, view_total, start, finish, has_start, has_finish, addressing_policy, radix);
    } else if (fixed_memory_read_view(path, &storage, row_cells, elem_width, elem_signed, two_state, dims, n_dims, strides, origin, view_total, start, finish, has_start, has_finish, addressing_policy, radix) != 0 &&
               notify) {
        notify(contents, shape, LLG_CONTAINER_CHANGED_CONTENTS);
    }
}

#define LLG_ROWS_MEMORY_ENTRY(name, type, write)                                        \
    void name(llg_string_t path, type* memory, sv4_t row, uint64_t row_cells,            \
              uint32_t elem_width, int8_t elem_signed, int8_t two_state,                 \
              const int32_t* dims, int n_dims, const uint64_t* strides, uint64_t origin, \
              uint64_t view_total, sv4_t start, sv4_t finish, int has_start,             \
              int has_finish, int addressing_policy, int radix) {                                          \
        llg_rows_memory_view(write, path, memory->data, memory->size, memory->element,   \
                             memory->notify, memory->contents_dependency,                \
                             memory->shape_dependency, row, row_cells, elem_width,       \
                             elem_signed, two_state, dims, n_dims, strides, origin,      \
                             view_total, start, finish, has_start, has_finish,           \
                             addressing_policy, radix);         \
    }
LLG_ROWS_MEMORY_ENTRY(llg_dyn_rows_memory_read_view, struct llg_dyn_value_array_t, 0)
LLG_ROWS_MEMORY_ENTRY(llg_dyn_rows_memory_write_view, struct llg_dyn_value_array_t, 1)
LLG_ROWS_MEMORY_ENTRY(llg_queue_rows_memory_read_view, struct llg_queue_value_array_t, 0)
LLG_ROWS_MEMORY_ENTRY(llg_queue_rows_memory_write_view, struct llg_queue_value_array_t, 1)
#undef LLG_ROWS_MEMORY_ENTRY

// ── Associative-array memories (IEEE 1800-2009 21.4.1, 21.5.3) ─────────────
// Keys are values of the array's integral index type: start, finish and file
// addresses are cast to it (7.8.4), so a wider file address keeps its low
// bits and a signed index reads a two's-complement address (S27-D16).

// The index type of an associative array, shared by the packed-element and
// fixed-array-element containers.
typedef struct {
    uint8_t kind;
    uint32_t index_width;
    int8_t index_signed;
} llg_assoc_keys_t;

static llg_assoc_keys_t llg_assoc_keys(const llg_assoc_t* array) {
    return (llg_assoc_keys_t){array->key_kind, array->key_width, array->key_signed};
}

static sv4_t llg_assoc_memory_key(llg_assoc_keys_t keys, sv4_t value) {
    return sv4_cast(value, keys.index_width, keys.index_signed);
}

static int llg_assoc_memory_less(sv4_t a, sv4_t b) {
    sv4_t less = sv4_lt(a, b);
    int result = sv4_to_bool(less);
    sv4_destroy(&less);
    return result;
}

static int llg_assoc_memory_in_range(sv4_t key, sv4_t first, sv4_t last) {
    int ascending = !llg_assoc_memory_less(last, first);
    sv4_t low = ascending ? first : last;
    sv4_t high = ascending ? last : first;
    return !llg_assoc_memory_less(key, low) && !llg_assoc_memory_less(high, key);
}

// Start and finish arguments of an associative memory task. Without a start
// a load begins at index 0 (S27-D15) and a dump at the lowest index.
static int llg_assoc_memory_bounds(const char* filename, llg_assoc_keys_t keys,
                                   sv4_t start, sv4_t finish, int has_start,
                                   int has_finish, sv4_t* first, sv4_t* last) {
    if (keys.kind != LLG_ASSOC_INTEGRAL || keys.index_width == 0) {
        llg_memory_error(filename, "associative memory needs an integral index type");
        return 0;
    }
    if (has_start && sv4_is_unknown(start)) {
        llg_memory_error(filename, "start address is unknown or outside the index range");
        return 0;
    }
    if (has_finish && sv4_is_unknown(finish)) {
        llg_memory_error(filename, "finish address is unknown or outside the index range");
        return 0;
    }
    *first = has_start ? llg_assoc_memory_key(keys, start)
                       : sv4_from_u64(0, keys.index_width, keys.index_signed);
    *last = has_finish ? llg_assoc_memory_key(keys, finish) : (sv4_t)SV4_EMPTY;
    return 1;
}

// Move `current` one key in the load direction. Stepping past the index
// type's maximum (or minimum) wraps, so the following words have no index
// (S27-D8 for fixed memories); returns 0 then and leaves `current` alone.
static int llg_assoc_memory_advance(sv4_t* current, sv4_t one, int descending,
                                    int8_t key_signed) {
    sv4_t next = descending ? sv4_sub(*current, one) : sv4_add(*current, one);
    llg_sv4_set_signed(&next, key_signed);
    if (descending ? llg_assoc_memory_less(*current, next)
                   : llg_assoc_memory_less(next, *current)) {
        sv4_destroy(&next);
        return 0;
    }
    sv4_replace(current, next);
    return 1;
}

// Load (S27-D15): each word creates the entry at the current index or
// updates it; other entries are untouched. Without a finish address the
// index advances upward until the file ends or the index type's maximum;
// with one it moves toward it and a file address must lie in the range.
static void llg_assoc_memory_read(llg_string_t path, llg_assoc_t* array,
                                  uint32_t elem_width, int8_t elem_signed,
                                  int8_t two_state, sv4_t start, sv4_t finish,
                                  int has_start, int has_finish,
                                  int radix) {
    char* filename = llg_memory_path_copy(path);
    llg_memory_stream_t stream_storage;
    llg_memory_stream_t* stream = &stream_storage;
    if (!llg_memory_open_read(filename, stream)) {
        free(filename);
        return;
    }
    sv4_t current, last;
    if (!llg_assoc_memory_bounds(filename, llg_assoc_keys(array), start, finish,
                                 has_start, has_finish, &current, &last)) {
        llg_memory_close(stream);
        free(filename);
        return;
    }
    sv4_t first = sv4_clone(&current);
    int descending = has_finish && llg_assoc_memory_less(last, first);
    sv4_t one = sv4_from_u64(1, array->key_width, array->key_signed);
    int exhausted = 0, warned_extra = 0, warned_unknown = 0, saw_address = 0;
    int terminated = 0;
    llg_memory_value_t token = {0};
    for (;;) {
        int kind = llg_memory_next_token(stream, radix, 1, &token);
        if (kind == LLG_MEMORY_TOKEN_EOF) break;
        if (kind == LLG_MEMORY_TOKEN_ERROR) {
            llg_memory_error(filename, "malformed or over-width memory token; load terminated");
            terminated = 1;
            break;
        }
        if (kind == LLG_MEMORY_TOKEN_ADDRESS) {
            saw_address = 1;
            sv4_t key = llg_assoc_memory_key(llg_assoc_keys(array), token.value);
            if (has_finish && !llg_assoc_memory_in_range(key, first, last)) {
                sv4_destroy(&key);
                llg_memory_error(filename,
                    "address jump is outside the destination memory or selected range; load terminated");
                terminated = 1;
                break;
            }
            sv4_replace(&current, key);
            exhausted = 0;
            continue;
        }
        if (exhausted) {
            if (!warned_extra) {
                llg_memory_warning(filename, "%s contains more words than the selected range", llg_memory_noun());
                warned_extra = 1;
            }
            continue;
        }
        sv4_t converted;
        if (!llg_memory_convert_word(filename, &token, elem_width, elem_signed,
                                     two_state, &warned_unknown, &converted)) {
            terminated = 1;
            break;
        }
        llg_assoc_set_integral(array, current, converted);
        sv4_destroy(&converted);
        if (has_finish && sv4_same(current, last)) {
            exhausted = 1;
            continue;
        }
        if (!llg_assoc_memory_advance(&current, one, descending, array->key_signed)) {
            exhausted = 1;
        }
    }
    if (has_finish && !saw_address && !exhausted && !terminated && !llg_memory_stream_failed(stream)) {
        llg_memory_warning(filename, "%s contains too few words for the selected range", llg_memory_noun());
    }
    if (llg_memory_stream_failed(stream)) llg_memory_error(filename, "read failed");
    sv4_destroy(&token.value);
    sv4_destroy(&one);
    sv4_destroy(&first);
    sv4_destroy(&last);
    sv4_destroy(&current);
    llg_memory_close(stream);
    free(filename);
}

// The lowercase hexadecimal digits of a key's bits, as many as the index
// type's width needs, leading zeros included (two's complement for a
// negative signed key; S27-D17, industry-standard behaviour).
static void llg_assoc_memory_format_key(sv4_t key, char* digits, size_t capacity) {
    sv4_t bits = key;
    llg_sv4_set_signed(&bits, 0);
    sv4_format('h', bits, digits, capacity);
}

// Dump (S27-D17): the existing entries in index order, each preceded by its
// own `@` address (21.5.3); start and finish select an inclusive index range
// and its direction.
static void llg_assoc_memory_write(llg_string_t path, const llg_assoc_t* array,
                                   uint32_t elem_width, sv4_t start, sv4_t finish,
                                   int has_start, int has_finish, int radix) {
    char* requested = llg_memory_path_copy(path);
    char* filename = llg_output_path(requested);
    free(requested);
    sv4_t first, last;
    if (!llg_assoc_memory_bounds(filename, llg_assoc_keys(array), start, finish,
                                 has_start, has_finish, &first, &last)) {
        free(filename);
        return;
    }
    FILE* stream = fopen(filename, "w");
    if (!stream) {
        llg_memory_warning(filename, "open for writing failed: %s", strerror(errno));
        sv4_destroy(&first);
        sv4_destroy(&last);
        free(filename);
        return;
    }
    int descending = has_finish && llg_assoc_memory_less(last, first);
    size_t key_capacity = (size_t)array->key_width + 2u;
    size_t word_capacity = (size_t)elem_width + 2u;
    char* key_digits = (char*)llg_checked_malloc(key_capacity, 1, "memory file address");
    char* digits = (char*)llg_checked_malloc(word_capacity, 1, "memory file word");
    char conversion = radix == 2 ? 'b' : 'h';
    for (size_t step = 0; step < array->size; ++step) {
        const llg_assoc_entry_t* entry =
            &array->entries[descending ? array->size - 1u - step : step];
        sv4_t key = entry->integral_key;
        if (has_finish ? !llg_assoc_memory_in_range(key, first, last)
                       : has_start && llg_assoc_memory_less(key, first)) {
            continue;
        }
        llg_assoc_memory_format_key(key, key_digits, key_capacity);
        sv4_format(conversion, entry->value, digits, word_capacity);
        if (fprintf(stream, "@%s\n%s\n", key_digits, digits) < 0) {
            llg_memory_error(filename, "write failed");
            break;
        }
    }
    free(key_digits);
    free(digits);
    if (fclose(stream) != 0) llg_memory_error(filename, "close after writing failed");
    sv4_destroy(&first);
    sv4_destroy(&last);
    free(filename);
}

void llg_assoc_memory_read_view(llg_string_t path, struct llg_assoc_t* memory, uint64_t total,
                                uint32_t elem_width, int8_t elem_signed, int8_t two_state,
                                const int32_t* dims, int n_dims, const uint64_t* strides,
                                uint64_t origin, uint64_t view_total, sv4_t start,
                                sv4_t finish, int has_start, int has_finish,
                                int addressing_policy, int radix) {
    (void)total; (void)dims; (void)n_dims; (void)strides; (void)origin;
    (void)view_total; (void)addressing_policy;
    llg_assoc_memory_read(path, memory, elem_width, elem_signed, two_state, start,
                          finish, has_start, has_finish, radix);
}

void llg_assoc_memory_write_view(llg_string_t path, struct llg_assoc_t* memory, uint64_t total,
                                 uint32_t elem_width, int8_t elem_signed, int8_t two_state,
                                 const int32_t* dims, int n_dims, const uint64_t* strides,
                                 uint64_t origin, uint64_t view_total, sv4_t start,
                                 sv4_t finish, int has_start, int has_finish,
                                 int addressing_policy, int radix) {
    (void)total; (void)elem_signed; (void)two_state; (void)dims; (void)n_dims;
    (void)strides; (void)origin; (void)view_total; (void)addressing_policy;
    llg_assoc_memory_write(path, memory, elem_width, start, finish, has_start,
                           has_finish, radix);
}

// Entry points for an associative array of fixed arrays: element `row` (a
// key) is a fixed memory addressed through the view inside it. A load creates
// a missing element once its range is valid (S27-D23); a dump of a missing or
// unknown key writes the default value and creates nothing (S27-D24).
static void llg_assoc_rows_memory_view(int write, llg_string_t path, llg_assoc_value_t* array,
                                       sv4_t row, uint64_t row_cells,
                                       uint32_t elem_width, int8_t elem_signed,
                                       int8_t two_state, const int32_t* dims, int n_dims,
                                       const uint64_t* strides, uint64_t origin,
                                       uint64_t view_total, sv4_t start, sv4_t finish,
                                       int has_start, int has_finish, int addressing_policy,
                                       int radix) {
    if (!llg_rows_descriptor_valid(array->element, row_cells, elem_width, dims, n_dims) ||
        array->key_kind != LLG_ASSOC_INTEGRAL || array->key_width == 0) {
        char* filename = llg_memory_path_copy(path);
        llg_memory_error(filename, "memory descriptor does not match its container");
        free(filename);
        return;
    }
    llg_memory_storage_t storage = {0};
    storage.row_cells = row_cells;
    if (write) {
        // A missing or unknown key reads like any associative element (IEEE
        // 1800-2009 7.8.6): a warning, and the array's default value is
        // dumped without creating the element (S27-D24, industry-standard
        // behaviour for a missing key).
        storage.rows = llg_assoc_value_element_integral(array, &row, 1, 0);
        llg_value_t fallback = {0};
        if (!storage.rows) {
            llg_native_value_init(&fallback, array->element);
            llg_value_element_read(&fallback, array->has_default_value
                                                  ? &array->default_value
                                                  : NULL);
            storage.rows = &fallback;
        }
        fixed_memory_write_view(path, &storage, row_cells, elem_width, elem_signed, two_state, dims, n_dims, strides, origin, view_total, start, finish, has_start, has_finish, addressing_policy, radix);
        if (storage.rows == &fallback) llg_native_value_destroy(&fallback);
        return;
    }
    if (sv4_is_unknown(row)) {
        char* filename = llg_memory_path_copy(path);
        llg_memory_error(filename, "selected index or slice is outside the memory");
        free(filename);
        return;
    }
    storage.assoc = array;
    storage.assoc_key = row;
    if (fixed_memory_read_view(path, &storage, row_cells, elem_width, elem_signed, two_state, dims, n_dims, strides, origin, view_total, start, finish, has_start, has_finish, addressing_policy, radix) != 0 &&
        array->notify) {
        array->notify(array->contents_dependency, array->shape_dependency,
                      LLG_CONTAINER_CHANGED_CONTENTS);
    }
}

#define LLG_ASSOC_ROWS_MEMORY_ENTRY(name, write)                                        \
    void name(llg_string_t path, struct llg_assoc_value_t* memory, sv4_t row,            \
              uint64_t row_cells, uint32_t elem_width, int8_t elem_signed,               \
              int8_t two_state, const int32_t* dims, int n_dims, const uint64_t* strides, \
              uint64_t origin, uint64_t view_total, sv4_t start, sv4_t finish,           \
              int has_start, int has_finish, int addressing_policy,                      \
              int radix) {                \
        llg_assoc_rows_memory_view(write, path, memory, row, row_cells, elem_width,       \
                                   elem_signed, two_state, dims, n_dims, strides,        \
                                   origin, view_total, start, finish, has_start,         \
                                   has_finish, addressing_policy, radix);                                   \
    }
LLG_ASSOC_ROWS_MEMORY_ENTRY(llg_assoc_rows_memory_read_view, 0)
LLG_ASSOC_ROWS_MEMORY_ENTRY(llg_assoc_rows_memory_write_view, 1)
#undef LLG_ASSOC_ROWS_MEMORY_ENTRY
