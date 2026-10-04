
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

// Sorting keys. Each element's key is evaluated once, then a stable bottom-up
// merge sort orders an index permutation by those keys (no recursion, no
// per-comparison allocation). The insertion sort this replaces never moved an
// element whose key has an X/Z bit (its comparison is not true) and never let
// another element cross one, so such elements are fixed barriers and each
// maximal run of known keys between them is sorted independently.
typedef struct {
    const sv4_t* keys;
    const uint64_t* fast;    // order-preserving keys when all share one <=64 bit shape
    int descending;
} llg_sort_ctx_t;

// Word `index` of a known key extended to `width` bits (sign-extended only when
// the comparison is signed), matching the packed relational operators.
static uint64_t llg_sort_key_word(sv4_t key, uint32_t width, int sign_extend,
                                  size_t index) {
    uint32_t own = llg_sv4_width(key);
    uint64_t word = llg_sv4_word(key, index, LLG_SV4_BITS);
    if (sign_extend && own && own < width) {
        size_t top = (own - 1) / 64;
        unsigned offset = (unsigned)((own - 1) % 64);
        if ((llg_sv4_word(key, top, LLG_SV4_BITS) >> offset) & 1u) {
            if (index == top && offset < 63) word |= ~UINT64_C(0) << (offset + 1);
            else if (index > top) word = ~UINT64_C(0);
        }
    }
    if (width % 64 && index == (width - 1) / 64)
        word &= (UINT64_C(1) << (width % 64)) - 1u;
    return word;
}

static int llg_sort_key_negative(sv4_t key) {
    uint32_t width = llg_sv4_width(key);
    if (!llg_sv4_signed(key) || !width) return 0;
    return (int)((llg_sv4_word(key, (width - 1) / 64, LLG_SV4_BITS) >>
                  ((width - 1) % 64)) & 1u);
}

// Strict "a < b" for known keys; operand widths and signs follow sv4_lt.
static int llg_sort_key_less(sv4_t a, sv4_t b) {
    uint32_t width = llg_sv4_width(a) > llg_sv4_width(b) ? llg_sv4_width(a)
                                                         : llg_sv4_width(b);
    int sign_extend = llg_sv4_signed(a) && llg_sv4_signed(b);
    for (size_t word = width ? (width - 1) / 64 + 1 : 0; word-- > 0;) {
        uint64_t left = llg_sort_key_word(a, width, sign_extend, word);
        uint64_t right = llg_sort_key_word(b, width, sign_extend, word);
        if (left == right) continue;
        if (sign_extend) {
            int left_negative = llg_sort_key_negative(a);
            int right_negative = llg_sort_key_negative(b);
            if (left_negative != right_negative) return left_negative;
        }
        return left < right;
    }
    return 0;
}

// True when element `later` must be placed before `earlier`; strictness keeps
// equal keys in their original order, for rsort as well.
static int llg_sort_before(const llg_sort_ctx_t* ctx, size_t later,
                           size_t earlier) {
    if (ctx->descending) {
        size_t swap = later;
        later = earlier;
        earlier = swap;
    }
    if (ctx->fast) return ctx->fast[later] < ctx->fast[earlier];
    return llg_sort_key_less(ctx->keys[later], ctx->keys[earlier]);
}

