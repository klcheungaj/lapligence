/* Kind-specific identity operations installed by the scheduler. The container
 * runtime is scheduler-independent: without hooks a new event element is null and
 * process references are not counted. */
static llg_value_handle_hooks_t llg_value_hooks;

void llg_value_set_handle_hooks(const llg_value_handle_hooks_t* hooks) {
    if (hooks) llg_value_hooks = *hooks;
    else memset(&llg_value_hooks, 0, sizeof(llg_value_hooks));
}

static void llg_value_handle_retain(const llg_value_desc_t* desc, void* handle) {
    if (handle && desc->kind == LLG_VALUE_PROCESS && llg_value_hooks.retain)
        llg_value_hooks.retain(handle);
}

static void llg_value_handle_release(const llg_value_desc_t* desc, void* handle) {
    if (handle && desc->kind == LLG_VALUE_PROCESS && llg_value_hooks.release)
        llg_value_hooks.release(handle);
}

/* Replace the identity held by handle value `target`, keeping process
 * reference counts balanced. Returns 0 when the identity is unchanged. */
static int llg_value_store_handle(llg_value_t* target, void* handle) {
    if (target->value.handle == handle) return 0;
    llg_value_handle_retain(target->desc, handle);
    llg_value_handle_release(target->desc, target->value.handle);
    target->value.handle = handle;
    return 1;
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
        case LLG_VALUE_PROCESS:
            llg_value_handle_release(desc, value->value.handle);
            break;
        default:
            break;
    }
    value->desc = NULL;
    memset(&value->value, 0, sizeof(value->value));
}

/* Descriptor item arrays use a checked, non-fatal allocation so that one
 * construction can be abandoned before it publishes anything. Probes may
 * define LLG_VALUE_ITEMS_MALLOC to inject an allocation failure. */
#ifndef LLG_VALUE_ITEMS_MALLOC
#define LLG_VALUE_ITEMS_MALLOC malloc
#endif

static void* llg_value_try_items(size_t count, size_t item_size) {
    if (count == 0 || count > SIZE_MAX / item_size) return NULL;
    void* items = LLG_VALUE_ITEMS_MALLOC(count * item_size);
    if (items) memset(items, 0, count * item_size);
    return items;
}

/* Construct the default value of `desc` into an empty `value`. On failure the
 * partially built value is released, `value` stays empty and 0 is returned.
 * `initial` selects the Table 6-7 initial value of a newly created element,
 * where an event refers to a new synchronization object; otherwise every
 * handle is null, the Table 7-1 value read from a missing element. */
static int llg_value_try_default_mode(llg_value_t* value,
                                      const llg_value_desc_t* desc,
                                      int initial) {
    memset(&value->value, 0, sizeof(value->value));
    value->desc = desc;
    switch (desc->kind) {
        case LLG_VALUE_PACKED:
            value->value.packed = desc->packed_two_state
                ? sv4_from_u64(0, desc->packed_width, desc->packed_signed)
                : sv4_x(desc->packed_width, desc->packed_signed);
            return 1;
        case LLG_VALUE_REAL:
            value->value.real = 0.0;
            return 1;
        case LLG_VALUE_STRING:
            value->value.string = (llg_string_t){0};
            return 1;
        case LLG_VALUE_AGGREGATE:
        case LLG_VALUE_FIXED_ARRAY:
            if (desc->item_count) {
                value->value.items = llg_value_try_items(
                    desc->item_count, sizeof(*value->value.items));
                if (!value->value.items) {
                    value->desc = NULL;
                    return 0;
                }
                for (size_t i = 0; i < desc->item_count; ++i) {
                    if (!llg_value_try_default_mode(&value->value.items[i],
                                                    llg_value_item_desc(desc, i),
                                                    initial)) {
                        llg_value_drop(value);
                        return 0;
                    }
                }
            }
            return 1;
        case LLG_VALUE_CONTAINER:
            /* A nested dynamic array has the standard null-handle default. */
            value->value.container = NULL;
            return 1;
        case LLG_VALUE_EVENT:
            value->value.handle = initial && llg_value_hooks.event_new
                ? llg_value_hooks.event_new()
                : NULL;
            return 1;
        case LLG_VALUE_CHANDLE:
        case LLG_VALUE_OPAQUE:
        case LLG_VALUE_PROCESS:
            value->value.handle = NULL;
            return 1;
        default:
            llg_container_fatal("invalid recursive container value kind");
            return 0;
    }
}

