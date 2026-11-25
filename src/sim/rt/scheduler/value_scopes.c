/* Exact descriptor identity lookup. Hash integer representations, but compare
 * pointers only for equality; never order/subtract unrelated C pointers.
 * Entries outlive lexical scope exit when an NBA retains the scope.
 * Linear probing; a removal leaves a tombstone (one store, no branch on its
 * neighbours). An insertion takes the first slot of its probe that holds no
 * live entry, so a scope that is opened and closed over and over at the same
 * addresses reuses its own tombstones and never fills the index. Only an
 * insertion that needs an empty slot counts against the load, and tombstones
 * are dropped in place when live entries and tombstones together would fill
 * more than 1/N of the index. Backward-shift deletion needs no tombstones,
 * but its branch on each later entry of the cluster mispredicted on
 * scope-heavy designs.
 * Most lookups are for a model global, which never has an entry. The lowest
 * and highest integer representation of the indexed addresses bound them:
 * a target outside is absent without a probe, whatever the index holds.
 * The bounds are a filter over the representations the hash already uses,
 * not an ordering of pointers: a target inside them is still found, or not,
 * by equality alone. */
typedef struct {
    const void* target;
    llg_value_scope_t* scope;
} llg_value_scope_entry_t;

static llg_value_scope_entry_t* value_scope_index;
static size_t value_scope_capacity;
static size_t value_scope_count;
/* Live entries plus tombstones. */
static size_t value_scope_used;
/* A real, private object is a portable tombstone, not a fabricated pointer. */
static sv4_t value_scope_deleted_key = SV4_EMPTY;
/* Live entries and tombstones together stay within 1/LOAD of the index. The
 * common lookup misses (a global target never has an entry), and a miss
 * walks its whole cluster, tombstones included: on 1,000 flops with NBAs a
 * miss probed 1.4 slots on average at half against 29 at three quarters.
 * A live entry then costs 32 to 85 bytes instead of 21 to 43. Tombstones are
 * dropped at the same size only when they hold at least 1/RECLAIM of the
 * index, so each pass over it is paid for by that many insertions into
 * empty slots; with fewer the index grows. */
#define LLG_VALUE_SCOPE_INDEX_LOAD_DIVISOR 2u
#define LLG_VALUE_SCOPE_INDEX_RECLAIM_DIVISOR 8u
/* Slots of the first index: a design with a handful of processes holds
 * more than 8 descriptors, so 16 slots only added a growth at start-up. */
#define LLG_VALUE_SCOPE_INDEX_MIN_CAPACITY 32u

/* 64 - log2(value_scope_capacity) while the index exists. */
static uint32_t value_scope_shift;

/* Bounds of the indexed addresses as integers: every live entry lies in
 * [low, low + extent). They widen with each entry, are exact again when the
 * tombstones are dropped, and admit nothing while the index is empty
 * (extent 0; no entry has address 0, so the extent never wraps). */
static uintptr_t value_scope_low = UINTPTR_MAX;
static uintptr_t value_scope_high;
static uintptr_t value_scope_extent;

static void value_scope_bounds_reset(void) {
    value_scope_low = UINTPTR_MAX;
    value_scope_high = 0;
    value_scope_extent = 0;
}

static void value_scope_bounds_admit(const void* target) {
    uintptr_t address = (uintptr_t)target;
    if (address < value_scope_low) {
        value_scope_low = address;
        if (address > value_scope_high) value_scope_high = address;
        value_scope_extent = value_scope_high - value_scope_low + 1;
    } else if (address > value_scope_high) {
        value_scope_high = address;
        value_scope_extent = value_scope_high - value_scope_low + 1;
    }
}

/* Home slot: the top bits of a multiplicative (Fibonacci) hash, which depend
 * on every bit of the address. It runs on every lookup, insertion and
 * removal, so it is one multiply rather than a full mixer. */
static size_t value_scope_home(const void* target, uint32_t shift) {
    return (size_t)(((uint64_t)(uintptr_t)target * UINT64_C(0x9e3779b97f4a7c15)) >> shift);
}

static void value_scope_index_rebuild(size_t capacity) {
    llg_value_scope_entry_t* entries = (llg_value_scope_entry_t*)llg_checked_calloc(
        capacity, sizeof(*entries), "value scope address index");
    /* One step per doubling from the index there is, or from a single slot. */
    uint32_t shift = value_scope_capacity ? value_scope_shift : 64;
    for (size_t size = value_scope_capacity ? value_scope_capacity : 1; size < capacity; size <<= 1)
        --shift;
    for (size_t i = 0; i < value_scope_capacity; ++i) {
        llg_value_scope_entry_t entry = value_scope_index[i];
        if (!entry.scope) continue;
        size_t slot = value_scope_home(entry.target, shift);
        while (entries[slot].target) slot = (slot + 1) & (capacity - 1);
        entries[slot] = entry;
    }
    free(value_scope_index);
    value_scope_index = entries;
    value_scope_capacity = capacity;
    value_scope_shift = shift;
    value_scope_used = value_scope_count;
}

