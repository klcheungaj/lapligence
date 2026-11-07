
static llg_value_t* llg_dyn_value_at(const llg_dyn_value_array_t* array,
                                     sv4_t index) {
    size_t native;
    if (!llg_index(index, array->size, 0, &native)) return NULL;
    return &array->data[native];
}

static llg_value_t* llg_dyn_value_nested_at(
    const llg_dyn_value_array_t* array, const sv4_t* indices, size_t count) {
    if (!indices || count == 0) return NULL;
    llg_value_t* value = llg_dyn_value_at(array, indices[0]);
    for (size_t i = 1; value && i < count; ++i) {
        if (value->desc->kind != LLG_VALUE_CONTAINER ||
            !value->value.container)
            return NULL;
        value = llg_dyn_value_at(value->value.container, indices[i]);
    }
    return value;
}

static const llg_value_desc_t* llg_dyn_value_nested_desc(
    const llg_value_desc_t* desc, size_t count) {
    if (!desc || count == 0) return NULL;
    for (size_t i = 1; i < count; ++i) {
        if (desc->kind != LLG_VALUE_CONTAINER) return NULL;
        desc = desc->element;
    }
    return desc;
}

static void llg_dyn_value_commit(llg_dyn_value_array_t* array,
                                 llg_value_t* data, size_t size) {
    int shape_changed = array->size != size;
    int contents_changed = shape_changed;
    if (!contents_changed) {
        for (size_t i = 0; i < size; ++i) {
            if (!llg_value_equal(&array->data[i], &data[i])) {
                contents_changed = 1;
                break;
            }
        }
    }
    llg_container_notify_fn notify = array->notify;
    sv4_t* contents_dependency = array->contents_dependency;
    sv4_t* shape_dependency = array->shape_dependency;
    if (array->references)
        llg_native_refs_rekey(&array->references, array->data, data, size);
    for (size_t i = 0; i < array->size; ++i)
        llg_value_drop(&array->data[i]);
    free(array->data);
    array->data = data;
    array->size = size;
    llg_notify(notify, contents_dependency, shape_dependency,
               (contents_changed ? LLG_CONTAINER_CHANGED_CONTENTS : 0) |
                   (shape_changed ? LLG_CONTAINER_CHANGED_SHAPE : 0));
}

void llg_dyn_value_init(llg_dyn_value_array_t* array,
                        const llg_value_desc_t* element) {
    if (!element) llg_container_fatal("missing recursive container value descriptor");
    memset(array, 0, sizeof(*array));
    array->element = element;
}

void llg_dyn_value_destroy(llg_dyn_value_array_t* array) {
    llg_native_refs_outdate_all(&array->references);
    for (size_t i = 0; i < array->size; ++i)
        llg_value_drop(&array->data[i]);
    free(array->data);
    memset(array, 0, sizeof(*array));
}

void llg_dyn_value_delete(llg_dyn_value_array_t* array) {
    int changed = array->size != 0;
    llg_native_refs_outdate_all(&array->references);
    for (size_t i = 0; i < array->size; ++i)
        llg_value_drop(&array->data[i]);
    free(array->data);
    array->data = NULL;
    array->size = 0;
    llg_notify(array->notify, array->contents_dependency,
               array->shape_dependency,
               changed ? LLG_CONTAINER_CHANGED_CONTENTS |
                            LLG_CONTAINER_CHANGED_SHAPE
                       : 0);
}

static void llg_dyn_value_new_count(llg_dyn_value_array_t* dst, size_t size,
                              const llg_dyn_value_array_t* initializer) {
    if (initializer &&
        !llg_value_desc_compatible(dst->element, initializer->element))
        llg_container_fatal("incompatible recursive container element types");
    llg_value_t* data = llg_alloc_items(size, sizeof(*data));
    if (size) memset(data, 0, size * sizeof(*data));
    size_t copied = initializer && initializer->size < size
        ? initializer->size
        : size;
    if (!initializer) copied = 0;
    for (size_t i = 0; i < size; ++i) {
        const llg_value_t* source =
            initializer && i < copied ? &initializer->data[i] : NULL;
        llg_value_copy(&data[i], dst->element, source);
    }
    llg_dyn_value_commit(dst, data, size);
}

