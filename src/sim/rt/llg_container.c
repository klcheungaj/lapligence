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
    free(array->data);
    memset(array, 0, sizeof(*array));
}

void llg_dyn_delete(llg_dyn_array_t* array) {
    int changed = array->size != 0;
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

static void llg_value_copy(llg_value_t* target,
                           const llg_value_desc_t* target_desc,
                           const llg_value_t* source) {
    llg_value_drop(target);
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
                ? source->value.packed
                : sv4_from_u64(0, target_desc->packed_width,
                               target_desc->packed_signed);
            value = sv4_cast(value, target_desc->packed_width,
                             target_desc->packed_signed);
            target->value.packed = target_desc->packed_two_state
                ? sv4_to_two_state(value)
                : value;
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

void llg_dyn_value_new(llg_dyn_value_array_t* dst, sv4_t requested_size,
                       const llg_dyn_value_array_t* initializer) {
    size_t size = llg_checked_count(llg_dynamic_size(requested_size),
                                    sizeof(llg_value_t));
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

void llg_dyn_value_copy(llg_dyn_value_array_t* dst,
                        const llg_dyn_value_array_t* src) {
    if (dst == src) return;
    llg_dyn_value_new(dst, sv4_from_u64((uint64_t)src->size, 64, 0), src);
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
        return value->value.packed;
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
        assigned = sv4_to_two_state(assigned);
    if (sv4_same(target->value.packed, assigned)) return 1;
    target->value.packed = assigned;
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
            data[i].value.packed = dst->element->packed_two_state
                ? sv4_to_two_state(value)
                : value;
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

static void llg_queue_value_append(llg_queue_value_array_t* queue,
                                   const llg_value_t* source) {
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
    llg_notify(queue->notify, queue->contents_dependency,
               queue->shape_dependency,
               LLG_CONTAINER_CHANGED_CONTENTS | LLG_CONTAINER_CHANGED_SHAPE);
}

static void llg_queue_value_prepend(llg_queue_value_array_t* queue,
                                    const llg_value_t* source) {
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
    llg_notify(queue->notify, queue->contents_dependency,
               queue->shape_dependency,
               LLG_CONTAINER_CHANGED_CONTENTS |
                   (old_size != new_size ? LLG_CONTAINER_CHANGED_SHAPE : 0));
}

void llg_queue_value_push_front(llg_queue_value_array_t* queue, sv4_t value) {
    llg_value_t source = llg_value_from_packed(queue->element, value);
    llg_queue_value_prepend(queue, &source);
    llg_value_drop(&source);
}

void llg_queue_value_push_back(llg_queue_value_array_t* queue, sv4_t value) {
    llg_value_t source = llg_value_from_packed(queue->element, value);
    llg_queue_value_append(queue, &source);
    llg_value_drop(&source);
}

void llg_queue_value_push_front_real(llg_queue_value_array_t* queue, double value) {
    llg_value_t source = llg_value_from_real(queue->element, value);
    llg_queue_value_prepend(queue, &source);
    llg_value_drop(&source);
}

void llg_queue_value_push_back_real(llg_queue_value_array_t* queue, double value) {
    llg_value_t source = llg_value_from_real(queue->element, value);
    llg_queue_value_append(queue, &source);
    llg_value_drop(&source);
}

void llg_queue_value_push_front_string(llg_queue_value_array_t* queue,
                                       llg_string_t value) {
    llg_value_t source = llg_value_from_string(queue->element, value);
    llg_queue_value_prepend(queue, &source);
    llg_value_drop(&source);
}

void llg_queue_value_push_back_string(llg_queue_value_array_t* queue,
                                      llg_string_t value) {
    llg_value_t source = llg_value_from_string(queue->element, value);
    llg_queue_value_append(queue, &source);
    llg_value_drop(&source);
}

void llg_queue_value_push_front_chandle(llg_queue_value_array_t* queue,
                                        void* value) {
    llg_value_t source = llg_value_from_chandle(queue->element, value);
    llg_queue_value_prepend(queue, &source);
    llg_value_drop(&source);
}

void llg_queue_value_push_back_chandle(llg_queue_value_array_t* queue,
                                       void* value) {
    llg_value_t source = llg_value_from_chandle(queue->element, value);
    llg_queue_value_append(queue, &source);
    llg_value_drop(&source);
}

void llg_queue_value_push_front_container(
    llg_queue_value_array_t* queue, const llg_dyn_value_array_t* source) {
    llg_value_t value = llg_value_from_container(queue->element, source);
    llg_queue_value_prepend(queue, &value);
    llg_value_drop(&value);
}

void llg_queue_value_push_back_container(
    llg_queue_value_array_t* queue, const llg_dyn_value_array_t* source) {
    llg_value_t value = llg_value_from_container(queue->element, source);
    llg_queue_value_append(queue, &value);
    llg_value_drop(&value);
}

void llg_queue_value_push_front_container_from_packed(
    llg_queue_value_array_t* queue, const llg_dyn_array_t* source) {
    llg_value_t value = llg_value_from_packed_container(queue->element, source);
    llg_queue_value_prepend(queue, &value);
    llg_value_drop(&value);
}

void llg_queue_value_push_back_container_from_packed(
    llg_queue_value_array_t* queue, const llg_dyn_array_t* source) {
    llg_value_t value = llg_value_from_packed_container(queue->element, source);
    llg_queue_value_append(queue, &value);
    llg_value_drop(&value);
}

static int llg_queue_value_insert_source(llg_queue_value_array_t* queue,
                                         sv4_t index,
                                         const llg_value_t* source) {
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
    llg_notify(queue->notify, queue->contents_dependency,
               queue->shape_dependency,
               LLG_CONTAINER_CHANGED_CONTENTS |
                   (old_size != new_size ? LLG_CONTAINER_CHANGED_SHAPE : 0));
    return 1;
}

int llg_queue_value_insert(llg_queue_value_array_t* queue, sv4_t index,
                           sv4_t value) {
    llg_value_t source = llg_value_from_packed(queue->element, value);
    int result = llg_queue_value_insert_source(queue, index, &source);
    llg_value_drop(&source);
    return result;
}

int llg_queue_value_insert_real(llg_queue_value_array_t* queue, sv4_t index,
                                double value) {
    llg_value_t source = llg_value_from_real(queue->element, value);
    int result = llg_queue_value_insert_source(queue, index, &source);
    llg_value_drop(&source);
    return result;
}

int llg_queue_value_insert_string(llg_queue_value_array_t* queue, sv4_t index,
                                  llg_string_t value) {
    llg_value_t source = llg_value_from_string(queue->element, value);
    int result = llg_queue_value_insert_source(queue, index, &source);
    llg_value_drop(&source);
    return result;
}

int llg_queue_value_insert_chandle(llg_queue_value_array_t* queue,
                                   sv4_t index, void* value) {
    llg_value_t source = llg_value_from_chandle(queue->element, value);
    int result = llg_queue_value_insert_source(queue, index, &source);
    llg_value_drop(&source);
    return result;
}

int llg_queue_value_insert_container(llg_queue_value_array_t* queue,
                                     sv4_t index,
                                     const llg_dyn_value_array_t* source) {
    llg_value_t value = llg_value_from_container(queue->element, source);
    int result = llg_queue_value_insert_source(queue, index, &value);
    llg_value_drop(&value);
    return result;
}

int llg_queue_value_insert_container_from_packed(
    llg_queue_value_array_t* queue, sv4_t index, const llg_dyn_array_t* source) {
    llg_value_t value = llg_value_from_packed_container(queue->element, source);
    int result = llg_queue_value_insert_source(queue, index, &value);
    llg_value_drop(&value);
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
    llg_queue_value_invalidate_refs(queue);
    llg_notify(queue->notify, queue->contents_dependency,
               queue->shape_dependency,
               LLG_CONTAINER_CHANGED_CONTENTS | LLG_CONTAINER_CHANGED_SHAPE);
    return 1;
}

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
    for (size_t i = copied; i < size; ++i) data[i] = initial;
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
    free(dst->data);
    dst->data = data;
    dst->size = size;
    llg_notify(notify, contents_dependency, shape_dependency,
               (contents_changed ? LLG_CONTAINER_CHANGED_CONTENTS : 0) |
                   (shape_changed ? LLG_CONTAINER_CHANGED_SHAPE : 0));
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
    return array->data[native];
}

int llg_dyn_set(llg_dyn_array_t* array, sv4_t index, sv4_t value) {
    size_t native;
    if (!llg_index(index, array->size, 0, &native)) return 0;
    sv4_t assigned = llg_element_assign(
        value, array->element_width, array->element_signed,
        array->element_two_state);
    if (sv4_same(array->data[native], assigned)) return 1;
    array->data[native] = assigned;
    llg_notify(array->notify, array->contents_dependency,
               array->shape_dependency, LLG_CONTAINER_CHANGED_CONTENTS);
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
    queue->element_ids = llg_realloc_items(
        queue->element_ids, capacity, sizeof(*queue->element_ids));
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
    queue->next_element_id = 1;
    queue->limit = maximum_elements == UINT64_MAX
                       ? SIZE_MAX
                       : llg_checked_count(maximum_elements, sizeof(sv4_t));
}

void llg_queue_destroy(llg_queue_t* queue) {
    free(queue->data);
    free(queue->element_ids);
    memset(queue, 0, sizeof(*queue));
}

void llg_queue_delete(llg_queue_t* queue) {
    int changed = queue->size != 0;
    queue->size = 0;
    llg_notify(queue->notify, queue->contents_dependency,
               queue->shape_dependency,
               changed ? LLG_CONTAINER_CHANGED_CONTENTS |
                            LLG_CONTAINER_CHANGED_SHAPE
                       : 0);
}

void llg_queue_copy(llg_queue_t* dst, const llg_queue_t* src) {
    if (dst == src) return;
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
    return queue->data[index];
}

int llg_queue_ref_write(llg_queue_t* queue, uint64_t identity, sv4_t value) {
    size_t index = llg_queue_ref_index(queue, identity);
    if (index == SIZE_MAX) return 0;
    sv4_t assigned = llg_element_assign(value, queue->element_width,
                                        queue->element_signed,
                                        queue->element_two_state);
    if (sv4_same(queue->data[index], assigned)) return 1;
    queue->data[index] = assigned;
    llg_notify(queue->notify, queue->contents_dependency,
               queue->shape_dependency, LLG_CONTAINER_CHANGED_CONTENTS);
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
    if (key_width > LLG_MAX_WIDTH)
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
    if (sv4_is_unknown(input)) return 0;
    if (array->key_width) {
        *output = sv4_cast(input, array->key_width, array->key_signed);
        if (array->key_two_state) *output = sv4_to_two_state(*output);
    } else {
        *output = sv4_cast(input, LLG_MAX_WIDTH, input.is_signed);
        output->is_signed = 0;
    }
    return !sv4_is_unknown(*output);
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
                                      const llg_value_t* source) {
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
            ? *integral_key : (sv4_t){0};
        array->entries[position].string_key = key_copy;
        array->entries[position].string_length = string_length;
        ++array->size;
        llg_value_copy(&array->entries[position].value, array->element, source);
    } else if (contents_changed) {
        llg_value_copy(&array->entries[position].value, array->element, source);
    }
    if (shape_changed) llg_assoc_value_invalidate_refs(array);
    llg_notify(array->notify, array->contents_dependency,
               array->shape_dependency,
               (contents_changed ? LLG_CONTAINER_CHANGED_CONTENTS : 0) |
                   (shape_changed ? LLG_CONTAINER_CHANGED_SHAPE : 0));
    return 1;
}

static llg_value_t llg_assoc_value_packed_source(
    const llg_assoc_value_t* array, sv4_t value) {
    return llg_value_from_packed(array->element, value);
}

sv4_t llg_assoc_value_get_integral(const llg_assoc_value_t* array, sv4_t key) {
    sv4_t normalized;
    int found = 0;
    if (llg_assoc_value_normalize_key(array, key, &normalized)) {
        size_t position = llg_assoc_value_integral_position(array, normalized,
                                                             &found);
        if (found && array->entries[position].value.desc->kind == LLG_VALUE_PACKED)
            return array->entries[position].value.value.packed;
    }
    return array->default_value.desc &&
                   array->default_value.desc->kind == LLG_VALUE_PACKED
        ? array->default_value.value.packed
        : sv4_from_u64(0, 1, 0);
}

double llg_assoc_value_get_integral_real(const llg_assoc_value_t* array,
                                         sv4_t key) {
    sv4_t normalized;
    int found = 0;
    if (llg_assoc_value_normalize_key(array, key, &normalized)) {
        size_t position = llg_assoc_value_integral_position(array, normalized,
                                                             &found);
        if (found && array->entries[position].value.desc->kind == LLG_VALUE_REAL)
            return array->entries[position].value.value.real;
    }
    return array->default_value.desc &&
                   array->default_value.desc->kind == LLG_VALUE_REAL
        ? array->default_value.value.real
        : 0.0;
}

llg_string_t llg_assoc_value_get_integral_string(
    const llg_assoc_value_t* array, sv4_t key) {
    sv4_t normalized;
    int found = 0;
    if (llg_assoc_value_normalize_key(array, key, &normalized)) {
        size_t position = llg_assoc_value_integral_position(array, normalized,
                                                             &found);
        if (found && array->entries[position].value.desc->kind == LLG_VALUE_STRING)
            return llg_string_clone(&array->entries[position].value.value.string);
    }
    return array->default_value.desc &&
                   array->default_value.desc->kind == LLG_VALUE_STRING
        ? llg_string_clone(&array->default_value.value.string)
        : (llg_string_t){0};
}

void* llg_assoc_value_get_integral_chandle(
    const llg_assoc_value_t* array, sv4_t key) {
    sv4_t normalized;
    int found = 0;
    if (llg_assoc_value_normalize_key(array, key, &normalized)) {
        size_t position = llg_assoc_value_integral_position(array, normalized,
                                                             &found);
        if (found && (array->entries[position].value.desc->kind == LLG_VALUE_CHANDLE ||
                      array->entries[position].value.desc->kind == LLG_VALUE_EVENT))
            return array->entries[position].value.value.handle;
    }
    return array->default_value.desc &&
                   (array->default_value.desc->kind == LLG_VALUE_CHANDLE ||
                    array->default_value.desc->kind == LLG_VALUE_EVENT)
        ? array->default_value.value.handle
        : NULL;
}

static llg_value_t* llg_assoc_value_nested_at_integral(
    const llg_assoc_value_t* array, const sv4_t* indices, size_t count) {
    if (!indices || count < 2 || array->key_kind != LLG_ASSOC_INTEGRAL)
        return NULL;
    sv4_t normalized;
    int found = 0;
    if (!llg_assoc_value_normalize_key(array, indices[0], &normalized))
        return NULL;
    size_t position = llg_assoc_value_integral_position(array, normalized,
                                                         &found);
    if (!found) return NULL;
    llg_value_t* value = &array->entries[position].value;
    for (size_t index = 1; value && index < count; ++index) {
        if (!value->desc || value->desc->kind != LLG_VALUE_CONTAINER ||
            !value->value.container)
            return NULL;
        value = llg_dyn_value_at(value->value.container, indices[index]);
    }
    return value;
}

sv4_t llg_assoc_value_get_nested_integral(
    const llg_assoc_value_t* array, const sv4_t* indices, size_t count) {
    llg_value_t* value = llg_assoc_value_nested_at_integral(array, indices, count);
    return value && value->desc->kind == LLG_VALUE_PACKED
        ? value->value.packed
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
    sv4_t normalized;
    if (!llg_assoc_value_normalize_key(array, key, &normalized)) return 0;
    llg_value_t source = llg_assoc_value_packed_source(array, value);
    int result = llg_assoc_value_set_source(array, &normalized, NULL, 0, &source);
    llg_value_drop(&source);
    return result;
}

int llg_assoc_value_set_integral_real(llg_assoc_value_t* array, sv4_t key,
                                      double value) {
    sv4_t normalized;
    if (!llg_assoc_value_normalize_key(array, key, &normalized)) return 0;
    llg_value_t source = llg_value_from_real(array->element, value);
    int result = llg_assoc_value_set_source(array, &normalized, NULL, 0, &source);
    llg_value_drop(&source);
    return result;
}

int llg_assoc_value_set_integral_string(llg_assoc_value_t* array, sv4_t key,
                                        llg_string_t value) {
    sv4_t normalized;
    if (!llg_assoc_value_normalize_key(array, key, &normalized)) {
        llg_string_destroy(&value);
        return 0;
    }
    llg_value_t source = llg_value_from_string(array->element, value);
    int result = llg_assoc_value_set_source(array, &normalized, NULL, 0, &source);
    llg_value_drop(&source);
    return result;
}

int llg_assoc_value_set_integral_chandle(llg_assoc_value_t* array, sv4_t key,
                                         void* value) {
    sv4_t normalized;
    if (!llg_assoc_value_normalize_key(array, key, &normalized)) return 0;
    llg_value_t source = llg_value_from_chandle(array->element, value);
    return llg_assoc_value_set_source(array, &normalized, NULL, 0, &source);
}

int llg_assoc_value_set_nested_integral_container(
    llg_assoc_value_t* array, const sv4_t* indices, size_t count,
    const llg_dyn_value_array_t* source) {
    sv4_t normalized;
    if (!indices || count == 0 ||
        !llg_assoc_value_normalize_key(array, indices[0], &normalized))
        return 0;
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
        return 0;
    llg_value_t value = llg_value_from_container(target_desc, source);
    int result;
    if (count == 1) {
        result = llg_assoc_value_set_source(array, &normalized, NULL, 0, &value);
    } else {
        result = 0;
        if (target && target->desc->kind == LLG_VALUE_CONTAINER) {
            result = 1;
            if (!llg_value_equal(target, &value)) {
                llg_value_copy(target, target->desc, &value);
                llg_notify(array->notify, array->contents_dependency,
                           array->shape_dependency,
                           LLG_CONTAINER_CHANGED_CONTENTS);
            }
        }
    }
    llg_value_drop(&value);
    return result;
}

int llg_assoc_value_set_nested_integral_container_from_packed(
    llg_assoc_value_t* array, const sv4_t* indices, size_t count,
    const llg_dyn_array_t* source) {
    if (!indices || count == 0 || !source) return 0;
    sv4_t normalized;
    if (!llg_assoc_value_normalize_key(array, indices[0], &normalized)) return 0;

    const llg_value_desc_t* target_desc = NULL;
    if (count == 1) {
        target_desc = array->element;
    } else {
        llg_value_t* target = llg_assoc_value_nested_at_integral(array, indices,
                                                                  count);
        if (!target || target->desc->kind != LLG_VALUE_CONTAINER)
            return 0;
        target_desc = target->desc;
    }
    if (!target_desc || target_desc->kind != LLG_VALUE_CONTAINER ||
        !target_desc->element ||
        (target_desc->element->kind != LLG_VALUE_PACKED &&
         target_desc->element->kind != LLG_VALUE_REAL))
        return 0;

    llg_value_t value = llg_value_from_packed_container(target_desc, source);
    int result;
    if (count == 1) {
        result = llg_assoc_value_set_source(array, &normalized, NULL, 0, &value);
    } else {
        result = 0;
        llg_value_t* target = llg_assoc_value_nested_at_integral(array, indices,
                                                                  count);
        if (target && target->desc->kind == LLG_VALUE_CONTAINER) {
            result = 1;
            if (!llg_value_equal(target, &value)) {
                llg_value_copy(target, target->desc, &value);
                llg_notify(array->notify, array->contents_dependency,
                           array->shape_dependency,
                           LLG_CONTAINER_CHANGED_CONTENTS);
            }
        }
    }
    llg_value_drop(&value);
    return result;
}

int llg_assoc_value_set_nested_integral(
    llg_assoc_value_t* array, const sv4_t* indices, size_t count,
    sv4_t value) {
    llg_value_t* target = llg_assoc_value_nested_at_integral(array, indices, count);
    if (!target || target->desc->kind != LLG_VALUE_PACKED) return 0;
    sv4_t assigned = sv4_cast(value, target->desc->packed_width,
                              target->desc->packed_signed);
    if (target->desc->packed_two_state) assigned = sv4_to_two_state(assigned);
    if (sv4_same(target->value.packed, assigned)) return 1;
    target->value.packed = assigned;
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
    sv4_t normalized;
    int found = 0;
    if (!llg_assoc_value_normalize_key(array, key, &normalized)) return 0;
    (void)llg_assoc_value_integral_position(array, normalized, &found);
    return found;
}

int llg_assoc_value_delete_integral(llg_assoc_value_t* array, sv4_t key) {
    sv4_t normalized;
    int found = 0;
    if (!llg_assoc_value_normalize_key(array, key, &normalized)) return 0;
    size_t position = llg_assoc_value_integral_position(array, normalized, &found);
    if (!found) return 0;
    llg_value_drop(&array->entries[position].value);
    if (position + 1 < array->size)
        memmove(array->entries + position, array->entries + position + 1,
                (array->size - position - 1) * sizeof(*array->entries));
    --array->size;
    llg_assoc_value_invalidate_refs(array);
    llg_notify(array->notify, array->contents_dependency,
               array->shape_dependency,
               LLG_CONTAINER_CHANGED_CONTENTS | LLG_CONTAINER_CHANGED_SHAPE);
    return 1;
}

static void llg_assoc_value_set_default_source(llg_assoc_value_t* array,
                                               const llg_value_t* source) {
    int changed = !array->has_default_value ||
                  !llg_value_equal(&array->default_value, source);
    if (changed)
        llg_value_copy(&array->default_value, array->element, source);
    array->has_default_value = 1;
    llg_notify(array->notify, array->contents_dependency,
               array->shape_dependency,
               changed ? LLG_CONTAINER_CHANGED_CONTENTS : 0);
}

void llg_assoc_value_set_default(llg_assoc_value_t* array, sv4_t value) {
    llg_value_t source = llg_assoc_value_packed_source(array, value);
    llg_assoc_value_set_default_source(array, &source);
    llg_value_drop(&source);
}

void llg_assoc_value_set_default_real(llg_assoc_value_t* array, double value) {
    llg_value_t source = llg_value_from_real(array->element, value);
    llg_assoc_value_set_default_source(array, &source);
    llg_value_drop(&source);
}

void llg_assoc_value_set_default_string(llg_assoc_value_t* array,
                                        llg_string_t value) {
    llg_value_t source = llg_value_from_string(array->element, value);
    llg_assoc_value_set_default_source(array, &source);
    llg_value_drop(&source);
}

void llg_assoc_value_set_default_chandle(llg_assoc_value_t* array, void* value) {
    llg_value_t source = llg_value_from_chandle(array->element, value);
    llg_assoc_value_set_default_source(array, &source);
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
    llg_assoc_value_check_kind(array, LLG_ASSOC_INTEGRAL);
    if (!array->key_width)
        llg_container_fatal("wildcard associative-array traversal is illegal");
    if (!array->size) return 0;
    if (endpoint) {
        *key = array->entries[direction > 0 ? 0 : array->size - 1].integral_key;
        return 1;
    }
    sv4_t normalized;
    if (!llg_assoc_value_normalize_key(array, *key, &normalized)) return 0;
    int found;
    size_t position = llg_assoc_value_integral_position(array, normalized, &found);
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
    llg_assoc_value_check_string_key(array, key, key_length);
    llg_value_t source = llg_assoc_value_packed_source(array, value);
    int result = llg_assoc_value_set_source(array, NULL, key, key_length, &source);
    llg_value_drop(&source);
    return result;
}

int llg_assoc_value_set_string_real(llg_assoc_value_t* array, const void* key,
                                    size_t key_length, double value) {
    llg_assoc_value_check_string_key(array, key, key_length);
    llg_value_t source = llg_value_from_real(array->element, value);
    int result = llg_assoc_value_set_source(array, NULL, key, key_length, &source);
    llg_value_drop(&source);
    return result;
}

int llg_assoc_value_set_string_string(llg_assoc_value_t* array, const void* key,
                                      size_t key_length, llg_string_t value) {
    llg_assoc_value_check_string_key(array, key, key_length);
    llg_value_t source = llg_value_from_string(array->element, value);
    int result = llg_assoc_value_set_source(array, NULL, key, key_length, &source);
    llg_value_drop(&source);
    return result;
}

int llg_assoc_value_set_string_chandle(llg_assoc_value_t* array, const void* key,
                                       size_t key_length, void* value) {
    llg_assoc_value_check_string_key(array, key, key_length);
    llg_value_t source = llg_value_from_chandle(array->element, value);
    return llg_assoc_value_set_source(array, NULL, key, key_length, &source);
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
    llg_value_drop(&array->entries[position].value);
    if (position + 1 < array->size)
        memmove(array->entries + position, array->entries + position + 1,
                (array->size - position - 1) * sizeof(*array->entries));
    --array->size;
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
        entries[i].integral_key = src->entries[i].integral_key;
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
