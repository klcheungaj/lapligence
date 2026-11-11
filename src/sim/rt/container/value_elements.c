/* ---- Whole elements and in-place element members (SIM-006) -------------- */

const llg_value_desc_t* llg_value_element_desc(const llg_value_desc_t* element,
                                               size_t count) {
    const llg_value_desc_t* desc = element;
    for (size_t i = 1; desc && i < count; ++i)
        desc = desc->kind == LLG_VALUE_CONTAINER ? desc->element : NULL;
    return desc;
}

llg_value_t* llg_dyn_value_element(llg_dyn_value_array_t* array,
                                   const sv4_t* indices, size_t count) {
    return array ? llg_dyn_value_nested_at(array, indices, count) : NULL;
}

llg_value_t* llg_queue_value_element(llg_queue_value_array_t* queue,
                                     const sv4_t* indices, size_t count) {
    return queue ? llg_queue_value_nested_at(queue, indices, count) : NULL;
}

/* An index of a queue or dynamic array of unpacked arrays that selects no
 * element; `size` is the array's. Out of line: no other path calls it. */
static LLG_OUTLYING void llg_value_cell_invalid(const char* operation,
                                                const char* kind, sv4_t index,
                                                size_t size, int queue_write) {
    int64_t value = 0;
    if (llg_index_stop_hook) {
        char detail[160];
        if (sv4_is_unknown(index))
            snprintf(detail, sizeof(detail), "the index has X or Z bits");
        else if (size == 0)
            snprintf(detail, sizeof(detail), "the %s is empty", kind);
        else if (sv4_to_index_i64(index, &value))
            snprintf(detail, sizeof(detail),
                     "index %lld is outside the valid range 0 to %llu",
                     (long long)value, (unsigned long long)(size - 1));
        else
            snprintf(detail, sizeof(detail),
                     "the index is outside the valid range 0 to %llu",
                     (unsigned long long)(size - 1));
        llg_index_stop_hook(operation, detail);
        return;
    }
    if (queue_write) llg_container_warning("invalid queue write index");
}

LLG_OUTLYING llg_value_t* llg_dyn_value_cell_miss(
    const llg_dyn_value_array_t* array, sv4_t index, int write) {
    llg_value_cell_invalid(write ? "dynamic-array element write"
                                 : "dynamic-array element read",
                           "dynamic array", index, array ? array->size : 0, 0);
    return NULL;
}

LLG_OUTLYING llg_value_t* llg_queue_value_cell_miss(
    llg_queue_value_array_t* queue, sv4_t index, int write) {
    int64_t value = 0;
    if (write && queue && sv4_to_index_i64(index, &value) && value >= 0 &&
        (uint64_t)value == (uint64_t)queue->size) {
        llg_value_t initial = {0};
        llg_native_value_init(&initial, queue->element);
        int written = llg_queue_value_set_element(queue, &index, 1, &initial);
        llg_native_value_destroy(&initial);
        return written ? llg_queue_value_element(queue, &index, 1) : NULL;
    }
    llg_value_cell_invalid(write ? "queue element write" : "queue element read",
                           "queue", index, queue ? queue->size : 0, write);
    return NULL;
}

LLG_OUTLYING void llg_queue_value_store_miss(
    const llg_queue_value_array_t* queue, sv4_t index) {
    int64_t value = 0;
    if (sv4_to_index_i64(index, &value) && value >= 0 &&
        (uint64_t)value == (uint64_t)queue->size)
        return;
    llg_value_cell_invalid("queue element write", "queue", index, queue->size,
                           1);
}

/* Insert a missing outer entry with its Table 6-7 initial value, as a write
 * through a missing key does (SV 7.8). Returns 0 for an invalid key. */
