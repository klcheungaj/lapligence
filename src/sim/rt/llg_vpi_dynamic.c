// llg_vpi_dynamic.c — VPI dynamic objects (IEEE 1800-2009 37.3.7, 37.17,
// 37.27-37.29, 38.36.1): string, chandle and class variables, class
// definitions, typespecs, objects and properties, and the elements of
// dynamic arrays, queues and associative arrays.
//
// This file is appended to llg_vpi.c and compiled in the same translation
// unit: it uses the slot, error and value helpers there, and llg_vpi.c calls
// the dyn_* entry points from its public routines.
//
// Lifetime (37.3.7, 37.3.8): a handle to a class object, one of its
// properties or an element of a container property names the object by
// address but does not keep it alive. The collector reports every object it
// reclaims before finalizing any (llg_gc_reclaim_hook); cbReclaimObj and
// cbEndOfObject run then, and every handle into a reclaimed object is
// released, so a later use is a checked stale-handle error, never a use of
// freed storage. Container elements are named by (container, index or key)
// and resolved again at each access.

/* Sub-kinds of an ITER_DYN iterator (slot->span). */
enum {
    DYN_ITER_FIELDS = 1,   /* properties: pointer object (NULL: declaration), right class */
    DYN_ITER_ELEMENTS = 2, /* container elements: base row/object, lsb field */
    DYN_ITER_DERIVED = 3,  /* class defns whose base is class `right` */
};

/* How a SLOT_DYNELEM names its element (slot->lsb). */
enum {
    DYN_AT_INDEX = 0,     /* element: dynamic array or queue index */
    DYN_AT_INTEGRAL = 1,  /* element: low 64 key bits (key width <= 64) */
    DYN_AT_STRING = 2,    /* items/count: a copy of the string key */
};

/* One class object VPI has named: its vpiObjId and whether a handle,
 * iterator or callback refers to it. */
typedef struct {
    void* object;             /* NULL: an empty bucket */
    uint64_t id;
    uint8_t named;
    uint8_t dying;            /* being reclaimed (cbReclaimObj is running) */
} llg_vpi_live_t;

static struct {
    const llg_vpi_dynamic_tables_t* tables;
    uint32_t class_base;      /* static slots: defn, typespec, extends per class */
    llg_vpi_live_t* live;     /* open addressing by object address */
    size_t live_capacity;
    size_t live_count;
    uint64_t next_id;
    uint32_t object_cbs;      /* live cbCreateObj/cbReclaimObj/cbEndOfObject records */
    int hooked;               /* the collector's reclaim hook is installed */
    int reclaiming;
} g_dyn;

static void dyn_reclaimed(void* const* objects, size_t count);

/* ── tables ─────────────────────────────────────────────────────────────── */

int llg_vpi_model_dynamic(const llg_vpi_dynamic_tables_t* tables) {
    g_dyn.tables = tables;
    return 1;
}

static uint32_t dyn_class_count(void) {
    return g_dyn.tables ? g_dyn.tables->class_count : 0u;
}

static const llg_vpi_shape_t* dyn_shape(uint32_t index) {
    const llg_vpi_dynamic_tables_t* t = g_dyn.tables;
    return t && index < t->shape_count ? &t->shapes[index] : NULL;
}

static const llg_vpi_class_t* dyn_class(uint32_t index) {
    const llg_vpi_dynamic_tables_t* t = g_dyn.tables;
    return t && index < t->class_count ? &t->classes[index] : NULL;
}

static const llg_vpi_class_field_t* dyn_field(uint32_t class_index, uint32_t field) {
    const llg_vpi_class_t* layout = dyn_class(class_index);
    if (!layout || field >= layout->field_count) return NULL;
    return &g_dyn.tables->fields[layout->first_field + field];
}

/* Fields a class inherits: the field count of its base layout. */
static uint32_t dyn_inherited_fields(uint32_t class_index) {
    const llg_vpi_class_t* layout = dyn_class(class_index);
    const llg_vpi_class_t* base = layout && layout->base ? dyn_class(layout->base - 1u) : NULL;
    return base ? base->field_count : 0u;
}

static int dyn_tables_valid(void) {
    const llg_vpi_dynamic_tables_t* t = g_dyn.tables;
    if (!t) return 1;
    if ((t->shape_count && !t->shapes) || (t->class_count && !t->classes) ||
        (t->field_count && !t->fields) || (t->class_count && (!t->object || !t->field)))
        return 0;
    uint32_t typespecs = g_vpi.types ? g_vpi.types->typespec_count : 0u;
    for (uint32_t i = 0; i < t->shape_count; ++i) {
        const llg_vpi_shape_t* shape = &t->shapes[i];
        if (shape->shape > LLG_VPI_SHAPE_OTHER || shape->class_index > t->class_count ||
            shape->typespec > typespecs || shape->width >= LLG_SUPPORTED_WIDTH_LIMIT)
            return 0;
        if (shape->shape == LLG_VPI_SHAPE_PACKED && shape->width == 0) return 0;
        if (shape->shape == LLG_VPI_SHAPE_CONTAINER &&
            (shape->element >= i || shape->key > LLG_VPI_KEY_OTHER ||
             (shape->array_type != vpiDynamicArray && shape->array_type != vpiQueueArray &&
              shape->array_type != vpiAssocArray)))
            return 0;
    }
    for (uint32_t i = 0; i < t->class_count; ++i) {
        const llg_vpi_class_t* layout = &t->classes[i];
        if (!layout->name || layout->base > t->class_count || layout->base == i + 1u ||
            layout->first_field > t->field_count ||
            layout->field_count > t->field_count - layout->first_field)
            return 0;
    }
    for (uint32_t i = 0; i < t->field_count; ++i)
        if (t->fields[i].name && t->fields[i].shape >= t->shape_count) return 0;
    return 1;
}

/* A catalog row with LLG_VPI_OBJ_DYNAMIC: storage, a variable shape, and a
 * change marker only for handle variables. */
static int dyn_row_valid(const llg_vpi_model_object_t* object) {
    const llg_vpi_shape_t* shape = object->shape ? dyn_shape(object->shape - 1u) : NULL;
    if (!shape || !object->storage || shape->shape == LLG_VPI_SHAPE_OTHER ||
        object->packed || object->real || object->dim_count)
        return 0;
    if (object->dependency && shape->shape != LLG_VPI_SHAPE_CLASS &&
        shape->shape != LLG_VPI_SHAPE_CHANDLE)
        return 0;
    return object->type == shape->type;
}

/* Three static slots per class: its defn, its typespec, its extends. */
static size_t dyn_static_slot_count(void) { return (size_t)dyn_class_count() * 3u; }

static void dyn_static_slots(uint32_t base) {
    g_dyn.class_base = base;
    uint32_t classes = dyn_class_count();
    for (uint32_t i = 0; i < classes * 3u; ++i) {
        llg_vpi_slot_t* slot = &g_vpi.slots[base + i];
        slot->kind = i < classes ? SLOT_CLASSDEFN : i < 2u * classes ? SLOT_CLASSTS : SLOT_EXTENDS;
        slot->aux = (int32_t)(i % classes);
    }
}

static vpiHandle dyn_class_handle(uint32_t class_index, unsigned which) {
    if (class_index >= dyn_class_count()) return NULL;
    return encode_handle(g_dyn.class_base + which * dyn_class_count() + class_index);
}

static vpiHandle defn_handle(uint32_t class_index) { return dyn_class_handle(class_index, 0); }
static vpiHandle class_typespec_handle(uint32_t class_index) {
    return dyn_class_handle(class_index, 1);
}

/* ── live objects ───────────────────────────────────────────────────────── */

static size_t live_bucket(const void* object, size_t capacity) {
    return watch_bucket(object, capacity);
}

static llg_vpi_live_t* live_find(const void* object) {
    if (!g_dyn.live_capacity || !object) return NULL;
    size_t mask = g_dyn.live_capacity - 1u;
    for (size_t i = live_bucket(object, g_dyn.live_capacity);; i = (i + 1u) & mask) {
        llg_vpi_live_t* entry = &g_dyn.live[i];
        if (!entry->object) return NULL;
        if (entry->object == object) return entry;
    }
}

static int live_grow(void) {
    size_t capacity = g_dyn.live_capacity ? g_dyn.live_capacity * 2u : 64u;
    llg_vpi_live_t* table = (llg_vpi_live_t*)calloc(capacity, sizeof(*table));
    if (!table) return 0;
    for (size_t i = 0; i < g_dyn.live_capacity; ++i) {
        llg_vpi_live_t* entry = &g_dyn.live[i];
        if (!entry->object) continue;
        size_t at = live_bucket(entry->object, capacity);
        while (table[at].object) at = (at + 1u) & (capacity - 1u);
        table[at] = *entry;
    }
    free(g_dyn.live);
    g_dyn.live = table;
    g_dyn.live_capacity = capacity;
    return 1;
}

static void dyn_hook(void) {
    if (g_dyn.hooked) return;
    llg_gc_reclaim_hook(dyn_reclaimed);
    g_dyn.hooked = 1;
}

