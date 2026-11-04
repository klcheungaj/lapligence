/* Element references of descriptor-backed containers (SV 13.5.2, 7.10.3).
 *
 * A string, handle or record `ref` formal is a plain address and its callee is
 * unaware of what it is bound to, so an element, whose storage moves when the
 * container grows, shifts or is reassigned, binds through a reference instead.
 * The formal addresses `value`, which never moves:
 *
 * - A record shares its element's `items`: both headers see the same leaves,
 *   and copies keep items in place while a reference lives
 *   (llg_value_replace_in_place).
 * - A string or handle is a copy of the element. Stores through the formal
 *   reach the marker (the string's own notify, or the marker a handle store
 *   publishes), which runs llg_native_ref_stored. Container mutations reach
 *   llg_native_refs_notify, which the first reference installs as the
 *   container's notify function, so the mutation paths stay unchanged and
 *   element reads need no check at all.
 *
 * A reference tracks its element by position or key; each operation that moves
 * elements adjusts it. An element leaving its container outdates the
 * reference: `value` then owns the element's last value and stores stay
 * private to the reference.
 *
 * A member of a record binds a second reference whose `member` addresses the
 * leaf in place and whose `parent` keeps the record's reference, and so its
 * items, alive. A string member's notify runs its marker, and other stores go
 * through the formal's marker or llg_ref_t cell functions. */
typedef struct llg_native_ref llg_native_ref_t;

/* `marker` comes first: llg_native_ref_stored gets its address. */
struct llg_native_ref {
    sv4_t marker;
    llg_value_t value;
    void* owner;
    llg_native_ref_t* parent;
    llg_value_t* member;
    struct llg_native_ref* next;
    struct llg_native_ref* all_prev;
    struct llg_native_ref* all_next;
    llg_container_notify_fn base_notify;
    size_t refs;
    size_t index;
    sv4_t key;
    unsigned char* string_key;
    size_t string_length;
    uint8_t kind;
    uint8_t valid;
    uint8_t fixed;
    uint8_t syncing;
    /* `value` shares a root's items and owns none of them. */
    uint8_t borrowed;
};

enum {
    LLG_NATIVE_REF_DYNAMIC = 0,
    LLG_NATIVE_REF_QUEUE = 1,
    LLG_NATIVE_REF_ASSOC_INTEGRAL = 2,
    LLG_NATIVE_REF_ASSOC_STRING = 3,
};

static struct llg_native_ref* llg_native_refs_all;
/* The one notify function containers use; references forward to it. */
static llg_container_notify_fn llg_native_refs_forward;

static void llg_native_ref_link_all(llg_native_ref_t* ref) {
    ref->all_next = llg_native_refs_all;
    if (ref->all_next) ref->all_next->all_prev = ref;
    llg_native_refs_all = ref;
    ++llg_native_refs_live;
}

static int llg_native_ref_has_items(const llg_native_ref_t* ref) {
    uint8_t kind = ref->value.desc->kind;
    return kind == LLG_VALUE_AGGREGATE || kind == LLG_VALUE_FIXED_ARRAY;
}

static void llg_native_ref_owner_fields(llg_native_ref_t* ref,
                                        llg_native_ref_t*** list,
                                        llg_container_notify_fn** notify) {
    switch (ref->kind) {
    case LLG_NATIVE_REF_DYNAMIC: {
        llg_dyn_value_array_t* array = ref->owner;
        *list = &array->references;
        *notify = &array->notify;
        break;
    }
    case LLG_NATIVE_REF_QUEUE: {
        llg_queue_value_array_t* queue = ref->owner;
        *list = &queue->references;
        *notify = &queue->notify;
        break;
    }
    default: {
        llg_assoc_value_t* array = ref->owner;
        *list = &array->references;
        *notify = &array->notify;
        break;
    }
    }
}