void llg_dyn_value_new(llg_dyn_value_array_t* dst, sv4_t requested_size,
                 const llg_dyn_value_array_t* initializer) {
    size_t size = llg_checked_count(llg_dynamic_size(requested_size), sizeof(llg_value_t));
    llg_dyn_value_new_count(dst, size, initializer);
}

void llg_dyn_value_copy(llg_dyn_value_array_t* dst,
                        const llg_dyn_value_array_t* src) {
    if (dst == src) return;
    /* An array of the same size with no element reference cannot observe its
     * storage being kept: only the elements that differ change, so a repeated
     * copy allocates nothing. */
    if (dst->size == src->size && !dst->references) {
        int kind = dst->element->kind, changed = 0;
        int handles = kind == LLG_VALUE_CHANDLE || kind == LLG_VALUE_OPAQUE ||
                      kind == LLG_VALUE_EVENT || kind == LLG_VALUE_PROCESS;
        if (handles && src->element->kind == kind) {
            llg_value_t* target = dst->data;
            const llg_value_t* source = src->data;
            size_t size = dst->size, i = 0;
            while (i < size) {
                /* A run of equal handles costs only its comparisons. */
                if (size - i >= 8) {
                    uintptr_t differ = 0;
                    for (size_t j = 0; j < 8; ++j)
                        differ |= (uintptr_t)target[i + j].value.handle ^
                                  (uintptr_t)source[i + j].value.handle;
                    if (!differ) {
                        i += 8;
                        continue;
                    }
                }
                if (target[i].value.handle != source[i].value.handle) {
                    llg_value_copy(&target[i], dst->element, &source[i]);
                    changed = 1;
                }
                ++i;
            }
        } else if (llg_value_desc_compatible(dst->element, src->element)) {
            for (size_t i = 0; i < dst->size; ++i) {
                if (llg_value_equal(&dst->data[i], &src->data[i])) continue;
                llg_value_copy(&dst->data[i], dst->element, &src->data[i]);
                changed = 1;
            }
        } else goto replace;
        if (changed)
            llg_notify(dst->notify, dst->contents_dependency, dst->shape_dependency,
                       LLG_CONTAINER_CHANGED_CONTENTS);
        return;
    }
replace:;
    size_t size = llg_checked_count((uint64_t)src->size, sizeof(llg_value_t));
    llg_dyn_value_new_count(dst, size, src);
}

void llg_dyn_value_copy_range(llg_dyn_value_array_t* dst, sv4_t dst_start,
                              const llg_dyn_value_array_t* src,
                              sv4_t src_start, uint64_t count) {
    if (!llg_value_desc_compatible(dst->element, src->element))
        llg_container_fatal("incompatible recursive container element types");
    int64_t to = 0;
    if (!sv4_to_index_i64(dst_start, &to)) return;
    int64_t from = 0;
    int from_known = sv4_to_index_i64(src_start, &from);
    size_t n = llg_checked_count(count, sizeof(llg_value_t));
    /* Snapshot every selected source element (the default outside the
     * source) before the first write, so overlapping ranges of one array
     * read their old elements (SV 7.6). */
    llg_value_t* items = llg_alloc_items(n, sizeof(*items));
    if (n) memset(items, 0, n * sizeof(*items));
    for (size_t i = 0; i < n; ++i) {
        const llg_value_t* source = NULL;
        if (from_known && from >= 0 && (uint64_t)from <= SIZE_MAX - i &&
            (size_t)from + i < src->size)
            source = &src->data[(size_t)from + i];
        llg_value_copy(&items[i], dst->element, source);
    }
    int changed = 0;
    for (size_t i = 0; i < n; ++i) {
        if (to < 0 || (uint64_t)to > SIZE_MAX - i || (size_t)to + i >= dst->size)
            continue;
        llg_value_t* target = &dst->data[(size_t)to + i];
        if (llg_value_equal(target, &items[i])) continue;
        if (llg_native_refs_live) {
            llg_value_replace_in_place(target, &items[i]);
        } else {
            llg_value_drop(target);
            *target = items[i]; // exclusive ownership transfer
        }
        memset(&items[i], 0, sizeof(items[i]));
        changed = 1;
    }
    for (size_t i = 0; i < n; ++i) llg_value_drop(&items[i]);
    free(items);
    llg_notify(dst->notify, dst->contents_dependency, dst->shape_dependency,
               changed ? LLG_CONTAINER_CHANGED_CONTENTS : 0);
}