// Stable bottom-up merge of order[0, count) using scratch[0, count).
static void llg_sort_merge(const llg_sort_ctx_t* ctx, size_t* order,
                           size_t* scratch, size_t count) {
    size_t* source = order;
    size_t* target = scratch;
    for (size_t run = 1; run < count; run *= 2) {
        for (size_t low = 0; low < count; low += 2 * run) {
            size_t middle = low + run < count ? low + run : count;
            size_t high = low + 2 * run < count ? low + 2 * run : count;
            size_t left = low, right = middle, out = low;
            while (left < middle && right < high) {
                // The right element goes first only when strictly before.
                if (llg_sort_before(ctx, source[right], source[left]))
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

// Order the positions [0, count) of `keys`: on return order[i] names the
// original position whose element belongs at i. `scratch` holds count entries.
// Returns whether any element moves.
static int llg_sort_permutation(const sv4_t* keys, size_t count,
                                int descending, size_t* order,
                                size_t* scratch) {
    for (size_t i = 0; i < count; ++i) order[i] = i;
    if (count < 2) return 0;

    uint32_t shape_width = llg_sv4_width(keys[0]);
    int shape_signed = llg_sv4_signed(keys[0]) != 0;
    int uniform = shape_width <= 64;
    for (size_t i = 1; uniform && i < count; ++i)
        uniform = llg_sv4_width(keys[i]) == shape_width &&
                  (llg_sv4_signed(keys[i]) != 0) == shape_signed;
    uint64_t* fast = NULL;
    if (uniform) {
        fast = llg_alloc_items(count, sizeof(*fast));
        uint64_t flip = shape_signed && shape_width
            ? UINT64_C(1) << (shape_width - 1) : 0;
        for (size_t i = 0; i < count; ++i)
            fast[i] = llg_sort_key_word(keys[i], shape_width, 0, 0) ^ flip;
    }

    llg_sort_ctx_t ctx = { keys, fast, descending };
    size_t start = 0;
    for (size_t i = 0; i <= count; ++i) {
        if (i < count && !sv4_is_unknown(keys[i])) continue;
        if (i - start > 1)
            llg_sort_merge(&ctx, order + start, scratch, i - start);
        start = i + 1;
    }
    free(fast);

    for (size_t i = 0; i < count; ++i)
        if (order[i] != i) return 1;
    return 0;
}

// Returns whether any element moved. `item.index` is the element's position
// before sorting (LRM 7.12.4), so keys do not depend on the sort progress.
static int llg_method_sort(sv4_t* data, uint64_t* element_ids, size_t count,
                           llg_container_eval_fn eval, void* context,
                           int descending) {
    if (count < 2) return 0;
    size_t* order = llg_alloc_items(count, 2 * sizeof(*order));
    size_t* scratch = order + count;

    sv4_t* owned_keys = NULL;
    if (eval) {
        owned_keys = llg_alloc_items(count, sizeof(*owned_keys));
        for (size_t i = 0; i < count; ++i) owned_keys[i] = (sv4_t)SV4_EMPTY;
        for (size_t i = 0; i < count; ++i) {
            sv4_t index = sv4_from_u64((uint64_t)i, 32, 1);
            eval(&owned_keys[i], data[i], index, context);
            sv4_destroy(&index);
        }
    }
    const sv4_t* keys = owned_keys ? owned_keys : data;
    int changed = llg_sort_permutation(keys, count, descending, order, scratch);

    for (size_t i = 0; changed && i < count; ++i) {
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

    if (owned_keys) {
        sv4_destroy_array(owned_keys, count);
        free(owned_keys);
    }
    free(order);
    return changed;
}

void llg_fixed_order_init(llg_fixed_order_t* order, uint64_t count,
                          uint64_t row_cells) {
    if (!order) llg_container_fatal("null fixed-array ordering workspace");
    size_t elements = llg_checked_count(count, 2 * sizeof(size_t));
    size_t cells = llg_checked_count(row_cells, sizeof(sv4_t));
    order->count = elements;
    order->row_cells = cells;
    order->keys = llg_alloc_items(elements, sizeof(*order->keys));
    order->order = llg_alloc_items(elements, 2 * sizeof(*order->order));
    order->row = llg_alloc_items(cells, sizeof(*order->row));
    for (size_t i = 0; i < elements; ++i) order->keys[i] = (sv4_t)SV4_EMPTY;
    for (size_t i = 0; i < cells; ++i) order->row[i] = (sv4_t)SV4_EMPTY;
}

int llg_fixed_order_sort(llg_fixed_order_t* order, int descending) {
    if (!order || (order->count && (!order->keys || !order->order)))
        llg_container_fatal("malformed fixed-array ordering workspace");
    return llg_sort_permutation(order->keys, order->count, descending,
                                order->order, order->order + order->count);
}

void llg_fixed_order_destroy(void* object) {
    llg_fixed_order_t* order = object;
    if (!order) return;
    if (order->keys) {
        sv4_destroy_array(order->keys, order->count);
        free(order->keys);
    }
    if (order->row) {
        sv4_destroy_array(order->row, order->row_cells);
        free(order->row);
    }
    free(order->order);
    order->keys = NULL;
    order->row = NULL;
    order->order = NULL;
    order->count = 0;
    order->row_cells = 0;
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
    return llg_method_sort(data, element_ids, count, eval, context,
                           method == LLG_CONTAINER_METHOD_RSORT);
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
