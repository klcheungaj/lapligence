
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
    cell->value = identity ? queue->data[index] : llg_element_default(
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
    free(cell);
}

sv4_t llg_queue_cell_read(const void* ptr) {
    const struct llg_queue_cell* cell = ptr;
    if (!cell) llg_container_fatal("null retained queue reference");
    if (cell->owner) {
        size_t index = llg_queue_ref_index(cell->owner, cell->identity);
        if (index == SIZE_MAX) llg_container_fatal("queue reference was not disconnected");
        return cell->owner->data[index];
    }
    return cell->value;
}

int llg_queue_cell_write(void* ptr, sv4_t value) {
    struct llg_queue_cell* cell = ptr;
    if (!cell) llg_container_fatal("null retained queue reference");
    if (!cell->identity) return 0; // invalid actual, not a removed valid element
    if (cell->owner) return llg_queue_ref_write(cell->owner, cell->identity, value);
    cell->value = llg_element_assign(value, cell->value.width,
                                     cell->value.is_signed, cell->two_state);
    return 1;
}
