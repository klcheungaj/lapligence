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
                     ? base + (width - 1)
                     : base - (width - 1);
    } else {
        llg_container_fatal("invalid streaming selector kind");
    }
    uint64_t distance = *left >= *right
                            ? (uint64_t)*left - (uint64_t)*right
                            : (uint64_t)*right - (uint64_t)*left;
    if (distance == UINT64_MAX)
        llg_container_fatal("streaming selector range is too large");
    *count = llg_checked_count(distance + 1, sizeof(sv4_t));
    // A `with` range streams the selected elements in storage order, like an
    // array slice (IEEE 1800-2009 11.4.14.4): ascending for queues, dynamic
    // arrays and ascending fixed arrays. Descending fixed arrays re-orient.
    if (*left > *right) {
        int64_t low = *right;
        *right = *left;
        *left = low;
    }
}

/* Walk a selected range from the declaration's left bound toward its right
 * bound, which is the storage order of a fixed unpacked array. */
static void llg_fixed_stream_orient(int64_t declaration_left,
                                    int64_t declaration_right, int64_t* left,
                                    int64_t* right) {
    if (declaration_left > declaration_right && *left < *right) {
        int64_t high = *right;
        *right = *left;
        *left = high;
    }
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

void llg_fixed_stream_bounds(int selector_kind, sv4_t first, sv4_t second,
                             int64_t declaration_left, int64_t declaration_right,
                             int64_t* left, int64_t* right, size_t* count) {
    llg_stream_bounds(selector_kind, first, second, 0, left, right, count);
    llg_fixed_stream_orient(declaration_left, declaration_right, left, right);
}

uint32_t llg_fixed_stream_width(int selector_kind, sv4_t first, sv4_t second,
                                uint32_t element_width) {
    return llg_stream_selector_width(selector_kind, first, second,
                                     element_width);
}

static int64_t llg_stream_index_at(int64_t left, int64_t right,
                                   size_t offset) {
    if (offset > (size_t)INT64_MAX)
        llg_container_fatal("streaming selector offset is too large");
    int64_t delta = (int64_t)offset;
    if (left > right) {
        if (left < INT64_MIN + delta)
            llg_container_fatal("streaming selector index overflows host index");
        return left - delta;
    }
    if (left > INT64_MAX - delta)
        llg_container_fatal("streaming selector index overflows host index");
    return left + delta;
}

/* Reject insufficient source bits before the emitter publishes staged pieces.
 * Fixed-array bounds errors differ: in-range elements must still be written. */
void llg_stream_require_bits(int64_t available, uint32_t required) {
    if (available < 0 || (uint64_t)required > (uint64_t)available)
        llg_container_fatal("streaming unpack source has insufficient bits");
}

int llg_fixed_stream_target_in_bounds(int64_t declaration_left,
                                      int64_t declaration_right,
                                      int64_t left, int64_t right, size_t count) {
    if (!count) return 0;
    int64_t low = declaration_left < declaration_right
                      ? declaration_left : declaration_right;
    int64_t high = declaration_left > declaration_right
                       ? declaration_left : declaration_right;
    return left >= low && left <= high && right >= low && right <= high;
}

int64_t llg_fixed_stream_index_at(int64_t left, int64_t right, size_t offset) {
    return llg_stream_index_at(left, right, offset);
}

sv4_t llg_stream_to_fixed(sv4_t value, uint32_t width, int is_signed) {
    if (llg_sv4_width(value) > width)
        llg_container_fatal("streaming concatenation is larger than its fixed-size target");
    sv4_t result = sv4_zero(width, is_signed);
    if (llg_sv4_width(value))
        sv4_part_select_set(&result, (int64_t)width - 1,
                            (int64_t)(width - llg_sv4_width(value)), value);
    return result;
}

/* SV 11.4.14.3 consumes a wider unpack source from its left end, before the
 * stream operator reorders the consumed bits. */
sv4_t llg_stream_unpack_source(sv4_t value, uint64_t bits, uint32_t slice,
                               int right_to_left) {
    uint32_t width = llg_sv4_width(value);
    if (bits > width)
        llg_container_fatal("streaming unpack source has insufficient bits");
    if (!bits) {
        sv4_t empty = SV4_EMPTY;
        return empty;
    }
    sv4_t consumed = sv4_part_select(value, (int64_t)width - 1,
                                     (int64_t)(width - (uint32_t)bits));
    sv4_t result = sv4_unstream(consumed, slice, right_to_left);
    sv4_destroy(&consumed);
    return result;
}

/* Storage offset of a logical index of a fixed unpacked dimension, or -1
 * when the index lies outside the declared bounds. */
int64_t llg_fixed_stream_storage_offset(int64_t declaration_left,
                                        int64_t declaration_right,
                                        int64_t logical) {
    if (declaration_left >= declaration_right) {
        if (logical < declaration_right || logical > declaration_left) return -1;
        return declaration_left - logical;
    }
    if (logical < declaration_left || logical > declaration_right) return -1;
    return logical - declaration_left;
}

/* Resolve a runtime source selector and return its element count. `*result`
 * receives an empty value for a zero count and a zeroed packed owner of the
 * selected width otherwise. */
static size_t llg_fixed_stream_source_begin(int64_t declaration_left,
                                            int64_t declaration_right,
                                            uint32_t element_width,
                                            int selector_kind, sv4_t first,
                                            sv4_t second, int64_t* left,
                                            int64_t* right, sv4_t* result) {
    llg_check_element_type(element_width);
    if (selector_kind == LLG_STREAM_SELECTOR_NONE)
        llg_container_fatal("whole fixed-array streaming source has no selector");
    size_t count;
    llg_stream_bounds(selector_kind, first, second, 0, left, right, &count);
    llg_fixed_stream_orient(declaration_left, declaration_right, left, right);
    if (!count) {
        sv4_t empty = SV4_EMPTY;
        *result = empty;
        return 0;
    }
    if (count > (size_t)((LLG_SUPPORTED_WIDTH_LIMIT - 1u) / element_width))
        llg_container_fatal("streaming source reaches supported width limit");
    *result = sv4_zero((uint32_t)(count * element_width), 0);
    return count;
}

sv4_t llg_fixed_stream_source(const sv4_t* values, int64_t declaration_left,
                              int64_t declaration_right, uint32_t element_width,
                              sv4_t fallback, int selector_kind,
                              sv4_t first, sv4_t second) {
    int64_t left;
    int64_t right;
    sv4_t packed;
    size_t count = llg_fixed_stream_source_begin(
        declaration_left, declaration_right, element_width, selector_kind,
        first, second, &left, &right, &packed);
    uint32_t cursor = (uint32_t)(count * element_width);
    for (size_t i = 0; i < count; ++i) {
        int64_t offset = llg_fixed_stream_storage_offset(
            declaration_left, declaration_right,
            llg_fixed_stream_index_at(left, right, i));
        sv4_part_select_set(&packed, (int64_t)cursor - 1,
                            (int64_t)(cursor - element_width),
                            offset < 0 ? fallback : values[offset]);
        cursor -= element_width;
    }
    return packed;
}

sv4_t llg_fixed_image_stream_source(sv4_t image, int64_t declaration_left,
                                    int64_t declaration_right,
                                    uint32_t element_width, sv4_t fallback,
                                    int selector_kind, sv4_t first,
                                    sv4_t second) {
    int64_t left;
    int64_t right;
    sv4_t packed;
    size_t count = llg_fixed_stream_source_begin(
        declaration_left, declaration_right, element_width, selector_kind,
        first, second, &left, &right, &packed);
    uint32_t image_width = llg_sv4_width(image);
    uint32_t cursor = (uint32_t)(count * element_width);
    for (size_t i = 0; i < count; ++i) {
        int64_t offset = llg_fixed_stream_storage_offset(
            declaration_left, declaration_right,
            llg_fixed_stream_index_at(left, right, i));
        int64_t high = (int64_t)cursor - 1;
        int64_t low = (int64_t)(cursor - element_width);
        if (offset < 0) {
            sv4_part_select_set(&packed, high, low, fallback);
        } else {
            /* The image holds the left declared element in its MSBs. */
            int64_t image_high = (int64_t)image_width - 1 - offset * (int64_t)element_width;
            sv4_t element = sv4_part_select(image, image_high,
                                            image_high - (int64_t)element_width + 1);
            sv4_part_select_set(&packed, high, low, element);
            sv4_destroy(&element);
        }
        cursor -= element_width;
    }
    return packed;
}

void llg_fixed_image_stream_scatter(sv4_t* image, sv4_t segment,
                                    int64_t declaration_left,
                                    int64_t declaration_right,
                                    uint32_t element_width, int64_t left,
                                    int64_t right, size_t count) {
    llg_check_element_type(element_width);
    if ((uint64_t)count * element_width > llg_sv4_width(segment))
        llg_container_fatal("fixed streaming target segment is narrower than its selection");
    uint32_t image_width = llg_sv4_width(*image);
    uint32_t cursor = llg_sv4_width(segment);
    for (size_t i = 0; i < count; ++i) {
        int64_t offset = llg_fixed_stream_storage_offset(
            declaration_left, declaration_right,
            llg_fixed_stream_index_at(left, right, i));
        if (offset >= 0) {
            int64_t image_high = (int64_t)image_width - 1 - offset * (int64_t)element_width;
            sv4_t element = sv4_part_select(segment, (int64_t)cursor - 1,
                                            (int64_t)(cursor - element_width));
            sv4_part_select_set(image, image_high,
                                image_high - (int64_t)element_width + 1, element);
            sv4_destroy(&element);
        }
        cursor -= element_width;
    }
}

int64_t llg_fixed_image_element_lsb(int64_t declaration_left,
                                    int64_t declaration_right, int64_t logical,
                                    uint32_t element_width) {
    int64_t offset = llg_fixed_stream_storage_offset(declaration_left,
                                                     declaration_right, logical);
    if (offset < 0) llg_container_fatal("fixed streaming element is outside its bounds");
    uint64_t count = (uint64_t)(declaration_left >= declaration_right
                                    ? declaration_left - declaration_right
                                    : declaration_right - declaration_left) + 1u;
    return (int64_t)((count - 1u - (uint64_t)offset) * element_width);
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
    if (llg_sv4_width(value) == 0 || llg_sv4_width(value) > (LLG_SUPPORTED_WIDTH_LIMIT - 1u))
        llg_container_fatal("malformed dynamic-array size value");
    if (sv4_is_unknown(value) || (llg_sv4_signed(value) &&
        ((llg_sv4_word(value, (llg_sv4_width(value) - 1) / 64, LLG_SV4_BITS) >> ((llg_sv4_width(value) - 1) % 64)) & 1u)))
        llg_container_fatal("dynamic-array size is unknown or negative");
    uint64_t size = sv4_to_index(value);
    if (size == UINT64_MAX)
        llg_container_fatal("dynamic-array size is not host-representable");
    return size;
}

/* Streaming is one container assignment. Accumulate dependency flags while
 * building it, then publish only after all temporary packed values are freed.
 * The record is a descriptor used only for its width field, not a value owner. */
static void llg_stream_collect_change(sv4_t* record, sv4_t* unused, int flags) {
    (void)unused;
    record->width |= (uint32_t)flags;
}

static const llg_value_desc_t* llg_value_item_desc(
    const llg_value_desc_t* desc, size_t index) {
    if (desc->kind == LLG_VALUE_AGGREGATE)
        return index < desc->member_count ? desc->members[index].value : NULL;
    return desc->element;
}

static void llg_value_drop(llg_value_t* value) {
    if (!value || !value->desc) return;
    const llg_value_desc_t* desc = value->desc;
    switch (desc->kind) {
        case LLG_VALUE_PACKED:
            sv4_destroy(&value->value.packed);
            break;
        case LLG_VALUE_STRING:
            llg_string_destroy(&value->value.string);
            break;
        case LLG_VALUE_AGGREGATE:
        case LLG_VALUE_FIXED_ARRAY:
            if (value->value.items) {
                for (size_t i = 0; i < desc->item_count; ++i)
                    llg_value_drop(&value->value.items[i]);
                free(value->value.items);
            }
            break;
        case LLG_VALUE_CONTAINER:
            if (value->value.container) {
                llg_dyn_value_destroy(value->value.container);
                free(value->value.container);
            }
            break;
        default:
            break;
    }
    value->desc = NULL;
    memset(&value->value, 0, sizeof(value->value));
}

static void llg_value_default(llg_value_t* value,
                              const llg_value_desc_t* desc) {
    llg_value_drop(value);
    memset(&value->value, 0, sizeof(value->value));
    value->desc = desc;
    switch (desc->kind) {
        case LLG_VALUE_PACKED:
            value->value.packed = desc->packed_two_state
                ? sv4_from_u64(0, desc->packed_width, desc->packed_signed)
                : sv4_x(desc->packed_width, desc->packed_signed);
            break;
        case LLG_VALUE_REAL:
            value->value.real = 0.0;
            break;
        case LLG_VALUE_STRING:
            value->value.string = (llg_string_t){0};
            break;
        case LLG_VALUE_AGGREGATE:
        case LLG_VALUE_FIXED_ARRAY:
            if (desc->item_count) {
                value->value.items = llg_alloc_items(
                    desc->item_count, sizeof(*value->value.items));
                memset(value->value.items, 0,
                       desc->item_count * sizeof(*value->value.items));
                for (size_t i = 0; i < desc->item_count; ++i)
                    llg_value_default(&value->value.items[i],
                                      llg_value_item_desc(desc, i));
            }
            break;
        case LLG_VALUE_CONTAINER:
            /* A nested dynamic array has the standard null-handle default. */
            value->value.container = NULL;
            break;
        case LLG_VALUE_CHANDLE:
        case LLG_VALUE_EVENT:
        case LLG_VALUE_OPAQUE:
            value->value.handle = NULL;
            break;
        default:
            llg_container_fatal("invalid recursive container value kind");
    }
}

static int llg_value_desc_compatible(const llg_value_desc_t* dst,
                                     const llg_value_desc_t* src) {
    if (!dst || !src || dst->kind != src->kind) return 0;
    switch (dst->kind) {
        case LLG_VALUE_PACKED:
        case LLG_VALUE_REAL:
        case LLG_VALUE_STRING:
        case LLG_VALUE_CHANDLE:
        case LLG_VALUE_EVENT:
            return 1;
        case LLG_VALUE_AGGREGATE:
        case LLG_VALUE_CONTAINER:
        case LLG_VALUE_OPAQUE:
            return dst->type_id != 0 && dst->type_id == src->type_id;
        case LLG_VALUE_FIXED_ARRAY:
            return dst->item_count == src->item_count &&
                   llg_value_desc_compatible(dst->element, src->element);
        default:
            return 0;
    }
}

static double llg_value_real_convert(const llg_value_desc_t* target,
                                     double value) {
    return target->real_short ? (double)(float)value : value;
}

static int llg_value_real_same(double left, double right) {
    uint64_t left_bits;
    uint64_t right_bits;
    memcpy(&left_bits, &left, sizeof(left_bits));
    memcpy(&right_bits, &right, sizeof(right_bits));
    return left_bits == right_bits;
}

static void llg_value_copy(llg_value_t*, const llg_value_desc_t*,
                           const llg_value_t*);

static void llg_value_construct_copy(llg_value_t* target,
                           const llg_value_desc_t* target_desc,
                           const llg_value_t* source) {
    memset(&target->value, 0, sizeof(target->value));
    target->desc = target_desc;
    const llg_value_desc_t* source_desc = source ? source->desc : NULL;
    if (!source || !source_desc) {
        llg_value_default(target, target_desc);
        return;
    }
    switch (target_desc->kind) {
        case LLG_VALUE_PACKED: {
            sv4_t value = source_desc->kind == LLG_VALUE_PACKED
                ? sv4_cast(source->value.packed, target_desc->packed_width,
                           target_desc->packed_signed)
                : sv4_zero(target_desc->packed_width, target_desc->packed_signed);
            if (target_desc->packed_two_state)
                sv4_replace(&value, sv4_to_two_state(value));
            sv4_move(&target->value.packed, &value);
            break;
        }
        case LLG_VALUE_REAL:
            target->value.real = llg_value_real_convert(
                target_desc, source_desc->kind == LLG_VALUE_REAL
                ? source->value.real
                : sv4_to_real(source->value.packed));
            break;
        case LLG_VALUE_STRING:
            target->value.string = source_desc->kind == LLG_VALUE_STRING
                ? llg_string_clone(&source->value.string)
                : (llg_string_t){0};
            break;
        case LLG_VALUE_CHANDLE:
        case LLG_VALUE_EVENT:
        case LLG_VALUE_OPAQUE:
            target->value.handle = source->value.handle;
            break;
        case LLG_VALUE_AGGREGATE:
        case LLG_VALUE_FIXED_ARRAY:
            if (target_desc->item_count) {
                target->value.items = llg_alloc_items(
                    target_desc->item_count, sizeof(*target->value.items));
                memset(target->value.items, 0,
                       target_desc->item_count * sizeof(*target->value.items));
                for (size_t i = 0; i < target_desc->item_count; ++i) {
                    const llg_value_desc_t* item =
                        llg_value_item_desc(target_desc, i);
                    const llg_value_t* source_item =
                        source->value.items && i < source_desc->item_count
                            ? &source->value.items[i]
                            : NULL;
                    llg_value_copy(&target->value.items[i], item, source_item);
                }
            }
            break;
        case LLG_VALUE_CONTAINER:
            if (source_desc->kind == LLG_VALUE_CONTAINER &&
                source->value.container) {
                target->value.container = llg_alloc_items(1, sizeof(*target->value.container));
                memset(target->value.container, 0,
                       sizeof(*target->value.container));
                llg_dyn_value_init(target->value.container,
                                   target_desc->element);
                llg_dyn_value_copy(target->value.container,
                                   source->value.container);
            }
            break;
        default:
            llg_container_fatal("invalid recursive container value kind");
    }
}

static void llg_value_copy(llg_value_t* target,
                           const llg_value_desc_t* target_desc,
                           const llg_value_t* source) {
    // Construct before dropping target: source can be target or its descendant.
    llg_value_t replacement = {0};
    llg_value_construct_copy(&replacement, target_desc, source);
    llg_value_drop(target);
    *target = replacement; // exclusive ownership transfer, not a copy
}

static int llg_value_equal(const llg_value_t* a, const llg_value_t* b) {
    if (!a || !b || !a->desc || !b->desc ||
        !llg_value_desc_compatible(a->desc, b->desc))
        return 0;
    switch (a->desc->kind) {
        case LLG_VALUE_PACKED:
            return sv4_same(a->value.packed, b->value.packed);
        case LLG_VALUE_REAL:
            return llg_value_real_same(a->value.real, b->value.real);
        case LLG_VALUE_STRING:
            return a->value.string.len == b->value.string.len &&
                   (!a->value.string.len ||
                    memcmp(a->value.string.data, b->value.string.data,
                           a->value.string.len) == 0);
        case LLG_VALUE_CHANDLE:
        case LLG_VALUE_EVENT:
        case LLG_VALUE_OPAQUE:
            return a->value.handle == b->value.handle;
        case LLG_VALUE_AGGREGATE:
        case LLG_VALUE_FIXED_ARRAY:
            for (size_t i = 0; i < a->desc->item_count; ++i) {
                if (!a->value.items || !b->value.items ||
                    !llg_value_equal(&a->value.items[i],
                                     &b->value.items[i]))
                    return 0;
            }
            return 1;
        case LLG_VALUE_CONTAINER:
            if (a->value.container == b->value.container) return 1;
            if (!a->value.container || !b->value.container ||
                a->value.container->size != b->value.container->size)
                return 0;
            for (size_t i = 0; i < a->value.container->size; ++i) {
                if (!llg_value_equal(&a->value.container->data[i],
                                     &b->value.container->data[i]))
                    return 0;
            }
            return 1;
        default:
            return 0;
    }
}

static int llg_value_equal_after_conversion(
    const llg_value_t* target, const llg_value_desc_t* target_desc,
    const llg_value_t* source) {
    llg_value_t converted = {0};
    llg_value_copy(&converted, target_desc, source);
    int equal = llg_value_equal(target, &converted);
    llg_value_drop(&converted);
    return equal;
}

static llg_value_t* llg_dyn_value_at(const llg_dyn_value_array_t* array,
                                     sv4_t index) {
    size_t native;
    if (!llg_index(index, array->size, 0, &native)) return NULL;
    return &array->data[native];
}

static llg_value_t* llg_dyn_value_nested_at(
    const llg_dyn_value_array_t* array, const sv4_t* indices, size_t count) {
    if (!indices || count == 0) return NULL;
    llg_value_t* value = llg_dyn_value_at(array, indices[0]);
    for (size_t i = 1; value && i < count; ++i) {
        if (value->desc->kind != LLG_VALUE_CONTAINER ||
            !value->value.container)
            return NULL;
        value = llg_dyn_value_at(value->value.container, indices[i]);
    }
    return value;
}

static const llg_value_desc_t* llg_dyn_value_nested_desc(
    const llg_value_desc_t* desc, size_t count) {
    if (!desc || count == 0) return NULL;
    for (size_t i = 1; i < count; ++i) {
        if (desc->kind != LLG_VALUE_CONTAINER) return NULL;
        desc = desc->element;
    }
    return desc;
}

static void llg_dyn_value_commit(llg_dyn_value_array_t* array,
                                 llg_value_t* data, size_t size) {
    int shape_changed = array->size != size;
    int contents_changed = shape_changed;
    if (!contents_changed) {
        for (size_t i = 0; i < size; ++i) {
            if (!llg_value_equal(&array->data[i], &data[i])) {
                contents_changed = 1;
                break;
            }
        }
    }
    llg_container_notify_fn notify = array->notify;
    sv4_t* contents_dependency = array->contents_dependency;
    sv4_t* shape_dependency = array->shape_dependency;
    for (size_t i = 0; i < array->size; ++i)
        llg_value_drop(&array->data[i]);
    free(array->data);
    array->data = data;
    array->size = size;
    llg_notify(notify, contents_dependency, shape_dependency,
               (contents_changed ? LLG_CONTAINER_CHANGED_CONTENTS : 0) |
                   (shape_changed ? LLG_CONTAINER_CHANGED_SHAPE : 0));
}

void llg_dyn_value_init(llg_dyn_value_array_t* array,
                        const llg_value_desc_t* element) {
    if (!element) llg_container_fatal("missing recursive container value descriptor");
    memset(array, 0, sizeof(*array));
    array->element = element;
}

void llg_dyn_value_destroy(llg_dyn_value_array_t* array) {
    for (size_t i = 0; i < array->size; ++i)
        llg_value_drop(&array->data[i]);
    free(array->data);
    memset(array, 0, sizeof(*array));
}

void llg_dyn_value_delete(llg_dyn_value_array_t* array) {
    int changed = array->size != 0;
    for (size_t i = 0; i < array->size; ++i)
        llg_value_drop(&array->data[i]);
    free(array->data);
    array->data = NULL;
    array->size = 0;
    llg_notify(array->notify, array->contents_dependency,
               array->shape_dependency,
               changed ? LLG_CONTAINER_CHANGED_CONTENTS |
                            LLG_CONTAINER_CHANGED_SHAPE
                       : 0);
}

static void llg_dyn_value_new_count(llg_dyn_value_array_t* dst, size_t size,
                              const llg_dyn_value_array_t* initializer) {
    if (initializer &&
        !llg_value_desc_compatible(dst->element, initializer->element))
        llg_container_fatal("incompatible recursive container element types");
    llg_value_t* data = llg_alloc_items(size, sizeof(*data));
    if (size) memset(data, 0, size * sizeof(*data));
    size_t copied = initializer && initializer->size < size
        ? initializer->size
        : size;
    if (!initializer) copied = 0;
    for (size_t i = 0; i < size; ++i) {
        const llg_value_t* source =
            initializer && i < copied ? &initializer->data[i] : NULL;
        llg_value_copy(&data[i], dst->element, source);
    }
    llg_dyn_value_commit(dst, data, size);
}

void llg_dyn_value_new(llg_dyn_value_array_t* dst, sv4_t requested_size,
                 const llg_dyn_value_array_t* initializer) {
    size_t size = llg_checked_count(llg_dynamic_size(requested_size), sizeof(llg_value_t));
    llg_dyn_value_new_count(dst, size, initializer);
}

void llg_dyn_value_copy(llg_dyn_value_array_t* dst,
                        const llg_dyn_value_array_t* src) {
    if (dst == src) return;
    size_t size = llg_checked_count((uint64_t)src->size, sizeof(llg_value_t));
    llg_dyn_value_new_count(dst, size, src);
}

void llg_dyn_value_assign_reals(llg_dyn_value_array_t* dst,
                                const double* values, size_t count) {
    if (!dst->element || dst->element->kind != LLG_VALUE_REAL)
        llg_container_fatal("real assignment used with a non-real container");
    llg_value_t* data = llg_alloc_items(count, sizeof(*data));
    if (count) memset(data, 0, count * sizeof(*data));
    for (size_t i = 0; i < count; ++i) {
        llg_value_default(&data[i], dst->element);
        data[i].value.real = llg_value_real_convert(dst->element, values[i]);
    }
    llg_dyn_value_commit(dst, data, count);
}

void llg_dyn_value_assign_strings(llg_dyn_value_array_t* dst,
                                  llg_string_t* values, size_t count) {
    if (!dst->element || dst->element->kind != LLG_VALUE_STRING)
        llg_container_fatal("string assignment used with a non-string container");
    llg_value_t* data = llg_alloc_items(count, sizeof(*data));
    if (count) memset(data, 0, count * sizeof(*data));
    for (size_t i = 0; i < count; ++i) {
        llg_value_default(&data[i], dst->element);
        data[i].value.string = values[i];
        values[i] = (llg_string_t){0};
    }
    llg_dyn_value_commit(dst, data, count);
}

void llg_dyn_value_assign_chandles(llg_dyn_value_array_t* dst,
                                   void* const* values, size_t count) {
    if (!dst->element ||
        (dst->element->kind != LLG_VALUE_CHANDLE &&
         dst->element->kind != LLG_VALUE_EVENT))
        llg_container_fatal("handle assignment used with an incompatible container");
    llg_value_t* data = llg_alloc_items(count, sizeof(*data));
    if (count) memset(data, 0, count * sizeof(*data));
    for (size_t i = 0; i < count; ++i) {
        llg_value_default(&data[i], dst->element);
        data[i].value.handle = values[i];
    }
    llg_dyn_value_commit(dst, data, count);
}

size_t llg_dyn_value_size(const llg_dyn_value_array_t* array) {
    return array->size;
}

double llg_dyn_value_get_real(const llg_dyn_value_array_t* array, sv4_t index) {
    llg_value_t* value = llg_dyn_value_at(array, index);
    return value && value->desc->kind == LLG_VALUE_REAL
        ? value->value.real
        : 0.0;
}

llg_string_t llg_dyn_value_get_string(const llg_dyn_value_array_t* array,
                                      sv4_t index) {
    llg_value_t* value = llg_dyn_value_at(array, index);
    return value && value->desc->kind == LLG_VALUE_STRING
        ? llg_string_clone(&value->value.string)
        : (llg_string_t){0};
}

void* llg_dyn_value_get_chandle(const llg_dyn_value_array_t* array,
                                sv4_t index) {
    llg_value_t* value = llg_dyn_value_at(array, index);
    return value && (value->desc->kind == LLG_VALUE_CHANDLE ||
                     value->desc->kind == LLG_VALUE_EVENT)
        ? value->value.handle
        : NULL;
}

sv4_t llg_dyn_value_get_nested(const llg_dyn_value_array_t* array,
                               const sv4_t* indices, size_t count) {
    llg_value_t* value = llg_dyn_value_nested_at(array, indices, count);
    if (value && value->desc->kind == LLG_VALUE_PACKED)
        return sv4_clone(&value->value.packed);
    const llg_value_desc_t* desc =
        llg_dyn_value_nested_desc(array->element, count);
    return desc && desc->kind == LLG_VALUE_PACKED
        ? (desc->packed_two_state
            ? sv4_from_u64(0, desc->packed_width, desc->packed_signed)
            : sv4_x(desc->packed_width, desc->packed_signed))
        : sv4_from_u64(0, 1, 0);
}

double llg_dyn_value_get_nested_real(const llg_dyn_value_array_t* array,
                                     const sv4_t* indices, size_t count) {
    llg_value_t* value = llg_dyn_value_nested_at(array, indices, count);
    return value && value->desc->kind == LLG_VALUE_REAL
        ? value->value.real
        : 0.0;
}

llg_string_t llg_dyn_value_get_nested_string(
    const llg_dyn_value_array_t* array, const sv4_t* indices, size_t count) {
    llg_value_t* value = llg_dyn_value_nested_at(array, indices, count);
    return value && value->desc->kind == LLG_VALUE_STRING
        ? llg_string_clone(&value->value.string)
        : (llg_string_t){0};
}

void* llg_dyn_value_get_nested_chandle(
    const llg_dyn_value_array_t* array, const sv4_t* indices, size_t count) {
    llg_value_t* value = llg_dyn_value_nested_at(array, indices, count);
    return value && (value->desc->kind == LLG_VALUE_CHANDLE ||
                     value->desc->kind == LLG_VALUE_EVENT)
        ? value->value.handle
        : NULL;
}

int llg_dyn_value_set_real(llg_dyn_value_array_t* array, sv4_t index,
                           double value) {
    llg_value_t* target = llg_dyn_value_at(array, index);
    if (!target || target->desc->kind != LLG_VALUE_REAL) return 0;
    value = llg_value_real_convert(target->desc, value);
    if (llg_value_real_same(target->value.real, value)) return 1;
    target->value.real = value;
    llg_notify(array->notify, array->contents_dependency,
               array->shape_dependency, LLG_CONTAINER_CHANGED_CONTENTS);
    return 1;
}

int llg_dyn_value_set_string(llg_dyn_value_array_t* array, sv4_t index,
                             llg_string_t value) {
    llg_value_t* target = llg_dyn_value_at(array, index);
    if (!target || target->desc->kind != LLG_VALUE_STRING) {
        llg_string_destroy(&value);
        return 0;
    }
    int changed = target->value.string.len != value.len ||
                  (value.len &&
                   memcmp(target->value.string.data, value.data, value.len) != 0);
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

int llg_dyn_value_set_chandle(llg_dyn_value_array_t* array, sv4_t index,
                              void* value) {
    llg_value_t* target = llg_dyn_value_at(array, index);
    if (!target || (target->desc->kind != LLG_VALUE_CHANDLE &&
                    target->desc->kind != LLG_VALUE_EVENT))
        return 0;
    if (target->value.handle == value) return 1;
    target->value.handle = value;
    llg_notify(array->notify, array->contents_dependency,
               array->shape_dependency, LLG_CONTAINER_CHANGED_CONTENTS);
    return 1;
}

int llg_dyn_value_set_nested(llg_dyn_value_array_t* array,
                             const sv4_t* indices, size_t count, sv4_t value) {
    llg_value_t* target = llg_dyn_value_nested_at(array, indices, count);
    if (!target || target->desc->kind != LLG_VALUE_PACKED) return 0;
    sv4_t assigned = sv4_cast(value, target->desc->packed_width,
                              target->desc->packed_signed);
    if (target->desc->packed_two_state)
        sv4_replace(&assigned, sv4_to_two_state(assigned));
    if (sv4_same(target->value.packed, assigned)) {
        sv4_destroy(&assigned);
        return 1;
    }
    sv4_move(&target->value.packed, &assigned);
    llg_notify(array->notify, array->contents_dependency,
               array->shape_dependency, LLG_CONTAINER_CHANGED_CONTENTS);
    return 1;
}

int llg_dyn_value_set_nested_real(llg_dyn_value_array_t* array,
                                  const sv4_t* indices, size_t count,
                                  double value) {
    llg_value_t* target = llg_dyn_value_nested_at(array, indices, count);
    if (!target || target->desc->kind != LLG_VALUE_REAL) return 0;
    value = llg_value_real_convert(target->desc, value);
    if (llg_value_real_same(target->value.real, value)) return 1;
    target->value.real = value;
    llg_notify(array->notify, array->contents_dependency,
               array->shape_dependency, LLG_CONTAINER_CHANGED_CONTENTS);
    return 1;
}

int llg_dyn_value_set_nested_string(llg_dyn_value_array_t* array,
                                    const sv4_t* indices, size_t count,
                                    llg_string_t value) {
    llg_value_t* target = llg_dyn_value_nested_at(array, indices, count);
    if (!target || target->desc->kind != LLG_VALUE_STRING) {
        llg_string_destroy(&value);
        return 0;
    }
    int changed = target->value.string.len != value.len ||
                  (value.len &&
                   memcmp(target->value.string.data, value.data, value.len) != 0);
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

int llg_dyn_value_set_nested_chandle(llg_dyn_value_array_t* array,
                                     const sv4_t* indices, size_t count,
                                     void* value) {
    llg_value_t* target = llg_dyn_value_nested_at(array, indices, count);
    if (!target || (target->desc->kind != LLG_VALUE_CHANDLE &&
                    target->desc->kind != LLG_VALUE_EVENT))
        return 0;
    if (target->value.handle == value) return 1;
    target->value.handle = value;
    llg_notify(array->notify, array->contents_dependency,
               array->shape_dependency, LLG_CONTAINER_CHANGED_CONTENTS);
    return 1;
}

static void llg_dyn_value_replace_from_packed(
    llg_dyn_value_array_t* dst, const llg_dyn_array_t* source) {
    if (!dst->element || !source ||
        (dst->element->kind != LLG_VALUE_PACKED &&
         dst->element->kind != LLG_VALUE_REAL))
        llg_container_fatal("packed source is incompatible with recursive container element");
    llg_value_t* data = llg_alloc_items(source->size, sizeof(*data));
    if (source->size) memset(data, 0, source->size * sizeof(*data));
    for (size_t i = 0; i < source->size; ++i) {
        llg_value_default(&data[i], dst->element);
        if (dst->element->kind == LLG_VALUE_PACKED) {
            sv4_t value = sv4_cast(source->data[i], dst->element->packed_width,
                                   dst->element->packed_signed);
            if (dst->element->packed_two_state)
                sv4_replace(&value, sv4_to_two_state(value));
            sv4_move(&data[i].value.packed, &value);
        } else {
            data[i].value.real = llg_value_real_convert(
                dst->element, sv4_to_real(source->data[i]));
        }
    }
    llg_dyn_value_commit(dst, data, source->size);
}

int llg_dyn_value_set_nested_container(
    llg_dyn_value_array_t* array, const sv4_t* indices, size_t count,
    const llg_dyn_value_array_t* source) {
    llg_value_t* target = llg_dyn_value_nested_at(array, indices, count);
    if (!target || target->desc->kind != LLG_VALUE_CONTAINER || !source ||
        !llg_value_desc_compatible(target->desc->element, source->element))
        return 0;
    llg_value_t source_value = {0};
    source_value.desc = target->desc;
    source_value.value.container = (llg_dyn_value_array_t*)source;
    llg_value_copy(target, target->desc, &source_value);
    llg_notify(array->notify, array->contents_dependency,
               array->shape_dependency, LLG_CONTAINER_CHANGED_CONTENTS);
    return 1;
}

int llg_dyn_value_set_nested_container_from_packed(
    llg_dyn_value_array_t* array, const sv4_t* indices, size_t count,
    const llg_dyn_array_t* source) {
    llg_value_t* target = llg_dyn_value_nested_at(array, indices, count);
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
    llg_value_copy(target, target->desc, &source_value);
    llg_dyn_value_destroy(&converted);
    llg_notify(array->notify, array->contents_dependency,
               array->shape_dependency, LLG_CONTAINER_CHANGED_CONTENTS);
    return 1;
}

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
    if (desc->kind != LLG_VALUE_PACKED)
        llg_container_fatal("packed value used with a non-packed queue element");
    llg_value_t result = {0};
    result.desc = desc;
    result.value.packed = llg_element_assign(value, desc->packed_width,
                                            desc->packed_signed,
                                            desc->packed_two_state);
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
        ? sv4_clone(&value.value.packed)
        : sv4_from_u64(0, 1, 0);
    llg_value_drop(&value);
    return result;
}

sv4_t llg_queue_value_get(const llg_queue_value_array_t* queue, sv4_t index) {
    llg_value_t* value = llg_queue_value_at(queue, index);
    return value && value->desc->kind == LLG_VALUE_PACKED
        ? sv4_clone(&value->value.packed)
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
                                      const llg_value_t* source, int* change) {
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
        *change |= LLG_CONTAINER_CHANGED_CONTENTS |
                       LLG_CONTAINER_CHANGED_SHAPE;
        return 1;
    }
    if (!llg_queue_value_changed(&queue->data[native], source)) {
        llg_value_copy(&queue->data[native], queue->element, source);
        *change |= LLG_CONTAINER_CHANGED_CONTENTS;
    }
    return 1;
}

int llg_queue_value_set(llg_queue_value_array_t* queue, sv4_t index,
                        sv4_t value) {
    int change = 0;
    if (queue->element->kind != LLG_VALUE_PACKED)
        return 0;
    llg_value_t source = llg_value_from_packed(queue->element, value);
    int result = llg_queue_value_set_source(queue, index, &source, &change);
    llg_value_drop(&source);
    /* No temporary owner may remain live across the callback. */
    llg_notify(queue->notify, queue->contents_dependency,
               queue->shape_dependency, change);
    return result;
}

int llg_queue_value_set_real(llg_queue_value_array_t* queue, sv4_t index,
                             double value) {
    int change = 0;
    llg_value_t source = llg_value_from_real(queue->element, value);
    int result = llg_queue_value_set_source(queue, index, &source, &change);
    llg_value_drop(&source);
    /* No temporary owner may remain live across the callback. */
    llg_notify(queue->notify, queue->contents_dependency,
               queue->shape_dependency, change);
    return result;
}

int llg_queue_value_set_string(llg_queue_value_array_t* queue, sv4_t index,
                               llg_string_t value) {
    int change = 0;
    llg_value_t source = llg_value_from_string(queue->element, value);
    int result = llg_queue_value_set_source(queue, index, &source, &change);
    llg_value_drop(&source);
    /* No temporary owner may remain live across the callback. */
    llg_notify(queue->notify, queue->contents_dependency,
               queue->shape_dependency, change);
    return result;
}

int llg_queue_value_set_chandle(llg_queue_value_array_t* queue, sv4_t index,
                                void* value) {
    int change = 0;
    llg_value_t source = llg_value_from_chandle(queue->element, value);
    int result = llg_queue_value_set_source(queue, index, &source, &change);
    llg_value_drop(&source);
    /* No temporary owner may remain live across the callback. */
    llg_notify(queue->notify, queue->contents_dependency,
               queue->shape_dependency, change);
    return result;
}

sv4_t llg_queue_value_get_nested(const llg_queue_value_array_t* queue,
                                 const sv4_t* indices, size_t count) {
    llg_value_t* value = llg_queue_value_nested_at(queue, indices, count);
    return value && value->desc->kind == LLG_VALUE_PACKED
        ? sv4_clone(&value->value.packed)
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
    const llg_value_t* source, int* change) {
    llg_value_t* target = llg_queue_value_nested_at(queue, indices, count);
    if (!target || !source || !llg_value_desc_compatible(target->desc,
                                                         source->desc))
        return 0;
    if (llg_value_equal(target, source)) return 1;
    llg_value_copy(target, target->desc, source);
    *change |= LLG_CONTAINER_CHANGED_CONTENTS;
    return 1;
}

int llg_queue_value_set_nested(llg_queue_value_array_t* queue,
                               const sv4_t* indices, size_t count, sv4_t value) {
    int change = 0;
    llg_value_t* target = llg_queue_value_nested_at(queue, indices, count);
    if (!target || target->desc->kind != LLG_VALUE_PACKED) return 0;
    llg_value_t source = llg_value_from_packed(target->desc, value);
    int result = llg_queue_value_set_nested_source(queue, indices, count, &source, &change);
    llg_value_drop(&source);
    /* No temporary owner may remain live across the callback. */
    llg_notify(queue->notify, queue->contents_dependency,
               queue->shape_dependency, change);
    return result;
}

int llg_queue_value_set_nested_real(llg_queue_value_array_t* queue,
                                    const sv4_t* indices, size_t count,
                                    double value) {
    int change = 0;
    llg_value_t* target = llg_queue_value_nested_at(queue, indices, count);
    if (!target || target->desc->kind != LLG_VALUE_REAL) return 0;
    llg_value_t source = llg_value_from_real(target->desc, value);
    int result = llg_queue_value_set_nested_source(queue, indices, count, &source, &change);
    llg_value_drop(&source);
    /* No temporary owner may remain live across the callback. */
    llg_notify(queue->notify, queue->contents_dependency,
               queue->shape_dependency, change);
    return result;
}

int llg_queue_value_set_nested_string(
    llg_queue_value_array_t* queue, const sv4_t* indices, size_t count,
    llg_string_t value) {
    int change = 0;
    llg_value_t* target = llg_queue_value_nested_at(queue, indices, count);
    if (!target || target->desc->kind != LLG_VALUE_STRING) {
        llg_string_destroy(&value);
        return 0;
    }
    llg_value_t source = llg_value_from_string(target->desc, value);
    int result = llg_queue_value_set_nested_source(queue, indices, count, &source, &change);
    llg_value_drop(&source);
    /* No temporary owner may remain live across the callback. */
    llg_notify(queue->notify, queue->contents_dependency,
               queue->shape_dependency, change);
    return result;
}

int llg_queue_value_set_nested_chandle(
    llg_queue_value_array_t* queue, const sv4_t* indices, size_t count,
    void* value) {
    int change = 0;
    llg_value_t* target = llg_queue_value_nested_at(queue, indices, count);
    if (!target || (target->desc->kind != LLG_VALUE_CHANDLE &&
                    target->desc->kind != LLG_VALUE_EVENT))
        return 0;
    llg_value_t source = llg_value_from_chandle(target->desc, value);
    int result = llg_queue_value_set_nested_source(queue, indices, count, &source, &change);
    llg_value_drop(&source);
    /* No temporary owner may remain live across the callback. */
    llg_notify(queue->notify, queue->contents_dependency,
               queue->shape_dependency, change);
    return result;
}

int llg_queue_value_set_nested_container(
    llg_queue_value_array_t* queue, const sv4_t* indices, size_t count,
    const llg_dyn_value_array_t* source) {
    int change = 0;
    llg_value_t* target = llg_queue_value_nested_at(queue, indices, count);
    if (!target || target->desc->kind != LLG_VALUE_CONTAINER || !source ||
        !llg_value_desc_compatible(target->desc->element, source->element))
        return 0;
    llg_value_t source_value = {0};
    source_value.desc = target->desc;
    source_value.value.container = (llg_dyn_value_array_t*)source;
    int result = llg_queue_value_set_nested_source(queue, indices, count,
                                                   &source_value, &change);
    /* No temporary owner may remain live across the callback. */
    llg_notify(queue->notify, queue->contents_dependency,
               queue->shape_dependency, change);
    return result;
}

int llg_queue_value_set_nested_container_from_packed(
    llg_queue_value_array_t* queue, const sv4_t* indices, size_t count,
    const llg_dyn_array_t* source) {
    int change = 0;
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
                                                   &source_value, &change);
    llg_dyn_value_destroy(&converted);
    /* No temporary owner may remain live across the callback. */
    llg_notify(queue->notify, queue->contents_dependency,
               queue->shape_dependency, change);
    return result;
}