/* The entry of a live object, created with the next vpiObjId (37.29
 * detail 1: unique among live objects). */
static llg_vpi_live_t* live_get(void* object) {
    llg_vpi_live_t* entry = live_find(object);
    if (entry) return entry;
    if ((g_dyn.live_count + 1u) * 4u > g_dyn.live_capacity * 3u && !live_grow()) {
        vpi_error("LLG_VPI_NOMEM", "VPI object table allocation failed");
        return NULL;
    }
    dyn_hook();
    size_t mask = g_dyn.live_capacity - 1u;
    size_t at = live_bucket(object, g_dyn.live_capacity);
    while (g_dyn.live[at].object) at = (at + 1u) & mask;
    entry = &g_dyn.live[at];
    entry->object = object;
    entry->id = ++g_dyn.next_id;
    entry->named = 0;
    entry->dying = 0;
    ++g_dyn.live_count;
    return entry;
}

/* Backward-shift deletion keeps every probe chain unbroken. */
static void live_remove(llg_vpi_live_t* entry) {
    size_t mask = g_dyn.live_capacity - 1u;
    size_t hole = (size_t)(entry - g_dyn.live);
    for (size_t at = (hole + 1u) & mask; g_dyn.live[at].object; at = (at + 1u) & mask) {
        size_t home = live_bucket(g_dyn.live[at].object, g_dyn.live_capacity);
        /* Move the entry into the hole unless its home lies in (hole, at]. */
        if (((at - home) & mask) >= ((at - hole) & mask)) {
            g_dyn.live[hole] = g_dyn.live[at];
            hole = at;
        }
    }
    memset(&g_dyn.live[hole], 0, sizeof(g_dyn.live[hole]));
    --g_dyn.live_count;
}

/* The layout of a live object, or 0 for an object the model already
 * reclaimed (kept allocated by LLG_GC_VERIFY). */
static int dyn_object_class(void* object, uint32_t* class_index) {
    uint32_t id = 0;
    if (!object || !g_dyn.tables || !g_dyn.tables->object || !g_dyn.tables->object(object, &id) ||
        id >= dyn_class_count())
        return 0;
    *class_index = id;
    return 1;
}

static vpiHandle make_object(void* object) {
    llg_vpi_live_t* entry = live_get(object);
    if (!entry) return NULL;
    uint32_t index = alloc_slot(SLOT_CLASSOBJ);
    if (index == LLG_VPI_NONE) return NULL;
    entry->named = 1;
    g_vpi.slots[index].pointer = object;
    return encode_handle(index);
}

static vpiHandle make_property(void* object, uint32_t class_index, uint32_t field) {
    if (object) {
        llg_vpi_live_t* entry = live_get(object);
        if (!entry) return NULL;
        entry->named = 1;
    }
    uint32_t index = alloc_slot(SLOT_PROPERTY);
    if (index == LLG_VPI_NONE) return NULL;
    llg_vpi_slot_t* slot = &g_vpi.slots[index];
    slot->pointer = object;
    slot->aux = (int32_t)field;
    slot->right = class_index;
    return encode_handle(index);
}

/* ── places: the storage a handle names ─────────────────────────────────── */

typedef struct {
    const llg_vpi_shape_t* shape;
    void* storage;            /* NULL: a declaration without storage */
    void* owner;              /* the class object holding the storage, or NULL */
    uint32_t row;             /* the catalog row, or LLG_VPI_NONE */
    const char* name;         /* the variable's name */
} dyn_place_t;

static int dyn_is_dying(const void* object) {
    const llg_vpi_live_t* entry = object ? live_find(object) : NULL;
    return entry && entry->dying;
}

/* The storage of field `field` of a live object, when its slot kind agrees
 * with the field's shape. */
static void* dyn_field_storage(void* object, uint32_t field, const llg_vpi_shape_t* shape) {
    unsigned kind = 0;
    void* storage = g_dyn.tables->field(object, field, &kind);
    if (!storage || !shape) return NULL;
    switch (shape->shape) {
        case LLG_VPI_SHAPE_PACKED: return kind == LLG_VPI_FIELD_PACKED ? storage : NULL;
        case LLG_VPI_SHAPE_REAL: return kind == LLG_VPI_FIELD_REAL ? storage : NULL;
        case LLG_VPI_SHAPE_STRING: return kind == LLG_VPI_FIELD_STRING ? storage : NULL;
        case LLG_VPI_SHAPE_CLASS:
        case LLG_VPI_SHAPE_CHANDLE: return kind == LLG_VPI_FIELD_HANDLE ? storage : NULL;
        case LLG_VPI_SHAPE_CONTAINER: return kind == LLG_VPI_FIELD_PAYLOAD ? storage : NULL;
        default: return NULL;
    }
}

/* The place of a dynamic row or a property handle. */
static int dyn_place(const llg_vpi_slot_t* slot, dyn_place_t* place) {
    memset(place, 0, sizeof(*place));
    place->row = LLG_VPI_NONE;
    if (slot->kind == SLOT_OBJECT) {
        llg_vpi_model_object_t* object = row_object(slot->object);
        if (!object || !(object->flags & LLG_VPI_OBJ_DYNAMIC)) return 0;
        place->shape = dyn_shape(object->shape - 1u);
        place->storage = object->storage;
        place->row = slot->object;
        place->name = object->name;
        return place->shape != NULL;
    }
    if (slot->kind != SLOT_PROPERTY) return 0;
    uint32_t class_index = (uint32_t)slot->right;
    const llg_vpi_class_field_t* field = dyn_field(class_index, (uint32_t)slot->aux);
    if (!field || !field->name) return 0;
    place->shape = dyn_shape(field->shape);
    place->name = field->name;
    place->owner = slot->pointer;
    if (slot->pointer) {
        place->storage = dyn_field_storage(slot->pointer, (uint32_t)slot->aux, place->shape);
        if (!place->storage && place->shape->shape != LLG_VPI_SHAPE_OTHER) {
            vpi_error("LLG_VPI_MODEL", "class property storage disagrees with its layout");
            return 0;
        }
    }
    return place->shape != NULL;
}

/* The container place of an element slot or element iterator: the
 * catalog row, or property `field` of `owner`. */
static int dyn_container_place(uint32_t row, void* owner, uint32_t field, dyn_place_t* place) {
    memset(place, 0, sizeof(*place));
    place->row = LLG_VPI_NONE;
    if (owner) {
        uint32_t class_index;
        if (!dyn_object_class(owner, &class_index)) return 0;
        const llg_vpi_class_field_t* entry = dyn_field(class_index, field);
        if (!entry || !entry->name) return 0;
        place->shape = dyn_shape(entry->shape);
        place->storage = dyn_field_storage(owner, field, place->shape);
        place->owner = owner;
        place->name = entry->name;
    } else {
        llg_vpi_model_object_t* object = row_object(row);
        if (!object || !(object->flags & LLG_VPI_OBJ_DYNAMIC)) return 0;
        place->shape = dyn_shape(object->shape - 1u);
        place->storage = object->storage;
        place->row = row;
        place->name = object->name;
    }
    return place->shape && place->storage && place->shape->shape == LLG_VPI_SHAPE_CONTAINER;
}

static size_t dyn_container_size(const dyn_place_t* place) {
    const llg_vpi_shape_t* shape = place->shape;
    void* storage = place->storage;
    switch (shape->array_type) {
        case vpiDynamicArray:
            return shape->values ? llg_dyn_value_size((const llg_dyn_value_array_t*)storage)
                                 : llg_dyn_size((const llg_dyn_array_t*)storage);
        case vpiQueueArray:
            return shape->values ? llg_queue_value_size((const llg_queue_value_array_t*)storage)
                                 : llg_queue_size((const llg_queue_t*)storage);
        default:
            return shape->values ? llg_assoc_value_count((const llg_assoc_value_t*)storage)
                                 : llg_assoc_count((const llg_assoc_t*)storage);
    }
}

/* The integral key of an element slot as a value of the key type. */
static sv4_t dyn_integral_key(const dyn_place_t* place, uint64_t bits) {
    if (place->shape->values) {
        const llg_assoc_value_t* array = (const llg_assoc_value_t*)place->storage;
        return sv4_from_u64(bits, array->key_width, array->key_signed);
    }
    const llg_assoc_t* array = (const llg_assoc_t*)place->storage;
    return sv4_from_u64(bits, array->key_width, array->key_signed);
}

