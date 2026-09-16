
void llg_dyn_new(llg_dyn_array_t* dst, sv4_t requested_size,
                 const llg_dyn_array_t* initializer) {
    size_t size = llg_checked_count(llg_dynamic_size(requested_size), sizeof(sv4_t));
    sv4_t* data = llg_alloc_items(size, sizeof(*data));
    size_t copied = initializer && initializer->size < size
                        ? initializer->size
                        : size;
    if (!initializer) copied = 0;
    for (size_t i = 0; i < copied; ++i)
        data[i] = llg_element_assign(initializer->data[i], dst->element_width,
                                     dst->element_signed,
                                     dst->element_two_state);
    sv4_t initial = llg_element_default(dst->element_width,
                                        dst->element_signed,
                                        dst->element_two_state);
    for (size_t i = copied; i < size; ++i) data[i] = sv4_clone(&initial);
    sv4_destroy(&initial);
    int shape_changed = dst->size != size;
    int contents_changed = shape_changed;
    if (!contents_changed) {
        for (size_t i = 0; i < size; ++i) {
            if (!sv4_same(dst->data[i], data[i])) {
                contents_changed = 1;
                break;
            }
        }
    }
    llg_container_notify_fn notify = dst->notify;
    sv4_t* contents_dependency = dst->contents_dependency;
    sv4_t* shape_dependency = dst->shape_dependency;
    sv4_destroy_array(dst->data, dst->size);
    free(dst->data);
    dst->data = data;
    dst->size = size;
    llg_notify(notify, contents_dependency, shape_dependency,
               (contents_changed ? LLG_CONTAINER_CHANGED_CONTENTS : 0) |
                   (shape_changed ? LLG_CONTAINER_CHANGED_SHAPE : 0));
}

sv4_t llg_dyn_stream(const llg_dyn_array_t* array, uint32_t slice,
                     int right_to_left, int selector_kind, sv4_t first,
                     sv4_t second) {
    int64_t left;
    int64_t right;
    size_t count;
    llg_stream_bounds(selector_kind, first, second, array->size, &left, &right,
                      &count);
    if (!count) {
        sv4_t empty;
        memset(&empty, 0, sizeof(empty));
        return empty;
    }
    if (count > (size_t)((LLG_SUPPORTED_WIDTH_LIMIT - 1u) / array->element_width))
        llg_container_fatal("streaming value reaches supported width limit");
    sv4_t* values = llg_alloc_items(count, sizeof(*values));
    for (size_t offset = 0; offset < count; ++offset) {
        int64_t index = llg_stream_index_at(left, right, offset);
        sv4_t packed_index = sv4_from_i64(index, 64);
        values[offset] = llg_dyn_get(array, packed_index);
        sv4_destroy(&packed_index);
    }
    sv4_t result = llg_pack_stream_values(values, count, array->element_width,
                                          slice, right_to_left);
    sv4_destroy_array(values, count);
    free(values);
    return result;
}

static size_t llg_stream_unpacked_values(sv4_t source, uint32_t element_width,
                                         int selector_kind, sv4_t first,
                                         sv4_t second, int64_t* left,
                                         int64_t* right) {
    if (element_width == 0)
        llg_container_fatal("streaming destination has an empty element type");
    size_t count;
    llg_stream_bounds(selector_kind, first, second, 0, left, right, &count);
    if (!selector_kind) {
        if (element_width == 0 || source.width % element_width != 0)
            llg_container_fatal(
                "streaming source width is not divisible by destination element width");
        count = source.width / element_width;
        *left = 0;
        *right = count ? (int64_t)(count - 1) : -1;
    } else if (count > 0
               && (count > (size_t)((LLG_SUPPORTED_WIDTH_LIMIT - 1u) / element_width)
                   || (uint64_t)count * element_width != source.width)) {
        llg_container_fatal(
            "streaming selector width does not match source width");
    }
    if (count > (size_t)((LLG_SUPPORTED_WIDTH_LIMIT - 1u) / element_width))
        llg_container_fatal("streaming destination reaches supported width limit");
    return count;
}