/* Drop every tombstone without a second table. A probe never passes an
 * empty slot, so a run of occupied slots holds the home slot of each entry
 * in it. Emptying a run from its first slot and reinserting each live entry
 * at once puts it at or before the slot it had: the pass needs one entry of
 * storage and leaves every later slot of the run as it was until it gets
 * there. The load bound guarantees the empty slot the pass starts after. */
static void value_scope_index_purge(void) {
    size_t mask = value_scope_capacity - 1;
    size_t start = 0;
    while (value_scope_index[start].target) ++start;
    value_scope_bounds_reset();
    for (size_t step = 1; step <= mask; ++step) {
        size_t at = (start + step) & mask;
        llg_value_scope_entry_t entry = value_scope_index[at];
        if (!entry.target) continue;
        value_scope_index[at] = (llg_value_scope_entry_t){NULL, NULL};
        if (!entry.scope) continue;
        size_t slot = value_scope_home(entry.target, value_scope_shift);
        while (value_scope_index[slot].target) slot = (slot + 1) & mask;
        value_scope_index[slot] = entry;
        value_scope_bounds_admit(entry.target);
    }
    value_scope_used = value_scope_count;
}

/* Marks a runtime-private scope (value_scope_begin_private). */
static void value_scope_private_tag(void* object) { (void)object; }

static int32_t value_scope_is_private(const llg_value_scope_t* scope) {
    return scope->destroy_object == value_scope_private_tag;
}

/* Packed scopes index each descriptor; native scopes index their payload base.
 * Interior native fields do not acquire lifetime guarantees from this index. */
static size_t value_scope_key_count(const llg_value_scope_t* scope) {
    if (value_scope_is_private(scope)) return 0;
    return scope->count ? scope->count : (scope->object != NULL ? 1u : 0u);
}

static const void* value_scope_key(const llg_value_scope_t* scope, size_t index) {
    return scope->count ? (const void*)&scope->values[index] : scope->object;
}

/* Make room for an entry in an empty slot when live entries and tombstones
 * fill the load. Dropping the tombstones frees only their slots, so with few
 * of them the index grows instead: the live entries alone are close to the
 * load, and the next insertions would come back here at once. */
static void value_scope_index_reclaim(void) {
    if (value_scope_used - value_scope_count >=
        value_scope_capacity / LLG_VALUE_SCOPE_INDEX_RECLAIM_DIVISOR) {
        value_scope_index_purge();
        return;
    }
    if (value_scope_capacity > SIZE_MAX / 2) {
        llg_msg(LLG_MSG_INTERNAL, NULL, "value scope index capacity overflow");
        abort();
    }
    value_scope_index_rebuild(value_scope_capacity * 2);
}

static void value_scope_index_add(llg_value_scope_t* scope) {
    size_t count = value_scope_key_count(scope);
    if (!count) return;
    if (count > SIZE_MAX - value_scope_count) {
        llg_msg(LLG_MSG_INTERNAL, NULL, "value scope index size overflow");
        abort();
    }
    size_t needed = value_scope_count + count;
    size_t capacity =
        value_scope_capacity ? value_scope_capacity : LLG_VALUE_SCOPE_INDEX_MIN_CAPACITY;
    while (needed > capacity / LLG_VALUE_SCOPE_INDEX_LOAD_DIVISOR) {
        if (capacity > SIZE_MAX / 2) {
            llg_msg(LLG_MSG_INTERNAL, NULL, "value scope index capacity overflow");
            abort();
        }
        capacity *= 2;
    }
    if (capacity != value_scope_capacity) value_scope_index_rebuild(capacity);
    for (size_t i = 0; i < count; ++i) {
        const void* target = value_scope_key(scope, i);
        size_t slot = value_scope_home(target, value_scope_shift);
        while (value_scope_index[slot].scope)
            slot = (slot + 1) & (value_scope_capacity - 1);
        if (!value_scope_index[slot].target) {
            /* An empty slot: tombstones count against the load too. */
            if (value_scope_used >=
                value_scope_capacity / LLG_VALUE_SCOPE_INDEX_LOAD_DIVISOR) {
                value_scope_index_reclaim();
                slot = value_scope_home(target, value_scope_shift);
                while (value_scope_index[slot].target)
                    slot = (slot + 1) & (value_scope_capacity - 1);
            }
            ++value_scope_used;
        }
        value_scope_index[slot] = (llg_value_scope_entry_t){target, scope};
        ++value_scope_count;
        value_scope_bounds_admit(target);
    }
}