static void llg_queue_value_append(llg_queue_value_array_t* queue,
                                   const llg_value_t* source, int* change) {
    if (queue->size == queue->limit) {
        llg_container_warning("bounded queue push_back discarded new element");
        return;
    }
    if (queue->size == SIZE_MAX) llg_container_fatal("queue size overflow");
    llg_queue_value_reserve(queue, queue->size + 1);
    memset(&queue->data[queue->size], 0, sizeof(*queue->data));
    llg_value_copy(&queue->data[queue->size], queue->element, source);
    ++queue->size;
    llg_queue_value_invalidate_refs(queue);
    *change |= LLG_CONTAINER_CHANGED_CONTENTS | LLG_CONTAINER_CHANGED_SHAPE;
}

static void llg_queue_value_prepend(llg_queue_value_array_t* queue,
                                    const llg_value_t* source, int* change) {
    if (queue->limit == 0) {
        llg_container_warning("bounded queue push_front discarded new element");
        return;
    }
    size_t old_size = queue->size;
    size_t new_size = old_size < queue->limit ? old_size + 1 : old_size;
    llg_value_t incoming = {0};
    llg_value_copy(&incoming, queue->element, source);
    llg_queue_value_reserve(queue, new_size);
    if (old_size == queue->limit && old_size)
        llg_value_drop(&queue->data[old_size - 1]);
    if (new_size > 1)
        memmove(queue->data + 1, queue->data,
                (new_size - 1) * sizeof(*queue->data));
    queue->data[0] = incoming;
    queue->size = new_size;
    llg_queue_value_invalidate_refs(queue);
    if (old_size == queue->limit)
        llg_container_warning("bounded queue push_front discarded tail element");
    *change |= LLG_CONTAINER_CHANGED_CONTENTS |
                   (old_size != new_size ? LLG_CONTAINER_CHANGED_SHAPE : 0);
}

