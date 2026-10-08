// ── Collected object heap (SIM-018) ───────────────────────────────────────────
//
// Precise, non-moving, stop-the-world mark-sweep. Every live object is one
// entry of an open-addressing identity index keyed by its exact address; the
// index answers "is this handle a live object" without dereferencing the
// candidate and is also the sweep/teardown list. Marking uses an epoch so an
// abandoned collection needs no unmarking. Objects are allocated anywhere
// (including inside a process turn) but only collected at scheduler safe
// points or by an explicit llg_gc_collect from a caller with no running
// process. The heap is single-threaded like the scheduler.

typedef struct {
    llg_gc_root_fn fn;
    void* context;
} llg_gc_root_entry_t;

typedef struct {
    void (*drop)(void*);
    llg_gc_payload_trace_fn trace;
} llg_gc_payload_entry_t;

typedef struct {
    const llg_co_desc_t* desc;
    const llg_gc_frame_map_t* map;
} llg_gc_frame_entry_t;

struct llg_gc_tracer {
    uint32_t epoch;
    llg_gc_header_t** stack;
    size_t depth;
    size_t stack_capacity;
    // Exact addresses that may point into object storage; NULL marks a free
    // slot of this open-addressing set.
    const void** interior;
    size_t interior_capacity;
    size_t interior_count;
    uint64_t marked;
    int failed;
};

// A real, private object is the index tombstone, not a fabricated pointer.
static llg_gc_header_t llg_gc_tombstone;

static struct {
    llg_gc_header_t** index;
    size_t capacity;
    size_t count;
    size_t used; // live entries plus tombstones
    // LLG_GC_VERIFY: unreachable objects kept allocated until teardown.
    llg_gc_header_t** condemned;
    size_t condemned_count;
    size_t condemned_capacity;
    llg_gc_root_entry_t* roots;
    size_t root_count;
    size_t root_capacity;
    llg_gc_payload_entry_t* payloads;
    size_t payload_count;
    size_t payload_capacity;
    llg_gc_frame_entry_t* frames; // open addressing by descriptor address
    size_t frame_capacity;
    size_t frame_count;
    uint32_t epoch;
    uint64_t since_last;
    uint64_t threshold;
    uint64_t base_threshold;
    uint64_t growth_percent;
    int disabled;
    int stress;
    int verify;
    int print_stats;
    int collecting;
    llg_gc_stats_t stats;
} llg_gc;

// Read by the scheduler at every safe point; set by allocation policy.
static int llg_gc_pending;

static size_t llg_gc_hash(const void* address) {
    uint64_t hash = (uint64_t)(uintptr_t)address;
    hash ^= hash >> 30;
    hash *= UINT64_C(0xbf58476d1ce4e5b9);
    hash ^= hash >> 27;
    hash *= UINT64_C(0x94d049bb133111eb);
    hash ^= hash >> 31;
    return (size_t)hash;
}

// Rebuild the identity index at `capacity` (a power of two). Returns 0 and
// leaves the old index untouched when the allocation fails.
static int gc_index_rebuild(size_t capacity) {
    llg_gc_header_t** entries =
        (llg_gc_header_t**)calloc(capacity, sizeof(*entries));
    if (!entries) return 0;
    for (size_t i = 0; i < llg_gc.capacity; ++i) {
        llg_gc_header_t* entry = llg_gc.index[i];
        if (!entry || entry == &llg_gc_tombstone) continue;
        size_t slot = llg_gc_hash(entry) & (capacity - 1);
        while (entries[slot]) slot = (slot + 1) & (capacity - 1);
        entries[slot] = entry;
    }
    free(llg_gc.index);
    llg_gc.index = entries;
    llg_gc.capacity = capacity;
    llg_gc.used = llg_gc.count;
    return 1;
}