/* SV 11.4.11 combines aggregates element by element, recursively: record
 * members and fixed-array items merge item by item and a nested container
 * element by element, so only leaves (packed, real, string and handle
 * values) are compared. A leaf that is not known equal takes its default. */
static void llg_value_merge(llg_value_t* target, const llg_value_desc_t* desc,
                            const llg_value_t* a, const llg_value_t* b) {
    switch (desc->kind) {
        case LLG_VALUE_AGGREGATE:
        case LLG_VALUE_FIXED_ARRAY:
            llg_value_copy(target, desc, NULL);
            if (!target->value.items || !a->value.items || !b->value.items)
                return;
            for (size_t i = 0; i < desc->item_count; ++i)
                llg_value_merge(&target->value.items[i],
                                llg_value_item_desc(desc, i),
                                &a->value.items[i], &b->value.items[i]);
            return;
        case LLG_VALUE_CONTAINER: {
            llg_value_copy(target, desc, NULL);
            if (!a->value.container && !b->value.container) return;
            /* A null nested container is the empty default. */
            llg_dyn_value_array_t empty = {0};
            empty.element = desc->element;
            llg_dyn_value_array_t* merged =
                llg_alloc_items(1, sizeof(*merged));
            llg_dyn_value_init(merged, desc->element);
            llg_dyn_value_merge(merged,
                                a->value.container ? a->value.container : &empty,
                                b->value.container ? b->value.container : &empty);
            target->value.container = merged;
            return;
        }
        default:
            llg_value_copy(target, desc,
                           llg_value_equality(a, b, 0) == LLG_VALUE_EQUAL
                               ? a
                               : NULL);
            return;
    }
}

/* An element only one operand has merges with the element default, so a
 * nested container keeps its size with default leaves; every other leaf
 * kind takes its default either way. */
static void llg_value_merge_present(llg_value_t* target,
                                    const llg_value_desc_t* desc,
                                    const llg_value_t* present) {
    llg_value_t fallback = {0};
    llg_value_default(&fallback, desc);
    llg_value_merge(target, desc, &fallback, present);
    llg_value_drop(&fallback);
}

void llg_dyn_value_merge(llg_dyn_value_array_t* dst,
                         const llg_dyn_value_array_t* a,
                         const llg_dyn_value_array_t* b) {
    if (!llg_value_desc_compatible(dst->element, a->element) ||
        !llg_value_desc_compatible(dst->element, b->element))
        llg_container_fatal("incompatible recursive container element types");
    size_t size = a->size > b->size ? a->size : b->size;
    llg_value_t* data = llg_alloc_items(size, sizeof(*data));
    if (size) memset(data, 0, size * sizeof(*data));
    for (size_t i = 0; i < size; ++i) {
        if (i < a->size && i < b->size)
            llg_value_merge(&data[i], dst->element, &a->data[i], &b->data[i]);
        else
            llg_value_merge_present(&data[i], dst->element,
                                    i < a->size ? &a->data[i] : &b->data[i]);
    }
    llg_dyn_value_commit(dst, data, size);
}

void llg_dyn_value_assign_reals(llg_dyn_value_array_t* dst,
                                const double* values, size_t count) {
    if (!dst->element || dst->element->kind != LLG_VALUE_REAL)
        llg_container_fatal("real assignment used with a non-real container");
    llg_value_t* data = llg_alloc_items(count, sizeof(*data));
    if (count) memset(data, 0, count * sizeof(*data));
    for (size_t i = 0; i < count; ++i) {
        llg_value_default(&data[i], dst->element);
        data[i].value.real = llg_value_real_convert(dst->element, values[i]);
    }
    llg_dyn_value_commit(dst, data, count);
}

void llg_dyn_value_assign_strings(llg_dyn_value_array_t* dst,
                                  llg_string_t* values, size_t count) {
    if (!dst->element || dst->element->kind != LLG_VALUE_STRING)
        llg_container_fatal("string assignment used with a non-string container");
    llg_value_t* data = llg_alloc_items(count, sizeof(*data));
    if (count) memset(data, 0, count * sizeof(*data));
    for (size_t i = 0; i < count; ++i) {
        llg_value_default(&data[i], dst->element);
        data[i].value.string = values[i];
        values[i] = (llg_string_t){0};
    }
    llg_dyn_value_commit(dst, data, count);
}

