
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