/* An element handle; `key`/`key_length` copy a string key. */
static vpiHandle make_dyn_element(const dyn_place_t* place, uint32_t field, unsigned at,
                                  uint64_t element, const unsigned char* key, size_t key_length) {
    char* copy = NULL;
    if (at == DYN_AT_STRING) {
        copy = (char*)malloc(key_length + 1u);
        if (!copy) {
            vpi_error("LLG_VPI_NOMEM", "VPI element key allocation failed");
            return NULL;
        }
        if (key_length) memcpy(copy, key, key_length);
        copy[key_length] = '\0';
    }
    if (place->owner) {
        llg_vpi_live_t* entry = live_get(place->owner);
        if (!entry) {
            free(copy);
            return NULL;
        }
        entry->named = 1;
    }
    uint32_t index = alloc_slot(SLOT_DYNELEM);
    if (index == LLG_VPI_NONE) {
        free(copy);
        return NULL;
    }
    llg_vpi_slot_t* slot = &g_vpi.slots[index];
    slot->object = place->row;
    slot->pointer = place->owner;
    slot->aux = (int32_t)field;
    slot->lsb = at;
    slot->element = element;
    slot->items = (uintptr_t*)(void*)copy; /* freed by release_slot */
    slot->count = (uint32_t)key_length;
    return encode_handle(index);
}

/* A string key longer than a slot can record has no element handle. */
#define LLG_VPI_MAX_ELEMENT_KEY UINT32_MAX

/* The element at assoc entry `position` (key order), or NULL. */
static vpiHandle dyn_assoc_entry(const dyn_place_t* place, uint32_t field, size_t position) {
    const sv4_t* key = NULL;
    const unsigned char* text = NULL;
    size_t length = 0;
    int string_key;
    if (place->shape->values) {
        const llg_assoc_value_t* array = (const llg_assoc_value_t*)place->storage;
        if (position >= array->size) return NULL;
        string_key = array->key_kind == LLG_ASSOC_STRING;
        key = &array->entries[position].integral_key;
        text = array->entries[position].string_key;
        length = array->entries[position].string_length;
    } else {
        const llg_assoc_t* array = (const llg_assoc_t*)place->storage;
        if (position >= array->size) return NULL;
        string_key = array->key_kind == LLG_ASSOC_STRING;
        key = &array->entries[position].integral_key;
        text = array->entries[position].string_key;
        length = array->entries[position].string_length;
    }
    if (string_key) {
        if (length > LLG_VPI_MAX_ELEMENT_KEY) {
            vpi_error("LLG_VPI_UNSUPPORTED", "this associative array key is too long for a handle");
            return NULL;
        }
        return make_dyn_element(place, field, DYN_AT_STRING, 0, text, length);
    }
    if (llg_sv4_width(*key) > 64u || llg_sv4_has_x(*key) || llg_sv4_has_z(*key)) {
        vpi_error("LLG_VPI_UNSUPPORTED",
                  "llg names associative array elements by keys of at most 64 known bits");
        return NULL;
    }
    return make_dyn_element(place, field, DYN_AT_INTEGRAL, sv4_to_u64(*key), NULL, 0);
}

/* Locate the element an element slot names. Packed containers report
 * through `exists` only; value containers also return the element. */
static llg_value_t* dyn_locate(const dyn_place_t* place, const llg_vpi_slot_t* slot,
                               int* exists) {
    const llg_vpi_shape_t* shape = place->shape;
    *exists = 0;
    if (slot->lsb == DYN_AT_INDEX) {
        if (slot->element >= dyn_container_size(place)) return NULL;
        *exists = 1;
        if (!shape->values) return NULL;
        sv4_t index = sv4_from_u64(slot->element, 64, 0);
        llg_value_t* element = shape->array_type == vpiDynamicArray
            ? llg_dyn_value_element((llg_dyn_value_array_t*)place->storage, &index, 1)
            : llg_queue_value_element((llg_queue_value_array_t*)place->storage, &index, 1);
        sv4_destroy(&index);
        *exists = element != NULL;
        return element;
    }
    if (slot->lsb == DYN_AT_STRING) {
        const char* key = (const char*)(const void*)slot->items;
        if (shape->values) {
            /* A missing key is no read: test it first so the locator does
             * not report a read miss. */
            llg_assoc_value_t* array = (llg_assoc_value_t*)place->storage;
            if (!llg_assoc_value_exists_string(array, key, slot->count)) return NULL;
            llg_value_t* element = llg_assoc_value_element_string(array, key, slot->count, 0);
            *exists = element != NULL;
            return element;
        }
        *exists = llg_assoc_exists_string((const llg_assoc_t*)place->storage, key, slot->count);
        return NULL;
    }
    sv4_t key = dyn_integral_key(place, slot->element);
    llg_value_t* element = NULL;
    if (shape->values) {
        llg_assoc_value_t* array = (llg_assoc_value_t*)place->storage;
        if (llg_assoc_value_exists_integral(array, key))
            element = llg_assoc_value_element_integral(array, &key, 1, 0);
        *exists = element != NULL;
    } else {
        *exists = llg_assoc_exists_integral((const llg_assoc_t*)place->storage, key);
    }
    sv4_destroy(&key);
    return element;
}

/* A value element's storage for `shape`, when its kind agrees. */
static void* dyn_value_storage(llg_value_t* element, const llg_vpi_shape_t* shape) {
    if (!element || !element->desc || !shape) return NULL;
    switch (shape->shape) {
        case LLG_VPI_SHAPE_PACKED:
            return element->desc->kind == LLG_VALUE_PACKED ? (void*)&element->value.packed : NULL;
        case LLG_VPI_SHAPE_REAL:
            return element->desc->kind == LLG_VALUE_REAL ? (void*)&element->value.real : NULL;
        case LLG_VPI_SHAPE_STRING:
            return element->desc->kind == LLG_VALUE_STRING ? (void*)&element->value.string : NULL;
        case LLG_VPI_SHAPE_CLASS:
        case LLG_VPI_SHAPE_CHANDLE:
            return element->desc->kind == LLG_VALUE_OPAQUE ||
                           element->desc->kind == LLG_VALUE_CHANDLE
                       ? (void*)&element->value.handle : NULL;
        default:
            return NULL;
    }
}

static void dyn_touch(const dyn_place_t* place) {
    switch (place->shape->array_type) {
        case vpiDynamicArray: llg_dyn_value_touch((llg_dyn_value_array_t*)place->storage); break;
        case vpiQueueArray: llg_queue_value_touch((llg_queue_value_array_t*)place->storage); break;
        default: llg_assoc_value_touch((llg_assoc_value_t*)place->storage); break;
    }
}

/* ── values ─────────────────────────────────────────────────────────────── */

static void dyn_fill_string(const llg_string_t* text, p_vpi_value output) {
    if (output->format == vpiObjTypeVal) output->format = vpiStringVal;
    if (output->format == vpiSuppressVal) return;
    if (output->format == vpiStringVal) {
        char* buffer = text_buffer(text->len + 1u);
        if (!buffer) {
            vpi_error("LLG_VPI_NOMEM", "VPI string value allocation failed");
            return;
        }
        if (text->len) memcpy(buffer, text->data, text->len);
        buffer[text->len] = '\0';
        output->value.str = buffer;
        return;
    }
    /* Other formats read the string as its packed image, 8 bits per
     * character, first character most significant (decision A19-D21). */
    llg_string_t copy = llg_string_clone(text);
    uint32_t width = copy.len ? (uint32_t)(copy.len * 8u) : 8u;
    llg_vpi_datum_t datum;
    memset(&datum, 0, sizeof(datum));
    datum.packed = llg_string_to_packed(copy, width, 0);
    fill_value(&datum, 0, vpiStringVar, output);
    datum_destroy(&datum);
}

/* Fill `output` from value storage of `shape`. */
static void dyn_read(const llg_vpi_shape_t* shape, void* storage, p_vpi_value output) {
    llg_vpi_datum_t datum;
    memset(&datum, 0, sizeof(datum));
    switch (shape->shape) {
        case LLG_VPI_SHAPE_PACKED:
            datum.packed = sv4_clone((const sv4_t*)storage);
            fill_value(&datum, shape->is_signed, shape->type, output);
            datum_destroy(&datum);
            return;
        case LLG_VPI_SHAPE_REAL:
            datum.is_real = 1;
            datum.real = *(const double*)storage;
            datum.packed = VPI_EMPTY;
            fill_value(&datum, 0, shape->type, output);
            return;
        case LLG_VPI_SHAPE_STRING:
            dyn_fill_string((const llg_string_t*)storage, output);
            return;
        case LLG_VPI_SHAPE_CLASS:
            vpi_error("LLG_VPI_VALUE",
                      "a class variable has no value; use vpiClassObj and vpiObjId (37.17 detail 11)");
            return;
        case LLG_VPI_SHAPE_CHANDLE:
            vpi_error("LLG_VPI_VALUE", "a chandle variable has no VPI value format (decision A19-D21)");
            return;
        case LLG_VPI_SHAPE_CONTAINER:
            vpi_error("LLG_VPI_VALUE", "an array variable has no value; access its elements (37.17 detail 11)");
            return;
        default:
            vpi_error("LLG_VPI_VALUE", "this VPI object has no value");
            return;
    }
}

/* Convert `input` for a packed or real destination of `shape`. */
static int dyn_packed_input(const llg_vpi_shape_t* shape, const s_vpi_value* input, sv4_t* out) {
    llg_vpi_datum_t datum;
    if (!value_from_vpi(input, shape->width, shape->is_signed, 0, &datum)) return 0;
    *out = datum.is_real ? sv4_from_real(datum.real, shape->width, shape->is_signed)
                         : sv4_resize(datum.packed, shape->width, shape->is_signed);
    if (shape->two_state) sv4_replace(out, sv4_to_two_state(*out));
    datum_destroy(&datum);
    return 1;
}

