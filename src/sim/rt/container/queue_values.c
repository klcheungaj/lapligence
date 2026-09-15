
/* Recursive queue storage ------------------------------------------------ */

static void llg_queue_value_invalidate_refs(llg_queue_value_array_t* queue) {
    if (queue->mutation_epoch == UINT64_MAX)
        llg_container_fatal("queue reference epoch overflow");
    ++queue->mutation_epoch;
}

static void llg_queue_value_reserve(llg_queue_value_array_t* queue,
                                    size_t needed) {
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
    queue->capacity = capacity;
}

static void llg_queue_value_commit(llg_queue_value_array_t* queue,
                                   llg_value_t* data, size_t size) {
    int shape_changed = queue->size != size;
    int contents_changed = shape_changed;
    if (!contents_changed) {
        for (size_t i = 0; i < size; ++i) {
            if (!llg_value_equal(&queue->data[i], &data[i])) {
                contents_changed = 1;
                break;
            }
        }
    }
    llg_container_notify_fn notify = queue->notify;
    sv4_t* contents_dependency = queue->contents_dependency;
    sv4_t* shape_dependency = queue->shape_dependency;
    for (size_t i = 0; i < queue->size; ++i)
        llg_value_drop(&queue->data[i]);
    free(queue->data);
    queue->data = data;
    queue->size = size;
    queue->capacity = size;
    llg_queue_value_invalidate_refs(queue);
    llg_notify(notify, contents_dependency, shape_dependency,
               (contents_changed ? LLG_CONTAINER_CHANGED_CONTENTS : 0) |
                   (shape_changed ? LLG_CONTAINER_CHANGED_SHAPE : 0));
}

static llg_value_t* llg_queue_value_at(const llg_queue_value_array_t* queue,
                                       sv4_t index) {
    size_t native;
    if (!queue || !llg_index(index, queue->size, 0, &native)) return NULL;
    return &queue->data[native];
}

static llg_value_t* llg_queue_value_nested_at(
    const llg_queue_value_array_t* queue, const sv4_t* indices, size_t count) {
    if (!indices || count == 0) return NULL;
    llg_value_t* value = llg_queue_value_at(queue, indices[0]);
    for (size_t i = 1; value && i < count; ++i) {
        if (value->desc->kind != LLG_VALUE_CONTAINER ||
            !value->value.container)
            return NULL;
        value = llg_dyn_value_at(value->value.container, indices[i]);
    }
    return value;
}

static llg_value_t llg_value_from_packed(const llg_value_desc_t* desc,
                                         sv4_t value) {
    llg_value_t result = {0};
    llg_value_default(&result, desc);
    if (desc->kind != LLG_VALUE_PACKED)
        llg_container_fatal("packed value used with a non-packed queue element");
    value = sv4_cast(value, desc->packed_width, desc->packed_signed);
    result.value.packed = desc->packed_two_state ? sv4_to_two_state(value) : value;
    return result;
}

static llg_value_t llg_value_from_real(const llg_value_desc_t* desc,
                                       double value) {
    llg_value_t result = {0};
    llg_value_default(&result, desc);
    if (desc->kind != LLG_VALUE_REAL)
        llg_container_fatal("real value used with an incompatible queue element");
    result.value.real = llg_value_real_convert(desc, value);
    return result;
}

static llg_value_t llg_value_from_string(const llg_value_desc_t* desc,
                                         llg_string_t value) {
    llg_value_t result = {0};
    llg_value_default(&result, desc);
    if (desc->kind != LLG_VALUE_STRING) {
        llg_string_destroy(&value);
        llg_container_fatal("string value used with an incompatible queue element");
    }
    result.value.string = value;
    return result;
}

static llg_value_t llg_value_from_chandle(const llg_value_desc_t* desc,
                                          void* value) {
    llg_value_t result = {0};
    llg_value_default(&result, desc);
    if (desc->kind != LLG_VALUE_CHANDLE && desc->kind != LLG_VALUE_EVENT)
        llg_container_fatal("handle value used with an incompatible queue element");
    result.value.handle = value;
    return result;
}

