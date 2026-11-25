/* Associative arrays nested in values (see llg_nested_assoc_t). */

llg_nested_assoc_t* llg_nested_assoc_new(const llg_value_desc_t* desc) {
    llg_nested_assoc_t* nested = llg_value_try_items(1, sizeof(*nested));
    if (!nested) llg_msg_abort_allocation("container", 0, 0);
    nested->pins = 1;
    nested->desc = desc;
    const llg_value_desc_t* element = desc->element;
    int32_t string_key = desc->item_count == 1;
    int32_t class_key = desc->item_count == 2;
    if (element->kind == LLG_VALUE_PACKED) {
        if (class_key)
            llg_assoc_init_class(&nested->array.packed, element->packed_width,
                                 element->packed_signed,
                                 element->packed_two_state);
        else if (string_key)
            llg_assoc_init_string(&nested->array.packed, element->packed_width,
                                  element->packed_signed,
                                  element->packed_two_state);
        else
            llg_assoc_init_integral(&nested->array.packed,
                                    element->packed_width,
                                    element->packed_signed,
                                    element->packed_two_state,
                                    desc->packed_width, desc->packed_signed,
                                    desc->packed_two_state);
    } else if (class_key) {
        llg_assoc_value_init_class(&nested->array.values, element);
    } else if (string_key) {
        llg_assoc_value_init_string(&nested->array.values, element);
    } else {
        llg_assoc_value_init_integral(&nested->array.values, element,
                                      desc->packed_width, desc->packed_signed,
                                      desc->packed_two_state);
    }
    return nested;
}

llg_nested_assoc_t* llg_nested_assoc_clone(const llg_nested_assoc_t* source) {
    llg_nested_assoc_t* nested = llg_nested_assoc_new(source->desc);
    if (source->desc->element->kind == LLG_VALUE_PACKED)
        llg_assoc_copy(&nested->array.packed, &source->array.packed);
    else
        llg_assoc_value_copy(&nested->array.values, &source->array.values);
    return nested;
}

void llg_nested_assoc_release(llg_nested_assoc_t* nested) {
    if (--nested->pins != 0) return;
    if (nested->desc->element->kind == LLG_VALUE_PACKED)
        llg_assoc_destroy(&nested->array.packed);
    else
        llg_assoc_value_destroy(&nested->array.values);
    free(nested);
}

size_t llg_nested_assoc_size(const llg_nested_assoc_t* nested) {
    if (!nested) return 0;
    return nested->desc->element->kind == LLG_VALUE_PACKED
        ? nested->array.packed.size
        : nested->array.values.size;
}

static int32_t llg_nested_assoc_key_equal(const sv4_t* left_integral,
                                      const uint8_t* left_text,
                                      size_t left_length,
                                      const sv4_t* right_integral,
                                      const uint8_t* right_text,
                                      size_t right_length) {
    if (left_text || right_text)
        return left_length == right_length &&
               (left_length == 0 ||
                memcmp(left_text, right_text, left_length) == 0);
    return sv4_same(*left_integral, *right_integral);
}

static int32_t llg_assoc_entry_key_equal(const llg_assoc_entry_t* a,
                                         const llg_assoc_entry_t* b) {
    return llg_nested_assoc_key_equal(&a->integral_key, a->string_key,
                                      a->string_length, &b->integral_key,
                                      b->string_key, b->string_length);
}

static int32_t llg_assoc_value_entry_key_equal(
    const llg_assoc_value_entry_t* a, const llg_assoc_value_entry_t* b) {
    return llg_nested_assoc_key_equal(&a->integral_key, a->string_key,
                                      a->string_length, &b->integral_key,
                                      b->string_key, b->string_length);
}

int32_t llg_nested_assoc_equal(const llg_nested_assoc_t* a,
                           const llg_nested_assoc_t* b) {
    if (a == b) return 1;
    size_t size = llg_nested_assoc_size(a);
    if (size != llg_nested_assoc_size(b)) return 0;
    if (size == 0) return 1;
    if (a->desc->element->kind == LLG_VALUE_PACKED) {
        for (size_t i = 0; i < size; ++i) {
            const llg_assoc_entry_t* left = &a->array.packed.entries[i];
            const llg_assoc_entry_t* right = &b->array.packed.entries[i];
            if (!llg_assoc_entry_key_equal(left, right) ||
                !sv4_same(left->value, right->value))
                return 0;
        }
        return 1;
    }
    for (size_t i = 0; i < size; ++i) {
        const llg_assoc_value_entry_t* left = &a->array.values.entries[i];
        const llg_assoc_value_entry_t* right = &b->array.values.entries[i];
        if (!llg_assoc_value_entry_key_equal(left, right) ||
            !llg_value_equal(&left->value, &right->value))
            return 0;
    }
    return 1;
}

/* SystemVerilog equality of associative arrays (7.6, 11.4.5): the same keys
 * with equal elements. The entries are in key order. */
