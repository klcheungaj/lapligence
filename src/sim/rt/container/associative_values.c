
/* Recursive associative storage ----------------------------------------- */

static void llg_assoc_value_check_kind(const llg_assoc_value_t* array,
                                       uint8_t kind) {
    if (array->key_kind != kind)
        llg_container_fatal("recursive associative-array key kind mismatch");
}

static void llg_assoc_value_invalidate_refs(llg_assoc_value_t* array) {
    if (array->mutation_epoch == UINT64_MAX)
        llg_container_fatal("associative-array reference epoch overflow");
    ++array->mutation_epoch;
}

void llg_assoc_value_init_integral(llg_assoc_value_t* array,
                                   const llg_value_desc_t* element,
                                   uint32_t key_width, int8_t key_signed,
                                   int key_two_state) {
    if (!element) llg_container_fatal("missing recursive associative value descriptor");
    if (key_width > LLG_MAX_WIDTH)
        llg_container_fatal("invalid recursive associative key width");
    memset(array, 0, sizeof(*array));
    array->element = element;
    array->key_kind = LLG_ASSOC_INTEGRAL;
    array->key_width = key_width;
    array->key_signed = key_width ? !!key_signed : 0;
    array->key_two_state = key_width ? !!key_two_state : 0;
    llg_value_default(&array->default_value, element);
}

void llg_assoc_value_init_string(llg_assoc_value_t* array,
                                 const llg_value_desc_t* element) {
    if (!element) llg_container_fatal("missing recursive associative value descriptor");
    memset(array, 0, sizeof(*array));
    array->element = element;
    array->key_kind = LLG_ASSOC_STRING;
    llg_value_default(&array->default_value, element);
}

void llg_assoc_value_delete(llg_assoc_value_t* array) {
    int changed = array->size != 0;
    for (size_t i = 0; i < array->size; ++i) {
        free(array->entries[i].string_key);
        llg_value_drop(&array->entries[i].value);
    }
    array->size = 0;
    if (changed) llg_assoc_value_invalidate_refs(array);
    llg_notify(array->notify, array->contents_dependency,
               array->shape_dependency,
               changed ? LLG_CONTAINER_CHANGED_CONTENTS |
                            LLG_CONTAINER_CHANGED_SHAPE
                       : 0);
}

void llg_assoc_value_destroy(llg_assoc_value_t* array) {
    llg_container_notify_fn notify = array->notify;
    array->notify = NULL;
    llg_assoc_value_delete(array);
    array->notify = notify;
    llg_value_drop(&array->default_value);
    free(array->entries);
    memset(array, 0, sizeof(*array));
}

static void llg_assoc_value_reserve(llg_assoc_value_t* array, size_t needed) {
    if (needed <= array->capacity) return;
    size_t capacity = array->capacity ? array->capacity : 4;
    while (capacity < needed) {
        if (capacity > SIZE_MAX / 2) {
            capacity = needed;
            break;
        }
        capacity *= 2;
    }
    array->entries = llg_realloc_items(array->entries, capacity,
                                       sizeof(*array->entries));
    array->capacity = capacity;
}

size_t llg_assoc_value_count(const llg_assoc_value_t* array) {
    return array->size;
}

static int llg_assoc_value_normalize_key(const llg_assoc_value_t* array,
                                         sv4_t input, sv4_t* output) {
    llg_assoc_value_check_kind(array, LLG_ASSOC_INTEGRAL);
    if (sv4_is_unknown(input)) return 0;
    if (array->key_width) {
        *output = sv4_cast(input, array->key_width, array->key_signed);
        if (array->key_two_state) *output = sv4_to_two_state(*output);
    } else {
        *output = sv4_cast(input, LLG_MAX_WIDTH, input.is_signed);
        output->is_signed = 0;
    }
    return !sv4_is_unknown(*output);
}