static llg_value_t llg_value_from_container(
    const llg_value_desc_t* desc, const llg_dyn_value_array_t* source) {
    llg_value_t result = {0};
    llg_value_default(&result, desc);
    if (desc->kind != LLG_VALUE_CONTAINER || !source ||
        !llg_value_desc_compatible(desc->element, source->element))
        llg_container_fatal("container value used with an incompatible element");
    result.value.container = llg_alloc_items(1, sizeof(*result.value.container));
    memset(result.value.container, 0, sizeof(*result.value.container));
    llg_dyn_value_init(result.value.container, desc->element);
    llg_dyn_value_copy(result.value.container, source);
    return result;
}

static llg_value_t llg_value_from_packed_container(
    const llg_value_desc_t* desc, const llg_dyn_array_t* source) {
    llg_value_t result = {0};
    llg_value_default(&result, desc);
    if (desc->kind != LLG_VALUE_CONTAINER || !source || !desc->element ||
        (desc->element->kind != LLG_VALUE_PACKED &&
         desc->element->kind != LLG_VALUE_REAL))
        llg_container_fatal("packed container used with an incompatible element");
    result.value.container = llg_alloc_items(1, sizeof(*result.value.container));
    memset(result.value.container, 0, sizeof(*result.value.container));
    llg_dyn_value_init(result.value.container, desc->element);
    llg_dyn_value_replace_from_packed(result.value.container, source);
    return result;
}

static void llg_queue_value_set_value(llg_queue_value_array_t* queue,
                                      size_t index, const llg_value_t* source) {
    llg_value_copy(&queue->data[index], queue->element, source);
    llg_notify(queue->notify, queue->contents_dependency,
               queue->shape_dependency, LLG_CONTAINER_CHANGED_CONTENTS);
}

void llg_queue_value_init(llg_queue_value_array_t* queue,
                          const llg_value_desc_t* element,
                          uint64_t maximum_elements) {
    if (!element) llg_container_fatal("missing recursive queue value descriptor");
    memset(queue, 0, sizeof(*queue));
    queue->element = element;
    queue->limit = maximum_elements == UINT64_MAX
        ? SIZE_MAX
        : llg_checked_count(maximum_elements, 1);
}

void llg_queue_value_destroy(llg_queue_value_array_t* queue) {
    for (size_t i = 0; i < queue->size; ++i)
        llg_value_drop(&queue->data[i]);
    free(queue->data);
    memset(queue, 0, sizeof(*queue));
}

void llg_queue_value_delete(llg_queue_value_array_t* queue) {
    int changed = queue->size != 0;
    for (size_t i = 0; i < queue->size; ++i)
        llg_value_drop(&queue->data[i]);
    free(queue->data);
    queue->data = NULL;
    queue->size = 0;
    queue->capacity = 0;
    if (changed) llg_queue_value_invalidate_refs(queue);
    llg_notify(queue->notify, queue->contents_dependency,
               queue->shape_dependency,
               changed ? LLG_CONTAINER_CHANGED_CONTENTS |
                            LLG_CONTAINER_CHANGED_SHAPE
                       : 0);
}

void llg_queue_value_copy(llg_queue_value_array_t* dst,
                          const llg_queue_value_array_t* src) {
    if (dst == src) return;
    if (!src || !llg_value_desc_compatible(dst->element, src->element))
        llg_container_fatal("incompatible recursive queue element types");
    size_t count = src->size < dst->limit ? src->size : dst->limit;
    llg_value_t* data = llg_alloc_items(count, sizeof(*data));
    if (count) memset(data, 0, count * sizeof(*data));
    for (size_t i = 0; i < count; ++i)
        llg_value_copy(&data[i], dst->element, &src->data[i]);
    llg_queue_value_commit(dst, data, count);
    if (count != src->size)
        llg_container_warning("bounded queue assignment discarded tail elements");
}

void llg_queue_value_assign_reals(llg_queue_value_array_t* dst,
                                  const double* values, size_t count) {
    size_t retained = count < dst->limit ? count : dst->limit;
    llg_value_t* data = llg_alloc_items(retained, sizeof(*data));
    if (retained) memset(data, 0, retained * sizeof(*data));
    for (size_t i = 0; i < retained; ++i)
        data[i] = llg_value_from_real(dst->element, values[i]);
    llg_queue_value_commit(dst, data, retained);
    if (retained != count)
        llg_container_warning("bounded queue assignment discarded tail elements");
}