static int llg_assoc_value_create_entry(llg_assoc_value_t* array,
                                        sv4_t* integral_key,
                                        const void* string_key,
                                        size_t string_length) {
    llg_value_t initial = {0};
    if (!llg_value_try_default_mode(&initial, array->element, 1))
        llg_container_fatal("container allocation failed");
    int change = 0;
    int result = llg_assoc_value_set_source(array, integral_key, string_key,
                                            string_length, &initial, &change);
    llg_value_drop(&initial);
    llg_notify(array->notify, array->contents_dependency,
               array->shape_dependency, change);
    return result;
}

llg_value_t* llg_assoc_value_element_integral(llg_assoc_value_t* array,
                                              const sv4_t* indices,
                                              size_t count, int create) {
    if (!array || !indices || count == 0 ||
        array->key_kind != LLG_ASSOC_INTEGRAL)
        return NULL;
    sv4_t normalized = SV4_EMPTY;
    llg_value_t* value = NULL;
    int valid = llg_assoc_value_normalize_key(array, indices[0], &normalized);
    if (valid) {
        int found = 0;
        size_t position =
            llg_assoc_value_integral_position(array, normalized, &found);
        if (!found && create &&
            llg_assoc_value_create_entry(array, &normalized, NULL, 0))
            position = llg_assoc_value_integral_position(array, normalized,
                                                         &found);
        if (found) value = &array->entries[position].value;
    }
    if (!value) {
        if (create) {
            if (!valid)
                llg_invalid_key_access("associative-array write", "invalid associative-array key write");
        } else {
            llg_assoc_read_miss(valid, array->has_default_value);
        }
    }
    sv4_destroy(&normalized);
    for (size_t i = 1; value && i < count; ++i)
        value = llg_nested_seq_value_at(value, indices[i]);
    return value;
}

llg_value_t* llg_assoc_value_element_string(llg_assoc_value_t* array,
                                            const void* key, size_t length,
                                            int create) {
    if (!array || array->key_kind != LLG_ASSOC_STRING) return NULL;
    int found = 0;
    size_t position =
        llg_assoc_value_string_position(array, key, length, &found);
    if (!found && create &&
        llg_assoc_value_create_entry(array, NULL, key, length))
        position = llg_assoc_value_string_position(array, key, length, &found);
    if (!found && !create) llg_assoc_read_miss(1, array->has_default_value);
    return found ? &array->entries[position].value : NULL;
}

llg_value_t* llg_nested_seq_write_at(llg_value_t* owner, sv4_t index,
                                     int* change) {
    if (!owner || !owner->desc || owner->desc->kind != LLG_VALUE_CONTAINER ||
        owner->desc->element->kind == LLG_VALUE_PACKED)
        return NULL;
    llg_nested_seq_t* seq = owner->value.container;
    size_t size = llg_nested_seq_size(seq);
    int queue = owner->desc->item_count != LLG_VALUE_CONTAINER_DYNAMIC;
    size_t native;
    if (!llg_index(index, size, queue, &native)) return NULL;
    if (native == size) {
        /* An empty nested sequence has no storage of its own yet. */
        if (!seq) seq = owner->value.container = llg_nested_seq_new(owner->desc);
        llg_value_t initial = {0};
        if (!llg_value_try_default_mode(&initial,
                                        seq->array.queue_values.element, 1))
            llg_container_fatal("container allocation failed");
        /* The queue's own write appends; at its bound it warns instead. */
        int appended = 0;
        int written = llg_queue_value_set_source(&seq->array.queue_values,
                                                 index, &initial, &appended);
        llg_value_drop(&initial);
        if (!written) return NULL;
        *change |= LLG_CONTAINER_CHANGED_CONTENTS;
    }
    return seq->kind == LLG_NESTED_SEQ_QUEUE_VALUES
        ? &seq->array.queue_values.data[native]
        : &seq->array.dyn_values.data[native];
}

/* The inner levels of `llg_*_extend_nested`, below the outer element. Only
 * a queue level takes a position one past its end. */