/* The element a live reference names; it exists while `owner` is set. */
static llg_value_t* llg_native_ref_element(const llg_native_ref_t* ref) {
    switch (ref->kind) {
    case LLG_NATIVE_REF_DYNAMIC:
        return &((llg_dyn_value_array_t*)ref->owner)->data[ref->index];
    case LLG_NATIVE_REF_QUEUE:
        return &((llg_queue_value_array_t*)ref->owner)->data[ref->index];
    default: {
        llg_assoc_value_t* array = ref->owner;
        int found = 0;
        size_t position = ref->kind == LLG_NATIVE_REF_ASSOC_STRING
            ? llg_assoc_value_string_position(array, ref->string_key,
                                              ref->string_length, &found)
            : llg_assoc_value_integral_position(array, ref->key, &found);
        if (!found) llg_container_fatal("element reference lost its entry");
        return &array->entries[position].value;
    }
    }
}

static void llg_native_refs_notify(sv4_t* contents, sv4_t* shape, int change);

static void llg_native_ref_publish(llg_native_ref_t* ref) {
    if (llg_value_hooks.publish) llg_value_hooks.publish(&ref->marker);
}

/* A new reference to `element` (NULL binds the default value of an invalid
 * selector), linked to `owner` when one is given. */
static llg_native_ref_t* llg_native_ref_new(const llg_value_desc_t* desc,
                                            void* owner, uint8_t kind,
                                            llg_value_t* element) {
    llg_native_ref_t* ref = llg_alloc_items(1, sizeof(*ref));
    memset(ref, 0, sizeof(*ref));
    ref->marker = sv4_from_u64(2u, LLG_NATIVE_REF_MARKER_WIDTH, 0);
    ref->refs = 1;
    ref->kind = kind;
    ref->valid = element != NULL;
    if (element && (desc->kind == LLG_VALUE_AGGREGATE ||
                    desc->kind == LLG_VALUE_FIXED_ARRAY)) {
        ref->value = *element; // shares items; the element owns them
    } else {
        llg_value_copy(&ref->value, desc, element);
    }
    if (desc->kind == LLG_VALUE_STRING && llg_value_hooks.publish) {
        ref->value.value.string.notify = llg_value_hooks.publish;
        ref->value.value.string.dependency = &ref->marker;
    }
    if (element) {
        ref->owner = owner;
        llg_native_ref_t** list;
        llg_container_notify_fn* notify;
        llg_native_ref_owner_fields(ref, &list, &notify);
        ref->base_notify = *list ? (*list)->base_notify : *notify;
        if (*notify && *notify != llg_native_refs_notify)
            llg_native_refs_forward = *notify;
        *notify = llg_native_refs_notify;
        ref->next = *list;
        *list = ref;
    }
    llg_native_ref_link_all(ref);
    return ref;
}

static void* llg_native_ref_share(llg_native_ref_t* ref) {
    if (ref->refs == SIZE_MAX) llg_container_fatal("element reference count overflow");
    ++ref->refs;
    return ref;
}

/* Disconnect from the owner's list; the owner's notify returns to normal
 * with its last reference. */
static void llg_native_ref_disconnect(llg_native_ref_t* ref) {
    llg_native_ref_t** list;
    llg_container_notify_fn* notify;
    llg_native_ref_owner_fields(ref, &list, &notify);
    llg_native_ref_t** link = list;
    while (*link && *link != ref) link = &(*link)->next;
    if (*link) *link = ref->next;
    if (!*list) *notify = ref->base_notify;
    ref->next = NULL;
    ref->owner = NULL;
}

/* The element leaves its container. A record reference keeps the leaves its
 * formal addresses, and `element` gets its own copy, as a popped value needs
 * one and a dropped element can use one. */
static void llg_native_ref_outdate(llg_native_ref_t* ref, llg_value_t* element) {
    if (llg_native_ref_has_items(ref)) {
        llg_value_t fresh = {0};
        if (!llg_value_try_construct_copy(&fresh, ref->value.desc, &ref->value))
            llg_container_fatal("container allocation failed");
        element->value.items = fresh.value.items;
    }
    llg_native_ref_disconnect(ref);
}

static void llg_native_refs_opened(llg_native_ref_t* list, size_t at) {
    for (; list; list = list->next)
        if (list->index >= at) ++list->index;
}

