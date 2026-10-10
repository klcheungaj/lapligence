/* Runtime-sized bit streams (IEEE 1800-2009 6.24.3, 11.4.14). A stream is a
 * growable sequence of {bits,x,z} words, independent of the packed value width
 * limit; only `llg_bitstream_value` materializes one packed value. Stream bit
 * `p` is bit `63 - p % 64` of word `p / 64`, so stream order is MSB-first and
 * whole words copy without per-bit work. Only public sv4 word accessors are
 * used, so both value backends share this code. */

/* Fixed-capacity word scratch for element-sized values. */
#define LLG_BITSTREAM_LOCAL_WORDS 4u

void llg_bitstream_init(llg_bitstream_t* stream) {
    stream->words = NULL;
    stream->length = 0;
    stream->capacity = 0;
}

void llg_bitstream_destroy(llg_bitstream_t* stream) {
    free(stream->words);
    stream->words = NULL;
    stream->length = 0;
    stream->capacity = 0;
}

static void llg_bitstream_reserve(llg_bitstream_t* stream, uint64_t bits) {
    if (bits > UINT64_MAX - 63u)
        llg_container_fatal("bit stream length overflows");
    uint64_t words = (bits + 63u) / 64u;
    if (words <= stream->capacity) return;
    uint64_t capacity = stream->capacity ? stream->capacity : 4u;
    while (capacity < words) {
        if (capacity > UINT64_MAX / 2u) {
            capacity = words;
            break;
        }
        capacity *= 2u;
    }
    size_t count = llg_checked_count(capacity, sizeof(llg_sv4_word_t));
    llg_sv4_word_t* grown = realloc(stream->words, count * sizeof(*grown));
    if (!grown) llg_container_fatal("bit stream allocation failed");
    memset(grown + stream->capacity, 0,
           (count - (size_t)stream->capacity) * sizeof(*grown));
    stream->words = grown;
    stream->capacity = capacity;
}

/* Append the `count` (1..64) high bits of each plane of `chunk`. */
static void llg_bitstream_push(llg_bitstream_t* stream, llg_sv4_word_t chunk,
                               unsigned count) {
    uint64_t mask = count == 64u ? UINT64_MAX : ~(UINT64_MAX >> count);
    chunk.bits &= mask;
    chunk.x &= mask;
    chunk.z &= mask;
    llg_bitstream_reserve(stream, stream->length + count);
    uint64_t index = stream->length / 64u;
    unsigned offset = (unsigned)(stream->length % 64u);
    llg_sv4_word_t* word = &stream->words[index];
    word->bits |= chunk.bits >> offset;
    word->x |= chunk.x >> offset;
    word->z |= chunk.z >> offset;
    if (offset && offset + count > 64u) {
        llg_sv4_word_t* next = &stream->words[index + 1u];
        next->bits |= chunk.bits << (64u - offset);
        next->x |= chunk.x << (64u - offset);
        next->z |= chunk.z << (64u - offset);
    }
    stream->length += count;
}

/* Read `count` (1..64) stream bits starting at `position`, MSB-aligned. */
static llg_sv4_word_t llg_bitstream_peek(const llg_bitstream_t* stream,
                                         uint64_t position, unsigned count) {
    uint64_t index = position / 64u;
    unsigned offset = (unsigned)(position % 64u);
    llg_sv4_word_t word = stream->words[index];
    llg_sv4_word_t chunk = {word.bits << offset, word.x << offset,
                            word.z << offset};
    if (offset && offset + count > 64u) {
        llg_sv4_word_t next = stream->words[index + 1u];
        chunk.bits |= next.bits >> (64u - offset);
        chunk.x |= next.x >> (64u - offset);
        chunk.z |= next.z >> (64u - offset);
    }
    uint64_t mask = count == 64u ? UINT64_MAX : ~(UINT64_MAX >> count);
    chunk.bits &= mask;
    chunk.x &= mask;
    chunk.z &= mask;
    return chunk;
}