static int llg_value_extend_path(llg_value_t* value, const sv4_t* indices,
                                 size_t count, int* change) {
    for (size_t level = 0; level < count; ++level) {
        if (!value || !value->desc || value->desc->kind != LLG_VALUE_CONTAINER)
            return 0;
        if (!llg_nested_seq_holds_packed(value)) {
            value = llg_nested_seq_write_at(value, indices[level], change);
            continue;
        }
        /* Packed elements are the last level: nothing lies below one. */
        if (level + 1 != count) return 0;
        llg_nested_seq_t* seq = value->value.container;
        size_t size = llg_nested_seq_size(seq);
        int queue = value->desc->item_count != LLG_VALUE_CONTAINER_DYNAMIC;
        size_t native;
        if (!llg_index(indices[level], size, queue, &native)) return 0;
        if (native < size) return 1;
        if (!seq) seq = value->value.container = llg_nested_seq_new(value->desc);
        sv4_t initial = llg_element_default(seq->array.queue.element_width,
                                            seq->array.queue.element_signed,
                                            seq->array.queue.element_two_state);
        llg_queue_push_back(&seq->array.queue, initial);
        sv4_destroy(&initial);
        /* A bounded queue at its bound takes no element. */
        if (llg_nested_seq_size(seq) == size) return 0;
        *change |= LLG_CONTAINER_CHANGED_CONTENTS;
        return 1;
    }
    return value != NULL;
}

int llg_dyn_value_extend_nested(llg_dyn_value_array_t* array,
                                const sv4_t* indices, size_t count) {
    if (!array || !indices || count < 2) return 0;
    int change = 0;
    int result = llg_value_extend_path(llg_dyn_value_at(array, indices[0]),
                                       indices + 1, count - 1, &change);
    llg_notify(array->notify, array->contents_dependency,
               array->shape_dependency, change);
    return result;
}

int llg_queue_value_extend_nested(llg_queue_value_array_t* queue,
                                  const sv4_t* indices, size_t count) {
    if (!queue || !indices || count < 2) return 0;
    int change = 0;
    size_t native;
    if (!llg_index(indices[0], queue->size, 1, &native)) return 0;
    if (native == queue->size) {
        llg_value_t initial = {0};
        if (!llg_value_try_default_mode(&initial, queue->element, 1))
            llg_container_fatal("container allocation failed");
        int appended =
            llg_queue_value_set_source(queue, indices[0], &initial, &change);
        llg_value_drop(&initial);
        if (!appended) {
            llg_notify(queue->notify, queue->contents_dependency,
                       queue->shape_dependency, change);
            return 0;
        }
    }
    int result = llg_value_extend_path(&queue->data[native], indices + 1,
                                       count - 1, &change);
    llg_notify(queue->notify, queue->contents_dependency,
               queue->shape_dependency, change);
    return result;
}

int llg_assoc_value_extend_nested_integral(llg_assoc_value_t* array,
                                           const sv4_t* indices, size_t count) {
    if (!array || !indices || count < 2) return 0;
    llg_value_t* outer = llg_assoc_value_element_integral(array, indices, 1, 1);
    int change = 0;
    int result = llg_value_extend_path(outer, indices + 1, count - 1, &change);
    llg_notify(array->notify, array->contents_dependency,
               array->shape_dependency, change);
    return result;
}

/* A write through a borrowed element pointer happens after the locator
 * returned; these publish it once the store is complete. */
void llg_dyn_value_touch(llg_dyn_value_array_t* array) {
    llg_notify(array->notify, array->contents_dependency,
               array->shape_dependency, LLG_CONTAINER_CHANGED_CONTENTS);
}

void llg_queue_value_touch(llg_queue_value_array_t* queue) {
    llg_notify(queue->notify, queue->contents_dependency,
               queue->shape_dependency, LLG_CONTAINER_CHANGED_CONTENTS);
}

void llg_assoc_value_touch(llg_assoc_value_t* array) {
    llg_notify(array->notify, array->contents_dependency,
               array->shape_dependency, LLG_CONTAINER_CHANGED_CONTENTS);
}