static void llg_native_refs_removed(llg_native_ref_t** list, size_t at) {
    for (llg_native_ref_t* ref = *list; ref;) {
        llg_native_ref_t* next = ref->next;
        if (ref->index == at)
            llg_native_ref_outdate(ref, llg_native_ref_element(ref));
        else if (ref->index > at)
            --ref->index;
        ref = next;
    }
}

static void llg_native_refs_outdate_all(llg_native_ref_t** list) {
    while (*list) llg_native_ref_outdate(*list, llg_native_ref_element(*list));
}

/* Element moves carry the marks: a reference finds its element's new
 * position from its own address stored in place of the element's descriptor. */
static void llg_native_refs_mark(llg_native_ref_t* list, llg_value_t* data) {
    for (; list; list = list->next) data[list->index].desc = (const void*)list;
}

static void llg_native_refs_unmark(llg_native_ref_t* list, llg_value_t* data,
                                   size_t size, const llg_value_desc_t* element) {
    if (!list) return;
    for (size_t i = 0; i < size; ++i) {
        for (llg_native_ref_t* ref = list; ref; ref = ref->next) {
            if (data[i].desc != (const void*)ref) continue;
            data[i].desc = element;
            ref->index = i;
            break;
        }
    }
}

/* An assignment replaces the whole array: a dynamic array's references
 * become outdated (SV 13.5.2), but a fixed array's element stays the same
 * variable, so a record keeps its leaves and takes the new value there. */
static void llg_native_refs_rekey(llg_native_ref_t** list, llg_value_t* old_data,
                                  llg_value_t* new_data, size_t new_size) {
    for (llg_native_ref_t* ref = *list; ref;) {
        llg_native_ref_t* next = ref->next;
        if (!ref->fixed || ref->index >= new_size) {
            llg_native_ref_outdate(ref, &old_data[ref->index]);
        } else if (llg_native_ref_has_items(ref)) {
            llg_value_t* kept = &old_data[ref->index];
            llg_value_t* incoming = &new_data[ref->index];
            llg_value_replace_in_place(kept, incoming);
            *incoming = *kept;
            kept->value.items = NULL;
        }
        ref = next;
    }
}

static int llg_native_ref_same_entry(const llg_native_ref_t* ref,
                                     const llg_assoc_value_entry_t* entry) {
    if (ref->kind == LLG_NATIVE_REF_ASSOC_STRING)
        return llg_assoc_string_compare(ref->string_key, ref->string_length,
                                        entry->string_key,
                                        entry->string_length) == 0;
    return llg_integral_compare(ref->key, entry->integral_key) == 0;
}

static void llg_native_refs_assoc_removed(llg_native_ref_t** list,
                                          const llg_assoc_value_entry_t* entry) {
    for (llg_native_ref_t* ref = *list; ref; ref = ref->next) {
        if (!llg_native_ref_same_entry(ref, entry)) continue;
        llg_native_ref_outdate(ref, (llg_value_t*)&entry->value);
        return;
    }
}

/* Bring one stale copy up to date. A subscriber of the marker may end the
 * process holding any reference, so the caller rescans afterwards. */
static int llg_native_ref_refresh(llg_native_ref_t* ref) {
    if (!ref->owner || ref->syncing || llg_native_ref_has_items(ref)) return 0;
    const llg_value_t* element = llg_native_ref_element(ref);
    if (llg_value_equal(&ref->value, element)) return 0;
    ref->syncing = 1;
    if (ref->value.desc->kind == LLG_VALUE_STRING) {
        llg_string_assign(&ref->value.value.string, &element->value.string);
    } else {
        llg_value_copy(&ref->value, ref->value.desc, element);
        llg_native_ref_publish(ref);
    }
    ref->syncing = 0;
    return 1;
}

static void llg_native_refs_notify(sv4_t* contents, sv4_t* shape, int change) {
    for (llg_native_ref_t* ref = llg_native_refs_all; ref;)
        ref = llg_native_ref_refresh(ref) ? llg_native_refs_all : ref->all_next;
    if (llg_native_refs_forward) llg_native_refs_forward(contents, shape, change);
}

