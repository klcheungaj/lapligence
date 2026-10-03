/* Lazy fixed cells never move: queued writes and selected dependencies retain
 * descriptor identity. Unvisited cells share only a read-only default. */
struct llg_fixed_inertial {
    const void* site;
    llg_inertial_t* handle;
    struct llg_fixed_inertial* next;
};

struct llg_fixed_cell {
    uint64_t index;
    sv4_t value;
    llg_fixed_array_t* array;
    struct llg_fixed_inertial* inertial;
    struct llg_fixed_cell *bucket_next, *next, *address_next;
};

static _Noreturn void fixed_bad_state(const char* reason) {
    fprintf(stderr, "llg runtime fatal: %s\n", reason);
    abort();
}

#define LLG_FIXED_INDEX_INITIAL_CAPACITY 16u
static llg_fixed_cell_t** fixed_addresses;
static size_t fixed_address_capacity, fixed_address_count;

static size_t fixed_hash(uint64_t key) {
    key ^= key >> 30;
    key *= UINT64_C(0xbf58476d1ce4e5b9);
    key ^= key >> 27;
    key *= UINT64_C(0x94d049bb133111eb);
    return (size_t)(key ^ (key >> 31));
}

static size_t fixed_grow(size_t capacity) {
    if (capacity > SIZE_MAX / 2) llg_fatal_allocation("fixed array index overflow", capacity, 2);
    return capacity ? capacity * 2 : LLG_FIXED_INDEX_INITIAL_CAPACITY;
}

static void fixed_address_reserve(void) {
    if (fixed_address_count < fixed_address_capacity / 2) return;
    size_t capacity = fixed_grow(fixed_address_capacity);
    llg_fixed_cell_t** buckets = llg_checked_calloc(capacity, sizeof(*buckets), "fixed cell address index");
    for (size_t i = 0; i < fixed_address_capacity; ++i) {
        llg_fixed_cell_t* cell = fixed_addresses[i];
        while (cell) {
            llg_fixed_cell_t* next = cell->address_next;
            size_t slot = fixed_hash((uint64_t)(uintptr_t)&cell->value) & (capacity - 1);
            cell->address_next = buckets[slot];
            buckets[slot] = cell;
            cell = next;
        }
    }
    free(fixed_addresses);
    fixed_addresses = buckets;
    fixed_address_capacity = capacity;
}

static void fixed_array_changed(sv4_t* target) {
    if (!fixed_address_capacity) return;
    size_t slot = fixed_hash((uint64_t)(uintptr_t)target) & (fixed_address_capacity - 1);
    for (llg_fixed_cell_t* cell = fixed_addresses[slot]; cell; cell = cell->address_next) {
        if (&cell->value == target) {
            llg_dependency_changed(cell->array->contents);
            return;
        }
    }
}

llg_inertial_t** llg_fixed_array_inertial(sv4_t* target, const void* site) {
    if (fixed_address_capacity) {
        size_t slot = fixed_hash((uint64_t)(uintptr_t)target) & (fixed_address_capacity - 1);
        for (llg_fixed_cell_t* cell = fixed_addresses[slot]; cell; cell = cell->address_next) {
            if (&cell->value != target) continue;
            for (struct llg_fixed_inertial* entry = cell->inertial; entry; entry = entry->next)
                if (entry->site == site) return &entry->handle;
            struct llg_fixed_inertial* entry = llg_checked_calloc(1, sizeof(*entry), "fixed inertial site");
            entry->site = site;
            entry->next = cell->inertial;
            cell->inertial = entry;
            return &entry->handle;
        }
    }
    fixed_bad_state("inertial target is not a fixed cell");
}

static llg_fixed_cell_t* fixed_find(const llg_fixed_array_t* array, uint64_t index) {
    if (!array->capacity) return NULL;
    size_t slot = fixed_hash(index) & (array->capacity - 1);
    for (llg_fixed_cell_t* cell = array->buckets[slot]; cell; cell = cell->bucket_next)
        if (cell->index == index) return cell;
    return NULL;
}

void llg_fixed_array_init(llg_fixed_array_t* array, uint64_t total,
                          sv4_t initial, sv4_t* contents) {
    if (!array || !total || array->total || !llg_sv4_width(initial))
        fixed_bad_state("invalid fixed array initialization");
    array->total = total;
    array->initial = initial; /* Consumes the fresh default owner. */
    array->contents = contents;
}

const sv4_t* llg_fixed_array_peek(const llg_fixed_array_t* array, uint64_t index) {
    if (!array || index >= array->total) fixed_bad_state("invalid fixed array index");
    llg_fixed_cell_t* cell = fixed_find(array, index);
    return cell ? &cell->value : &array->initial;
}

