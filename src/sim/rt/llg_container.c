// llg_container.c -- scheduler-independent dynamic array, queue, and
// associative-array storage for generated C11 models.
#include "llg_container.h"

#include <limits.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static void llg_container_fatal(const char* message) {
    fprintf(stderr, "llg container fatal: %s\n", message);
    abort();
}

static void llg_container_warning(const char* message) {
    fprintf(stderr, "llg container warning: %s\n", message);
}

static void llg_check_element_type(uint32_t width) {
    if (width == 0 || width > LLG_MAX_WIDTH)
        llg_container_fatal("invalid packed element width");
}

static size_t llg_checked_count(uint64_t count, size_t item_size) {
    if (count > (uint64_t)SIZE_MAX || (size_t)count > SIZE_MAX / item_size)
        llg_container_fatal("container allocation size overflow");
    return (size_t)count;
}

static void* llg_alloc_items(size_t count, size_t item_size) {
    if (count == 0) return NULL;
    if (count > SIZE_MAX / item_size)
        llg_container_fatal("container allocation size overflow");
    void* result = malloc(count * item_size);
    if (!result) llg_container_fatal("container allocation failed");
    return result;
}

static void* llg_realloc_items(void* old, size_t count, size_t item_size) {
    if (count == 0) {
        free(old);
        return NULL;
    }
    if (count > SIZE_MAX / item_size)
        llg_container_fatal("container allocation size overflow");
    void* result = realloc(old, count * item_size);
    if (!result) llg_container_fatal("container allocation failed");
    return result;
}

static sv4_t llg_element_default(uint32_t width, int8_t is_signed,
                                 uint8_t two_state) {
    return two_state ? sv4_from_u64(0, width, is_signed)
                     : sv4_x(width, is_signed);
}

static sv4_t llg_element_assign(sv4_t value, uint32_t width, int8_t is_signed,
                                uint8_t two_state) {
    sv4_t result = sv4_cast(value, width, is_signed);
    return two_state ? sv4_to_two_state(result) : result;
}

static sv4_t llg_reduce_identity(uint32_t width, int8_t is_signed,
                                 int operation) {
    switch (operation) {
        case LLG_CONTAINER_REDUCE_PRODUCT:
            return sv4_from_u64(1, width, is_signed);
        case LLG_CONTAINER_REDUCE_AND:
            return sv4_fill(1, width, is_signed);
        case LLG_CONTAINER_REDUCE_SUM:
        case LLG_CONTAINER_REDUCE_OR:
        case LLG_CONTAINER_REDUCE_XOR:
            return sv4_from_u64(0, width, is_signed);
        default:
            llg_container_fatal("invalid container reduction operation");
            return sv4_from_u64(0, width, is_signed);
    }
}

static sv4_t llg_reduce_step(sv4_t accumulated, sv4_t value, int operation) {
    switch (operation) {
        case LLG_CONTAINER_REDUCE_SUM:
            return sv4_add(accumulated, value);
        case LLG_CONTAINER_REDUCE_PRODUCT:
            return sv4_mul(accumulated, value);
        case LLG_CONTAINER_REDUCE_AND:
            return sv4_and(accumulated, value);
        case LLG_CONTAINER_REDUCE_OR:
            return sv4_or(accumulated, value);
        case LLG_CONTAINER_REDUCE_XOR:
            return sv4_xor(accumulated, value);
        default:
            llg_container_fatal("invalid container reduction operation");
            return accumulated;
    }
}

static sv4_t llg_reduce_values(const sv4_t* values, size_t count,
                               uint32_t width, int8_t is_signed,
                               int operation) {
    sv4_t result = llg_reduce_identity(width, is_signed, operation);
    for (size_t i = 0; i < count; ++i)
        result = llg_reduce_step(result, values[i], operation);
    return result;
}

static int llg_index(sv4_t value, size_t upper_exclusive, int allow_end,
                     size_t* result) {
    uint64_t index = sv4_to_index(value);
    if (index == UINT64_MAX || index > (uint64_t)SIZE_MAX) return 0;
    size_t native = (size_t)index;
    if (native > upper_exclusive || (!allow_end && native == upper_exclusive))
        return 0;
    *result = native;
    return 1;
}