static size_t llg_assoc_value_integral_position(
    const llg_assoc_value_t* array, sv4_t key, int* found) {
    size_t low = 0, high = array->size;
    while (low < high) {
        size_t mid = low + (high - low) / 2;
        int cmp = llg_integral_compare(array->entries[mid].integral_key, key);
        if (cmp < 0)
            low = mid + 1;
        else
            high = mid;
    }
    *found = low < array->size &&
             llg_integral_compare(array->entries[low].integral_key, key) == 0;
    return low;
}

static int llg_assoc_value_set_source(llg_assoc_value_t* array,
                                      sv4_t* integral_key,
                                      const void* string_key,
                                      size_t string_length,
                                      const llg_value_t* source) {
    int found;
    size_t position;
    if (array->key_kind == LLG_ASSOC_INTEGRAL) {
        position = llg_assoc_value_integral_position(array, *integral_key,
                                                      &found);
    } else {
        position = 0;
        while (position < array->size) {
            int cmp = llg_assoc_string_compare(
                array->entries[position].string_key,
                array->entries[position].string_length,
                string_key, string_length);
            if (cmp >= 0) break;
            ++position;
        }
        found = position < array->size &&
                llg_assoc_string_compare(array->entries[position].string_key,
                                         array->entries[position].string_length,
                                         string_key, string_length) == 0;
    }
    int shape_changed = !found;
    int contents_changed = !found ||
                           !llg_value_equal(&array->entries[position].value,
                                            source);
    if (!found) {
        if (array->size == SIZE_MAX)
            llg_container_fatal("recursive associative-array size overflow");
        unsigned char* key_copy = NULL;
        if (array->key_kind == LLG_ASSOC_STRING && string_length) {
            key_copy = llg_alloc_items(string_length, 1);
            memcpy(key_copy, string_key, string_length);
        }
        llg_assoc_value_reserve(array, array->size + 1);
        if (position < array->size)
            memmove(array->entries + position + 1, array->entries + position,
                    (array->size - position) * sizeof(*array->entries));
        memset(&array->entries[position], 0, sizeof(*array->entries));
        array->entries[position].integral_key = integral_key
            ? *integral_key : (sv4_t){0};
        array->entries[position].string_key = key_copy;
        array->entries[position].string_length = string_length;
        ++array->size;
        llg_value_copy(&array->entries[position].value, array->element, source);
    } else if (contents_changed) {
        llg_value_copy(&array->entries[position].value, array->element, source);
    }
    if (shape_changed) llg_assoc_value_invalidate_refs(array);
    llg_notify(array->notify, array->contents_dependency,
               array->shape_dependency,
               (contents_changed ? LLG_CONTAINER_CHANGED_CONTENTS : 0) |
                   (shape_changed ? LLG_CONTAINER_CHANGED_SHAPE : 0));
    return 1;
}

static llg_value_t llg_assoc_value_packed_source(
    const llg_assoc_value_t* array, sv4_t value) {
    return llg_value_from_packed(array->element, value);
}

sv4_t llg_assoc_value_get_integral(const llg_assoc_value_t* array, sv4_t key) {
    sv4_t normalized;
    int found = 0;
    if (llg_assoc_value_normalize_key(array, key, &normalized)) {
        size_t position = llg_assoc_value_integral_position(array, normalized,
                                                             &found);
        if (found && array->entries[position].value.desc->kind == LLG_VALUE_PACKED)
            return array->entries[position].value.value.packed;
    }
    return array->default_value.desc &&
                   array->default_value.desc->kind == LLG_VALUE_PACKED
        ? array->default_value.value.packed
        : sv4_from_u64(0, 1, 0);
}

