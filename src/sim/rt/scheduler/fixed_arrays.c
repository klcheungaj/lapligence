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

static sv4_t fixed_read(const llg_fixed_array_t*, uint64_t);
static void fixed_ranges_destroy(llg_fixed_range_t*);
static llg_fixed_range_t* fixed_ranges_copy(const llg_fixed_array_t*, uint64_t, uint64_t, uint64_t, int);
static void fixed_array_snapshot(llg_fixed_array_t*, const llg_fixed_array_t*, int);
static void fixed_array_apply(llg_fixed_array_t*, const llg_fixed_array_t*);

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
    if (array->owner) {
        if (array->origin == UINT64_MAX) return &array->initial;
        return llg_fixed_array_peek(array->owner, array->origin + index);
    }
    llg_fixed_cell_t* cell = fixed_find(array, index);
    if (!cell && array->ranges) return llg_fixed_array_cell((llg_fixed_array_t*)array, index);
    return cell ? &cell->value : &array->initial;
}

sv4_t* llg_fixed_array_cell(llg_fixed_array_t* array, uint64_t index) {
    if (!array || index >= array->total) fixed_bad_state("invalid fixed array index");
    if (array->owner) {
        if (array->origin == UINT64_MAX) fixed_bad_state("invalid writable fixed view");
        return llg_fixed_array_cell(array->owner, array->origin + index);
    }
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
    cell->value = fixed_read(array, index);
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
    fixed_ranges_destroy(array->ranges);
    array->ranges = NULL;
    for (llg_fixed_cell_t* cell = array->cells; cell; cell = cell->next)
        sv4_copy(&cell->value, &initial);
    sv4_replace(&array->initial, initial);
}