static int dyn_real_input(const llg_vpi_shape_t* shape, const s_vpi_value* input, double* out) {
    llg_vpi_datum_t datum;
    if (!value_from_vpi(input, 0, 0, 1, &datum)) return 0;
    double value = datum.is_real ? datum.real : sv4_to_real(datum.packed);
    if (shape->shortreal) value = (double)(float)value;
    datum_destroy(&datum);
    *out = value;
    return 1;
}

static int dyn_string_input(const s_vpi_value* input, const char** text) {
    if (!input || input->format != vpiStringVal || !input->value.str) {
        vpi_error("LLG_VPI_VALUE", "a string receives a vpiStringVal value (decision A19-D21)");
        return 0;
    }
    *text = input->value.str;
    return 1;
}

/* Write a property or variable (vpiNoDelay). */
static void dyn_write(const llg_vpi_shape_t* shape, void* storage, const s_vpi_value* input) {
    switch (shape->shape) {
        case LLG_VPI_SHAPE_PACKED: {
            sv4_t value;
            if (!dyn_packed_input(shape, input, &value)) return;
            llg_ba((sv4_t*)storage, value);
            sv4_destroy(&value);
            return;
        }
        case LLG_VPI_SHAPE_REAL: {
            double value;
            if (dyn_real_input(shape, input, &value)) llg_ba_d((double*)storage, value);
            return;
        }
        case LLG_VPI_SHAPE_STRING: {
            const char* text;
            if (!dyn_string_input(input, &text)) return;
            /* The store publishes the string's own change marker. */
            llg_string_t value = llg_string_bytes(text, strlen(text));
            llg_string_move_take((llg_string_t*)storage, &value);
            llg_string_destroy(&value);
            return;
        }
        default:
            vpi_error("LLG_VPI_VALUE", "this VPI object cannot receive a value");
            return;
    }
}

static void dyn_element_get(const llg_vpi_slot_t* slot, p_vpi_value output) {
    dyn_place_t place;
    if (!dyn_container_place(slot->object, slot->pointer, (uint32_t)slot->aux, &place)) {
        vpi_error("LLG_VPI_HANDLE", "the array of this element no longer exists");
        return;
    }
    const llg_vpi_shape_t* shape = dyn_shape(place.shape->element);
    int exists;
    llg_value_t* element = dyn_locate(&place, slot, &exists);
    if (!exists) {
        vpi_error("LLG_VPI_INDEX", "this element no longer exists (decision A19-D23)");
        return;
    }
    if (place.shape->values) {
        void* storage = dyn_value_storage(element, shape);
        if (!storage) {
            vpi_error("LLG_VPI_VALUE", "this element has no value llg exposes");
            return;
        }
        dyn_read(shape, storage, output);
        return;
    }
    /* Packed containers return an owned copy of the element. */
    sv4_t value;
    if (slot->lsb == DYN_AT_INDEX) {
        sv4_t index = sv4_from_u64(slot->element, 64, 0);
        value = place.shape->array_type == vpiDynamicArray
            ? llg_dyn_get((const llg_dyn_array_t*)place.storage, index)
            : llg_queue_get((const llg_queue_t*)place.storage, index);
        sv4_destroy(&index);
    } else if (slot->lsb == DYN_AT_STRING) {
        value = llg_assoc_get_string((const llg_assoc_t*)place.storage, slot->items, slot->count);
    } else {
        sv4_t key = dyn_integral_key(&place, slot->element);
        value = llg_assoc_get_integral((const llg_assoc_t*)place.storage, key);
        sv4_destroy(&key);
    }
    dyn_read(shape, &value, output);
    sv4_destroy(&value);
}

static void dyn_element_put(const llg_vpi_slot_t* slot, const s_vpi_value* input) {
    dyn_place_t place;
    if (!dyn_container_place(slot->object, slot->pointer, (uint32_t)slot->aux, &place)) {
        vpi_error("LLG_VPI_HANDLE", "the array of this element no longer exists");
        return;
    }
    const llg_vpi_shape_t* shape = dyn_shape(place.shape->element);
    int exists;
    llg_value_t* element = dyn_locate(&place, slot, &exists);
    if (!exists) {
        vpi_error("LLG_VPI_INDEX", "this element no longer exists (decision A19-D23)");
        return;
    }
    if (place.shape->values) {
        void* storage = dyn_value_storage(element, shape);
        if (!storage || shape->shape > LLG_VPI_SHAPE_STRING) {
            vpi_error("LLG_VPI_VALUE", "this element cannot receive a value");
            return;
        }
        if (shape->shape == LLG_VPI_SHAPE_PACKED) {
            sv4_t value;
            if (!dyn_packed_input(shape, input, &value)) return;
            sv4_replace((sv4_t*)storage, value);
        } else if (shape->shape == LLG_VPI_SHAPE_REAL) {
            double value;
            if (!dyn_real_input(shape, input, &value)) return;
            *(double*)storage = value;
        } else {
            const char* text;
            if (!dyn_string_input(input, &text)) return;
            llg_string_bytes_to((llg_string_t*)storage, text, strlen(text));
        }
        /* The container publishes the change to waiting processes. */
        dyn_touch(&place);
        return;
    }
    sv4_t value;
    if (!dyn_packed_input(shape, input, &value)) return;
    if (slot->lsb == DYN_AT_INDEX) {
        sv4_t index = sv4_from_u64(slot->element, 64, 0);
        if (place.shape->array_type == vpiDynamicArray)
            (void)llg_dyn_set((llg_dyn_array_t*)place.storage, index, value);
        else
            (void)llg_queue_set((llg_queue_t*)place.storage, index, value);
        sv4_destroy(&index);
    } else if (slot->lsb == DYN_AT_STRING) {
        (void)llg_assoc_set_string((llg_assoc_t*)place.storage, slot->items, slot->count, value);
    } else {
        sv4_t key = dyn_integral_key(&place, slot->element);
        (void)llg_assoc_set_integral((llg_assoc_t*)place.storage, key, value);
        sv4_destroy(&key);
    }
    sv4_destroy(&value);
}

/* The object a class handle place refers to (NULL: null handle or none). */
static void* dyn_class_target(const llg_vpi_slot_t* slot, int* ok) {
    *ok = 0;
    if (slot->kind == SLOT_DYNELEM) {
        dyn_place_t place;
        if (!dyn_container_place(slot->object, slot->pointer, (uint32_t)slot->aux, &place))
            return NULL;
        const llg_vpi_shape_t* shape = dyn_shape(place.shape->element);
        if (!shape || shape->shape != LLG_VPI_SHAPE_CLASS) return NULL;
        int exists;
        llg_value_t* element = dyn_locate(&place, slot, &exists);
        void** storage = (void**)dyn_value_storage(element, shape);
        if (!storage) return NULL;
        *ok = 1;
        return *storage;
    }
    dyn_place_t place;
    if (!dyn_place(slot, &place) || place.shape->shape != LLG_VPI_SHAPE_CLASS || !place.storage)
        return NULL;
    *ok = 1;
    return *(void**)place.storage;
}

/* The element shape of an element handle. */
static const llg_vpi_shape_t* dyn_element_shape(const llg_vpi_slot_t* slot) {
    dyn_place_t place;
    if (!dyn_container_place(slot->object, slot->pointer, (uint32_t)slot->aux, &place)) return NULL;
    return dyn_shape(place.shape->element);
}

/* ── classification ─────────────────────────────────────────────────────── */

/* Whether llg_vpi.c hands `slot` to this file. */
static int dyn_slot(const llg_vpi_slot_t* slot) {
    switch (slot->kind) {
        case SLOT_CLASSOBJ:
        case SLOT_PROPERTY:
        case SLOT_DYNELEM:
        case SLOT_CLASSDEFN:
        case SLOT_CLASSTS:
        case SLOT_EXTENDS:
            return 1;
        case SLOT_OBJECT: {
            const llg_vpi_model_object_t* object = row_object(slot->object);
            return object && (object->flags & LLG_VPI_OBJ_DYNAMIC);
        }
        default:
            return 0;
    }
}

static int dyn_type(const llg_vpi_slot_t* slot) {
    switch (slot->kind) {
        case SLOT_CLASSOBJ: return vpiClassObj;
        case SLOT_CLASSDEFN: return vpiClassDefn;
        case SLOT_CLASSTS: return vpiClassTypespec;
        case SLOT_EXTENDS: return vpiExtends;
        case SLOT_DYNELEM: {
            const llg_vpi_shape_t* shape = dyn_element_shape(slot);
            return shape ? shape->type : vpiUndefined;
        }
        default: {
            dyn_place_t place;
            return dyn_place(slot, &place) ? place.shape->type : vpiUndefined;
        }
    }
}