double llg_assoc_value_get_integral_real(const llg_assoc_value_t* array,
                                         sv4_t key) {
    sv4_t normalized;
    int found = 0;
    if (llg_assoc_value_normalize_key(array, key, &normalized)) {
        size_t position = llg_assoc_value_integral_position(array, normalized,
                                                             &found);
        if (found && array->entries[position].value.desc->kind == LLG_VALUE_REAL)
            return array->entries[position].value.value.real;
    }
    return array->default_value.desc &&
                   array->default_value.desc->kind == LLG_VALUE_REAL
        ? array->default_value.value.real
        : 0.0;
}

llg_string_t llg_assoc_value_get_integral_string(
    const llg_assoc_value_t* array, sv4_t key) {
    sv4_t normalized;
    int found = 0;
    if (llg_assoc_value_normalize_key(array, key, &normalized)) {
        size_t position = llg_assoc_value_integral_position(array, normalized,
                                                             &found);
        if (found && array->entries[position].value.desc->kind == LLG_VALUE_STRING)
            return llg_string_clone(&array->entries[position].value.value.string);
    }
    return array->default_value.desc &&
                   array->default_value.desc->kind == LLG_VALUE_STRING
        ? llg_string_clone(&array->default_value.value.string)
        : (llg_string_t){0};
}

void* llg_assoc_value_get_integral_chandle(
    const llg_assoc_value_t* array, sv4_t key) {
    sv4_t normalized;
    int found = 0;
    if (llg_assoc_value_normalize_key(array, key, &normalized)) {
        size_t position = llg_assoc_value_integral_position(array, normalized,
                                                             &found);
        if (found && (array->entries[position].value.desc->kind == LLG_VALUE_CHANDLE ||
                      array->entries[position].value.desc->kind == LLG_VALUE_EVENT))
            return array->entries[position].value.value.handle;
    }
    return array->default_value.desc &&
                   (array->default_value.desc->kind == LLG_VALUE_CHANDLE ||
                    array->default_value.desc->kind == LLG_VALUE_EVENT)
        ? array->default_value.value.handle
        : NULL;
}

static llg_value_t* llg_assoc_value_nested_at_integral(
    const llg_assoc_value_t* array, const sv4_t* indices, size_t count) {
    if (!indices || count < 2 || array->key_kind != LLG_ASSOC_INTEGRAL)
        return NULL;
    sv4_t normalized;
    int found = 0;
    if (!llg_assoc_value_normalize_key(array, indices[0], &normalized))
        return NULL;
    size_t position = llg_assoc_value_integral_position(array, normalized,
                                                         &found);
    if (!found) return NULL;
    llg_value_t* value = &array->entries[position].value;
    for (size_t index = 1; value && index < count; ++index) {
        if (!value->desc || value->desc->kind != LLG_VALUE_CONTAINER ||
            !value->value.container)
            return NULL;
        value = llg_dyn_value_at(value->value.container, indices[index]);
    }
    return value;
}

sv4_t llg_assoc_value_get_nested_integral(
    const llg_assoc_value_t* array, const sv4_t* indices, size_t count) {
    llg_value_t* value = llg_assoc_value_nested_at_integral(array, indices, count);
    return value && value->desc->kind == LLG_VALUE_PACKED
        ? value->value.packed
        : sv4_from_u64(0, 1, 0);
}

double llg_assoc_value_get_nested_integral_real(
    const llg_assoc_value_t* array, const sv4_t* indices, size_t count) {
    llg_value_t* value = llg_assoc_value_nested_at_integral(array, indices, count);
    return value && value->desc->kind == LLG_VALUE_REAL ? value->value.real : 0.0;
}

llg_string_t llg_assoc_value_get_nested_integral_string(
    const llg_assoc_value_t* array, const sv4_t* indices, size_t count) {
    llg_value_t* value = llg_assoc_value_nested_at_integral(array, indices, count);
    return value && value->desc->kind == LLG_VALUE_STRING
        ? llg_string_clone(&value->value.string)
        : (llg_string_t){0};
}