static llg_value_scope_t* value_scope_index_find(const void* target) {
    /* One unsigned comparison tests both bounds, and an empty index. */
    if ((uintptr_t)target - value_scope_low >= value_scope_extent) return NULL;
    size_t slot = value_scope_home(target, value_scope_shift);
    while (value_scope_index[slot].target) {
        if (value_scope_index[slot].target == target)
            return value_scope_index[slot].scope;
        slot = (slot + 1) & (value_scope_capacity - 1);
    }
    return NULL;
}

static void value_scope_index_remove(llg_value_scope_t* scope) {
    size_t count = value_scope_key_count(scope);
    for (size_t i = 0; i < count; ++i) {
        const void* target = value_scope_key(scope, i);
        size_t slot = value_scope_home(target, value_scope_shift);
        while (value_scope_index[slot].target != target ||
               value_scope_index[slot].scope != scope) {
            if (!value_scope_index[slot].target) {
                llg_msg(LLG_MSG_INTERNAL, NULL, "value scope index entry missing");
                abort();
            }
            slot = (slot + 1) & (value_scope_capacity - 1);
        }
        value_scope_index[slot] =
            (llg_value_scope_entry_t){&value_scope_deleted_key, NULL};
        --value_scope_count;
    }
    if (!value_scope_count) {
        free(value_scope_index);
        value_scope_index = NULL;
        value_scope_capacity = 0;
        value_scope_used = 0;
        value_scope_bounds_reset();
    }
}

/* Scope references protect descriptor addresses, not just their limb payloads.
 * A queued NBA may outlive both lexical scope exit and its issuing process. */
static void value_scope_release(llg_value_scope_t* scope) {
    if (!scope) return;
    if (scope->references == 0) {
        llg_msg(LLG_MSG_INTERNAL, NULL, "value scope reference underflow");
        abort();
    }
    if (--scope->references != 0) return;
    if (scope->all_prev) scope->all_prev->all_next = scope->all_next;
    else all_value_scopes = scope->all_next;
    if (scope->all_next) scope->all_next->all_prev = scope->all_prev;
    int32_t private_scope = value_scope_is_private(scope);
    if (!private_scope) {
        for (size_t i = 0; i < scope->count; ++i)
            llg_clocking_forget_signal(&scope->values[i]);
    }
    value_scope_index_remove(scope);
    sv4_destroy_array(scope->values, scope->count);
    if (!scope->intrusive && !private_scope) free(scope->values);
    if (scope->destroy_object) scope->destroy_object(scope->object);
    if (!scope->intrusive) {
        free(scope->object);
        free(scope);
    } else {
        *scope = (llg_value_scope_t){0};
    }
}

static llg_value_scope_t* value_scope_retain_target(const void* target) {
    if (!target) return NULL;
    llg_value_scope_t* scope = value_scope_index_find(target);
    if (scope) {
        if (scope->references == SIZE_MAX) {
            llg_msg(LLG_MSG_INTERNAL, NULL, "value scope reference overflow");
            abort();
        }
        ++scope->references;
    }
    return scope; /* Global storage has no entry; model teardown owns it. */
}

static llg_value_scope_t* value_scope_register(llg_value_scope_t* scope) {
    value_scope_index_add(scope);
    scope->references = 1;
    scope->active = 1;
    scope->owner = llg_current();
    llg_value_scope_t** head = scope->owner ? &scope->owner->value_scopes : &root_value_scopes;
    scope->next = *head;
    *head = scope;
    scope->all_next = all_value_scopes;
    if (all_value_scopes) all_value_scopes->all_prev = scope;
    all_value_scopes = scope;
    return scope;
}

llg_value_scope_t* llg_value_scope_begin(size_t count) {
    llg_value_scope_t* scope = (llg_value_scope_t*)llg_checked_calloc(
        1, sizeof(*scope), "value owner scope");
    scope->values = count ? (sv4_t*)llg_checked_calloc(
        count, sizeof(sv4_t), "scoped values") : NULL;
    scope->count = count;
    return value_scope_register(scope);
}

/* Owner of runtime-internal temporaries (publication snapshots, resolved net
 * values) that never become write, NBA or clocking targets: nothing looks
 * them up by address, so they skip the address index and clocking history.
 * Node and cells share one allocation; unwind still destroys them. */
