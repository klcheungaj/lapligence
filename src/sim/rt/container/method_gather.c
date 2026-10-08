/* Array-method results computed by generated loops (SIM-019) ------------- */

/* Generated code evaluates a method's `with` expression once per element, in
 * index (or key) order, inside the caller's frame, so it may read automatic
 * state and any element type. It records ordinal positions of the selected
 * elements, or one key per element, in temporary queues; the operations below
 * turn those into the method result without calling back into the model. */

/* Ordinal `slot` of `positions`, or 0 when it is unknown, negative or not
 * below `count` (the receiver changed size while the keys were computed). */
static int llg_method_position(const llg_queue_t* positions, size_t slot,
                               size_t count, size_t* position) {
    return llg_index(positions->data[slot], count, 0, position);
}

static void llg_method_check_positions(const llg_queue_t* positions) {
    if (!positions || (positions->size && !positions->data))
        llg_container_fatal("malformed array-method positions");
}

/* Packed elements (or, with `keys`, their int indices) of `data` at the
 * selected positions, into a packed queue. */
static void llg_method_gather_sv4(llg_queue_t* dst, const sv4_t* data,
                                  size_t count, const llg_queue_t* positions,
                                  int keys) {
    llg_method_check_positions(positions);
    if (!dst) llg_container_fatal("null array-method result");
    size_t selected = positions->size;
    sv4_t* values = llg_alloc_items(selected, sizeof(*values));
    size_t used = 0;
    for (size_t slot = 0; slot < selected; ++slot) {
        size_t position;
        if (!llg_method_position(positions, slot, count, &position)) continue;
        values[used++] = keys ? sv4_clone(&positions->data[slot])
                              : sv4_clone(&data[position]);
    }
    llg_queue_assign_values(dst, values, used);
    sv4_destroy_array(values, used);
    free(values);
}

void llg_method_gather_positions(llg_queue_t* dst, const llg_queue_t* positions,
                                 size_t count) {
    llg_method_gather_sv4(dst, NULL, count, positions, 1);
}

void llg_dyn_gather(llg_queue_t* dst, const llg_dyn_array_t* src,
                    const llg_queue_t* positions, int keys) {
    if (!src) llg_container_fatal("null dynamic-array method source");
    llg_method_gather_sv4(dst, src->data, src->size, positions, keys);
}

void llg_queue_gather(llg_queue_t* dst, const llg_queue_t* src,
                      const llg_queue_t* positions, int keys) {
    if (!src) llg_container_fatal("null queue method source");
    llg_method_gather_sv4(dst, src->data, src->size, positions, keys);
}

void llg_assoc_gather(llg_queue_t* dst, const llg_assoc_t* src,
                      const llg_queue_t* positions, int keys) {
    if (!src || !dst) llg_container_fatal("null associative-array method operand");
    llg_method_check_positions(positions);
    if (keys && src->key_kind != LLG_ASSOC_INTEGRAL)
        llg_container_fatal("associative index result requires integral keys");
    // Entries are in key order, so an ordinal position names one entry.
    sv4_t* values = llg_alloc_items(positions->size, sizeof(*values));
    size_t used = 0;
    for (size_t slot = 0; slot < positions->size; ++slot) {
        size_t position;
        if (!llg_method_position(positions, slot, src->size, &position)) continue;
        values[used++] = keys ? src->entries[position].integral_key
                              : src->entries[position].value;
    }
    // Borrowed views: assignment converts copies into the destination.
    llg_queue_assign_values(dst, values, used);
    free(values);
}

static void llg_method_gather_string_keys(llg_queue_value_array_t* dst,
                                          const unsigned char* const* keys,
                                          const size_t* lengths, size_t count) {
    llg_string_t* values = llg_alloc_items(count, sizeof(*values));
    for (size_t i = 0; i < count; ++i)
        values[i] = llg_string_bytes((const char*)keys[i], lengths[i]);
    llg_queue_value_assign_strings(dst, values, count);
    free(values);
}

