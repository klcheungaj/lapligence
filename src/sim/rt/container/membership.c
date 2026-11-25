/* Set membership over stored arrays (SV 11.4.13) -------------------------- */

/* The or reduction of two comparisons: a match decides, else an unknown. */
static int32_t llg_inside_fold(int32_t result, int32_t item) {
    if (result == LLG_VALUE_EQUAL || item == LLG_VALUE_EQUAL) return LLG_VALUE_EQUAL;
    return item == LLG_VALUE_EQUALITY_UNKNOWN ? item : result;
}

static int32_t llg_inside_equality(int32_t equal) {
    return equal ? LLG_VALUE_EQUAL : LLG_VALUE_UNEQUAL;
}

static int32_t llg_inside_packed(const llg_inside_selector_t* selector, sv4_t item) {
    if (selector->kind == LLG_VALUE_REAL)
        return llg_inside_equality(selector->value.real == sv4_to_real(item));
    if (selector->kind != LLG_VALUE_PACKED) return LLG_VALUE_UNEQUAL;
    sv4_t match = sv4_wild_eq(*selector->value.packed, item);
    int32_t result = sv4_is_unknown(match) ? LLG_VALUE_EQUALITY_UNKNOWN
                                           : llg_inside_equality(sv4_to_bool(match));
    sv4_destroy(&match);
    return result;
}

static int32_t llg_inside_packed_items(const llg_inside_selector_t* selector,
                                       const sv4_t* items, size_t count) {
    int32_t result = LLG_VALUE_UNEQUAL;
    for (size_t i = 0; i < count && result != LLG_VALUE_EQUAL; ++i)
        result = llg_inside_fold(result, llg_inside_packed(selector, items[i]));
    return result;
}

static int32_t llg_inside_items(const llg_inside_selector_t* selector,
                                const llg_value_t* items, size_t count) {
    int32_t result = LLG_VALUE_UNEQUAL;
    for (size_t i = 0; i < count && result != LLG_VALUE_EQUAL; ++i)
        result = llg_inside_fold(result, llg_value_inside(&items[i], *selector));
    return result;
}

static int32_t llg_inside_entries(const llg_inside_selector_t* selector,
                                  const llg_assoc_value_t* array) {
    int32_t result = LLG_VALUE_UNEQUAL;
    for (size_t i = 0; i < array->size && result != LLG_VALUE_EQUAL; ++i)
        result = llg_inside_fold(result, llg_value_inside(&array->entries[i].value, *selector));
    return result;
}

static int32_t llg_inside_nested_assoc(const llg_inside_selector_t* selector,
                                       const llg_nested_assoc_t* nested) {
    if (nested->desc->element->kind != LLG_VALUE_PACKED)
        return llg_inside_entries(selector, &nested->array.values);
    int32_t result = LLG_VALUE_UNEQUAL;
    const llg_assoc_t* array = &nested->array.packed;
    for (size_t i = 0; i < array->size && result != LLG_VALUE_EQUAL; ++i)
        result = llg_inside_fold(result, llg_inside_packed(selector, array->entries[i].value));
    return result;
}

int32_t llg_value_inside(const llg_value_t* value, llg_inside_selector_t selector) {
    switch (value->desc ? value->desc->kind : LLG_VALUE_AGGREGATE) {
        case LLG_VALUE_PACKED:
            return llg_inside_packed(&selector, value->value.packed);
        case LLG_VALUE_REAL:
            if (selector.kind == LLG_VALUE_PACKED)
                return llg_inside_equality(sv4_to_real(*selector.value.packed) ==
                                           value->value.real);
            return llg_inside_equality(selector.kind == LLG_VALUE_REAL &&
                                       selector.value.real == value->value.real);
        case LLG_VALUE_STRING: {
            const llg_string_t* text = selector.value.string;
            return llg_inside_equality(
                selector.kind == LLG_VALUE_STRING && text->len == value->value.string.len &&
                (!text->len || memcmp(text->data, value->value.string.data, text->len) == 0));
        }
        case LLG_VALUE_CHANDLE:
        case LLG_VALUE_EVENT:
        case LLG_VALUE_OPAQUE:
        case LLG_VALUE_PROCESS:
            return llg_inside_equality(selector.kind == LLG_VALUE_CHANDLE &&
                                       selector.value.handle == value->value.handle);
        case LLG_VALUE_FIXED_ARRAY:
            return value->value.items
                       ? llg_inside_items(&selector, value->value.items, value->desc->item_count)
                       : LLG_VALUE_UNEQUAL;
        case LLG_VALUE_CONTAINER: {
            llg_seq_view_t view = llg_nested_seq_view(value->value.container);
            return view.values ? llg_inside_items(&selector, view.values, view.size)
                               : llg_inside_packed_items(&selector, view.packed, view.size);
        }
        case LLG_VALUE_ASSOC:
            return value->value.assoc ? llg_inside_nested_assoc(&selector, value->value.assoc)
                                      : LLG_VALUE_UNEQUAL;
        default:
            return LLG_VALUE_UNEQUAL;
    }
}

int32_t llg_dyn_value_inside(const llg_dyn_value_array_t* array,
                             llg_inside_selector_t selector) {
    return llg_inside_items(&selector, array->data, array->size);
}

int32_t llg_queue_value_inside(const llg_queue_value_array_t* queue,
                               llg_inside_selector_t selector) {
    return llg_inside_items(&selector, queue->data, queue->size);
}

int32_t llg_assoc_value_inside(const llg_assoc_value_t* array,
                               llg_inside_selector_t selector) {
    return llg_inside_entries(&selector, array);
}

sv4_t llg_inside_result(int32_t match) { return llg_equality_result(match, 0); }