/* Bits [low, low + count) of little-endian words, LSB-aligned (count 1..64). */
static llg_sv4_word_t llg_bitstream_word_bits(const llg_sv4_word_t* words,
                                              uint64_t low, unsigned count) {
    size_t index = (size_t)(low / 64u);
    unsigned offset = (unsigned)(low % 64u);
    llg_sv4_word_t chunk = {words[index].bits >> offset,
                            words[index].x >> offset,
                            words[index].z >> offset};
    if (offset && offset + count > 64u) {
        chunk.bits |= words[index + 1u].bits << (64u - offset);
        chunk.x |= words[index + 1u].x << (64u - offset);
        chunk.z |= words[index + 1u].z << (64u - offset);
    }
    if (count < 64u) {
        uint64_t mask = (UINT64_C(1) << count) - 1u;
        chunk.bits &= mask;
        chunk.x &= mask;
        chunk.z &= mask;
    }
    return chunk;
}

/* Append little-endian words holding a `width`-bit value, MSB first. */
static void llg_bitstream_push_words(llg_bitstream_t* stream,
                                     const llg_sv4_word_t* words,
                                     uint64_t width) {
    llg_bitstream_reserve(stream, stream->length + width);
    uint64_t remaining = width;
    while (remaining) {
        unsigned count = remaining > 64u ? 64u : (unsigned)remaining;
        llg_sv4_word_t chunk =
            llg_bitstream_word_bits(words, remaining - count, count);
        unsigned shift = 64u - count;
        llg_sv4_word_t aligned = {chunk.bits << shift, chunk.x << shift,
                                  chunk.z << shift};
        llg_bitstream_push(stream, aligned, count);
        remaining -= count;
    }
}

void llg_bitstream_append_value(llg_bitstream_t* stream, sv4_t value) {
    uint32_t width = llg_sv4_width(value);
    if (!width) return;
    size_t count = llg_sv4_words(value);
    llg_sv4_word_t local[LLG_BITSTREAM_LOCAL_WORDS];
    llg_sv4_word_t* words = count <= LLG_BITSTREAM_LOCAL_WORDS
                                ? local
                                : llg_alloc_items(count, sizeof(*words));
    llg_sv4_export_words(value, 0, words, count);
    llg_bitstream_push_words(stream, words, width);
    if (words != local) free(words);
}

void llg_bitstream_append_values(llg_bitstream_t* stream, const sv4_t* values,
                                 uint64_t count) {
    for (uint64_t index = 0; index < count; ++index)
        llg_bitstream_append_value(stream, values[index]);
}

void llg_bitstream_append_stream(llg_bitstream_t* stream,
                                 const llg_bitstream_t* source) {
    llg_bitstream_reserve(stream, stream->length + source->length);
    for (uint64_t position = 0; position < source->length; position += 64u) {
        uint64_t left = source->length - position;
        unsigned count = left > 64u ? 64u : (unsigned)left;
        llg_bitstream_push(stream, llg_bitstream_peek(source, position, count),
                           count);
    }
}

/* Elements of a packed-element array in stream order; indices outside
 * [0, size) stream the element default (SV 11.4.14.4). */
static void llg_bitstream_append_elements(llg_bitstream_t* stream,
                                          const sv4_t* data, size_t size,
                                          uint32_t width, int8_t is_signed,
                                          uint8_t two_state,
                                          int selector_kind, sv4_t first,
                                          sv4_t second) {
    int64_t left;
    int64_t right;
    size_t count;
    llg_stream_bounds(selector_kind, first, second, size, &left, &right, &count);
    if (!count) return;
    if ((uint64_t)count > UINT64_MAX / width)
        llg_container_fatal("bit stream length overflows");
    llg_bitstream_reserve(stream, stream->length + (uint64_t)count * width);
    sv4_t fallback = SV4_EMPTY;
    int has_fallback = 0;
    for (size_t offset = 0; offset < count; ++offset) {
        int64_t index = llg_stream_index_at(left, right, offset);
        if (index >= 0 && (uint64_t)index < (uint64_t)size) {
            llg_bitstream_append_value(stream, data[index]);
            continue;
        }
        if (!has_fallback) {
            fallback = llg_element_default(width, is_signed, two_state);
            has_fallback = 1;
        }
        llg_bitstream_append_value(stream, fallback);
    }
    if (has_fallback) sv4_destroy(&fallback);
}