/* Tell the container of a live element its contents changed. */
static void llg_native_ref_touch(llg_native_ref_t* ref) {
    switch (ref->owner ? ref->kind : 0xffu) {
    case LLG_NATIVE_REF_DYNAMIC: {
        llg_dyn_value_array_t* array = ref->owner;
        llg_notify(array->notify, array->contents_dependency,
                   array->shape_dependency, LLG_CONTAINER_CHANGED_CONTENTS);
        break;
    }
    case LLG_NATIVE_REF_QUEUE: {
        llg_queue_value_array_t* queue = ref->owner;
        llg_notify(queue->notify, queue->contents_dependency,
                   queue->shape_dependency, LLG_CONTAINER_CHANGED_CONTENTS);
        break;
    }
    case LLG_NATIVE_REF_ASSOC_INTEGRAL:
    case LLG_NATIVE_REF_ASSOC_STRING: {
        llg_assoc_value_t* array = ref->owner;
        llg_notify(array->notify, array->contents_dependency,
                   array->shape_dependency, LLG_CONTAINER_CHANGED_CONTENTS);
        break;
    }
    default:
        break;
    }
}

void llg_native_ref_stored(sv4_t* marker) {
    llg_native_ref_t* ref = (llg_native_ref_t*)marker;
    if (ref->member) {
        llg_native_ref_touch(ref->parent);
        return;
    }
    if (ref->syncing) return;
    if (!ref->valid) {
        ref->syncing = 1;
        if (ref->value.desc->kind == LLG_VALUE_STRING) {
            llg_string_destroy(&ref->value.value.string);
            ref->value.value.string.data = NULL;
            ref->value.value.string.len = 0;
        } else {
            ref->value.value.handle = NULL;
        }
        ref->syncing = 0;
        return;
    }
    if (!ref->owner) return;
    ref->syncing = 1;
    llg_value_t* element = llg_native_ref_element(ref);
    llg_value_copy(element, element->desc, &ref->value);
    llg_native_ref_touch(ref);
    ref->syncing = 0;
}

void* llg_dyn_value_ref_acquire(llg_dyn_value_array_t* array, sv4_t index,
                                int fixed) {
    if (!array) llg_container_fatal("null dynamic-array reference source");
    size_t native;
    if (!llg_index(index, array->size, 0, &native))
        return llg_native_ref_new(array->element, NULL, LLG_NATIVE_REF_DYNAMIC, NULL);
    for (llg_native_ref_t* ref = array->references; ref; ref = ref->next)
        if (ref->index == native) return llg_native_ref_share(ref);
    llg_native_ref_t* ref = llg_native_ref_new(array->element, array,
                                               LLG_NATIVE_REF_DYNAMIC,
                                               &array->data[native]);
    ref->index = native;
    ref->fixed = (uint8_t)fixed;
    return ref;
}

void* llg_queue_value_ref_acquire(llg_queue_value_array_t* queue, sv4_t index) {
    if (!queue) llg_container_fatal("null queue reference source");
    size_t native;
    if (!llg_index(index, queue->size, 0, &native))
        return llg_native_ref_new(queue->element, NULL, LLG_NATIVE_REF_QUEUE, NULL);
    for (llg_native_ref_t* ref = queue->references; ref; ref = ref->next)
        if (ref->index == native) return llg_native_ref_share(ref);
    llg_native_ref_t* ref = llg_native_ref_new(queue->element, queue,
                                               LLG_NATIVE_REF_QUEUE,
                                               &queue->data[native]);
    ref->index = native;
    return ref;
}

void* llg_assoc_value_ref_acquire_integral(llg_assoc_value_t* array, sv4_t key) {
    if (!array) llg_container_fatal("null associative-array reference source");
    sv4_t normalized = SV4_EMPTY;
    if (!llg_assoc_value_normalize_key(array, key, &normalized)) {
        llg_container_warning("invalid associative-array key write");
        return llg_native_ref_new(array->element, NULL,
                                  LLG_NATIVE_REF_ASSOC_INTEGRAL, NULL);
    }
    for (llg_native_ref_t* ref = array->references; ref; ref = ref->next) {
        if (llg_integral_compare(ref->key, normalized) == 0) {
            sv4_destroy(&normalized);
            return llg_native_ref_share(ref);
        }
    }
    /* A reference creates its entry even when it is only read. */
    llg_value_t* element = llg_assoc_value_element_integral(array, &key, 1, 1);
    llg_native_ref_t* ref = llg_native_ref_new(array->element, array,
                                               LLG_NATIVE_REF_ASSOC_INTEGRAL,
                                               element);
    sv4_move(&ref->key, &normalized);
    return ref;
}