void llg_queue_value_push_front(llg_queue_value_array_t* queue, sv4_t value) {
    int change = 0;
    llg_value_t source = llg_value_from_packed(queue->element, value);
    llg_queue_value_prepend(queue, &source, &change);
    llg_value_drop(&source);
    /* No temporary owner may remain live across the callback. */
    llg_notify(queue->notify, queue->contents_dependency,
               queue->shape_dependency, change);
}

void llg_queue_value_push_back(llg_queue_value_array_t* queue, sv4_t value) {
    int change = 0;
    llg_value_t source = llg_value_from_packed(queue->element, value);
    llg_queue_value_append(queue, &source, &change);
    llg_value_drop(&source);
    /* No temporary owner may remain live across the callback. */
    llg_notify(queue->notify, queue->contents_dependency,
               queue->shape_dependency, change);
}

void llg_queue_value_push_front_real(llg_queue_value_array_t* queue, double value) {
    int change = 0;
    llg_value_t source = llg_value_from_real(queue->element, value);
    llg_queue_value_prepend(queue, &source, &change);
    llg_value_drop(&source);
    /* No temporary owner may remain live across the callback. */
    llg_notify(queue->notify, queue->contents_dependency,
               queue->shape_dependency, change);
}

void llg_queue_value_push_back_real(llg_queue_value_array_t* queue, double value) {
    int change = 0;
    llg_value_t source = llg_value_from_real(queue->element, value);
    llg_queue_value_append(queue, &source, &change);
    llg_value_drop(&source);
    /* No temporary owner may remain live across the callback. */
    llg_notify(queue->notify, queue->contents_dependency,
               queue->shape_dependency, change);
}

