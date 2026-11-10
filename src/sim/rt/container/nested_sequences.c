/* Queues and dynamic arrays nested in values (see llg_nested_seq_t). */

static llg_seq_view_t llg_nested_seq_view(const llg_nested_seq_t* seq) {
    llg_seq_view_t view = {0};
    if (!seq) return view;
    switch (seq->kind) {
        case LLG_NESTED_SEQ_DYN:
            view.packed = seq->array.dyn.data;
            view.size = seq->array.dyn.size;
            break;
        case LLG_NESTED_SEQ_QUEUE:
            view.packed = seq->array.queue.data;
            view.size = seq->array.queue.size;
            break;
        case LLG_NESTED_SEQ_DYN_VALUES:
            view.values = seq->array.dyn_values.data;
            view.size = seq->array.dyn_values.size;
            view.element = seq->array.dyn_values.element;
            break;
        default:
            view.values = seq->array.queue_values.data;
            view.size = seq->array.queue_values.size;
            view.element = seq->array.queue_values.element;
            break;
    }
    return view;
}

static llg_seq_view_t llg_seq_view_of_dyn(const llg_dyn_array_t* source) {
    llg_seq_view_t view = {0};
    view.packed = source->data;
    view.size = source->size;
    return view;
}

static llg_seq_view_t llg_seq_view_of_queue(const llg_queue_t* source) {
    llg_seq_view_t view = {0};
    view.packed = source->data;
    view.size = source->size;
    return view;
}

static llg_seq_view_t llg_seq_view_of_dyn_value(
    const llg_dyn_value_array_t* source) {
    llg_seq_view_t view = {0};
    view.values = source->data;
    view.size = source->size;
    view.element = source->element;
    return view;
}

static llg_seq_view_t llg_seq_view_of_queue_value(
    const llg_queue_value_array_t* source) {
    llg_seq_view_t view = {0};
    view.values = source->data;
    view.size = source->size;
    view.element = source->element;
    return view;
}

/* Maximum element count of the queue `desc` describes, in the form the queue
 * initializers take. */
static uint64_t llg_nested_seq_limit(const llg_value_desc_t* desc) {
    return desc->item_count == LLG_VALUE_CONTAINER_QUEUE
        ? UINT64_MAX
        : (uint64_t)(desc->item_count - LLG_VALUE_CONTAINER_QUEUE);
}

llg_nested_seq_t* llg_nested_seq_new(const llg_value_desc_t* desc) {
    llg_nested_seq_t* seq = llg_value_try_items(1, sizeof(*seq));
    if (!seq) llg_container_fatal("container allocation failed");
    memset(seq, 0, sizeof(*seq));
    seq->pins = 1;
    seq->desc = desc;
    const llg_value_desc_t* element = desc->element;
    int queue = desc->item_count != LLG_VALUE_CONTAINER_DYNAMIC;
    if (element->kind == LLG_VALUE_PACKED) {
        if (queue) {
            seq->kind = LLG_NESTED_SEQ_QUEUE;
            llg_queue_init(&seq->array.queue, element->packed_width,
                           element->packed_signed, element->packed_two_state,
                           llg_nested_seq_limit(desc));
        } else {
            seq->kind = LLG_NESTED_SEQ_DYN;
            llg_dyn_init(&seq->array.dyn, element->packed_width,
                         element->packed_signed, element->packed_two_state);
        }
    } else if (queue) {
        seq->kind = LLG_NESTED_SEQ_QUEUE_VALUES;
        llg_queue_value_init(&seq->array.queue_values, element,
                             llg_nested_seq_limit(desc));
    } else {
        seq->kind = LLG_NESTED_SEQ_DYN_VALUES;
        llg_dyn_value_init(&seq->array.dyn_values, element);
    }
    return seq;
}

/* Replace the elements of `seq` with converted copies of `source`: packed
 * payloads take the element type, a real element converts a packed payload,
 * and a bounded queue keeps its head and warns, as a variable does. */