static size_t gc_index_find(const void* address) {
    if (!llg_gc.capacity || !address) return SIZE_MAX;
    size_t mask = llg_gc.capacity - 1;
    size_t slot = llg_gc_hash(address) & mask;
    for (;;) {
        llg_gc_header_t* entry = llg_gc.index[slot];
        if (!entry) return SIZE_MAX;
        if ((const void*)entry == address) return slot;
        slot = (slot + 1) & mask;
    }
}

static void gc_index_insert(llg_gc_header_t* object) {
    size_t capacity = llg_gc.capacity ? llg_gc.capacity : 64;
    while (llg_gc.count + 1 > capacity - capacity / 4) {
        if (capacity > SIZE_MAX / 2 / sizeof(*llg_gc.index))
            llg_fatal_allocation("collected object index", capacity, 2);
        capacity *= 2;
    }
    // Reclaim tombstones before they force full-table probes.
    if (capacity != llg_gc.capacity ||
        llg_gc.used + 1 > capacity - capacity / 4) {
        if (!gc_index_rebuild(capacity))
            llg_fatal_allocation("collected object index", capacity,
                                 sizeof(*llg_gc.index));
    }
    size_t mask = llg_gc.capacity - 1;
    size_t slot = llg_gc_hash(object) & mask;
    while (llg_gc.index[slot] && llg_gc.index[slot] != &llg_gc_tombstone)
        slot = (slot + 1) & mask;
    if (!llg_gc.index[slot]) ++llg_gc.used;
    llg_gc.index[slot] = object;
    ++llg_gc.count;
}

static void gc_index_remove_slot(size_t slot) {
    llg_gc.index[slot] = &llg_gc_tombstone;
    --llg_gc.count;
}

static void gc_update_live_stats(void) {
    llg_gc.stats.live = llg_gc.count;
    if (llg_gc.stats.live > llg_gc.stats.peak_live)
        llg_gc.stats.peak_live = llg_gc.stats.live;
}

void* llg_gc_alloc(size_t size, const llg_gc_type_t* type) {
    if (!type || size < sizeof(llg_gc_header_t)) {
        fputs("llg runtime fatal: invalid collected object allocation\n", stderr);
        abort();
    }
    llg_gc_header_t* object =
        (llg_gc_header_t*)llg_checked_calloc(1, size, "collected object");
    object->type = type;
    gc_index_insert(object);
    ++llg_gc.stats.allocated;
    gc_update_live_stats();
    if (!llg_gc.disabled &&
        (llg_gc.stress || ++llg_gc.since_last >= llg_gc.threshold))
        llg_gc_pending = 1;
    return object;
}

int llg_gc_is_object(const void* handle) {
    return gc_index_find(handle) != SIZE_MAX;
}

static void gc_push(llg_gc_tracer_t* tracer, llg_gc_header_t* object) {
    if (tracer->depth == tracer->stack_capacity) {
        size_t capacity = tracer->stack_capacity ? tracer->stack_capacity * 2 : 256;
        llg_gc_header_t** stack = NULL;
        if (capacity <= SIZE_MAX / sizeof(*stack))
            stack = (llg_gc_header_t**)realloc(tracer->stack,
                                               capacity * sizeof(*stack));
        if (!stack) {
            tracer->failed = 1;
            return;
        }
        tracer->stack = stack;
        tracer->stack_capacity = capacity;
    }
    tracer->stack[tracer->depth++] = object;
}

void llg_gc_visit(llg_gc_tracer_t* tracer, const void* handle) {
    if (!tracer || !handle || tracer->failed) return;
    size_t slot = gc_index_find(handle);
    if (slot == SIZE_MAX) return;
    llg_gc_header_t* object = llg_gc.index[slot];
    if (object->mark == tracer->epoch) return;
    object->mark = tracer->epoch;
    ++tracer->marked;
    gc_push(tracer, object);
}

