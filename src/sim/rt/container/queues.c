
static void llg_queue_reserve(llg_queue_t* queue, size_t needed) {
    if (needed <= queue->capacity) return;
    size_t capacity = queue->capacity ? queue->capacity : 4;
    while (capacity < needed) {
        if (capacity > SIZE_MAX / 2) {
            capacity = needed;
            break;
        }
        capacity *= 2;
    }
    if (capacity > queue->limit) capacity = queue->limit;
    if (capacity < needed)
        llg_container_fatal("queue capacity exceeds declared bound");
    queue->data = llg_realloc_items(queue->data, capacity, sizeof(*queue->data));
    queue->element_ids = llg_realloc_items(
        queue->element_ids, capacity, sizeof(*queue->element_ids));
    queue->capacity = capacity;
}

struct llg_queue_cell {
    llg_queue_t* owner;
    struct llg_queue_cell* next;
    uint64_t identity;
    size_t refs;
    sv4_t value;
    uint8_t two_state;
};

/* Snapshot before storage is overwritten, and disconnect only the removed
 * identities. Surviving element references follow insertions and reorders. */
static void llg_queue_disconnect(llg_queue_t* queue, uint64_t identity) {
    struct llg_queue_cell** link = &queue->references;
    while (*link) {
        struct llg_queue_cell* cell = *link;
        if (identity && cell->identity != identity) {
            link = &cell->next;
            continue;
        }
        for (size_t i = 0; i < queue->size; ++i) {
            if (queue->element_ids[i] == cell->identity) {
                cell->value = queue->data[i];
                break;
            }
        }
        *link = cell->next;
        cell->next = NULL;
        cell->owner = NULL;
    }
}

void llg_queue_init(llg_queue_t* queue, uint32_t element_width,
                    int8_t element_signed, int element_two_state,
                    uint64_t maximum_elements) {
    llg_check_element_type(element_width);
    memset(queue, 0, sizeof(*queue));
    queue->element_width = element_width;
    queue->element_signed = !!element_signed;
    queue->element_two_state = !!element_two_state;
    queue->next_element_id = 1;
    queue->limit = maximum_elements == UINT64_MAX
                       ? SIZE_MAX
                       : llg_checked_count(maximum_elements, sizeof(sv4_t));
}

void llg_queue_destroy(llg_queue_t* queue) {
    llg_queue_disconnect(queue, 0);
    free(queue->data);
    free(queue->element_ids);
    memset(queue, 0, sizeof(*queue));
}

void llg_queue_delete(llg_queue_t* queue) {
    llg_queue_disconnect(queue, 0);
    int changed = queue->size != 0;
    queue->size = 0;
    llg_notify(queue->notify, queue->contents_dependency,
               queue->shape_dependency,
               changed ? LLG_CONTAINER_CHANGED_CONTENTS |
                            LLG_CONTAINER_CHANGED_SHAPE
                       : 0);
}

void llg_queue_copy(llg_queue_t* dst, const llg_queue_t* src) {
    llg_queue_disconnect(dst, 0);
    size_t count = src->size < dst->limit ? src->size : dst->limit;
    int shape_changed = dst->size != count;
    int contents_changed = shape_changed;
    if (!contents_changed) {
        for (size_t i = 0; i < count; ++i) {
            sv4_t assigned = llg_element_assign(
                src->data[i], dst->element_width, dst->element_signed,
                dst->element_two_state);
            if (!sv4_same(dst->data[i], assigned)) {
                contents_changed = 1;
                break;
            }
        }
    }
    llg_container_notify_fn notify = dst->notify;
    sv4_t* contents_dependency = dst->contents_dependency;
    sv4_t* shape_dependency = dst->shape_dependency;
    llg_queue_reserve(dst, count);
    for (size_t i = 0; i < count; ++i)
        dst->data[i] = llg_element_assign(
            src->data[i], dst->element_width, dst->element_signed,
            dst->element_two_state);
    dst->size = count;
    llg_queue_reset_element_ids(dst);
    if (count != src->size)
        llg_container_warning("bounded queue assignment discarded tail elements");
    llg_notify(notify, contents_dependency, shape_dependency,
               (contents_changed ? LLG_CONTAINER_CHANGED_CONTENTS : 0) |
                   (shape_changed ? LLG_CONTAINER_CHANGED_SHAPE : 0));
}