void* llg_assoc_value_ref_acquire_string(llg_assoc_value_t* array,
                                         const void* key, size_t length) {
    if (!array) llg_container_fatal("null associative-array reference source");
    for (llg_native_ref_t* ref = array->references; ref; ref = ref->next) {
        if (llg_assoc_string_compare(ref->string_key, ref->string_length, key,
                                     length) == 0)
            return llg_native_ref_share(ref);
    }
    llg_value_t* element = llg_assoc_value_element_string(array, key, length, 1);
    llg_native_ref_t* ref = llg_native_ref_new(array->element, array,
                                               LLG_NATIVE_REF_ASSOC_STRING,
                                               element);
    if (length) {
        ref->string_key = llg_alloc_items(length, 1);
        memcpy(ref->string_key, key, length);
    }
    ref->string_length = length;
    return ref;
}

void llg_native_ref_release(void* reference) {
    llg_native_ref_t* ref = reference;
    if (!ref) return;
    if (!ref->refs) llg_container_fatal("element reference count underflow");
    if (--ref->refs) return;
    llg_native_ref_t* parent = ref->parent;
    if (ref->owner) {
        if (llg_native_ref_has_items(ref)) ref->value.value.items = NULL;
        llg_native_ref_disconnect(ref);
    }
    if (ref->borrowed) ref->value.value.items = NULL;
    if (ref->member) {
        if (ref->member->desc->kind == LLG_VALUE_STRING) {
            ref->member->value.string.notify = NULL;
            ref->member->value.string.dependency = NULL;
        }
        ref->value.desc = NULL;
    }
    if (ref->all_prev) ref->all_prev->all_next = ref->all_next;
    else llg_native_refs_all = ref->all_next;
    if (ref->all_next) ref->all_next->all_prev = ref->all_prev;
    --llg_native_refs_live;
    llg_value_drop(&ref->value);
    sv4_destroy(&ref->marker);
    sv4_destroy(&ref->key);
    free(ref->string_key);
    free(ref);
    llg_native_ref_release(parent);
}

void llg_native_ref_scope_release(void* object) {
    llg_native_ref_release(*(void**)object);
}

llg_value_t* llg_native_ref_value(void* reference) {
    llg_native_ref_t* ref = reference;
    return ref->member ? ref->member : &ref->value;
}

llg_string_t* llg_native_ref_string(void* reference) {
    return &llg_native_ref_value(reference)->value.string;
}

void** llg_native_ref_handle(void* reference) {
    return &llg_native_ref_value(reference)->value.handle;
}

void* llg_native_ref_root(llg_value_t* value) {
    llg_native_ref_t* ref = llg_alloc_items(1, sizeof(*ref));
    memset(ref, 0, sizeof(*ref));
    ref->marker = sv4_from_u64(2u, LLG_NATIVE_REF_MARKER_WIDTH, 0);
    ref->refs = 1;
    ref->valid = 1;
    ref->borrowed = 1;
    ref->value = *value;
    llg_native_ref_link_all(ref);
    return ref;
}