void llg_queue_value_push_front_string(llg_queue_value_array_t* queue,
                                       llg_string_t value) {
    int change = 0;
    llg_value_t source = llg_value_from_string(queue->element, value);
    llg_queue_value_prepend(queue, &source, &change);
    llg_value_drop(&source);
    /* No temporary owner may remain live across the callback. */
    llg_notify(queue->notify, queue->contents_dependency,
               queue->shape_dependency, change);
}

void llg_queue_value_push_back_string(llg_queue_value_array_t* queue,
                                      llg_string_t value) {
    int change = 0;
    llg_value_t source = llg_value_from_string(queue->element, value);
    llg_queue_value_append(queue, &source, &change);
    llg_value_drop(&source);
    /* No temporary owner may remain live across the callback. */
    llg_notify(queue->notify, queue->contents_dependency,
               queue->shape_dependency, change);
}

void llg_queue_value_push_front_chandle(llg_queue_value_array_t* queue,
                                        void* value) {
    int change = 0;
    llg_value_t source = llg_value_from_chandle(queue->element, value);
    llg_queue_value_prepend(queue, &source, &change);
    llg_value_drop(&source);
    /* No temporary owner may remain live across the callback. */
    llg_notify(queue->notify, queue->contents_dependency,
               queue->shape_dependency, change);
}

void llg_queue_value_push_back_chandle(llg_queue_value_array_t* queue,
                                       void* value) {
    int change = 0;
    llg_value_t source = llg_value_from_chandle(queue->element, value);
    llg_queue_value_append(queue, &source, &change);
    llg_value_drop(&source);
    /* No temporary owner may remain live across the callback. */
    llg_notify(queue->notify, queue->contents_dependency,
               queue->shape_dependency, change);
}

void llg_queue_value_push_front_container(
    llg_queue_value_array_t* queue, const llg_dyn_value_array_t* source) {
    int change = 0;
    llg_value_t value = llg_value_from_container(queue->element, source);
    llg_queue_value_prepend(queue, &value, &change);
    llg_value_drop(&value);
    /* No temporary owner may remain live across the callback. */
    llg_notify(queue->notify, queue->contents_dependency,
               queue->shape_dependency, change);
}

void llg_queue_value_push_back_container(
    llg_queue_value_array_t* queue, const llg_dyn_value_array_t* source) {
    int change = 0;
    llg_value_t value = llg_value_from_container(queue->element, source);
    llg_queue_value_append(queue, &value, &change);
    llg_value_drop(&value);
    /* No temporary owner may remain live across the callback. */
    llg_notify(queue->notify, queue->contents_dependency,
               queue->shape_dependency, change);
}

void llg_queue_value_push_front_container_from_packed(
    llg_queue_value_array_t* queue, const llg_dyn_array_t* source) {
    int change = 0;
    llg_value_t value = llg_value_from_packed_container(queue->element, source);
    llg_queue_value_prepend(queue, &value, &change);
    llg_value_drop(&value);
    /* No temporary owner may remain live across the callback. */
    llg_notify(queue->notify, queue->contents_dependency,
               queue->shape_dependency, change);
}

void llg_queue_value_push_back_container_from_packed(
    llg_queue_value_array_t* queue, const llg_dyn_array_t* source) {
    int change = 0;
    llg_value_t value = llg_value_from_packed_container(queue->element, source);
    llg_queue_value_append(queue, &value, &change);
    llg_value_drop(&value);
    /* No temporary owner may remain live across the callback. */
    llg_notify(queue->notify, queue->contents_dependency,
               queue->shape_dependency, change);
}

