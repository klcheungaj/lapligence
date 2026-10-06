enum {
    LLG_ELEMENT_CELL_DYNAMIC = 0,
    LLG_ELEMENT_CELL_ASSOC_INTEGRAL = 1,
    LLG_ELEMENT_CELL_ASSOC_STRING = 2,
};

/* One retained `ref` element. While `owner` is set the cell names live
 * storage: a dynamic-array index, or an associative key looked up on every
 * access so entry insertion and reallocation do not move it. Outdating
 * snapshots the element into `value` and clears `owner`. `valid` is zero for
 * an actual whose selector was out of range or unknown: reads return the
 * element default and writes do nothing, as for the selected expression. */
struct llg_element_cell {
    void* owner;
    struct llg_element_cell* next;
    size_t refs;
    size_t index;
    sv4_t key;
    unsigned char* string_key;
    size_t string_length;
    sv4_t value;
    uint32_t element_width;
    int8_t element_signed;
    uint8_t element_two_state;
    uint8_t kind;
    uint8_t valid;
};

static struct llg_element_cell* llg_element_cell_new(
    void* owner, uint8_t kind, uint32_t width, int8_t is_signed,
    uint8_t two_state) {
    struct llg_element_cell* cell = llg_alloc_items(1, sizeof(*cell));
    memset(cell, 0, sizeof(*cell));
    cell->refs = 1;
    cell->owner = owner;
    cell->kind = kind;
    cell->element_width = width;
    cell->element_signed = is_signed;
    cell->element_two_state = two_state;
    cell->valid = 1;
    cell->value = llg_element_default(width, is_signed, two_state);
    return cell;
}

static void* llg_element_cell_share(struct llg_element_cell* cell) {
    if (cell->refs == SIZE_MAX) llg_container_fatal("element reference count overflow");
    ++cell->refs;
    return cell;
}

static struct llg_element_cell** llg_element_cell_list(struct llg_element_cell* cell) {
    if (cell->kind == LLG_ELEMENT_CELL_DYNAMIC)
        return &((llg_dyn_array_t*)cell->owner)->references;
    return &((llg_assoc_t*)cell->owner)->references;
}

void* llg_dyn_ref_acquire(llg_dyn_array_t* array, sv4_t index) {
    if (!array) llg_container_fatal("null dynamic-array reference source");
    size_t native;
    if (!llg_index(index, array->size, 0, &native)) {
        struct llg_element_cell* cell = llg_element_cell_new(
            NULL, LLG_ELEMENT_CELL_DYNAMIC, array->element_width,
            array->element_signed, array->element_two_state);
        cell->valid = 0;
        return cell;
    }
    for (struct llg_element_cell* cell = array->references; cell; cell = cell->next)
        if (cell->index == native) return llg_element_cell_share(cell);
    struct llg_element_cell* cell = llg_element_cell_new(
        array, LLG_ELEMENT_CELL_DYNAMIC, array->element_width,
        array->element_signed, array->element_two_state);
    cell->index = native;
    cell->next = array->references;
    array->references = cell;
    return cell;
}

void* llg_assoc_ref_acquire_integral(llg_assoc_t* array, sv4_t key) {
    if (!array) llg_container_fatal("null associative-array reference source");
    sv4_t normalized = SV4_EMPTY;
    if (!llg_assoc_normalize_key(array, key, &normalized)) {
        struct llg_element_cell* cell = llg_element_cell_new(
            NULL, LLG_ELEMENT_CELL_ASSOC_INTEGRAL, array->element_width,
            array->element_signed, array->element_two_state);
        cell->valid = 0;
        return cell;
    }
    for (struct llg_element_cell* cell = array->references; cell; cell = cell->next) {
        if (llg_integral_compare(cell->key, normalized) == 0) {
            sv4_destroy(&normalized);
            return llg_element_cell_share(cell);
        }
    }
    struct llg_element_cell* cell = llg_element_cell_new(
        array, LLG_ELEMENT_CELL_ASSOC_INTEGRAL, array->element_width,
        array->element_signed, array->element_two_state);
    sv4_move(&cell->key, &normalized);
    cell->next = array->references;
    array->references = cell;
    return cell;
}