void llg_queue_assign_values(llg_queue_t* dst, const sv4_t* values,
                             size_t count) {
    size_t retained = count < dst->limit ? count : dst->limit;
    sv4_t* data = llg_alloc_items(retained, sizeof(*data));
    for (size_t i = 0; i < retained; ++i)
        data[i] = llg_element_assign(values[i], dst->element_width,
                                     dst->element_signed,
                                     dst->element_two_state);
    int shape_changed = dst->size != retained;
    int contents_changed = shape_changed;
    if (!contents_changed) {
        for (size_t i = 0; i < retained; ++i) {
            if (!sv4_same(dst->data[i], data[i])) {
                contents_changed = 1;
                break;
            }
        }
    }
    llg_container_notify_fn notify = dst->notify;
    sv4_t* contents_dependency = dst->contents_dependency;
    sv4_t* shape_dependency = dst->shape_dependency;
    llg_queue_disconnect(dst, 0);
    free(dst->data);
    free(dst->element_ids);
    dst->data = data;
    dst->element_ids = llg_alloc_items(retained, sizeof(*dst->element_ids));
    dst->size = retained;
    dst->capacity = retained;
    llg_queue_reset_element_ids(dst);
    if (retained != count)
        llg_container_warning(
            "bounded queue assignment pattern discarded tail elements");
    llg_notify(notify, contents_dependency, shape_dependency,
               (contents_changed ? LLG_CONTAINER_CHANGED_CONTENTS : 0) |
                   (shape_changed ? LLG_CONTAINER_CHANGED_SHAPE : 0));
}

sv4_t llg_queue_stream(const llg_queue_t* queue, uint32_t slice,
                       int right_to_left, int selector_kind, sv4_t first,
                       sv4_t second) {
    int64_t left;
    int64_t right;
    size_t count;
    llg_stream_bounds(selector_kind, first, second, queue->size, &left, &right,
                      &count);
    if (!count) {
        sv4_t empty;
        memset(&empty, 0, sizeof(empty));
        return empty;
    }
    if (count > (size_t)(LLG_MAX_WIDTH / queue->element_width))
        llg_container_fatal("streaming value exceeds model capacity");
    sv4_t* values = llg_alloc_items(count, sizeof(*values));
    for (size_t offset = 0; offset < count; ++offset) {
        int64_t index = llg_stream_index_at(left, right, offset);
        values[offset] = llg_queue_get(
            queue, sv4_from_i64(index, 64));
    }
    sv4_t result = llg_pack_stream_values(values, count, queue->element_width,
                                          slice, right_to_left);
    free(values);
    return result;
}

static void llg_queue_resize_default(llg_queue_t* queue, size_t size) {
    if (size > queue->limit)
        llg_container_fatal("streaming target exceeds bounded queue capacity");
    if (size <= queue->size) return;
    llg_queue_reserve(queue, size);
    sv4_t initial = llg_element_default(queue->element_width,
                                        queue->element_signed,
                                        queue->element_two_state);
    for (size_t index = queue->size; index < size; ++index) {
        queue->data[index] = initial;
        queue->element_ids[index] = llg_queue_new_element_id(queue);
    }
    queue->size = size;
    llg_notify(queue->notify, queue->contents_dependency,
               queue->shape_dependency,
               LLG_CONTAINER_CHANGED_CONTENTS | LLG_CONTAINER_CHANGED_SHAPE);
}

void llg_queue_unstream_assign(llg_queue_t* dst, sv4_t source,
                               uint32_t slice, int right_to_left,
                               int selector_kind, sv4_t first, sv4_t second) {
    int64_t left;
    int64_t right;
    size_t count = llg_stream_unpacked_values(
        source, dst->element_width, selector_kind, first, second, &left, &right);
    sv4_t unpacked = sv4_unstream(source, slice, right_to_left);
    if (!selector_kind) {
        sv4_t* values = llg_alloc_items(count, sizeof(*values));
        uint32_t cursor = unpacked.width;
        for (size_t offset = 0; offset < count; ++offset) {
            uint32_t right_bit = cursor - dst->element_width;
            values[offset] = sv4_part_select(
                unpacked, (int64_t)cursor - 1, (int64_t)right_bit);
            cursor = right_bit;
        }
        llg_queue_assign_values(dst, values, count);
        free(values);
        return;
    }
    if (left < 0 || right < 0) {
        if (count) llg_container_fatal(
            "queue streaming target selector must be a nonnegative range");
        return;
    }
    int64_t high = left > right ? left : right;
    if (high == INT64_MAX)
        llg_container_fatal("queue streaming target index overflows size");
    if ((uint64_t)high >= (uint64_t)dst->size)
        llg_queue_resize_default(dst, (size_t)high + 1);
    uint32_t cursor = unpacked.width;
    for (size_t offset = 0; offset < count; ++offset) {
        uint32_t right_bit = cursor - dst->element_width;
        sv4_t value = sv4_part_select(
            unpacked, (int64_t)cursor - 1, (int64_t)right_bit);
        llg_queue_set(
            dst,
            sv4_from_i64(llg_stream_index_at(left, right, offset), 64), value);
        cursor = right_bit;
    }
}

