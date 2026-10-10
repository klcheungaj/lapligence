// llg_container.c -- scheduler-independent dynamic array, queue, and
// associative-array storage for generated C11 models.
#include "llg_container.h"
#include "llg_rng.h"

#include <limits.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

/* Outdate retained element references before storage they name is replaced
 * or removed (container/element_references.c). `position` SIZE_MAX outdates
 * every associative entry. */
static void llg_dyn_outdate_references(llg_dyn_array_t* array);
static void llg_assoc_outdate_references(llg_assoc_t* array, size_t position);

/* _Noreturn, and no statement follows an unconditional call: MSVC /O2 inlines
 * this helper, sees abort() and reports such a statement as unreachable
 * (C4702), an error in the /W4 /WX standalone probe builds. */
static _Noreturn void llg_container_fatal(const char* message) {
    fprintf(stderr, "llg container fatal: %s\n", message);
    abort();
}

static void llg_container_warning(const char* message) {
    fprintf(stderr, "llg container warning: %s\n", message);
}

/* SV 7.8.6: a read through an invalid (X/Z) key or of a nonexistent entry
 * warns and yields the default; an explicit default (SV 7.9.11) is
 * returned without a warning. */
static void llg_assoc_read_miss(int valid_key, int has_default) {
    if (!valid_key)
        llg_container_warning("invalid associative-array key read");
    else if (!has_default)
        llg_container_warning(
            "associative-array read of a nonexistent entry returns the default");
}

/* Elements stored as one identity pointer: borrowed chandles, events and
 * class-like object handles. */
static int llg_value_is_handle_kind(const llg_value_desc_t* desc) {
    return desc && (desc->kind == LLG_VALUE_CHANDLE ||
                    desc->kind == LLG_VALUE_EVENT ||
                    desc->kind == LLG_VALUE_OPAQUE ||
                    desc->kind == LLG_VALUE_PROCESS);
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
                                               result_signed, (uint8_t)result_two_state));
        sv4_replace(&result, llg_reduce_step(result, value, operation));
        sv4_replace(&result, llg_element_assign(result, result_width,
                                                result_signed, (uint8_t)result_two_state));
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
    sv4_t result = sv4_zero(width, (int8_t)is_signed);
    if (llg_sv4_width(value))
        sv4_part_select_set(&result, (int64_t)width - 1,
                            (int64_t)(width - llg_sv4_width(value)), value);
    return result;
}

sv4_t llg_stream_cast_fixed(sv4_t value, uint32_t width, int is_signed) {
    if (llg_sv4_width(value) != width)
        llg_container_fatal(
            "bit-stream cast source size does not match its fixed-size target");
    return llg_stream_to_fixed(value, width, is_signed);
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

/* Apply member-wise two-state conversion (SV 6.24.3) to every element of a
 * selected stream segment before it is unpacked into mixed-domain elements.
 * Elements occupy `element_width` bits from the segment's MSB; `runs` holds
 * `run_count` pairs of element-relative (lsb, width). */
void llg_stream_segment_two_state(sv4_t* segment, uint32_t element_width,
                                  const uint32_t* runs, size_t run_count) {
    llg_check_element_type(element_width);
    uint32_t width = llg_sv4_width(*segment);
    if (width % element_width)
        llg_container_fatal("fixed streaming segment is not a whole number of elements");
    for (uint32_t low = 0; low < width; low += element_width) {
        for (size_t i = 0; i < run_count; ++i) {
            int64_t run_low = (int64_t)low + runs[2 * i];
            int64_t run_high = run_low + runs[2 * i + 1] - 1;
            sv4_t run = sv4_part_select(*segment, run_high, run_low);
            sv4_t known = sv4_to_two_state(run);
            sv4_part_select_set(segment, run_high, run_low, known);
            sv4_destroy(&run);
            sv4_destroy(&known);
        }
    }
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
    llg_dyn_outdate_references(array);
    sv4_destroy_array(array->data, array->size);
    free(array->data);
    memset(array, 0, sizeof(*array));
}

void llg_dyn_delete(llg_dyn_array_t* array) {
    llg_dyn_outdate_references(array);
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