static const char* dyn_type_name(int type) {
    switch (type) {
        case vpiClassObj: return "vpiClassObj";
        case vpiClassDefn: return "vpiClassDefn";
        case vpiClassTypespec: return "vpiClassTypespec";
        case vpiExtends: return "vpiExtends";
        case vpiClassVar: return "vpiClassVar";
        case vpiStringVar: return "vpiStringVar";
        case vpiChandleVar: return "vpiChandleVar";
        case vpiArrayVar: return "vpiArrayVar";
        default: return type_name(type);
    }
}

/* Navigation from an object being reclaimed is refused (38.36.1: using
 * its handle "as a reference for purposes of navigation ... is undefined");
 * only the object's own properties stay readable. */
static int dyn_refuse_dying(const void* owner, const char* what) {
    if (!dyn_is_dying(owner)) return 0;
    vpi_set_errorf(vpi_state(), vpiError, "LLG_VPI_RECLAIMED",
                   "%s: the class object is being reclaimed (decision A19-D22)", what);
    return 1;
}

/* ── one-to-one relations ───────────────────────────────────────────────── */

static vpiHandle dyn_constant(int64_t value) { return make_constant(value, vpiDecConst); }

/* vpi_handle(type, slot); `*handled` is 0 when llg_vpi.c should answer. */
static vpiHandle dyn_handle(int type, llg_vpi_slot_t* slot, int* handled) {
    *handled = 1;
    switch (slot->kind) {
        case SLOT_CLASSDEFN:
            if (type == vpiClassTypespec || type == vpiTypespec)
                return class_typespec_handle((uint32_t)slot->aux);
            break;
        case SLOT_CLASSTS: {
            uint32_t class_index = (uint32_t)slot->aux;
            if (type == vpiClassDefn) return defn_handle(class_index);
            if (type == vpiExtends) {
                /* 37.28 detail 9: the base class typespec, through the
                 * extends object; NULL without a base. */
                const llg_vpi_class_t* layout = dyn_class(class_index);
                return layout && layout->base ? dyn_class_handle(class_index, 2) : NULL;
            }
            break;
        }
        case SLOT_EXTENDS:
            if (type == vpiClassTypespec) {
                const llg_vpi_class_t* layout = dyn_class((uint32_t)slot->aux);
                return layout && layout->base ? class_typespec_handle(layout->base - 1u) : NULL;
            }
            break;
        case SLOT_CLASSOBJ: {
            if (type == vpiClassTypespec) {
                uint32_t class_index;
                if (!dyn_object_class(slot->pointer, &class_index)) break;
                return class_typespec_handle(class_index);
            }
            break;
        }
        case SLOT_DYNELEM: {
            if (dyn_refuse_dying(slot->pointer, "vpi_handle")) return NULL;
            if (type == vpiParent) {
                /* 37.17 detail 26: the containing array. */
                if (!slot->pointer) return object_handle(slot->object);
                uint32_t class_index;
                if (!dyn_object_class(slot->pointer, &class_index)) break;
                return make_property(slot->pointer, class_index, (uint32_t)slot->aux);
            }
            if (type == vpiIndex) {
                if (slot->lsb == DYN_AT_STRING) {
                    vpi_error("LLG_VPI_UNSUPPORTED", "a string key has no index constant in llg");
                    return NULL;
                }
                if (slot->lsb == DYN_AT_INTEGRAL) {
                    dyn_place_t place;
                    if (!dyn_container_place(slot->object, slot->pointer, (uint32_t)slot->aux,
                                             &place))
                        break;
                    sv4_t key = dyn_integral_key(&place, slot->element);
                    int64_t value = sv4_to_i64(key);
                    if (!llg_sv4_signed(key) && llg_sv4_width(key) < 64u)
                        value = (int64_t)sv4_to_u64(key);
                    sv4_destroy(&key);
                    return dyn_constant(value);
                }
                return dyn_constant((int64_t)slot->element);
            }
            if (type == vpiClassObj) {
                int ok;
                void* object = dyn_class_target(slot, &ok);
                if (!ok) break;
                return object ? make_object(object) : NULL;
            }
            if (type == vpiClassTypespec || type == vpiTypespec) {
                const llg_vpi_shape_t* shape = dyn_element_shape(slot);
                if (!shape) break;
                if (shape->shape == LLG_VPI_SHAPE_CLASS)
                    return shape->class_index ? class_typespec_handle(shape->class_index - 1u)
                                              : NULL;
                if (type == vpiTypespec && shape->typespec)
                    return typespec_handle(typespec_ref(shape->typespec));
            }
            break;
        }
        case SLOT_PROPERTY:
        case SLOT_OBJECT: {
            dyn_place_t place;
            if (!dyn_place(slot, &place)) {
                if (slot->kind == SLOT_OBJECT) {
                    *handled = 0;
                    return NULL;
                }
                break;
            }
            const llg_vpi_shape_t* shape = place.shape;
            if (type == vpiClassObj && shape->shape == LLG_VPI_SHAPE_CLASS) {
                if (!place.storage) {
                    vpi_error("LLG_VPI_RELATION",
                              "a property of a class definition refers to no object (37.27 detail 2)");
                    return NULL;
                }
                if (dyn_refuse_dying(place.owner, "vpiClassObj")) return NULL;
                void* object = *(void**)place.storage;
                /* 37.29 detail 5: NULL for a null class variable. */
                return object ? make_object(object) : NULL;
            }
            if ((type == vpiClassTypespec || type == vpiTypespec) &&
                shape->shape == LLG_VPI_SHAPE_CLASS)
                return shape->class_index ? class_typespec_handle(shape->class_index - 1u) : NULL;
            if (type == vpiTypespec && shape->shape == LLG_VPI_SHAPE_PACKED && shape->typespec)
                return typespec_handle(typespec_ref(shape->typespec));
            if (slot->kind == SLOT_PROPERTY && type == vpiParent) {
                /* 37.17 detail 26: a property reached by iteration has no
                 * parent. */
                return NULL;
            }
            if ((type == vpiLeftRange || type == vpiRightRange) &&
                shape->shape == LLG_VPI_SHAPE_CONTAINER && place.storage &&
                shape->array_type != vpiAssocArray) {
                /* 37.17 detail 6: [0:size-1]; NULL with no element. */
                size_t size = dyn_container_size(&place);
                if (!size) return NULL;
                return dyn_constant(type == vpiLeftRange ? 0 : (int64_t)size - 1);
            }
            if (slot->kind == SLOT_OBJECT) {
                *handled = 0;
                return NULL;
            }
            break;
        }
        default:
            break;
    }
    vpi_error("LLG_VPI_RELATION", "unsupported VPI relation for this dynamic object");
    return NULL;
}

/* ── iteration ──────────────────────────────────────────────────────────── */

static uint32_t dyn_visible_fields(uint32_t class_index, uint32_t first) {
    const llg_vpi_class_t* layout = dyn_class(class_index);
    uint32_t count = 0;
    for (uint32_t i = first; layout && i < layout->field_count; ++i)
        if (dyn_field(class_index, i)->name) ++count;
    return count;
}

static vpiHandle dyn_iterator(unsigned sub_kind, uint32_t count) {
    if (count == 0) return NULL;
    uint32_t index = alloc_slot(SLOT_ITERATOR);
    if (index == LLG_VPI_NONE) return NULL;
    llg_vpi_slot_t* slot = &g_vpi.slots[index];
    slot->iter_kind = ITER_DYN;
    slot->span = sub_kind;
    slot->count = count;
    return encode_handle(index);
}

static vpiHandle dyn_iterate_fields(void* object, uint32_t class_index, uint32_t first) {
    vpiHandle iterator = dyn_iterator(DYN_ITER_FIELDS, dyn_visible_fields(class_index, first));
    llg_vpi_slot_t* slot = iterator ? decode_handle(iterator, NULL) : NULL;
    if (!slot) return NULL;
    if (object) {
        llg_vpi_live_t* entry = live_get(object);
        if (!entry) {
            vpi_release_handle(iterator);
            return NULL;
        }
        entry->named = 1;
    }
    slot->pointer = object;
    slot->right = class_index;
    slot->left = first;
    return iterator;
}

static int dyn_element_type_matches(const llg_vpi_shape_t* element, int type) {
    return type == vpiReg || type == vpiMemoryWord || type == element->type;
}

