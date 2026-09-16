// llg_container.c -- scheduler-independent dynamic array, queue, and
// associative-array storage for generated C11 models.
#include "llg_container.h"
#include "llg_rng.h"

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
    if (width == 0 || width > (LLG_SUPPORTED_WIDTH_LIMIT - 1u))
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
    if (two_state) sv4_replace(&result, sv4_to_two_state(result));
    return result;
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
            return sv4_clone(&accumulated);
    }
}

static sv4_t llg_reduce_values(const sv4_t* values, size_t count,
                               uint32_t width, int8_t is_signed,
                               int operation) {
    sv4_t result = llg_reduce_identity(width, is_signed, operation);
    for (size_t i = 0; i < count; ++i)
        sv4_replace(&result, llg_reduce_step(result, values[i], operation));
    return result;
}

static sv4_t llg_container_eval(llg_container_eval_fn eval, sv4_t item,
                                sv4_t index, void* context) {
    if (!eval) return sv4_clone(&item);
    // Evaluators receive an empty output owner and must replace it explicitly.
    // Their item/index arguments remain borrowed for this call only.
    sv4_t result = SV4_EMPTY;
    eval(&result, item, index, context);
    return result;
}

static sv4_t llg_reduce_values_with(const sv4_t* values, size_t count,
                                    int operation, uint32_t result_width,
                                    int8_t result_signed,
                                    int result_two_state,
                                    llg_container_eval_fn eval,
                                    void* context) {
    sv4_t result = llg_reduce_identity(result_width, result_signed, operation);
    for (size_t i = 0; i < count; ++i) {
        sv4_t index = sv4_from_u64((uint64_t)i, 32, 1);
        sv4_t value = llg_container_eval(eval, values[i], index, context);
        sv4_replace(&value, llg_element_assign(value, result_width,
                                               result_signed, result_two_state));
        sv4_replace(&result, llg_reduce_step(result, value, operation));
        sv4_replace(&result, llg_element_assign(result, result_width,
                                                result_signed, result_two_state));
        sv4_destroy(&value);
        sv4_destroy(&index);
    }
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

/* Resolve one streaming `with` selector to its requested logical index range.
 * The range is deliberately independent of the container's current size:
 * streaming an out-of-range source element yields that element's default. */
static void llg_stream_bounds(int selector_kind, sv4_t first, sv4_t second,
                              size_t container_size, int64_t* left,
                              int64_t* right, size_t* count) {
    if (!left || !right || !count)
        llg_container_fatal("malformed streaming selector output");
    if (selector_kind == LLG_STREAM_SELECTOR_NONE) {
        if (container_size > (size_t)INT64_MAX)
            llg_container_fatal("streaming container size exceeds host index");
        *left = 0;
        *right = container_size ? (int64_t)(container_size - 1) : -1;
        *count = container_size;
        return;
    }
    int64_t base;
    if (!sv4_to_index_i64(first, &base)) {
        *left = 0;
        *right = -1;
        *count = 0;
        return;
    }
    if (selector_kind == LLG_STREAM_SELECTOR_INDEX) {
        *left = base;
        *right = base;
        *count = 1;
        return;
    }
    if (selector_kind == LLG_STREAM_SELECTOR_RANGE) {
        if (!sv4_to_index_i64(second, right)) {
            *left = 0;
            *right = -1;
            *count = 0;
            return;
        }
        *left = base;
    } else if (selector_kind == LLG_STREAM_SELECTOR_INDEXED_PLUS
               || selector_kind == LLG_STREAM_SELECTOR_INDEXED_MINUS) {
        int64_t width;
        if (!sv4_to_index_i64(second, &width) || width <= 0) {
            llg_container_fatal("streaming indexed selector width is invalid");
        }
        uint64_t amount = (uint64_t)(width - 1);
        uint64_t plus_available = (uint64_t)INT64_MAX - (uint64_t)base;
        uint64_t minus_available = (uint64_t)base - (uint64_t)INT64_MIN;
        if ((selector_kind == LLG_STREAM_SELECTOR_INDEXED_PLUS
             && amount > plus_available)
            || (selector_kind == LLG_STREAM_SELECTOR_INDEXED_MINUS
                && amount > minus_available)) {
            llg_container_fatal("streaming selector range overflows host index");
        }
        *left = base;
        *right = selector_kind == LLG_STREAM_SELECTOR_INDEXED_PLUS
                     ? base + width - 1
                     : base - width + 1;
    } else {
        llg_container_fatal("invalid streaming selector kind");
    }
    uint64_t distance = *left >= *right
                            ? (uint64_t)*left - (uint64_t)*right
                            : (uint64_t)*right - (uint64_t)*left;
    if (distance == UINT64_MAX)
        llg_container_fatal("streaming selector range is too large");
    *count = llg_checked_count(distance + 1, sizeof(sv4_t));
}

uint32_t llg_stream_selector_width(int selector_kind, sv4_t first,
                                   sv4_t second, uint32_t element_width) {
    if (element_width == 0 || element_width > (LLG_SUPPORTED_WIDTH_LIMIT - 1u))
        llg_container_fatal("invalid streaming selector element width");
    int64_t left;
    int64_t right;
    size_t count;
    llg_stream_bounds(selector_kind, first, second, 0, &left, &right, &count);
    if (selector_kind == LLG_STREAM_SELECTOR_NONE)
        llg_container_fatal("whole streaming target has no selector width");
    if (count > (size_t)((LLG_SUPPORTED_WIDTH_LIMIT - 1u) / element_width))
        llg_container_fatal("streaming selector reaches supported width limit");
    return (uint32_t)(count * element_width);
}

static int64_t llg_stream_index_at(int64_t left, int64_t right,
                                   size_t offset) {
    if (offset > (size_t)INT64_MAX)
        llg_container_fatal("streaming selector offset is too large");
    int64_t delta = (int64_t)offset;
    if (left > right) {
        if ((uint64_t)delta > (uint64_t)left - (uint64_t)INT64_MIN)
            llg_container_fatal("streaming selector index overflows host index");
        return left - delta;
    }
    if (delta > INT64_MAX - left)
        llg_container_fatal("streaming selector index overflows host index");
    return left + delta;
}

static sv4_t llg_pack_stream_values(const sv4_t* values, size_t count,
                                    uint32_t element_width, uint32_t slice,
                                    int right_to_left) {
    llg_check_element_type(element_width);
    if (count > (size_t)((LLG_SUPPORTED_WIDTH_LIMIT - 1u) / element_width))
        llg_container_fatal("streaming value reaches supported width limit");
    uint32_t width = (uint32_t)(count * element_width);
    sv4_t packed = sv4_zero(width, 0);
    uint32_t cursor = width;
    for (size_t i = 0; i < count; ++i) {
        sv4_part_select_set(&packed, (int64_t)cursor - 1,
                            (int64_t)(cursor - element_width), values[i]);
        cursor -= element_width;
    }
    sv4_t result = sv4_stream(packed, slice, right_to_left);
    sv4_destroy(&packed);
    return result;
}

static void llg_notify(llg_container_notify_fn notify, sv4_t* contents,
                       sv4_t* shape, int change) {
    if (notify && change) notify(contents, shape, change);
}

static uint64_t llg_queue_new_element_id(llg_queue_t* queue) {
    if (queue->next_element_id == 0 || queue->next_element_id == UINT64_MAX)
        llg_container_fatal("queue element identity overflow");
    return queue->next_element_id++;
}

static void llg_queue_reset_element_ids(llg_queue_t* queue) {
    for (size_t i = 0; i < queue->size; ++i)
        queue->element_ids[i] = llg_queue_new_element_id(queue);
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
    sv4_destroy_array(array->data, array->size);
    free(array->data);
    memset(array, 0, sizeof(*array));
}

void llg_dyn_delete(llg_dyn_array_t* array) {
    int changed = array->size != 0;
    sv4_destroy_array(array->data, array->size);
    free(array->data);
    array->data = NULL;
    array->size = 0;
    llg_notify(array->notify, array->contents_dependency,
               array->shape_dependency,
               changed ? LLG_CONTAINER_CHANGED_CONTENTS |
                            LLG_CONTAINER_CHANGED_SHAPE
                       : 0);
}

static uint64_t llg_dynamic_size(sv4_t value) {
    if (value.width == 0 || value.width > (LLG_SUPPORTED_WIDTH_LIMIT - 1u))
        llg_container_fatal("malformed dynamic-array size value");
    if (sv4_is_unknown(value) || (value.is_signed &&
        ((value.bits[(value.width - 1) / 64] >> ((value.width - 1) % 64)) & 1u)))
        llg_container_fatal("dynamic-array size is unknown or negative");
    uint64_t size = sv4_to_index(value);
    if (size == UINT64_MAX)
        llg_container_fatal("dynamic-array size is not host-representable");
    return size;
}