void llg_assoc_gather_string_keys(llg_queue_value_array_t* dst,
                                  const llg_assoc_t* src,
                                  const llg_queue_t* positions) {
    if (!src || !dst || src->key_kind != LLG_ASSOC_STRING)
        llg_container_fatal("string index result requires a string-keyed array");
    llg_method_check_positions(positions);
    const unsigned char** keys =
        llg_alloc_items(positions->size, sizeof(*keys));
    size_t* lengths = llg_alloc_items(positions->size, sizeof(*lengths));
    size_t used = 0;
    for (size_t slot = 0; slot < positions->size; ++slot) {
        size_t position;
        if (!llg_method_position(positions, slot, src->size, &position)) continue;
        keys[used] = src->entries[position].string_key;
        lengths[used++] = src->entries[position].string_length;
    }
    llg_method_gather_string_keys(dst, keys, lengths, used);
    free(lengths);
    free(keys);
}

/* Descriptor-backed elements at the selected positions, converted into the
 * destination element type. */
static void llg_method_gather_values(llg_queue_value_array_t* dst,
                                     const llg_value_t* const* items,
                                     size_t count) {
    size_t retained = count < dst->limit ? count : dst->limit;
    llg_value_t* data = llg_alloc_items(retained, sizeof(*data));
    if (retained) memset(data, 0, retained * sizeof(*data));
    for (size_t i = 0; i < retained; ++i)
        llg_value_copy(&data[i], dst->element, items[i]);
    llg_queue_value_commit(dst, data, retained);
    if (retained != count)
        llg_container_warning("bounded queue assignment discarded tail elements");
}

static void llg_method_gather_value_array(llg_queue_value_array_t* dst,
                                          const llg_value_t* data, size_t count,
                                          const llg_queue_t* positions) {
    llg_method_check_positions(positions);
    if (!dst) llg_container_fatal("null array-method result");
    const llg_value_t** items = llg_alloc_items(positions->size, sizeof(*items));
    size_t used = 0;
    for (size_t slot = 0; slot < positions->size; ++slot) {
        size_t position;
        if (llg_method_position(positions, slot, count, &position))
            items[used++] = &data[position];
    }
    llg_method_gather_values(dst, items, used);
    free(items);
}

void llg_dyn_value_gather(llg_queue_value_array_t* dst,
                          const llg_dyn_value_array_t* src,
                          const llg_queue_t* positions) {
    if (!src) llg_container_fatal("null dynamic-array method source");
    llg_method_gather_value_array(dst, src->data, src->size, positions);
}

void llg_queue_value_gather(llg_queue_value_array_t* dst,
                            const llg_queue_value_array_t* src,
                            const llg_queue_t* positions) {
    if (!src) llg_container_fatal("null queue method source");
    llg_method_gather_value_array(dst, src->data, src->size, positions);
}

void llg_assoc_value_gather(llg_queue_value_array_t* dst,
                            const llg_assoc_value_t* src,
                            const llg_queue_t* positions) {
    if (!src || !dst) llg_container_fatal("null associative-array method operand");
    llg_method_check_positions(positions);
    const llg_value_t** items = llg_alloc_items(positions->size, sizeof(*items));
    size_t used = 0;
    for (size_t slot = 0; slot < positions->size; ++slot) {
        size_t position;
        if (llg_method_position(positions, slot, src->size, &position))
            items[used++] = &src->entries[position].value;
    }
    llg_method_gather_values(dst, items, used);
    free(items);
}

void llg_assoc_value_gather_keys(llg_queue_t* dst, const llg_assoc_value_t* src,
                                 const llg_queue_t* positions) {
    if (!src || !dst || src->key_kind != LLG_ASSOC_INTEGRAL)
        llg_container_fatal("associative index result requires integral keys");
    llg_method_check_positions(positions);
    sv4_t* keys = llg_alloc_items(positions->size, sizeof(*keys));
    size_t used = 0;
    for (size_t slot = 0; slot < positions->size; ++slot) {
        size_t position;
        if (llg_method_position(positions, slot, src->size, &position))
            keys[used++] = src->entries[position].integral_key;
    }
    llg_queue_assign_values(dst, keys, used);
    free(keys);
}