static vpiHandle dyn_iterate(int type, llg_vpi_slot_t* slot, int* handled) {
    *handled = 1;
    switch (slot->kind) {
        case SLOT_CLASSOBJ: {
            if (type != vpiVariables) break;
            uint32_t class_index;
            if (!dyn_object_class(slot->pointer, &class_index)) break;
            /* 37.29 detail 6: every property, inherited ones first. */
            return dyn_iterate_fields(slot->pointer, class_index, 0);
        }
        case SLOT_CLASSDEFN:
        case SLOT_CLASSTS: {
            uint32_t class_index = (uint32_t)slot->aux;
            if (type == vpiVariables)
                /* The properties the class declares (37.27 detail 4: base
                 * classes through vpiExtends); they have no value. */
                return dyn_iterate_fields(NULL, class_index, dyn_inherited_fields(class_index));
            if (type == vpiDerivedClasses && slot->kind == SLOT_CLASSDEFN) {
                uint32_t count = 0;
                for (uint32_t i = 0; i < dyn_class_count(); ++i)
                    if (dyn_class(i)->base == class_index + 1u) ++count;
                vpiHandle iterator = dyn_iterator(DYN_ITER_DERIVED, count);
                llg_vpi_slot_t* state = iterator ? decode_handle(iterator, NULL) : NULL;
                if (state) state->right = class_index;
                return iterator;
            }
            break;
        }
        case SLOT_DYNELEM:
        case SLOT_PROPERTY:
        case SLOT_OBJECT: {
            dyn_place_t place;
            if (slot->kind == SLOT_DYNELEM || !dyn_place(slot, &place)) {
                if (slot->kind == SLOT_OBJECT) {
                    *handled = 0;
                    return NULL;
                }
                break;
            }
            if (dyn_refuse_dying(place.owner, "vpi_iterate")) return NULL;
            if (place.shape->shape != LLG_VPI_SHAPE_CONTAINER) {
                if (slot->kind == SLOT_OBJECT) {
                    *handled = 0;
                    return NULL;
                }
                break;
            }
            const llg_vpi_shape_t* element = dyn_shape(place.shape->element);
            if (!element || !dyn_element_type_matches(element, type)) break;
            if (!place.storage) {
                vpi_error("LLG_VPI_RELATION",
                          "a property of a class definition has no elements (37.27 detail 2)");
                return NULL;
            }
            if (place.shape->key == LLG_VPI_KEY_OTHER) {
                vpi_error("LLG_VPI_UNSUPPORTED",
                          "llg does not iterate associative arrays with a wildcard index");
                return NULL;
            }
            size_t size = dyn_container_size(&place);
            if (size > UINT32_MAX) size = UINT32_MAX;
            vpiHandle iterator = dyn_iterator(DYN_ITER_ELEMENTS, (uint32_t)size);
            llg_vpi_slot_t* state = iterator ? decode_handle(iterator, NULL) : NULL;
            if (!state) return NULL;
            if (place.owner) {
                llg_vpi_live_t* entry = live_get(place.owner);
                if (!entry) {
                    vpi_release_handle(iterator);
                    return NULL;
                }
                entry->named = 1;
            }
            state->object = place.row;
            state->pointer = place.owner;
            state->lsb = slot->kind == SLOT_PROPERTY ? (uint32_t)slot->aux : 0u;
            return iterator;
        }
        default:
            break;
    }
    vpi_error("LLG_VPI_RELATION", "unsupported VPI iteration for this dynamic object");
    return NULL;
}

/* The next handle of an ITER_DYN iterator. */
static vpiHandle dyn_scan(llg_vpi_slot_t* slot, uint32_t position) {
    switch (slot->span) {
        case DYN_ITER_FIELDS: {
            uint32_t class_index = (uint32_t)slot->right;
            const llg_vpi_class_t* layout = dyn_class(class_index);
            while (layout && slot->left < (int64_t)layout->field_count) {
                uint32_t field = (uint32_t)slot->left++;
                if (dyn_field(class_index, field)->name)
                    return make_property(slot->pointer, class_index, field);
            }
            return NULL;
        }
        case DYN_ITER_DERIVED: {
            while (slot->left < (int64_t)dyn_class_count()) {
                uint32_t candidate = (uint32_t)slot->left++;
                if (dyn_class(candidate)->base == (uint32_t)slot->right + 1u)
                    return defn_handle(candidate);
            }
            return NULL;
        }
        case DYN_ITER_ELEMENTS: {
            dyn_place_t place;
            uint32_t field = slot->lsb;
            if (!dyn_container_place(slot->object, slot->pointer, field, &place)) return NULL;
            /* Elements beyond the current size are gone (the array shrank
             * while the iterator was open). */
            if (position >= dyn_container_size(&place)) return NULL;
            if (place.shape->array_type == vpiAssocArray)
                return dyn_assoc_entry(&place, field, position);
            return make_dyn_element(&place, field, DYN_AT_INDEX, position, NULL, 0);
        }
        default:
            return NULL;
    }
}

/* ── properties ─────────────────────────────────────────────────────────── */

static PLI_INT64 dyn_size(const dyn_place_t* place, int* ok) {
    *ok = 1;
    const llg_vpi_shape_t* shape = place->shape;
    if (shape->shape == LLG_VPI_SHAPE_STRING && place->storage)
        /* 37.17 detail 9: the characters the string holds. */
        return (PLI_INT64)((const llg_string_t*)place->storage)->len;
    if (shape->shape == LLG_VPI_SHAPE_CONTAINER && place->storage)
        return (PLI_INT64)dyn_container_size(place);
    if (shape->shape == LLG_VPI_SHAPE_PACKED) return shape->width;
    *ok = 0;
    return 0;
}

/* vpi_get64(property, slot); `*handled` 0 when llg_vpi.c should answer. */
static PLI_INT64 dyn_get(int property, llg_vpi_slot_t* slot, int* handled) {
    *handled = 1;
    if (property == vpiType) return dyn_type(slot);
    switch (slot->kind) {
        case SLOT_CLASSDEFN:
        case SLOT_CLASSTS:
            if (property == vpiClassType) return vpiUserDefinedClass;
            /* 37.3.7: class methods are automatic by default. */
            if (property == vpiAutomatic) return 1;
            break;
        case SLOT_EXTENDS:
            break;
        case SLOT_CLASSOBJ: {
            llg_vpi_live_t* entry = live_find(slot->pointer);
            if (property == vpiObjId) return entry ? (PLI_INT64)entry->id : 0;
            if (property == vpiAllocScheme) return vpiDynamicScheme;
            if (property == vpiAutomatic) return 1;
            break;
        }
        case SLOT_DYNELEM: {
            const llg_vpi_shape_t* shape = dyn_element_shape(slot);
            if (!shape) break;
            if (property == vpiArrayMember) return 1;
            if (property == vpiAllocScheme) return vpiDynamicScheme;
            if (property == vpiAutomatic) return 1;
            if (property == vpiSigned) return shape->is_signed;
            if (property == vpiSize) {
                if (shape->shape == LLG_VPI_SHAPE_PACKED) return shape->width;
                if (shape->shape == LLG_VPI_SHAPE_STRING) {
                    s_vpi_value value;
                    value.format = vpiStringVal;
                    dyn_element_get(slot, &value);
                    return g_vpi.error_pending ? vpiUndefined : (PLI_INT64)strlen(value.value.str);
                }
            }
            if (property == vpiObjId && shape->shape == LLG_VPI_SHAPE_CLASS) {
                int ok;
                void* object = dyn_class_target(slot, &ok);
                if (!ok) break;
                llg_vpi_live_t* entry = object ? live_get(object) : NULL;
                return entry ? (PLI_INT64)entry->id : 0;
            }
            if (property == vpiArrayType && shape->shape == LLG_VPI_SHAPE_CONTAINER) break;
            break;
        }
        case SLOT_PROPERTY:
        case SLOT_OBJECT: {
            dyn_place_t place;
            if (!dyn_place(slot, &place)) break;
            const llg_vpi_shape_t* shape = place.shape;
            switch (property) {
                case vpiSize: {
                    int ok;
                    PLI_INT64 size = dyn_size(&place, &ok);
                    if (ok) return size;
                    break;
                }
                case vpiArrayType:
                    if (shape->shape == LLG_VPI_SHAPE_CONTAINER) return shape->array_type;
                    break;
                case vpiObjId: {
                    /* 37.29 detail 2: the referenced object's id, 0 for null. */
                    if (shape->shape != LLG_VPI_SHAPE_CLASS || !place.storage) break;
                    void* object = *(void**)place.storage;
                    llg_vpi_live_t* entry = object ? live_get(object) : NULL;
                    return entry ? (PLI_INT64)entry->id : 0;
                }
                case vpiSigned:
                    return shape->shape == LLG_VPI_SHAPE_PACKED ? shape->is_signed : 0;
                case vpiArrayMember:
                    return 0;
                case vpiAutomatic:
                    return slot->kind == SLOT_PROPERTY;
                case vpiAllocScheme:
                    return slot->kind == SLOT_PROPERTY && place.owner ? vpiDynamicScheme
                                                                      : vpiOtherScheme;
                case vpiLineNo:
                    if (slot->kind == SLOT_OBJECT) return g_vpi.objects[slot->object].line;
                    break;
                default:
                    break;
            }
            break;
        }
        default:
            break;
    }
    unsupported_property();
    return vpiUndefined;
}

static char* dyn_text(const char* format, ...) {
    va_list arguments;
    va_start(arguments, format);
    int length = vsnprintf(g_vpi.str_buffer, sizeof(g_vpi.str_buffer), format, arguments);
    va_end(arguments);
    if (length < 0 || (size_t)length >= sizeof(g_vpi.str_buffer)) {
        vpi_error("LLG_VPI_LIMIT", "the VPI name is too long");
        return NULL;
    }
    return g_vpi.str_buffer;
}