sv4_t* llg_fixed_array_cell(llg_fixed_array_t* array, uint64_t index) {
    if (!array || index >= array->total) fixed_bad_state("invalid fixed array index");
    llg_fixed_cell_t* cell = fixed_find(array, index);
    if (cell) return &cell->value;
    if (array->count >= array->capacity / 2) {
        size_t capacity = fixed_grow(array->capacity);
        llg_fixed_cell_t** buckets = llg_checked_calloc(capacity, sizeof(*buckets), "fixed array index");
        for (cell = array->cells; cell; cell = cell->next) {
            size_t slot = fixed_hash(cell->index) & (capacity - 1);
            cell->bucket_next = buckets[slot];
            buckets[slot] = cell;
        }
        free(array->buckets);
        array->buckets = buckets;
        array->capacity = capacity;
    }
    fixed_address_reserve();
    cell = llg_checked_calloc(1, sizeof(*cell), "fixed array cell");
    cell->index = index;
    cell->array = array;
    cell->value = sv4_clone(&array->initial);
    size_t slot = fixed_hash(index) & (array->capacity - 1);
    cell->bucket_next = array->buckets[slot];
    array->buckets[slot] = cell;
    cell->next = array->cells;
    array->cells = cell;
    ++array->count;
    slot = fixed_hash((uint64_t)(uintptr_t)&cell->value) & (fixed_address_capacity - 1);
    cell->address_next = fixed_addresses[slot];
    fixed_addresses[slot] = cell;
    ++fixed_address_count;
    return &cell->value;
}

void llg_fixed_array_reset(llg_fixed_array_t* array, sv4_t initial) {
    for (llg_fixed_cell_t* cell = array->cells; cell; cell = cell->next)
        sv4_copy(&cell->value, &initial);
    sv4_replace(&array->initial, initial);
}

void llg_fixed_array_destroy(void* object) {
    llg_fixed_array_t* array = object;
    llg_fixed_cell_t* cell = array->cells;
    while (cell) {
        llg_fixed_cell_t* next = cell->next;
        size_t slot = fixed_hash((uint64_t)(uintptr_t)&cell->value) & (fixed_address_capacity - 1);
        llg_fixed_cell_t** link = &fixed_addresses[slot];
        while (*link != cell) link = &(*link)->address_next;
        *link = cell->address_next;
        --fixed_address_count;
        llg_clocking_forget_signal(&cell->value);
        sv4_destroy(&cell->value);
        while (cell->inertial) {
            struct llg_fixed_inertial* next = cell->inertial->next;
            free(cell->inertial);
            cell->inertial = next;
        }
        free(cell);
        cell = next;
    }
    if (!fixed_address_count) {
        free(fixed_addresses);
        fixed_addresses = NULL;
        fixed_address_capacity = 0;
    }
    free(array->buckets);
    sv4_destroy(&array->initial);
    *array = (llg_fixed_array_t){0};
}

static void fixed_array_apply(llg_fixed_array_t* dst, const llg_fixed_array_t* snapshot) {
    for (llg_fixed_cell_t* cell = snapshot->cells; cell; cell = cell->next)
        (void)llg_fixed_array_cell(dst, cell->index);
    int default_changed = !sv4_same(dst->initial, snapshot->initial);
    sv4_copy(&dst->initial, &snapshot->initial);
    for (llg_fixed_cell_t* cell = dst->cells; cell; cell = cell->next)
        llg_ba(&cell->value, *llg_fixed_array_peek(snapshot, cell->index));
    if (default_changed) llg_dependency_changed(dst->contents);
}