void* llg_assoc_value_get_nested_integral_chandle(
    const llg_assoc_value_t* array, const sv4_t* indices, size_t count) {
    llg_value_t* value = llg_assoc_value_nested_at_integral(array, indices, count);
    return value && (value->desc->kind == LLG_VALUE_CHANDLE ||
                     value->desc->kind == LLG_VALUE_EVENT)
        ? value->value.handle
        : NULL;
}

int llg_assoc_value_set_integral(llg_assoc_value_t* array, sv4_t key,
                                 sv4_t value) {
    sv4_t normalized;
    if (!llg_assoc_value_normalize_key(array, key, &normalized)) return 0;
    llg_value_t source = llg_assoc_value_packed_source(array, value);
    int result = llg_assoc_value_set_source(array, &normalized, NULL, 0, &source);
    llg_value_drop(&source);
    return result;
}

int llg_assoc_value_set_integral_real(llg_assoc_value_t* array, sv4_t key,
                                      double value) {
    sv4_t normalized;
    if (!llg_assoc_value_normalize_key(array, key, &normalized)) return 0;
    llg_value_t source = llg_value_from_real(array->element, value);
    int result = llg_assoc_value_set_source(array, &normalized, NULL, 0, &source);
    llg_value_drop(&source);
    return result;
}

int llg_assoc_value_set_integral_string(llg_assoc_value_t* array, sv4_t key,
                                        llg_string_t value) {
    sv4_t normalized;
    if (!llg_assoc_value_normalize_key(array, key, &normalized)) {
        llg_string_destroy(&value);
        return 0;
    }
    llg_value_t source = llg_value_from_string(array->element, value);
    int result = llg_assoc_value_set_source(array, &normalized, NULL, 0, &source);
    llg_value_drop(&source);
    return result;
}

int llg_assoc_value_set_integral_chandle(llg_assoc_value_t* array, sv4_t key,
                                         void* value) {
    sv4_t normalized;
    if (!llg_assoc_value_normalize_key(array, key, &normalized)) return 0;
    llg_value_t source = llg_value_from_chandle(array->element, value);
    return llg_assoc_value_set_source(array, &normalized, NULL, 0, &source);
}

int llg_assoc_value_set_nested_integral_container(
    llg_assoc_value_t* array, const sv4_t* indices, size_t count,
    const llg_dyn_value_array_t* source) {
    sv4_t normalized;
    if (!indices || count == 0 ||
        !llg_assoc_value_normalize_key(array, indices[0], &normalized))
        return 0;
    llg_value_t* target = count == 1
        ? NULL
        : llg_assoc_value_nested_at_integral(array, indices, count);
    const llg_value_desc_t* target_desc = count == 1
        ? array->element
        : target && target->desc->kind == LLG_VALUE_CONTAINER
            ? target->desc
            : NULL;
    if (!target_desc || target_desc->kind != LLG_VALUE_CONTAINER || !source ||
        !llg_value_desc_compatible(target_desc->element, source->element))
        return 0;
    llg_value_t value = llg_value_from_container(target_desc, source);
    int result;
    if (count == 1) {
        result = llg_assoc_value_set_source(array, &normalized, NULL, 0, &value);
    } else {
        result = 0;
        if (target && target->desc->kind == LLG_VALUE_CONTAINER) {
            result = 1;
            if (!llg_value_equal(target, &value)) {
                llg_value_copy(target, target->desc, &value);
                llg_notify(array->notify, array->contents_dependency,
                           array->shape_dependency,
                           LLG_CONTAINER_CHANGED_CONTENTS);
            }
        }
    }
    llg_value_drop(&value);
    return result;
}