/* `name[index]`, `name[key]` or `name["key"]` of an element; `full` uses
 * the array's full name, which a class property does not have. */
static char* dyn_element_name(const llg_vpi_slot_t* slot, int full) {
    dyn_place_t place;
    if (!dyn_container_place(slot->object, slot->pointer, (uint32_t)slot->aux, &place)) return NULL;
    const char* base = place.name;
    if (full) {
        if (place.owner) {
            vpi_error("LLG_VPI_PROPERTY",
                      "an element of a class property has no vpiFullName (37.17 detail 25)");
            return NULL;
        }
        base = g_vpi.objects[place.row].full_name;
    }
    if (slot->lsb == DYN_AT_STRING)
        return dyn_text("%s[\"%.*s\"]", base, (int)slot->count, (const char*)(const void*)slot->items);
    if (slot->lsb == DYN_AT_INTEGRAL) {
        sv4_t key = dyn_integral_key(&place, slot->element);
        char* text = llg_sv4_signed(key)
            ? dyn_text("%s[%lld]", base, (long long)sv4_to_i64(key))
            : dyn_text("%s[%llu]", base, (unsigned long long)sv4_to_u64(key));
        sv4_destroy(&key);
        return text;
    }
    return dyn_text("%s[%llu]", base, (unsigned long long)slot->element);
}

/* vpi_get_str(property, slot); `*handled` 0 when llg_vpi.c should answer. */
static const char* dyn_get_str(int property, llg_vpi_slot_t* slot, int* handled) {
    *handled = 1;
    if (property == vpiType) {
        const char* name = dyn_type_name(dyn_type(slot));
        if (name) return name;
        vpi_error("LLG_VPI_PROPERTY", "unknown VPI object type");
        return NULL;
    }
    switch (slot->kind) {
        case SLOT_CLASSDEFN:
        case SLOT_CLASSTS:
            if (property == vpiName) return dyn_class((uint32_t)slot->aux)->name;
            break;
        case SLOT_PROPERTY:
            if (property == vpiName) return dyn_field((uint32_t)slot->right, (uint32_t)slot->aux)->name;
            if (property == vpiFullName) {
                /* 37.17 detail 25. */
                vpi_error("LLG_VPI_PROPERTY", "a non-static class property has no vpiFullName");
                return NULL;
            }
            break;
        case SLOT_DYNELEM:
            if (property == vpiName || property == vpiFullName)
                return dyn_element_name(slot, property == vpiFullName);
            break;
        case SLOT_OBJECT:
            *handled = 0;
            return NULL;
        default:
            break;
    }
    vpi_error("LLG_VPI_PROPERTY", "unsupported VPI string property for this dynamic object");
    return NULL;
}

/* ── values ─────────────────────────────────────────────────────────────── */

static void dyn_get_value(llg_vpi_slot_t* slot, p_vpi_value output) {
    switch (slot->kind) {
        case SLOT_DYNELEM:
            dyn_element_get(slot, output);
            return;
        case SLOT_OBJECT:
        case SLOT_PROPERTY: {
            dyn_place_t place;
            if (!dyn_place(slot, &place)) return;
            if (!place.storage) {
                vpi_error("LLG_VPI_VALUE",
                          "a property of a class definition has no value (37.27 detail 2)");
                return;
            }
            dyn_read(place.shape, place.storage, output);
            return;
        }
        default:
            vpi_error("LLG_VPI_VALUE", "this VPI object has no value");
            return;
    }
}

/* vpi_put_value of a dynamic object: vpiNoDelay writes only. */
static void dyn_put_value(llg_vpi_slot_t* slot, p_vpi_value input, int mode) {
    if (mode != vpiNoDelay) {
        vpi_error("LLG_VPI_FLAGS",
                  "llg writes strings, class properties and array elements with vpiNoDelay only "
                  "(decision A19-D23)");
        return;
    }
    if (g_dyn.reclaiming) {
        vpi_error("LLG_VPI_RECLAIMED",
                  "no value can be written while class objects are reclaimed (decision A19-D22)");
        return;
    }
    if (slot->kind == SLOT_DYNELEM) {
        dyn_element_put(slot, input);
        return;
    }
    dyn_place_t place;
    if (slot->kind == SLOT_CLASSOBJ || !dyn_place(slot, &place)) {
        vpi_error("LLG_VPI_VALUE", "this VPI object cannot receive a value");
        return;
    }
    if (!place.storage) {
        vpi_error("LLG_VPI_VALUE", "a property of a class definition has no value (37.27 detail 2)");
        return;
    }
    dyn_write(place.shape, place.storage, input);
}

/* vpi_compare_objects of two dynamic handles: 1 same object, 0 different,
 * -1 when either is not a dynamic handle. */
static int dyn_compare(const llg_vpi_slot_t* a, uint32_t a_index, const llg_vpi_slot_t* b,
                       uint32_t b_index) {
    int a_dynamic = a->kind == SLOT_CLASSOBJ || a->kind == SLOT_PROPERTY || a->kind == SLOT_DYNELEM;
    int b_dynamic = b->kind == SLOT_CLASSOBJ || b->kind == SLOT_PROPERTY || b->kind == SLOT_DYNELEM;
    if (!a_dynamic && !b_dynamic) return -1;
    if (a->kind != b->kind) return 0;
    switch (a->kind) {
        case SLOT_CLASSOBJ:
            return a->pointer == b->pointer;
        case SLOT_PROPERTY:
            /* A declaration's property is its own object (no storage). */
            if (!a->pointer || !b->pointer) return a_index == b_index;
            return a->pointer == b->pointer && a->aux == b->aux;
        default:
            if (a->object != b->object || a->pointer != b->pointer || a->aux != b->aux ||
                a->lsb != b->lsb)
                return 0;
            if (a->lsb == DYN_AT_STRING)
                return a->count == b->count &&
                       (a->count == 0 || memcmp(a->items, b->items, a->count) == 0);
            return a->element == b->element;
    }
}

/* ── names and indices ──────────────────────────────────────────────────── */

/* The property `name` (`length` bytes) of the object a class place refers
 * to; the last declaration wins, so a derived class's property hides an
 * inherited one of the same name. */
static vpiHandle dyn_property_by_name(void* object, const char* name, size_t length) {
    uint32_t class_index;
    if (!dyn_object_class(object, &class_index)) return NULL;
    const llg_vpi_class_t* layout = dyn_class(class_index);
    for (uint32_t i = layout->field_count; i-- > 0;) {
        const char* candidate = dyn_field(class_index, i)->name;
        if (candidate && strlen(candidate) == length && memcmp(candidate, name, length) == 0)
            return make_property(object, class_index, i);
    }
    return NULL;
}

/* An element of container `place` at `index` (dynamic array, queue, or an
 * integral key of an associative array). */
static vpiHandle dyn_element_by_index(const dyn_place_t* place, uint32_t field, int64_t index,
                                      int quiet) {
    if (!place->storage) {
        if (!quiet)
            vpi_error("LLG_VPI_INDEX",
                      "a property of a class definition has no elements (37.27 detail 2)");
        return NULL;
    }
    const llg_vpi_shape_t* shape = place->shape;
    if (shape->array_type == vpiAssocArray) {
        if (shape->key != LLG_VPI_KEY_INTEGRAL) {
            if (!quiet)
                vpi_error("LLG_VPI_INDEX", "llg selects associative elements by integral keys only");
            return NULL;
        }
        sv4_t key = dyn_integral_key(place, (uint64_t)index);
        int exists = shape->values
            ? llg_assoc_value_exists_integral((const llg_assoc_value_t*)place->storage, key)
            : llg_assoc_exists_integral((const llg_assoc_t*)place->storage, key);
        int64_t stored = llg_sv4_signed(key) ? sv4_to_i64(key) : (int64_t)sv4_to_u64(key);
        sv4_destroy(&key);
        if (!exists || stored != index) {
            if (!quiet) vpi_error("LLG_VPI_INDEX", "the associative array has no such key");
            return NULL;
        }
        return make_dyn_element(place, field, DYN_AT_INTEGRAL, (uint64_t)index, NULL, 0);
    }
    if (index < 0 || (uint64_t)index >= dyn_container_size(place)) {
        if (!quiet) vpi_error("LLG_VPI_INDEX", "array index is outside the current size");
        return NULL;
    }
    return make_dyn_element(place, field, DYN_AT_INDEX, (uint64_t)index, NULL, 0);
}

static vpiHandle dyn_handle_by_index(llg_vpi_slot_t* slot, PLI_INT32 index) {
    dyn_place_t place;
    if (slot->kind == SLOT_DYNELEM || !dyn_place(slot, &place) ||
        place.shape->shape != LLG_VPI_SHAPE_CONTAINER) {
        vpi_error("LLG_VPI_INDEX", "object has no access by index");
        return NULL;
    }
    if (dyn_refuse_dying(place.owner, "vpi_handle_by_index")) return NULL;
    return dyn_element_by_index(&place, slot->kind == SLOT_PROPERTY ? (uint32_t)slot->aux : 0u,
                                index, 0);
}

