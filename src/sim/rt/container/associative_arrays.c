
static void llg_assoc_check_kind(const llg_assoc_t* array, uint8_t kind) {
    if (array->key_kind != kind)
        llg_container_fatal("associative-array key kind mismatch");
}

void llg_assoc_init_integral(llg_assoc_t* array, uint32_t element_width,
                             int8_t element_signed, int element_two_state,
                             uint32_t key_width, int8_t key_signed,
                             int key_two_state) {
    llg_check_element_type(element_width);
    if (key_width > LLG_MAX_WIDTH)
        llg_container_fatal("invalid associative-array key width");
    memset(array, 0, sizeof(*array));
    array->element_width = element_width;
    array->element_signed = !!element_signed;
    array->element_two_state = !!element_two_state;
    array->key_kind = LLG_ASSOC_INTEGRAL;
    array->key_width = key_width;
    array->key_signed = key_width ? !!key_signed : 0;
    array->key_two_state = key_width ? !!key_two_state : 0;
    array->default_value = llg_element_default(element_width,
                                                array->element_signed,
                                                array->element_two_state);
}

void llg_assoc_init_string(llg_assoc_t* array, uint32_t element_width,
                           int8_t element_signed, int element_two_state) {
    llg_check_element_type(element_width);
    memset(array, 0, sizeof(*array));
    array->element_width = element_width;
    array->element_signed = !!element_signed;
    array->element_two_state = !!element_two_state;
    array->key_kind = LLG_ASSOC_STRING;
    array->default_value = llg_element_default(element_width,
                                                array->element_signed,
                                                array->element_two_state);
}

void llg_assoc_delete(llg_assoc_t* array) {
    int changed = array->size != 0;
    for (size_t i = 0; i < array->size; ++i)
        free(array->entries[i].string_key);
    array->size = 0;
    llg_notify(array->notify, array->contents_dependency,
               array->shape_dependency,
               changed ? LLG_CONTAINER_CHANGED_CONTENTS |
                            LLG_CONTAINER_CHANGED_SHAPE
                       : 0);
}

void llg_assoc_destroy(llg_assoc_t* array) {
    llg_container_notify_fn notify = array->notify;
    array->notify = NULL;
    llg_assoc_delete(array);
    array->notify = notify;
    free(array->entries);
    memset(array, 0, sizeof(*array));
}