static void llg_check_same_element_type(uint32_t dst_width, int8_t dst_signed,
                                        uint8_t dst_two_state,
                                        uint32_t src_width, int8_t src_signed,
                                        uint8_t src_two_state) {
    if (dst_width != src_width || dst_signed != src_signed ||
        dst_two_state != src_two_state)
        llg_container_fatal("incompatible container element types");
}

void llg_dyn_init(llg_dyn_array_t* array, uint32_t element_width,
                  int8_t element_signed, int element_two_state) {
    llg_check_element_type(element_width);
    memset(array, 0, sizeof(*array));
    array->element_width = element_width;
    array->element_signed = !!element_signed;
    array->element_two_state = !!element_two_state;
}

void llg_dyn_destroy(llg_dyn_array_t* array) {
    free(array->data);
    memset(array, 0, sizeof(*array));
}

void llg_dyn_delete(llg_dyn_array_t* array) {
    free(array->data);
    array->data = NULL;
    array->size = 0;
}

static uint64_t llg_dynamic_size(sv4_t value) {
    if (value.width == 0 || value.width > LLG_MAX_WIDTH)
        llg_container_fatal("malformed dynamic-array size value");
    if (sv4_is_unknown(value) || (value.is_signed &&
        ((value.bits[(value.width - 1) / 64] >> ((value.width - 1) % 64)) & 1u)))
        llg_container_fatal("dynamic-array size is unknown or negative");
    uint64_t size = sv4_to_index(value);
    if (size == UINT64_MAX)
        llg_container_fatal("dynamic-array size is not host-representable");
    return size;
}

void llg_dyn_new(llg_dyn_array_t* dst, sv4_t requested_size,
                 const llg_dyn_array_t* initializer) {
    size_t size = llg_checked_count(llg_dynamic_size(requested_size), sizeof(sv4_t));
    if (initializer)
        llg_check_same_element_type(dst->element_width, dst->element_signed,
                                    dst->element_two_state,
                                    initializer->element_width,
                                    initializer->element_signed,
                                    initializer->element_two_state);
    sv4_t* data = llg_alloc_items(size, sizeof(*data));
    size_t copied = initializer && initializer->size < size
                        ? initializer->size
                        : size;
    if (!initializer) copied = 0;
    for (size_t i = 0; i < copied; ++i) data[i] = initializer->data[i];
    sv4_t initial = llg_element_default(dst->element_width,
                                        dst->element_signed,
                                        dst->element_two_state);
    for (size_t i = copied; i < size; ++i) data[i] = initial;
    free(dst->data);
    dst->data = data;
    dst->size = size;
}

void llg_dyn_resize(llg_dyn_array_t* array, sv4_t size) {
    llg_dyn_new(array, size, array);
}

void llg_dyn_copy(llg_dyn_array_t* dst, const llg_dyn_array_t* src) {
    if (dst == src) return;
    llg_dyn_new(dst, sv4_from_u64((uint64_t)src->size, 64, 0), src);
}

void llg_dyn_assign_values(llg_dyn_array_t* dst, const sv4_t* values,
                           size_t count) {
    sv4_t* data = llg_alloc_items(count, sizeof(*data));
    for (size_t i = 0; i < count; ++i)
        data[i] = llg_element_assign(values[i], dst->element_width,
                                     dst->element_signed,
                                     dst->element_two_state);
    free(dst->data);
    dst->data = data;
    dst->size = count;
}

size_t llg_dyn_size(const llg_dyn_array_t* array) { return array->size; }

sv4_t llg_dyn_get(const llg_dyn_array_t* array, sv4_t index) {
    size_t native;
    if (!llg_index(index, array->size, 0, &native))
        return llg_element_default(array->element_width, array->element_signed,
                                   array->element_two_state);
    return array->data[native];
}

int llg_dyn_set(llg_dyn_array_t* array, sv4_t index, sv4_t value) {
    size_t native;
    if (!llg_index(index, array->size, 0, &native)) return 0;
    array->data[native] = llg_element_assign(
        value, array->element_width, array->element_signed,
        array->element_two_state);
    return 1;
}

sv4_t llg_dyn_reduce(const llg_dyn_array_t* array, int operation) {
    return llg_reduce_values(array->data, array->size, array->element_width,
                             array->element_signed, operation);
}

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
    queue->capacity = capacity;
}

