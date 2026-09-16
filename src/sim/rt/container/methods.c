
static int llg_method_is_index(int method) {
    return method == LLG_CONTAINER_METHOD_FIND_INDEX ||
           method == LLG_CONTAINER_METHOD_FIND_FIRST_INDEX ||
           method == LLG_CONTAINER_METHOD_FIND_LAST_INDEX ||
           method == LLG_CONTAINER_METHOD_UNIQUE_INDEX;
}

static int llg_method_is_locator(int method) {
    return method >= LLG_CONTAINER_METHOD_FIND &&
           method <= LLG_CONTAINER_METHOD_FIND_LAST_INDEX;
}

static int llg_method_is_last(int method) {
    return method == LLG_CONTAINER_METHOD_FIND_LAST ||
           method == LLG_CONTAINER_METHOD_FIND_LAST_INDEX;
}

static int llg_method_is_first_only(int method) {
    return method == LLG_CONTAINER_METHOD_FIND_FIRST ||
           method == LLG_CONTAINER_METHOD_FIND_FIRST_INDEX ||
           method == LLG_CONTAINER_METHOD_FIND_LAST ||
           method == LLG_CONTAINER_METHOD_FIND_LAST_INDEX;
}

static void llg_method_assign_values(llg_queue_t* dst, const sv4_t* values,
                                     const sv4_t* indices, size_t count,
                                     int method, llg_container_eval_fn eval,
                                     void* context) {
    if (!dst || (!values && count))
        llg_container_fatal("malformed array-method result");
    if (method < LLG_CONTAINER_METHOD_FIND ||
        method > LLG_CONTAINER_METHOD_UNIQUE_INDEX)
        llg_container_fatal("invalid queue-valued array method");

    sv4_t* result = llg_alloc_items(count, sizeof(*result));
    size_t result_count = 0;
    if (llg_method_is_locator(method)) {
        if (count) {
            size_t index = llg_method_is_last(method) ? count - 1 : 0;
            for (;;) {
                sv4_t item_index = indices
                    ? sv4_clone(&indices[index])
                    : sv4_from_u64((uint64_t)index, 32, 1);
                sv4_t selected = llg_container_eval(
                    eval, values[index], item_index, context);
                int selected_bool = sv4_to_bool(selected);
                sv4_destroy(&selected);
                sv4_destroy(&item_index);
                if (selected_bool) {
                    result[result_count++] = llg_method_is_index(method)
                        ? (indices ? sv4_clone(&indices[index])
                                   : sv4_from_u64((uint64_t)index, 32, 1))
                        : sv4_clone(&values[index]);
                    if (llg_method_is_first_only(method)) break;
                }
                if (llg_method_is_last(method)) {
                    if (index == 0) break;
                    --index;
                } else {
                    ++index;
                    if (index == count) break;
                }
            }
        }
    } else if (method == LLG_CONTAINER_METHOD_MIN ||
               method == LLG_CONTAINER_METHOD_MAX) {
        if (count) {
            size_t best = 0;
            sv4_t best_index = indices ? sv4_clone(&indices[0]) : sv4_from_u64(0, 32, 1);
            sv4_t best_key = llg_container_eval(
                eval, values[0], best_index, context);
            for (size_t index = 1; index < count; ++index) {
                sv4_t item_index = indices
                    ? sv4_clone(&indices[index])
                    : sv4_from_u64((uint64_t)index, 32, 1);
                sv4_t key = llg_container_eval(
                    eval, values[index], item_index, context);
                sv4_t comparison = method == LLG_CONTAINER_METHOD_MIN
                    ? sv4_lt(key, best_key)
                    : sv4_gt(key, best_key);
                if (sv4_to_bool(comparison)) {
                    best = index;
                    sv4_move(&best_key, &key);
                }
                sv4_destroy(&comparison);
                sv4_destroy(&key);
                sv4_destroy(&item_index);
            }
            result[result_count++] = sv4_clone(&values[best]);
            sv4_destroy(&best_index);
            sv4_destroy(&best_key);
        }
    } else if (method == LLG_CONTAINER_METHOD_UNIQUE ||
               method == LLG_CONTAINER_METHOD_UNIQUE_INDEX) {
        sv4_t* seen = llg_alloc_items(count, sizeof(*seen));
        size_t seen_count = 0;
        for (size_t index = 0; index < count; ++index) {
            sv4_t item_index = indices
                ? sv4_clone(&indices[index])
                : sv4_from_u64((uint64_t)index, 32, 1);
            sv4_t key = llg_container_eval(
                eval, values[index], item_index, context);
            int duplicate = 0;
            for (size_t seen_index = 0; seen_index < seen_count; ++seen_index) {
                if (sv4_same(seen[seen_index], key)) {
                    duplicate = 1;
                    break;
                }
            }
            if (!duplicate) {
                seen[seen_count++] = key; // move into the new slot
                key = (sv4_t)SV4_EMPTY;
                result[result_count++] = method == LLG_CONTAINER_METHOD_UNIQUE_INDEX
                    ? (indices ? sv4_clone(&indices[index])
                               : sv4_from_u64((uint64_t)index, 32, 1))
                    : sv4_clone(&values[index]);
            }
            sv4_destroy(&item_index);
            sv4_destroy(&key);
        }
        sv4_destroy_array(seen, seen_count);
        free(seen);
    } else {
        llg_container_fatal("invalid queue-valued array method");
    }
    llg_queue_assign_values(dst, result, result_count);
    sv4_destroy_array(result, result_count);
    free(result);
}