void* llg_assoc_ref_acquire_string(llg_assoc_t* array, const void* key,
                                   size_t key_length) {
    if (!array) llg_container_fatal("null associative-array reference source");
    llg_check_string_key(array, key, key_length);
    for (struct llg_element_cell* cell = array->references; cell; cell = cell->next) {
        if (llg_assoc_string_compare(cell->string_key, cell->string_length, key,
                                     key_length) == 0)
            return llg_element_cell_share(cell);
    }
    struct llg_element_cell* cell = llg_element_cell_new(
        array, LLG_ELEMENT_CELL_ASSOC_STRING, array->element_width,
        array->element_signed, array->element_two_state);
    if (key_length) {
        cell->string_key = llg_alloc_items(key_length, 1);
        memcpy(cell->string_key, key, key_length);
    }
    cell->string_length = key_length;
    cell->next = array->references;
    array->references = cell;
    return cell;
}

static void llg_element_cell_unlink(struct llg_element_cell* cell) {
    struct llg_element_cell** link = llg_element_cell_list(cell);
    while (*link && *link != cell) link = &(*link)->next;
    if (*link) *link = cell->next;
    cell->next = NULL;
    cell->owner = NULL;
}

void llg_element_ref_release(void* ptr) {
    struct llg_element_cell* cell = ptr;
    if (!cell) return;
    if (!cell->refs) llg_container_fatal("element reference count underflow");
    if (--cell->refs) return;
    if (cell->owner) llg_element_cell_unlink(cell);
    sv4_destroy(&cell->key);
    sv4_destroy(&cell->value);
    free(cell->string_key);
    free(cell);
}

static void llg_dyn_outdate_references(llg_dyn_array_t* array) {
    while (array->references) {
        struct llg_element_cell* cell = array->references;
        if (cell->index < array->size) sv4_copy(&cell->value, &array->data[cell->index]);
        llg_element_cell_unlink(cell);
    }
}

static void llg_assoc_outdate_references(llg_assoc_t* array, size_t position) {
    struct llg_element_cell** link = &array->references;
    while (*link) {
        struct llg_element_cell* cell = *link;
        const llg_assoc_entry_t* entry = NULL;
        int found = 0;
        size_t at = cell->kind == LLG_ELEMENT_CELL_ASSOC_STRING
                        ? llg_assoc_string_position(array, cell->string_key,
                                                    cell->string_length, &found)
                        : llg_assoc_integral_position(array, cell->key, &found);
        if (found) entry = &array->entries[at];
        if (position != SIZE_MAX && (!found || at != position)) {
            link = &cell->next;
            continue;
        }
        if (entry) sv4_copy(&cell->value, &entry->value);
        *link = cell->next;
        cell->next = NULL;
        cell->owner = NULL;
    }
}

sv4_t llg_element_cell_read(const void* ptr) {
    const struct llg_element_cell* cell = ptr;
    if (!cell) llg_container_fatal("null retained element reference");
    if (!cell->owner) return sv4_clone(&cell->value);
    switch (cell->kind) {
    case LLG_ELEMENT_CELL_DYNAMIC: {
        const llg_dyn_array_t* array = cell->owner;
        return sv4_clone(&array->data[cell->index]);
    }
    case LLG_ELEMENT_CELL_ASSOC_INTEGRAL:
        return llg_assoc_get_integral(cell->owner, cell->key);
    default:
        return llg_assoc_get_string(cell->owner, cell->string_key, cell->string_length);
    }
}

int llg_element_cell_write(void* ptr, sv4_t value) {
    struct llg_element_cell* cell = ptr;
    if (!cell) llg_container_fatal("null retained element reference");
    if (!cell->valid) return 0;
    if (!cell->owner) {
        sv4_replace(&cell->value, llg_element_assign(value, cell->element_width, cell->element_signed,
                                                     cell->element_two_state));
        return 1;
    }
    switch (cell->kind) {
    case LLG_ELEMENT_CELL_DYNAMIC: {
        llg_dyn_array_t* array = cell->owner;
        sv4_t assigned = llg_element_assign(value, array->element_width,
                                            array->element_signed,
                                            array->element_two_state);
        if (sv4_same(array->data[cell->index], assigned)) {
            sv4_destroy(&assigned);
            return 1;
        }
        sv4_move(&array->data[cell->index], &assigned);
        llg_notify(array->notify, array->contents_dependency,
                   array->shape_dependency, LLG_CONTAINER_CHANGED_CONTENTS);
        return 1;
    }
    case LLG_ELEMENT_CELL_ASSOC_INTEGRAL:
        return llg_assoc_set_integral(cell->owner, cell->key, value);
    default:
        return llg_assoc_set_string(cell->owner, cell->string_key, cell->string_length,
                                    value);
    }
}