void llg_dyn_value_assign_chandles(llg_dyn_value_array_t* dst,
                                   void* const* values, size_t count) {
    if (!dst->element ||
        !llg_value_is_handle_kind(dst->element))
        llg_container_fatal("handle assignment used with an incompatible container");
    llg_value_t* data = llg_alloc_items(count, sizeof(*data));
    if (count) memset(data, 0, count * sizeof(*data));
    for (size_t i = 0; i < count; ++i) {
        llg_value_default(&data[i], dst->element);
        llg_value_store_handle(&data[i], values[i]);
    }
    llg_dyn_value_commit(dst, data, count);
}

size_t llg_dyn_value_size(const llg_dyn_value_array_t* array) {
    return array->size;
}

double llg_dyn_value_get_real(const llg_dyn_value_array_t* array, sv4_t index) {
    llg_value_t* value = llg_dyn_value_at(array, index);
    return value && value->desc->kind == LLG_VALUE_REAL
        ? value->value.real
        : 0.0;
}

static llg_string_t dyn_value_read_string(const llg_value_t* value) {
    return value && value->desc->kind == LLG_VALUE_STRING
        ? llg_string_clone(&value->value.string)
        : (llg_string_t){0};
}

static void* dyn_value_read_chandle(const llg_value_t* value) {
    return value && llg_value_is_handle_kind(value->desc)
        ? value->value.handle
        : NULL;
}

llg_string_t llg_dyn_value_get_string(const llg_dyn_value_array_t* array,
                                      sv4_t index) {
    return dyn_value_read_string(llg_dyn_value_at(array, index));
}

void* llg_dyn_value_get_chandle(const llg_dyn_value_array_t* array,
                                sv4_t index) {
    return dyn_value_read_chandle(llg_dyn_value_at(array, index));
}

sv4_t llg_dyn_value_get_nested(const llg_dyn_value_array_t* array,
                               const sv4_t* indices, size_t count) {
    llg_value_t* value = llg_dyn_value_nested_at(array, indices, count);
    if (value && value->desc->kind == LLG_VALUE_PACKED)
        return sv4_clone(&value->value.packed);
    const llg_value_desc_t* desc =
        llg_dyn_value_nested_desc(array->element, count);
    return desc && desc->kind == LLG_VALUE_PACKED
        ? (desc->packed_two_state
            ? sv4_from_u64(0, desc->packed_width, desc->packed_signed)
            : sv4_x(desc->packed_width, desc->packed_signed))
        : sv4_from_u64(0, 1, 0);
}

double llg_dyn_value_get_nested_real(const llg_dyn_value_array_t* array,
                                     const sv4_t* indices, size_t count) {
    llg_value_t* value = llg_dyn_value_nested_at(array, indices, count);
    return value && value->desc->kind == LLG_VALUE_REAL
        ? value->value.real
        : 0.0;
}

llg_string_t llg_dyn_value_get_nested_string(
    const llg_dyn_value_array_t* array, const sv4_t* indices, size_t count) {
    llg_value_t* value = llg_dyn_value_nested_at(array, indices, count);
    return value && value->desc->kind == LLG_VALUE_STRING
        ? llg_string_clone(&value->value.string)
        : (llg_string_t){0};
}

void* llg_dyn_value_get_nested_chandle(
    const llg_dyn_value_array_t* array, const sv4_t* indices, size_t count) {
    llg_value_t* value = llg_dyn_value_nested_at(array, indices, count);
    return value && llg_value_is_handle_kind(value->desc)
        ? value->value.handle
        : NULL;
}

int llg_dyn_value_set_real(llg_dyn_value_array_t* array, sv4_t index,
                           double value) {
    llg_value_t* target = llg_dyn_value_at(array, index);
    if (!target || target->desc->kind != LLG_VALUE_REAL) return 0;
    value = llg_value_real_convert(target->desc, value);
    if (llg_value_real_same(target->value.real, value)) return 1;
    target->value.real = value;
    llg_notify(array->notify, array->contents_dependency,
               array->shape_dependency, LLG_CONTAINER_CHANGED_CONTENTS);
    return 1;
}