size_t llg_value_container_size(const llg_value_t* value) {
    if (value && value->desc && value->desc->kind == LLG_VALUE_ASSOC)
        return llg_nested_assoc_size(value->value.assoc);
    if (!value || !value->desc || value->desc->kind != LLG_VALUE_CONTAINER)
        return 0;
    return llg_nested_seq_size(value->value.container);
}

void llg_value_element_read(llg_value_t* dst, const llg_value_t* element) {
    if (!dst || !dst->desc) llg_container_fatal("element read requires a value");
    if (element) {
        if (!llg_value_desc_compatible(dst->desc, element->desc))
            llg_container_fatal("element read has an incompatible value type");
        llg_value_copy(dst, dst->desc, element);
        return;
    }
    /* A missing element reads as the Table 7-1 default (null handles). */
    const llg_value_desc_t* desc = dst->desc;
    llg_value_drop(dst);
    if (!llg_value_try_default(dst, desc))
        llg_container_fatal("container allocation failed");
}

/* Replace one existing element with a converted copy of `value`, notifying
 * readers. Returns 0 (no write) for an invalid index or missing key. */
static int llg_value_element_store(llg_value_t* target, const llg_value_t* value,
                                   int* change) {
    if (!target) return 0;
    if (!value || !llg_value_desc_compatible(target->desc, value->desc))
        llg_container_fatal("element write has an incompatible value type");
    if (llg_value_equal(target, value)) return 1;
    llg_value_copy(target, target->desc, value);
    *change |= LLG_CONTAINER_CHANGED_CONTENTS;
    return 1;
}

/* A record element written in place: each store reports whether the leaf
 * changed, so the element publishes once and only when some leaf differs. */
int llg_value_item_set_packed(llg_value_t* item, sv4_t value) {
    if (sv4_same(item->value.packed, value)) return 0;
    sv4_copy(&item->value.packed, &value);
    return 1;
}

int llg_value_item_set_real(llg_value_t* item, double value) {
    if (llg_value_real_same(item->value.real, value)) return 0;
    item->value.real = value;
    return 1;
}

static int value_item_same_string(const llg_value_t* item, const llg_string_t* value) {
    return item->value.string.len == value->len &&
           (!value->len ||
            memcmp(item->value.string.data, value->data, value->len) == 0);
}

int llg_value_item_set_string(llg_value_t* item, const llg_string_t* value) {
    if (value_item_same_string(item, value)) return 0;
    llg_string_t copy = llg_string_clone(value);
    llg_string_destroy(&item->value.string);
    item->value.string = copy;
    return 1;
}

int llg_value_item_take_string(llg_value_t* item, llg_string_t value) {
    if (value_item_same_string(item, &value)) {
        llg_string_destroy(&value);
        return 0;
    }
    llg_string_destroy(&item->value.string);
    item->value.string = value;
    return 1;
}

int llg_value_item_set_handle(llg_value_t* item, void* value) {
    return llg_value_store_handle(item, value);
}

int llg_dyn_value_set_element(llg_dyn_value_array_t* array,
                              const sv4_t* indices, size_t count,
                              const llg_value_t* value) {
    int change = 0;
    int result = llg_value_element_store(
        llg_dyn_value_nested_target(array, indices, count, &change), value,
        &change);
    llg_notify(array->notify, array->contents_dependency,
               array->shape_dependency, change);
    return result;
}

int llg_queue_value_set_element(llg_queue_value_array_t* queue,
                                const sv4_t* indices, size_t count,
                                const llg_value_t* value) {
    if (count == 1 &&
        (!value || !llg_value_desc_compatible(queue->element, value->desc)))
        llg_container_fatal("queue write has an incompatible value type");
    int change = 0;
    int result;
    if (count == 1) {
        /* `q[$+1] = v` appends (SV 7.10.1). */
        result = llg_queue_value_set_source(queue, indices[0], value, &change);
    } else {
        result = llg_value_element_store(
            llg_queue_value_nested_target(queue, indices, count, &change),
            value, &change);
        if (result) llg_queue_value_invalidate_refs(queue);
    }
    llg_notify(queue->notify, queue->contents_dependency,
               queue->shape_dependency, change);
    return result;
}

