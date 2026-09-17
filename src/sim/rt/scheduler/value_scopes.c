/* Exact descriptor identity lookup. Hash integer representations, but compare
 * pointers only for equality; never order/subtract unrelated C pointers.
 * Entries outlive lexical scope exit when an NBA retains the scope. */
typedef struct {
    const void* target;
    llg_value_scope_t* scope;
} llg_value_scope_entry_t;

static llg_value_scope_entry_t* value_scope_index;
static size_t value_scope_capacity;
static size_t value_scope_count;
static size_t value_scope_used;
/* A real, private object is a portable tombstone, not a fabricated pointer. */
static sv4_t value_scope_deleted_key = SV4_EMPTY;

static size_t value_scope_hash(const void* target) {
    uint64_t hash = (uint64_t)(uintptr_t)target;
    hash ^= hash >> 30;
    hash *= UINT64_C(0xbf58476d1ce4e5b9);
    hash ^= hash >> 27;
    hash *= UINT64_C(0x94d049bb133111eb);
    hash ^= hash >> 31;
    return (size_t)hash;
}

static void value_scope_index_rebuild(size_t capacity) {
    llg_value_scope_entry_t* entries = (llg_value_scope_entry_t*)llg_checked_calloc(
        capacity, sizeof(*entries), "value scope address index");
    for (size_t i = 0; i < value_scope_capacity; ++i) {
        llg_value_scope_entry_t entry = value_scope_index[i];
        if (!entry.scope) continue;
        size_t slot = value_scope_hash(entry.target) & (capacity - 1);
        while (entries[slot].target) slot = (slot + 1) & (capacity - 1);
        entries[slot] = entry;
    }
    free(value_scope_index);
    value_scope_index = entries;
    value_scope_capacity = capacity;
    value_scope_used = value_scope_count;
}

/* Packed scopes index each descriptor; native scopes index their payload base.
 * Interior native fields do not acquire lifetime guarantees from this index. */
static size_t value_scope_key_count(const llg_value_scope_t* scope) {
    return scope->count ? scope->count : (scope->object != NULL ? 1u : 0u);
}

static const void* value_scope_key(const llg_value_scope_t* scope, size_t index) {
    return scope->count ? (const void*)&scope->values[index] : scope->object;
}

static void value_scope_index_add(llg_value_scope_t* scope) {
    size_t count = value_scope_key_count(scope);
    if (!count) return;
    if (count > SIZE_MAX - value_scope_count) {
        fputs("llg runtime fatal: value scope index size overflow\n", stderr);
        abort();
    }
    size_t needed = value_scope_count + count;
    size_t capacity = value_scope_capacity ? value_scope_capacity : 16;
    while (needed > capacity - capacity / 4) {
        if (capacity > SIZE_MAX / 2) {
            fputs("llg runtime fatal: value scope index capacity overflow\n", stderr);
            abort();
        }
        capacity *= 2;
    }
    /* Reclaim tombstones before they can force a full-table probe. */
    if (capacity != value_scope_capacity ||
        count > capacity - capacity / 4 - value_scope_used)
        value_scope_index_rebuild(capacity);
    for (size_t i = 0; i < count; ++i) {
        const void* target = value_scope_key(scope, i);
        size_t slot = value_scope_hash(target) & (capacity - 1);
        while (value_scope_index[slot].scope)
            slot = (slot + 1) & (capacity - 1);
        if (!value_scope_index[slot].target) ++value_scope_used;
        value_scope_index[slot] = (llg_value_scope_entry_t){target, scope};
        ++value_scope_count;
    }
}

static llg_value_scope_t* value_scope_index_find(const void* target) {
    if (!value_scope_count) return NULL;
    size_t slot = value_scope_hash(target) & (value_scope_capacity - 1);
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
        size_t slot = value_scope_hash(target) & (value_scope_capacity - 1);
        while (value_scope_index[slot].target != target) {
            if (!value_scope_index[slot].target) {
                fputs("llg runtime fatal: value scope index entry missing\n", stderr);
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
    }
}

/* Scope references protect descriptor addresses, not just their limb payloads.
 * A queued NBA may outlive both lexical scope exit and its issuing process. */
static void value_scope_release(llg_value_scope_t* scope) {
    if (!scope) return;
    if (scope->references == 0) {
        fputs("llg runtime fatal: value scope reference underflow\n", stderr);
        abort();
    }
    if (--scope->references != 0) return;
    if (scope->all_prev) scope->all_prev->all_next = scope->all_next;
    else all_value_scopes = scope->all_next;
    if (scope->all_next) scope->all_next->all_prev = scope->all_prev;
    value_scope_index_remove(scope);
    sv4_destroy_array(scope->values, scope->count);
    free(scope->values);
    if (scope->destroy_object) scope->destroy_object(scope->object);
    free(scope->object);
    free(scope);
}

static llg_value_scope_t* value_scope_retain_target(const void* target) {
    if (!target) return NULL;
    llg_value_scope_t* scope = value_scope_index_find(target);
    if (scope) {
        if (scope->references == SIZE_MAX) {
            fputs("llg runtime fatal: value scope reference overflow\n", stderr);
            abort();
        }
        ++scope->references;
    }
    return scope; /* Global storage has no entry; model teardown owns it. */
}

llg_value_scope_t* llg_value_scope_begin(size_t count) {
    llg_value_scope_t* scope = (llg_value_scope_t*)llg_checked_calloc(
        1, sizeof(*scope), "value owner scope");
    scope->values = count ? (sv4_t*)llg_checked_calloc(
        count, sizeof(sv4_t), "scoped values") : NULL;
    scope->count = count;
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

sv4_t* llg_value_scope_values(llg_value_scope_t* scope) {
    return scope ? scope->values : NULL;
}

void llg_value_scope_end(llg_value_scope_t* scope) {
    if (!scope) return;
    llg_value_scope_t** head = scope->owner ? &scope->owner->value_scopes : &root_value_scopes;
    while (*head && *head != scope) head = &(*head)->next;
    if (!scope->active || !*head) {
        fputs("llg runtime fatal: value scope is not registered\n", stderr);
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
            fputs("llg runtime fatal: value scope mark is not registered\n", stderr);
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