void llg_dyn_method_assign(llg_queue_t* dst, const llg_dyn_array_t* src,
                           int method, llg_container_eval_fn eval,
                           void* context) {
    if (!src) llg_container_fatal("null dynamic-array method source");
    llg_method_assign_values(dst, src->data, NULL, src->size, method, eval,
                             context);
}

void llg_queue_method_assign(llg_queue_t* dst, const llg_queue_t* src,
                             int method, llg_container_eval_fn eval,
                             void* context) {
    if (!src) llg_container_fatal("null queue method source");
    llg_method_assign_values(dst, src->data, NULL, src->size, method, eval,
                             context);
}

void llg_assoc_method_assign(llg_queue_t* dst, const llg_assoc_t* src,
                             int method, llg_container_eval_fn eval,
                             void* context) {
    if (!dst || !src)
        llg_container_fatal("null associative-array method source or result");
    if (method < LLG_CONTAINER_METHOD_FIND ||
        method > LLG_CONTAINER_METHOD_UNIQUE_INDEX)
        llg_container_fatal("invalid associative-array method");
    if (llg_method_is_index(method) && src->key_kind != LLG_ASSOC_INTEGRAL)
        llg_container_fatal(
            "string-keyed associative index result requires a string queue");

    sv4_t* values = llg_alloc_items(src->size, sizeof(*values));
    sv4_t* indices = src->key_kind == LLG_ASSOC_INTEGRAL
        ? llg_alloc_items(src->size, sizeof(*indices))
        : NULL;
    for (size_t index = 0; index < src->size; ++index) {
        values[index] = src->entries[index].value;
        if (indices) indices[index] = src->entries[index].integral_key;
    }
    llg_method_assign_values(dst, values, indices, src->size, method, eval,
                             context);
    free(indices);
    free(values);
}

static llg_rng_state_t llg_container_rng_state = {
    UINT64_C(0), UINT64_C(0), UINT64_C(0)
};
static int llg_container_rng_initialized;

void llg_container_seed(uint64_t seed) {
    llg_rng_state_seed(&llg_container_rng_state, seed);
    llg_container_rng_initialized = 1;
}

