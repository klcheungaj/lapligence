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