void llg_assoc_value_gather_string_keys(llg_queue_value_array_t* dst,
                                        const llg_assoc_value_t* src,
                                        const llg_queue_t* positions) {
    if (!src || !dst || src->key_kind != LLG_ASSOC_STRING)
        llg_container_fatal("string index result requires a string-keyed array");
    llg_method_check_positions(positions);
    const unsigned char** keys =
        llg_alloc_items(positions->size, sizeof(*keys));
    size_t* lengths = llg_alloc_items(positions->size, sizeof(*lengths));
    size_t used = 0;
    for (size_t slot = 0; slot < positions->size; ++slot) {
        size_t position;
        if (!llg_method_position(positions, slot, src->size, &position)) continue;
        keys[used] = src->entries[position].string_key;
        lengths[used++] = src->entries[position].string_length;
    }
    llg_method_gather_string_keys(dst, keys, lengths, used);
    free(lengths);
    free(keys);
}

/* Distinct keys ------------------------------------------------------------ */

static uint64_t llg_method_mix(uint64_t hash, uint64_t word) {
    hash ^= word + UINT64_C(0x9e3779b97f4a7c15) + (hash << 6) + (hash >> 2);
    return hash;
}

/* A hash consistent with sv4_same: every plane word up to the last word with
 * any state set, so equal values of different widths agree. */
static uint64_t llg_method_sv4_hash(sv4_t value) {
    size_t words = llg_sv4_words(value);
    while (words && !llg_sv4_word(value, words - 1, LLG_SV4_BITS) &&
           !llg_sv4_word(value, words - 1, LLG_SV4_X) &&
           !llg_sv4_word(value, words - 1, LLG_SV4_Z))
        --words;
    uint64_t hash = UINT64_C(0xcbf29ce484222325);
    for (size_t word = 0; word < words; ++word) {
        hash = llg_method_mix(hash, llg_sv4_word(value, word, LLG_SV4_BITS));
        hash = llg_method_mix(hash, llg_sv4_word(value, word, LLG_SV4_X));
        hash = llg_method_mix(hash, llg_sv4_word(value, word, LLG_SV4_Z));
    }
    return hash;
}

static uint64_t llg_method_bytes_hash(const char* data, size_t length) {
    uint64_t hash = UINT64_C(0xcbf29ce484222325);
    for (size_t i = 0; i < length; ++i) {
        hash ^= (unsigned char)data[i];
        hash *= UINT64_C(0x100000001b3);
    }
    return hash;
}

/* Open-addressing set of element positions keyed by `hashes`; `same` decides
 * equality of two positions. Marks the first position of every distinct key
 * in `keep`, so the selection keeps source order, in expected O(n). */
static void llg_method_mark_distinct(size_t count, const uint64_t* hashes,
                                     int (*same)(const void*, size_t, size_t),
                                     const void* keys, unsigned char* keep) {
    size_t capacity = 16;
    while (capacity < count * 2) {
        if (capacity > SIZE_MAX / 2)
            llg_container_fatal("container allocation size overflow");
        capacity *= 2;
    }
    size_t* table = llg_alloc_items(capacity, sizeof(*table));
    for (size_t i = 0; i < capacity; ++i) table[i] = SIZE_MAX;
    for (size_t position = 0; position < count; ++position) {
        size_t probe = (size_t)hashes[position] & (capacity - 1);
        for (;;) {
            size_t other = table[probe];
            if (other == SIZE_MAX) {
                table[probe] = position;
                keep[position] = 1;
                break;
            }
            if (hashes[other] == hashes[position] &&
                same(keys, other, position))
                break;
            probe = (probe + 1) & (capacity - 1);
        }
    }
    free(table);
}