static void llg_nested_seq_assign(llg_nested_seq_t* seq, llg_seq_view_t source) {
    switch (seq->kind) {
        case LLG_NESTED_SEQ_DYN:
        case LLG_NESTED_SEQ_QUEUE:
            if (source.size && !source.packed)
                llg_container_fatal("packed container used with an incompatible element");
            if (seq->kind == LLG_NESTED_SEQ_DYN)
                llg_dyn_assign_values(&seq->array.dyn, source.packed,
                                      source.size);
            else
                llg_queue_assign_values(&seq->array.queue, source.packed,
                                        source.size);
            return;
        default:
            break;
    }
    llg_dyn_value_array_t converted = {0};
    llg_dyn_value_array_t borrowed = {0};
    const llg_dyn_value_array_t* elements = &borrowed;
    if (source.packed) {
        /* Packed payloads into real elements. */
        llg_dyn_array_t packed = {0};
        packed.data = (sv4_t*)source.packed;
        packed.size = source.size;
        llg_dyn_value_init(&converted, seq->desc->element);
        llg_dyn_value_replace_from_packed(&converted, &packed);
        elements = &converted;
    } else {
        borrowed.data = (llg_value_t*)source.values;
        borrowed.size = source.size;
        borrowed.element = source.element ? source.element : seq->desc->element;
    }
    if (seq->kind == LLG_NESTED_SEQ_DYN_VALUES) {
        llg_dyn_value_copy(&seq->array.dyn_values, elements);
    } else {
        llg_queue_value_array_t queue = {0};
        queue.data = elements->data;
        queue.size = elements->size;
        queue.element = elements->element;
        llg_queue_value_copy(&seq->array.queue_values, &queue);
    }
    if (elements == &converted) llg_dyn_value_destroy(&converted);
}

/* A sequence of type `desc` holding converted copies of `source`; an empty
 * one is the null default. */
static llg_nested_seq_t* llg_nested_seq_from_view(const llg_value_desc_t* desc,
                                                  llg_seq_view_t source) {
    if (!source.size) return NULL;
    llg_nested_seq_t* seq = llg_nested_seq_new(desc);
    llg_nested_seq_assign(seq, source);
    return seq;
}

llg_nested_seq_t* llg_nested_seq_clone(const llg_value_desc_t* desc,
                                       const llg_nested_seq_t* source) {
    return llg_nested_seq_from_view(desc, llg_nested_seq_view(source));
}

void llg_nested_seq_release(llg_nested_seq_t* seq) {
    if (--seq->pins != 0) return;
    switch (seq->kind) {
        case LLG_NESTED_SEQ_DYN:
            llg_dyn_destroy(&seq->array.dyn);
            break;
        case LLG_NESTED_SEQ_QUEUE:
            llg_queue_destroy(&seq->array.queue);
            break;
        case LLG_NESTED_SEQ_DYN_VALUES:
            llg_dyn_value_destroy(&seq->array.dyn_values);
            break;
        default:
            llg_queue_value_destroy(&seq->array.queue_values);
            break;
    }
    free(seq);
}

size_t llg_nested_seq_size(const llg_nested_seq_t* seq) {
    return llg_nested_seq_view(seq).size;
}

int llg_nested_seq_equal(const llg_nested_seq_t* a, const llg_nested_seq_t* b) {
    if (a == b) return 1;
    llg_seq_view_t left = llg_nested_seq_view(a);
    llg_seq_view_t right = llg_nested_seq_view(b);
    if (left.size != right.size) return 0;
    if (!left.size) return 1;
    if (left.packed && right.packed) {
        for (size_t i = 0; i < left.size; ++i)
            if (!sv4_same(left.packed[i], right.packed[i])) return 0;
        return 1;
    }
    if (!left.values || !right.values) return 0;
    for (size_t i = 0; i < left.size; ++i)
        if (!llg_value_equal(&left.values[i], &right.values[i])) return 0;
    return 1;
}