static int gc_interior_grow(llg_gc_tracer_t* tracer) {
    size_t capacity = tracer->interior_capacity ? tracer->interior_capacity * 2 : 64;
    if (capacity > SIZE_MAX / sizeof(*tracer->interior)) return 0;
    const void** entries = (const void**)calloc(capacity, sizeof(*entries));
    if (!entries) return 0;
    for (size_t i = 0; i < tracer->interior_capacity; ++i) {
        const void* entry = tracer->interior[i];
        if (!entry) continue;
        size_t slot = llg_gc_hash(entry) & (capacity - 1);
        while (entries[slot]) slot = (slot + 1) & (capacity - 1);
        entries[slot] = entry;
    }
    free((void*)tracer->interior);
    tracer->interior = entries;
    tracer->interior_capacity = capacity;
    return 1;
}

void llg_gc_visit_interior(llg_gc_tracer_t* tracer, const void* address) {
    if (!tracer || !address || tracer->failed) return;
    if (tracer->interior_count + 1 >
            tracer->interior_capacity - tracer->interior_capacity / 4 &&
        !gc_interior_grow(tracer)) {
        tracer->failed = 1;
        return;
    }
    size_t mask = tracer->interior_capacity - 1;
    size_t slot = llg_gc_hash(address) & mask;
    while (tracer->interior[slot]) {
        if (tracer->interior[slot] == address) return;
        slot = (slot + 1) & mask;
    }
    tracer->interior[slot] = address;
    ++tracer->interior_count;
}

int llg_gc_interior_hit(const llg_gc_tracer_t* tracer, const void* address) {
    if (!tracer || !address || !tracer->interior_count) return 0;
    size_t mask = tracer->interior_capacity - 1;
    size_t slot = llg_gc_hash(address) & mask;
    while (tracer->interior[slot]) {
        if (tracer->interior[slot] == address) return 1;
        slot = (slot + 1) & mask;
    }
    return 0;
}

// Only opaque handles can name collected objects. Events and process handles
// have their own lifetimes; a NULL descriptor is an untyped handle message.
static void gc_visit_value_slot(void* const* slot, const llg_value_desc_t* desc,
                                void* context) {
    if (desc && desc->kind != LLG_VALUE_OPAQUE) return;
    llg_gc_visit((llg_gc_tracer_t*)context, *slot);
}

void llg_gc_visit_value(llg_gc_tracer_t* tracer, const llg_value_t* value) {
    if (tracer && value) llg_value_trace(value, gc_visit_value_slot, tracer);
}

void llg_gc_visit_payload(llg_gc_tracer_t* tracer, void (*drop)(void*),
                          const void* payload) {
    if (!tracer || !drop || !payload) return;
    for (size_t i = 0; i < llg_gc.payload_count; ++i) {
        if (llg_gc.payloads[i].drop == drop) {
            llg_gc.payloads[i].trace(payload, tracer);
            return;
        }
    }
}

static int gc_grow(void** items, size_t* capacity, size_t needed, size_t size) {
    if (needed <= *capacity) return 1;
    size_t next = *capacity ? *capacity : 8;
    while (next < needed) {
        if (next > SIZE_MAX / 2 / size) return 0;
        next *= 2;
    }
    void* grown = realloc(*items, next * size);
    if (!grown) return 0;
    *items = grown;
    *capacity = next;
    return 1;
}

int llg_gc_register_roots(llg_gc_root_fn fn, void* context) {
    if (!fn) return 0;
    for (size_t i = 0; i < llg_gc.root_count; ++i)
        if (llg_gc.roots[i].fn == fn && llg_gc.roots[i].context == context) return 1;
    if (!gc_grow((void**)&llg_gc.roots, &llg_gc.root_capacity,
                 llg_gc.root_count + 1, sizeof(*llg_gc.roots)))
        return 0;
    llg_gc.roots[llg_gc.root_count++] = (llg_gc_root_entry_t){fn, context};
    return 1;
}