int llg_assoc_value_set_nested_integral_container_from_packed(
    llg_assoc_value_t* array, const sv4_t* indices, size_t count,
    const llg_dyn_array_t* source) {
    if (!indices || count == 0 || !source) return 0;
    sv4_t normalized;
    if (!llg_assoc_value_normalize_key(array, indices[0], &normalized)) return 0;

    const llg_value_desc_t* target_desc = NULL;
    if (count == 1) {
        target_desc = array->element;
    } else {
        llg_value_t* target = llg_assoc_value_nested_at_integral(array, indices,
                                                                  count);
        if (!target || target->desc->kind != LLG_VALUE_CONTAINER)
            return 0;
        target_desc = target->desc;
    }
    if (!target_desc || target_desc->kind != LLG_VALUE_CONTAINER ||
        !target_desc->element ||
        (target_desc->element->kind != LLG_VALUE_PACKED &&
         target_desc->element->kind != LLG_VALUE_REAL))
        return 0;

    llg_value_t value = llg_value_from_packed_container(target_desc, source);
    int result;
    if (count == 1) {
        result = llg_assoc_value_set_source(array, &normalized, NULL, 0, &value);
    } else {
        result = 0;
        llg_value_t* target = llg_assoc_value_nested_at_integral(array, indices,
                                                                  count);
        if (target && target->desc->kind == LLG_VALUE_CONTAINER) {
            result = 1;
            if (!llg_value_equal(target, &value)) {
                llg_value_copy(target, target->desc, &value);
                llg_notify(array->notify, array->contents_dependency,
                           array->shape_dependency,
                           LLG_CONTAINER_CHANGED_CONTENTS);
            }
        }
    }
    llg_value_drop(&value);
    return result;
}

int llg_assoc_value_set_nested_integral(
    llg_assoc_value_t* array, const sv4_t* indices, size_t count,
    sv4_t value) {
    llg_value_t* target = llg_assoc_value_nested_at_integral(array, indices, count);
    if (!target || target->desc->kind != LLG_VALUE_PACKED) return 0;
    sv4_t assigned = sv4_cast(value, target->desc->packed_width,
                              target->desc->packed_signed);
    if (target->desc->packed_two_state) assigned = sv4_to_two_state(assigned);
    if (sv4_same(target->value.packed, assigned)) return 1;
    target->value.packed = assigned;
    llg_notify(array->notify, array->contents_dependency,
               array->shape_dependency, LLG_CONTAINER_CHANGED_CONTENTS);
    return 1;
}

int llg_assoc_value_set_nested_integral_real(
    llg_assoc_value_t* array, const sv4_t* indices, size_t count,
    double value) {
    llg_value_t* target = llg_assoc_value_nested_at_integral(array, indices, count);
    if (!target || target->desc->kind != LLG_VALUE_REAL) return 0;
    value = llg_value_real_convert(target->desc, value);
    if (llg_value_real_same(target->value.real, value)) return 1;
    target->value.real = value;
    llg_notify(array->notify, array->contents_dependency,
               array->shape_dependency, LLG_CONTAINER_CHANGED_CONTENTS);
    return 1;
}

int llg_assoc_value_set_nested_integral_string(
    llg_assoc_value_t* array, const sv4_t* indices, size_t count,
    llg_string_t value) {
    llg_value_t* target = llg_assoc_value_nested_at_integral(array, indices, count);
    if (!target || target->desc->kind != LLG_VALUE_STRING) {
        llg_string_destroy(&value);
        return 0;
    }
    int changed = target->value.string.len != value.len ||
                  (value.len && memcmp(target->value.string.data, value.data,
                                       value.len) != 0);
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

int llg_assoc_value_set_nested_integral_chandle(
    llg_assoc_value_t* array, const sv4_t* indices, size_t count,
    void* value) {
    llg_value_t* target = llg_assoc_value_nested_at_integral(array, indices, count);
    if (!target || (target->desc->kind != LLG_VALUE_CHANDLE &&
                    target->desc->kind != LLG_VALUE_EVENT))
        return 0;
    if (target->value.handle == value) return 1;
    target->value.handle = value;
    llg_notify(array->notify, array->contents_dependency,
               array->shape_dependency, LLG_CONTAINER_CHANGED_CONTENTS);
    return 1;
}