int llg_nested_seq_equality(const llg_nested_seq_t* a,
                            const llg_nested_seq_t* b, int case_equality) {
    llg_seq_view_t left = llg_nested_seq_view(a);
    llg_seq_view_t right = llg_nested_seq_view(b);
    if (left.size != right.size) return LLG_VALUE_UNEQUAL;
    if (!left.size) return LLG_VALUE_EQUAL;
    if (left.values && right.values)
        return llg_value_items_equality(left.values, left.size, right.values,
                                        right.size, case_equality);
    if (!left.packed || !right.packed) return LLG_VALUE_UNEQUAL;
    int result = LLG_VALUE_EQUAL;
    for (size_t i = 0; i < left.size && result != LLG_VALUE_UNEQUAL; ++i)
        result = llg_value_equality_fold(
            result,
            llg_packed_equality(left.packed[i], right.packed[i], case_equality));
    return result;
}

void llg_nested_seq_trace(const llg_nested_seq_t* seq, llg_value_visit_fn visit,
                          void* context) {
    llg_seq_view_t view = llg_nested_seq_view(seq);
    if (!view.values) return;
    for (size_t i = 0; i < view.size; ++i)
        llg_value_trace_at(&view.values[i], visit, context);
}

llg_nested_seq_t* llg_nested_seq_merge(const llg_value_desc_t* desc,
                                       const llg_nested_seq_t* a,
                                       const llg_nested_seq_t* b) {
    llg_seq_view_t left = llg_nested_seq_view(a);
    llg_seq_view_t right = llg_nested_seq_view(b);
    if (!left.size && !right.size) return NULL;
    llg_nested_seq_t* seq = llg_nested_seq_new(desc);
    const llg_value_desc_t* element = desc->element;
    switch (seq->kind) {
        case LLG_NESTED_SEQ_DYN:
        case LLG_NESTED_SEQ_QUEUE: {
            llg_dyn_array_t first = {0};
            llg_dyn_array_t second = {0};
            first.data = (sv4_t*)left.packed;
            first.size = left.size;
            second.data = (sv4_t*)right.packed;
            second.size = right.size;
            /* Merge without the bound, then assign as a variable does. */
            llg_dyn_array_t merged;
            llg_dyn_init(&merged, element->packed_width, element->packed_signed,
                         element->packed_two_state);
            llg_dyn_merge(&merged, &first, &second);
            llg_nested_seq_assign(seq, llg_seq_view_of_dyn(&merged));
            llg_dyn_destroy(&merged);
            break;
        }
        case LLG_NESTED_SEQ_DYN_VALUES: {
            llg_dyn_value_array_t first = {0};
            llg_dyn_value_array_t second = {0};
            first.data = (llg_value_t*)left.values;
            first.size = left.size;
            first.element = left.element ? left.element : element;
            second.data = (llg_value_t*)right.values;
            second.size = right.size;
            second.element = right.element ? right.element : element;
            llg_dyn_value_merge(&seq->array.dyn_values, &first, &second);
            break;
        }
        default: {
            llg_queue_value_array_t first = {0};
            llg_queue_value_array_t second = {0};
            first.data = (llg_value_t*)left.values;
            first.size = left.size;
            first.element = left.element ? left.element : element;
            second.data = (llg_value_t*)right.values;
            second.size = right.size;
            second.element = right.element ? right.element : element;
            llg_queue_value_merge(&seq->array.queue_values, &first, &second);
            break;
        }
    }
    return seq;
}

void* llg_nested_seq_bind(llg_value_t* value, llg_nested_seq_t** pin) {
    if (!value->value.container)
        value->value.container = llg_nested_seq_new(value->desc);
    llg_nested_seq_t* seq = value->value.container;
    ++seq->pins;
    ++llg_nested_seq_pins_live;
    *pin = seq;
    return &seq->array;
}

void llg_nested_seq_unpin(void* pin) {
    llg_nested_seq_t** owner = pin;
    if (!*owner) return;
    --llg_nested_seq_pins_live;
    llg_nested_seq_release(*owner);
    *owner = NULL;
}

/* What an in-place assignment changed in a nested sequence, which has no
 * dependencies of its own: its notification lands here for the owner. */
static int llg_nested_seq_changes;

static void llg_nested_seq_note_change(sv4_t* contents, sv4_t* shape,
                                       int change) {
    (void)contents;
    (void)shape;
    llg_nested_seq_changes |= change;
}