int llg_gc_register_payload_tracer(void (*drop)(void*),
                                   llg_gc_payload_trace_fn trace) {
    if (!drop || !trace) return 0;
    for (size_t i = 0; i < llg_gc.payload_count; ++i) {
        if (llg_gc.payloads[i].drop == drop) {
            llg_gc.payloads[i].trace = trace;
            return 1;
        }
    }
    if (!gc_grow((void**)&llg_gc.payloads, &llg_gc.payload_capacity,
                 llg_gc.payload_count + 1, sizeof(*llg_gc.payloads)))
        return 0;
    llg_gc.payloads[llg_gc.payload_count++] = (llg_gc_payload_entry_t){drop, trace};
    return 1;
}

static const llg_gc_frame_map_t* gc_frame_map(const llg_co_desc_t* desc) {
    if (!llg_gc.frame_capacity || !desc) return NULL;
    size_t mask = llg_gc.frame_capacity - 1;
    size_t slot = llg_gc_hash(desc) & mask;
    while (llg_gc.frames[slot].desc) {
        if (llg_gc.frames[slot].desc == desc) return llg_gc.frames[slot].map;
        slot = (slot + 1) & mask;
    }
    return NULL;
}

int llg_gc_register_frame_map(const llg_co_desc_t* desc,
                              const llg_gc_frame_map_t* map) {
    if (!desc || !map) return 0;
    if (llg_gc.frame_count + 1 > llg_gc.frame_capacity - llg_gc.frame_capacity / 4) {
        size_t capacity = llg_gc.frame_capacity ? llg_gc.frame_capacity * 2 : 32;
        if (capacity > SIZE_MAX / sizeof(*llg_gc.frames)) return 0;
        llg_gc_frame_entry_t* entries =
            (llg_gc_frame_entry_t*)calloc(capacity, sizeof(*entries));
        if (!entries) return 0;
        for (size_t i = 0; i < llg_gc.frame_capacity; ++i) {
            if (!llg_gc.frames[i].desc) continue;
            size_t slot = llg_gc_hash(llg_gc.frames[i].desc) & (capacity - 1);
            while (entries[slot].desc) slot = (slot + 1) & (capacity - 1);
            entries[slot] = llg_gc.frames[i];
        }
        free(llg_gc.frames);
        llg_gc.frames = entries;
        llg_gc.frame_capacity = capacity;
    }
    size_t mask = llg_gc.frame_capacity - 1;
    size_t slot = llg_gc_hash(desc) & mask;
    while (llg_gc.frames[slot].desc && llg_gc.frames[slot].desc != desc)
        slot = (slot + 1) & mask;
    if (!llg_gc.frames[slot].desc) ++llg_gc.frame_count;
    llg_gc.frames[slot] = (llg_gc_frame_entry_t){desc, map};
    return 1;
}

// Trace one live coroutine frame through its descriptor's map at the frame's
// current resume state. Frames without a map hold no handles.
static void gc_trace_frame(llg_gc_tracer_t* tracer, const llg_co_desc_t* desc,
                           const llg_co_frame_t* frame) {
    const llg_gc_frame_map_t* map = gc_frame_map(desc);
    if (!map || !frame || frame->state >= map->n_states) return;
    uint32_t first = map->first[frame->state];
    uint32_t count = map->count[frame->state];
    for (uint32_t i = 0; i < count; ++i) {
        const llg_gc_frame_slot_t* slot = &map->slots[first + i];
        void* value;
        memcpy(&value, (const char*)frame + slot->offset, sizeof(value));
        if (slot->kind == LLG_GC_FRAME_HANDLE) llg_gc_visit(tracer, value);
        else llg_gc_visit_interior(tracer, value);
    }
}

void llg_gc_handle_cell_drop(void* cell) {
    if (cell) *(void**)cell = NULL;
}

int llg_gc_pin(void* handle) {
    size_t slot = gc_index_find(handle);
    if (slot == SIZE_MAX) return 0;
    llg_gc_header_t* object = llg_gc.index[slot];
    if (object->pins == UINT32_MAX) {
        fputs("llg runtime fatal: collected object pin count overflow\n", stderr);
        abort();
    }
    if (object->pins++ == 0) ++llg_gc.stats.pinned;
    return 1;
}