static int llg_queue_value_insert_source(llg_queue_value_array_t* queue,
                                         sv4_t index,
                                         const llg_value_t* source, int* change) {
    size_t native;
    if (!llg_index(index, queue->size, 1, &native)) {
        llg_container_warning("invalid recursive queue insert index");
        return 0;
    }
    if (queue->limit == 0 ||
        (queue->size == queue->limit && native == queue->size)) {
        llg_container_warning("bounded queue insert discarded new element");
        return 0;
    }
    size_t old_size = queue->size;
    size_t new_size = old_size < queue->limit ? old_size + 1 : old_size;
    llg_value_t incoming = {0};
    llg_value_copy(&incoming, queue->element, source);
    llg_queue_value_reserve(queue, new_size);
    if (old_size == queue->limit && old_size)
        llg_value_drop(&queue->data[old_size - 1]);
    if (native < new_size - 1)
        memmove(queue->data + native + 1, queue->data + native,
                (new_size - native - 1) * sizeof(*queue->data));
    queue->data[native] = incoming;
    queue->size = new_size;
    llg_queue_value_invalidate_refs(queue);
    if (old_size == queue->limit)
        llg_container_warning("bounded queue insert discarded tail element");
    *change |= LLG_CONTAINER_CHANGED_CONTENTS |
                   (old_size != new_size ? LLG_CONTAINER_CHANGED_SHAPE : 0);
    return 1;
}

int llg_queue_value_insert(llg_queue_value_array_t* queue, sv4_t index,
                           sv4_t value) {
    int change = 0;
    llg_value_t source = llg_value_from_packed(queue->element, value);
    int result = llg_queue_value_insert_source(queue, index, &source, &change);
    llg_value_drop(&source);
    /* No temporary owner may remain live across the callback. */
    llg_notify(queue->notify, queue->contents_dependency,
               queue->shape_dependency, change);
    return result;
}

int llg_queue_value_insert_real(llg_queue_value_array_t* queue, sv4_t index,
                                double value) {
    int change = 0;
    llg_value_t source = llg_value_from_real(queue->element, value);
    int result = llg_queue_value_insert_source(queue, index, &source, &change);
    llg_value_drop(&source);
    /* No temporary owner may remain live across the callback. */
    llg_notify(queue->notify, queue->contents_dependency,
               queue->shape_dependency, change);
    return result;
}

int llg_queue_value_insert_string(llg_queue_value_array_t* queue, sv4_t index,
                                  llg_string_t value) {
    int change = 0;
    llg_value_t source = llg_value_from_string(queue->element, value);
    int result = llg_queue_value_insert_source(queue, index, &source, &change);
    llg_value_drop(&source);
    /* No temporary owner may remain live across the callback. */
    llg_notify(queue->notify, queue->contents_dependency,
               queue->shape_dependency, change);
    return result;
}

int llg_queue_value_insert_chandle(llg_queue_value_array_t* queue,
                                   sv4_t index, void* value) {
    int change = 0;
    llg_value_t source = llg_value_from_chandle(queue->element, value);
    int result = llg_queue_value_insert_source(queue, index, &source, &change);
    llg_value_drop(&source);
    /* No temporary owner may remain live across the callback. */
    llg_notify(queue->notify, queue->contents_dependency,
               queue->shape_dependency, change);
    return result;
}

int llg_queue_value_insert_container(llg_queue_value_array_t* queue,
                                     sv4_t index,
                                     const llg_dyn_value_array_t* source) {
    int change = 0;
    llg_value_t value = llg_value_from_container(queue->element, source);
    int result = llg_queue_value_insert_source(queue, index, &value, &change);
    llg_value_drop(&value);
    /* No temporary owner may remain live across the callback. */
    llg_notify(queue->notify, queue->contents_dependency,
               queue->shape_dependency, change);
    return result;
}

int llg_queue_value_insert_container_from_packed(
    llg_queue_value_array_t* queue, sv4_t index, const llg_dyn_array_t* source) {
    int change = 0;
    llg_value_t value = llg_value_from_packed_container(queue->element, source);
    int result = llg_queue_value_insert_source(queue, index, &value, &change);
    llg_value_drop(&value);
    /* No temporary owner may remain live across the callback. */
    llg_notify(queue->notify, queue->contents_dependency,
               queue->shape_dependency, change);
    return result;
}

int llg_queue_value_delete_index(llg_queue_value_array_t* queue, sv4_t index) {
    size_t native;
    if (!llg_index(index, queue->size, 0, &native)) return 0;
    llg_value_drop(&queue->data[native]);
    if (native + 1 < queue->size)
        memmove(queue->data + native, queue->data + native + 1,
                (queue->size - native - 1) * sizeof(*queue->data));
    --queue->size;
    // Relocation transfers ownership; the unused tail must not retain an alias.
    memset(&queue->data[queue->size], 0, sizeof(*queue->data));
    llg_queue_value_invalidate_refs(queue);
    llg_notify(queue->notify, queue->contents_dependency,
               queue->shape_dependency,
               LLG_CONTAINER_CHANGED_CONTENTS | LLG_CONTAINER_CHANGED_SHAPE);
    return 1;
}

static void llg_dyn_new_count(llg_dyn_array_t* dst, size_t size,
                              const llg_dyn_array_t* initializer) {
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

void llg_dyn_new(llg_dyn_array_t* dst, sv4_t requested_size,
                 const llg_dyn_array_t* initializer) {
    size_t size = llg_checked_count(llg_dynamic_size(requested_size), sizeof(sv4_t));
    llg_dyn_new_count(dst, size, initializer);
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
        if (element_width == 0 || llg_sv4_width(source) % element_width != 0)
            llg_container_fatal(
                "streaming source width is not divisible by destination element width");
        count = llg_sv4_width(source) / element_width;
        *left = 0;
        *right = count ? (int64_t)(count - 1) : -1;
    } else if (count > 0
               && (count > (size_t)((LLG_SUPPORTED_WIDTH_LIMIT - 1u) / element_width)
                   || (uint64_t)count * element_width != llg_sv4_width(source))) {
        llg_container_fatal(
            "streaming selector width does not match source width");
    }
    if (count > (size_t)((LLG_SUPPORTED_WIDTH_LIMIT - 1u) / element_width))
        llg_container_fatal("streaming destination reaches supported width limit");
    return count;
}