static int dyn_value_store_string(llg_dyn_value_array_t* array, llg_value_t* target,
                                  llg_string_t value) {
    if (!target || target->desc->kind != LLG_VALUE_STRING) {
        llg_string_destroy(&value);
        return 0;
    }
    int changed = target->value.string.len != value.len ||
                  (value.len &&
                   memcmp(target->value.string.data, value.data, value.len) != 0);
    if (changed) {
        llg_string_destroy(&target->value.string);
        target->value.string = value;
        llg_notify(array->notify, array->contents_dependency,
                   array->shape_dependency, LLG_CONTAINER_CHANGED_CONTENTS);
    } else {
        llg_string_destroy(&value);
    }
    return 1;
}

static int dyn_value_store_chandle(llg_dyn_value_array_t* array, llg_value_t* target,
                                   void* value) {
    if (!target || !llg_value_is_handle_kind(target->desc))
        return 0;
    if (!llg_value_store_handle(target, value)) return 1;
    llg_notify(array->notify, array->contents_dependency,
               array->shape_dependency, LLG_CONTAINER_CHANGED_CONTENTS);
    return 1;
}

int llg_dyn_value_set_string(llg_dyn_value_array_t* array, sv4_t index,
                             llg_string_t value) {
    return dyn_value_store_string(array, llg_dyn_value_at(array, index), value);
}

int llg_dyn_value_move_string_at(llg_dyn_value_array_t* array, uint64_t position,
                                 llg_string_t* value) {
    llg_value_t* target = llg_dyn_value_element_at(array, position);
    if (!target) {
        llg_string_destroy(value);
        return 0;
    }
    llg_string_t* stored = &target->value.string;
    if (stored->len == value->len &&
        (!value->len || memcmp(stored->data, value->data, value->len) == 0)) {
        llg_string_destroy(value);
        return 1;
    }
    llg_string_destroy(stored);
    *stored = *value;
    *value = (llg_string_t){0};
    llg_notify(array->notify, array->contents_dependency,
               array->shape_dependency, LLG_CONTAINER_CHANGED_CONTENTS);
    return 1;
}

int llg_dyn_value_set_chandle(llg_dyn_value_array_t* array, sv4_t index,
                              void* value) {
    return dyn_value_store_chandle(array, llg_dyn_value_at(array, index), value);
}

int llg_dyn_value_store_chandle_at(llg_dyn_value_array_t* array, llg_value_t* target,
                                   void* value) {
    return dyn_value_store_chandle(array, target, value);
}

int llg_dyn_value_set_nested(llg_dyn_value_array_t* array,
                             const sv4_t* indices, size_t count, sv4_t value) {
    llg_value_t* target = llg_dyn_value_nested_at(array, indices, count);
    if (!target || target->desc->kind != LLG_VALUE_PACKED) return 0;
    sv4_t assigned = sv4_cast(value, target->desc->packed_width,
                              target->desc->packed_signed);
    if (target->desc->packed_two_state)
        sv4_replace(&assigned, sv4_to_two_state(assigned));
    if (sv4_same(target->value.packed, assigned)) {
        sv4_destroy(&assigned);
        return 1;
    }
    sv4_move(&target->value.packed, &assigned);
    llg_notify(array->notify, array->contents_dependency,
               array->shape_dependency, LLG_CONTAINER_CHANGED_CONTENTS);
    return 1;
}

int llg_dyn_value_set_nested_real(llg_dyn_value_array_t* array,
                                  const sv4_t* indices, size_t count,
                                  double value) {
    llg_value_t* target = llg_dyn_value_nested_at(array, indices, count);
    if (!target || target->desc->kind != LLG_VALUE_REAL) return 0;
    value = llg_value_real_convert(target->desc, value);
    if (llg_value_real_same(target->value.real, value)) return 1;
    target->value.real = value;
    llg_notify(array->notify, array->contents_dependency,
               array->shape_dependency, LLG_CONTAINER_CHANGED_CONTENTS);
    return 1;
}