int llg_gc_unpin(void* handle) {
    size_t slot = gc_index_find(handle);
    if (slot == SIZE_MAX) return 0;
    llg_gc_header_t* object = llg_gc.index[slot];
    if (object->pins == 0) {
        fputs("llg runtime fatal: unbalanced collected object unpin\n", stderr);
        abort();
    }
    if (--object->pins == 0) --llg_gc.stats.pinned;
    return 1;
}

static void gc_drain(llg_gc_tracer_t* tracer) {
    while (tracer->depth && !tracer->failed) {
        llg_gc_header_t* object = tracer->stack[--tracer->depth];
        if (object->type->trace) object->type->trace(object, tracer);
    }
}

static void gc_tracer_release(llg_gc_tracer_t* tracer) {
    free(tracer->stack);
    free((void*)tracer->interior);
    *tracer = (llg_gc_tracer_t){0};
}

static void gc_finalize_free(llg_gc_header_t* object) {
    if (object->type->finalize) object->type->finalize(object);
    free(object);
}

void llg_gc_collect(void) {
    if (llg_gc.collecting || llg_gc.disabled) return;
    llg_gc.collecting = 1;
    llg_gc_pending = 0;
    if (++llg_gc.epoch == 0) {
        // Epoch wrap: clear stale marks once so no old mark can match.
        for (size_t i = 0; i < llg_gc.capacity; ++i)
            if (llg_gc.index[i] && llg_gc.index[i] != &llg_gc_tombstone)
                llg_gc.index[i]->mark = 0;
        llg_gc.epoch = 1;
    }
    llg_gc_tracer_t tracer = {0};
    tracer.epoch = llg_gc.epoch;
    if (llg_gc.stats.pinned) {
        for (size_t i = 0; i < llg_gc.capacity; ++i) {
            llg_gc_header_t* object = llg_gc.index[i];
            if (object && object != &llg_gc_tombstone && object->pins)
                llg_gc_visit(&tracer, object);
        }
        gc_drain(&tracer);
    }
    for (size_t i = 0; i < llg_gc.root_count && !tracer.failed; ++i) {
        llg_gc.roots[i].fn(&tracer, llg_gc.roots[i].context);
        gc_drain(&tracer);
    }
    // An object that a queued write, a wait or a suspended caller addresses
    // directly stays alive with everything it reaches.
    if (tracer.interior_count && !tracer.failed) {
        for (size_t i = 0; i < llg_gc.capacity && !tracer.failed; ++i) {
            llg_gc_header_t* object = llg_gc.index[i];
            if (!object || object == &llg_gc_tombstone ||
                object->mark == tracer.epoch || !object->type->interior)
                continue;
            if (object->type->interior(object, &tracer)) {
                llg_gc_visit(&tracer, object);
                gc_drain(&tracer);
            }
        }
    }
    llg_gc_header_t** dead = NULL;
    size_t dead_count = 0;
    if (!tracer.failed && tracer.marked < llg_gc.count) {
        dead_count = llg_gc.count - (size_t)tracer.marked;
        dead = (llg_gc_header_t**)malloc(dead_count * sizeof(*dead));
        if (!dead ||
            (llg_gc.verify &&
             !gc_grow((void**)&llg_gc.condemned, &llg_gc.condemned_capacity,
                      llg_gc.condemned_count + dead_count,
                      sizeof(*llg_gc.condemned))))
            tracer.failed = 1;
    }
    if (tracer.failed) {
        // Nothing was finalized: abandoning keeps every object, which is
        // always safe. Retry later rather than at every safe point.
        free(dead);
        gc_tracer_release(&tracer);
        ++llg_gc.stats.failed_collections;
        if (llg_gc.threshold <= UINT64_MAX / 2) llg_gc.threshold *= 2;
        llg_gc.since_last = 0;
        llg_gc.collecting = 0;
        return;
    }
    size_t found = 0;
    for (size_t i = 0; i < llg_gc.capacity && found < dead_count; ++i) {
        llg_gc_header_t* object = llg_gc.index[i];
        if (!object || object == &llg_gc_tombstone || object->mark == tracer.epoch)
            continue;
        dead[found++] = object;
        gc_index_remove_slot(i);
    }
    gc_tracer_release(&tracer);
    // Finalizers release fields only and never follow handles, so cycles and
    // order are irrelevant.
    for (size_t i = 0; i < found; ++i) {
        if (llg_gc.verify) {
            if (dead[i]->type->condemn) dead[i]->type->condemn(dead[i]);
            llg_gc.condemned[llg_gc.condemned_count++] = dead[i];
        } else {
            gc_finalize_free(dead[i]);
        }
    }
    free(dead);
    if (llg_gc.verify) llg_gc.stats.condemned += found;
    else llg_gc.stats.freed += found;
    ++llg_gc.stats.collections;
    llg_gc.stats.last_marked = llg_gc.count;
    gc_update_live_stats();
    // Drop tombstones once they dominate; failure keeps a valid index.
    if (llg_gc.capacity > 64 && llg_gc.count < llg_gc.capacity / 8) {
        size_t capacity = llg_gc.capacity;
        while (capacity > 64 && llg_gc.count < capacity / 8) capacity /= 2;
        (void)gc_index_rebuild(capacity);
    } else if (llg_gc.used - llg_gc.count > llg_gc.capacity / 4) {
        (void)gc_index_rebuild(llg_gc.capacity);
    }
    // An embedding that collects before runtime initialization uses the
    // compiled defaults.
    uint64_t growth = llg_gc.growth_percent ? llg_gc.growth_percent
                                            : (uint64_t)LLG_GC_DEFAULT_GROWTH_PERCENT;
    uint64_t base = llg_gc.base_threshold ? llg_gc.base_threshold
                                          : (uint64_t)LLG_GC_DEFAULT_THRESHOLD;
    uint64_t grown = llg_gc.count;
    grown = grown > UINT64_MAX / growth ? UINT64_MAX : grown * growth / 100u;
    llg_gc.threshold = grown > base ? grown : base;
    llg_gc.since_last = 0;
    llg_gc_pending = llg_gc.stress && llg_gc.count != 0;
    llg_gc.collecting = 0;
}