static int llg_queue_source_range(const llg_queue_source_t* source,
                                  size_t* first, size_t* count) {
    const llg_queue_t* queue = source->queue;
    if (!queue || !first || !count)
        llg_container_fatal("malformed queue assignment source");
    if (!queue->size) {
        *first = 0;
        *count = 0;
        return 1;
    }
    size_t left;
    size_t right;
    if (source->left_unbounded) {
        left = queue->size - 1;
    } else if (!llg_index(source->left, queue->size, 0, &left)) {
        *first = 0;
        *count = 0;
        return 1;
    }
    if (source->right_unbounded) {
        right = queue->size - 1;
    } else if (!llg_index(source->right, queue->size, 0, &right)) {
        *first = 0;
        *count = 0;
        return 1;
    }
    if (left > right) {
        *first = 0;
        *count = 0;
        return 1;
    }
    *first = left;
    *count = right - left + 1;
    return 1;
}

void llg_queue_assign_sources(llg_queue_t* dst,
                              const llg_queue_source_t* sources,
                              size_t source_count) {
    if (!dst || (!sources && source_count))
        llg_container_fatal("malformed queue assignment");
    size_t total = 0;
    for (size_t source_index = 0; source_index < source_count; ++source_index) {
        const llg_queue_source_t* source = &sources[source_index];
        size_t first;
        size_t count;
        llg_queue_source_range(source, &first, &count);
        if (count > SIZE_MAX - total)
            llg_container_fatal("queue assignment source size overflow");
        total += count;
    }
    size_t retained = total < dst->limit ? total : dst->limit;
    sv4_t* data = llg_alloc_items(retained, sizeof(*data));
    size_t copied = 0;
    for (size_t source_index = 0; source_index < source_count && copied < retained;
         ++source_index) {
        const llg_queue_source_t* source = &sources[source_index];
        size_t first;
        size_t count;
        llg_queue_source_range(source, &first, &count);
        if (count > retained - copied) count = retained - copied;
        for (size_t source_offset = 0; source_offset < count; ++source_offset)
            data[copied++] = llg_element_assign(
                source->queue->data[first + source_offset], dst->element_width,
                dst->element_signed, dst->element_two_state);
    }
    int shape_changed = dst->size != retained;
    int contents_changed = shape_changed;
    if (!contents_changed) {
        for (size_t i = 0; i < retained; ++i) {
            if (!sv4_same(dst->data[i], data[i])) {
                contents_changed = 1;
                break;
            }
        }
    }
    llg_container_notify_fn notify = dst->notify;
    sv4_t* contents_dependency = dst->contents_dependency;
    sv4_t* shape_dependency = dst->shape_dependency;
    llg_queue_disconnect(dst, 0);
    free(dst->data);
    free(dst->element_ids);
    dst->data = data;
    dst->element_ids = llg_alloc_items(retained, sizeof(*dst->element_ids));
    dst->size = retained;
    dst->capacity = retained;
    llg_queue_reset_element_ids(dst);
    if (retained != total)
        llg_container_warning("bounded queue assignment discarded tail elements");
    llg_notify(notify, contents_dependency, shape_dependency,
               (contents_changed ? LLG_CONTAINER_CHANGED_CONTENTS : 0) |
                   (shape_changed ? LLG_CONTAINER_CHANGED_SHAPE : 0));
}

size_t llg_queue_size(const llg_queue_t* queue) { return queue->size; }