static void llg_assoc_reserve(llg_assoc_t* array, size_t needed) {
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

size_t llg_assoc_count(const llg_assoc_t* array) { return array->size; }

sv4_t llg_assoc_value_at(const llg_assoc_t* array, size_t index) {
    if (!array || index >= array->size) return SV4_C(0, 1);
    return array->entries[index].value;
}

sv4_t llg_assoc_reduce(const llg_assoc_t* array, int operation) {
    sv4_t result = llg_reduce_identity(array->element_width,
                                       array->element_signed, operation);
    for (size_t i = 0; i < array->size; ++i)
        result = llg_reduce_step(result, array->entries[i].value, operation);
    return result;
}

sv4_t llg_assoc_reduce_with(const llg_assoc_t* array, int operation,
                            uint32_t result_width, int8_t result_signed,
                            int result_two_state, llg_container_eval_fn eval,
                            void* context) {
    llg_check_element_type(result_width);
    sv4_t result = llg_reduce_identity(result_width, result_signed, operation);
    for (size_t i = 0; i < array->size; ++i) {
        sv4_t index = array->key_kind == LLG_ASSOC_INTEGRAL
            ? array->entries[i].integral_key
            : sv4_from_u64(0, 32, 1);
        sv4_t value = llg_container_eval(
            eval, array->entries[i].value, index, context);
        value = llg_element_assign(value, result_width, result_signed,
                                   result_two_state);
        result = llg_element_assign(
            llg_reduce_step(result, value, operation), result_width,
            result_signed, result_two_state);
    }
    return result;
}

static int llg_assoc_normalize_key(const llg_assoc_t* array, sv4_t input,
                                   sv4_t* output) {
    llg_assoc_check_kind(array, LLG_ASSOC_INTEGRAL);
    // IEEE 1800-2009 7.8.4/7.8.6 makes any 4-state index expression
    // containing X/Z invalid. Do not let a narrowing or two-state index cast
    // erase the evidence before validation.
    if (sv4_is_unknown(input)) return 0;
    if (array->key_width) {
        *output = sv4_cast(input, array->key_width, array->key_signed);
        if (array->key_two_state) *output = sv4_to_two_state(*output);
    } else {
        // A wildcard index has no declared width.  Normalize to the model
        // width after applying the index expression's signed extension so
        // equal integral values from different operand widths share one key.
        // Keeping the model-sized value avoids host-integer narrowing.
        *output = sv4_cast(input, LLG_MAX_WIDTH, input.is_signed);
        output->is_signed = 0;
    }
    return !sv4_is_unknown(*output);
}

static int llg_integral_compare(sv4_t a, sv4_t b) {
    if (a.is_signed != b.is_signed)
        llg_container_fatal("incompatible associative-array key metadata");
    if (a.is_signed) {
        if (a.width != b.width)
            llg_container_fatal("non-normalized signed associative-array key");
        uint32_t sign = a.width - 1;
        int a_negative = (int)((a.bits[sign / 64] >> (sign % 64)) & 1u);
        int b_negative = (int)((b.bits[sign / 64] >> (sign % 64)) & 1u);
        if (a_negative != b_negative) return a_negative ? -1 : 1;
    }
    size_t a_limbs = (a.width + 63u) / 64u;
    size_t b_limbs = (b.width + 63u) / 64u;
    size_t limbs = a_limbs > b_limbs ? a_limbs : b_limbs;
    while (limbs--) {
        uint64_t av = limbs < a_limbs ? a.bits[limbs] : 0;
        uint64_t bv = limbs < b_limbs ? b.bits[limbs] : 0;
        if (av < bv) return -1;
        if (av > bv) return 1;
    }
    return 0;
}

static size_t llg_assoc_integral_position(const llg_assoc_t* array, sv4_t key,
                                          int* found) {
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

sv4_t llg_assoc_get_integral(const llg_assoc_t* array, sv4_t key) {
    sv4_t normalized;
    int found = 0;
    if (!llg_assoc_normalize_key(array, key, &normalized)) {
        llg_container_warning("invalid associative-array integral key read");
    } else {
        size_t position = llg_assoc_integral_position(array, normalized, &found);
        if (found) return array->entries[position].value;
    }
    return array->default_value;
}

int llg_assoc_set_integral(llg_assoc_t* array, sv4_t key, sv4_t value) {
    sv4_t normalized;
    if (!llg_assoc_normalize_key(array, key, &normalized)) {
        llg_container_warning("invalid associative-array integral key write");
        return 0;
    }
    int found;
    size_t position = llg_assoc_integral_position(array, normalized, &found);
    sv4_t assigned = llg_element_assign(
        value, array->element_width, array->element_signed,
        array->element_two_state);
    int shape_changed = !found;
    int contents_changed = !found;
    if (found && !sv4_same(array->entries[position].value, assigned))
        contents_changed = 1;
    if (!found) {
        if (array->size == SIZE_MAX)
            llg_container_fatal("associative-array size overflow");
        llg_assoc_reserve(array, array->size + 1);
        if (position < array->size)
            memmove(array->entries + position + 1, array->entries + position,
                    (array->size - position) * sizeof(*array->entries));
        memset(array->entries + position, 0, sizeof(*array->entries));
        array->entries[position].integral_key = normalized;
        ++array->size;
    }
    array->entries[position].value = assigned;
    llg_notify(array->notify, array->contents_dependency,
               array->shape_dependency,
               (contents_changed ? LLG_CONTAINER_CHANGED_CONTENTS : 0) |
                   (shape_changed ? LLG_CONTAINER_CHANGED_SHAPE : 0));
    return 1;
}

int llg_assoc_exists_integral(const llg_assoc_t* array, sv4_t key) {
    sv4_t normalized;
    if (!llg_assoc_normalize_key(array, key, &normalized)) return 0;
    int found;
    (void)llg_assoc_integral_position(array, normalized, &found);
    return found;
}

int llg_assoc_delete_integral(llg_assoc_t* array, sv4_t key) {
    sv4_t normalized;
    if (!llg_assoc_normalize_key(array, key, &normalized)) return 0;
    int found;
    size_t position = llg_assoc_integral_position(array, normalized, &found);
    if (!found) return 0;
    if (position + 1 < array->size)
        memmove(array->entries + position, array->entries + position + 1,
                (array->size - position - 1) * sizeof(*array->entries));
    --array->size;
    llg_notify(array->notify, array->contents_dependency,
               array->shape_dependency,
               LLG_CONTAINER_CHANGED_CONTENTS | LLG_CONTAINER_CHANGED_SHAPE);
    return 1;
}

void llg_assoc_set_default(llg_assoc_t* array, sv4_t value) {
    sv4_t assigned = llg_element_assign(
        value, array->element_width, array->element_signed,
        array->element_two_state);
    int changed = !array->has_default_value ||
                  !sv4_same(array->default_value, assigned);
    array->default_value = assigned;
    array->has_default_value = 1;
    llg_notify(array->notify, array->contents_dependency,
               array->shape_dependency,
               changed ? LLG_CONTAINER_CHANGED_CONTENTS : 0);
}

void llg_assoc_reset_default(llg_assoc_t* array) {
    sv4_t default_value = llg_element_default(
        array->element_width, array->element_signed, array->element_two_state);
    int changed = array->has_default_value ||
                  !sv4_same(array->default_value, default_value);
    array->default_value = default_value;
    array->has_default_value = 0;
    llg_notify(array->notify, array->contents_dependency,
               array->shape_dependency,
               changed ? LLG_CONTAINER_CHANGED_CONTENTS : 0);
}

static int llg_assoc_integral_traversal(const llg_assoc_t* array, sv4_t* key,
                                        int direction, int endpoint) {
    llg_assoc_check_kind(array, LLG_ASSOC_INTEGRAL);
    if (!array->key_width)
        llg_container_fatal("wildcard associative-array traversal is illegal");
    if (!array->size) return 0;
    if (endpoint) {
        *key = array->entries[direction > 0 ? 0 : array->size - 1].integral_key;
        return 1;
    }
    sv4_t normalized;
    if (!llg_assoc_normalize_key(array, *key, &normalized)) return 0;
    int found;
    size_t position = llg_assoc_integral_position(array, normalized, &found);
    if (direction > 0) {
        if (found) ++position;
        if (position >= array->size) return 0;
    } else {
        if (position == 0) return 0;
        --position;
    }
    *key = array->entries[position].integral_key;
    return 1;
}

int llg_assoc_first_integral(const llg_assoc_t* a, sv4_t* key) {
    return llg_assoc_integral_traversal(a, key, 1, 1);
}
int llg_assoc_last_integral(const llg_assoc_t* a, sv4_t* key) {
    return llg_assoc_integral_traversal(a, key, -1, 1);
}
int llg_assoc_next_integral(const llg_assoc_t* a, sv4_t* key) {
    return llg_assoc_integral_traversal(a, key, 1, 0);
}
int llg_assoc_prev_integral(const llg_assoc_t* a, sv4_t* key) {
    return llg_assoc_integral_traversal(a, key, -1, 0);
}

static int llg_assoc_string_compare(const void* a, size_t a_len, const void* b,
                                    size_t b_len) {
    size_t shared = a_len < b_len ? a_len : b_len;
    int cmp = shared ? memcmp(a, b, shared) : 0;
    if (cmp) return cmp < 0 ? -1 : 1;
    return a_len < b_len ? -1 : a_len > b_len;
}

static size_t llg_assoc_string_position(const llg_assoc_t* array,
                                        const void* key, size_t key_length,
                                        int* found) {
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
                                array->entries[low].string_length, key,
                                key_length) == 0;
    return low;
}

static void llg_check_string_key(const llg_assoc_t* array, const void* key,
                                 size_t key_length) {
    llg_assoc_check_kind(array, LLG_ASSOC_STRING);
    if (!key && key_length)
        llg_container_fatal("null associative-array string key");
}

sv4_t llg_assoc_get_string(const llg_assoc_t* array, const void* key,
                           size_t key_length) {
    llg_check_string_key(array, key, key_length);
    int found;
    size_t position = llg_assoc_string_position(array, key, key_length, &found);
    if (found) return array->entries[position].value;
    return array->default_value;
}

int llg_assoc_set_string(llg_assoc_t* array, const void* key, size_t key_length,
                         sv4_t value) {
    llg_check_string_key(array, key, key_length);
    int found;
    size_t position = llg_assoc_string_position(array, key, key_length, &found);
    sv4_t assigned = llg_element_assign(
        value, array->element_width, array->element_signed,
        array->element_two_state);
    int shape_changed = !found;
    int contents_changed = !found;
    if (found && !sv4_same(array->entries[position].value, assigned))
        contents_changed = 1;
    if (!found) {
        if (array->size == SIZE_MAX)
            llg_container_fatal("associative-array size overflow");
        unsigned char* copy = llg_alloc_items(key_length, 1);
        if (key_length) memcpy(copy, key, key_length);
        llg_assoc_reserve(array, array->size + 1);
        if (position < array->size)
            memmove(array->entries + position + 1, array->entries + position,
                    (array->size - position) * sizeof(*array->entries));
        memset(array->entries + position, 0, sizeof(*array->entries));
        array->entries[position].string_key = copy;
        array->entries[position].string_length = key_length;
        ++array->size;
    }
    array->entries[position].value = assigned;
    llg_notify(array->notify, array->contents_dependency,
               array->shape_dependency,
               (contents_changed ? LLG_CONTAINER_CHANGED_CONTENTS : 0) |
                   (shape_changed ? LLG_CONTAINER_CHANGED_SHAPE : 0));
    return 1;
}

int llg_assoc_exists_string(const llg_assoc_t* array, const void* key,
                            size_t key_length) {
    llg_check_string_key(array, key, key_length);
    int found;
    (void)llg_assoc_string_position(array, key, key_length, &found);
    return found;
}

int llg_assoc_delete_string(llg_assoc_t* array, const void* key,
                            size_t key_length) {
    llg_check_string_key(array, key, key_length);
    int found;
    size_t position = llg_assoc_string_position(array, key, key_length, &found);
    if (!found) return 0;
    free(array->entries[position].string_key);
    if (position + 1 < array->size)
        memmove(array->entries + position, array->entries + position + 1,
                (array->size - position - 1) * sizeof(*array->entries));
    --array->size;
    llg_notify(array->notify, array->contents_dependency,
               array->shape_dependency,
               LLG_CONTAINER_CHANGED_CONTENTS | LLG_CONTAINER_CHANGED_SHAPE);
    return 1;
}

static int llg_assoc_string_endpoint(const llg_assoc_t* array, int last,
                                     const unsigned char** key,
                                     size_t* key_length) {
    llg_assoc_check_kind(array, LLG_ASSOC_STRING);
    if (!array->size) return 0;
    size_t position = last ? array->size - 1 : 0;
    *key = array->entries[position].string_key;
    *key_length = array->entries[position].string_length;
    return 1;
}

int llg_assoc_first_string(const llg_assoc_t* a, const unsigned char** key,
                           size_t* length) {
    return llg_assoc_string_endpoint(a, 0, key, length);
}
int llg_assoc_last_string(const llg_assoc_t* a, const unsigned char** key,
                          size_t* length) {
    return llg_assoc_string_endpoint(a, 1, key, length);
}

int llg_assoc_next_string(const llg_assoc_t* array, const void* current,
                          size_t current_length, const unsigned char** key,
                          size_t* key_length) {
    llg_check_string_key(array, current, current_length);
    int found;
    size_t position = llg_assoc_string_position(array, current, current_length,
                                                &found);
    if (found) ++position;
    if (position >= array->size) return 0;
    *key = array->entries[position].string_key;
    *key_length = array->entries[position].string_length;
    return 1;
}

int llg_assoc_prev_string(const llg_assoc_t* array, const void* current,
                          size_t current_length, const unsigned char** key,
                          size_t* key_length) {
    llg_check_string_key(array, current, current_length);
    int found;
    size_t position = llg_assoc_string_position(array, current, current_length,
                                                &found);
    if (position == 0) return 0;
    --position;
    *key = array->entries[position].string_key;
    *key_length = array->entries[position].string_length;
    return 1;
}

void llg_assoc_copy(llg_assoc_t* dst, const llg_assoc_t* src) {
    if (dst == src) return;
    if (dst->key_kind != src->key_kind || dst->key_width != src->key_width ||
        dst->key_signed != src->key_signed ||
        dst->key_two_state != src->key_two_state)
        llg_container_fatal("incompatible associative-array index types");

    llg_assoc_entry_t* entries = llg_alloc_items(src->size, sizeof(*entries));
    if (src->size) memset(entries, 0, src->size * sizeof(*entries));
    for (size_t i = 0; i < src->size; ++i) {
        entries[i].integral_key = src->entries[i].integral_key;
        entries[i].value = llg_element_assign(
            src->entries[i].value, dst->element_width, dst->element_signed,
            dst->element_two_state);
        entries[i].string_length = src->entries[i].string_length;
        if (src->entries[i].string_length) {
            entries[i].string_key = llg_alloc_items(
                src->entries[i].string_length, 1);
            memcpy(entries[i].string_key, src->entries[i].string_key,
                   src->entries[i].string_length);
        }
    }
    sv4_t default_value = llg_element_assign(
        src->default_value, dst->element_width, dst->element_signed,
        dst->element_two_state);

    int shape_changed = dst->size != src->size;
    int contents_changed = shape_changed;
    if (!sv4_same(dst->default_value, default_value) ||
        dst->has_default_value != src->has_default_value)
        contents_changed = 1;
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
                               memcmp(dst->entries[i].string_key,
                                      src->entries[i].string_key,
                                      src->entries[i].string_length) != 0);
            }
            if (key_changed) {
                shape_changed = 1;
                contents_changed = 1;
                break;
            }
            if (!sv4_same(dst->entries[i].value, entries[i].value)) {
                contents_changed = 1;
                break;
            }
        }
    }
    llg_container_notify_fn notify = dst->notify;
    sv4_t* contents_dependency = dst->contents_dependency;
    sv4_t* shape_dependency = dst->shape_dependency;

    dst->notify = NULL;
    llg_assoc_delete(dst);
    dst->notify = notify;
    free(dst->entries);
    dst->entries = entries;
    dst->size = src->size;
    dst->capacity = src->size;
    dst->default_value = default_value;
    dst->has_default_value = src->has_default_value;
    llg_notify(notify, contents_dependency, shape_dependency,
               (contents_changed ? LLG_CONTAINER_CHANGED_CONTENTS : 0) |
                   (shape_changed ? LLG_CONTAINER_CHANGED_SHAPE : 0));
}