void llg_gc_get_stats(llg_gc_stats_t* stats) {
    if (stats) *stats = llg_gc.stats;
}

static int gc_parse_flag(const char* name, int* value) {
    const char* text = getenv(name);
    if (!text) return 1;
    if (strcmp(text, "0") == 0 || strcmp(text, "1") == 0) {
        *value = text[0] == '1';
        return 1;
    }
    fprintf(stderr, "llg: invalid %s `%s` (expected 0 or 1)\n", name, text);
    return 0;
}

static int gc_parse_count(const char* name, uint64_t fallback, uint64_t* value) {
    const char* text = getenv(name);
    if (!text) {
        *value = fallback;
        return 1;
    }
    if (!parse_positive_u64(text, value)) {
        fprintf(stderr, "llg: invalid %s (must be a positive decimal uint64)\n", name);
        return 0;
    }
    return 1;
}

// Runtime cleanup: no collection can run afterwards, so registrations go.
// Objects are model storage and survive until llg_gc_teardown.
static void gc_release_registrations(void) {
    free(llg_gc.roots);
    free(llg_gc.payloads);
    free(llg_gc.frames);
    llg_gc.roots = NULL;
    llg_gc.payloads = NULL;
    llg_gc.frames = NULL;
    llg_gc.root_count = llg_gc.root_capacity = 0;
    llg_gc.payload_count = llg_gc.payload_capacity = 0;
    llg_gc.frame_count = llg_gc.frame_capacity = 0;
    llg_gc_pending = 0;
}