void llg_queue_value_assign_strings(llg_queue_value_array_t* dst,
                                    llg_string_t* values, size_t count) {
    size_t retained = count < dst->limit ? count : dst->limit;
    llg_value_t* data = llg_alloc_items(retained, sizeof(*data));
    if (retained) memset(data, 0, retained * sizeof(*data));
    for (size_t i = 0; i < retained; ++i) {
        data[i] = llg_value_from_string(dst->element, values[i]);
        values[i] = (llg_string_t){0};
    }
    for (size_t i = retained; i < count; ++i)
        llg_string_destroy(&values[i]);
    llg_queue_value_commit(dst, data, retained);
    if (retained != count)
        llg_container_warning("bounded queue assignment discarded tail elements");
}

void llg_queue_value_assign_chandles(llg_queue_value_array_t* dst,
                                     void* const* values, size_t count) {
    size_t retained = count < dst->limit ? count : dst->limit;
    llg_value_t* data = llg_alloc_items(retained, sizeof(*data));
    if (retained) memset(data, 0, retained * sizeof(*data));
    for (size_t i = 0; i < retained; ++i)
        data[i] = llg_value_from_chandle(dst->element, values[i]);
    llg_queue_value_commit(dst, data, retained);
    if (retained != count)
        llg_container_warning("bounded queue assignment discarded tail elements");
}