void llg_dyn_unstream_assign(llg_dyn_array_t* dst, sv4_t source,
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
        llg_dyn_assign_values(dst, values, count);
        sv4_destroy_array(values, count);
        free(values);
        sv4_destroy(&unpacked);
        return;
    }
    if (left < 0 || right < 0) {
        if (count) llg_container_fatal(
            "dynamic-array streaming target selector must be a nonnegative range");
        sv4_destroy(&unpacked);
        return;
    }
    int64_t high = left > right ? left : right;
    if (high == INT64_MAX)
        llg_container_fatal("dynamic-array streaming target index overflows size");
    if ((uint64_t)high >= (uint64_t)dst->size) {
        sv4_t new_size = sv4_from_u64((uint64_t)high + 1, 64, 0);
        llg_dyn_resize(dst, new_size);
        sv4_destroy(&new_size);
    }
    uint32_t cursor = unpacked.width;
    for (size_t offset = 0; offset < count; ++offset) {
        uint32_t right_bit = cursor - dst->element_width;
        sv4_t value = sv4_part_select(
            unpacked, (int64_t)cursor - 1, (int64_t)right_bit);
        sv4_t index = sv4_from_i64(llg_stream_index_at(left, right, offset), 64);
        llg_dyn_set(dst, index, value);
        sv4_destroy(&index);
        sv4_destroy(&value);
        cursor = right_bit;
    }
    sv4_destroy(&unpacked);
}

void llg_dyn_resize(llg_dyn_array_t* array, sv4_t size) {
    llg_dyn_new(array, size, array);
}

void llg_dyn_copy(llg_dyn_array_t* dst, const llg_dyn_array_t* src) {
    if (dst == src) return;
    sv4_t size = sv4_from_u64((uint64_t)src->size, 64, 0);
    llg_dyn_new(dst, size, src);
    sv4_destroy(&size);
}

void llg_dyn_assign_values(llg_dyn_array_t* dst, const sv4_t* values,
                           size_t count) {
    sv4_t* data = llg_alloc_items(count, sizeof(*data));
    for (size_t i = 0; i < count; ++i)
        data[i] = llg_element_assign(values[i], dst->element_width,
                                     dst->element_signed,
                                     dst->element_two_state);
    int shape_changed = dst->size != count;
    int contents_changed = shape_changed;
    if (!contents_changed) {
        for (size_t i = 0; i < count; ++i) {
            if (!sv4_same(dst->data[i], data[i])) {
                contents_changed = 1;
                break;
            }
        }
    }
    llg_container_notify_fn notify = dst->notify;
    sv4_t* contents_dependency = dst->contents_dependency;
    sv4_t* shape_dependency = dst->shape_dependency;
    sv4_destroy_array(dst->data, dst->size);
    free(dst->data);
    dst->data = data;
    dst->size = count;
    llg_notify(notify, contents_dependency, shape_dependency,
               (contents_changed ? LLG_CONTAINER_CHANGED_CONTENTS : 0) |
                   (shape_changed ? LLG_CONTAINER_CHANGED_SHAPE : 0));
}

size_t llg_dyn_size(const llg_dyn_array_t* array) { return array->size; }

sv4_t llg_dyn_get(const llg_dyn_array_t* array, sv4_t index) {
    size_t native;
    if (!llg_index(index, array->size, 0, &native))
        return llg_element_default(array->element_width, array->element_signed,
                                   array->element_two_state);
    return sv4_clone(&array->data[native]);
}

int llg_dyn_set(llg_dyn_array_t* array, sv4_t index, sv4_t value) {
    size_t native;
    if (!llg_index(index, array->size, 0, &native)) return 0;
    sv4_t assigned = llg_element_assign(
        value, array->element_width, array->element_signed,
        array->element_two_state);
    if (sv4_same(array->data[native], assigned)) {
        sv4_destroy(&assigned);
        return 1;
    }
    sv4_move(&array->data[native], &assigned);
    llg_notify(array->notify, array->contents_dependency,
               array->shape_dependency, LLG_CONTAINER_CHANGED_CONTENTS);
    return 1;
}

sv4_t llg_dyn_reduce(const llg_dyn_array_t* array, int operation) {
    return llg_reduce_values(array->data, array->size, array->element_width,
                             array->element_signed, operation);
}

sv4_t llg_dyn_reduce_with(const llg_dyn_array_t* array, int operation,
                          uint32_t result_width, int8_t result_signed,
                          int result_two_state, llg_container_eval_fn eval,
                          void* context) {
    llg_check_element_type(result_width);
    return llg_reduce_values_with(
        array->data, array->size, operation, result_width, result_signed,
        result_two_state, eval, context);
}