static void llg_dyn_unstream_assign_impl(llg_dyn_array_t* dst, sv4_t source,
                             uint32_t slice, int right_to_left,
                             int selector_kind, sv4_t first, sv4_t second) {
    int64_t left;
    int64_t right;
    size_t count = llg_stream_unpacked_values(
        source, dst->element_width, selector_kind, first, second, &left, &right);
    sv4_t unpacked = sv4_unstream(source, slice, right_to_left);
    if (!selector_kind) {
        sv4_t* values = llg_alloc_items(count, sizeof(*values));
        uint32_t cursor = llg_sv4_width(unpacked);
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
    uint32_t cursor = llg_sv4_width(unpacked);
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

void llg_dyn_unstream_assign(llg_dyn_array_t* dst, sv4_t source,
                                  uint32_t slice, int right_to_left,
                                  int selector_kind, sv4_t first, sv4_t second) {
    llg_container_notify_fn notify = dst->notify;
    sv4_t* contents = dst->contents_dependency;
    sv4_t* shape = dst->shape_dependency;
    sv4_t changes = SV4_EMPTY;
    dst->notify = llg_stream_collect_change;
    dst->contents_dependency = &changes;
    dst->shape_dependency = NULL;
    llg_dyn_unstream_assign_impl(dst, source, slice, right_to_left,
                                     selector_kind, first, second);
    dst->notify = notify;
    dst->contents_dependency = contents;
    dst->shape_dependency = shape;
    llg_notify(notify, contents, shape, changes.width);
}

void llg_dyn_resize(llg_dyn_array_t* array, sv4_t size) {
    llg_dyn_new(array, size, array);
}

void llg_dyn_copy(llg_dyn_array_t* dst, const llg_dyn_array_t* src) {
    if (dst == src) return;
    size_t size = llg_checked_count((uint64_t)src->size, sizeof(sv4_t));
    llg_dyn_new_count(dst, size, src);
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
    for (size_t i = queue->capacity; i < capacity; ++i)
        queue->data[i] = (sv4_t)SV4_EMPTY;
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
                sv4_copy(&cell->value, &queue->data[i]);
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
    sv4_destroy_array(queue->data, queue->size);
    free(queue->data);
    free(queue->element_ids);
    memset(queue, 0, sizeof(*queue));
}

void llg_queue_delete(llg_queue_t* queue) {
    llg_queue_disconnect(queue, 0);
    int changed = queue->size != 0;
    sv4_destroy_array(queue->data, queue->size);
    queue->size = 0;
    llg_notify(queue->notify, queue->contents_dependency,
               queue->shape_dependency,
               changed ? LLG_CONTAINER_CHANGED_CONTENTS |
                            LLG_CONTAINER_CHANGED_SHAPE
                       : 0);
}

void llg_queue_copy(llg_queue_t* dst, const llg_queue_t* src) {
    // assign_values snapshots all inputs before destroying old storage. This
    // also gives self-assignment the normal queue identity invalidation rule.
    llg_queue_assign_values(dst, src->data, src->size);
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
    sv4_destroy_array(dst->data, dst->size);
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
    if (count > (size_t)((LLG_SUPPORTED_WIDTH_LIMIT - 1u) / queue->element_width))
        llg_container_fatal("streaming value reaches supported width limit");
    sv4_t* values = llg_alloc_items(count, sizeof(*values));
    for (size_t offset = 0; offset < count; ++offset) {
        int64_t index = llg_stream_index_at(left, right, offset);
        sv4_t packed_index = sv4_from_i64(index, 64);
        values[offset] = llg_queue_get(queue, packed_index);
        sv4_destroy(&packed_index);
    }
    sv4_t result = llg_pack_stream_values(values, count, queue->element_width,
                                          slice, right_to_left);
    sv4_destroy_array(values, count);
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
        sv4_copy(&queue->data[index], &initial);
        queue->element_ids[index] = llg_queue_new_element_id(queue);
    }
    sv4_destroy(&initial);
    queue->size = size;
    llg_notify(queue->notify, queue->contents_dependency,
               queue->shape_dependency,
               LLG_CONTAINER_CHANGED_CONTENTS | LLG_CONTAINER_CHANGED_SHAPE);
}

static void llg_queue_unstream_assign_impl(llg_queue_t* dst, sv4_t source,
                               uint32_t slice, int right_to_left,
                               int selector_kind, sv4_t first, sv4_t second) {
    int64_t left;
    int64_t right;
    size_t count = llg_stream_unpacked_values(
        source, dst->element_width, selector_kind, first, second, &left, &right);
    sv4_t unpacked = sv4_unstream(source, slice, right_to_left);
    if (!selector_kind) {
        sv4_t* values = llg_alloc_items(count, sizeof(*values));
        uint32_t cursor = llg_sv4_width(unpacked);
        for (size_t offset = 0; offset < count; ++offset) {
            uint32_t right_bit = cursor - dst->element_width;
            values[offset] = sv4_part_select(
                unpacked, (int64_t)cursor - 1, (int64_t)right_bit);
            cursor = right_bit;
        }
        llg_queue_assign_values(dst, values, count);
        sv4_destroy_array(values, count);
        free(values);
        sv4_destroy(&unpacked);
        return;
    }
    if (left < 0 || right < 0) {
        if (count) llg_container_fatal(
            "queue streaming target selector must be a nonnegative range");
        sv4_destroy(&unpacked);
        return;
    }
    int64_t high = left > right ? left : right;
    if (high == INT64_MAX)
        llg_container_fatal("queue streaming target index overflows size");
    if ((uint64_t)high >= (uint64_t)dst->size)
        llg_queue_resize_default(dst, (size_t)high + 1);
    uint32_t cursor = llg_sv4_width(unpacked);
    for (size_t offset = 0; offset < count; ++offset) {
        uint32_t right_bit = cursor - dst->element_width;
        sv4_t value = sv4_part_select(
            unpacked, (int64_t)cursor - 1, (int64_t)right_bit);
        sv4_t index = sv4_from_i64(llg_stream_index_at(left, right, offset), 64);
        llg_queue_set(dst, index, value);
        sv4_destroy(&index);
        sv4_destroy(&value);
        cursor = right_bit;
    }
    sv4_destroy(&unpacked);
}

void llg_queue_unstream_assign(llg_queue_t* dst, sv4_t source,
                                  uint32_t slice, int right_to_left,
                                  int selector_kind, sv4_t first, sv4_t second) {
    llg_container_notify_fn notify = dst->notify;
    sv4_t* contents = dst->contents_dependency;
    sv4_t* shape = dst->shape_dependency;
    sv4_t changes = SV4_EMPTY;
    dst->notify = llg_stream_collect_change;
    dst->contents_dependency = &changes;
    dst->shape_dependency = NULL;
    llg_queue_unstream_assign_impl(dst, source, slice, right_to_left,
                                     selector_kind, first, second);
    dst->notify = notify;
    dst->contents_dependency = contents;
    dst->shape_dependency = shape;
    llg_notify(notify, contents, shape, changes.width);
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
    sv4_destroy_array(dst->data, dst->size);
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
    return sv4_clone(&queue->data[native]);
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
        sv4_destroy(&queue->data[queue->size - 1]);
        llg_container_warning("bounded queue push_front discarded tail element");
    }
    if (queue->size > 1) {
        for (size_t i = queue->size - 1; i > 0; --i)
            sv4_move(&queue->data[i], &queue->data[i - 1]);
        memmove(queue->element_ids + 1, queue->element_ids,
                (queue->size - 1) * sizeof(*queue->element_ids));
    }
    sv4_move(&queue->data[0], &assigned);
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
    if (sv4_same(queue->data[native], assigned)) {
        sv4_destroy(&assigned);
        return 1;
    }
    sv4_move(&queue->data[native], &assigned);
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
        sv4_destroy(&queue->data[queue->size - 1]);
        llg_container_warning("bounded queue insert discarded tail element");
    }
    if (native + 1 < queue->size) {
        for (size_t i = queue->size - 1; i > native; --i)
            sv4_move(&queue->data[i], &queue->data[i - 1]);
        memmove(queue->element_ids + native + 1,
                queue->element_ids + native,
                (queue->size - native - 1) * sizeof(*queue->element_ids));
    }
    sv4_move(&queue->data[native], &assigned);
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
    sv4_destroy(&queue->data[native]);
    if (native + 1 < queue->size) {
        for (size_t i = native; i + 1 < queue->size; ++i)
            sv4_move(&queue->data[i], &queue->data[i + 1]);
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

void llg_queue_pop_front_into(llg_queue_t* queue, sv4_t* out) {
    /* Publish ownership before notifying. Generated callers provide a
     * registered destination, so callback termination cannot strand it. */
    if (!out) llg_container_fatal("queue pop requires output storage");
    sv4_replace(out, llg_queue_front(queue));
    if (queue->size) {
        llg_queue_disconnect(queue, queue->element_ids[0]);
        sv4_destroy(&queue->data[0]);
        if (queue->size > 1) {
            for (size_t i = 0; i + 1 < queue->size; ++i)
                sv4_move(&queue->data[i], &queue->data[i + 1]);
            memmove(queue->element_ids, queue->element_ids + 1,
                    (queue->size - 1) * sizeof(*queue->element_ids));
        }
        --queue->size;
        llg_notify(queue->notify, queue->contents_dependency,
                   queue->shape_dependency,
                   LLG_CONTAINER_CHANGED_CONTENTS |
                       LLG_CONTAINER_CHANGED_SHAPE);
    }

}

sv4_t llg_queue_pop_front(llg_queue_t* queue) {
    sv4_t result = SV4_EMPTY;
    llg_queue_pop_front_into(queue, &result);
    return result;
}

void llg_queue_pop_back_into(llg_queue_t* queue, sv4_t* out) {
    /* Publish ownership before notifying. Generated callers provide a
     * registered destination, so callback termination cannot strand it. */
    if (!out) llg_container_fatal("queue pop requires output storage");
    sv4_replace(out, llg_queue_back(queue));
    if (queue->size) {
        llg_queue_disconnect(queue, queue->element_ids[queue->size - 1]);
        sv4_destroy(&queue->data[queue->size - 1]);
        --queue->size;
        llg_notify(queue->notify, queue->contents_dependency,
                   queue->shape_dependency,
                   LLG_CONTAINER_CHANGED_CONTENTS |
                       LLG_CONTAINER_CHANGED_SHAPE);
    }

}

sv4_t llg_queue_pop_back(llg_queue_t* queue) {
    sv4_t result = SV4_EMPTY;
    llg_queue_pop_back_into(queue, &result);
    return result;
}

sv4_t llg_queue_front(const llg_queue_t* queue) {
    if (!queue->size)
        return llg_element_default(queue->element_width, queue->element_signed,
                                   queue->element_two_state);
    return sv4_clone(&queue->data[0]);
}

sv4_t llg_queue_back(const llg_queue_t* queue) {
    if (!queue->size)
        return llg_element_default(queue->element_width, queue->element_signed,
                                   queue->element_two_state);
    return sv4_clone(&queue->data[queue->size - 1]);
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

uint64_t llg_queue_ref_identity(const llg_queue_t* queue, uint64_t index) {
    return queue && index < queue->size ? queue->element_ids[index] : 0;
}

static size_t llg_queue_ref_index(const llg_queue_t* queue,
                                  uint64_t identity) {
    if (!queue || identity == 0) return SIZE_MAX;
    for (size_t index = 0; index < queue->size; ++index) {
        if (queue->element_ids[index] == identity) return index;
    }
    return SIZE_MAX;
}

sv4_t llg_queue_ref_read(const llg_queue_t* queue, uint64_t identity) {
    size_t index = llg_queue_ref_index(queue, identity);
    if (index == SIZE_MAX)
        return llg_element_default(queue ? queue->element_width : 1,
                                   queue ? queue->element_signed : 0,
                                   queue ? queue->element_two_state : 0);
    return sv4_clone(&queue->data[index]);
}

int llg_queue_ref_write(llg_queue_t* queue, uint64_t identity, sv4_t value) {
    size_t index = llg_queue_ref_index(queue, identity);
    if (index == SIZE_MAX) return 0;
    sv4_t assigned = llg_element_assign(value, queue->element_width,
                                        queue->element_signed,
                                        queue->element_two_state);
    if (sv4_same(queue->data[index], assigned)) {
        sv4_destroy(&assigned);
        return 1;
    }
    sv4_move(&queue->data[index], &assigned);
    llg_notify(queue->notify, queue->contents_dependency,
               queue->shape_dependency, LLG_CONTAINER_CHANGED_CONTENTS);
    return 1;
}

void* llg_queue_ref_acquire(llg_queue_t* queue, uint64_t index) {
    if (!queue) llg_container_fatal("null queue reference source");
    uint64_t identity = llg_queue_ref_identity(queue, index);
    for (struct llg_queue_cell* cell = queue->references; cell; cell = cell->next) {
        if (cell->identity == identity && identity) {
            if (cell->refs == SIZE_MAX) llg_container_fatal("queue reference count overflow");
            ++cell->refs;
            return cell;
        }
    }
    struct llg_queue_cell* cell = llg_alloc_items(1, sizeof(*cell));
    memset(cell, 0, sizeof(*cell));
    cell->refs = 1;
    cell->identity = identity;
    cell->two_state = queue->element_two_state;
    cell->value = identity ? sv4_clone(&queue->data[index]) : llg_element_default(
        queue->element_width, queue->element_signed, queue->element_two_state);
    if (identity) {
        cell->owner = queue;
        cell->next = queue->references;
        queue->references = cell;
    }
    return cell;
}

void llg_queue_ref_release(void* ptr) {
    struct llg_queue_cell* cell = ptr;
    if (!cell) return;
    if (!cell->refs) llg_container_fatal("queue reference count underflow");
    if (--cell->refs) return;
    if (cell->owner) {
        struct llg_queue_cell** link = &cell->owner->references;
        while (*link && *link != cell) link = &(*link)->next;
        if (*link) *link = cell->next;
    }
    sv4_destroy(&cell->value);
    free(cell);
}

sv4_t llg_queue_cell_read(const void* ptr) {
    const struct llg_queue_cell* cell = ptr;
    if (!cell) llg_container_fatal("null retained queue reference");
    if (cell->owner) {
        size_t index = llg_queue_ref_index(cell->owner, cell->identity);
        if (index == SIZE_MAX) llg_container_fatal("queue reference was not disconnected");
        return sv4_clone(&cell->owner->data[index]);
    }
    return sv4_clone(&cell->value);
}

int llg_queue_cell_write(void* ptr, sv4_t value) {
    struct llg_queue_cell* cell = ptr;
    if (!cell) llg_container_fatal("null retained queue reference");
    if (!cell->identity) return 0; // invalid actual, not a removed valid element
    if (cell->owner) return llg_queue_ref_write(cell->owner, cell->identity, value);
    sv4_replace(&cell->value, llg_element_assign(value, llg_sv4_width(cell->value),
                                                llg_sv4_signed(cell->value), cell->two_state));
    return 1;
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
    if (key_width > (LLG_SUPPORTED_WIDTH_LIMIT - 1u))
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
    for (size_t i = 0; i < array->size; ++i) {
        sv4_destroy(&array->entries[i].integral_key);
        sv4_destroy(&array->entries[i].value);
        free(array->entries[i].string_key);
        memset(&array->entries[i], 0, sizeof(*array->entries));
    }
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
    sv4_destroy(&array->default_value);
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
    return sv4_clone(&array->entries[index].value);
}

sv4_t llg_assoc_reduce(const llg_assoc_t* array, int operation) {
    sv4_t result = llg_reduce_identity(array->element_width,
                                       array->element_signed, operation);
    for (size_t i = 0; i < array->size; ++i)
        sv4_replace(&result, llg_reduce_step(result, array->entries[i].value, operation));
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
            ? sv4_clone(&array->entries[i].integral_key)
            : sv4_from_u64(0, 32, 1);
        sv4_t value = llg_container_eval(
            eval, array->entries[i].value, index, context);
        sv4_replace(&value, llg_element_assign(value, result_width, result_signed,
                                               result_two_state));
        sv4_replace(&result, llg_reduce_step(result, value, operation));
        sv4_replace(&result, llg_element_assign(result, result_width,
                                               result_signed, result_two_state));
        sv4_destroy(&value);
        sv4_destroy(&index);
    }
    return result;
}

static int llg_key_negative(sv4_t value) {
    return llg_sv4_width(value) && llg_sv4_signed(value) &&
        ((llg_sv4_word(value, (llg_sv4_width(value) - 1u) / 64u, LLG_SV4_BITS) >>
          ((llg_sv4_width(value) - 1u) % 64u)) & 1u);
}

static int llg_normalize_integral_key(sv4_t input, uint32_t width,
                                      int8_t is_signed, int two_state,
                                      sv4_t* output) {
    // Validate before any narrowing/two-state conversion can erase X/Z.
    if (sv4_is_unknown(input)) return 0;
    if (width) {
        sv4_replace(output, sv4_cast(input, width, is_signed));
        if (two_state) sv4_replace(output, sv4_to_two_state(*output));
    } else {
        // Wildcard keys retain only the significant bits, not an artificial
        // model/support-limit width. Keep one sign bit for negative values.
        int negative = llg_key_negative(input);
        uint32_t used = llg_sv4_width(input);
        while (used > 1u) {
            uint32_t bit = negative ? used - 2u : used - 1u;
            int value = (int)((llg_sv4_word(input, bit / 64u, LLG_SV4_BITS) >> (bit % 64u)) & 1u);
            if (value != negative) break;
            --used;
        }
        if (!used) used = 1;
        sv4_replace(output, sv4_resize(input, used, llg_sv4_signed(input)));
        llg_sv4_set_signed(output, (int8_t)negative);
    }
    return 1;
}

static int llg_assoc_normalize_key(const llg_assoc_t* array, sv4_t input,
                                   sv4_t* output) {
    llg_assoc_check_kind(array, LLG_ASSOC_INTEGRAL);
    return llg_normalize_integral_key(input, array->key_width, array->key_signed,
                                      array->key_two_state, output);
}

static uint64_t llg_key_word(sv4_t value, size_t limb, int negative) {
    size_t count = (llg_sv4_width(value) + 63u) / 64u;
    if (limb >= count) return negative ? UINT64_MAX : 0;
    uint64_t word = llg_sv4_word(value, limb, LLG_SV4_BITS);
    uint32_t tail = llg_sv4_width(value) % 64u;
    if (negative && limb + 1 == count && tail)
        word |= UINT64_MAX << tail;
    return word;
}

static int llg_integral_compare(sv4_t a, sv4_t b) {
    int an = llg_key_negative(a), bn = llg_key_negative(b);
    if (an != bn) return an ? -1 : 1;
    size_t count = ((llg_sv4_width(a) > llg_sv4_width(b) ? llg_sv4_width(a) : llg_sv4_width(b)) + 63u) / 64u;
    while (count--) {
        uint64_t av = llg_key_word(a, count, an);
        uint64_t bv = llg_key_word(b, count, bn);
        if (av != bv) return av < bv ? -1 : 1;
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
    sv4_t normalized = SV4_EMPTY;
    const sv4_t* result = &array->default_value;
    if (!llg_assoc_normalize_key(array, key, &normalized)) {
        llg_container_warning("invalid associative-array integral key read");
    } else {
        int found;
        size_t position = llg_assoc_integral_position(array, normalized, &found);
        if (found) result = &array->entries[position].value;
    }
    sv4_destroy(&normalized);
    return sv4_clone(result);
}

int llg_assoc_set_integral(llg_assoc_t* array, sv4_t key, sv4_t value) {
    sv4_t normalized = SV4_EMPTY;
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
        sv4_move(&array->entries[position].integral_key, &normalized);
        ++array->size;
    }
    sv4_move(&array->entries[position].value, &assigned);
    sv4_destroy(&normalized);
    llg_notify(array->notify, array->contents_dependency,
               array->shape_dependency,
               (contents_changed ? LLG_CONTAINER_CHANGED_CONTENTS : 0) |
                   (shape_changed ? LLG_CONTAINER_CHANGED_SHAPE : 0));
    return 1;
}

int llg_assoc_exists_integral(const llg_assoc_t* array, sv4_t key) {
    sv4_t normalized = SV4_EMPTY;
    if (!llg_assoc_normalize_key(array, key, &normalized)) return 0;
    int found;
    (void)llg_assoc_integral_position(array, normalized, &found);
    sv4_destroy(&normalized);
    return found;
}

int llg_assoc_delete_integral(llg_assoc_t* array, sv4_t key) {
    sv4_t normalized = SV4_EMPTY;
    if (!llg_assoc_normalize_key(array, key, &normalized)) return 0;
    int found;
    size_t position = llg_assoc_integral_position(array, normalized, &found);
    sv4_destroy(&normalized);
    if (!found) return 0;
    sv4_destroy(&array->entries[position].integral_key);
    sv4_destroy(&array->entries[position].value);
    if (position + 1 < array->size)
        memmove(array->entries + position, array->entries + position + 1,
                (array->size - position - 1) * sizeof(*array->entries));
    --array->size;
    memset(&array->entries[array->size], 0, sizeof(*array->entries));
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
    sv4_move(&array->default_value, &assigned);
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
    sv4_move(&array->default_value, &default_value);
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
        sv4_copy(key, &array->entries[direction > 0 ? 0 : array->size - 1].integral_key);
        return 1;
    }
    sv4_t normalized = SV4_EMPTY;
    if (!llg_assoc_normalize_key(array, *key, &normalized)) return 0;
    int found;
    size_t position = llg_assoc_integral_position(array, normalized, &found);
    sv4_destroy(&normalized);
    if (direction > 0) {
        if (found) ++position;
        if (position >= array->size) return 0;
    } else {
        if (position == 0) return 0;
        --position;
    }
    sv4_copy(key, &array->entries[position].integral_key);
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
    if (found) return sv4_clone(&array->entries[position].value);
    return sv4_clone(&array->default_value);
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
    sv4_move(&array->entries[position].value, &assigned);
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
    sv4_destroy(&array->entries[position].value);
    free(array->entries[position].string_key);
    if (position + 1 < array->size)
        memmove(array->entries + position, array->entries + position + 1,
                (array->size - position - 1) * sizeof(*array->entries));
    --array->size;
    memset(&array->entries[array->size], 0, sizeof(*array->entries));
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
        entries[i].integral_key = sv4_clone(&src->entries[i].integral_key);
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
    sv4_move(&dst->default_value, &default_value);
    dst->has_default_value = src->has_default_value;
    llg_notify(notify, contents_dependency, shape_dependency,
               (contents_changed ? LLG_CONTAINER_CHANGED_CONTENTS : 0) |
                   (shape_changed ? LLG_CONTAINER_CHANGED_SHAPE : 0));
}

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
    if (key_width > (LLG_SUPPORTED_WIDTH_LIMIT - 1u))
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
        sv4_destroy(&array->entries[i].integral_key);
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
    return llg_normalize_integral_key(input, array->key_width, array->key_signed,
                                      array->key_two_state, output);
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
                                      const llg_value_t* source, int* change) {
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
            ? sv4_clone(integral_key) : (sv4_t)SV4_EMPTY;
        array->entries[position].string_key = key_copy;
        array->entries[position].string_length = string_length;
        ++array->size;
        llg_value_copy(&array->entries[position].value, array->element, source);
    } else if (contents_changed) {
        llg_value_copy(&array->entries[position].value, array->element, source);
    }
    if (shape_changed) llg_assoc_value_invalidate_refs(array);
    *change |= (contents_changed ? LLG_CONTAINER_CHANGED_CONTENTS : 0) |
                   (shape_changed ? LLG_CONTAINER_CHANGED_SHAPE : 0);
    return 1;
}

