
int llg_assoc_value_exists_integral(const llg_assoc_value_t* array, sv4_t key) {
    sv4_t normalized = SV4_EMPTY;
    int result_value;

    int found = 0;
    if (!llg_assoc_value_normalize_key(array, key, &normalized)) do { result_value = 0; goto cleanup_key; } while (0);
    (void)llg_assoc_value_integral_position(array, normalized, &found);
    do { result_value = found; goto cleanup_key; } while (0);
cleanup_key:
    sv4_destroy(&normalized);
    return result_value;
}

int llg_assoc_value_delete_integral(llg_assoc_value_t* array, sv4_t key) {
    int change = 0;
    sv4_t normalized = SV4_EMPTY;
    int result_value;

    int found = 0;
    if (!llg_assoc_value_normalize_key(array, key, &normalized)) do { result_value = 0; goto cleanup_key; } while (0);
    size_t position = llg_assoc_value_integral_position(array, normalized, &found);
    if (!found) do { result_value = 0; goto cleanup_key; } while (0);
    sv4_destroy(&array->entries[position].integral_key);
    llg_value_drop(&array->entries[position].value);
    if (position + 1 < array->size)
        memmove(array->entries + position, array->entries + position + 1,
                (array->size - position - 1) * sizeof(*array->entries));
    --array->size;
    memset(&array->entries[array->size], 0, sizeof(*array->entries));
    llg_assoc_value_invalidate_refs(array);
    change = LLG_CONTAINER_CHANGED_CONTENTS | LLG_CONTAINER_CHANGED_SHAPE;
    do { result_value = 1; goto cleanup_key; } while (0);
cleanup_key:
    sv4_destroy(&normalized);
    /* No temporary owner may remain live across the callback. */
    llg_notify(array->notify, array->contents_dependency,
               array->shape_dependency, change);
    return result_value;
}

static void llg_assoc_value_set_default_source(llg_assoc_value_t* array,
                                               const llg_value_t* source, int* change) {
    int changed = !array->has_default_value ||
                  !llg_value_equal(&array->default_value, source);
    if (changed)
        llg_value_copy(&array->default_value, array->element, source);
    array->has_default_value = 1;
    *change |= changed ? LLG_CONTAINER_CHANGED_CONTENTS : 0;
}

void llg_assoc_value_set_default(llg_assoc_value_t* array, sv4_t value) {
    int change = 0;
    llg_value_t source = llg_assoc_value_packed_source(array, value);
    llg_assoc_value_set_default_source(array, &source, &change);
    llg_value_drop(&source);
    /* No temporary owner may remain live across the callback. */
    llg_notify(array->notify, array->contents_dependency,
               array->shape_dependency, change);
}

void llg_assoc_value_set_default_real(llg_assoc_value_t* array, double value) {
    int change = 0;
    llg_value_t source = llg_value_from_real(array->element, value);
    llg_assoc_value_set_default_source(array, &source, &change);
    llg_value_drop(&source);
    /* No temporary owner may remain live across the callback. */
    llg_notify(array->notify, array->contents_dependency,
               array->shape_dependency, change);
}

void llg_assoc_value_set_default_string(llg_assoc_value_t* array,
                                        llg_string_t value) {
    int change = 0;
    llg_value_t source = llg_value_from_string(array->element, value);
    llg_assoc_value_set_default_source(array, &source, &change);
    llg_value_drop(&source);
    /* No temporary owner may remain live across the callback. */
    llg_notify(array->notify, array->contents_dependency,
               array->shape_dependency, change);
}

void llg_assoc_value_set_default_chandle(llg_assoc_value_t* array, void* value) {
    int change = 0;
    llg_value_t source = llg_value_from_chandle(array->element, value);
    llg_assoc_value_set_default_source(array, &source, &change);
    /* No temporary owner may remain live across the callback. */
    llg_notify(array->notify, array->contents_dependency,
               array->shape_dependency, change);
}

void llg_assoc_value_reset_default(llg_assoc_value_t* array) {
    llg_value_t source = {0};
    llg_value_default(&source, array->element);
    int changed = array->has_default_value ||
                  !llg_value_equal(&array->default_value, &source);
    if (changed) llg_value_copy(&array->default_value, array->element, &source);
    array->has_default_value = 0;
    llg_value_drop(&source);
    llg_notify(array->notify, array->contents_dependency,
               array->shape_dependency,
               changed ? LLG_CONTAINER_CHANGED_CONTENTS : 0);
}