void llg_bitstream_append_dyn(llg_bitstream_t* stream,
                              const llg_dyn_array_t* array, int selector_kind,
                              sv4_t first, sv4_t second) {
    llg_bitstream_append_elements(stream, array->data, array->size,
                                  array->element_width, array->element_signed,
                                  array->element_two_state, selector_kind,
                                  first, second);
}

void llg_bitstream_append_queue(llg_bitstream_t* stream,
                                const llg_queue_t* queue, int selector_kind,
                                sv4_t first, sv4_t second) {
    llg_bitstream_append_elements(stream, queue->data, queue->size,
                                  queue->element_width, queue->element_signed,
                                  queue->element_two_state, selector_kind,
                                  first, second);
}

/* Associative arrays stream in index-sorted order (SV 11.4.14.1), which is
 * the order their entries are kept in. */
void llg_bitstream_append_assoc(llg_bitstream_t* stream,
                                const llg_assoc_t* array) {
    for (size_t index = 0; index < array->size; ++index)
        llg_bitstream_append_value(stream, array->entries[index].value);
}

/* One recursive value in stream order. A null value, or an aggregate or
 * container without storage, streams its type's default. */
static void llg_bitstream_append_item(llg_bitstream_t* stream,
                                      const llg_value_t* value,
                                      const llg_value_desc_t* desc) {
    if (value && !value->desc) value = NULL;
    switch (desc->kind) {
        case LLG_VALUE_PACKED:
            if (value) {
                llg_bitstream_append_value(stream, value->value.packed);
            } else {
                sv4_t fallback = llg_element_default(
                    desc->packed_width, desc->packed_signed, desc->packed_two_state);
                llg_bitstream_append_value(stream, fallback);
                sv4_destroy(&fallback);
            }
            return;
        case LLG_VALUE_STRING:
            if (value) llg_bitstream_append_string(stream, value->value.string);
            return;
        case LLG_VALUE_AGGREGATE:
        case LLG_VALUE_FIXED_ARRAY: {
            const llg_value_t* items = value ? value->value.items : NULL;
            for (size_t index = 0; index < desc->item_count; ++index)
                llg_bitstream_append_item(stream, items ? &items[index] : NULL,
                                          llg_value_item_desc(desc, index));
            return;
        }
        case LLG_VALUE_CONTAINER: {
            const llg_dyn_value_array_t* items = value ? value->value.container : NULL;
            if (items)
                for (size_t index = 0; index < items->size; ++index)
                    llg_bitstream_append_item(stream, &items->data[index],
                                              desc->element);
            return;
        }
        default:
            llg_container_fatal("streamed value is not a bit-stream type");
    }
}

static void llg_bitstream_append_items(llg_bitstream_t* stream,
                                       const llg_value_t* data, size_t size,
                                       const llg_value_desc_t* element,
                                       int selector_kind, sv4_t first,
                                       sv4_t second) {
    int64_t left;
    int64_t right;
    size_t count;
    llg_stream_bounds(selector_kind, first, second, size, &left, &right, &count);
    for (size_t offset = 0; offset < count; ++offset) {
        int64_t index = llg_stream_index_at(left, right, offset);
        int in_range = index >= 0 && (uint64_t)index < (uint64_t)size;
        llg_bitstream_append_item(stream, in_range ? &data[index] : NULL, element);
    }
}

void llg_bitstream_append_dyn_values(llg_bitstream_t* stream,
                                     const llg_dyn_value_array_t* array,
                                     int selector_kind, sv4_t first,
                                     sv4_t second) {
    llg_bitstream_append_items(stream, array->data, array->size,
                               array->element, selector_kind, first, second);
}