static llg_value_t llg_assoc_value_packed_source(
    const llg_assoc_value_t* array, sv4_t value) {
    return llg_value_from_packed(array->element, value);
}

sv4_t llg_assoc_value_get_integral(const llg_assoc_value_t* array, sv4_t key) {
    sv4_t normalized = SV4_EMPTY;
    sv4_t result_value;

    int found = 0;
    if (llg_assoc_value_normalize_key(array, key, &normalized)) {
        size_t position = llg_assoc_value_integral_position(array, normalized,
                                                             &found);
        if (found && array->entries[position].value.desc->kind == LLG_VALUE_PACKED)
            do { result_value = sv4_clone(&array->entries[position].value.value.packed); goto cleanup_key; } while (0);
    }
    do { result_value = array->default_value.desc &&
                   array->default_value.desc->kind == LLG_VALUE_PACKED
        ? sv4_clone(&array->default_value.value.packed)
        : sv4_from_u64(0, 1, 0); goto cleanup_key; } while (0);
cleanup_key:
    sv4_destroy(&normalized);
    return result_value;
}

double llg_assoc_value_get_integral_real(const llg_assoc_value_t* array,
                                         sv4_t key) {
    sv4_t normalized = SV4_EMPTY;
    double result_value;

    int found = 0;
    if (llg_assoc_value_normalize_key(array, key, &normalized)) {
        size_t position = llg_assoc_value_integral_position(array, normalized,
                                                             &found);
        if (found && array->entries[position].value.desc->kind == LLG_VALUE_REAL)
            do { result_value = array->entries[position].value.value.real; goto cleanup_key; } while (0);
    }
    do { result_value = array->default_value.desc &&
                   array->default_value.desc->kind == LLG_VALUE_REAL
        ? array->default_value.value.real
        : 0.0; goto cleanup_key; } while (0);
cleanup_key:
    sv4_destroy(&normalized);
    return result_value;
}

llg_string_t llg_assoc_value_get_integral_string(
    const llg_assoc_value_t* array, sv4_t key) {
    sv4_t normalized = SV4_EMPTY;
    llg_string_t result_value;

    int found = 0;
    if (llg_assoc_value_normalize_key(array, key, &normalized)) {
        size_t position = llg_assoc_value_integral_position(array, normalized,
                                                             &found);
        if (found && array->entries[position].value.desc->kind == LLG_VALUE_STRING)
            do { result_value = llg_string_clone(&array->entries[position].value.value.string); goto cleanup_key; } while (0);
    }
    do { result_value = array->default_value.desc &&
                   array->default_value.desc->kind == LLG_VALUE_STRING
        ? llg_string_clone(&array->default_value.value.string)
        : (llg_string_t){0}; goto cleanup_key; } while (0);
cleanup_key:
    sv4_destroy(&normalized);
    return result_value;
}

void* llg_assoc_value_get_integral_chandle(
    const llg_assoc_value_t* array, sv4_t key) {
    sv4_t normalized = SV4_EMPTY;
    void* result_value;

    int found = 0;
    if (llg_assoc_value_normalize_key(array, key, &normalized)) {
        size_t position = llg_assoc_value_integral_position(array, normalized,
                                                             &found);
        if (found && (array->entries[position].value.desc->kind == LLG_VALUE_CHANDLE ||
                      array->entries[position].value.desc->kind == LLG_VALUE_EVENT))
            do { result_value = array->entries[position].value.value.handle; goto cleanup_key; } while (0);
    }
    do { result_value = array->default_value.desc &&
                   (array->default_value.desc->kind == LLG_VALUE_CHANDLE ||
                    array->default_value.desc->kind == LLG_VALUE_EVENT)
        ? array->default_value.value.handle
        : NULL; goto cleanup_key; } while (0);
cleanup_key:
    sv4_destroy(&normalized);
    return result_value;
}

static llg_value_t* llg_assoc_value_nested_at_integral(
    const llg_assoc_value_t* array, const sv4_t* indices, size_t count) {
    sv4_t normalized = SV4_EMPTY;
    llg_value_t* result_value;

    if (!indices || count < 2 || array->key_kind != LLG_ASSOC_INTEGRAL)
        do { result_value = NULL; goto cleanup_key; } while (0);
    int found = 0;
    if (!llg_assoc_value_normalize_key(array, indices[0], &normalized))
        do { result_value = NULL; goto cleanup_key; } while (0);
    size_t position = llg_assoc_value_integral_position(array, normalized,
                                                         &found);
    if (!found) do { result_value = NULL; goto cleanup_key; } while (0);
    llg_value_t* value = &array->entries[position].value;
    for (size_t index = 1; value && index < count; ++index) {
        if (!value->desc || value->desc->kind != LLG_VALUE_CONTAINER ||
            !value->value.container)
            do { result_value = NULL; goto cleanup_key; } while (0);
        value = llg_dyn_value_at(value->value.container, indices[index]);
    }
    do { result_value = value; goto cleanup_key; } while (0);
cleanup_key:
    sv4_destroy(&normalized);
    return result_value;
}

sv4_t llg_assoc_value_get_nested_integral(
    const llg_assoc_value_t* array, const sv4_t* indices, size_t count) {
    llg_value_t* value = llg_assoc_value_nested_at_integral(array, indices, count);
    return value && value->desc->kind == LLG_VALUE_PACKED
        ? sv4_clone(&value->value.packed)
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
    int change = 0;
    sv4_t normalized = SV4_EMPTY;
    int result_value;

    if (!llg_assoc_value_normalize_key(array, key, &normalized)) do { result_value = 0; goto cleanup_key; } while (0);
    llg_value_t source = llg_assoc_value_packed_source(array, value);
    int result = llg_assoc_value_set_source(array, &normalized, NULL, 0, &source, &change);
    llg_value_drop(&source);
    do { result_value = result; goto cleanup_key; } while (0);
cleanup_key:
    sv4_destroy(&normalized);
    /* No temporary owner may remain live across the callback. */
    llg_notify(array->notify, array->contents_dependency,
               array->shape_dependency, change);
    return result_value;
}

int llg_assoc_value_set_integral_real(llg_assoc_value_t* array, sv4_t key,
                                      double value) {
    int change = 0;
    sv4_t normalized = SV4_EMPTY;
    int result_value;

    if (!llg_assoc_value_normalize_key(array, key, &normalized)) do { result_value = 0; goto cleanup_key; } while (0);
    llg_value_t source = llg_value_from_real(array->element, value);
    int result = llg_assoc_value_set_source(array, &normalized, NULL, 0, &source, &change);
    llg_value_drop(&source);
    do { result_value = result; goto cleanup_key; } while (0);
cleanup_key:
    sv4_destroy(&normalized);
    /* No temporary owner may remain live across the callback. */
    llg_notify(array->notify, array->contents_dependency,
               array->shape_dependency, change);
    return result_value;
}

int llg_assoc_value_set_integral_string(llg_assoc_value_t* array, sv4_t key,
                                        llg_string_t value) {
    int change = 0;
    sv4_t normalized = SV4_EMPTY;
    int result_value;

    if (!llg_assoc_value_normalize_key(array, key, &normalized)) {
        llg_string_destroy(&value);
        do { result_value = 0; goto cleanup_key; } while (0);
    }
    llg_value_t source = llg_value_from_string(array->element, value);
    int result = llg_assoc_value_set_source(array, &normalized, NULL, 0, &source, &change);
    llg_value_drop(&source);
    do { result_value = result; goto cleanup_key; } while (0);
cleanup_key:
    sv4_destroy(&normalized);
    /* No temporary owner may remain live across the callback. */
    llg_notify(array->notify, array->contents_dependency,
               array->shape_dependency, change);
    return result_value;
}

int llg_assoc_value_set_integral_chandle(llg_assoc_value_t* array, sv4_t key,
                                         void* value) {
    int change = 0;
    sv4_t normalized = SV4_EMPTY;
    int result_value;

    if (!llg_assoc_value_normalize_key(array, key, &normalized)) do { result_value = 0; goto cleanup_key; } while (0);
    llg_value_t source = llg_value_from_chandle(array->element, value);
    do { result_value = llg_assoc_value_set_source(array, &normalized, NULL, 0, &source, &change); goto cleanup_key; } while (0);
cleanup_key:
    sv4_destroy(&normalized);
    /* No temporary owner may remain live across the callback. */
    llg_notify(array->notify, array->contents_dependency,
               array->shape_dependency, change);
    return result_value;
}

