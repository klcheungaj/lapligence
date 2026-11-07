/* Associative arrays nested in values (see llg_nested_assoc_t). */

llg_nested_assoc_t* llg_nested_assoc_new(const llg_value_desc_t* desc) {
    llg_nested_assoc_t* nested = llg_value_try_items(1, sizeof(*nested));
    if (!nested) llg_container_fatal("container allocation failed");
    nested->pins = 1;
    nested->desc = desc;
    const llg_value_desc_t* element = desc->element;
    int string_key = desc->item_count != 0;
    if (element->kind == LLG_VALUE_PACKED) {
        if (string_key)
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

static int llg_nested_assoc_key_equal(const sv4_t* left_integral,
                                      const unsigned char* left_text,
                                      size_t left_length,
                                      const sv4_t* right_integral,
                                      const unsigned char* right_text,
                                      size_t right_length) {
    if (left_text || right_text)
        return left_length == right_length &&
               (left_length == 0 ||
                memcmp(left_text, right_text, left_length) == 0);
    return sv4_same(*left_integral, *right_integral);
}

int llg_nested_assoc_equal(const llg_nested_assoc_t* a,
                           const llg_nested_assoc_t* b) {
    if (a == b) return 1;
    size_t size = llg_nested_assoc_size(a);
    if (size != llg_nested_assoc_size(b)) return 0;
    if (size == 0) return 1;
    if (a->desc->element->kind == LLG_VALUE_PACKED) {
        for (size_t i = 0; i < size; ++i) {
            const llg_assoc_entry_t* left = &a->array.packed.entries[i];
            const llg_assoc_entry_t* right = &b->array.packed.entries[i];
            if (!llg_nested_assoc_key_equal(
                    &left->integral_key, left->string_key, left->string_length,
                    &right->integral_key, right->string_key,
                    right->string_length) ||
                !sv4_same(left->value, right->value))
                return 0;
        }
        return 1;
    }
    for (size_t i = 0; i < size; ++i) {
        const llg_assoc_value_entry_t* left = &a->array.values.entries[i];
        const llg_assoc_value_entry_t* right = &b->array.values.entries[i];
        if (!llg_nested_assoc_key_equal(
                &left->integral_key, left->string_key, left->string_length,
                &right->integral_key, right->string_key,
                right->string_length) ||
            !llg_value_equal(&left->value, &right->value))
            return 0;
    }
    return 1;
}

void llg_nested_assoc_trace(const llg_nested_assoc_t* nested,
                            llg_value_visit_fn visit, void* context) {
    if (nested->desc->element->kind == LLG_VALUE_PACKED) return;
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

void llg_queue_value_push_assoc(llg_queue_value_array_t* queue, int back,
                                const void* source) {
    llg_value_t value;
    llg_nested_assoc_value(&value, queue->element, source);
    llg_queue_value_push_value(queue, back, &value);
    llg_value_drop(&value);
}

int llg_queue_value_insert_assoc(llg_queue_value_array_t* queue, sv4_t index,
                                 const void* source) {
    llg_value_t value;
    llg_nested_assoc_value(&value, queue->element, source);
    int inserted = llg_queue_value_insert_value(queue, index, &value);
    llg_value_drop(&value);
    return inserted;
}
