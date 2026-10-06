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
                llg_container_warning("invalid associative-array key write");
        } else {
            llg_assoc_read_miss(valid, array->has_default_value);
        }
    }
    sv4_destroy(&normalized);
    for (size_t i = 1; value && i < count; ++i) {
        if (value->desc->kind != LLG_VALUE_CONTAINER || !value->value.container)
            return NULL;
        value = llg_dyn_value_at(value->value.container, indices[i]);
    }
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
    if (!value || !value->desc || value->desc->kind != LLG_VALUE_CONTAINER ||
        !value->value.container)
        return 0;
    return llg_dyn_value_size(value->value.container);
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

int llg_dyn_value_set_element(llg_dyn_value_array_t* array,
                              const sv4_t* indices, size_t count,
                              const llg_value_t* value) {
    int change = 0;
    int result = llg_value_element_store(
        llg_dyn_value_nested_at(array, indices, count), value, &change);
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
            llg_queue_value_nested_at(queue, indices, count), value, &change);
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
            llg_container_warning("invalid associative-array key write");
        else
            result = llg_assoc_value_set_source(array, &normalized, NULL, 0,
                                                value, &change);
        sv4_destroy(&normalized);
    } else {
        result = llg_value_element_store(
            llg_assoc_value_nested_at_integral(array, indices, count), value,
            &change);
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

/* The nested dynamic array of a record member slot, or NULL for the empty
 * (null) default. */
static const llg_dyn_value_array_t* llg_value_item_container(
    const llg_value_t* item) {
    if (!item || !item->desc || item->desc->kind != LLG_VALUE_CONTAINER)
        llg_container_fatal("record container member has no container slot");
    return item->value.container;
}

/* Borrowed packed payloads of a nested array of packed elements. The caller
 * frees the returned array without destroying its items. */
static sv4_t* llg_value_item_packed_view(const llg_dyn_value_array_t* nested,
                                         size_t* count) {
    *count = nested ? nested->size : 0;
    if (nested && nested->element->kind != LLG_VALUE_PACKED)
        llg_container_fatal("packed container member has a non-packed element");
    sv4_t* values = llg_alloc_items(*count, sizeof(*values));
    for (size_t i = 0; i < *count; ++i) values[i] = nested->data[i].value.packed;
    return values;
}

void llg_value_item_to_dyn(llg_dyn_array_t* dst, const llg_value_t* item) {
    size_t count = 0;
    sv4_t* values =
        llg_value_item_packed_view(llg_value_item_container(item), &count);
    llg_dyn_assign_values(dst, values, count);
    free(values);
}

void llg_value_item_to_queue(llg_queue_t* dst, const llg_value_t* item) {
    size_t count = 0;
    sv4_t* values =
        llg_value_item_packed_view(llg_value_item_container(item), &count);
    llg_queue_assign_values(dst, values, count);
    free(values);
}

void llg_value_item_to_dyn_value(llg_dyn_value_array_t* dst,
                                 const llg_value_t* item) {
    const llg_dyn_value_array_t* nested = llg_value_item_container(item);
    if (nested) {
        llg_dyn_value_copy(dst, nested);
    } else {
        llg_dyn_value_delete(dst);
    }
}

void llg_value_item_to_queue_value(llg_queue_value_array_t* dst,
                                   const llg_value_t* item) {
    const llg_dyn_value_array_t* nested = llg_value_item_container(item);
    if (!nested) {
        llg_queue_value_delete(dst);
        return;
    }
    if (!llg_value_desc_compatible(dst->element, nested->element))
        llg_container_fatal("incompatible recursive queue element types");
    size_t count = nested->size < dst->limit ? nested->size : dst->limit;
    llg_value_t* data = llg_alloc_items(count, sizeof(*data));
    if (count) memset(data, 0, count * sizeof(*data));
    for (size_t i = 0; i < count; ++i)
        llg_value_copy(&data[i], dst->element, &nested->data[i]);
    llg_queue_value_commit(dst, data, count);
    if (count != nested->size)
        llg_container_warning("bounded queue assignment discarded tail elements");
}

/* Replace a record member slot with `replacement`, keeping the null default
 * for an empty container so equal values stay identical. */
static void llg_value_item_store(llg_value_t* item, llg_value_t* replacement) {
    if (replacement->value.container && !replacement->value.container->size) {
        llg_dyn_value_destroy(replacement->value.container);
        free(replacement->value.container);
        replacement->value.container = NULL;
    }
    llg_value_drop(item);
    *item = *replacement;
}

void llg_value_item_from_dyn(llg_value_t* item, const llg_dyn_array_t* src) {
    llg_value_item_container(item);
    llg_value_t replacement = llg_value_from_packed_container(item->desc, src);
    llg_value_item_store(item, &replacement);
}

void llg_value_item_from_queue(llg_value_t* item, const llg_queue_t* src) {
    llg_value_item_container(item);
    /* A borrowed view of the queue's contiguous payloads. */
    llg_dyn_array_t view = {0};
    view.data = src->data;
    view.size = src->size;
    view.element_width = src->element_width;
    view.element_signed = src->element_signed;
    view.element_two_state = src->element_two_state;
    llg_value_t replacement = llg_value_from_packed_container(item->desc, &view);
    llg_value_item_store(item, &replacement);
}

void llg_value_item_from_dyn_value(llg_value_t* item,
                                   const llg_dyn_value_array_t* src) {
    llg_value_item_container(item);
    llg_value_t replacement = llg_value_from_container(item->desc, src);
    llg_value_item_store(item, &replacement);
}

void llg_value_item_from_queue_value(llg_value_t* item,
                                     const llg_queue_value_array_t* src) {
    llg_value_item_container(item);
    /* A borrowed view of the queue's contiguous elements. */
    llg_dyn_value_array_t view = {0};
    view.data = src->data;
    view.size = src->size;
    view.element = src->element;
    llg_value_t replacement = llg_value_from_container(item->desc, &view);
    llg_value_item_store(item, &replacement);
}