sv4_t llg_queue_get(const llg_queue_t* queue, sv4_t index) {
    size_t native;
    if (!llg_index(index, queue->size, 0, &native))
        return llg_element_default(queue->element_width, queue->element_signed,
                                   queue->element_two_state);
    return queue->data[native];
}

void llg_queue_push_back(llg_queue_t* queue, sv4_t value) {
    if (queue->size == queue->limit) {
        llg_container_warning("bounded queue push_back discarded new element");
        return;
    }
    if (queue->size == SIZE_MAX) llg_container_fatal("queue size overflow");
    llg_queue_reserve(queue, queue->size + 1);
    queue->data[queue->size] = llg_element_assign(
        value, queue->element_width, queue->element_signed,
        queue->element_two_state);
    queue->element_ids[queue->size] = llg_queue_new_element_id(queue);
    ++queue->size;
    llg_notify(queue->notify, queue->contents_dependency,
               queue->shape_dependency,
               LLG_CONTAINER_CHANGED_CONTENTS | LLG_CONTAINER_CHANGED_SHAPE);
}

void llg_queue_push_front(llg_queue_t* queue, sv4_t value) {
    if (queue->limit == 0) {
        llg_container_warning("bounded queue push_front discarded new element");
        return;
    }
    sv4_t assigned = llg_element_assign(value, queue->element_width,
                                        queue->element_signed,
                                        queue->element_two_state);
    int shape_changed = queue->size < queue->limit;
    int contents_changed = shape_changed;
    if (!shape_changed) {
        for (size_t i = 0; i < queue->size; ++i) {
            sv4_t expected = i == 0 ? assigned : queue->data[i - 1];
            if (!sv4_same(queue->data[i], expected)) {
                contents_changed = 1;
                break;
            }
        }
    }
    if (queue->size < queue->limit) {
        if (queue->size == SIZE_MAX) llg_container_fatal("queue size overflow");
        llg_queue_reserve(queue, queue->size + 1);
        ++queue->size;
    } else {
        llg_queue_disconnect(queue, queue->element_ids[queue->size - 1]);
        llg_container_warning("bounded queue push_front discarded tail element");
    }
    if (queue->size > 1) {
        memmove(queue->data + 1, queue->data,
                (queue->size - 1) * sizeof(*queue->data));
        memmove(queue->element_ids + 1, queue->element_ids,
                (queue->size - 1) * sizeof(*queue->element_ids));
    }
    queue->data[0] = assigned;
    queue->element_ids[0] = llg_queue_new_element_id(queue);
    llg_notify(queue->notify, queue->contents_dependency,
               queue->shape_dependency,
               (contents_changed ? LLG_CONTAINER_CHANGED_CONTENTS : 0) |
                   (shape_changed ? LLG_CONTAINER_CHANGED_SHAPE : 0));
}

int llg_queue_set(llg_queue_t* queue, sv4_t index, sv4_t value) {
    size_t native;
    if (!llg_index(index, queue->size, 1, &native)) {
        llg_container_warning("invalid queue write index");
        return 0;
    }
    if (native == queue->size) {
        size_t before = queue->size;
        llg_queue_push_back(queue, value);
        return queue->size != before;
    }
    sv4_t assigned = llg_element_assign(
        value, queue->element_width, queue->element_signed,
        queue->element_two_state);
    if (sv4_same(queue->data[native], assigned)) return 1;
    queue->data[native] = assigned;
    llg_notify(queue->notify, queue->contents_dependency,
               queue->shape_dependency, LLG_CONTAINER_CHANGED_CONTENTS);
    return 1;
}