static int llg_queue_value_source_range(const llg_queue_source_t* source,
                                        const llg_queue_value_array_t** queue,
                                        size_t* first, size_t* count) {
    *queue = source->value_queue;
    if (!*queue || !first || !count)
        llg_container_fatal("malformed recursive queue assignment source");
    if (!(*queue)->size) {
        *first = 0;
        *count = 0;
        return 1;
    }
    size_t left;
    size_t right;
    if (source->left_unbounded) {
        left = (*queue)->size - 1;
    } else if (!llg_index(source->left, (*queue)->size, 0, &left)) {
        *first = 0;
        *count = 0;
        return 1;
    }
    if (source->right_unbounded) {
        right = (*queue)->size - 1;
    } else if (!llg_index(source->right, (*queue)->size, 0, &right)) {
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

void llg_queue_value_assign_sources(llg_queue_value_array_t* dst,
                                    const llg_queue_source_t* sources,
                                    size_t source_count) {
    if (!dst || (!sources && source_count))
        llg_container_fatal("malformed recursive queue assignment");
    size_t total = 0;
    for (size_t i = 0; i < source_count; ++i) {
        const llg_queue_value_array_t* source;
        size_t first, count;
        if (!sources[i].value_kind) {
            llg_container_fatal("packed source used with recursive queue assignment");
        }
        llg_queue_value_source_range(&sources[i], &source, &first, &count);
        if (!llg_value_desc_compatible(dst->element, source->element))
            llg_container_fatal("incompatible recursive queue assignment source");
        if (count > SIZE_MAX - total)
            llg_container_fatal("queue assignment source size overflow");
        total += count;
    }
    size_t retained = total < dst->limit ? total : dst->limit;
    llg_value_t* data = llg_alloc_items(retained, sizeof(*data));
    if (retained) memset(data, 0, retained * sizeof(*data));
    size_t copied = 0;
    for (size_t i = 0; i < source_count && copied < retained; ++i) {
        const llg_queue_value_array_t* source;
        size_t first, count;
        llg_queue_value_source_range(&sources[i], &source, &first, &count);
        if (count > retained - copied) count = retained - copied;
        for (size_t j = 0; j < count; ++j)
            llg_value_copy(&data[copied++], dst->element,
                           &source->data[first + j]);
    }
    llg_queue_value_commit(dst, data, retained);
    if (retained != total)
        llg_container_warning("bounded queue assignment discarded tail elements");
}

size_t llg_queue_value_size(const llg_queue_value_array_t* queue) {
    return queue->size;
}

static sv4_t llg_queue_value_default_packed(
    const llg_queue_value_array_t* queue) {
    llg_value_t value = {0};
    llg_value_default(&value, queue->element);
    sv4_t result = value.desc->kind == LLG_VALUE_PACKED
        ? value.value.packed
        : sv4_from_u64(0, 1, 0);
    llg_value_drop(&value);
    return result;
}

sv4_t llg_queue_value_get(const llg_queue_value_array_t* queue, sv4_t index) {
    llg_value_t* value = llg_queue_value_at(queue, index);
    return value && value->desc->kind == LLG_VALUE_PACKED
        ? value->value.packed
        : llg_queue_value_default_packed(queue);
}

double llg_queue_value_get_real(const llg_queue_value_array_t* queue,
                                sv4_t index) {
    llg_value_t* value = llg_queue_value_at(queue, index);
    return value && value->desc->kind == LLG_VALUE_REAL ? value->value.real : 0.0;
}

llg_string_t llg_queue_value_get_string(const llg_queue_value_array_t* queue,
                                        sv4_t index) {
    llg_value_t* value = llg_queue_value_at(queue, index);
    return value && value->desc->kind == LLG_VALUE_STRING
        ? llg_string_clone(&value->value.string)
        : (llg_string_t){0};
}

void* llg_queue_value_get_chandle(const llg_queue_value_array_t* queue,
                                  sv4_t index) {
    llg_value_t* value = llg_queue_value_at(queue, index);
    return value && (value->desc->kind == LLG_VALUE_CHANDLE ||
                     value->desc->kind == LLG_VALUE_EVENT)
        ? value->value.handle
        : NULL;
}

static int llg_queue_value_changed(llg_value_t* target,
                                   const llg_value_t* source) {
    if (llg_value_equal(target, source)) return 1;
    return 0;
}

static int llg_queue_value_set_source(llg_queue_value_array_t* queue,
                                      sv4_t index,
                                      const llg_value_t* source) {
    size_t native;
    if (!llg_index(index, queue->size, 1, &native)) return 0;
    if (native == queue->size) {
        if (queue->size == queue->limit) {
            llg_container_warning("bounded queue write discarded new element");
            return 0;
        }
        if (queue->size == SIZE_MAX) llg_container_fatal("queue size overflow");
        llg_queue_value_reserve(queue, queue->size + 1);
        memset(&queue->data[queue->size], 0, sizeof(*queue->data));
        llg_value_copy(&queue->data[queue->size], queue->element, source);
        ++queue->size;
        llg_queue_value_invalidate_refs(queue);
        llg_notify(queue->notify, queue->contents_dependency,
                   queue->shape_dependency,
                   LLG_CONTAINER_CHANGED_CONTENTS |
                       LLG_CONTAINER_CHANGED_SHAPE);
        return 1;
    }
    if (!llg_queue_value_changed(&queue->data[native], source))
        llg_queue_value_set_value(queue, native, source);
    return 1;
}

int llg_queue_value_set(llg_queue_value_array_t* queue, sv4_t index,
                        sv4_t value) {
    if (queue->element->kind != LLG_VALUE_PACKED)
        return 0;
    llg_value_t source = llg_value_from_packed(queue->element, value);
    int result = llg_queue_value_set_source(queue, index, &source);
    llg_value_drop(&source);
    return result;
}

int llg_queue_value_set_real(llg_queue_value_array_t* queue, sv4_t index,
                             double value) {
    llg_value_t source = llg_value_from_real(queue->element, value);
    int result = llg_queue_value_set_source(queue, index, &source);
    llg_value_drop(&source);
    return result;
}

int llg_queue_value_set_string(llg_queue_value_array_t* queue, sv4_t index,
                               llg_string_t value) {
    llg_value_t source = llg_value_from_string(queue->element, value);
    int result = llg_queue_value_set_source(queue, index, &source);
    llg_value_drop(&source);
    return result;
}

int llg_queue_value_set_chandle(llg_queue_value_array_t* queue, sv4_t index,
                                void* value) {
    llg_value_t source = llg_value_from_chandle(queue->element, value);
    int result = llg_queue_value_set_source(queue, index, &source);
    llg_value_drop(&source);
    return result;
}

sv4_t llg_queue_value_get_nested(const llg_queue_value_array_t* queue,
                                 const sv4_t* indices, size_t count) {
    llg_value_t* value = llg_queue_value_nested_at(queue, indices, count);
    return value && value->desc->kind == LLG_VALUE_PACKED
        ? value->value.packed
        : sv4_from_u64(0, 1, 0);
}

double llg_queue_value_get_nested_real(const llg_queue_value_array_t* queue,
                                       const sv4_t* indices, size_t count) {
    llg_value_t* value = llg_queue_value_nested_at(queue, indices, count);
    return value && value->desc->kind == LLG_VALUE_REAL ? value->value.real : 0.0;
}

llg_string_t llg_queue_value_get_nested_string(
    const llg_queue_value_array_t* queue, const sv4_t* indices, size_t count) {
    llg_value_t* value = llg_queue_value_nested_at(queue, indices, count);
    return value && value->desc->kind == LLG_VALUE_STRING
        ? llg_string_clone(&value->value.string)
        : (llg_string_t){0};
}

void* llg_queue_value_get_nested_chandle(
    const llg_queue_value_array_t* queue, const sv4_t* indices, size_t count) {
    llg_value_t* value = llg_queue_value_nested_at(queue, indices, count);
    return value && (value->desc->kind == LLG_VALUE_CHANDLE ||
                     value->desc->kind == LLG_VALUE_EVENT)
        ? value->value.handle
        : NULL;
}

static int llg_queue_value_set_nested_source(
    llg_queue_value_array_t* queue, const sv4_t* indices, size_t count,
    const llg_value_t* source) {
    llg_value_t* target = llg_queue_value_nested_at(queue, indices, count);
    if (!target || !source || !llg_value_desc_compatible(target->desc,
                                                         source->desc))
        return 0;
    if (llg_value_equal(target, source)) return 1;
    llg_value_copy(target, target->desc, source);
    llg_notify(queue->notify, queue->contents_dependency,
               queue->shape_dependency, LLG_CONTAINER_CHANGED_CONTENTS);
    return 1;
}

int llg_queue_value_set_nested(llg_queue_value_array_t* queue,
                               const sv4_t* indices, size_t count, sv4_t value) {
    llg_value_t* target = llg_queue_value_nested_at(queue, indices, count);
    if (!target || target->desc->kind != LLG_VALUE_PACKED) return 0;
    llg_value_t source = llg_value_from_packed(target->desc, value);
    int result = llg_queue_value_set_nested_source(queue, indices, count, &source);
    llg_value_drop(&source);
    return result;
}

int llg_queue_value_set_nested_real(llg_queue_value_array_t* queue,
                                    const sv4_t* indices, size_t count,
                                    double value) {
    llg_value_t* target = llg_queue_value_nested_at(queue, indices, count);
    if (!target || target->desc->kind != LLG_VALUE_REAL) return 0;
    llg_value_t source = llg_value_from_real(target->desc, value);
    int result = llg_queue_value_set_nested_source(queue, indices, count, &source);
    llg_value_drop(&source);
    return result;
}

int llg_queue_value_set_nested_string(
    llg_queue_value_array_t* queue, const sv4_t* indices, size_t count,
    llg_string_t value) {
    llg_value_t* target = llg_queue_value_nested_at(queue, indices, count);
    if (!target || target->desc->kind != LLG_VALUE_STRING) {
        llg_string_destroy(&value);
        return 0;
    }
    llg_value_t source = llg_value_from_string(target->desc, value);
    int result = llg_queue_value_set_nested_source(queue, indices, count, &source);
    llg_value_drop(&source);
    return result;
}

int llg_queue_value_set_nested_chandle(
    llg_queue_value_array_t* queue, const sv4_t* indices, size_t count,
    void* value) {
    llg_value_t* target = llg_queue_value_nested_at(queue, indices, count);
    if (!target || (target->desc->kind != LLG_VALUE_CHANDLE &&
                    target->desc->kind != LLG_VALUE_EVENT))
        return 0;
    llg_value_t source = llg_value_from_chandle(target->desc, value);
    int result = llg_queue_value_set_nested_source(queue, indices, count, &source);
    llg_value_drop(&source);
    return result;
}

int llg_queue_value_set_nested_container(
    llg_queue_value_array_t* queue, const sv4_t* indices, size_t count,
    const llg_dyn_value_array_t* source) {
    llg_value_t* target = llg_queue_value_nested_at(queue, indices, count);
    if (!target || target->desc->kind != LLG_VALUE_CONTAINER || !source ||
        !llg_value_desc_compatible(target->desc->element, source->element))
        return 0;
    llg_value_t source_value = {0};
    source_value.desc = target->desc;
    source_value.value.container = (llg_dyn_value_array_t*)source;
    int result = llg_queue_value_set_nested_source(queue, indices, count,
                                                   &source_value);
    return result;
}

int llg_queue_value_set_nested_container_from_packed(
    llg_queue_value_array_t* queue, const sv4_t* indices, size_t count,
    const llg_dyn_array_t* source) {
    llg_value_t* target = llg_queue_value_nested_at(queue, indices, count);
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
    int result = llg_queue_value_set_nested_source(queue, indices, count,
                                                   &source_value);
    llg_dyn_value_destroy(&converted);
    return result;
}