static int llg_method_same_sv4(const void* keys, size_t a, size_t b) {
    const sv4_t* values = keys;
    return sv4_same(values[a], values[b]);
}

static int llg_method_same_string(const void* keys, size_t a, size_t b) {
    const llg_value_t* values = keys;
    const llg_string_t* left = &values[a].value.string;
    const llg_string_t* right = &values[b].value.string;
    return left->len == right->len &&
           (!left->len || !memcmp(left->data, right->data, left->len));
}

/* Replace `positions` with the selected ordinals in ascending order. */
static void llg_method_assign_kept(llg_queue_t* positions,
                                   const unsigned char* keep, size_t count) {
    sv4_t* values = llg_alloc_items(count, sizeof(*values));
    size_t used = 0;
    for (size_t position = 0; position < count; ++position)
        if (keep[position])
            values[used++] = sv4_from_u64((uint64_t)position, 32, 1);
    llg_queue_assign_values(positions, values, used);
    sv4_destroy_array(values, used);
    free(values);
}

/* Packed keys compare with sv4_same, as the packed `unique` method does. */
static void llg_method_distinct_sv4(unsigned char* keep, const sv4_t* keys,
                                    size_t count) {
    uint64_t* hashes = llg_alloc_items(count, sizeof(*hashes));
    for (size_t i = 0; i < count; ++i) hashes[i] = llg_method_sv4_hash(keys[i]);
    llg_method_mark_distinct(count, hashes, llg_method_same_sv4, keys, keep);
    free(hashes);
}

void llg_method_unique_positions(llg_queue_t* positions, const llg_queue_t* keys) {
    if (!positions || !keys) llg_container_fatal("null array-method keys");
    size_t count = keys->size;
    unsigned char* keep = llg_alloc_items(count, sizeof(*keep));
    if (count) memset(keep, 0, count);
    llg_method_distinct_sv4(keep, keys->data, count);
    llg_method_assign_kept(positions, keep, count);
    free(keep);
}

/* Real keys are equal numerically (so -0.0 equals 0.0) and NaN equals
 * nothing; string keys compare their bytes. */
void llg_method_unique_value_positions(llg_queue_t* positions,
                                       const llg_queue_value_array_t* keys) {
    if (!positions || !keys) llg_container_fatal("null array-method keys");
    size_t count = keys->size;
    unsigned char* keep = llg_alloc_items(count, sizeof(*keep));
    if (count) memset(keep, 0, count);
    if (count && keys->element->kind == LLG_VALUE_STRING) {
        uint64_t* hashes = llg_alloc_items(count, sizeof(*hashes));
        for (size_t i = 0; i < count; ++i)
            hashes[i] = llg_method_bytes_hash(keys->data[i].value.string.data,
                                              keys->data[i].value.string.len);
        llg_method_mark_distinct(count, hashes, llg_method_same_string,
                                 keys->data, keep);
        free(hashes);
    } else if (count) {
        if (keys->element->kind != LLG_VALUE_REAL)
            llg_container_fatal("array-method keys must be packed, real or string");
        // NaN keys equal nothing, so each is kept; the known keys are sorted
        // stably and the first (lowest) position of each equal run is kept.
        double* values = llg_alloc_items(count, sizeof(*values));
        size_t* known = llg_alloc_items(count, sizeof(*known));
        size_t* order = llg_alloc_items(count, 2 * sizeof(*order));
        size_t known_count = 0;
        for (size_t i = 0; i < count; ++i) {
            double value = keys->data[i].value.real;
            if (value != value) {
                keep[i] = 1;
                continue;
            }
            values[known_count] = value;
            known[known_count++] = i;
        }
        (void)llg_real_sort_order(values, known_count, 0, order);
        for (size_t rank = 0; rank < known_count; ++rank)
            if (rank == 0 || values[order[rank]] != values[order[rank - 1]])
                keep[known[order[rank]]] = 1;
        free(order);
        free(known);
        free(values);
    }
    llg_method_assign_kept(positions, keep, count);
    free(keep);
}