void llg_bitstream_append_queue_values(llg_bitstream_t* stream,
                                       const llg_queue_value_array_t* queue,
                                       int selector_kind, sv4_t first,
                                       sv4_t second) {
    llg_bitstream_append_items(stream, queue->data, queue->size,
                               queue->element, selector_kind, first, second);
}

/* A string streams as a dynamic array of bytes, index 0 leftmost (6.24.3). */
void llg_bitstream_append_string(llg_bitstream_t* stream, llg_string_t value) {
    llg_bitstream_reserve(stream, stream->length + (uint64_t)value.len * 8u);
    for (size_t index = 0; index < value.len; ++index) {
        llg_sv4_word_t chunk = {
            (uint64_t)(unsigned char)value.data[index] << 56, 0, 0};
        llg_bitstream_push(stream, chunk, 8u);
    }
}

/* Copy `count` bits from `source` at `position` to the end of `target`. */
static void llg_bitstream_copy(llg_bitstream_t* target,
                               const llg_bitstream_t* source,
                               uint64_t position, uint64_t count) {
    while (count) {
        unsigned step = count > 64u ? 64u : (unsigned)count;
        llg_bitstream_push(target, llg_bitstream_peek(source, position, step),
                           step);
        position += step;
        count -= step;
    }
}

/* Reverse the order of `slice`-bit blocks (SV 11.4.14.2). A pack (`<<` on
 * the right-hand side) slices from the right, leaving any short block last;
 * an unpack inverts it by slicing from the left. */
void llg_bitstream_reverse(llg_bitstream_t* stream, uint32_t slice,
                           int from_left) {
    if (!slice) llg_container_fatal("streaming slice size must be positive");
    if (stream->length <= slice) return;
    llg_bitstream_t result;
    llg_bitstream_init(&result);
    llg_bitstream_reserve(&result, stream->length);
    uint64_t length = stream->length;
    uint64_t blocks = (length + slice - 1u) / slice;
    uint64_t tail = length % slice;
    for (uint64_t block = 0; block < blocks; ++block) {
        uint64_t start;
        uint64_t size = slice;
        if (from_left) {
            // Block `block` from the end of the left-sliced source.
            uint64_t source_block = blocks - 1u - block;
            start = source_block * slice;
            if (tail && source_block == blocks - 1u) size = tail;
        } else {
            uint64_t end = length - block * slice;
            if (end < slice) size = end;
            start = end - size;
        }
        llg_bitstream_copy(&result, stream, start, size);
    }
    llg_bitstream_destroy(stream);
    *stream = result;
}

/* Stream bits [position, position + width) as one packed value. */
static sv4_t llg_bitstream_slice(const llg_bitstream_t* stream,
                                 uint64_t position, uint32_t width) {
    sv4_t value = sv4_zero(width, 0);
    size_t count = llg_sv4_words(value);
    llg_sv4_word_t local[LLG_BITSTREAM_LOCAL_WORDS];
    llg_sv4_word_t* words = count <= LLG_BITSTREAM_LOCAL_WORDS
                                ? local
                                : llg_alloc_items(count, sizeof(*words));
    for (size_t index = 0; index < count; ++index) {
        uint64_t low = (uint64_t)index * 64u;
        unsigned step = width - low > 64u ? 64u : (unsigned)(width - low);
        // Value bits [low, low + step) are the stream bits ending
        // `low` bits before the right end of the slice.
        llg_sv4_word_t chunk = llg_bitstream_peek(
            stream, position + (width - low - step), step);
        unsigned shift = 64u - step;
        words[index].bits = chunk.bits >> shift;
        words[index].x = chunk.x >> shift;
        words[index].z = chunk.z >> shift;
    }
    llg_sv4_import_words(&value, 0, words, count);
    if (words != local) free(words);
    return value;
}

sv4_t llg_bitstream_bits(const llg_bitstream_t* stream, uint64_t position,
                         uint32_t width) {
    if (!width || position > stream->length || stream->length - position < width)
        llg_container_fatal("bit stream read is out of range");
    return llg_bitstream_slice(stream, position, width);
}