void llg_queue_value_push_value(llg_queue_value_array_t* queue, int back,
                                const llg_value_t* value) {
    if (!value || !llg_value_desc_compatible(queue->element, value->desc))
        llg_container_fatal("queue push has an incompatible value type");
    int change = 0;
    if (back) llg_queue_value_append(queue, value, &change);
    else llg_queue_value_prepend(queue, value, &change);
    llg_notify(queue->notify, queue->contents_dependency,
               queue->shape_dependency, change);
}

int llg_queue_value_insert_value(llg_queue_value_array_t* queue, sv4_t index,
                                 const llg_value_t* value) {
    if (!value || !llg_value_desc_compatible(queue->element, value->desc))
        llg_container_fatal("queue insert has an incompatible value type");
    int change = 0;
    int result = llg_queue_value_insert_source(queue, index, value, &change);
    llg_notify(queue->notify, queue->contents_dependency,
               queue->shape_dependency, change);
    return result;
}

int llg_assoc_value_set_element_integral(llg_assoc_value_t* array,
                                         const sv4_t* indices, size_t count,
                                         const llg_value_t* value) {
    if (count == 1 &&
        (!value || !llg_value_desc_compatible(array->element, value->desc)))
        llg_container_fatal("associative write has an incompatible value type");
    int change = 0;
    int result;
    if (count == 1) {
        sv4_t normalized = SV4_EMPTY;
        result = llg_assoc_value_normalize_key(array, indices[0], &normalized);
        if (!result)
            llg_invalid_key_access("associative-array write", "invalid associative-array key write");
        else
            result = llg_assoc_value_set_source(array, &normalized, NULL, 0,
                                                value, &change);
        sv4_destroy(&normalized);
    } else {
        result = llg_value_element_store(
            llg_assoc_value_nested_target_integral(array, indices, count,
                                                   &change),
            value, &change);
    }
    llg_notify(array->notify, array->contents_dependency,
               array->shape_dependency, change);
    return result;
}

int llg_assoc_value_set_element_string(llg_assoc_value_t* array,
                                       const void* key, size_t length,
                                       const llg_value_t* value) {
    if (!value || !llg_value_desc_compatible(array->element, value->desc))
        llg_container_fatal("associative write has an incompatible value type");
    int change = 0;
    int result = llg_assoc_value_set_source(array, NULL, key, length, value,
                                            &change);
    llg_notify(array->notify, array->contents_dependency,
               array->shape_dependency, change);
    return result;
}

void llg_queue_value_pop_value(llg_queue_value_array_t* queue, int back,
                               llg_value_t* dst) {
    llg_value_t removed = {0};
    int taken = llg_queue_value_take(queue, back, &removed);
    llg_value_element_read(dst, taken ? &removed : NULL);
    llg_value_drop(&removed);
    if (taken) llg_queue_value_pop_notify(queue);
}

/* ---- Container members of record values (SIM-007) ------------------------ */

static void llg_value_item_check(const llg_value_t* item) {
    if (!item || !item->desc || item->desc->kind != LLG_VALUE_CONTAINER)
        llg_container_fatal("record container member has no container slot");
}

/* The elements of a record member slot: the sequence nested in it (none for
 * the null default), or the items of a fixed member array moved as one
 * container. */
static llg_seq_view_t llg_value_item_view(const llg_value_t* item) {
    if (item && item->desc && item->desc->kind == LLG_VALUE_FIXED_ARRAY) {
        llg_seq_view_t view = {0};
        view.values = item->value.items;
        view.size = item->desc->item_count;
        view.element = item->desc->element;
        return view;
    }
    llg_value_item_check(item);
    return llg_nested_seq_view(item->value.container);
}

/* Whether a slot is a fixed member array of `count` items, moved cell by cell
 * into its own items. */