static int llg_assoc_value_integral_traversal(
    const llg_assoc_value_t* array, sv4_t* key, int direction, int endpoint) {
    sv4_t normalized = SV4_EMPTY;
    int result_value;

    llg_assoc_value_check_kind(array, LLG_ASSOC_INTEGRAL);
    if (!array->key_width)
        llg_container_fatal("wildcard associative-array traversal is illegal");
    if (!array->size) do { result_value = 0; goto cleanup_key; } while (0);
    if (endpoint) {
        sv4_copy(key, &array->entries[direction > 0 ? 0 : array->size - 1].integral_key);
        do { result_value = 1; goto cleanup_key; } while (0);
    }
    if (!llg_assoc_value_normalize_key(array, *key, &normalized)) do { result_value = 0; goto cleanup_key; } while (0);
    int found;
    size_t position = llg_assoc_value_integral_position(array, normalized, &found);
    if (direction > 0) {
        if (found) ++position;
        if (position >= array->size) do { result_value = 0; goto cleanup_key; } while (0);
    } else {
        if (position == 0) do { result_value = 0; goto cleanup_key; } while (0);
        --position;
    }
    sv4_copy(key, &array->entries[position].integral_key);
    do { result_value = 1; goto cleanup_key; } while (0);
cleanup_key:
    sv4_destroy(&normalized);
    return result_value;
}

int llg_assoc_value_first_integral(const llg_assoc_value_t* a, sv4_t* key) {
    return llg_assoc_value_integral_traversal(a, key, 1, 1);
}
int llg_assoc_value_last_integral(const llg_assoc_value_t* a, sv4_t* key) {
    return llg_assoc_value_integral_traversal(a, key, -1, 1);
}
int llg_assoc_value_next_integral(const llg_assoc_value_t* a, sv4_t* key) {
    return llg_assoc_value_integral_traversal(a, key, 1, 0);
}
int llg_assoc_value_prev_integral(const llg_assoc_value_t* a, sv4_t* key) {
    return llg_assoc_value_integral_traversal(a, key, -1, 0);
}

static size_t llg_assoc_value_string_position(const llg_assoc_value_t* array,
                                              const void* key,
                                              size_t key_length, int* found) {
    size_t low = 0, high = array->size;
    while (low < high) {
        size_t mid = low + (high - low) / 2;
        int cmp = llg_assoc_string_compare(array->entries[mid].string_key,
                                           array->entries[mid].string_length,
                                           key, key_length);
        if (cmp < 0)
            low = mid + 1;
        else
            high = mid;
    }
    *found = low < array->size &&
             llg_assoc_string_compare(array->entries[low].string_key,
                                      array->entries[low].string_length,
                                      key, key_length) == 0;
    return low;
}

static void llg_assoc_value_check_string_key(const llg_assoc_value_t* array,
                                             const void* key,
                                             size_t key_length) {
    llg_assoc_value_check_kind(array, LLG_ASSOC_STRING);
    if (!key && key_length)
        llg_container_fatal("null recursive associative-array string key");
}

llg_string_t llg_assoc_value_get_string(const llg_assoc_value_t* array,
                                        const void* key, size_t key_length) {
    llg_assoc_value_check_string_key(array, key, key_length);
    int found;
    size_t position = llg_assoc_value_string_position(array, key, key_length,
                                                      &found);
    if (found && array->entries[position].value.desc->kind == LLG_VALUE_STRING)
        return llg_string_clone(&array->entries[position].value.value.string);
    return array->default_value.desc &&
                   array->default_value.desc->kind == LLG_VALUE_STRING
        ? llg_string_clone(&array->default_value.value.string)
        : (llg_string_t){0};
}

double llg_assoc_value_get_string_real(const llg_assoc_value_t* array,
                                       const void* key, size_t key_length) {
    llg_assoc_value_check_string_key(array, key, key_length);
    int found;
    size_t position = llg_assoc_value_string_position(array, key, key_length,
                                                      &found);
    if (found && array->entries[position].value.desc->kind == LLG_VALUE_REAL)
        return array->entries[position].value.value.real;
    return array->default_value.desc &&
                   array->default_value.desc->kind == LLG_VALUE_REAL
        ? array->default_value.value.real
        : 0.0;
}

llg_string_t llg_assoc_value_get_string_string(const llg_assoc_value_t* array,
                                               const void* key,
                                               size_t key_length) {
    return llg_assoc_value_get_string(array, key, key_length);
}