static int llg_value_try_default(llg_value_t* value,
                                 const llg_value_desc_t* desc) {
    return llg_value_try_default_mode(value, desc, 0);
}

static void llg_value_default(llg_value_t* value,
                              const llg_value_desc_t* desc) {
    llg_value_drop(value);
    if (!llg_value_try_default(value, desc))
        llg_container_fatal("container allocation failed");
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
        case LLG_VALUE_PROCESS:
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

/* Construct a converted deep copy of `source` into an empty `target`. Value
 * leaves are cloned, identity handles are shared and borrowed chandles are
 * copied without taking ownership. On failure nothing is published: the
 * partial copy is released, `target` stays empty and 0 is returned. */
static int llg_value_try_construct_copy(llg_value_t* target,
                                        const llg_value_desc_t* target_desc,
                                        const llg_value_t* source) {
    memset(&target->value, 0, sizeof(target->value));
    target->desc = NULL;
    const llg_value_desc_t* source_desc = source ? source->desc : NULL;
    if (!source || !source_desc)
        return llg_value_try_default_mode(target, target_desc, 1);
    target->desc = target_desc;
    switch (target_desc->kind) {
        case LLG_VALUE_PACKED: {
            sv4_t value = source_desc->kind == LLG_VALUE_PACKED
                ? sv4_cast(source->value.packed, target_desc->packed_width,
                           target_desc->packed_signed)
                : sv4_zero(target_desc->packed_width, target_desc->packed_signed);
            if (target_desc->packed_two_state)
                sv4_replace(&value, sv4_to_two_state(value));
            sv4_move(&target->value.packed, &value);
            return 1;
        }
        case LLG_VALUE_REAL:
            target->value.real = llg_value_real_convert(
                target_desc, source_desc->kind == LLG_VALUE_REAL
                ? source->value.real
                : sv4_to_real(source->value.packed));
            return 1;
        case LLG_VALUE_STRING:
            target->value.string = source_desc->kind == LLG_VALUE_STRING
                ? llg_string_clone(&source->value.string)
                : (llg_string_t){0};
            return 1;
        case LLG_VALUE_CHANDLE:
        case LLG_VALUE_EVENT:
        case LLG_VALUE_OPAQUE:
        case LLG_VALUE_PROCESS:
            target->value.handle = source->value.handle;
            llg_value_handle_retain(target_desc, target->value.handle);
            return 1;
        case LLG_VALUE_AGGREGATE:
        case LLG_VALUE_FIXED_ARRAY:
            if (target_desc->item_count) {
                target->value.items = llg_value_try_items(
                    target_desc->item_count, sizeof(*target->value.items));
                if (!target->value.items) {
                    target->desc = NULL;
                    return 0;
                }
                for (size_t i = 0; i < target_desc->item_count; ++i) {
                    const llg_value_desc_t* item =
                        llg_value_item_desc(target_desc, i);
                    const llg_value_t* source_item =
                        source->value.items && i < source_desc->item_count
                            ? &source->value.items[i]
                            : NULL;
                    if (!llg_value_try_construct_copy(&target->value.items[i],
                                                      item, source_item)) {
                        llg_value_drop(target);
                        return 0;
                    }
                }
            }
            return 1;
        case LLG_VALUE_CONTAINER:
            if (source_desc->kind == LLG_VALUE_CONTAINER &&
                source->value.container) {
                target->value.container = llg_value_try_items(
                    1, sizeof(*target->value.container));
                if (!target->value.container) {
                    target->desc = NULL;
                    return 0;
                }
                llg_dyn_value_init(target->value.container,
                                   target_desc->element);
                llg_dyn_value_copy(target->value.container,
                                   source->value.container);
            }
            return 1;
        default:
            llg_container_fatal("invalid recursive container value kind");
            return 0;
    }
}

/* Replace `target` with a converted deep copy of `source`. The replacement is
 * complete before the old value is dropped, so `source` may alias `target` or
 * one of its descendants and a failed construction leaves `target` intact. */
static int llg_value_try_copy(llg_value_t* target,
                              const llg_value_desc_t* target_desc,
                              const llg_value_t* source) {
    llg_value_t replacement = {0};
    if (!llg_value_try_construct_copy(&replacement, target_desc, source))
        return 0;
    llg_value_drop(target);
    *target = replacement; // exclusive ownership transfer, not a copy
    return 1;
}

static void llg_value_copy(llg_value_t* target,
                           const llg_value_desc_t* target_desc,
                           const llg_value_t* source) {
    if (!llg_value_try_copy(target, target_desc, source))
        llg_container_fatal("container allocation failed");
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
        case LLG_VALUE_PROCESS:
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

/* ---- SystemVerilog equality of recursive values (SIM-007) -------------- */

/* Fold one item result into an aggregate result: a known mismatch dominates,
 * then an unknown item (SV 7.2.2, 7.6, 11.4.5). */
static int llg_value_equality_fold(int result, int item) {
    if (result == LLG_VALUE_UNEQUAL || item == LLG_VALUE_UNEQUAL)
        return LLG_VALUE_UNEQUAL;
    return item == LLG_VALUE_EQUALITY_UNKNOWN ? item : result;
}

static int llg_packed_equality(sv4_t a, sv4_t b, int case_equality) {
    if (case_equality)
        return sv4_same(a, b) ? LLG_VALUE_EQUAL : LLG_VALUE_UNEQUAL;
    sv4_t equal = sv4_eq(a, b);
    int result = sv4_is_unknown(equal)
        ? LLG_VALUE_EQUALITY_UNKNOWN
        : (sv4_to_bool(equal) ? LLG_VALUE_EQUAL : LLG_VALUE_UNEQUAL);
    sv4_destroy(&equal);
    return result;
}

static int llg_value_items_equality(const llg_value_t* a, size_t a_count,
                                    const llg_value_t* b, size_t b_count,
                                    int case_equality) {
    if (a_count != b_count) return LLG_VALUE_UNEQUAL;
    int result = LLG_VALUE_EQUAL;
    for (size_t i = 0; i < a_count && result != LLG_VALUE_UNEQUAL; ++i)
        result = llg_value_equality_fold(
            result, llg_value_equality(&a[i], &b[i], case_equality));
    return result;
}

int llg_value_equality(const llg_value_t* a, const llg_value_t* b,
                       int case_equality) {
    if (!a || !b || !a->desc || !b->desc ||
        !llg_value_desc_compatible(a->desc, b->desc))
        return LLG_VALUE_UNEQUAL;
    switch (a->desc->kind) {
        case LLG_VALUE_PACKED:
            return llg_packed_equality(a->value.packed, b->value.packed,
                                       case_equality);
        case LLG_VALUE_REAL:
            /* Numeric comparison: 0.0 equals -0.0 and NaN equals nothing. */
            return a->value.real == b->value.real ? LLG_VALUE_EQUAL
                                                  : LLG_VALUE_UNEQUAL;
        case LLG_VALUE_STRING:
            return a->value.string.len == b->value.string.len &&
                           (!a->value.string.len ||
                            memcmp(a->value.string.data, b->value.string.data,
                                   a->value.string.len) == 0)
                       ? LLG_VALUE_EQUAL
                       : LLG_VALUE_UNEQUAL;
        case LLG_VALUE_CHANDLE:
        case LLG_VALUE_EVENT:
        case LLG_VALUE_OPAQUE:
        case LLG_VALUE_PROCESS:
            return a->value.handle == b->value.handle ? LLG_VALUE_EQUAL
                                                      : LLG_VALUE_UNEQUAL;
        case LLG_VALUE_AGGREGATE:
        case LLG_VALUE_FIXED_ARRAY:
            if (!a->value.items || !b->value.items)
                return a->value.items == b->value.items ? LLG_VALUE_EQUAL
                                                        : LLG_VALUE_UNEQUAL;
            return llg_value_items_equality(a->value.items, a->desc->item_count,
                                            b->value.items, b->desc->item_count,
                                            case_equality);
        case LLG_VALUE_CONTAINER: {
            /* A null nested container is the empty default. */
            const llg_dyn_value_array_t* left = a->value.container;
            const llg_dyn_value_array_t* right = b->value.container;
            return llg_value_items_equality(
                left ? left->data : NULL, left ? left->size : 0,
                right ? right->data : NULL, right ? right->size : 0,
                case_equality);
        }
        default:
            return LLG_VALUE_UNEQUAL;
    }
}

static sv4_t llg_equality_result(int equality, int negate) {
    if (equality == LLG_VALUE_EQUALITY_UNKNOWN) return sv4_x(1, 0);
    return sv4_from_u64((equality == LLG_VALUE_EQUAL) != (negate != 0), 1, 0);
}

sv4_t llg_dyn_value_equal(const llg_dyn_value_array_t* a,
                          const llg_dyn_value_array_t* b, int case_equality,
                          int negate) {
    return llg_equality_result(
        llg_value_items_equality(a->data, a->size, b->data, b->size,
                                 case_equality),
        negate);
}

sv4_t llg_queue_value_equal(const llg_queue_value_array_t* a,
                            const llg_queue_value_array_t* b,
                            int case_equality, int negate) {
    return llg_equality_result(
        llg_value_items_equality(a->data, a->size, b->data, b->size,
                                 case_equality),
        negate);
}

static int llg_packed_items_equality(const sv4_t* a, size_t a_count,
                                     const sv4_t* b, size_t b_count,
                                     int case_equality) {
    if (a_count != b_count) return LLG_VALUE_UNEQUAL;
    int result = LLG_VALUE_EQUAL;
    for (size_t i = 0; i < a_count && result != LLG_VALUE_UNEQUAL; ++i)
        result = llg_value_equality_fold(
            result, llg_packed_equality(a[i], b[i], case_equality));
    return result;
}

sv4_t llg_dyn_equal(const llg_dyn_array_t* a, const llg_dyn_array_t* b,
                    int case_equality, int negate) {
    return llg_equality_result(
        llg_packed_items_equality(a->data, a->size, b->data, b->size,
                                  case_equality),
        negate);
}

sv4_t llg_queue_equal(const llg_queue_t* a, const llg_queue_t* b,
                      int case_equality, int negate) {
    return llg_equality_result(
        llg_packed_items_equality(a->data, a->size, b->data, b->size,
                                  case_equality),
        negate);
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

/* ---- Descriptor contract (SIM-003) ------------------------------------- */

int llg_value_desc_copy_policy(const llg_value_desc_t* desc) {
    switch (desc ? desc->kind : 0xffu) {
        case LLG_VALUE_PACKED:
        case LLG_VALUE_REAL:
        case LLG_VALUE_STRING:
        case LLG_VALUE_AGGREGATE:
        case LLG_VALUE_FIXED_ARRAY:
        case LLG_VALUE_CONTAINER:
            return LLG_VALUE_COPY_DEEP;
        case LLG_VALUE_EVENT:
        case LLG_VALUE_OPAQUE:
        case LLG_VALUE_PROCESS:
            return LLG_VALUE_COPY_IDENTITY;
        case LLG_VALUE_CHANDLE:
            return LLG_VALUE_COPY_BORROWED;
        default:
            return -1;
    }
}

/* `path` holds the descriptors on the current descent; a descriptor that
 * reappears there is a cycle. Recursion is bounded by the same depth. */
static int llg_value_desc_valid_at(const llg_value_desc_t* desc,
                                   const llg_value_desc_t** path,
                                   size_t depth) {
    if (!desc || depth >= LLG_VALUE_DESC_MAX_DEPTH) return 0;
    for (size_t i = 0; i < depth; ++i)
        if (path[i] == desc) return 0;
    path[depth] = desc;
    switch (desc->kind) {
        case LLG_VALUE_PACKED:
            return desc->packed_width != 0 &&
                   desc->packed_width < LLG_SUPPORTED_WIDTH_LIMIT &&
                   desc->item_count == 0 && !desc->element &&
                   !desc->members && desc->member_count == 0;
        case LLG_VALUE_REAL:
        case LLG_VALUE_STRING:
        case LLG_VALUE_CHANDLE:
        case LLG_VALUE_EVENT:
        case LLG_VALUE_OPAQUE:
        case LLG_VALUE_PROCESS:
            return desc->item_count == 0 && !desc->element &&
                   !desc->members && desc->member_count == 0;
        case LLG_VALUE_AGGREGATE:
            if (desc->type_id == 0 || desc->member_count == 0 ||
                !desc->members || desc->element ||
                desc->item_count != desc->member_count ||
                desc->item_count > SIZE_MAX / sizeof(llg_value_t))
                return 0;
            for (size_t i = 0; i < desc->member_count; ++i)
                if (!llg_value_desc_valid_at(desc->members[i].value, path,
                                             depth + 1))
                    return 0;
            return 1;
        case LLG_VALUE_FIXED_ARRAY:
            return desc->item_count != 0 &&
                   desc->item_count <= SIZE_MAX / sizeof(llg_value_t) &&
                   !desc->members && desc->member_count == 0 &&
                   llg_value_desc_valid_at(desc->element, path, depth + 1);
        case LLG_VALUE_CONTAINER:
            return desc->item_count == 0 && !desc->members &&
                   desc->member_count == 0 &&
                   llg_value_desc_valid_at(desc->element, path, depth + 1);
        default:
            return 0;
    }
}

int llg_value_desc_valid(const llg_value_desc_t* desc) {
    const llg_value_desc_t* path[LLG_VALUE_DESC_MAX_DEPTH];
    return llg_value_desc_valid_at(desc, path, 0);
}

void llg_value_desc_check(const llg_value_desc_t* desc, const char* label) {
    if (llg_value_desc_valid(desc)) return;
    fprintf(stderr, "llg container fatal: invalid value descriptor for %s\n",
            label ? label : "<unnamed>");
    abort();
}

static void llg_value_trace_at(const llg_value_t* value,
                               llg_value_visit_fn visit, void* context) {
    if (!value || !value->desc) return;
    const llg_value_desc_t* desc = value->desc;
    switch (desc->kind) {
        case LLG_VALUE_EVENT:
        case LLG_VALUE_OPAQUE:
        case LLG_VALUE_PROCESS:
            if (value->value.handle)
                visit((void* const*)&value->value.handle, desc, context);
            break;
        case LLG_VALUE_AGGREGATE:
        case LLG_VALUE_FIXED_ARRAY:
            if (value->value.items)
                for (size_t i = 0; i < desc->item_count; ++i)
                    llg_value_trace_at(&value->value.items[i], visit, context);
            break;
        case LLG_VALUE_CONTAINER:
            if (value->value.container)
                for (size_t i = 0; i < value->value.container->size; ++i)
                    llg_value_trace_at(&value->value.container->data[i], visit,
                                       context);
            break;
        default:
            break;
    }
}

void llg_value_trace(const llg_value_t* value, llg_value_visit_fn visit,
                     void* context) {
    if (visit) llg_value_trace_at(value, visit, context);
}

void llg_native_value_init(llg_value_t* value, const llg_value_desc_t* desc) {
    if (!value || !desc)
        llg_container_fatal("missing native value descriptor");
    if (!llg_value_try_default(value, desc))
        llg_container_fatal("container allocation failed");
}

void llg_native_value_destroy(void* value) {
    llg_value_drop((llg_value_t*)value);
}

int llg_native_value_try_copy(llg_value_t* dst, const llg_value_t* src) {
    if (!dst || !dst->desc || !src || !src->desc ||
        !llg_value_desc_compatible(dst->desc, src->desc))
        llg_container_fatal("incompatible native value copy");
    return llg_value_try_copy(dst, dst->desc, src);
}

void llg_native_value_copy(llg_value_t* dst, const llg_value_t* src) {
    if (!llg_native_value_try_copy(dst, src))
        llg_container_fatal("container allocation failed");
}

void llg_native_value_clone(llg_value_t* dst, const llg_value_t* src) {
    if (!dst || !src || !src->desc)
        llg_container_fatal("missing native value descriptor");
    if (!llg_value_try_construct_copy(dst, src->desc, src))
        llg_container_fatal("container allocation failed");
}

static llg_native_root_t* llg_native_roots_head;
static size_t llg_native_roots_live;

void llg_native_root_init(llg_native_root_t* root, const llg_value_desc_t* desc) {
    if (!root) llg_container_fatal("missing native root");
    memset(root, 0, sizeof(*root));
    llg_native_value_init(&root->value, desc);
    root->next = llg_native_roots_head;
    if (llg_native_roots_head) llg_native_roots_head->prev = root;
    llg_native_roots_head = root;
    ++llg_native_roots_live;
}

void llg_native_root_destroy(void* opaque) {
    llg_native_root_t* root = (llg_native_root_t*)opaque;
    if (!root) return;
    llg_value_drop(&root->value);
    if (root->prev || root->next || llg_native_roots_head == root) {
        if (root->prev) root->prev->next = root->next;
        else llg_native_roots_head = root->next;
        if (root->next) root->next->prev = root->prev;
        root->prev = root->next = NULL;
        --llg_native_roots_live;
    }
}

size_t llg_native_roots_count(void) { return llg_native_roots_live; }

void llg_native_roots_trace(llg_value_visit_fn visit, void* context) {
    for (llg_native_root_t* root = llg_native_roots_head; root; root = root->next)
        llg_value_trace(&root->value, visit, context);
}