static int llg_value_item_is_fixed(const llg_value_t* item, size_t count) {
    if (!item || !item->desc || item->desc->kind != LLG_VALUE_FIXED_ARRAY)
        return 0;
    if (item->desc->item_count != count)
        llg_container_fatal("fixed member array has another size than its value");
    return 1;
}

/* Packed payloads of a slot's elements, borrowed. A fixed member array keeps
 * values: `*gathered` then owns the returned array (not its items). */
static const sv4_t* llg_value_item_packed(const llg_value_t* item,
                                          size_t* count, sv4_t** gathered) {
    llg_seq_view_t view = llg_value_item_view(item);
    *count = view.size;
    *gathered = NULL;
    if (!view.values) return view.packed;
    if (view.element->kind != LLG_VALUE_PACKED)
        llg_container_fatal("packed container member has a non-packed element");
    sv4_t* values = llg_alloc_items(view.size, sizeof(*values));
    for (size_t i = 0; i < view.size; ++i)
        values[i] = view.values[i].value.packed;
    *gathered = values;
    return values;
}

void llg_value_item_to_dyn(llg_dyn_array_t* dst, const llg_value_t* item) {
    size_t count = 0;
    sv4_t* gathered = NULL;
    const sv4_t* values = llg_value_item_packed(item, &count, &gathered);
    llg_dyn_assign_values(dst, values, count);
    free(gathered);
}

void llg_value_item_to_queue(llg_queue_t* dst, const llg_value_t* item) {
    size_t count = 0;
    sv4_t* gathered = NULL;
    const sv4_t* values = llg_value_item_packed(item, &count, &gathered);
    llg_queue_assign_values(dst, values, count);
    free(gathered);
}

void llg_value_item_to_dyn_value(llg_dyn_value_array_t* dst,
                                 const llg_value_t* item) {
    llg_seq_view_t view = llg_value_item_view(item);
    if (!view.size) {
        llg_dyn_value_delete(dst);
        return;
    }
    if (!view.values)
        llg_container_fatal("incompatible recursive container element types");
    llg_dyn_value_array_t source = {0};
    source.data = (llg_value_t*)view.values;
    source.size = view.size;
    source.element = view.element;
    llg_dyn_value_copy(dst, &source);
}

void llg_value_item_to_queue_value(llg_queue_value_array_t* dst,
                                   const llg_value_t* item) {
    llg_seq_view_t view = llg_value_item_view(item);
    if (!view.size) {
        llg_queue_value_delete(dst);
        return;
    }
    if (!view.values)
        llg_container_fatal("incompatible recursive queue element types");
    llg_queue_value_array_t source = {0};
    source.data = (llg_value_t*)view.values;
    source.size = view.size;
    source.element = view.element;
    llg_queue_value_copy(dst, &source);
}

void llg_value_item_from_dyn(llg_value_t* item, const llg_dyn_array_t* src) {
    if (llg_value_item_is_fixed(item, src->size)) {
        for (size_t i = 0; i < src->size; ++i)
            sv4_copy(&item->value.items[i].value.packed, &src->data[i]);
        return;
    }
    llg_value_item_check(item);
    (void)llg_nested_seq_store(item, llg_seq_view_of_dyn(src));
}

void llg_value_item_from_queue(llg_value_t* item, const llg_queue_t* src) {
    llg_value_item_check(item);
    (void)llg_nested_seq_store(item, llg_seq_view_of_queue(src));
}

void llg_value_item_from_dyn_value(llg_value_t* item,
                                   const llg_dyn_value_array_t* src) {
    if (llg_value_item_is_fixed(item, src->size)) {
        for (size_t i = 0; i < src->size; ++i)
            llg_value_copy(&item->value.items[i], item->desc->element, &src->data[i]);
        return;
    }
    llg_value_item_check(item);
    (void)llg_nested_seq_store(item, llg_seq_view_of_dyn_value(src));
}

void llg_value_item_from_queue_value(llg_value_t* item,
                                     const llg_queue_value_array_t* src) {
    llg_value_item_check(item);
    (void)llg_nested_seq_store(item, llg_seq_view_of_queue_value(src));
}