void llg_gc_trace_dyn_values(const void* payload, llg_gc_tracer_t* tracer) {
    const llg_dyn_value_array_t* array = (const llg_dyn_value_array_t*)payload;
    for (size_t i = 0; array && i < array->size; ++i)
        llg_gc_visit_value(tracer, &array->data[i]);
}

void llg_gc_trace_queue_values(const void* payload, llg_gc_tracer_t* tracer) {
    const llg_queue_value_array_t* queue = (const llg_queue_value_array_t*)payload;
    for (size_t i = 0; queue && i < queue->size; ++i)
        llg_gc_visit_value(tracer, &queue->data[i]);
}

void llg_gc_trace_assoc_values(const void* payload, llg_gc_tracer_t* tracer) {
    const llg_assoc_value_t* assoc = (const llg_assoc_value_t*)payload;
    if (!assoc) return;
    for (size_t i = 0; i < assoc->size; ++i)
        llg_gc_visit_value(tracer, &assoc->entries[i].value);
    if (assoc->has_default_value) llg_gc_visit_value(tracer, &assoc->default_value);
}

static void gc_trace_native_value(const void* payload, llg_gc_tracer_t* tracer) {
    llg_gc_visit_value(tracer, (const llg_value_t*)payload);
}

// Runtime initialization: read the policy. Registrations were released by
// the preceding runtime cleanup.
static int gc_configure(void) {
    gc_release_registrations();
    llg_gc.since_last = 0;
    int enabled = 1;
    llg_gc.stress = llg_gc.verify = llg_gc.print_stats = 0;
    if (!gc_parse_flag("LLG_GC", &enabled) ||
        !gc_parse_flag("LLG_GC_STRESS", &llg_gc.stress) ||
        !gc_parse_flag("LLG_GC_VERIFY", &llg_gc.verify) ||
        !gc_parse_flag("LLG_GC_STATS", &llg_gc.print_stats) ||
        !gc_parse_count("LLG_GC_THRESHOLD", (uint64_t)LLG_GC_DEFAULT_THRESHOLD,
                        &llg_gc.base_threshold) ||
        !gc_parse_count("LLG_GC_GROWTH_PERCENT",
                        (uint64_t)LLG_GC_DEFAULT_GROWTH_PERCENT,
                        &llg_gc.growth_percent))
        return 0;
    llg_gc.disabled = !enabled;
    llg_gc.threshold = llg_gc.base_threshold;
    return 1;
}

void llg_gc_teardown(void) {
    if (llg_gc.print_stats) {
        fprintf(stderr,
                "llg: gc: collections=%llu failed=%llu allocated=%llu freed=%llu "
                "condemned=%llu live=%llu peak=%llu pinned=%llu\n",
                (unsigned long long)llg_gc.stats.collections,
                (unsigned long long)llg_gc.stats.failed_collections,
                (unsigned long long)llg_gc.stats.allocated,
                (unsigned long long)llg_gc.stats.freed,
                (unsigned long long)llg_gc.stats.condemned,
                (unsigned long long)llg_gc.stats.live,
                (unsigned long long)llg_gc.stats.peak_live,
                (unsigned long long)llg_gc.stats.pinned);
    }
    for (size_t i = 0; i < llg_gc.capacity; ++i) {
        llg_gc_header_t* object = llg_gc.index[i];
        if (object && object != &llg_gc_tombstone) gc_finalize_free(object);
    }
    for (size_t i = 0; i < llg_gc.condemned_count; ++i)
        gc_finalize_free(llg_gc.condemned[i]);
    free(llg_gc.index);
    free(llg_gc.condemned);
    free(llg_gc.roots);
    free(llg_gc.payloads);
    free(llg_gc.frames);
    memset(&llg_gc, 0, sizeof(llg_gc));
    llg_gc_pending = 0;
}