void llg_fixed_array_destroy(void* object) {
    llg_fixed_array_t* array = object;
    fixed_ranges_destroy(array->ranges);
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
            struct llg_fixed_inertial* inertial_next = cell->inertial->next;
            free(cell->inertial);
            cell->inertial = inertial_next;
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


static void fixed_array_simple_stream_copy(llg_fixed_array_t* dst, const llg_fixed_array_t* src,
                          int two_state, int nba, uint32_t slice) {
    if (region_is_read_only_now(g.current_region) && !region_private_store("fixed array write"))
        return;
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

/* Runtime `with` source over descriptor storage: reads each selected cell
 * through `peek`, so untouched cells are not materialized by the stream. */
sv4_t llg_fixed_array_stream_source(const llg_fixed_array_t* array,
                                    int64_t declaration_left, int64_t declaration_right,
                                    uint32_t element_width, sv4_t fallback,
                                    int selector_kind, sv4_t first, sv4_t second) {
    int64_t left;
    int64_t right;
    size_t count;
    llg_fixed_stream_bounds(selector_kind, first, second, declaration_left,
                            declaration_right, &left, &right, &count);
    uint32_t width = count ? llg_fixed_stream_width(selector_kind, first, second, element_width) : 0;
    if (!width) {
        sv4_t empty = SV4_EMPTY;
        return empty;
    }
    sv4_t packed = sv4_zero(width, 0);
    uint32_t cursor = width;
    for (size_t i = 0; i < count; ++i) {
        int64_t offset = llg_fixed_stream_storage_offset(
            declaration_left, declaration_right, llg_fixed_stream_index_at(left, right, i));
        sv4_part_select_set(&packed, (int64_t)cursor - 1, (int64_t)(cursor - element_width),
                            offset < 0 ? fallback : *llg_fixed_array_peek(array, (uint64_t)offset));
        cursor -= element_width;
    }
    return packed;
}

sv4_t llg_fixed_array_compare(const llg_fixed_array_t* left,
                             const llg_fixed_array_t* right, int case_eq, int negate) {
    if (left->total != right->total) fixed_bad_state("fixed comparison shape mismatch");
    if (left->owner || right->owner || left->ranges || right->ranges) {
        int result = 1;
        for (uint64_t index = 0; index < left->total; ++index) {
            sv4_t a = fixed_read(left, index), b = fixed_read(right, index);
            int state = fixed_compare_leaf(&a, &b, case_eq);
            sv4_destroy(&a); sv4_destroy(&b);
            if (!state) return sv4_from_u64((uint64_t)negate, 1, 0);
            if (state > 1) result = 2;
        }
        return result > 1 ? sv4_x(1, 0) : sv4_from_u64((uint64_t)(result ^ !!negate), 1, 0);
    }
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

struct llg_fixed_range {
    uint64_t start, count, source;
    int two_state;
    sv4_t value;
    llg_fixed_image_t* image;
    llg_fixed_range_t* next;
};

struct llg_fixed_image {
    size_t refs, count;
    llg_fixed_array_t* sources;
    uint64_t bit_count, element_cells;
    uint32_t slice, cell_width;
    int two_state, merge;
};

static void fixed_image_release(llg_fixed_image_t* image) {
    if (!image || --image->refs) return;
    for (size_t i = 0; i < image->count; ++i) llg_fixed_array_destroy(&image->sources[i]);
    free(image->sources);
    free(image);
}

static void fixed_ranges_destroy(llg_fixed_range_t* range) {
    while (range) {
        llg_fixed_range_t* next = range->next;
        sv4_destroy(&range->value);
        fixed_image_release(range->image);
        free(range);
        range = next;
    }
}

static sv4_t fixed_convert(sv4_t value, uint32_t width, int sign, int two_state) {
    sv4_t result = sv4_cast(value, width, (int8_t)sign);
    if (two_state) sv4_replace(&result, sv4_to_two_state(result));
    return result;
}

static sv4_t fixed_image_read(const llg_fixed_image_t* image, uint64_t index) {
    if (image->merge) {
        uint64_t first = index / image->element_cells * image->element_cells;
        int equal = 1;
        for (uint64_t i = 0; i < image->element_cells; ++i) {
            sv4_t left = fixed_read(&image->sources[0], first + i);
            sv4_t right = fixed_read(&image->sources[1], first + i);
            int state = fixed_compare_leaf(&left, &right, 0);
            sv4_destroy(&left); sv4_destroy(&right);
            if (state != 1) { equal = 0; break; }
        }
        if (!equal) return image->two_state ? sv4_zero(image->cell_width, 0) : sv4_x(image->cell_width, 0);
        return fixed_read(&image->sources[0], index);
    }
    sv4_t result = sv4_zero(image->cell_width, 0);
    sv4_t cached = SV4_EMPTY;
    const llg_fixed_array_t* cached_source = NULL;
    uint64_t cached_index = UINT64_MAX;
    for (uint32_t bit = 0; bit < image->cell_width; ++bit) {
        uint64_t position = index * image->cell_width + (image->cell_width - 1 - bit);
        if (position >= image->bit_count) continue;
        if (image->slice) {
            uint64_t first_size = image->bit_count % image->slice;
            if (!first_size) first_size = image->slice;
            position = position < first_size ? image->bit_count - first_size + position
                : image->bit_count - first_size - ((position - first_size) / image->slice + 1) * image->slice
                    + (position - first_size) % image->slice;
        }
        const llg_fixed_array_t* source = NULL;
        uint64_t local = position;
        uint32_t width = 0;
        for (size_t i = 0; i < image->count; ++i) {
            width = llg_sv4_width(image->sources[i].initial);
            uint64_t bits = image->sources[i].total * width;
            if (local < bits) { source = &image->sources[i]; break; }
            local -= bits;
        }
        if (!source) fixed_bad_state("fixed stream source cursor overflow");
        uint64_t cell = local / width;
        if (cached_source != source || cached_index != cell) {
            sv4_replace(&cached, fixed_read(source, cell));
            cached_source = source;
            cached_index = cell;
        }
        unsigned state = llg_sv4_state(cached, width - 1 - local % width);
        llg_sv4_set_state(&result, bit, image->two_state && state > 1 ? 0 : state);
    }
    sv4_destroy(&cached);
    return result;
}

static sv4_t fixed_read(const llg_fixed_array_t* array, uint64_t index) {
    if (index >= array->total) fixed_bad_state("fixed read exceeds view");
    if (array->owner) {
        return array->origin == UINT64_MAX ? sv4_clone(&array->initial)
            : fixed_read(array->owner, array->origin + index);
    }
    llg_fixed_cell_t* cell = fixed_find(array, index);
    if (cell) return sv4_clone(&cell->value);
    for (const llg_fixed_range_t* range = array->ranges; range; range = range->next) {
        if (index < range->start || index - range->start >= range->count) continue;
        sv4_t value = range->image ? fixed_image_read(range->image, range->source + index - range->start)
            : sv4_clone(&range->value);
        sv4_t converted = fixed_convert(value, llg_sv4_width(array->initial), llg_sv4_signed(array->initial), range->two_state);
        sv4_destroy(&value);
        return converted;
    }
    return sv4_clone(&array->initial);
}

static llg_fixed_range_t* fixed_range_clone(const llg_fixed_range_t* range, uint64_t start, uint64_t count, uint64_t source, int two_state) {
    llg_fixed_range_t* result = llg_checked_calloc(1, sizeof(*result), "fixed value range");
    result->start = start; result->count = count; result->source = source;
    result->two_state = two_state || range->two_state;
    result->value = sv4_clone(&range->value);
    result->image = range->image;
    if (result->image) {
        if (result->image->refs == SIZE_MAX) llg_fatal_allocation("fixed image references", SIZE_MAX, 1);
        ++result->image->refs;
    }
    return result;
}

/* Earlier ranges take precedence, so lists keep their order. A range wholly
 * shadowed by an earlier one is dropped; repeated row copies therefore keep
 * the list bounded by the number of distinct intervals instead of growing. */
static void fixed_range_append(llg_fixed_range_t** head, llg_fixed_range_t*** tail, llg_fixed_range_t* range) {
    for (const llg_fixed_range_t* earlier = *head; earlier; earlier = earlier->next) {
        if (earlier->start <= range->start
            && range->start + range->count <= earlier->start + earlier->count) {
            range->next = NULL;
            fixed_ranges_destroy(range);
            return;
        }
    }
    range->next = NULL;
    **tail = range;
    *tail = &range->next;
}

static llg_fixed_range_t* fixed_ranges_copy(const llg_fixed_array_t* array, uint64_t first, uint64_t count, uint64_t target, int two_state) {
    llg_fixed_range_t* result = NULL;
    llg_fixed_range_t** tail = &result;
    for (const llg_fixed_range_t* range = array->ranges; range; range = range->next) {
        uint64_t begin = range->start > first ? range->start : first;
        uint64_t end = range->start + range->count < first + count ? range->start + range->count : first + count;
        if (begin >= end) continue;
        fixed_range_append(&result, &tail, fixed_range_clone(range, target + begin - first, end - begin, range->source + begin - range->start, two_state));
    }
    return result;
}

void llg_fixed_array_view_init(llg_fixed_array_t* view, llg_fixed_array_t* owner, uint64_t origin, uint64_t total, int two_state) {
    if (!view || !owner || !total || view->total) fixed_bad_state("invalid fixed view initialization");
    llg_fixed_array_init(view, total, two_state ? sv4_zero(llg_sv4_width(owner->initial), llg_sv4_signed(owner->initial))
        : sv4_x(llg_sv4_width(owner->initial), llg_sv4_signed(owner->initial)), NULL);
    view->owner = owner->owner ? owner->owner : owner;
    view->origin = origin == UINT64_MAX || owner->origin == UINT64_MAX || origin > owner->total || total > owner->total - origin
        ? UINT64_MAX : origin + owner->origin;
}

static void fixed_array_snapshot(llg_fixed_array_t* result, const llg_fixed_array_t* source, int two_state) {
    const llg_fixed_array_t* owner = source->owner ? source->owner : source;
    uint64_t origin = source->owner ? source->origin : 0;
    sv4_t initial = fixed_convert(origin == UINT64_MAX ? source->initial : owner->initial,
        llg_sv4_width(source->initial), llg_sv4_signed(source->initial), two_state);
    llg_fixed_array_init(result, source->total, initial, NULL);
    if (origin == UINT64_MAX) return;
    result->ranges = fixed_ranges_copy(owner, origin, source->total, 0, two_state);
    for (llg_fixed_cell_t* cell = owner->cells; cell; cell = cell->next) {
        if (cell->index < origin || cell->index - origin >= source->total) continue;
        sv4_t value = fixed_convert(cell->value, llg_sv4_width(initial), llg_sv4_signed(initial), two_state);
        sv4_move(llg_fixed_array_cell(result, cell->index - origin), &value);
    }
}

struct fixed_publication { sv4_t* target; sv4_t old, value, replacement; };
struct fixed_transaction { size_t count; struct fixed_publication* cells; llg_fixed_range_t* ranges; sv4_t initial; };

static void fixed_transaction_destroy(void* object) {
    struct fixed_transaction* transaction = object;
    for (size_t i = 0; i < transaction->count; ++i) {
        sv4_destroy(&transaction->cells[i].old);
        sv4_destroy(&transaction->cells[i].value);
        sv4_destroy(&transaction->cells[i].replacement);
    }
    free(transaction->cells);
    fixed_ranges_destroy(transaction->ranges);
    sv4_destroy(&transaction->initial);
}

static void fixed_array_apply(llg_fixed_array_t* destination, const llg_fixed_array_t* snapshot) {
    llg_fixed_array_t* dst = destination->owner ? destination->owner : destination;
    uint64_t origin = destination->owner ? destination->origin : 0;
    if (origin == UINT64_MAX) return;
    if (destination->total != snapshot->total) fixed_bad_state("fixed publication shape mismatch");
    llg_value_scope_t* scope = llg_value_scope_begin_object(sizeof(struct fixed_transaction), fixed_transaction_destroy);
    struct fixed_transaction* prepared = llg_value_scope_object(scope);
    for (llg_fixed_cell_t* cell = snapshot->cells; cell; cell = cell->next)
        (void)llg_fixed_array_cell(dst, origin + cell->index);
    prepared->ranges = fixed_ranges_copy(snapshot, 0, snapshot->total, origin, 0);
    if (origin || destination->total != dst->total) {
        llg_fixed_range_t** tail = &prepared->ranges;
        while (*tail) tail = &(*tail)->next;
        /* Source ranges take precedence over this interval's uniform default. */
        llg_fixed_range_t uniform = {0}; uniform.value = snapshot->initial;
        fixed_range_append(&prepared->ranges, &tail, fixed_range_clone(&uniform, origin, snapshot->total, 0, 0));
        /* Retained ranges lie outside the interval; their relative order is kept. */
        for (llg_fixed_range_t* old = dst->ranges; old; old = old->next) {
            uint64_t end = old->start + old->count, limit = origin + snapshot->total;
            uint64_t before = end < origin ? end : origin;
            if (before > old->start)
                fixed_range_append(&prepared->ranges, &tail, fixed_range_clone(old, old->start, before - old->start, old->source, 0));
            uint64_t after = old->start > limit ? old->start : limit;
            if (after < end)
                fixed_range_append(&prepared->ranges, &tail, fixed_range_clone(old, after, end - after, old->source + after - old->start, 0));
        }
        prepared->initial = sv4_clone(&dst->initial);
    } else prepared->initial = sv4_clone(&snapshot->initial);
    prepared->cells = llg_checked_calloc(dst->count, sizeof(*prepared->cells), "fixed publication values");
    for (llg_fixed_cell_t* cell = dst->cells; cell; cell = cell->next) {
        if (cell->index < origin || cell->index - origin >= snapshot->total) continue;
        struct fixed_publication* entry = &prepared->cells[prepared->count++];
        entry->target = &cell->value;
        entry->old = sv4_clone(&cell->value);
        entry->value = fixed_read(snapshot, cell->index - origin);
        entry->replacement = sv4_clone(&entry->value);
    }
    int default_changed = !sv4_same(dst->initial, prepared->initial) || dst->ranges || prepared->ranges;
    /* Every owner and publication record is prepared before any value becomes visible. */
    sv4_move(&dst->initial, &prepared->initial);
    llg_fixed_range_t* old_ranges = dst->ranges;
    dst->ranges = prepared->ranges; prepared->ranges = old_ranges;
    for (size_t i = 0; i < prepared->count; ++i)
        sv4_move(prepared->cells[i].target, &prepared->cells[i].replacement);
    /* Observer callbacks run only after the complete image has been committed.
     * A private Postponed evaluation stores helper-owned cells unpublished. */
    for (size_t i = 0; i < prepared->count && !g.private_evaluation; ++i) {
        struct fixed_publication* entry = &prepared->cells[i];
        if (!sv4_same(entry->old, entry->value)) sig_publish_changed(entry->target, entry->old, entry->value, entry->value);
    }
    if (default_changed) llg_dependency_changed(dst->contents);
    llg_value_scope_end(scope);
}

static void fixed_snapshot_publish(llg_fixed_array_t* dst, llg_fixed_array_t* snapshot, int nba) {
    if (dst->owner && dst->origin == UINT64_MAX) return;
    if (nba) {
        llg_nba_t* update = new_nba(0);
        if (update) {
            update->fixed_target = dst;
            update->target_scope = value_scope_retain_target(dst);
            update->fixed_value = llg_checked_calloc(1, sizeof(*snapshot), "fixed array NBA image");
            *update->fixed_value = *snapshot;
            /* Snapshot cell identities are not exposed; keep their reverse-index owner accurate. */
            for (llg_fixed_cell_t* cell = snapshot->cells; cell; cell = cell->next) cell->array = update->fixed_value;
            *snapshot = (llg_fixed_array_t){0};
            enqueue_nba(update);
        }
    } else fixed_array_apply(dst, snapshot);
}

/* Snapshot `count` stream sources into one lazily read bit image whose cells
 * are `cell_width` bits; cell bits past the sources read zero. */
static llg_fixed_image_t* fixed_stream_image(const llg_fixed_array_t* const* sources, size_t count, uint32_t slice, uint32_t cell_width, int two_state) {
    llg_fixed_image_t* image = llg_checked_calloc(1, sizeof(*image), "fixed stream image");
    image->refs = 1; image->count = count; image->slice = slice;
    image->cell_width = cell_width; image->two_state = two_state;
    image->sources = llg_checked_calloc(count, sizeof(*image->sources), "fixed stream sources");
    for (size_t i = 0; i < count; ++i) {
        fixed_array_snapshot(&image->sources[i], sources[i], 0);
        uint64_t width = llg_sv4_width(sources[i]->initial);
        if (sources[i]->total > (UINT64_MAX - image->bit_count) / width) fixed_bad_state("fixed stream width overflow");
        image->bit_count += sources[i]->total * width;
    }
    return image;
}

/* Every operand may be empty (a runtime `with` selection of no elements), so
 * a stream of no sources publishes zeros: the empty stream is left-justified
 * and zero-filled (SV 11.4.14). */
void llg_fixed_array_stream_segments(llg_fixed_array_t* dst, const llg_fixed_array_t* const* sources, size_t count, int two_state, int nba, uint32_t slice) {
    if (region_is_read_only_now(g.current_region) && !region_private_store("fixed array stream"))
        return;
    llg_value_scope_t* pin = value_target_pin(dst);
    llg_value_scope_t* scope = llg_value_scope_begin_object(sizeof(llg_fixed_array_t), llg_fixed_array_destroy);
    llg_fixed_array_t* snapshot = llg_value_scope_object(scope);
    if (!count) {
        llg_fixed_array_init(snapshot, dst->total, sv4_zero(llg_sv4_width(dst->initial), llg_sv4_signed(dst->initial)), NULL);
    } else if (count == 1 && !slice && dst->total == sources[0]->total && llg_sv4_width(dst->initial) == llg_sv4_width(sources[0]->initial)) {
        fixed_array_snapshot(snapshot, sources[0], two_state);
    } else {
        llg_fixed_image_t* image = fixed_stream_image(sources, count, slice, llg_sv4_width(dst->initial), two_state);
        if (dst->total > UINT64_MAX / image->cell_width || image->bit_count > dst->total * image->cell_width) {
            fixed_image_release(image);
            fixed_bad_state("fixed stream exceeds destination");
        }
        llg_fixed_array_init(snapshot, dst->total, sv4_zero(image->cell_width, llg_sv4_signed(dst->initial)), NULL);
        llg_fixed_range_t* range = llg_checked_calloc(1, sizeof(*range), "fixed stream range");
        range->count = dst->total; range->image = image; snapshot->ranges = range;
    }
    fixed_snapshot_publish(dst, snapshot, nba);
    llg_value_scope_end(scope);
    if (pin) llg_value_scope_end(pin);
}

static uint32_t fixed_gcd(uint32_t a, uint32_t b) {
    while (b) { uint32_t t = a % b; a = b; b = t; }
    return a;
}

int llg_fixed_array_stream_value(llg_fixed_array_t* result, const llg_fixed_array_t* const* sources, size_t count, uint32_t slice) {
    if (!result || result->total) fixed_bad_state("invalid fixed stream operand");
    /* The common divisor of every source cell width divides the stream
     * width, so the image has no padding cells. */
    uint32_t cell_width = 0;
    for (size_t i = 0; i < count; ++i) cell_width = fixed_gcd(cell_width, llg_sv4_width(sources[i]->initial));
    if (!count) return 0;
    llg_fixed_image_t* image = fixed_stream_image(sources, count, slice, cell_width, 0);
    llg_fixed_array_init(result, image->bit_count / cell_width, sv4_zero(cell_width, 0), NULL);
    llg_fixed_range_t* range = llg_checked_calloc(1, sizeof(*range), "fixed stream range");
    range->count = result->total; range->image = image; result->ranges = range;
    return 1;
}

int llg_fixed_array_packed_source(llg_fixed_array_t* result, sv4_t value, uint32_t cell_width) {
    if (!result || result->total) fixed_bad_state("invalid fixed stream operand");
    uint32_t width = llg_sv4_width(value);
    if (!width) return 0;
    if (!cell_width || width % cell_width) fixed_bad_state("packed fixed operand is not whole cells");
    uint64_t total = width / cell_width;
    llg_fixed_array_init(result, total, sv4_zero(cell_width, 0), NULL);
    for (uint64_t i = 0; i < total; ++i) {
        int64_t high = (int64_t)(width - i * cell_width) - 1;
        sv4_t cell = sv4_part_select(value, high, high - (int64_t)cell_width + 1);
        if (!sv4_same(cell, result->initial)) sv4_move(llg_fixed_array_cell(result, i), &cell);
        sv4_destroy(&cell);
    }
    return 1;
}

void llg_fixed_array_dense_source(llg_fixed_array_t* result, const sv4_t* cells,
                                  uint64_t origin, uint64_t total, sv4_t fallback) {
    if (!result || result->total || !cells || !total) fixed_bad_state("invalid fixed stream operand");
    llg_fixed_array_init(result, total, sv4_clone(&fallback), NULL);
    if (origin == UINT64_MAX) return;
    for (uint64_t i = 0; i < total; ++i)
        if (!sv4_same(cells[origin + i], result->initial)) sv4_copy(llg_fixed_array_cell(result, i), &cells[origin + i]);
}

int llg_fixed_array_with_source(llg_fixed_array_t* result, const llg_fixed_array_t* array,
                                int64_t declaration_left, int64_t declaration_right,
                                sv4_t fallback, int selector_kind, sv4_t first, sv4_t second) {
    if (!result || result->total || !array) fixed_bad_state("invalid fixed stream operand");
    int64_t left, right;
    size_t count;
    llg_fixed_stream_bounds(selector_kind, first, second, declaration_left, declaration_right, &left, &right, &count);
    if (!count) return 0;
    uint64_t extent = (uint64_t)(declaration_left >= declaration_right ? declaration_left - declaration_right
                                                                         : declaration_right - declaration_left) + 1u;
    if (extent != array->total || llg_sv4_width(fallback) != llg_sv4_width(array->initial))
        fixed_bad_state("fixed `with` operand shape mismatch");
    llg_fixed_array_init(result, count, sv4_clone(&fallback), NULL);
    /* Selected offset i reads storage offset base + i: the selection is
     * oriented to storage order, which advances one cell per element. */
    int64_t minuend = declaration_left <= declaration_right ? left : declaration_left;
    int64_t subtrahend = declaration_left <= declaration_right ? declaration_left : left;
    uint64_t skip = minuend < subtrahend ? (uint64_t)subtrahend - (uint64_t)minuend : 0;
    uint64_t start = minuend < subtrahend ? 0 : (uint64_t)minuend - (uint64_t)subtrahend;
    if (skip >= count || start >= extent) return 1;
    uint64_t in_bounds = count - skip < extent - start ? count - skip : extent - start;
    const llg_fixed_array_t* owner = array->owner ? array->owner : array;
    uint64_t origin = array->owner ? array->origin : 0;
    if (origin == UINT64_MAX) return 1;
    start += origin;
    /* Owner ranges precede the owner's uniform default for the window. */
    result->ranges = fixed_ranges_copy(owner, start, in_bounds, skip, 0);
    llg_fixed_range_t** tail = &result->ranges;
    while (*tail) tail = &(*tail)->next;
    llg_fixed_range_t uniform = {0}; uniform.value = owner->initial;
    fixed_range_append(&result->ranges, &tail, fixed_range_clone(&uniform, skip, in_bounds, 0, 0));
    for (llg_fixed_cell_t* cell = owner->cells; cell; cell = cell->next) {
        if (cell->index < start || cell->index - start >= in_bounds) continue;
        sv4_copy(llg_fixed_array_cell(result, cell->index - start + skip), &cell->value);
    }
    return 1;
}

void llg_fixed_array_stream_copy(llg_fixed_array_t* dst, const llg_fixed_array_t* src, int two_state, int nba, uint32_t slice) {
    uint32_t width = llg_sv4_width(src->initial);
    if (!dst->owner && !src->owner && !dst->ranges && !src->ranges && dst->total == src->total
        && llg_sv4_width(dst->initial) == width && (!slice || !(width % slice) || !(slice % width)))
        fixed_array_simple_stream_copy(dst, src, two_state, nba, slice);
    else { const llg_fixed_array_t* sources[] = {src}; llg_fixed_array_stream_segments(dst, sources, 1, two_state, nba, slice); }
}

void llg_fixed_array_merge(llg_fixed_array_t* dst, const llg_fixed_array_t* left, const llg_fixed_array_t* right, uint64_t element_cells, int two_state) {
    if (!element_cells || dst->total != left->total || dst->total != right->total || dst->total % element_cells) fixed_bad_state("fixed conditional shape mismatch");
    llg_value_scope_t* scope = llg_value_scope_begin_object(sizeof(llg_fixed_array_t), llg_fixed_array_destroy);
    llg_fixed_array_t* snapshot = llg_value_scope_object(scope);
    llg_fixed_image_t* image = llg_checked_calloc(1, sizeof(*image), "fixed conditional image");
    image->refs = 1; image->count = 2; image->merge = 1; image->element_cells = element_cells;
    image->cell_width = llg_sv4_width(dst->initial); image->two_state = two_state;
    image->sources = llg_checked_calloc(2, sizeof(*image->sources), "fixed conditional sources");
    fixed_array_snapshot(&image->sources[0], left, 0); fixed_array_snapshot(&image->sources[1], right, 0);
    llg_fixed_array_init(snapshot, dst->total, two_state ? sv4_zero(image->cell_width, 0) : sv4_x(image->cell_width, 0), NULL);
    llg_fixed_range_t* range = llg_checked_calloc(1, sizeof(*range), "fixed conditional range");
    range->count = dst->total; range->image = image; snapshot->ranges = range;
    fixed_array_apply(dst, snapshot);
    llg_value_scope_end(scope);
}

void llg_real_cells_order(double* cells, uint64_t count, uint64_t element_cells,
                          int method) {
    if (!cells || count < 2 || !element_cells) return;
    if (count > SIZE_MAX / (2 * sizeof(size_t)) ||
        element_cells > SIZE_MAX / sizeof(double) / count)
        llg_fatal_allocation("real array reorder", (size_t)count,
                             (size_t)element_cells);
    size_t elements = (size_t)count;
    size_t width = (size_t)element_cells;
    size_t total = elements * width;
    if (total > (SIZE_MAX - 2 * elements * sizeof(size_t)) / sizeof(double))
        llg_fatal_allocation("real array reorder", elements, width);
    // The workspace belongs to the process unwind stack: a publication that
    // ends the current coroutine still releases it.
    llg_value_scope_t* scope = llg_value_scope_begin_object(
        total * sizeof(double) + 2 * elements * sizeof(size_t), NULL);
    double* moved = (double*)llg_value_scope_object(scope);
    size_t* order = (size_t*)(moved + total);
    int changed = 1;
    if (method == LLG_CONTAINER_METHOD_REVERSE) {
        for (size_t i = 0; i < elements; ++i) order[i] = elements - 1 - i;
    } else if ((method == LLG_CONTAINER_METHOD_SORT ||
                method == LLG_CONTAINER_METHOD_RSORT) && width == 1) {
        changed = llg_real_sort_order(cells, elements,
                                      method == LLG_CONTAINER_METHOD_RSORT,
                                      order);
    } else {
        fixed_bad_state("invalid real array reorder");
    }
    if (changed) {
        for (size_t i = 0; i < elements; ++i)
            memcpy(moved + i * width, cells + order[i] * width,
                   width * sizeof(double));
        for (size_t cell = 0; cell < total; ++cell)
            llg_ba_d(&cells[cell], moved[cell]);
    }
    llg_value_scope_end(scope);
}