int llg_dyn_value_set_nested_string(llg_dyn_value_array_t* array,
                                    const sv4_t* indices, size_t count,
                                    llg_string_t value) {
    llg_value_t* target = llg_dyn_value_nested_at(array, indices, count);
    if (!target || target->desc->kind != LLG_VALUE_STRING) {
        llg_string_destroy(&value);
        return 0;
    }
    int changed = target->value.string.len != value.len ||
                  (value.len &&
                   memcmp(target->value.string.data, value.data, value.len) != 0);
    if (changed) {
        llg_string_destroy(&target->value.string);
        target->value.string = value;
        llg_notify(array->notify, array->contents_dependency,
                   array->shape_dependency, LLG_CONTAINER_CHANGED_CONTENTS);
    } else {
        llg_string_destroy(&value);
    }
    return 1;
}

int llg_dyn_value_set_nested_chandle(llg_dyn_value_array_t* array,
                                     const sv4_t* indices, size_t count,
                                     void* value) {
    llg_value_t* target = llg_dyn_value_nested_at(array, indices, count);
    if (!target || !llg_value_is_handle_kind(target->desc))
        return 0;
    if (!llg_value_store_handle(target, value)) return 1;
    llg_notify(array->notify, array->contents_dependency,
               array->shape_dependency, LLG_CONTAINER_CHANGED_CONTENTS);
    return 1;
}

static void llg_dyn_value_replace_from_packed(
    llg_dyn_value_array_t* dst, const llg_dyn_array_t* source) {
    if (!dst->element || !source ||
        (dst->element->kind != LLG_VALUE_PACKED &&
         dst->element->kind != LLG_VALUE_REAL))
        llg_container_fatal("packed source is incompatible with recursive container element");
    llg_value_t* data = llg_alloc_items(source->size, sizeof(*data));
    if (source->size) memset(data, 0, source->size * sizeof(*data));
    for (size_t i = 0; i < source->size; ++i) {
        llg_value_default(&data[i], dst->element);
        if (dst->element->kind == LLG_VALUE_PACKED) {
            sv4_t value = sv4_cast(source->data[i], dst->element->packed_width,
                                   dst->element->packed_signed);
            if (dst->element->packed_two_state)
                sv4_replace(&value, sv4_to_two_state(value));
            sv4_move(&data[i].value.packed, &value);
        } else {
            data[i].value.real = llg_value_real_convert(
                dst->element, sv4_to_real(source->data[i]));
        }
    }
    llg_dyn_value_commit(dst, data, source->size);
}

int llg_dyn_value_set_nested_container(
    llg_dyn_value_array_t* array, const sv4_t* indices, size_t count,
    const llg_dyn_value_array_t* source) {
    llg_value_t* target = llg_dyn_value_nested_at(array, indices, count);
    if (!target || target->desc->kind != LLG_VALUE_CONTAINER || !source ||
        !llg_value_desc_compatible(target->desc->element, source->element))
        return 0;
    llg_value_t source_value = {0};
    source_value.desc = target->desc;
    source_value.value.container = (llg_dyn_value_array_t*)source;
    llg_value_copy(target, target->desc, &source_value);
    llg_notify(array->notify, array->contents_dependency,
               array->shape_dependency, LLG_CONTAINER_CHANGED_CONTENTS);
    return 1;
}

int llg_dyn_value_set_nested_container_from_packed(
    llg_dyn_value_array_t* array, const sv4_t* indices, size_t count,
    const llg_dyn_array_t* source) {
    llg_value_t* target = llg_dyn_value_nested_at(array, indices, count);
    if (!target || target->desc->kind != LLG_VALUE_CONTAINER || !source ||
        !target->desc->element ||
        (target->desc->element->kind != LLG_VALUE_PACKED &&
         target->desc->element->kind != LLG_VALUE_REAL))
        return 0;
    llg_dyn_value_array_t converted = {0};
    llg_dyn_value_init(&converted, target->desc->element);
    llg_dyn_value_replace_from_packed(&converted, source);
    llg_value_t source_value = {0};
    source_value.desc = target->desc;
    source_value.value.container = &converted;
    llg_value_copy(target, target->desc, &source_value);
    llg_dyn_value_destroy(&converted);
    llg_notify(array->notify, array->contents_dependency,
               array->shape_dependency, LLG_CONTAINER_CHANGED_CONTENTS);
    return 1;
}