void llg_fixed_array_stream_copy(llg_fixed_array_t* dst, const llg_fixed_array_t* src,
                          int two_state, int nba, uint32_t slice) {
    if (!region_can_mutate("fixed array write")) return;
    llg_value_scope_t* target_pin = value_target_pin(dst);
    if (dst->total != src->total) fixed_bad_state("fixed array copy shape mismatch");
    uint32_t width = llg_sv4_width(src->initial);
    if (slice && width % slice && slice % width) fixed_bad_state("unaligned fixed stream slice");
    llg_value_scope_t* scope = llg_value_scope_begin_object(sizeof(llg_fixed_array_t), llg_fixed_array_destroy);
    llg_fixed_array_t* snapshot = llg_value_scope_object(scope);
    sv4_t initial = sv4_cast(src->initial, llg_sv4_width(dst->initial), llg_sv4_signed(dst->initial));
    if (slice && slice < width) sv4_replace(&initial, sv4_stream(initial, slice, 1));
    if (two_state) {
        sv4_t converted = sv4_to_two_state(initial);
        sv4_move(&initial, &converted);
    }
    llg_fixed_array_init(snapshot, src->total, initial, NULL);
    for (llg_fixed_cell_t* cell = src->cells; cell; cell = cell->next) {
        sv4_t value = sv4_cast(cell->value, llg_sv4_width(dst->initial), llg_sv4_signed(dst->initial));
        if (slice && slice < width) sv4_replace(&value, sv4_stream(value, slice, 1));
        if (two_state) {
            sv4_t converted = sv4_to_two_state(value);
            sv4_move(&value, &converted);
        }
        uint64_t index = cell->index;
        if (slice) {
            uint64_t group = slice <= width ? 1 : slice / width;
            uint64_t low = src->total - 1 - index;
            uint64_t base = low / group * group;
            uint64_t count = src->total - base < group ? src->total - base : group;
            index = base + count - 1 - low % group;
        }
        if (!sv4_same(value, snapshot->initial))
            sv4_move(llg_fixed_array_cell(snapshot, index), &value);
        sv4_destroy(&value);
    }
    /* Allocate all missing destinations before the first publication. */
    for (llg_fixed_cell_t* cell = snapshot->cells; cell; cell = cell->next)
        (void)llg_fixed_array_cell(dst, cell->index);
    if (nba) {
        llg_nba_t* update = new_nba(0);
        if (update) {
            update->fixed_target = dst;
            update->target_scope = value_scope_retain_target(dst);
            update->fixed_value = llg_checked_calloc(1, sizeof(*snapshot), "fixed array NBA snapshot");
            llg_fixed_array_init(update->fixed_value, snapshot->total, sv4_clone(&snapshot->initial), NULL);
            for (llg_fixed_cell_t* cell = snapshot->cells; cell; cell = cell->next)
                sv4_copy(llg_fixed_array_cell(update->fixed_value, cell->index), &cell->value);
            enqueue_nba(update);
        }
    } else {
        fixed_array_apply(dst, snapshot);
    }
    llg_value_scope_end(scope);
    if (target_pin) llg_value_scope_end(target_pin);
}

static int fixed_compare_leaf(const sv4_t* left, const sv4_t* right, int case_eq) {
    sv4_t result = case_eq ? sv4_case_eq(*left, *right) : sv4_eq(*left, *right);
    int state = llg_sv4_state(result, 0);
    sv4_destroy(&result);
    return state;
}

sv4_t llg_fixed_array_compare(const llg_fixed_array_t* left,
                             const llg_fixed_array_t* right, int case_eq, int negate) {
    if (left->total != right->total) fixed_bad_state("fixed comparison shape mismatch");
    int result = 1;
    size_t represented = left->count;
    for (llg_fixed_cell_t* cell = left->cells; cell; cell = cell->next) {
        int state = fixed_compare_leaf(&cell->value, llg_fixed_array_peek(right, cell->index), case_eq);
        if (!state) return sv4_from_u64((uint64_t)negate, 1, 0);
        if (state > 1) result = 2;
    }
    for (llg_fixed_cell_t* cell = right->cells; cell; cell = cell->next) {
        if (fixed_find(left, cell->index)) continue;
        ++represented;
        int state = fixed_compare_leaf(&left->initial, &cell->value, case_eq);
        if (!state) return sv4_from_u64((uint64_t)negate, 1, 0);
        if (state > 1) result = 2;
    }
    if (represented < left->total) {
        int state = fixed_compare_leaf(&left->initial, &right->initial, case_eq);
        if (!state) return sv4_from_u64((uint64_t)negate, 1, 0);
        if (state > 1) result = 2;
    }
    return result > 1 ? sv4_x(1, 0) : sv4_from_u64((uint64_t)(result ^ !!negate), 1, 0);
}

void llg_fixed_array_copy(llg_fixed_array_t* dst, const llg_fixed_array_t* src,
                          int two_state, int nba) {
    llg_fixed_array_stream_copy(dst, src, two_state, nba, 0);
}

void llg_fixed_array_fill(llg_fixed_array_t* dst, sv4_t value, int two_state, int nba) {
    llg_value_scope_t* scope = llg_value_scope_begin_object(sizeof(llg_fixed_array_t), llg_fixed_array_destroy);
    llg_fixed_array_t* source = llg_value_scope_object(scope);
    llg_fixed_array_init(source, dst->total, sv4_clone(&value), NULL);
    llg_fixed_array_copy(dst, source, two_state, nba);
    llg_value_scope_end(scope);
}