static int llg_method_reorder(sv4_t* data, uint64_t* element_ids,
                              size_t count, int method,
                              llg_container_eval_fn eval, void* context) {
    if (!data && count)
        llg_container_fatal("malformed array-method storage");
    if (method == LLG_CONTAINER_METHOD_REVERSE) {
        int changed = 0;
        for (size_t left = 0; left < count / 2; ++left) {
            size_t right = count - left - 1;
            if (!sv4_same(data[left], data[right]) ||
                (element_ids && element_ids[left] != element_ids[right]))
                changed = 1;
            sv4_t value = SV4_EMPTY;
            sv4_move(&value, &data[left]);
            sv4_move(&data[left], &data[right]);
            sv4_move(&data[right], &value);
            if (element_ids) {
                uint64_t identity = element_ids[left];
                element_ids[left] = element_ids[right];
                element_ids[right] = identity;
            }
        }
        return changed;
    }
    if (method == LLG_CONTAINER_METHOD_SHUFFLE) {
        int changed = 0;
        for (size_t index = count; index > 1; --index) {
            if (index > UINT32_MAX)
                llg_container_fatal("shuffle size exceeds random range");
            if (!llg_container_rng_initialized) llg_container_seed(0);
            size_t other = (size_t)llg_rng_state_uniform(
                &llg_container_rng_state, (uint32_t)(index - 1), 0);
            if (other == index - 1) continue;
            if (!sv4_same(data[other], data[index - 1]) ||
                (element_ids && element_ids[other] != element_ids[index - 1]))
                changed = 1;
            sv4_t value = SV4_EMPTY;
            sv4_move(&value, &data[other]);
            sv4_move(&data[other], &data[index - 1]);
            sv4_move(&data[index - 1], &value);
            if (element_ids) {
                uint64_t identity = element_ids[other];
                element_ids[other] = element_ids[index - 1];
                element_ids[index - 1] = identity;
            }
        }
        return changed;
    }
    if (method != LLG_CONTAINER_METHOD_SORT &&
        method != LLG_CONTAINER_METHOD_RSORT)
        llg_container_fatal("invalid in-place array method");
    int descending = method == LLG_CONTAINER_METHOD_RSORT;
    int changed = 0;
    for (size_t index = 1; index < count; ++index) {
        sv4_t value = SV4_EMPTY;
        sv4_move(&value, &data[index]);
        uint64_t identity = element_ids ? element_ids[index] : 0;
        size_t position = index;
        while (position) {
            sv4_t previous_index =
                sv4_from_u64((uint64_t)(position - 1), 32, 1);
            sv4_t value_index = sv4_from_u64((uint64_t)index, 32, 1);
            sv4_t previous_key = llg_container_eval(
                eval, data[position - 1], previous_index, context);
            sv4_t value_key = llg_container_eval(
                eval, value, value_index, context);
            sv4_t comparison = descending
                ? sv4_lt(previous_key, value_key)
                : sv4_lt(value_key, previous_key);
            int ordered = sv4_to_bool(comparison);
            sv4_destroy(&comparison);
            sv4_destroy(&value_key);
            sv4_destroy(&previous_key);
            sv4_destroy(&value_index);
            sv4_destroy(&previous_index);
            if (!ordered) break;
            sv4_move(&data[position], &data[position - 1]);
            if (element_ids) element_ids[position] = element_ids[position - 1];
            position--;
            changed = 1;
        }
        sv4_move(&data[position], &value);
        if (element_ids) element_ids[position] = identity;
    }
    return changed;
}

void llg_dyn_method(llg_dyn_array_t* array, int method,
                    llg_container_eval_fn eval, void* context) {
    if (!array) llg_container_fatal("null dynamic-array method target");
    int changed = llg_method_reorder(array->data, NULL, array->size, method,
                                     eval, context);
    if (changed)
        llg_notify(array->notify, array->contents_dependency,
                   array->shape_dependency, LLG_CONTAINER_CHANGED_CONTENTS);
}

void llg_queue_method(llg_queue_t* queue, int method,
                      llg_container_eval_fn eval, void* context) {
    if (!queue) llg_container_fatal("null queue method target");
    int changed = llg_method_reorder(queue->data, queue->element_ids,
                                     queue->size, method, eval, context);
    if (changed)
        llg_notify(queue->notify, queue->contents_dependency,
                   queue->shape_dependency, LLG_CONTAINER_CHANGED_CONTENTS);
}