void* llg_native_ref_item(void* reference, int64_t position) {
    llg_native_ref_t* ref = reference;
    if (!ref->parent) {
        /* First step: a new reference that keeps the record's alive. */
        llg_native_ref_t* cursor = llg_alloc_items(1, sizeof(*cursor));
        memset(cursor, 0, sizeof(*cursor));
        cursor->marker = sv4_from_u64(2u, LLG_NATIVE_REF_MARKER_WIDTH, 0);
        cursor->refs = 1;
        cursor->valid = ref->valid;
        cursor->parent = ref;
        cursor->member = ref->valid ? llg_native_ref_value(ref) : NULL;
        cursor->value.desc = ref->value.desc;
        llg_native_ref_link_all(cursor);
        ref = cursor;
    }
    const llg_value_desc_t* desc = ref->value.desc;
    if (position < 0 || (size_t)position >= desc->item_count) {
        ref->valid = 0;
        ref->member = NULL;
        if (desc->kind == LLG_VALUE_AGGREGATE)
            llg_container_fatal("invalid record member reference");
        ref->value.desc = desc->element;
        return ref;
    }
    ref->value.desc = llg_value_item_desc(desc, (size_t)position);
    if (ref->member) ref->member = &ref->member->value.items[position];
    return ref;
}

void* llg_native_ref_bind(void* reference) {
    llg_native_ref_t* ref = reference;
    if (!ref->member) {
        /* An invalid selection owns the default the formal reads. */
        const llg_value_desc_t* desc = ref->value.desc;
        ref->value.desc = NULL;
        llg_value_copy(&ref->value, desc, NULL);
        if (desc->kind == LLG_VALUE_STRING && llg_value_hooks.publish) {
            ref->value.value.string.notify = llg_value_hooks.publish;
            ref->value.value.string.dependency = &ref->marker;
        }
        return ref;
    }
    /* Two formals bound to one member share its reference and marker. */
    for (llg_native_ref_t* other = llg_native_refs_all; other;
         other = other->all_next) {
        if (other == ref || other->member != ref->member) continue;
        llg_native_ref_release(ref);
        return llg_native_ref_share(other);
    }
    if (ref->value.desc->kind == LLG_VALUE_STRING && llg_value_hooks.publish) {
        ref->member->value.string.notify = llg_value_hooks.publish;
        ref->member->value.string.dependency = &ref->marker;
    }
    return ref;
}

int llg_native_ref_selects(sv4_t index, int64_t declared) {
    int64_t position = 0;
    return sv4_to_index_i64(index, &position) && position == declared;
}

int64_t llg_native_ref_position(sv4_t index) {
    int64_t position = -1;
    return sv4_to_index_i64(index, &position) ? position : -1;
}

sv4_t llg_native_ref_packed_read(const void* reference) {
    const llg_native_ref_t* ref = reference;
    const llg_value_t* value = ref->member ? ref->member : &ref->value;
    return sv4_clone(&value->value.packed);
}

int llg_native_ref_packed_write(void* reference, sv4_t value) {
    llg_native_ref_t* ref = reference;
    if (!ref->valid) return 0;
    llg_value_t* target = llg_native_ref_value(ref);
    const llg_value_desc_t* desc = target->desc;
    sv4_t assigned = llg_element_assign(value, desc->packed_width,
                                        desc->packed_signed,
                                        desc->packed_two_state);
    if (sv4_same(target->value.packed, assigned)) {
        sv4_destroy(&assigned);
        return 1;
    }
    sv4_move(&target->value.packed, &assigned);
    llg_native_ref_publish(ref);
    llg_native_ref_touch(ref->parent);
    return 1;
}

/* A container's readers follow its contents marker; other roots have only
 * the stores through this reference. */
sv4_t* llg_native_ref_dependency(const void* reference) {
    const llg_native_ref_t* ref = reference;
    const llg_native_ref_t* root = ref->parent;
    sv4_t* contents = NULL;
    if (root && root->owner) {
        switch (root->kind) {
        case LLG_NATIVE_REF_DYNAMIC:
            contents = ((llg_dyn_value_array_t*)root->owner)->contents_dependency;
            break;
        case LLG_NATIVE_REF_QUEUE:
            contents = ((llg_queue_value_array_t*)root->owner)->contents_dependency;
            break;
        default:
            contents = ((llg_assoc_value_t*)root->owner)->contents_dependency;
            break;
        }
    }
    return contents ? contents : (sv4_t*)&ref->marker;
}

sv4_t* llg_native_ref_marker(void* reference) {
    return &((llg_native_ref_t*)reference)->marker;
}