void* llg_assoc_value_get_string_chandle(const llg_assoc_value_t* array,
                                         const void* key, size_t key_length) {
    llg_assoc_value_check_string_key(array, key, key_length);
    int found;
    size_t position = llg_assoc_value_string_position(array, key, key_length,
                                                      &found);
    if (found && (array->entries[position].value.desc->kind == LLG_VALUE_CHANDLE ||
                  array->entries[position].value.desc->kind == LLG_VALUE_EVENT))
        return array->entries[position].value.value.handle;
    return array->default_value.desc &&
                   (array->default_value.desc->kind == LLG_VALUE_CHANDLE ||
                    array->default_value.desc->kind == LLG_VALUE_EVENT)
        ? array->default_value.value.handle
        : NULL;
}

int llg_assoc_value_set_string(llg_assoc_value_t* array, const void* key,
                               size_t key_length, sv4_t value) {
    int change = 0;
    llg_assoc_value_check_string_key(array, key, key_length);
    llg_value_t source = llg_assoc_value_packed_source(array, value);
    int result = llg_assoc_value_set_source(array, NULL, key, key_length, &source, &change);
    llg_value_drop(&source);
    /* No temporary owner may remain live across the callback. */
    llg_notify(array->notify, array->contents_dependency,
               array->shape_dependency, change);
    return result;
}

int llg_assoc_value_set_string_real(llg_assoc_value_t* array, const void* key,
                                    size_t key_length, double value) {
    int change = 0;
    llg_assoc_value_check_string_key(array, key, key_length);
    llg_value_t source = llg_value_from_real(array->element, value);
    int result = llg_assoc_value_set_source(array, NULL, key, key_length, &source, &change);
    llg_value_drop(&source);
    /* No temporary owner may remain live across the callback. */
    llg_notify(array->notify, array->contents_dependency,
               array->shape_dependency, change);
    return result;
}

int llg_assoc_value_set_string_string(llg_assoc_value_t* array, const void* key,
                                      size_t key_length, llg_string_t value) {
    int change = 0;
    llg_assoc_value_check_string_key(array, key, key_length);
    llg_value_t source = llg_value_from_string(array->element, value);
    int result = llg_assoc_value_set_source(array, NULL, key, key_length, &source, &change);
    llg_value_drop(&source);
    /* No temporary owner may remain live across the callback. */
    llg_notify(array->notify, array->contents_dependency,
               array->shape_dependency, change);
    return result;
}

int llg_assoc_value_set_string_chandle(llg_assoc_value_t* array, const void* key,
                                       size_t key_length, void* value) {
    int change = 0;
    llg_assoc_value_check_string_key(array, key, key_length);
    llg_value_t source = llg_value_from_chandle(array->element, value);
    int result = llg_assoc_value_set_source(array, NULL, key, key_length, &source, &change);
    llg_value_drop(&source);
    /* No temporary owner may remain live across the callback. */
    llg_notify(array->notify, array->contents_dependency,
               array->shape_dependency, change);
    return result;
}

int llg_assoc_value_exists_string(const llg_assoc_value_t* array,
                                  const void* key, size_t key_length) {
    llg_assoc_value_check_string_key(array, key, key_length);
    int found;
    (void)llg_assoc_value_string_position(array, key, key_length, &found);
    return found;
}

int llg_assoc_value_delete_string(llg_assoc_value_t* array, const void* key,
                                  size_t key_length) {
    llg_assoc_value_check_string_key(array, key, key_length);
    int found;
    size_t position = llg_assoc_value_string_position(array, key, key_length,
                                                      &found);
    if (!found) return 0;
    free(array->entries[position].string_key);
    sv4_destroy(&array->entries[position].integral_key);
    llg_value_drop(&array->entries[position].value);
    if (position + 1 < array->size)
        memmove(array->entries + position, array->entries + position + 1,
                (array->size - position - 1) * sizeof(*array->entries));
    --array->size;
    memset(&array->entries[array->size], 0, sizeof(*array->entries));
    llg_assoc_value_invalidate_refs(array);
    llg_notify(array->notify, array->contents_dependency,
               array->shape_dependency,
               LLG_CONTAINER_CHANGED_CONTENTS | LLG_CONTAINER_CHANGED_SHAPE);
    return 1;
}

static int llg_assoc_value_string_endpoint(const llg_assoc_value_t* array,
                                           int last,
                                           const unsigned char** key,
                                           size_t* key_length) {
    llg_assoc_value_check_kind(array, LLG_ASSOC_STRING);
    if (!array->size) return 0;
    size_t position = last ? array->size - 1 : 0;
    *key = array->entries[position].string_key;
    *key_length = array->entries[position].string_length;
    return 1;
}