void llg_queue_init(llg_queue_t* queue, uint32_t element_width,
                    int8_t element_signed, int element_two_state,
                    uint64_t maximum_elements) {
    llg_check_element_type(element_width);
    memset(queue, 0, sizeof(*queue));
    queue->element_width = element_width;
    queue->element_signed = !!element_signed;
    queue->element_two_state = !!element_two_state;
    queue->limit = maximum_elements == UINT64_MAX
                       ? SIZE_MAX
                       : llg_checked_count(maximum_elements, sizeof(sv4_t));
}

void llg_queue_destroy(llg_queue_t* queue) {
    free(queue->data);
    memset(queue, 0, sizeof(*queue));
}

void llg_queue_delete(llg_queue_t* queue) { queue->size = 0; }

void llg_queue_copy(llg_queue_t* dst, const llg_queue_t* src) {
    if (dst == src) return;
    llg_check_same_element_type(dst->element_width, dst->element_signed,
                                dst->element_two_state, src->element_width,
                                src->element_signed, src->element_two_state);
    size_t count = src->size < dst->limit ? src->size : dst->limit;
    llg_queue_reserve(dst, count);
    if (count) memcpy(dst->data, src->data, count * sizeof(*dst->data));
    dst->size = count;
    if (count != src->size)
        llg_container_warning("bounded queue assignment discarded tail elements");
}

void llg_queue_assign_values(llg_queue_t* dst, const sv4_t* values,
                             size_t count) {
    size_t retained = count < dst->limit ? count : dst->limit;
    sv4_t* data = llg_alloc_items(retained, sizeof(*data));
    for (size_t i = 0; i < retained; ++i)
        data[i] = llg_element_assign(values[i], dst->element_width,
                                     dst->element_signed,
                                     dst->element_two_state);
    free(dst->data);
    dst->data = data;
    dst->size = retained;
    dst->capacity = retained;
    if (retained != count)
        llg_container_warning(
            "bounded queue assignment pattern discarded tail elements");
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
    queue->data[queue->size++] = llg_element_assign(
        value, queue->element_width, queue->element_signed,
        queue->element_two_state);
}

void llg_queue_push_front(llg_queue_t* queue, sv4_t value) {
    if (queue->limit == 0) {
        llg_container_warning("bounded queue push_front discarded new element");
        return;
    }
    if (queue->size < queue->limit) {
        if (queue->size == SIZE_MAX) llg_container_fatal("queue size overflow");
        llg_queue_reserve(queue, queue->size + 1);
        ++queue->size;
    } else {
        llg_container_warning("bounded queue push_front discarded tail element");
    }
    if (queue->size > 1)
        memmove(queue->data + 1, queue->data,
                (queue->size - 1) * sizeof(*queue->data));
    queue->data[0] = llg_element_assign(value, queue->element_width,
                                        queue->element_signed,
                                        queue->element_two_state);
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
    queue->data[native] = llg_element_assign(
        value, queue->element_width, queue->element_signed,
        queue->element_two_state);
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
    if (queue->size < queue->limit) {
        if (queue->size == SIZE_MAX) llg_container_fatal("queue size overflow");
        llg_queue_reserve(queue, queue->size + 1);
        ++queue->size;
    } else {
        llg_container_warning("bounded queue insert discarded tail element");
    }
    if (native + 1 < queue->size)
        memmove(queue->data + native + 1, queue->data + native,
                (queue->size - native - 1) * sizeof(*queue->data));
    queue->data[native] = llg_element_assign(
        value, queue->element_width, queue->element_signed,
        queue->element_two_state);
    return 1;
}

int llg_queue_delete_index(llg_queue_t* queue, sv4_t index) {
    size_t native;
    if (!llg_index(index, queue->size, 0, &native)) return 0;
    if (native + 1 < queue->size)
        memmove(queue->data + native, queue->data + native + 1,
                (queue->size - native - 1) * sizeof(*queue->data));
    --queue->size;
    return 1;
}

sv4_t llg_queue_pop_front(llg_queue_t* queue) {
    sv4_t result = llg_queue_front(queue);
    if (queue->size) {
        if (queue->size > 1)
            memmove(queue->data, queue->data + 1,
                    (queue->size - 1) * sizeof(*queue->data));
        --queue->size;
    }
    return result;
}