/* Ordering by computed keys ------------------------------------------------ */

static int llg_method_string_less(const llg_string_t* a, const llg_string_t* b) {
    size_t shared = a->len < b->len ? a->len : b->len;
    int order = shared ? memcmp(a->data, b->data, shared) : 0;
    return order ? order < 0 : a->len < b->len;
}

/* Stable bottom-up merge sort of order[0, count) by string keys. */
static void llg_method_string_order(const llg_value_t* keys, size_t count,
                                    int descending, size_t* order) {
    size_t* source = order;
    size_t* target = order + count;
    for (size_t i = 0; i < count; ++i) order[i] = i;
    for (size_t run = 1; run < count; run *= 2) {
        for (size_t low = 0; low < count; low += 2 * run) {
            size_t middle = low + run < count ? low + run : count;
            size_t high = low + 2 * run < count ? low + 2 * run : count;
            size_t left = low, right = middle, out = low;
            while (left < middle && right < high) {
                const llg_string_t* l = &keys[source[left]].value.string;
                const llg_string_t* r = &keys[source[right]].value.string;
                int before = descending ? llg_method_string_less(l, r)
                                        : llg_method_string_less(r, l);
                if (before)
                    target[out++] = source[right++];
                else
                    target[out++] = source[left++];
            }
            while (left < middle) target[out++] = source[left++];
            while (right < high) target[out++] = source[right++];
        }
        size_t* swap = source;
        source = target;
        target = swap;
    }
    if (source != order) memcpy(order, source, count * sizeof(*order));
}

/* Permutation for `count` elements from packed or value keys; returns
 * whether any element moves, or -1 when the key count no longer matches. */
static int llg_method_key_order(size_t count, const llg_queue_t* packed_keys,
                                const llg_queue_value_array_t* value_keys,
                                int descending, size_t* order) {
    size_t keys = packed_keys ? packed_keys->size : value_keys->size;
    if (keys != count) {
        llg_container_warning(
            "array ordering keys do not match the receiver size; order unchanged");
        return -1;
    }
    if (packed_keys)
        return llg_sort_permutation(packed_keys->data, count, descending, order,
                                    order + count);
    if (count && value_keys->element->kind == LLG_VALUE_REAL) {
        double* values = llg_alloc_items(count, sizeof(*values));
        for (size_t i = 0; i < count; ++i) values[i] = value_keys->data[i].value.real;
        int changed = llg_real_sort_order(values, count, descending, order);
        free(values);
        return changed;
    }
    if (count && value_keys->element->kind != LLG_VALUE_STRING)
        llg_container_fatal("array-method keys must be packed, real or string");
    llg_method_string_order(value_keys->data, count, descending, order);
    for (size_t i = 0; i < count; ++i)
        if (order[i] != i) return 1;
    return 0;
}

/* Apply order (order[i] names the element that moves to i) by following
 * cycles, so every element moves once; consumes `order`. */
static void llg_method_permute_sv4(sv4_t* data, uint64_t* element_ids,
                                   size_t* order, size_t count) {
    for (size_t i = 0; i < count; ++i) {
        if (order[i] == i) continue;
        sv4_t saved = SV4_EMPTY;
        sv4_move(&saved, &data[i]);
        uint64_t saved_id = element_ids ? element_ids[i] : 0;
        size_t hole = i;
        for (;;) {
            size_t source = order[hole];
            order[hole] = hole;
            if (source == i) {
                sv4_move(&data[hole], &saved);
                if (element_ids) element_ids[hole] = saved_id;
                break;
            }
            sv4_move(&data[hole], &data[source]);
            if (element_ids) element_ids[hole] = element_ids[source];
            hole = source;
        }
    }
}