sv4_t llg_bitstream_value(const llg_bitstream_t* stream) {
    if (!stream->length) {
        sv4_t empty = SV4_EMPTY;
        return empty;
    }
    if (stream->length > (uint64_t)(LLG_SUPPORTED_WIDTH_LIMIT - 1u))
        llg_container_fatal("streaming value reaches supported width limit");
    return llg_bitstream_slice(stream, 0, (uint32_t)stream->length);
}

/* Element count for a resizable destination of `width`-bit elements. An
 * assignment (`exact` 0) left-aligns the stream and zero-fills the last
 * element (SV 11.4.14); a bit-stream cast or whole unpack (`exact` 1)
 * requires whole elements (SV 6.24.3, 11.4.14.4). */
static size_t llg_bitstream_elements(const llg_bitstream_t* stream,
                                     uint32_t width, int exact) {
    if (!width) llg_container_fatal("streaming destination has an empty element type");
    if (exact && stream->length % width)
        llg_container_fatal(
            "bit stream size does not match a whole number of destination elements");
    uint64_t count = (stream->length + width - 1u) / width;
    return llg_checked_count(count, sizeof(sv4_t));
}

/* The `count` destination elements, each MSB-first, zero-filling the bits of
 * the last element past the end of the stream. */
static sv4_t* llg_bitstream_element_values(const llg_bitstream_t* stream,
                                           uint32_t width, size_t count) {
    sv4_t* values = llg_alloc_items(count, sizeof(*values));
    for (size_t index = 0; index < count; ++index) {
        uint64_t position = (uint64_t)index * width;
        uint64_t available = stream->length - position;
        if (available >= width) {
            values[index] = llg_bitstream_slice(stream, position, width);
            continue;
        }
        sv4_t head = llg_bitstream_slice(stream, position, (uint32_t)available);
        values[index] = sv4_zero(width, 0);
        sv4_part_select_set(&values[index], (int64_t)width - 1,
                            (int64_t)(width - (uint32_t)available), head);
        sv4_destroy(&head);
    }
    return values;
}

void llg_bitstream_to_dyn(llg_dyn_array_t* dst, const llg_bitstream_t* stream,
                          int exact) {
    size_t count = llg_bitstream_elements(stream, dst->element_width, exact);
    sv4_t* values = llg_bitstream_element_values(stream, dst->element_width,
                                                 count);
    llg_dyn_assign_values(dst, values, count);
    sv4_destroy_array(values, count);
    free(values);
}

void llg_bitstream_to_queue(llg_queue_t* dst, const llg_bitstream_t* stream,
                            int exact) {
    size_t count = llg_bitstream_elements(stream, dst->element_width, exact);
    sv4_t* values = llg_bitstream_element_values(stream, dst->element_width,
                                                 count);
    llg_queue_assign_values(dst, values, count);
    sv4_destroy_array(values, count);
    free(values);
}

/* A string destination is a dynamic array of bytes (6.24.3); it cannot hold
 * "\0" (6.16), so zero bytes, including zero fill, are dropped. */
llg_string_t llg_bitstream_string(const llg_bitstream_t* stream, int exact) {
    size_t count = llg_bitstream_elements(stream, 8u, exact);
    char* bytes = count ? llg_alloc_items(count, 1) : NULL;
    size_t length = 0;
    for (size_t index = 0; index < count; ++index) {
        uint64_t position = (uint64_t)index * 8u;
        uint64_t available = stream->length - position;
        unsigned step = available >= 8u ? 8u : (unsigned)available;
        llg_sv4_word_t chunk = llg_bitstream_peek(stream, position, step);
        // X and Z bits become zero, as in a cast to a 2-state byte.
        unsigned char byte =
            (unsigned char)((chunk.bits & ~(chunk.x | chunk.z)) >> 56);
        if (byte) bytes[length++] = (char)byte;
    }
    llg_string_t result = llg_string_bytes(bytes, length);
    free(bytes);
    return result;
}

void llg_bitstream_value_to(sv4_t* dst, const llg_bitstream_t* stream) {
    sv4_replace(dst, llg_bitstream_value(stream));
}