sv4_t llg_queue_pop_back(llg_queue_t* queue) {
    sv4_t result = llg_queue_back(queue);
    if (queue->size) --queue->size;
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
}

void llg_assoc_init_string(llg_assoc_t* array, uint32_t element_width,
                           int8_t element_signed, int element_two_state) {
    llg_check_element_type(element_width);
    memset(array, 0, sizeof(*array));
    array->element_width = element_width;
    array->element_signed = !!element_signed;
    array->element_two_state = !!element_two_state;
    array->key_kind = LLG_ASSOC_STRING;
}

void llg_assoc_delete(llg_assoc_t* array) {
    for (size_t i = 0; i < array->size; ++i)
        free(array->entries[i].string_key);
    array->size = 0;
}

void llg_assoc_destroy(llg_assoc_t* array) {
    llg_assoc_delete(array);
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

sv4_t llg_assoc_reduce(const llg_assoc_t* array, int operation) {
    sv4_t result = llg_reduce_identity(array->element_width,
                                       array->element_signed, operation);
    for (size_t i = 0; i < array->size; ++i)
        result = llg_reduce_step(result, array->entries[i].value, operation);
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
        uint32_t width = input.width;
        while (width > 1) {
            uint32_t bit = width - 1;
            if ((input.bits[bit / 64] >> (bit % 64)) & 1u) break;
            --width;
        }
        *output = sv4_resize(input, width ? width : 1, 0);
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
        llg_container_warning("nonexistent associative-array entry read");
    }
    return llg_element_default(array->element_width, array->element_signed,
                               array->element_two_state);
}

int llg_assoc_set_integral(llg_assoc_t* array, sv4_t key, sv4_t value) {
    sv4_t normalized;
    if (!llg_assoc_normalize_key(array, key, &normalized)) {
        llg_container_warning("invalid associative-array integral key write");
        return 0;
    }
    int found;
    size_t position = llg_assoc_integral_position(array, normalized, &found);
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
    array->entries[position].value = llg_element_assign(
        value, array->element_width, array->element_signed,
        array->element_two_state);
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
    return 1;
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

static int llg_string_compare(const void* a, size_t a_len, const void* b,
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
        int cmp = llg_string_compare(array->entries[mid].string_key,
                                     array->entries[mid].string_length,
                                     key, key_length);
        if (cmp < 0)
            low = mid + 1;
        else
            high = mid;
    }
    *found = low < array->size &&
             llg_string_compare(array->entries[low].string_key,
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
    llg_container_warning("nonexistent associative-array entry read");
    return llg_element_default(array->element_width, array->element_signed,
                               array->element_two_state);
}

int llg_assoc_set_string(llg_assoc_t* array, const void* key, size_t key_length,
                         sv4_t value) {
    llg_check_string_key(array, key, key_length);
    int found;
    size_t position = llg_assoc_string_position(array, key, key_length, &found);
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
    array->entries[position].value = llg_element_assign(
        value, array->element_width, array->element_signed,
        array->element_two_state);
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
    llg_check_same_element_type(dst->element_width, dst->element_signed,
                                dst->element_two_state, src->element_width,
                                src->element_signed, src->element_two_state);
    if (dst->key_kind != src->key_kind || dst->key_width != src->key_width ||
        dst->key_signed != src->key_signed ||
        dst->key_two_state != src->key_two_state)
        llg_container_fatal("incompatible associative-array index types");

    llg_assoc_entry_t* entries = llg_alloc_items(src->size, sizeof(*entries));
    if (src->size) memset(entries, 0, src->size * sizeof(*entries));
    for (size_t i = 0; i < src->size; ++i) {
        entries[i].integral_key = src->entries[i].integral_key;
        entries[i].value = src->entries[i].value;
        entries[i].string_length = src->entries[i].string_length;
        if (src->entries[i].string_length) {
            entries[i].string_key = llg_alloc_items(
                src->entries[i].string_length, 1);
            memcpy(entries[i].string_key, src->entries[i].string_key,
                   src->entries[i].string_length);
        }
    }
    llg_assoc_delete(dst);
    free(dst->entries);
    dst->entries = entries;
    dst->size = src->size;
    dst->capacity = src->size;
}