static llg_value_scope_t* value_scope_begin_private(size_t count) {
    if (!LLG_GATE_FAST_PATH) return llg_value_scope_begin(count);
    if (count > (SIZE_MAX - sizeof(llg_value_scope_t)) / sizeof(sv4_t))
        llg_fatal_allocation("value owner scope", count, sizeof(sv4_t));
    llg_value_scope_t* scope = (llg_value_scope_t*)llg_checked_calloc(
        1, sizeof(*scope) + count * sizeof(sv4_t), "value owner scope");
    scope->values = count ? (sv4_t*)(scope + 1) : NULL;
    scope->count = count;
    scope->destroy_object = value_scope_private_tag;
    return value_scope_register(scope);
}

llg_value_scope_t* llg_value_scope_register(llg_value_scope_t* node,
                                          sv4_t* values, size_t count) {
    *node = (llg_value_scope_t){.intrusive = 1, .count = count, .values = values};
    return value_scope_register(node);
}

llg_value_scope_t* llg_value_scope_register_object(llg_value_scope_t* node,
    void* object, void (*destroy)(void*)) {
    *node = (llg_value_scope_t){.intrusive = 1, .object = object,
                                .destroy_object = destroy};
    return value_scope_register(node);
}

llg_value_scope_t* llg_value_scope_begin_object(size_t size, void (*destroy)(void*)) {
    llg_value_scope_t* scope = llg_value_scope_begin(0);
    scope->object = size ? llg_checked_calloc(1, size, "scoped native object") : NULL;
    scope->destroy_object = destroy;
    value_scope_index_add(scope);
    return scope;
}

void* llg_value_scope_object(llg_value_scope_t* scope) {
    return scope ? scope->object : NULL;
}

static void llg_native_ref_detached_release(void* object) {
    llg_string_destroy(object);
}

llg_string_t* llg_native_ref_detached_string(void) {
    return llg_value_scope_object(llg_value_scope_begin_object(
        sizeof(llg_string_t), llg_native_ref_detached_release));
}

double* llg_native_ref_detached_real(void) {
    return (double*)llg_value_scope_object(llg_value_scope_begin_object(
        sizeof(double), NULL));
}

void** llg_native_ref_detached_handle(void) {
    return llg_value_scope_object(
        llg_value_scope_begin_object(sizeof(void*), NULL));
}

sv4_t* llg_value_scope_values(llg_value_scope_t* scope) {
    return scope ? scope->values : NULL;
}

void llg_value_scope_end(llg_value_scope_t* scope) {
    if (!scope) return;
    if (scope->intrusive && scope->references != 1) {
        llg_msg(LLG_MSG_INTERNAL, NULL, "frame value cell escaped its scope");
        abort();
    }
    llg_value_scope_t** head = scope->owner ? &scope->owner->value_scopes : &root_value_scopes;
    while (*head && *head != scope) head = &(*head)->next;
    if (!scope->active || !*head) {
        llg_msg(LLG_MSG_INTERNAL, NULL, "value scope is not registered");
        abort();
    }
    *head = scope->next;
    scope->next = NULL;
    scope->owner = NULL;
    scope->active = 0;
    value_scope_release(scope);
}

llg_value_scope_t* llg_value_scope_mark(void) {
    llg_proc_t* owner = llg_current();
    return owner ? owner->value_scopes : root_value_scopes;
}

void llg_value_scopes_end_since(llg_value_scope_t* mark) {
    llg_proc_t* owner = llg_current();
    llg_value_scope_t** head = owner ? &owner->value_scopes : &root_value_scopes;
    /* Validate first so a stale/foreign mark cannot partially unwind a caller. */
    if (mark) {
        llg_value_scope_t* cursor = *head;
        while (cursor && cursor != mark) cursor = cursor->next;
        if (!cursor) {
            llg_msg(LLG_MSG_INTERNAL, NULL, "value scope mark is not registered");
            abort();
        }
    }
    while (*head != mark) llg_value_scope_end(*head);
}

static void value_scopes_unwind(llg_proc_t* proc) {
    llg_value_scope_t** head = proc ? &proc->value_scopes : &root_value_scopes;
    while (*head) llg_value_scope_end(*head);
}

/* Pin another process's descriptor while publishing to reentrant callbacks.
 * The pin itself is registered on the writer, so nonlocal exit also releases it. */
static void value_target_pin_destroy(void* payload) {
    value_scope_release(*(llg_value_scope_t**)payload);
}

static llg_value_scope_t* value_target_pin(const void* target) {
    llg_value_scope_t* target_scope = value_scope_retain_target(target);
    if (!target_scope) return NULL;
    llg_value_scope_t* owner = llg_value_scope_begin_object(
        sizeof(llg_value_scope_t*), value_target_pin_destroy);
    *(llg_value_scope_t**)llg_value_scope_object(owner) = target_scope;
    return owner;
}