int llg_assoc_value_first_string(const llg_assoc_value_t* array,
                                 const unsigned char** key, size_t* key_length) {
    return llg_assoc_value_string_endpoint(array, 0, key, key_length);
}
int llg_assoc_value_last_string(const llg_assoc_value_t* array,
                                const unsigned char** key, size_t* key_length) {
    return llg_assoc_value_string_endpoint(array, 1, key, key_length);
}
int llg_assoc_value_next_string(const llg_assoc_value_t* array,
                                const void* current, size_t current_length,
                                const unsigned char** key, size_t* key_length) {
    llg_assoc_value_check_string_key(array, current, current_length);
    int found;
    size_t position = llg_assoc_value_string_position(array, current,
                                                      current_length, &found);
    if (found) ++position;
    if (position >= array->size) return 0;
    *key = array->entries[position].string_key;
    *key_length = array->entries[position].string_length;
    return 1;
}
int llg_assoc_value_prev_string(const llg_assoc_value_t* array,
                                const void* current, size_t current_length,
                                const unsigned char** key, size_t* key_length) {
    llg_assoc_value_check_string_key(array, current, current_length);
    int found;
    size_t position = llg_assoc_value_string_position(array, current,
                                                      current_length, &found);
    if (position == 0) return 0;
    --position;
    *key = array->entries[position].string_key;
    *key_length = array->entries[position].string_length;
    return 1;
}

void llg_assoc_value_copy(llg_assoc_value_t* dst,
                          const llg_assoc_value_t* src) {
    if (dst == src) return;
    if (!src || !llg_value_desc_compatible(dst->element, src->element) ||
        dst->key_kind != src->key_kind || dst->key_width != src->key_width ||
        dst->key_signed != src->key_signed ||
        dst->key_two_state != src->key_two_state)
        llg_container_fatal("incompatible recursive associative-array types");
    int shape_changed = dst->size != src->size;
    int contents_changed = shape_changed ||
        dst->has_default_value != src->has_default_value ||
        !llg_value_equal_after_conversion(&dst->default_value, dst->element,
                                          &src->default_value);
    if (!contents_changed) {
        for (size_t i = 0; i < src->size; ++i) {
            int key_changed;
            if (src->key_kind == LLG_ASSOC_INTEGRAL) {
                key_changed = !sv4_same(dst->entries[i].integral_key,
                                        src->entries[i].integral_key);
            } else {
                key_changed = dst->entries[i].string_length !=
                                  src->entries[i].string_length ||
                    (src->entries[i].string_length &&
                     memcmp(dst->entries[i].string_key, src->entries[i].string_key,
                            src->entries[i].string_length) != 0);
            }
            if (key_changed ||
                !llg_value_equal_after_conversion(&dst->entries[i].value,
                                                  dst->element,
                                                  &src->entries[i].value)) {
                contents_changed = 1;
                if (key_changed) shape_changed = 1;
                break;
            }
        }
    }
    llg_assoc_value_entry_t* entries = llg_alloc_items(src->size,
                                                        sizeof(*entries));
    if (src->size) memset(entries, 0, src->size * sizeof(*entries));
    for (size_t i = 0; i < src->size; ++i) {
        entries[i].integral_key = sv4_clone(&src->entries[i].integral_key);
        entries[i].string_length = src->entries[i].string_length;
        if (src->entries[i].string_length) {
            entries[i].string_key = llg_alloc_items(
                src->entries[i].string_length, 1);
            memcpy(entries[i].string_key, src->entries[i].string_key,
                   src->entries[i].string_length);
        }
        llg_value_copy(&entries[i].value, dst->element, &src->entries[i].value);
    }
    llg_value_t default_value = {0};
    llg_value_copy(&default_value, dst->element, &src->default_value);
    llg_container_notify_fn notify = dst->notify;
    sv4_t* contents_dependency = dst->contents_dependency;
    sv4_t* shape_dependency = dst->shape_dependency;
    dst->notify = NULL;
    llg_assoc_value_delete(dst);
    dst->notify = notify;
    llg_value_drop(&dst->default_value);
    free(dst->entries);
    dst->entries = entries;
    dst->size = src->size;
    dst->capacity = src->size;
    dst->default_value = default_value;
    dst->has_default_value = src->has_default_value;
    llg_assoc_value_invalidate_refs(dst);
    llg_notify(notify, contents_dependency, shape_dependency,
               (contents_changed ? LLG_CONTAINER_CHANGED_CONTENTS : 0) |
                   (shape_changed ? LLG_CONTAINER_CHANGED_SHAPE : 0));
}