/* Replace the elements of the sequence in slot `target` with converted
 * copies of `source`, in place: an alias a statement holds still names the
 * element. Returns whether the elements changed. An empty sequence no alias
 * names is the null default. */
static int llg_nested_seq_store(llg_value_t* target, llg_seq_view_t source) {
    llg_nested_seq_t* seq = target->value.container;
    if (!seq) {
        target->value.container = llg_nested_seq_from_view(target->desc, source);
        return target->value.container != NULL;
    }
    llg_container_notify_fn* notify;
    switch (seq->kind) {
        case LLG_NESTED_SEQ_DYN:
            notify = &seq->array.dyn.notify;
            break;
        case LLG_NESTED_SEQ_QUEUE:
            notify = &seq->array.queue.notify;
            break;
        case LLG_NESTED_SEQ_DYN_VALUES:
            notify = &seq->array.dyn_values.notify;
            break;
        default:
            notify = &seq->array.queue_values.notify;
            break;
    }
    int outer = llg_nested_seq_changes;
    llg_nested_seq_changes = 0;
    *notify = llg_nested_seq_note_change;
    llg_nested_seq_assign(seq, source);
    *notify = NULL;
    int changed = llg_nested_seq_changes != 0;
    llg_nested_seq_changes = outer;
    /* Emptied and unaliased: back to the null default. */
    if (seq->pins == 1 && !llg_nested_seq_size(seq)) {
        llg_nested_seq_release(seq);
        target->value.container = NULL;
    }
    return changed;
}

llg_value_t* llg_nested_seq_value_at(const llg_value_t* owner, sv4_t index) {
    if (!owner || !owner->desc || owner->desc->kind != LLG_VALUE_CONTAINER)
        return NULL;
    llg_seq_view_t view = llg_nested_seq_view(owner->value.container);
    size_t native;
    if (!view.values || !llg_index(index, view.size, 0, &native)) return NULL;
    return (llg_value_t*)&view.values[native];
}

int llg_nested_seq_holds_packed(const llg_value_t* owner) {
    return owner && owner->desc && owner->desc->kind == LLG_VALUE_CONTAINER &&
           owner->desc->element->kind == LLG_VALUE_PACKED;
}

sv4_t llg_nested_seq_get_packed(const llg_value_t* owner, sv4_t index) {
    llg_seq_view_t view = llg_nested_seq_view(owner->value.container);
    const llg_value_desc_t* element = owner->desc->element;
    size_t native;
    if (!view.packed || !llg_index(index, view.size, 0, &native))
        return llg_element_default(element->packed_width,
                                   element->packed_signed,
                                   element->packed_two_state);
    return sv4_clone(&view.packed[native]);
}

int llg_nested_seq_set_packed(llg_value_t* owner, sv4_t index, sv4_t value,
                              int* change) {
    const llg_value_desc_t* desc = owner->desc;
    int queue = desc->item_count != LLG_VALUE_CONTAINER_DYNAMIC;
    llg_nested_seq_t* seq = owner->value.container;
    size_t size = llg_nested_seq_size(seq);
    size_t native;
    if (llg_index(index, size, 0, &native)) {
        sv4_t* target = seq->kind == LLG_NESTED_SEQ_QUEUE
            ? &seq->array.queue.data[native]
            : &seq->array.dyn.data[native];
        sv4_t assigned = llg_element_assign(value, desc->element->packed_width,
                                            desc->element->packed_signed,
                                            desc->element->packed_two_state);
        if (sv4_same(*target, assigned)) {
            sv4_destroy(&assigned);
            return 1;
        }
        sv4_move(target, &assigned);
        *change |= LLG_CONTAINER_CHANGED_CONTENTS;
        return 1;
    }
    /* A queue takes a write at its size as an append (SV 7.10.1); the null
     * default is the empty queue. */
    if (!queue) return 0;
    if (!seq) {
        if (!llg_index(index, 0, 1, &native)) {
            llg_container_warning("invalid queue write index");
            return 0;
        }
        seq = owner->value.container = llg_nested_seq_new(desc);
    }
    if (!llg_queue_set(&seq->array.queue, index, value)) return 0;
    *change |= LLG_CONTAINER_CHANGED_CONTENTS;
    return 1;
}