int llg_assoc_value_set_nested_integral_container(
    llg_assoc_value_t* array, const sv4_t* indices, size_t count,
    const llg_dyn_value_array_t* source) {
    int change = 0;
    sv4_t normalized = SV4_EMPTY;
    int result_value;

    if (!indices || count == 0 ||
        !llg_assoc_value_normalize_key(array, indices[0], &normalized))
        do { result_value = 0; goto cleanup_key; } while (0);
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
        do { result_value = 0; goto cleanup_key; } while (0);
    llg_value_t value = llg_value_from_container(target_desc, source);
    int result;
    if (count == 1) {
        result = llg_assoc_value_set_source(array, &normalized, NULL, 0, &value, &change);
    } else {
        result = 0;
        if (target && target->desc->kind == LLG_VALUE_CONTAINER) {
            result = 1;
            if (!llg_value_equal(target, &value)) {
                llg_value_copy(target, target->desc, &value);
                change |= LLG_CONTAINER_CHANGED_CONTENTS;
            }
        }
    }
    llg_value_drop(&value);
    do { result_value = result; goto cleanup_key; } while (0);
cleanup_key:
    sv4_destroy(&normalized);
    /* No temporary owner may remain live across the callback. */
    llg_notify(array->notify, array->contents_dependency,
               array->shape_dependency, change);
    return result_value;
}

int llg_assoc_value_set_nested_integral_container_from_packed(
    llg_assoc_value_t* array, const sv4_t* indices, size_t count,
    const llg_dyn_array_t* source) {
    int change = 0;
    sv4_t normalized = SV4_EMPTY;
    int result_value;

    if (!indices || count == 0 || !source) do { result_value = 0; goto cleanup_key; } while (0);
    if (!llg_assoc_value_normalize_key(array, indices[0], &normalized)) do { result_value = 0; goto cleanup_key; } while (0);

    const llg_value_desc_t* target_desc = NULL;
    if (count == 1) {
        target_desc = array->element;
    } else {
        llg_value_t* target = llg_assoc_value_nested_at_integral(array, indices,
                                                                  count);
        if (!target || target->desc->kind != LLG_VALUE_CONTAINER)
            do { result_value = 0; goto cleanup_key; } while (0);
        target_desc = target->desc;
    }
    if (!target_desc || target_desc->kind != LLG_VALUE_CONTAINER ||
        !target_desc->element ||
        (target_desc->element->kind != LLG_VALUE_PACKED &&
         target_desc->element->kind != LLG_VALUE_REAL))
        do { result_value = 0; goto cleanup_key; } while (0);

    llg_value_t value = llg_value_from_packed_container(target_desc, source);
    int result;
    if (count == 1) {
        result = llg_assoc_value_set_source(array, &normalized, NULL, 0, &value, &change);
    } else {
        result = 0;
        llg_value_t* target = llg_assoc_value_nested_at_integral(array, indices,
                                                                  count);
        if (target && target->desc->kind == LLG_VALUE_CONTAINER) {
            result = 1;
            if (!llg_value_equal(target, &value)) {
                llg_value_copy(target, target->desc, &value);
                change |= LLG_CONTAINER_CHANGED_CONTENTS;
            }
        }
    }
    llg_value_drop(&value);
    do { result_value = result; goto cleanup_key; } while (0);
cleanup_key:
    sv4_destroy(&normalized);
    /* No temporary owner may remain live across the callback. */
    llg_notify(array->notify, array->contents_dependency,
               array->shape_dependency, change);
    return result_value;
}

int llg_assoc_value_set_nested_integral(
    llg_assoc_value_t* array, const sv4_t* indices, size_t count,
    sv4_t value) {
    llg_value_t* target = llg_assoc_value_nested_at_integral(array, indices, count);
    if (!target || target->desc->kind != LLG_VALUE_PACKED) return 0;
    sv4_t assigned = sv4_cast(value, target->desc->packed_width,
                              target->desc->packed_signed);
    if (target->desc->packed_two_state) sv4_replace(&assigned, sv4_to_two_state(assigned));
    if (sv4_same(target->value.packed, assigned)) {
        sv4_destroy(&assigned);
        return 1;
    }
    sv4_move(&target->value.packed, &assigned);
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
/* Destination-passing `_to` forms; private fragment, see value/destinations.h. */
void llg_dyn_value_get_nested_to(sv4_t* dst, const llg_dyn_value_array_t* array, const sv4_t* indices, size_t count) {
    sv4_replace(dst, llg_dyn_value_get_nested(array, indices, count));
}
void llg_fixed_stream_source_to(sv4_t* dst, const sv4_t* values, int64_t declaration_left, int64_t declaration_right, uint32_t element_width, const sv4_t* fallback, int selector_kind, const sv4_t* first, const sv4_t* second) {
    sv4_replace(dst, llg_fixed_stream_source(values, declaration_left, declaration_right, element_width, *fallback, selector_kind, *first, *second));
}
void llg_stream_unpack_source_to(sv4_t* dst, const sv4_t* value, uint64_t bits, uint32_t slice, int right_to_left) {
    sv4_replace(dst, llg_stream_unpack_source(*value, bits, slice, right_to_left));
}
void llg_fixed_image_stream_source_to(sv4_t* dst, const sv4_t* image, int64_t declaration_left, int64_t declaration_right, uint32_t element_width, const sv4_t* fallback, int selector_kind, const sv4_t* first, const sv4_t* second) {
    sv4_replace(dst, llg_fixed_image_stream_source(*image, declaration_left, declaration_right, element_width, *fallback, selector_kind, *first, *second));
}
void llg_stream_to_fixed_to(sv4_t* dst, const sv4_t* value, uint32_t width, int is_signed) {
    sv4_replace(dst, llg_stream_to_fixed(*value, width, is_signed));
}
void llg_queue_value_get_to(sv4_t* dst, const llg_queue_value_array_t* queue, const sv4_t* index) {
    sv4_replace(dst, llg_queue_value_get(queue, *index));
}
void llg_queue_value_get_nested_to(sv4_t* dst, const llg_queue_value_array_t* queue, const sv4_t* indices, size_t count) {
    sv4_replace(dst, llg_queue_value_get_nested(queue, indices, count));
}
void llg_dyn_stream_to(sv4_t* dst, const llg_dyn_array_t* array, uint32_t slice, int right_to_left, int selector_kind, const sv4_t* first, const sv4_t* second) {
    sv4_replace(dst, llg_dyn_stream(array, slice, right_to_left, selector_kind, *first, *second));
}
void llg_dyn_get_to(sv4_t* dst, const llg_dyn_array_t* array, const sv4_t* index) {
    sv4_replace(dst, llg_dyn_get(array, *index));
}
void llg_dyn_reduce_to(sv4_t* dst, const llg_dyn_array_t* array, int operation) {
    sv4_replace(dst, llg_dyn_reduce(array, operation));
}
void llg_dyn_reduce_with_to(sv4_t* dst, const llg_dyn_array_t* array, int operation, uint32_t result_width, int8_t result_signed, int result_two_state, llg_container_eval_fn eval, void* context) {
    sv4_replace(dst, llg_dyn_reduce_with(array, operation, result_width, result_signed, result_two_state, eval, context));
}
void llg_queue_stream_to(sv4_t* dst, const llg_queue_t* queue, uint32_t slice, int right_to_left, int selector_kind, const sv4_t* first, const sv4_t* second) {
    sv4_replace(dst, llg_queue_stream(queue, slice, right_to_left, selector_kind, *first, *second));
}
void llg_queue_get_to(sv4_t* dst, const llg_queue_t* queue, const sv4_t* index) {
    sv4_replace(dst, llg_queue_get(queue, *index));
}
void llg_queue_pop_front_to(sv4_t* dst, llg_queue_t* queue) {
    sv4_replace(dst, llg_queue_pop_front(queue));
}
void llg_queue_pop_back_to(sv4_t* dst, llg_queue_t* queue) {
    sv4_replace(dst, llg_queue_pop_back(queue));
}
void llg_queue_front_to(sv4_t* dst, const llg_queue_t* queue) {
    sv4_replace(dst, llg_queue_front(queue));
}
void llg_queue_back_to(sv4_t* dst, const llg_queue_t* queue) {
    sv4_replace(dst, llg_queue_back(queue));
}
void llg_queue_reduce_to(sv4_t* dst, const llg_queue_t* queue, int operation) {
    sv4_replace(dst, llg_queue_reduce(queue, operation));
}
void llg_queue_reduce_with_to(sv4_t* dst, const llg_queue_t* queue, int operation, uint32_t result_width, int8_t result_signed, int result_two_state, llg_container_eval_fn eval, void* context) {
    sv4_replace(dst, llg_queue_reduce_with(queue, operation, result_width, result_signed, result_two_state, eval, context));
}
void llg_queue_cell_read_to(sv4_t* dst, const void* cell) {
    sv4_replace(dst, llg_queue_cell_read(cell));
}
void llg_queue_ref_read_to(sv4_t* dst, const llg_queue_t* queue, uint64_t identity) {
    sv4_replace(dst, llg_queue_ref_read(queue, identity));
}
void llg_assoc_value_get_integral_to(sv4_t* dst, const llg_assoc_value_t* array, const sv4_t* key) {
    sv4_replace(dst, llg_assoc_value_get_integral(array, *key));
}
void llg_assoc_value_get_nested_integral_to(sv4_t* dst, const llg_assoc_value_t* array, const sv4_t* indices, size_t count) {
    sv4_replace(dst, llg_assoc_value_get_nested_integral(array, indices, count));
}
void llg_assoc_value_at_to(sv4_t* dst, const llg_assoc_t* array, size_t index) {
    sv4_replace(dst, llg_assoc_value_at(array, index));
}
void llg_assoc_reduce_to(sv4_t* dst, const llg_assoc_t* array, int operation) {
    sv4_replace(dst, llg_assoc_reduce(array, operation));
}
void llg_assoc_reduce_with_to(sv4_t* dst, const llg_assoc_t* array, int operation, uint32_t result_width, int8_t result_signed, int result_two_state, llg_container_eval_fn eval, void* context) {
    sv4_replace(dst, llg_assoc_reduce_with(array, operation, result_width, result_signed, result_two_state, eval, context));
}
void llg_assoc_get_integral_to(sv4_t* dst, const llg_assoc_t* array, const sv4_t* key) {
    sv4_replace(dst, llg_assoc_get_integral(array, *key));
}
void llg_assoc_get_string_to(sv4_t* dst, const llg_assoc_t* array, const void* key, size_t key_length) {
    sv4_replace(dst, llg_assoc_get_string(array, key, key_length));
}
void llg_dyn_value_get_string_to(llg_string_t* dst, const llg_dyn_value_array_t* array, const sv4_t* index) {
    llg_string_replace(dst, llg_dyn_value_get_string(array, *index));
}
void llg_dyn_value_get_nested_string_to(llg_string_t* dst, const llg_dyn_value_array_t* array, const sv4_t* indices, size_t count) {
    llg_string_replace(dst, llg_dyn_value_get_nested_string(array, indices, count));
}
void llg_queue_value_get_string_to(llg_string_t* dst, const llg_queue_value_array_t* queue, const sv4_t* index) {
    llg_string_replace(dst, llg_queue_value_get_string(queue, *index));
}
void llg_queue_value_get_nested_string_to(llg_string_t* dst, const llg_queue_value_array_t* queue, const sv4_t* indices, size_t count) {
    llg_string_replace(dst, llg_queue_value_get_nested_string(queue, indices, count));
}
void llg_assoc_value_get_integral_string_to(llg_string_t* dst, const llg_assoc_value_t* array, const sv4_t* key) {
    llg_string_replace(dst, llg_assoc_value_get_integral_string(array, *key));
}
void llg_assoc_value_get_nested_integral_string_to(llg_string_t* dst, const llg_assoc_value_t* array, const sv4_t* indices, size_t count) {
    llg_string_replace(dst, llg_assoc_value_get_nested_integral_string(array, indices, count));
}
void llg_assoc_value_get_string_to(llg_string_t* dst, const llg_assoc_value_t* array, const void* key, size_t key_length) {
    llg_string_replace(dst, llg_assoc_value_get_string(array, key, key_length));
}
void llg_assoc_value_get_string_string_to(llg_string_t* dst, const llg_assoc_value_t* array, const void* key, size_t key_length) {
    llg_string_replace(dst, llg_assoc_value_get_string_string(array, key, key_length));
}