static void llg_method_permute_values(llg_value_t* data, size_t* order,
                                      size_t count) {
    for (size_t i = 0; i < count; ++i) {
        if (order[i] == i) continue;
        llg_value_t saved = data[i];
        size_t hole = i;
        for (;;) {
            size_t source = order[hole];
            order[hole] = hole;
            if (source == i) {
                data[hole] = saved;
                break;
            }
            data[hole] = data[source];
            hole = source;
        }
    }
}

static int llg_method_sort_sv4_by_keys(sv4_t* data, uint64_t* element_ids,
                                       size_t count,
                                       const llg_queue_t* packed_keys,
                                       const llg_queue_value_array_t* value_keys,
                                       int descending) {
    if (count < 2) return 0;
    size_t* order = llg_alloc_items(count, 2 * sizeof(*order));
    int changed = llg_method_key_order(count, packed_keys, value_keys,
                                       descending, order);
    if (changed > 0) llg_method_permute_sv4(data, element_ids, order, count);
    free(order);
    return changed > 0;
}

static int llg_method_sort_values_by_keys(
    llg_value_t* data, size_t count, const llg_queue_t* packed_keys,
    const llg_queue_value_array_t* value_keys, int descending) {
    if (count < 2) return 0;
    size_t* order = llg_alloc_items(count, 2 * sizeof(*order));
    int changed = llg_method_key_order(count, packed_keys, value_keys,
                                       descending, order);
    if (changed > 0) llg_method_permute_values(data, order, count);
    free(order);
    return changed > 0;
}

/* `packed_keys` or `value_keys` (exactly one non-null) holds one key per
 * element, computed in index order before the call (SV 7.12.2). */
void llg_dyn_sort_by_keys(llg_dyn_array_t* array, const llg_queue_t* packed_keys,
                          const llg_queue_value_array_t* value_keys,
                          int descending) {
    if (!array || (!packed_keys == !value_keys))
        llg_container_fatal("malformed dynamic-array ordering");
    if (llg_method_sort_sv4_by_keys(array->data, NULL, array->size, packed_keys,
                                    value_keys, descending))
        llg_notify(array->notify, array->contents_dependency,
                   array->shape_dependency, LLG_CONTAINER_CHANGED_CONTENTS);
}

void llg_queue_sort_by_keys(llg_queue_t* queue, const llg_queue_t* packed_keys,
                            const llg_queue_value_array_t* value_keys,
                            int descending) {
    if (!queue || (!packed_keys == !value_keys))
        llg_container_fatal("malformed queue ordering");
    if (llg_method_sort_sv4_by_keys(queue->data, queue->element_ids, queue->size,
                                    packed_keys, value_keys, descending))
        llg_notify(queue->notify, queue->contents_dependency,
                   queue->shape_dependency, LLG_CONTAINER_CHANGED_CONTENTS);
}

void llg_dyn_value_sort_by_keys(llg_dyn_value_array_t* array,
                                const llg_queue_t* packed_keys,
                                const llg_queue_value_array_t* value_keys,
                                int descending) {
    if (!array || (!packed_keys == !value_keys))
        llg_container_fatal("malformed dynamic-array ordering");
    if (llg_method_sort_values_by_keys(array->data, array->size, packed_keys,
                                       value_keys, descending))
        llg_notify(array->notify, array->contents_dependency,
                   array->shape_dependency, LLG_CONTAINER_CHANGED_CONTENTS);
}

void llg_queue_value_sort_by_keys(llg_queue_value_array_t* queue,
                                  const llg_queue_t* packed_keys,
                                  const llg_queue_value_array_t* value_keys,
                                  int descending) {
    if (!queue || (!packed_keys == !value_keys))
        llg_container_fatal("malformed queue ordering");
    if (llg_method_sort_values_by_keys(queue->data, queue->size, packed_keys,
                                       value_keys, descending)) {
        llg_queue_value_invalidate_refs(queue);
        llg_notify(queue->notify, queue->contents_dependency,
                   queue->shape_dependency, LLG_CONTAINER_CHANGED_CONTENTS);
    }
}