/* `suffix` after dynamic row `row`: `[i]` selects an element, `.name` a
 * property of the object a class variable or property refers to (37.29
 * detail 9: `top.p.Id`). A name that denotes nothing is NULL. */
static vpiHandle dyn_by_suffix(uint32_t row, const char* suffix) {
    vpiHandle current = object_handle(row);
    const char* p = suffix;
    while (current && *p) {
        llg_vpi_slot_t* slot = decode_handle(current, NULL);
        vpiHandle next = NULL;
        if (*p == '[') {
            ++p;
            int64_t index;
            dyn_place_t place;
            if (parse_index(&p, &index) && *p == ']' && slot->kind != SLOT_DYNELEM &&
                dyn_place(slot, &place) && place.shape->shape == LLG_VPI_SHAPE_CONTAINER)
                next = dyn_element_by_index(
                    &place, slot->kind == SLOT_PROPERTY ? (uint32_t)slot->aux : 0u, index, 1);
            if (next) ++p;
        } else if (*p == '.') {
            const char* name = ++p;
            while (*p && *p != '.' && *p != '[') ++p;
            int ok;
            void* object = dyn_class_target(slot, &ok);
            if (ok && object) next = dyn_property_by_name(object, name, (size_t)(p - name));
        }
        if (current != object_handle(row)) vpi_release_handle(current);
        current = next;
    }
    return current;
}

/* ── callbacks ──────────────────────────────────────────────────────────── */

/* cbValueChange on a class variable: its value is opaque, so the callback
 * runs with a NULL value when the variable refers to another object or to
 * none (38.36.1). Strings and arrays are excluded by the clause. Returns
 * -1 when `slot` is not a dynamic object. */
static int dyn_bind_value_cb(struct llg_vpi_cb* cb, llg_vpi_slot_t* slot) {
    if (!dyn_slot(slot)) return -1;
    llg_vpi_model_object_t* object =
        slot->kind == SLOT_OBJECT ? row_object(slot->object) : NULL;
    const llg_vpi_shape_t* shape = object ? dyn_shape(object->shape - 1u) : NULL;
    if (!shape || shape->shape != LLG_VPI_SHAPE_CLASS || !object->dependency) {
        vpi_error("LLG_VPI_CALLBACK",
                  "cbValueChange applies to class variables of a scope, not to strings, arrays, "
                  "class objects or their properties (38.36.1)");
        return 0;
    }
    llg_vpi_watch_t* watch = (llg_vpi_watch_t*)calloc(1, sizeof(*watch));
    if (!watch) {
        vpi_error("LLG_VPI_NOMEM", "VPI value-change watch allocation failed");
        return 0;
    }
    watch->key = object->dependency;
    watch->cb = cb;
    watch->element = LLG_VPI_NO_ELEMENT;
    watch->event = LLG_VPI_WATCH_OPAQUE;
    cb->watches = watch;
    cb->watch_total = 1;
    cb->target.object = slot->object;
    cb->target.element = LLG_VPI_NO_ELEMENT;
    cb->scope = object;
    cb->value_type = vpiClassVar;
    return 1;
}

/* cbCreateObj on a class typespec; cbReclaimObj on a class typespec or a
 * class object; cbEndOfObject on a class object (38.36.1). The record keeps
 * the class in target.object and a watched object's address in
 * target.element. */
static int dyn_bind_object_cb(struct llg_vpi_cb* cb, const s_cb_data* data) {
    llg_vpi_slot_t* slot = checked_handle(data->obj, NULL);
    if (!slot) return 0;
    int reason = data->reason;
    if (slot->kind == SLOT_CLASSTS && reason != cbEndOfObject) {
        cb->target.object = (uint32_t)slot->aux;
        cb->target.element = 0;
        return 1;
    }
    if (slot->kind == SLOT_CLASSOBJ && reason != cbCreateObj) {
        if (dyn_refuse_dying(slot->pointer, "vpi_register_cb")) return 0;
        uint32_t class_index;
        if (!dyn_object_class(slot->pointer, &class_index)) return 0;
        cb->target.object = class_index;
        cb->target.element = (uint64_t)(uintptr_t)slot->pointer;
        return 1;
    }
    vpi_error("LLG_VPI_CALLBACK",
              reason == cbCreateObj ? "cbCreateObj needs a class typespec handle (38.36.1)"
                                    : "cbReclaimObj needs a class typespec or class object handle, "
                                      "cbEndOfObject a class object handle (38.36.1)");
    return 0;
}

static void dyn_object_cb_added(void) {
    ++g_dyn.object_cbs;
    dyn_hook();
}

static void dyn_object_cb_removed(void) {
    if (g_dyn.object_cbs) --g_dyn.object_cbs;
}

typedef struct {
    int reason;
    uint32_t class_index;
    const void* object;
} dyn_cb_match_t;

/* A typespec record matches objects of exactly its class (decision
 * A19-D22); an object record matches its object. */
static int matches_object_cb(const llg_vpi_cb_t* cb, const void* context) {
    const dyn_cb_match_t* match = (const dyn_cb_match_t*)context;
    if (cb->kind != CB_OBJECT) return 0;
    if (cb->target.element)
        return (match->reason == cbReclaimObj) &&
               (cb->data.reason == cbReclaimObj || cb->data.reason == cbEndOfObject) &&
               (const void*)(uintptr_t)cb->target.element == match->object;
    return cb->data.reason == match->reason && cb->target.object == match->class_index;
}

/* Run the object records matching `object` with a handle to it that is
 * released when they returned. */
static void dyn_run_object_cbs(int reason, void* object, uint32_t class_index) {
    dyn_cb_match_t match = { reason, class_index, object };
    llg_vpi_cb_t* buffer[LLG_VPI_DISPATCH_INLINE];
    size_t count;
    llg_vpi_cb_t** items = collect_cbs(g_vpi.cbs, matches_object_cb, &match, buffer, &count);
    if (count) {
        vpiHandle handle = make_object(object);
        uint32_t index;
        if (handle && decode_handle(handle, &index)) {
            for (size_t i = 0; i < count; ++i) {
                llg_vpi_cb_t* cb = items[i];
                if (cb->removed) continue;
                s_cb_data data;
                s_vpi_time time;
                base_data(cb, &data);
                data.obj = handle;
                /* cbReclaimObj and cbEndOfObject pass no time (38.36.1). */
                data.time = reason == cbCreateObj
                    ? callback_time(cb->time.type, NULL, llg_time(), &time) : NULL;
                invoke_cb(cb, &data);
                /* An object record runs once: its object is gone. */
                if (cb->target.element) vpi_cb_fired(cb);
            }
            if (decode_handle(handle, &index)) release_slot(index);
        }
    }
    if (items != buffer) free(items);
}

void llg_vpi_object_created(void* handle) {
    if (!g_dyn.object_cbs || !handle) return;
    uint32_t class_index;
    if (!dyn_object_class(handle, &class_index)) return;
    dispatch_begin();
    dyn_run_object_cbs(cbCreateObj, handle, class_index);
    dispatch_end();
}

/* The collector reclaims `objects` (no finalizer ran yet): run the
 * reclaim records, then release every handle and iterator that names one
 * of them and forget their ids. */
static void dyn_reclaimed(void* const* objects, size_t count) {
    if (!g_vpi.indexed || (!g_dyn.live_count && !g_dyn.object_cbs)) return;
    g_dyn.reclaiming = 1;
    int named = 0;
    for (size_t i = 0; i < count; ++i) {
        llg_vpi_live_t* entry = live_find(objects[i]);
        if (!entry) continue;
        entry->dying = 1;
        named |= entry->named;
    }
    if (g_dyn.object_cbs) {
        dispatch_begin();
        for (size_t i = 0; i < count; ++i) {
            uint32_t class_index;
            if (!dyn_object_class(objects[i], &class_index)) continue;
            llg_vpi_live_t* entry = live_get(objects[i]);
            if (!entry) continue;
            entry->dying = 1;
            dyn_run_object_cbs(cbReclaimObj, objects[i], class_index);
            named |= entry->named;
        }
        dispatch_end();
    }
    if (named) {
        for (size_t i = g_vpi.static_count; i < g_vpi.slot_count; ++i) {
            llg_vpi_slot_t* slot = &g_vpi.slots[i];
            int bound = slot->kind == SLOT_CLASSOBJ || slot->kind == SLOT_PROPERTY ||
                        slot->kind == SLOT_DYNELEM ||
                        (slot->kind == SLOT_ITERATOR && slot->iter_kind == ITER_DYN);
            if (bound && slot->pointer && dyn_is_dying(slot->pointer)) release_slot((uint32_t)i);
        }
    }
    for (size_t i = 0; i < count; ++i) {
        llg_vpi_live_t* entry = live_find(objects[i]);
        if (entry) live_remove(entry);
    }
    g_dyn.reclaiming = 0;
}

static void dyn_shutdown(void) {
    if (g_dyn.hooked) llg_gc_reclaim_hook(NULL);
    free(g_dyn.live);
    memset(&g_dyn, 0, sizeof(g_dyn));
}