int llg_queue_insert(llg_queue_t* queue, sv4_t index, sv4_t value) {
    size_t native;
    if (!llg_index(index, queue->size, 1, &native)) {
        llg_container_warning("invalid queue insert index");
        return 0;
    }
    if (queue->limit == 0 ||
        (queue->size == queue->limit && native == queue->size)) {
        llg_container_warning("bounded queue insert discarded new element");
        return 0;
    }
    sv4_t assigned = llg_element_assign(value, queue->element_width,
                                        queue->element_signed,
                                        queue->element_two_state);
    int shape_changed = queue->size < queue->limit;
    int contents_changed = shape_changed;
    if (!shape_changed) {
        for (size_t i = 0; i < queue->size; ++i) {
            sv4_t expected = i == native
                                  ? assigned
                                  : queue->data[i < native ? i : i - 1];
            if (!sv4_same(queue->data[i], expected)) {
                contents_changed = 1;
                break;
            }
        }
    }
    if (queue->size < queue->limit) {
        if (queue->size == SIZE_MAX) llg_container_fatal("queue size overflow");
        llg_queue_reserve(queue, queue->size + 1);
        ++queue->size;
    } else {
        llg_queue_disconnect(queue, queue->element_ids[queue->size - 1]);
        llg_container_warning("bounded queue insert discarded tail element");
    }
    if (native + 1 < queue->size) {
        memmove(queue->data + native + 1, queue->data + native,
                (queue->size - native - 1) * sizeof(*queue->data));
        memmove(queue->element_ids + native + 1,
                queue->element_ids + native,
                (queue->size - native - 1) * sizeof(*queue->element_ids));
    }
    queue->data[native] = assigned;
    queue->element_ids[native] = llg_queue_new_element_id(queue);
    llg_notify(queue->notify, queue->contents_dependency,
               queue->shape_dependency,
               (contents_changed ? LLG_CONTAINER_CHANGED_CONTENTS : 0) |
                   (shape_changed ? LLG_CONTAINER_CHANGED_SHAPE : 0));
    return 1;
}

int llg_queue_delete_index(llg_queue_t* queue, sv4_t index) {
    size_t native;
    if (!llg_index(index, queue->size, 0, &native)) return 0;
    llg_queue_disconnect(queue, queue->element_ids[native]);
    if (native + 1 < queue->size) {
        memmove(queue->data + native, queue->data + native + 1,
                (queue->size - native - 1) * sizeof(*queue->data));
        memmove(queue->element_ids + native,
                queue->element_ids + native + 1,
                (queue->size - native - 1) * sizeof(*queue->element_ids));
    }
    --queue->size;
    llg_notify(queue->notify, queue->contents_dependency,
               queue->shape_dependency,
               LLG_CONTAINER_CHANGED_CONTENTS | LLG_CONTAINER_CHANGED_SHAPE);
    return 1;
}

sv4_t llg_queue_pop_front(llg_queue_t* queue) {
    sv4_t result = llg_queue_front(queue);
    if (queue->size) {
        llg_queue_disconnect(queue, queue->element_ids[0]);
        if (queue->size > 1) {
            memmove(queue->data, queue->data + 1,
                    (queue->size - 1) * sizeof(*queue->data));
            memmove(queue->element_ids, queue->element_ids + 1,
                    (queue->size - 1) * sizeof(*queue->element_ids));
        }
        --queue->size;
        llg_notify(queue->notify, queue->contents_dependency,
                   queue->shape_dependency,
                   LLG_CONTAINER_CHANGED_CONTENTS |
                       LLG_CONTAINER_CHANGED_SHAPE);
    }
    return result;
}

sv4_t llg_queue_pop_back(llg_queue_t* queue) {
    sv4_t result = llg_queue_back(queue);
    if (queue->size) {
        llg_queue_disconnect(queue, queue->element_ids[queue->size - 1]);
        --queue->size;
        llg_notify(queue->notify, queue->contents_dependency,
                   queue->shape_dependency,
                   LLG_CONTAINER_CHANGED_CONTENTS |
                       LLG_CONTAINER_CHANGED_SHAPE);
    }
    return result;
}

sv4_t llg_queue_front(const llg_queue_t* queue) {
    if (!queue->size)
        return llg_element_default(queue->element_width, queue->element_signed,
                                   queue->element_two_state);
    return queue->data[0];
}

sv4_t llg_queue_back(const llg_queue_t* queue) {
    if (!queue->size)
        return llg_element_default(queue->element_width, queue->element_signed,
                                   queue->element_two_state);
    return queue->data[queue->size - 1];
}

sv4_t llg_queue_reduce(const llg_queue_t* queue, int operation) {
    return llg_reduce_values(queue->data, queue->size, queue->element_width,
                             queue->element_signed, operation);
}

sv4_t llg_queue_reduce_with(const llg_queue_t* queue, int operation,
                            uint32_t result_width, int8_t result_signed,
                            int result_two_state, llg_container_eval_fn eval,
                            void* context) {
    llg_check_element_type(result_width);
    return llg_reduce_values_with(
        queue->data, queue->size, operation, result_width, result_signed,
        result_two_state, eval, context);
}
