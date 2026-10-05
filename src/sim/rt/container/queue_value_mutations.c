
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

/* Remove one endpoint element (SV 7.10.2.4-5) and transfer its value to
 * `removed`, which must be empty. Returns 0 and leaves `removed` empty for an
 * empty queue. The caller publishes the result before notifying readers. */
static int llg_queue_value_take(llg_queue_value_array_t* queue, int back,
                                llg_value_t* removed) {
    if (!queue) llg_container_fatal("null queue pop target");
    if (!queue->size) return 0;
    size_t position = back ? queue->size - 1 : 0;
    *removed = queue->data[position]; // exclusive ownership transfer
    if (!back && queue->size > 1)
        memmove(queue->data, queue->data + 1,
                (queue->size - 1) * sizeof(*queue->data));
    --queue->size;
    // Relocation transfers ownership; the unused tail must not retain an alias.
    memset(&queue->data[queue->size], 0, sizeof(*queue->data));
    llg_queue_value_invalidate_refs(queue);
    return 1;
}

static void llg_queue_value_pop_notify(llg_queue_value_array_t* queue) {
    llg_notify(queue->notify, queue->contents_dependency,
               queue->shape_dependency,
               LLG_CONTAINER_CHANGED_CONTENTS | LLG_CONTAINER_CHANGED_SHAPE);
}

double llg_queue_value_pop_real(llg_queue_value_array_t* queue, int back) {
    llg_value_t removed = {0};
    if (!llg_queue_value_take(queue, back, &removed)) return 0.0;
    double result = removed.desc && removed.desc->kind == LLG_VALUE_REAL
        ? removed.value.real
        : 0.0;
    llg_value_drop(&removed);
    llg_queue_value_pop_notify(queue);
    return result;
}

void llg_queue_value_pop_string_to(llg_string_t* dst,
                                   llg_queue_value_array_t* queue, int back) {
    if (!dst) llg_container_fatal("queue pop requires output storage");
    llg_value_t removed = {0};
    llg_string_destroy(dst);
    if (!llg_queue_value_take(queue, back, &removed)) return;
    if (removed.desc && removed.desc->kind == LLG_VALUE_STRING) {
        *dst = removed.value.string; // the result owner adopts the bytes
        removed.value.string = (llg_string_t){0};
    }
    llg_value_drop(&removed);
    llg_queue_value_pop_notify(queue);
}

void* llg_queue_value_pop_chandle(llg_queue_value_array_t* queue, int back) {
    llg_value_t removed = {0};
    if (!llg_queue_value_take(queue, back, &removed)) return NULL;
    void* result = llg_value_is_handle_kind(removed.desc)
        ? removed.value.handle
        : NULL;
    llg_value_drop(&removed);
    llg_queue_value_pop_notify(queue);
    return result;
}