static int32_t llg_assoc_equality(const llg_assoc_t* a, const llg_assoc_t* b,
                                  int32_t case_equality) {
    if (a->size != b->size) return LLG_VALUE_UNEQUAL;
    for (size_t i = 0; i < a->size; ++i) {
        if (!llg_assoc_entry_key_equal(&a->entries[i], &b->entries[i]))
            return LLG_VALUE_UNEQUAL;
    }
    int32_t result = LLG_VALUE_EQUAL;
    for (size_t i = 0; i < a->size && result != LLG_VALUE_UNEQUAL; ++i)
        result = llg_value_equality_fold(
            result, llg_packed_equality(a->entries[i].value,
                                        b->entries[i].value, case_equality));
    return result;
}

static int32_t llg_assoc_value_equality(const llg_assoc_value_t* a,
                                        const llg_assoc_value_t* b,
                                        int32_t case_equality) {
    if (a->size != b->size) return LLG_VALUE_UNEQUAL;
    for (size_t i = 0; i < a->size; ++i) {
        if (!llg_assoc_value_entry_key_equal(&a->entries[i], &b->entries[i]))
            return LLG_VALUE_UNEQUAL;
    }
    int32_t result = LLG_VALUE_EQUAL;
    for (size_t i = 0; i < a->size && result != LLG_VALUE_UNEQUAL; ++i)
        result = llg_value_equality_fold(
            result, llg_value_equality(&a->entries[i].value,
                                       &b->entries[i].value, case_equality));
    return result;
}

sv4_t llg_assoc_equal(const llg_assoc_t* a, const llg_assoc_t* b,
                      int32_t case_equality, int32_t negate) {
    return llg_equality_result(llg_assoc_equality(a, b, case_equality), negate);
}

sv4_t llg_assoc_value_equal(const llg_assoc_value_t* a,
                            const llg_assoc_value_t* b, int32_t case_equality,
                            int32_t negate) {
    return llg_equality_result(llg_assoc_value_equality(a, b, case_equality),
                               negate);
}

int32_t llg_nested_assoc_equality(const llg_nested_assoc_t* a,
                                  const llg_nested_assoc_t* b,
                                  int32_t case_equality) {
    if (llg_nested_assoc_size(a) != llg_nested_assoc_size(b))
        return LLG_VALUE_UNEQUAL;
    if (llg_nested_assoc_size(a) == 0) return LLG_VALUE_EQUAL;
    if (a->desc->element->kind == LLG_VALUE_PACKED)
        return llg_assoc_equality(&a->array.packed, &b->array.packed,
                                  case_equality);
    return llg_assoc_value_equality(&a->array.values, &b->array.values,
                                    case_equality);
}

void llg_nested_assoc_trace(const llg_nested_assoc_t* nested,
                            llg_value_visit_fn visit, void* context) {
    int32_t packed = nested->desc->element->kind == LLG_VALUE_PACKED;
    if (nested->desc->item_count == 2) {
        size_t size = llg_nested_assoc_size(nested);
        for (size_t i = 0; i < size; ++i) {
            void* key = (void*)(uintptr_t)sv4_to_u64(
                packed ? nested->array.packed.entries[i].integral_key
                       : nested->array.values.entries[i].integral_key);
            if (key) visit((void* const*)&key, NULL, context);
        }
    }
    if (packed) return;
    for (size_t i = 0; i < nested->array.values.size; ++i)
        llg_value_trace_at(&nested->array.values.entries[i].value, visit,
                           context);
}

void* llg_nested_assoc_bind(llg_value_t* value, llg_nested_assoc_t** pin) {
    if (!value->value.assoc)
        value->value.assoc = llg_nested_assoc_new(value->desc);
    llg_nested_assoc_t* nested = value->value.assoc;
    ++nested->pins;
    *pin = nested;
    return nested->desc->element->kind == LLG_VALUE_PACKED
        ? (void*)&nested->array.packed
        : (void*)&nested->array.values;
}

void llg_nested_assoc_unpin(void* pin) {
    llg_nested_assoc_t** owner = pin;
    if (*owner) llg_nested_assoc_release(*owner);
    *owner = NULL;
}

static void llg_nested_assoc_value(llg_value_t* value,
                                   const llg_value_desc_t* desc,
                                   const void* source) {
    value->desc = desc;
    value->value.assoc = llg_nested_assoc_new(desc);
    if (desc->element->kind == LLG_VALUE_PACKED)
        llg_assoc_copy(&value->value.assoc->array.packed, source);
    else
        llg_assoc_value_copy(&value->value.assoc->array.values, source);
}

void llg_queue_value_push_assoc(llg_queue_value_array_t* queue, int32_t back,
                                const void* source) {
    llg_value_t value;
    llg_nested_assoc_value(&value, queue->element, source);
    llg_queue_value_push_value(queue, back, &value);
    llg_value_drop(&value);
}

int32_t llg_queue_value_insert_assoc(llg_queue_value_array_t* queue, sv4_t index,
                                 const void* source) {
    llg_value_t value;
    llg_nested_assoc_value(&value, queue->element, source);
    int32_t inserted = llg_queue_value_insert_value(queue, index, &value);
    llg_value_drop(&value);
    return inserted;
}
