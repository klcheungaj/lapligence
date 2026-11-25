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
    DYN_ITER_FIELDS = 1,   /* properties: pointer object (NULL: declaration), right class,
                            * aux the vpiType they must have (0: any variable) */
    DYN_ITER_ELEMENTS = 2, /* container elements: base row/object, lsb field */
    DYN_ITER_DERIVED = 3,  /* class defns whose base is class `right` */
    DYN_ITER_ITEMS = 4,    /* items of class `right` the relation `aux` selects
                            * (arguments: of method item `element`) */
    DYN_ITER_CLASSES = 5,  /* class defns declared in catalog scope row `right`
                            * (LLG_VPI_NONE: all) */
};

/* How a SLOT_DYNELEM names its element (slot->lsb), and each further
 * selection of its path (dyn_step_t::at). */
enum {
    DYN_AT_INDEX = 0,     /* element: dynamic array or queue index */
    DYN_AT_INTEGRAL = 1,  /* element: low 64 key bits (key width <= 64) */
    DYN_AT_STRING = 2,    /* items/count: a copy of the string key */
    DYN_AT_MEMBER = 3,    /* element: member number of a record (paths only) */
};

/* The most declared dimensions of a fixed-size array row, as many as a
 * vpi_handle_by_multi_index call names. */
#define DYN_MAX_DIMS 16u

/* One selection of an element handle's path (37.17 detail 26: the prefix
 * order of a compound name). A SLOT_DYNELEM's first selection is the slot's
 * own (lsb, element, items, count); the `span` further ones, an index or
 * key of a container nested in the element or a record member, are an
 * array of these in `use`, owned by the slot with their keys. */
typedef struct {
    uint8_t at;               /* DYN_AT_* */
    uint32_t length;          /* DYN_AT_STRING: key bytes */
    uint64_t element;         /* index, low key bits or member number */
    const char* key;          /* DYN_AT_STRING: the key */
} dyn_step_t;

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
    int32_t hooked;               /* the collector's reclaim hook is installed */
    int32_t reclaiming;
} g_dyn;

static void dyn_reclaimed(void* const* objects, size_t count);
static int32_t dyn_is_event_array(const llg_vpi_shape_t* shape);
static vpiHandle dyn_iterator(uint32_t sub_kind, uint32_t count);
static char* dyn_text(const char* format, ...);

/* ── tables ─────────────────────────────────────────────────────────────── */

int32_t llg_vpi_model_dynamic(const llg_vpi_dynamic_tables_t* tables) {
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

/* Property numbers from DYN_STATIC_FIELD on name static properties (8.9):
 * entry `field - DYN_STATIC_FIELD` of the static table, whichever class or
 * object reaches it. */
#define DYN_STATIC_FIELD 0x40000000u

/* The static property `field` names, or NULL for an instance property. */
static LLG_OUTLYING const llg_vpi_class_static_t* dyn_static(uint32_t field) {
    if (field < DYN_STATIC_FIELD || !g_dyn.tables || field - DYN_STATIC_FIELD >= g_dyn.tables->static_count)
        return NULL;
    return &g_dyn.tables->statics[field - DYN_STATIC_FIELD];
}

static const llg_vpi_class_field_t* dyn_field(uint32_t class_index, uint32_t field) {
    if (field >= DYN_STATIC_FIELD) {
        const llg_vpi_class_static_t* entry = dyn_static(field);
        return entry ? &entry->field : NULL;
    }
    const llg_vpi_class_t* layout = dyn_class(class_index);
    if (!layout || field >= layout->field_count) return NULL;
    return &g_dyn.tables->fields[layout->first_field + field];
}

/* Static property `k` of class `class_index` (with `inherited`, of its
 * base classes first and then its own, as an object lists them), as a
 * property number, or LLG_VPI_NONE. */
static LLG_OUTLYING uint32_t dyn_class_static(uint32_t class_index, int32_t inherited, uint32_t k) {
    uint32_t chain[64];
    uint32_t depth = 0;
    for (uint32_t current = class_index; depth < 64u;) {
        const llg_vpi_class_t* layout = dyn_class(current);
        if (!layout) break;
        chain[depth++] = current;
        if (!inherited || !layout->base) break;
        current = layout->base - 1u;
    }
    for (uint32_t d = depth; d-- > 0;) {
        const llg_vpi_class_t* layout = dyn_class(chain[d]);
        if (k < layout->static_count) return DYN_STATIC_FIELD + layout->first_static + k;
        k -= layout->static_count;
    }
    return LLG_VPI_NONE;
}

/* Fields a class inherits: the field count of its base layout. */
static uint32_t dyn_inherited_fields(uint32_t class_index) {
    const llg_vpi_class_t* layout = dyn_class(class_index);
    const llg_vpi_class_t* base = layout && layout->base ? dyn_class(layout->base - 1u) : NULL;
    return base ? base->field_count : 0u;
}

/* The class layout that declares field `field` of layout `class_index`
 * (a derived layout lists its base's fields first). */
static uint32_t dyn_declaring_class(uint32_t class_index, uint32_t field) {
    const llg_vpi_class_static_t* entry = dyn_static(field);
    if (entry) return entry->class_index;
    uint32_t classes = g_dyn.tables ? g_dyn.tables->class_count : 0u;
    for (uint32_t guard = 0; guard < classes; ++guard) {
        const llg_vpi_class_t* layout = dyn_class(class_index);
        if (!layout || !layout->base || field >= dyn_inherited_fields(class_index)) break;
        class_index = layout->base - 1u;
    }
    return class_index;
}

/* The located scope (decision A19-D86) of class layout `class_index`: a
 * class is no catalog scope, so the bounds and indices of its properties
 * are located at the scope that declares it. */
static uint32_t dyn_class_located_scope(uint32_t class_index) {
    const llg_vpi_class_t* layout = dyn_class(class_index);
    return layout && layout->scope ? located_scope(row_object(layout->scope - 1u)) : LLG_VPI_NONE;
}

static int32_t dyn_tables_valid(void) {
    const llg_vpi_dynamic_tables_t* t = g_dyn.tables;
    if (!t) return 1;
    if ((t->shape_count && !t->shapes) || (t->class_count && !t->classes) ||
        (t->field_count && !t->fields) || (t->class_count && (!t->object || !t->field)) ||
        (t->vif_count && !t->vifs) || (t->method_call_count && !t->method_calls) ||
        (t->member_count && !t->members) || (t->static_count && !t->statics) ||
        t->static_count >= DYN_STATIC_FIELD)
        return 0;
    for (uint32_t i = 0; i < t->method_call_count; ++i) {
        const uint32_t* call = &t->method_calls[3u * i];
        if (call[1] >= t->class_count || call[2] >= t->classes[call[1]].item_count ||
            (i && call[0] <= t->method_calls[3u * (i - 1u)]))
            return 0;
    }
    for (uint32_t i = 0; i < t->vif_count; ++i)
        if (!t->vifs[i].env || t->vifs[i].row >= g_vpi.object_count) return 0;
    uint32_t typespecs = g_vpi.types ? g_vpi.types->typespec_count : 0u;
    for (uint32_t i = 0; i < t->shape_count; ++i) {
        const llg_vpi_shape_t* shape = &t->shapes[i];
        if (shape->shape > LLG_VPI_SHAPE_RECORD || shape->class_index > t->class_count ||
            shape->typespec > typespecs || shape->width >= LLG_SUPPORTED_WIDTH_LIMIT)
            return 0;
        if (shape->shape == LLG_VPI_SHAPE_PACKED && shape->width == 0) return 0;
        if (shape->builtin > LLG_VPI_BUILTIN_SEMAPHORE ||
            (shape->builtin && (shape->shape != LLG_VPI_SHAPE_CLASS || shape->class_index)))
            return 0;
        if (shape->shape == LLG_VPI_SHAPE_CONTAINER &&
            (shape->element >= i || shape->key > LLG_VPI_KEY_OTHER ||
             (shape->array_type != vpiDynamicArray && shape->array_type != vpiQueueArray &&
              shape->array_type != vpiAssocArray && shape->array_type != vpiStaticArray)))
            return 0;
        /* A record's members come before it, so paths through values end. */
        if ((shape->shape == LLG_VPI_SHAPE_RECORD) != (shape->member_count != 0) ||
            shape->first_member > t->member_count ||
            shape->member_count > t->member_count - shape->first_member)
            return 0;
        for (uint32_t m = 0; m < shape->member_count; ++m) {
            const llg_vpi_member_t* member = &t->members[shape->first_member + m];
            if (!member->name || member->shape >= i) return 0;
        }
        /* Ranges on the shape: a fixed-size array only, at most as many
         * as a multi-index names; inline cells are values. */
        int32_t fixed = shape->shape == LLG_VPI_SHAPE_CONTAINER && shape->array_type == vpiStaticArray;
        if (shape->dim_count &&
            (!fixed || !shape->dims || shape->dim_count > DYN_MAX_DIMS || dyn_is_event_array(shape)))
            return 0;
        if (shape->inline_value && (!shape->dim_count || !shape->values)) return 0;
    }
    for (uint32_t i = 0; i < t->class_count; ++i) {
        const llg_vpi_class_t* layout = &t->classes[i];
        if (!layout->name || layout->base > t->class_count || layout->base == i + 1u ||
            layout->first_field > t->field_count ||
            layout->field_count > t->field_count - layout->first_field ||
            layout->first_static > t->static_count ||
            layout->static_count > t->static_count - layout->first_static)
            return 0;
        for (uint32_t k = 0; k < layout->static_count; ++k)
            if (t->statics[layout->first_static + k].class_index != i) return 0;
    }
    for (uint32_t i = 0; i < t->static_count; ++i) {
        const llg_vpi_class_static_t* entry = &t->statics[i];
        const llg_vpi_shape_t* shape =
            entry->field.shape < t->shape_count ? &t->shapes[entry->field.shape] : NULL;
        int32_t fixed = shape && shape->shape == LLG_VPI_SHAPE_CONTAINER &&
                    shape->array_type == vpiStaticArray;
        if (!entry->field.name || !shape || !entry->storage || entry->field.mode ||
            entry->class_index >= t->class_count ||
            (fixed ? !entry->dims || !entry->dim_count || entry->dim_count > DYN_MAX_DIMS
                   : entry->dim_count != 0u))
            return 0;
    }
    for (uint32_t i = 0; i < t->field_count; ++i) {
        const llg_vpi_class_field_t* field = &t->fields[i];
        if (!field->name) continue;
        if (field->shape >= t->shape_count || field->rand < vpiNotRand ||
            field->rand > vpiRandC || field->visibility < vpiPublicVis ||
            field->visibility > vpiLocalVis)
            return 0;
    }
    if (t->item_count && !t->items) return 0;
    uint32_t items = 0;
    for (uint32_t i = 0; i < t->class_count; ++i) {
        const llg_vpi_class_t* layout = &t->classes[i];
        if (!layout->full_name || layout->scope > g_vpi.object_count ||
            layout->first_item != items || layout->item_count > t->item_count - items)
            return 0;
        for (uint32_t f = 0; f < layout->field_count; ++f)
            if (t->fields[layout->first_field + f].mode > layout->field_count) return 0;
        for (uint32_t k = 0; k < layout->item_count; ++k) {
            const llg_vpi_class_item_t* item = &t->items[items + k];
            int32_t method = item->type == vpiTask || item->type == vpiFunction;
            if (!item->name || item->typespec > typespecs || item->mode > layout->field_count ||
                (item->code && (!g_vpi.code || item->code > g_vpi.code->node_count)))
                return 0;
            if (!method && item->type != vpiParameter && item->type != vpiConstraint &&
                item->type != vpiIODecl)
                return 0;
            /* An argument follows its method. */
            if ((item->type == vpiIODecl) !=
                    (item->owner != 0) ||
                (item->owner && (item->owner > k ||
                                 (t->items[items + item->owner - 1u].type != vpiTask &&
                                  t->items[items + item->owner - 1u].type != vpiFunction))))
                return 0;
        }
        items += layout->item_count;
    }
    return items == t->item_count;
}

/* A catalog row with LLG_VPI_OBJ_DYNAMIC: storage, a variable shape, and a
 * change marker only for handle variables. */
static int32_t dyn_row_valid(const llg_vpi_model_object_t* object) {
    const llg_vpi_shape_t* shape = object->shape ? dyn_shape(object->shape - 1u) : NULL;
    if (!shape || !object->storage || shape->shape == LLG_VPI_SHAPE_OTHER ||
        object->packed || object->real)
        return 0;
    /* A fixed-size array row carries its declared ranges, at most as many
     * as a multi-index names. */
    int32_t fixed = shape->shape == LLG_VPI_SHAPE_CONTAINER && shape->array_type == vpiStaticArray;
    if (fixed ? (!object->dim_count || object->dim_count > DYN_MAX_DIMS || !object->dims)
              : object->dim_count != 0u)
        return 0;
    if (object->dependency && shape->shape != LLG_VPI_SHAPE_CLASS &&
        shape->shape != LLG_VPI_SHAPE_CHANDLE)
        return 0;
    return object->type == shape->type;
}

static uint32_t dyn_item_count(void) { return g_dyn.tables ? g_dyn.tables->item_count : 0u; }

/* Item `item` (in the class's own numbering) of class `class_index`. */
static const llg_vpi_class_item_t* dyn_item(uint32_t class_index, uint32_t item) {
    const llg_vpi_class_t* layout = dyn_class(class_index);
    if (!layout || item >= layout->item_count) return NULL;
    return &g_dyn.tables->items[layout->first_item + item];
}

static int32_t dyn_item_is_method(const llg_vpi_class_item_t* item) {
    return item->type == vpiTask || item->type == vpiFunction;
}

/* Three static slots per class (its defn, its typespec, its extends), then
 * one per declared item. */
static size_t dyn_static_slot_count(void) {
    return (size_t)dyn_class_count() * 3u + dyn_item_count();
}

static void dyn_static_slots(uint32_t base) {
    g_dyn.class_base = base;
    uint32_t classes = dyn_class_count();
    for (uint32_t i = 0; i < classes * 3u; ++i) {
        llg_vpi_slot_t* slot = slot_at(base + i);
        slot->kind = i < classes ? SLOT_CLASSDEFN : i < 2u * classes ? SLOT_CLASSTS : SLOT_EXTENDS;
        slot->aux = (int32_t)(i % classes);
    }
    for (uint32_t c = 0; c < classes; ++c) {
        const llg_vpi_class_t* layout = dyn_class(c);
        for (uint32_t k = 0; k < layout->item_count; ++k) {
            llg_vpi_slot_t* slot = slot_at(base + classes * 3u + layout->first_item + k);
            slot->kind = SLOT_CLASSITEM;
            slot->aux = (int32_t)k;
            slot->right = c;
        }
    }
}

static vpiHandle dyn_class_handle(uint32_t class_index, uint32_t which) {
    if (class_index >= dyn_class_count()) return NULL;
    return static_handle(g_dyn.class_base + which * dyn_class_count() + class_index);
}

static vpiHandle defn_handle(uint32_t class_index) { return dyn_class_handle(class_index, 0); }

/* The static handle of item `item` of class `class_index`. */
static vpiHandle item_handle(uint32_t class_index, uint32_t item) {
    const llg_vpi_class_t* layout = dyn_class(class_index);
    if (!layout || item >= layout->item_count) return NULL;
    return static_handle(g_dyn.class_base + dyn_class_count() * 3u + layout->first_item + item);
}
static vpiHandle class_typespec_handle(uint32_t class_index) {
    return dyn_class_handle(class_index, 1);
}

/* The method item a call of a class method (code row `row`) names, or
 * NULL when the row calls none. */
static vpiHandle dyn_method_call_item(uint32_t row) {
    const llg_vpi_dynamic_tables_t* t = g_dyn.tables;
    uint32_t low = 0, high = t ? t->method_call_count : 0u;
    while (low < high) {
        uint32_t middle = low + (high - low) / 2u;
        const uint32_t* call = &t->method_calls[3u * middle];
        if (call[0] == row) return item_handle(call[1], call[2]);
        if (call[0] < row) low = middle + 1u;
        else high = middle;
    }
    return NULL;
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

static void live_grow(void) {
    size_t capacity = g_dyn.live_capacity ? g_dyn.live_capacity * 2u : 64u;
    llg_vpi_live_t* table = (llg_vpi_live_t*)vpi_calloc(capacity, sizeof(*table));
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
    if ((g_dyn.live_count + 1u) * 4u > g_dyn.live_capacity * 3u) live_grow();
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
static int32_t dyn_object_class(void* object, uint32_t* class_index) {
    uint32_t id = 0;
    if (!object || !g_dyn.tables || !g_dyn.tables->object || !g_dyn.tables->object(object, &id) ||
        id >= dyn_class_count())
        return 0;
    *class_index = id;
    return 1;
}

static vpiHandle make_object(void* object) {
    llg_vpi_live_t* entry = live_get(object);
    vpiHandle handle;
    llg_vpi_slot_t* slot = alloc_slot(SLOT_CLASSOBJ, &handle);
    if (!slot) return NULL;
    entry->named = 1;
    slot->pointer = object;
    return handle;
}

/* The class object a handle of `shape` names: a mailbox or semaphore is a
 * built-in object the runtime owns, never read through the class layout
 * accessor. */
static vpiHandle make_shape_object(const llg_vpi_shape_t* shape, void* object) {
    if (!shape || !shape->builtin) return make_object(object);
    vpiHandle handle;
    llg_vpi_slot_t* slot = alloc_slot(SLOT_BUILTINOBJ, &handle);
    if (!slot) return NULL;
    slot->pointer = object;
    slot->aux = (int32_t)shape->builtin;
    return handle;
}

static vpiHandle make_property(void* object, uint32_t class_index, uint32_t field) {
    if (object) {
        live_get(object)->named = 1;
    }
    vpiHandle handle;
    llg_vpi_slot_t* slot = alloc_slot(SLOT_PROPERTY, &handle);
    if (!slot) return NULL;
    slot->pointer = object;
    slot->aux = (int32_t)field;
    slot->right = class_index;
    return handle;
}

/* The handle of static property `field` (a property number from
 * DYN_STATIC_FIELD): one object for the class and every object of it. */
static LLG_OUTLYING vpiHandle make_static_property(uint32_t field) {
    const llg_vpi_class_static_t* entry = dyn_static(field);
    return entry ? make_property(NULL, entry->class_index, field) : NULL;
}

/* Item `item` of class `class_index` reached from class object `object`: a
 * constraint, whose vpiIsConstraintEnabled and vpiParent are the object's
 * (37.30). The handle tracks the object like a property handle. */
static vpiHandle object_item_handle(void* object, uint32_t class_index, uint32_t item) {
    llg_vpi_live_t* entry = live_get(object);
    vpiHandle handle;
    llg_vpi_slot_t* slot = alloc_slot(SLOT_CLASSITEM, &handle);
    if (!slot) return NULL;
    entry->named = 1;
    slot->pointer = object;
    slot->aux = (int32_t)item;
    slot->right = class_index;
    return handle;
}

/* ── places: the storage a handle names ─────────────────────────────────── */

typedef struct {
    const llg_vpi_shape_t* shape;
    void* storage;            /* NULL: a declaration without storage */
    void* owner;              /* the class object holding the storage, or NULL */
    uint32_t row;             /* the catalog row, LLG_VPI_NONE, or DYN_AUTO_ROW */
    const char* name;         /* the variable's name */
    int64_t serial;           /* DYN_AUTO_ROW: the frame serial */
    uint32_t class_index;     /* a property's class layout */
    /* DYN_AUTO_ROW: the declared (left, right) ranges of a fixed-size
     * array, leftmost first. */
    const int64_t* dims;
    uint32_t dim_count;
} dyn_place_t;

/* The `row` of an automatic container variable's elements and element
 * iterators: `right` holds the frame serial and the field (an element's
 * `aux`, an iterator's `lsb`) the variable's index in its frame. */
#define DYN_AUTO_ROW (LLG_VPI_NONE - 2u)

/* The `row` of a static property's elements and element iterators: the
 * field (an element's `aux`, an iterator's `lsb`) is the property. */
#define DYN_STATIC_ROW (LLG_VPI_NONE - 4u)

/* The `row` of an event of a class definition's named event array
 * property (37.25): `aux` holds the property and `right` the class layout.
 * It names no storage (37.27 detail 2). */
#define DYN_DEFN_ROW (LLG_VPI_NONE - 3u)

/* llg_vpi_runtime.c: the container automatic `index` of frame `serial`,
 * or 0 once that frame has ended; the same for a container or record
 * automatic; its vpiFullName prefix (the frame). */
static int32_t rts_auto_container(uint64_t serial, uint32_t index, dyn_place_t* place);
static int32_t rts_auto_value_place(uint64_t serial, uint32_t index, dyn_place_t* place);
static const char* rts_auto_frame_name(uint64_t serial);
/* llg_vpi_runtime.c: where the automatic `index` of frame `serial` is
 * declared (37.3.3), 0 once that frame has ended; the located scope of its
 * subprogram (decision A19-D86). */
static int32_t rts_auto_location(uint64_t serial, uint32_t index, uint32_t* line, const char** file);
static uint32_t rts_auto_scope(uint64_t serial);
static vpiHandle rts_auto_handle_by_serial(uint64_t serial, uint32_t index);

/* Whether `shape` is a named event array class property (37.25): a
 * fixed-size container of events with its declared range. */
static int32_t dyn_is_event_array(const llg_vpi_shape_t* shape) {
    return shape && shape->shape == LLG_VPI_SHAPE_CONTAINER && shape->type == vpiNamedEventArray;
}

/* The number of declared ranges of a fixed-size array row or automatic
 * (37.17) or of a named event array property (37.25); 0 for any other
 * container (one reached through a class property or nested in a value). */
static LLG_OUTLYING uint32_t dyn_fixed_dim_count(const dyn_place_t* place) {
    if (dyn_is_event_array(place->shape)) return 1u;
    if (place->shape->shape != LLG_VPI_SHAPE_CONTAINER ||
        place->shape->array_type != vpiStaticArray)
        return 0u;
    /* A class instance property or an array member of a structure value:
     * its shape carries the ranges. */
    if (place->shape->dim_count) return place->shape->dim_count;
    if (place->row == LLG_VPI_NONE) return 0u;
    if (place->row == DYN_AUTO_ROW || place->row == DYN_STATIC_ROW)
        return place->dims ? place->dim_count : 0u;
    const llg_vpi_model_object_t* row = row_object(place->row);
    return row ? row->dim_count : 0u;
}

/* Declared range `d` (leftmost first) of a fixed-size array place. */
static LLG_OUTLYING void dyn_fixed_dim(const dyn_place_t* place, uint32_t d, int64_t* left, int64_t* right) {
    if (dyn_is_event_array(place->shape)) {
        *left = place->shape->left;
        *right = place->shape->right;
        return;
    }
    if (place->shape->dim_count) {
        *left = place->shape->dims[2u * d];
        *right = place->shape->dims[2u * d + 1u];
        return;
    }
    const int64_t* dims = place->row == DYN_AUTO_ROW || place->row == DYN_STATIC_ROW
                              ? place->dims : row_object(place->row)->dims;
    *left = dims[2u * d];
    *right = dims[2u * d + 1u];
}

/* The declared range of a one-dimensional fixed-size array: its elements
 * are numbered from `left`. */
static int32_t dyn_fixed_range(const dyn_place_t* place, int64_t* left, int64_t* right) {
    if (dyn_fixed_dim_count(place) != 1u) return 0;
    dyn_fixed_dim(place, 0, left, right);
    return 1;
}

/* The declared indices, leftmost first, of element `offset` of a
 * fixed-size array (its cells are row-major), or the offset itself for
 * other containers; returns how many. */
static LLG_OUTLYING uint32_t dyn_declared_indices(const dyn_place_t* place, uint64_t offset, int64_t* indices) {
    uint32_t count = dyn_fixed_dim_count(place);
    if (!count) {
        indices[0] = (int64_t)offset;
        return 1u;
    }
    for (uint32_t d = count; d-- > 0;) {
        int64_t left, right;
        dyn_fixed_dim(place, d, &left, &right);
        uint64_t extent = (uint64_t)(left > right ? left - right : right - left) + 1u;
        uint64_t index = offset % extent;
        offset /= extent;
        indices[d] = left >= right ? left - (int64_t)index : left + (int64_t)index;
    }
    return count;
}

/* The declared index of element `offset` of a one-dimensional fixed-size
 * array, or the offset itself for other containers. */
static int64_t dyn_declared_index(const dyn_place_t* place, uint64_t offset) {
    int64_t indices[DYN_MAX_DIMS];
    dyn_declared_indices(place, offset, indices);
    return indices[0];
}

/* The cell of the declared `indices` (one per range) of a fixed-size
 * array; 0, with an error unless `quiet`, when one is outside its range. */
static LLG_OUTLYING int32_t dyn_fixed_offset(const dyn_place_t* place, const int64_t* indices, uint64_t* offset,
                            int32_t quiet) {
    uint32_t count = dyn_fixed_dim_count(place);
    uint64_t cell = 0;
    for (uint32_t d = 0; d < count; ++d) {
        int64_t left, right;
        dyn_fixed_dim(place, d, &left, &right);
        int64_t low = left < right ? left : right, high = left < right ? right : left;
        if (indices[d] < low || indices[d] > high) {
            if (!quiet) vpi_error("LLG_VPI_INDEX", "index outside the array range");
            return 0;
        }
        uint64_t extent = (uint64_t)(high - low) + 1u;
        cell = cell * extent + (uint64_t)(left >= right ? left - indices[d] : indices[d] - left);
    }
    *offset = cell;
    return 1;
}

static int32_t dyn_is_dying(const void* object) {
    const llg_vpi_live_t* entry = object ? live_find(object) : NULL;
    return entry && entry->dying;
}

/* The storage of field `field` of a live object, when its slot kind agrees
 * with the field's shape. */
static void* dyn_field_storage(void* object, uint32_t field, const llg_vpi_shape_t* shape) {
    const llg_vpi_class_static_t* entry = dyn_static(field);
    if (entry) return entry->storage;
    uint32_t kind = 0;
    void* storage = g_dyn.tables->field(object, field, &kind);
    if (!storage || !shape) return NULL;
    switch (shape->shape) {
        case LLG_VPI_SHAPE_PACKED: return kind == LLG_VPI_FIELD_PACKED ? storage : NULL;
        case LLG_VPI_SHAPE_REAL: return kind == LLG_VPI_FIELD_REAL ? storage : NULL;
        case LLG_VPI_SHAPE_STRING: return kind == LLG_VPI_FIELD_STRING ? storage : NULL;
        case LLG_VPI_SHAPE_CLASS:
        case LLG_VPI_SHAPE_CHANDLE:
        case LLG_VPI_SHAPE_EVENT:
        case LLG_VPI_SHAPE_VIF: return kind == LLG_VPI_FIELD_HANDLE ? storage : NULL;
        case LLG_VPI_SHAPE_CONTAINER: return kind == LLG_VPI_FIELD_PAYLOAD ? storage : NULL;
        /* A record value's root starts with its llg_value_t. */
        case LLG_VPI_SHAPE_RECORD: return kind == LLG_VPI_FIELD_PAYLOAD ? storage : NULL;
        default: return NULL;
    }
}

/* The place of a dynamic row or a property handle. */
static int32_t dyn_place(const llg_vpi_slot_t* slot, dyn_place_t* place) {
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
    place->class_index = class_index;
    const llg_vpi_class_static_t* entry = dyn_static((uint32_t)slot->aux);
    if (entry) {
        /* A static property is one variable for the class and its
         * objects (8.9). */
        place->storage = entry->storage;
        place->row = DYN_STATIC_ROW;
        place->dims = entry->dims;
        place->dim_count = entry->dim_count;
        return place->shape != NULL;
    }
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
static int32_t dyn_container_place(uint32_t row, void* owner, uint32_t field, int64_t serial,
                               dyn_place_t* place) {
    memset(place, 0, sizeof(*place));
    place->row = LLG_VPI_NONE;
    if (row == DYN_AUTO_ROW) {
        /* A record automatic is the top of its members' paths. */
        if (!rts_auto_value_place((uint64_t)serial, field, place)) return 0;
        return 1;
    } else if (row == DYN_STATIC_ROW) {
        const llg_vpi_class_static_t* entry = dyn_static(field);
        if (!entry) return 0;
        place->shape = dyn_shape(entry->field.shape);
        place->storage = entry->storage;
        place->row = DYN_STATIC_ROW;
        place->name = entry->field.name;
        place->class_index = entry->class_index;
        place->dims = entry->dims;
        place->dim_count = entry->dim_count;
    } else if (owner) {
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
    /* A structure property is the top of its members' paths (A19-D89). */
    return place->shape && place->storage &&
           (place->shape->shape == LLG_VPI_SHAPE_CONTAINER || place->shape->shape == LLG_VPI_SHAPE_RECORD);
}

static size_t dyn_container_size(const dyn_place_t* place) {
    const llg_vpi_shape_t* shape = place->shape;
    void* storage = place->storage;
    /* A nested container in its null default is empty. */
    if (!storage) return 0;
    /* An array member of a structure value: one fixed-array value. */
    if (shape->inline_value) return ((const llg_value_t*)storage)->desc->item_count;
    switch (shape->array_type) {
        /* A fixed array keeps dynamic array storage. */
        case vpiDynamicArray:
        case vpiStaticArray:
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

/* Free the path a SLOT_DYNELEM owns (release_slot). */
static LLG_OUTLYING void dyn_release_path(llg_vpi_slot_t* slot) {
    dyn_step_t* steps = (dyn_step_t*)(uintptr_t)slot->use;
    for (uint32_t i = 0; steps && i < slot->span; ++i) free((void*)(uintptr_t)steps[i].key);
    free(steps);
    slot->use = 0;
    slot->span = 0;
}

/* A copy of `count` bytes of `key` with a terminating NUL. */
static LLG_OUTLYING char* dyn_key_copy(const char* key, size_t count) {
    char* copy = (char*)vpi_malloc(count + 1u);
    if (count) memcpy(copy, key, count);
    copy[count] = '\0';
    return copy;
}

/* Selection `i` of element slot `slot`'s path (0: its own). */
static LLG_OUTLYING dyn_step_t dyn_step_at(const llg_vpi_slot_t* slot, uint32_t i) {
    if (i) return ((const dyn_step_t*)(uintptr_t)slot->use)[i - 1u];
    dyn_step_t step;
    step.at = (uint8_t)slot->lsb;
    step.length = slot->count;
    step.element = slot->element;
    step.key = (const char*)(const void*)slot->items;
    return step;
}

/* An element handle of the container `place` (property `field` of its
 * owner, or automatic `field` of its frame) through `count` selections. */
static LLG_OUTLYING vpiHandle make_dyn_path(const dyn_place_t* place, uint32_t field, const dyn_step_t* steps,
                               uint32_t count) {
    char* first = NULL;
    dyn_step_t* rest = NULL;
    if (steps[0].at == DYN_AT_STRING) first = dyn_key_copy(steps[0].key, steps[0].length);
    if (count > 1u) {
        rest = (dyn_step_t*)vpi_calloc(count - 1u, sizeof(*rest));
        for (uint32_t i = 1; i < count; ++i) {
            rest[i - 1u] = steps[i];
            rest[i - 1u].key = steps[i].at == DYN_AT_STRING
                ? dyn_key_copy(steps[i].key, steps[i].length) : NULL;
        }
    }
    if (place->owner) {
        live_get(place->owner)->named = 1;
    }
    vpiHandle handle;
    llg_vpi_slot_t* slot = alloc_slot(SLOT_DYNELEM, &handle);
    if (!slot) {
        for (uint32_t i = 0; rest && i + 1u < count; ++i) free((void*)(uintptr_t)rest[i].key);
        free(rest);
        free(first);
        return NULL;
    }
    slot->object = place->row;
    slot->right = place->serial;
    slot->pointer = place->owner;
    slot->aux = (int32_t)field;
    slot->lsb = steps[0].at;
    slot->element = steps[0].element;
    slot->items = (uintptr_t*)(void*)first; /* freed by release_slot */
    slot->count = steps[0].length;
    slot->span = count - 1u;
    slot->use = (uintptr_t)rest; /* freed by release_slot (dyn_release_path) */
    return handle;
}

/* An element handle; `key`/`key_length` copy a string key. */
static vpiHandle make_dyn_element(const dyn_place_t* place, uint32_t field, uint32_t at,
                                  uint64_t element, const uint8_t* key, size_t key_length) {
    dyn_step_t step;
    step.at = (uint8_t)at;
    step.element = element;
    step.key = (const char*)key;
    step.length = (uint32_t)key_length;
    return make_dyn_path(place, field, &step, 1u);
}

/* A string key longer than a slot can record has no element handle. */
#define LLG_VPI_MAX_ELEMENT_KEY UINT32_MAX

/* The selection of assoc entry `position` (key order) of `place`; 0, with
 * an error for a key no handle can name, when there is none. */
static LLG_OUTLYING int32_t dyn_assoc_step(const dyn_place_t* place, size_t position, dyn_step_t* step) {
    const sv4_t* key = NULL;
    const uint8_t* text = NULL;
    size_t length = 0;
    int32_t string_key;
    if (place->shape->values) {
        const llg_assoc_value_t* array = (const llg_assoc_value_t*)place->storage;
        if (position >= array->size) return 0;
        string_key = array->key_kind == LLG_ASSOC_STRING;
        key = &array->entries[position].integral_key;
        text = array->entries[position].string_key;
        length = array->entries[position].string_length;
    } else {
        const llg_assoc_t* array = (const llg_assoc_t*)place->storage;
        if (position >= array->size) return 0;
        string_key = array->key_kind == LLG_ASSOC_STRING;
        key = &array->entries[position].integral_key;
        text = array->entries[position].string_key;
        length = array->entries[position].string_length;
    }
    memset(step, 0, sizeof(*step));
    if (string_key) {
        if (length > LLG_VPI_MAX_ELEMENT_KEY) {
            vpi_error("LLG_VPI_UNSUPPORTED", "this associative array key is too long for a handle");
            return 0;
        }
        step->at = DYN_AT_STRING;
        step->key = (const char*)text;
        step->length = (uint32_t)length;
        return 1;
    }
    if (llg_sv4_width(*key) > 64u || llg_sv4_has_x(*key) || llg_sv4_has_z(*key)) {
        vpi_error("LLG_VPI_UNSUPPORTED",
                  "llg names associative array elements by keys of at most 64 known bits");
        return 0;
    }
    step->at = DYN_AT_INTEGRAL;
    step->element = sv4_to_u64(*key);
    return 1;
}

/* The element at assoc entry `position` (key order), or NULL. */
static vpiHandle dyn_assoc_entry(const dyn_place_t* place, uint32_t field, size_t position) {
    dyn_step_t step;
    return dyn_assoc_step(place, position, &step) ? make_dyn_path(place, field, &step, 1u) : NULL;
}

/* Locate the element `step` selects in `place`. Packed containers report
 * through `exists` only; value containers also return the element. */
static LLG_OUTLYING llg_value_t* dyn_locate_step(const dyn_place_t* place, const dyn_step_t* step,
                                    int32_t* exists) {
    const llg_vpi_shape_t* shape = place->shape;
    *exists = 0;
    if (!place->storage || step->at == DYN_AT_MEMBER) return NULL;
    if (step->at == DYN_AT_INDEX) {
        if (step->element >= dyn_container_size(place)) return NULL;
        *exists = 1;
        if (shape->inline_value) return &((llg_value_t*)place->storage)->value.items[step->element];
        if (!shape->values) return NULL;
        sv4_t index = sv4_from_u64(step->element, 64, 0);
        llg_value_t* element = (shape->array_type == vpiDynamicArray || shape->array_type == vpiStaticArray)
            ? llg_dyn_value_element((llg_dyn_value_array_t*)place->storage, &index, 1)
            : llg_queue_value_element((llg_queue_value_array_t*)place->storage, &index, 1);
        sv4_destroy(&index);
        *exists = element != NULL;
        return element;
    }
    if (step->at == DYN_AT_STRING) {
        const char* key = step->key;
        if (shape->values) {
            /* A missing key is no read: test it first so the locator does
             * not report a read miss. */
            llg_assoc_value_t* array = (llg_assoc_value_t*)place->storage;
            if (!llg_assoc_value_exists_string(array, key, step->length)) return NULL;
            llg_value_t* element = llg_assoc_value_element_string(array, key, step->length, 0);
            *exists = element != NULL;
            return element;
        }
        *exists = llg_assoc_exists_string((const llg_assoc_t*)place->storage, key, step->length);
        return NULL;
    }
    sv4_t key = dyn_integral_key(place, step->element);
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

/* Locate the element an element slot's own selection names. */
static llg_value_t* dyn_locate(const dyn_place_t* place, const llg_vpi_slot_t* slot,
                               int32_t* exists) {
    dyn_step_t step = dyn_step_at(slot, 0);
    return dyn_locate_step(place, &step, exists);
}

/* The container nested in element value `value` of shape `shape` (a
 * container shape) as a place: the variable-form storage its value holds,
 * NULL for the empty null default (C2 nested sequences, nested
 * associative arrays). 0 when the value is not of that shape. */
static LLG_OUTLYING int32_t dyn_nested_level(llg_value_t* value, const llg_vpi_shape_t* shape, dyn_place_t* level) {
    memset(level, 0, sizeof(*level));
    level->row = LLG_VPI_NONE;
    level->shape = shape;
    if (!value || !value->desc || shape->shape != LLG_VPI_SHAPE_CONTAINER) return 0;
    if (shape->inline_value) {
        /* An array member of a structure value: the value is the array. */
        if (value->desc->kind != LLG_VALUE_FIXED_ARRAY || !value->value.items) return 0;
        uint64_t cells = 1u;
        for (uint32_t d = 0; d < shape->dim_count; ++d) {
            int64_t left = shape->dims[2u * d], right = shape->dims[2u * d + 1u];
            cells *= (uint64_t)(left > right ? left - right : right - left) + 1u;
        }
        if (value->desc->item_count != cells) return 0;
        level->storage = value;
        return 1;
    }
    if (shape->array_type == vpiAssocArray) {
        if (value->desc->kind != LLG_VALUE_ASSOC) return 0;
        llg_nested_assoc_t* nested = value->value.assoc;
        if (nested)
            level->storage = shape->values ? (void*)&nested->array.values
                                           : (void*)&nested->array.packed;
        return 1;
    }
    if (value->desc->kind != LLG_VALUE_CONTAINER) return 0;
    llg_nested_seq_t* seq = value->value.container;
    if (!seq) return 1;
    int32_t queue = shape->array_type == vpiQueueArray;
    switch (seq->kind) {
        case LLG_NESTED_SEQ_DYN:
            if (queue || shape->values) return 0;
            level->storage = &seq->array.dyn;
            return 1;
        case LLG_NESTED_SEQ_QUEUE:
            if (!queue || shape->values) return 0;
            level->storage = &seq->array.queue;
            return 1;
        case LLG_NESTED_SEQ_DYN_VALUES:
            if (queue || !shape->values) return 0;
            level->storage = &seq->array.dyn_values;
            return 1;
        default:
            if (!queue || !shape->values) return 0;
            level->storage = &seq->array.queue_values;
            return 1;
    }
}

/* The record member `member` of a record shape, or NULL. */
static LLG_OUTLYING const llg_vpi_member_t* dyn_member(const llg_vpi_shape_t* shape, uint64_t member) {
    if (!shape || shape->shape != LLG_VPI_SHAPE_RECORD || member >= shape->member_count) return NULL;
    return &g_dyn.tables->members[shape->first_member + member];
}

/* What an element slot names, resolved at this access (decision A19-D23):
 * the array its path starts in, the container of its last index or key,
 * its shape and its value storage (NULL for a packed element of `level`,
 * read and written by `step`). */
typedef struct {
    dyn_place_t top;
    dyn_place_t level;
    const llg_vpi_shape_t* shape;
    llg_value_t* value;
    dyn_step_t step;
    int32_t packed;
} dyn_target_t;

/* Resolve the first `count` selections of `slot` (a SLOT_DYNELEM of
 * storage); 0 with an error when its array or any of them no longer
 * exists. */
static LLG_OUTLYING int32_t dyn_resolve_steps(const llg_vpi_slot_t* slot, uint32_t count, dyn_target_t* t) {
    memset(t, 0, sizeof(*t));
    if (!dyn_container_place(slot->object, slot->pointer, (uint32_t)slot->aux, slot->right, &t->top)) {
        vpi_error("LLG_VPI_HANDLE", "the array of this element no longer exists");
        return 0;
    }
    t->level = t->top;
    if (t->top.shape->shape == LLG_VPI_SHAPE_RECORD) {
        /* A record automatic: the path starts with one of its members. */
        t->shape = t->top.shape;
        t->value = (llg_value_t*)t->top.storage;
    }
    for (uint32_t i = 0; i < count; ++i) {
        dyn_step_t step = dyn_step_at(slot, i);
        if (step.at == DYN_AT_MEMBER) {
            const llg_vpi_member_t* member = dyn_member(t->shape, step.element);
            if (!member || t->packed || !t->value || !t->value->desc ||
                t->value->desc->kind != LLG_VALUE_AGGREGATE ||
                step.element >= t->value->desc->member_count) {
                vpi_error("LLG_VPI_MODEL", "a structure element disagrees with its layout");
                return 0;
            }
            t->value = &t->value->value.items[step.element];
            t->shape = dyn_shape(member->shape);
            continue;
        }
        if (i && (t->packed || !dyn_nested_level(t->value, t->shape, &t->level))) {
            vpi_error("LLG_VPI_MODEL", "a nested array disagrees with its layout");
            return 0;
        }
        int32_t exists;
        llg_value_t* element = dyn_locate_step(&t->level, &step, &exists);
        if (!exists) {
            vpi_error("LLG_VPI_INDEX", "this element no longer exists (decision A19-D23)");
            return 0;
        }
        t->shape = dyn_shape(t->level.shape->element);
        t->value = element;
        t->step = step;
        t->packed = !t->level.shape->values;
    }
    return t->shape != NULL;
}

/* Resolve every selection of `slot` (dyn_resolve_steps). */
static LLG_OUTLYING int32_t dyn_resolve(const llg_vpi_slot_t* slot, dyn_target_t* t) {
    return dyn_resolve_steps(slot, 1u + slot->span, t);
}

/* The position of the first selection after the last member selection of
 * `slot`'s path (0: none): the indices from there select an element of the
 * array that member is (37.24), or of the array the path starts in. */
static LLG_OUTLYING uint32_t dyn_tail_first(const llg_vpi_slot_t* slot) {
    uint32_t first = 0;
    for (uint32_t i = 0; i <= slot->span; ++i)
        if (dyn_step_at(slot, i).at == DYN_AT_MEMBER) first = i + 1u;
    return first;
}

/* The array selection `first` of `slot`'s path selects in (dyn_tail_first):
 * the array the path starts in, or an array member of a structure. 0 with
 * an error when it no longer exists. */
static LLG_OUTLYING int32_t dyn_tail_level(const llg_vpi_slot_t* slot, uint32_t first, dyn_place_t* level) {
    if (!first) {
        if (dyn_container_place(slot->object, slot->pointer, (uint32_t)slot->aux, slot->right, level))
            return 1;
        vpi_error("LLG_VPI_HANDLE", "the array of this element no longer exists");
        return 0;
    }
    dyn_target_t t;
    if (!dyn_resolve_steps(slot, first, &t)) return 0;
    if (t.packed || !dyn_nested_level(t.value, t.shape, level)) {
        vpi_error("LLG_VPI_MODEL", "a nested array disagrees with its layout");
        return 0;
    }
    level->owner = t.top.owner;
    return 1;
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
        /* A fixed array keeps dynamic array storage. */
        case vpiDynamicArray:
        case vpiStaticArray: llg_dyn_value_touch((llg_dyn_value_array_t*)place->storage); break;
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
            /* The pointer as a 64-bit unsigned two-state image, 0 for null
             * (decision A19-D21). */
            datum.packed = sv4_from_u64((uint64_t)(uintptr_t)*(void* const*)storage, 64, 0);
            fill_value(&datum, 0, vpiLongIntVar, output);
            datum_destroy(&datum);
            return;
        case LLG_VPI_SHAPE_CONTAINER:
            vpi_error("LLG_VPI_VALUE", "an array variable has no value; access its elements (37.17 detail 11)");
            return;
        case LLG_VPI_SHAPE_RECORD:
            vpi_error("LLG_VPI_VALUE",
                      "an unpacked structure has no value; access its members (37.17 detail 11)");
            return;
        default:
            vpi_error("LLG_VPI_VALUE", "this VPI object has no value");
            return;
    }
}

/* Convert `input` for a packed or real destination of `shape`. */
static int32_t dyn_packed_input(const llg_vpi_shape_t* shape, const s_vpi_value* input, sv4_t* out) {
    llg_vpi_datum_t datum;
    if (!value_from_vpi(input, shape->width, shape->is_signed, 0, &datum)) return 0;
    *out = datum.is_real ? sv4_from_real(datum.real, shape->width, shape->is_signed)
                         : sv4_resize(datum.packed, shape->width, shape->is_signed);
    if (shape->two_state) sv4_replace(out, sv4_to_two_state(*out));
    datum_destroy(&datum);
    return 1;
}

static int32_t dyn_real_input(const llg_vpi_shape_t* shape, const s_vpi_value* input, double* out) {
    llg_vpi_datum_t datum;
    if (!value_from_vpi(input, 0, 0, 1, &datum)) return 0;
    double value = datum.is_real ? datum.real : sv4_to_real(datum.packed);
    if (shape->shortreal) value = (double)(float)value;
    datum_destroy(&datum);
    *out = value;
    return 1;
}

/* The string a put stores: vpiStringVal text, or a vpiIntVal converted as
 * an integral cast to string (6.16: 32 bits, 8 per character, every zero
 * byte removed; decision A19-D21). Owned by the caller. */
static int32_t dyn_string_input(const s_vpi_value* input, llg_string_t* out) {
    if (input && input->format == vpiIntVal) {
        uint32_t bits = (uint32_t)input->value.integer;
        char bytes[4];
        size_t count = 0;
        for (int32_t shift = 24; shift >= 0; shift -= 8) {
            char byte = (char)((bits >> shift) & 0xffu);
            if (byte) bytes[count++] = byte;
        }
        *out = llg_string_bytes(bytes, count);
        return 1;
    }
    if (!input || input->format != vpiStringVal || !input->value.str) {
        vpi_error("LLG_VPI_VALUE",
                  "a string receives a vpiStringVal or vpiIntVal value (decision A19-D21)");
        return 0;
    }
    *out = llg_string_bytes(input->value.str, strlen(input->value.str));
    return 1;
}

/* The pointer a chandle put stores: a vpiIntVal zero-extended, any other
 * format as a 64-bit unsigned value with x and z bits 0 (decision
 * A19-D21). */
static int32_t dyn_chandle_input(const s_vpi_value* input, void** out) {
    if (input && input->format == vpiIntVal) {
        *out = (void*)(uintptr_t)(uint32_t)input->value.integer;
        return 1;
    }
    if (input && input->format == vpiRealVal) {
        vpi_error("LLG_VPI_FORMAT", "a chandle receives no vpiRealVal value (decision A19-D21)");
        return 0;
    }
    llg_vpi_datum_t datum;
    if (!value_from_vpi(input, 64, 0, 0, &datum)) return 0;
    sv4_t known = sv4_to_two_state(datum.packed);
    *out = (void*)(uintptr_t)sv4_to_u64(known);
    sv4_destroy(&known);
    datum_destroy(&datum);
    return 1;
}

/* Write a property or variable (vpiNoDelay); `dependency` is the change
 * marker of a chandle variable, or NULL. */
static void dyn_write(const llg_vpi_shape_t* shape, void* storage, sv4_t* dependency,
                      const s_vpi_value* input) {
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
            llg_string_t value;
            if (!dyn_string_input(input, &value)) return;
            /* The store publishes the string's own change marker. */
            llg_string_move_take((llg_string_t*)storage, &value);
            llg_string_destroy(&value);
            return;
        }
        case LLG_VPI_SHAPE_CHANDLE: {
            void* value;
            if (!dyn_chandle_input(input, &value)) return;
            void** target = (void**)storage;
            int32_t changed = *target != value;
            *target = value;
            if (changed && dependency) llg_dependency_changed(dependency);
            return;
        }
        default:
            vpi_error("LLG_VPI_VALUE", "this VPI object cannot receive a value");
            return;
    }
}

/* A packed element of `level` that `step` selects: an owned copy. */
static LLG_OUTLYING sv4_t dyn_packed_get(const dyn_place_t* level, const dyn_step_t* step) {
    sv4_t value;
    if (step->at == DYN_AT_INDEX) {
        sv4_t index = sv4_from_u64(step->element, 64, 0);
        value = (level->shape->array_type == vpiDynamicArray || level->shape->array_type == vpiStaticArray)
            ? llg_dyn_get((const llg_dyn_array_t*)level->storage, index)
            : llg_queue_get((const llg_queue_t*)level->storage, index);
        sv4_destroy(&index);
    } else if (step->at == DYN_AT_STRING) {
        value = llg_assoc_get_string((const llg_assoc_t*)level->storage, step->key, step->length);
    } else {
        sv4_t key = dyn_integral_key(level, step->element);
        value = llg_assoc_get_integral((const llg_assoc_t*)level->storage, key);
        sv4_destroy(&key);
    }
    return value;
}

/* Write a packed element of `level` that `step` selects. */
static LLG_OUTLYING void dyn_packed_set(const dyn_place_t* level, const dyn_step_t* step, sv4_t value) {
    if (step->at == DYN_AT_INDEX) {
        sv4_t index = sv4_from_u64(step->element, 64, 0);
        if ((level->shape->array_type == vpiDynamicArray || level->shape->array_type == vpiStaticArray))
            (void)llg_dyn_set((llg_dyn_array_t*)level->storage, index, value);
        else
            (void)llg_queue_set((llg_queue_t*)level->storage, index, value);
        sv4_destroy(&index);
    } else if (step->at == DYN_AT_STRING) {
        (void)llg_assoc_set_string((llg_assoc_t*)level->storage, step->key, step->length, value);
    } else {
        sv4_t key = dyn_integral_key(level, step->element);
        (void)llg_assoc_set_integral((llg_assoc_t*)level->storage, key, value);
        sv4_destroy(&key);
    }
}

static void dyn_element_get(const llg_vpi_slot_t* slot, p_vpi_value output) {
    dyn_target_t target;
    if (!dyn_resolve(slot, &target)) return;
    if (!target.packed) {
        if (target.shape->shape == LLG_VPI_SHAPE_RECORD ||
            target.shape->shape == LLG_VPI_SHAPE_CONTAINER) {
            dyn_read(target.shape, NULL, output);
            return;
        }
        void* storage = dyn_value_storage(target.value, target.shape);
        if (!storage) {
            vpi_error("LLG_VPI_VALUE", "this element has no value llg exposes");
            return;
        }
        dyn_read(target.shape, storage, output);
        return;
    }
    /* Packed containers return an owned copy of the element. */
    sv4_t value = dyn_packed_get(&target.level, &target.step);
    dyn_read(target.shape, &value, output);
    sv4_destroy(&value);
}

static void dyn_element_put(const llg_vpi_slot_t* slot, const s_vpi_value* input) {
    dyn_target_t target;
    if (!dyn_resolve(slot, &target)) return;
    const llg_vpi_shape_t* shape = target.shape;
    if (!target.packed) {
        void* storage = dyn_value_storage(target.value, shape);
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
            llg_string_t value;
            if (!dyn_string_input(input, &value)) return;
            llg_string_replace((llg_string_t*)storage, value);
        }
        /* The array the path starts in publishes the change to waiting
         * processes: nested storage has no dependencies of its own. A
         * record automatic is an activation value no process waits on. */
        if (target.top.shape->shape == LLG_VPI_SHAPE_CONTAINER) dyn_touch(&target.top);
        return;
    }
    sv4_t value;
    if (!dyn_packed_input(shape, input, &value)) return;
    dyn_packed_set(&target.level, &target.step, value);
    sv4_destroy(&value);
    if (target.top.shape->values) dyn_touch(&target.top);
}

/* The object a class handle place refers to (NULL: null handle or none). */
static void* dyn_class_target(const llg_vpi_slot_t* slot, int32_t* ok) {
    *ok = 0;
    if (slot->kind == SLOT_DYNELEM) {
        if (slot->object == DYN_DEFN_ROW) return NULL;
        dyn_target_t target;
        if (!dyn_resolve(slot, &target)) return NULL;
        if (target.shape->shape != LLG_VPI_SHAPE_CLASS) return NULL;
        void** storage = (void**)dyn_value_storage(target.value, target.shape);
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

/* The shape after `count` selections of `slot`'s path from the array
 * shape `array`, or NULL. */
static LLG_OUTLYING const llg_vpi_shape_t* dyn_path_shape(const llg_vpi_slot_t* slot,
                                             const llg_vpi_shape_t* array, uint32_t count) {
    const llg_vpi_shape_t* shape = array;
    for (uint32_t i = 0; shape && i < count; ++i) {
        dyn_step_t step = dyn_step_at(slot, i);
        if (step.at == DYN_AT_MEMBER) {
            const llg_vpi_member_t* member = dyn_member(shape, step.element);
            shape = member ? dyn_shape(member->shape) : NULL;
        } else {
            shape = shape->shape == LLG_VPI_SHAPE_CONTAINER ? dyn_shape(shape->element) : NULL;
        }
    }
    return shape;
}

/* The element shape of an element handle. */
static const llg_vpi_shape_t* dyn_element_shape(const llg_vpi_slot_t* slot) {
    if (slot->object == DYN_DEFN_ROW) {
        const llg_vpi_class_field_t* field = dyn_field((uint32_t)slot->right, (uint32_t)slot->aux);
        const llg_vpi_shape_t* array = field && field->name ? dyn_shape(field->shape) : NULL;
        return dyn_is_event_array(array) ? dyn_shape(array->element) : NULL;
    }
    dyn_place_t place;
    if (!dyn_container_place(slot->object, slot->pointer, (uint32_t)slot->aux, slot->right, &place)) return NULL;
    return dyn_path_shape(slot, place.shape, 1u + slot->span);
}

/* Whether the last selection of element slot `slot` is a record member
 * (37.24: a member variable, not an array element). */
static LLG_OUTLYING int32_t dyn_is_member(const llg_vpi_slot_t* slot) {
    /* A member of a record automatic has its member as its first step. */
    return slot->kind == SLOT_DYNELEM && (slot->span || slot->lsb == DYN_AT_MEMBER) &&
           dyn_step_at(slot, slot->span).at == DYN_AT_MEMBER;
}

/* The number of indices and keys of element slot `slot`'s path. */
static LLG_OUTLYING uint32_t dyn_index_count(const llg_vpi_slot_t* slot) {
    /* An element of an array member: the indices after that member. */
    return 1u + slot->span - dyn_tail_first(slot);
}

/* Whether element slot `slot` is an element of a fixed-size array of
 * more than one dimension. */
static LLG_OUTLYING int32_t dyn_multi_dimensional(const llg_vpi_slot_t* slot) {
    dyn_place_t place;
    return slot->object != DYN_DEFN_ROW && !dyn_is_member(slot) &&
           dyn_tail_level(slot, dyn_tail_first(slot), &place) && dyn_fixed_dim_count(&place) > 1u;
}

/* An element handle that extends `slot`'s path by `step`. */
static LLG_OUTLYING vpiHandle dyn_extend(const llg_vpi_slot_t* slot, const dyn_step_t* step) {
    dyn_place_t place;
    if (!dyn_container_place(slot->object, slot->pointer, (uint32_t)slot->aux, slot->right, &place)) {
        vpi_error("LLG_VPI_HANDLE", "the array of this element no longer exists");
        return NULL;
    }
    uint32_t count = 2u + slot->span;
    if (count > 64u) {
        vpi_error("LLG_VPI_LIMIT", "this element path is too deep");
        return NULL;
    }
    dyn_step_t steps[64];
    for (uint32_t i = 0; i + 1u < count; ++i) steps[i] = dyn_step_at(slot, i);
    steps[count - 1u] = *step;
    return make_dyn_path(&place, (uint32_t)slot->aux, steps, count);
}

/* The handle of the first `count` (at least 1) selections of `slot`'s
 * path: the array member an element of it belongs to. */
static LLG_OUTLYING vpiHandle dyn_prefix(const llg_vpi_slot_t* slot, uint32_t count) {
    dyn_place_t place;
    if (!dyn_container_place(slot->object, slot->pointer, (uint32_t)slot->aux, slot->right, &place)) {
        vpi_error("LLG_VPI_HANDLE", "the array of this element no longer exists");
        return NULL;
    }
    dyn_step_t steps[64];
    if (!count || count > 64u) return NULL;
    for (uint32_t i = 0; i < count; ++i) steps[i] = dyn_step_at(slot, i);
    return make_dyn_path(&place, (uint32_t)slot->aux, steps, count);
}

/* Whether element handle `slot` is an array member of a structure (37.24:
 * an array variable of the structure, not a further dimension). */
static LLG_OUTLYING int32_t dyn_is_member_array(const llg_vpi_slot_t* slot) {
    if (slot->kind != SLOT_DYNELEM || slot->object == DYN_DEFN_ROW || !dyn_is_member(slot)) return 0;
    const llg_vpi_shape_t* shape = dyn_element_shape(slot);
    return shape && shape->shape == LLG_VPI_SHAPE_CONTAINER && !dyn_is_event_array(shape);
}

/* The array an array member handle names, resolved at this access; 0 with
 * an error when it no longer exists. */
static LLG_OUTLYING int32_t dyn_member_array(const llg_vpi_slot_t* slot, dyn_place_t* level) {
    return dyn_tail_level(slot, 1u + slot->span, level);
}

/* The prefix of `slot` without its last selection: the structure holding
 * a member (37.17 detail 26). */
static LLG_OUTLYING vpiHandle dyn_truncate(const llg_vpi_slot_t* slot) {
    dyn_place_t place;
    if (!dyn_container_place(slot->object, slot->pointer, (uint32_t)slot->aux, slot->right, &place)) {
        vpi_error("LLG_VPI_HANDLE", "the array of this element no longer exists");
        return NULL;
    }
    dyn_step_t steps[64];
    uint32_t count = slot->span;
    /* A member of a record automatic: the automatic itself. */
    if (!count && slot->object == DYN_AUTO_ROW)
        return rts_auto_handle_by_serial((uint64_t)slot->right, (uint32_t)slot->aux);
    /* A member of a structure property: the property (A19-D89). */
    if (!count && place.owner && place.shape->shape == LLG_VPI_SHAPE_RECORD) {
        uint32_t class_index;
        if (!dyn_object_class(place.owner, &class_index)) return NULL;
        return make_property(place.owner, class_index, (uint32_t)slot->aux);
    }
    if (!count || count > 64u) return NULL;
    for (uint32_t i = 0; i < count; ++i) steps[i] = dyn_step_at(slot, i);
    return make_dyn_path(&place, (uint32_t)slot->aux, steps, count);
}

/* ── classification ─────────────────────────────────────────────────────── */

/* Whether llg_vpi.c hands `slot` to this file. */
static int32_t dyn_slot(const llg_vpi_slot_t* slot) {
    switch (slot->kind) {
        case SLOT_CLASSOBJ:
        case SLOT_BUILTINOBJ:
        case SLOT_PROPERTY:
        case SLOT_DYNELEM:
        case SLOT_CLASSDEFN:
        case SLOT_CLASSTS:
        case SLOT_EXTENDS:
        case SLOT_CLASSITEM:
            return 1;
        case SLOT_OBJECT: {
            const llg_vpi_model_object_t* object = row_object(slot->object);
            return object && (object->flags & LLG_VPI_OBJ_DYNAMIC);
        }
        default:
            return 0;
    }
}

static int32_t dyn_type(const llg_vpi_slot_t* slot) {
    switch (slot->kind) {
        case SLOT_CLASSOBJ:
        case SLOT_BUILTINOBJ: return vpiClassObj;
        case SLOT_CLASSDEFN: return vpiClassDefn;
        case SLOT_CLASSTS: return vpiClassTypespec;
        case SLOT_EXTENDS: return vpiExtends;
        case SLOT_CLASSITEM: {
            const llg_vpi_class_item_t* item = dyn_item((uint32_t)slot->right, (uint32_t)slot->aux);
            return item ? item->type : vpiUndefined;
        }
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

static const char* dyn_type_name(int32_t type) {
    switch (type) {
        case vpiClassObj: return "vpiClassObj";
        case vpiClassDefn: return "vpiClassDefn";
        case vpiClassTypespec: return "vpiClassTypespec";
        case vpiExtends: return "vpiExtends";
        case vpiClassVar: return "vpiClassVar";
        case vpiStringVar: return "vpiStringVar";
        case vpiChandleVar: return "vpiChandleVar";
        case vpiArrayVar: return "vpiArrayVar";
        case vpiConstraint: return "vpiConstraint";
        default: return type_name(type);
    }
}

/* Navigation from an object being reclaimed is refused (38.36.1: using
 * its handle "as a reference for purposes of navigation ... is undefined");
 * only the object's own properties stay readable. */
static int32_t dyn_refuse_dying(const void* owner, const char* what) {
    if (!dyn_is_dying(owner)) return 0;
    vpi_set_errorf(vpi_state(), vpiError, "LLG_VPI_RECLAIMED",
                   "%s: the class object is being reclaimed (decision A19-D22)", what);
    return 1;
}

/* ── class definitions and their items (37.27, 37.37, 37.26, 37.13) ───── */

/* The one-to-many relations from a class defn, typespec or object to the
 * items its class declares (37.27-37.29). */
static int32_t dyn_class_iteration(int32_t type) {
    return type == vpiMethods || type == vpiParameter || type == vpiInternalScope ||
           type == vpiConstraint;
}

/* A class defn, typespec or object, or a method: scopes whose items A.1.9
 * (class_item) and 13 (tf_item_declaration) give no sequence, property or
 * assertion declarations. */
static int32_t dyn_is_class_scope(const llg_vpi_slot_t* slot) {
    switch (slot->kind) {
        case SLOT_CLASSDEFN:
        case SLOT_CLASSTS:
        case SLOT_CLASSOBJ:
            return 1;
        case SLOT_CLASSITEM: {
            const llg_vpi_class_item_t* item =
                dyn_item((uint32_t)slot->right, (uint32_t)slot->aux);
            return item && dyn_item_is_method(item);
        }
        default:
            return 0;
    }
}

/* The class an item handle belongs to and the item. */
static const llg_vpi_class_item_t* class_item_of(const llg_vpi_slot_t* slot) {
    return slot->kind == SLOT_CLASSITEM ? dyn_item((uint32_t)slot->right, (uint32_t)slot->aux)
                                        : NULL;
}

/* The catalog scope that declares class `class_index`, or LLG_VPI_NONE. */
static uint32_t dyn_class_scope(uint32_t class_index) {
    const llg_vpi_class_t* layout = dyn_class(class_index);
    return layout && layout->scope ? layout->scope - 1u : LLG_VPI_NONE;
}

/* vpiInstance or vpiModule (37.10 details 2 and 3) of an object declared
 * in class `class_index`: NULL outside instances and modules. */
static vpiHandle dyn_class_container(uint32_t class_index, int32_t type) {
    uint32_t scope = dyn_class_scope(class_index);
    if (scope == LLG_VPI_NONE) return NULL;
    uint32_t row = type == vpiInstance ? enclosing_instance(scope) : enclosing_module(scope);
    return row == LLG_VPI_NONE ? NULL : object_handle(row);
}

/* Whether item `item` belongs to the one-to-many relation `relation`;
 * `owner` + 1 names the method whose arguments vpiIODecl selects. */
static int32_t dyn_item_selected(const llg_vpi_class_item_t* item, int32_t relation, uint64_t owner) {
    switch (relation) {
        /* 37.27: the methods; they are the internal scopes of the class
         * (37.12, decision A19-D36). */
        case vpiMethods:
        case vpiInternalScope: return dyn_item_is_method(item);
        case vpiParameter: return item->type == vpiParameter;
        case vpiConstraint: return item->type == vpiConstraint;
        case vpiIODecl: return item->type == vpiIODecl && item->owner == owner;
        default: return 0;
    }
}

static vpiHandle dyn_iterate_items(uint32_t class_index, int32_t relation, uint64_t owner,
                                   void* object) {
    const llg_vpi_class_t* layout = dyn_class(class_index);
    uint32_t count = 0;
    for (uint32_t k = 0; layout && k < layout->item_count; ++k)
        if (dyn_item_selected(dyn_item(class_index, k), relation, owner)) ++count;
    vpiHandle iterator = dyn_iterator(DYN_ITER_ITEMS, count);
    llg_vpi_slot_t* state = iterator ? decode_handle(iterator) : NULL;
    if (!state) return NULL;
    if (object) {
        live_get(object)->named = 1;
    }
    state->right = class_index;
    state->aux = relation;
    state->element = owner;
    state->pointer = object;
    return iterator;
}

/* Whether class `class_index` is listed by vpiClassDefn from catalog
 * scope `scope` (LLG_VPI_NONE: a NULL reference, every class). */
static int32_t dyn_class_in_scope(uint32_t class_index, uint32_t scope) {
    const llg_vpi_class_t* layout = dyn_class(class_index);
    if (layout->flags & LLG_VPI_CLASS_GENERIC) return 0;
    return scope == LLG_VPI_NONE || layout->scope == scope + 1u;
}

/* vpiClassDefn from a NULL reference or an instance (37.10): the classes
 * declared directly in that instance; from NULL, every class definition
 * (decision A19-D36). */
static vpiHandle dyn_iterate_class_defns(uint32_t scope) {
    uint32_t count = 0;
    for (uint32_t i = 0; i < dyn_class_count(); ++i)
        if (dyn_class_in_scope(i, scope)) ++count;
    vpiHandle iterator = dyn_iterator(DYN_ITER_CLASSES, count);
    llg_vpi_slot_t* state = iterator ? decode_handle(iterator) : NULL;
    if (state) state->right = scope;
    return iterator;
}

/* `<class full name>::<name>` of a method or parameter (37.10 detail 5,
 * 37.37 detail 5). */
static const char* dyn_item_full_name(uint32_t class_index, const llg_vpi_class_item_t* item) {
    return dyn_text("%s::%s", dyn_class(class_index)->full_name, item->name);
}

/* vpiIsConstraintEnabled (37.30, 18.9): a non-static constraint is active
 * per object while its hidden off bit is 0; one obtained from a class defn
 * belongs to no object and has no state: vpiUndefined without an error, as
 * another simulator answers (decision A19-D37). */
static PLI_INT64 dyn_constraint_enabled(const llg_vpi_slot_t* slot,
                                        const llg_vpi_class_item_t* item) {
    /* A static constraint shares one bit across the class; without one the
     * design never changes its mode. */
    if (item->mode_signal) return sv4_to_u64(*item->mode_signal) == 0;
    if (item->flags & LLG_VPI_ITEM_STATIC) return 1;
    if (!slot->pointer) return vpiUndefined;
    if (!item->mode) return 1;
    uint32_t kind = 0;
    const sv4_t* off = (const sv4_t*)g_dyn.tables->field(slot->pointer, item->mode - 1u, &kind);
    if (!off || kind != LLG_VPI_FIELD_PACKED) {
        vpi_error("LLG_VPI_MODEL", "the constraint_mode state disagrees with the class layout");
        return vpiUndefined;
    }
    return sv4_to_u64(*off) == 0;
}

static PLI_INT64 dyn_item_get(int32_t property, const llg_vpi_slot_t* slot, int32_t* ok) {
    const llg_vpi_class_item_t* item = class_item_of(slot);
    *ok = 1;
    if (property == vpiLineNo) return item->line;
    switch (item->type) {
        case vpiTask:
        case vpiFunction:
            switch (property) {
                case vpiMethod: return 1;
                case vpiVirtual: return (item->flags & LLG_VPI_ITEM_VIRTUAL) != 0;
                case vpiVisibility: return item->visibility;
                /* 8.6: class methods have automatic lifetime. */
                case vpiAutomatic: return 1;
                case vpiFuncType:
                    if (item->type == vpiFunction && item->detail) return item->detail;
                    break;
                case vpiSize:
                    /* 37.37 detail 12: 0 for a void function. */
                    if (item->type == vpiFunction) return item->width;
                    break;
                case vpiSigned:
                    if (item->type == vpiFunction) return (item->flags & LLG_VPI_ITEM_SIGNED) != 0;
                    break;
                default:
                    break;
            }
            break;
        case vpiIODecl:
            switch (property) {
                case vpiDirection: return item->detail;
                case vpiSize: return item->width;
                case vpiSigned: return (item->flags & LLG_VPI_ITEM_SIGNED) != 0;
                default:
                    break;
            }
            break;
        case vpiParameter:
            switch (property) {
                case vpiLocalParam: return (item->flags & LLG_VPI_ITEM_LOCAL_PARAM) != 0;
                case vpiConstType: return item->detail;
                case vpiSize:
                    if (item->value) return item->width;
                    break;
                case vpiSigned: return (item->flags & LLG_VPI_ITEM_SIGNED) != 0;
                default:
                    break;
            }
            break;
        case vpiConstraint:
            switch (property) {
                /* 37.30 detail 1: the keyword, not a lifetime. */
                case vpiAutomatic: return (item->flags & LLG_VPI_ITEM_STATIC) == 0;
                case vpiVirtual: return (item->flags & LLG_VPI_ITEM_VIRTUAL) != 0;
                /* 37.3.7: a non-static constraint of an object is part of
                 * it; a static one and one from a class defn are not. */
                case vpiAllocScheme:
                    return slot->pointer && !(item->flags & LLG_VPI_ITEM_STATIC)
                               ? vpiDynamicScheme
                               : vpiOtherScheme;
                case vpiIsConstraintEnabled: return dyn_constraint_enabled(slot, item);
                default:
                    break;
            }
            break;
        default:
            break;
    }
    *ok = 0;
    return 0;
}

static const char* dyn_item_get_str(int32_t property, const llg_vpi_slot_t* slot) {
    const llg_vpi_class_item_t* item = class_item_of(slot);
    if (property == vpiName) return item->name;
    if (property == vpiFile && item->file) return item->file;
    if (property == vpiFullName && item->type != vpiIODecl)
        return dyn_item_full_name((uint32_t)slot->right, item);
    vpi_error("LLG_VPI_PROPERTY", "unsupported VPI string property for this class item");
    return NULL;
}

static vpiHandle dyn_item_handle(int32_t type, const llg_vpi_slot_t* slot) {
    const llg_vpi_class_item_t* item = class_item_of(slot);
    uint32_t class_index = (uint32_t)slot->right;
    if (dyn_item_is_method(item)) {
        switch (type) {
            case vpiClassDefn:
            /* 37.12: a method is declared in its class defn, which is a scope
             * (decision A19-D36). */
            case vpiScope:
            case vpiParent:
                return defn_handle(class_index);
            case vpiInstance:
            case vpiModule:
                return dyn_class_container(class_index, type);
            case vpiStmt:
                /* 37.37: the statement of the method body (NULL for an
                 * empty body or a prototype, 37.12 detail 5). */
                if (!g_vpi.code) {
                    vpi_error("LLG_VPI_UNSUPPORTED",
                              "statements are objects only in models generated with --vpi or "
                              "--dpi-lib");
                    return NULL;
                }
                return item->code ? code_handle(item->code - 1u) : NULL;
            default:
                break;
        }
    } else if (item->type == vpiIODecl) {
        switch (type) {
            case vpiTaskFunc: return item_handle(class_index, item->owner - 1u);
            case vpiInstance:
            case vpiModule:
                return dyn_class_container(class_index, type);
            /* An automatic argument is not an object (decision A19-D42). */
            case vpiExpr: return NULL;
            case vpiTypespec:
                return item->typespec ? typespec_handle(typespec_at(item->typespec - 1u)) : NULL;
            default:
                break;
        }
    } else if (item->type == vpiConstraint) {
        /* 37.30: the class object or, from a class defn, the defn. */
        if (type == vpiParent) return slot->pointer ? make_object(slot->pointer) : defn_handle(class_index);
    } else if (item->type == vpiParameter) {
        switch (type) {
            case vpiTypespec:
                return item->typespec ? typespec_handle(typespec_at(item->typespec - 1u)) : NULL;
            case vpiScope:
            case vpiParent:
                return defn_handle(class_index);
            case vpiInstance:
            case vpiModule:
                return dyn_class_container(class_index, type);
            default:
                break;
        }
    }
    vpi_error("LLG_VPI_RELATION", "unsupported VPI relation for this class item");
    return NULL;
}

static vpiHandle dyn_item_iterate(int32_t type, const llg_vpi_slot_t* slot) {
    const llg_vpi_class_item_t* item = class_item_of(slot);
    if (dyn_item_is_method(item)) {
        if (type == vpiIODecl)
            return dyn_iterate_items((uint32_t)slot->right, vpiIODecl, (uint64_t)slot->aux + 1u,
                                     NULL);
        if (type == vpiImport) {
            /* 37.12 detail 4: keyed by the method's index in the item table
             * with LLG_VPI_IMPORT_CLASS set (decision A19-D85). */
            uint32_t index = dyn_class((uint32_t)slot->right)->first_item + (uint32_t)slot->aux;
            return index < LLG_VPI_IMPORT_CLASS ? iterate_scope_imports(index | LLG_VPI_IMPORT_CLASS)
                                                : NULL;
        }
        /* 37.12: a method declares only automatic variables, which llg keeps
         * no objects for (decision A19-D42). */
        if (is_scope_iteration(type)) return NULL;
    }
    if (item->type == vpiConstraint && type == vpiConstraintItem)
        /* 37.30; an empty constraint has no items. */
        return item->code ? iterate_code(item->code - 1u, vpiConstraintItem) : NULL;
    vpi_error("LLG_VPI_RELATION", "unsupported VPI iteration for this class item");
    return NULL;
}

static void dyn_item_value(const llg_vpi_slot_t* slot, p_vpi_value output) {
    const llg_vpi_class_item_t* item = class_item_of(slot);
    if (item->type != vpiParameter) {
        vpi_error("LLG_VPI_VALUE", "this VPI object has no value");
        return;
    }
    llg_vpi_datum_t datum;
    memset(&datum, 0, sizeof(datum));
    if (!item->value) {
        datum.is_real = 1;
        datum.real = item->real;
        datum.packed = VPI_EMPTY;
        fill_value(&datum, 0, vpiParameter, output);
        return;
    }
    datum.packed = digits_value(item->value, item->width, (item->flags & LLG_VPI_ITEM_SIGNED) != 0,
                                item->detail == vpiStringConst);
    fill_value(&datum, (item->flags & LLG_VPI_ITEM_SIGNED) != 0, vpiParameter, output);
    datum_destroy(&datum);
}

/* ── one-to-one relations ───────────────────────────────────────────────── */

static vpiHandle dyn_constant(int64_t value, uint32_t scope) {
    return make_constant(value, vpiDecConst, scope);
}

/* 37.3.3: where a property, a container element or an extends object is
 * in the source: a property on its declaration's line (in a class
 * definition or an object), an element on its array's (decision A19-D47),
 * an extends object on its class declaration's (decision A19-D86). */
static int32_t dyn_location(const llg_vpi_slot_t* slot, uint32_t* line, const char** file) {
    const llg_vpi_class_field_t* field = NULL;
    switch (slot->kind) {
        case SLOT_EXTENDS: {
            const llg_vpi_class_t* layout = dyn_class((uint32_t)slot->aux);
            if (!layout) return 0;
            *line = layout->line;
            *file = layout->file;
            return *line && *file;
        }
        case SLOT_PROPERTY:
            field = dyn_field((uint32_t)slot->right, (uint32_t)slot->aux);
            break;
        case SLOT_DYNELEM:
            if (slot->object == DYN_AUTO_ROW)
                return rts_auto_location((uint64_t)slot->right, (uint32_t)slot->aux, line, file);
            if (slot->object == DYN_DEFN_ROW || slot->object == DYN_STATIC_ROW) {
                field = dyn_field((uint32_t)slot->right, (uint32_t)slot->aux);
            } else if (slot->pointer) {
                uint32_t class_index;
                if (!dyn_object_class(slot->pointer, &class_index)) return 0;
                field = dyn_field(class_index, (uint32_t)slot->aux);
            } else {
                const llg_vpi_model_object_t* row = row_object(slot->object);
                if (!row || row->line <= 0) return 0;
                *line = (uint32_t)row->line;
                *file = row->file;
                return *file != NULL;
            }
            break;
        default:
            return 0;
    }
    if (!field) return 0;
    *line = field->line;
    *file = field->file;
    return *line && *file;
}

/* The located scope of the bounds and indices of a dynamic row, property
 * or container element (decision A19-D86): the scope declaring the row,
 * the class or the subprogram of an automatic. */
static uint32_t dyn_constant_scope(const llg_vpi_slot_t* slot) {
    uint32_t field = (uint32_t)slot->aux;
    if (slot->kind == SLOT_PROPERTY)
        return dyn_class_located_scope(dyn_declaring_class((uint32_t)slot->right, field));
    if (slot->kind != SLOT_DYNELEM) return constant_scope(slot);
    if (slot->object == DYN_AUTO_ROW) return rts_auto_scope((uint64_t)slot->right);
    if (slot->object == DYN_DEFN_ROW || slot->object == DYN_STATIC_ROW)
        return dyn_class_located_scope(dyn_declaring_class((uint32_t)slot->right, field));
    if (slot->pointer) {
        uint32_t class_index;
        if (!dyn_object_class(slot->pointer, &class_index)) return LLG_VPI_NONE;
        return dyn_class_located_scope(dyn_declaring_class(class_index, field));
    }
    return declaration_scope(slot->object);
}

/* vpi_handle(type, slot); `*handled` is 0 when llg_vpi.c should answer. */
static vpiHandle dyn_handle(int32_t type, llg_vpi_slot_t* slot, int32_t* handled) {
    *handled = 1;
    switch (slot->kind) {
        case SLOT_CLASSDEFN: {
            uint32_t class_index = (uint32_t)slot->aux;
            if (type == vpiClassTypespec || type == vpiTypespec)
                return class_typespec_handle(class_index);
            if (type == vpiExtends) {
                /* 37.27 detail 6: the extends object of a derived class. */
                const llg_vpi_class_t* layout = dyn_class(class_index);
                return layout && layout->base ? dyn_class_handle(class_index, 2) : NULL;
            }
            if (type == vpiScope) {
                uint32_t scope = dyn_class_scope(class_index);
                return scope == LLG_VPI_NONE ? NULL : object_handle(scope);
            }
            if (type == vpiInstance || type == vpiModule)
                return dyn_class_container(class_index, type);
            break;
        }
        case SLOT_CLASSITEM:
            if (slot->pointer && dyn_refuse_dying(slot->pointer, "vpi_handle")) return NULL;
            return dyn_item_handle(type, slot);
        case SLOT_CLASSTS: {
            uint32_t class_index = (uint32_t)slot->aux;
            if (type == vpiClassDefn) return defn_handle(class_index);
            /* A class typespec stands for its class's one specialization: it
             * is declared where its definition is (37.12). */
            if (type == vpiScope) {
                uint32_t scope = dyn_class_scope(class_index);
                return scope == LLG_VPI_NONE ? NULL : object_handle(scope);
            }
            if (type == vpiInstance || type == vpiModule)
                return dyn_class_container(class_index, type);
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
                /* 37.17 detail 26: a member's structure; an element's
                 * containing array, the outermost one for an element of a
                 * nested container (whose rows are no objects). */
                if (dyn_is_member(slot)) return dyn_truncate(slot);
                /* An element of an array member of a structure: that
                 * member (37.24). */
                uint32_t first = dyn_tail_first(slot);
                if (first) return dyn_prefix(slot, first);
                if (slot->object == DYN_AUTO_ROW)
                    return rts_auto_handle_by_serial((uint64_t)slot->right, (uint32_t)slot->aux);
                if (slot->object == DYN_DEFN_ROW)
                    return make_property(NULL, (uint32_t)slot->right, (uint32_t)slot->aux);
                if (slot->object == DYN_STATIC_ROW)
                    return make_property(NULL, dyn_declaring_class(0u, (uint32_t)slot->aux),
                                         (uint32_t)slot->aux);
                if (!slot->pointer) return object_handle(slot->object);
                uint32_t class_index;
                if (!dyn_object_class(slot->pointer, &class_index)) break;
                return make_property(slot->pointer, class_index, (uint32_t)slot->aux);
            }
            if (slot->object == DYN_AUTO_ROW && type == vpiScope) {
                /* An automatic's element: the subprogram of its frame. */
                vpiHandle array = rts_auto_handle_by_serial((uint64_t)slot->right, (uint32_t)slot->aux);
                if (!array) return NULL;
                vpiHandle result = vpi_handle(type, array);
                vpi_release_handle(array);
                return result;
            }
            if (!slot->pointer && slot->object != DYN_AUTO_ROW && slot->object != DYN_STATIC_ROW &&
                (type == vpiModule || type == vpiInstance || type == vpiScope)) {
                /* 37.17: an element of a module-level array is declared
                 * where the array is. */
                vpiHandle array = object_handle(slot->object);
                if (!array) return NULL;
                vpiHandle result = vpi_handle(type, array);
                vpi_release_handle(array);
                return result;
            }
            if (type == vpiIndex) {
                uint32_t scope = dyn_constant_scope(slot);
                if (dyn_is_member(slot)) {
                    vpi_error("LLG_VPI_RELATION", "a structure member is not an array element");
                    return NULL;
                }
                if (dyn_index_count(slot) > 1u || dyn_multi_dimensional(slot)) {
                    /* 37.17 detail 5: the indices of an element of a
                     * multidimensional array are an iteration. */
                    vpi_error("LLG_VPI_RELATION",
                              "an element of a multidimensional array has several indices; "
                              "iterate vpiIndex (37.17 detail 5)");
                    return NULL;
                }
                /* The index in the element's own array: the one the path
                 * starts in, or an array member of a structure. */
                uint32_t first = dyn_tail_first(slot);
                dyn_step_t step = dyn_step_at(slot, first);
                /* A string key is a string constant (decision A19-D23). */
                if (step.at == DYN_AT_STRING) return make_string_constant(step.key, step.length, scope);
                dyn_place_t place;
                if (!dyn_tail_level(slot, first, &place)) return NULL;
                if (step.at == DYN_AT_INTEGRAL) {
                    sv4_t key = dyn_integral_key(&place, step.element);
                    int64_t value = sv4_to_i64(key);
                    if (!llg_sv4_signed(key) && llg_sv4_width(key) < 64u)
                        value = (int64_t)sv4_to_u64(key);
                    sv4_destroy(&key);
                    return dyn_constant(value, scope);
                }
                return dyn_constant(dyn_declared_index(&place, step.element), scope);
            }
            if ((type == vpiLeftRange || type == vpiRightRange) && dyn_is_member_array(slot)) {
                dyn_place_t level;
                if (!dyn_member_array(slot, &level)) return NULL;
                if (level.shape->array_type == vpiAssocArray) break;
                int64_t left, right;
                uint32_t scope = dyn_constant_scope(slot);
                if (dyn_fixed_range(&level, &left, &right))
                    return dyn_constant(type == vpiLeftRange ? left : right, scope);
                if (dyn_fixed_dim_count(&level)) break;
                /* 37.17 detail 6: [0:size-1]; NULL with no element. */
                size_t size = dyn_container_size(&level);
                if (!size) return NULL;
                return dyn_constant(type == vpiLeftRange ? 0 : (int64_t)size - 1, scope);
            }
            if (type == vpiClassObj) {
                int32_t ok;
                void* object = dyn_class_target(slot, &ok);
                if (!ok) break;
                return object ? make_shape_object(dyn_element_shape(slot), object) : NULL;
            }
            if (type == vpiClassTypespec || type == vpiTypespec) {
                const llg_vpi_shape_t* shape = dyn_element_shape(slot);
                if (!shape) break;
                if (shape->shape == LLG_VPI_SHAPE_CLASS)
                    return shape->class_index ? class_typespec_handle(shape->class_index - 1u)
                                              : NULL;
                if (type == vpiTypespec && shape->typespec)
                    return typespec_handle(typespec_ref(shape->typespec));
                if (type == vpiTypespec && !slot->pointer && !slot->span &&
                    slot->object != DYN_STATIC_ROW) {
                    /* The element type of the array's typespec (37.17). */
                    vpiHandle array = object_handle(slot->object);
                    vpiHandle array_ts = array ? vpi_handle(vpiTypespec, array) : NULL;
                    vpiHandle result = array_ts ? vpi_handle(vpiElemTypespec, array_ts) : NULL;
                    if (array_ts) vpi_release_handle(array_ts);
                    if (array) vpi_release_handle(array);
                    if (result) return result;
                }
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
            if (type == vpiActual && shape->shape == LLG_VPI_SHAPE_VIF) {
                /* 37.15: the interface instance a virtual interface property
                 * names; NULL while it names none. */
                if (!place.storage) {
                    vpi_error("LLG_VPI_RELATION",
                              "a property of a class definition names no interface (37.27 detail 2)");
                    return NULL;
                }
                if (dyn_refuse_dying(place.owner, "vpiActual")) return NULL;
                const void* env = *(void* const*)place.storage;
                if (!env) return NULL;
                for (uint32_t i = 0; i < g_dyn.tables->vif_count; ++i)
                    if (g_dyn.tables->vifs[i].env == env) return object_handle(g_dyn.tables->vifs[i].row);
                vpi_error("LLG_VPI_RELATION", "the interface instance this virtual interface names has no object");
                return NULL;
            }
            if (type == vpiClassObj && shape->shape == LLG_VPI_SHAPE_CLASS) {
                if (!place.storage) {
                    vpi_error("LLG_VPI_RELATION",
                              "a property of a class definition refers to no object (37.27 detail 2)");
                    return NULL;
                }
                if (dyn_refuse_dying(place.owner, "vpiClassObj")) return NULL;
                void* object = *(void**)place.storage;
                /* 37.29 detail 5: NULL for a null class variable. */
                return object ? make_shape_object(shape, object) : NULL;
            }
            if ((type == vpiClassTypespec || type == vpiTypespec) &&
                shape->shape == LLG_VPI_SHAPE_CLASS)
                return shape->class_index ? class_typespec_handle(shape->class_index - 1u) : NULL;
            if (type == vpiTypespec && shape->shape == LLG_VPI_SHAPE_PACKED && shape->typespec)
                return typespec_handle(typespec_ref(shape->typespec));
            /* A chandle property's typespec (37.23); a catalog row's comes
             * from its row, which names its declaration. */
            if (type == vpiTypespec && slot->kind == SLOT_PROPERTY &&
                shape->shape == LLG_VPI_SHAPE_CHANDLE && shape->typespec)
                return typespec_handle(typespec_ref(shape->typespec));
            if (slot->kind == SLOT_PROPERTY && type == vpiParent &&
                shape->shape != LLG_VPI_SHAPE_VIF) {
                /* 37.17 detail 26: a property reached by iteration has no
                 * parent. */
                return NULL;
            }
            if ((type == vpiLeftRange || type == vpiRightRange) &&
                shape->shape == LLG_VPI_SHAPE_CONTAINER && place.storage &&
                shape->array_type != vpiAssocArray && !dyn_is_event_array(shape)) {
                int64_t left, right;
                uint32_t scope = dyn_constant_scope(slot);
                if (dyn_fixed_range(&place, &left, &right))
                    return dyn_constant(type == vpiLeftRange ? left : right, scope);
                /* 37.17 detail 6: [0:size-1]; NULL with no element. */
                size_t size = dyn_container_size(&place);
                if (!size) return NULL;
                return dyn_constant(type == vpiLeftRange ? 0 : (int64_t)size - 1, scope);
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

/* Whether property `field` is listed by an iteration for objects of
 * `type` (0: vpiVariables, every variable). A property without a variable
 * form (a virtual interface) is not a variable. */
static int32_t dyn_field_listed(uint32_t class_index, uint32_t field, int32_t type) {
    const llg_vpi_class_field_t* entry = dyn_field(class_index, field);
    if (!entry || !entry->name) return 0;
    const llg_vpi_shape_t* shape = dyn_shape(entry->shape);
    if (!shape || shape->shape == LLG_VPI_SHAPE_OTHER) return 0;
    /* An event property is a named event, not a variable (37.25, 37.29). */
    if (shape->shape == LLG_VPI_SHAPE_EVENT) return type == vpiNamedEvent;
    if (dyn_is_event_array(shape)) return type == vpiNamedEventArray;
    /* A virtual interface property is an interface declaration (37.27
     * detail 7). */
    if (shape->shape == LLG_VPI_SHAPE_VIF) return type == vpiInterfaceDecl;
    return !type || shape->type == type;
}

/* The properties an iteration lists: the instance properties from field
 * `first` on, then the static properties of the class (with `inherited`,
 * those of its base classes first, as an object lists them). */
static uint32_t dyn_visible_fields(uint32_t class_index, uint32_t first, int32_t type, int32_t inherited) {
    const llg_vpi_class_t* layout = dyn_class(class_index);
    uint32_t count = 0;
    for (uint32_t i = first; layout && i < layout->field_count; ++i)
        if (dyn_field_listed(class_index, i, type)) ++count;
    for (uint32_t k = 0;; ++k) {
        uint32_t field = dyn_class_static(class_index, inherited, k);
        if (field == LLG_VPI_NONE) break;
        if (dyn_field_listed(class_index, field, type)) ++count;
    }
    return count;
}

static vpiHandle dyn_iterator(uint32_t sub_kind, uint32_t count) {
    if (count == 0) return NULL;
    vpiHandle handle;
    llg_vpi_slot_t* slot = alloc_slot(SLOT_ITERATOR, &handle);
    if (!slot) return NULL;
    slot->iter_kind = ITER_DYN;
    slot->span = sub_kind;
    slot->count = count;
    return handle;
}

static vpiHandle dyn_iterate_fields(void* object, uint32_t class_index, uint32_t first,
                                    int32_t type) {
    /* An object lists the static properties of its whole class chain, a
     * definition those it declares (37.27 detail 4, 37.29 detail 6). */
    int32_t inherited = first == 0;
    vpiHandle iterator =
        dyn_iterator(DYN_ITER_FIELDS, dyn_visible_fields(class_index, first, type, inherited));
    llg_vpi_slot_t* slot = iterator ? decode_handle(iterator) : NULL;
    if (!slot) return NULL;
    slot->lsb = (uint32_t)inherited;
    if (object) {
        live_get(object)->named = 1;
    }
    slot->pointer = object;
    slot->right = class_index;
    slot->left = first;
    slot->aux = type;
    return iterator;
}

/* vpiRange of a named event array property (field `field` of layout
 * `class_index`): its one declared range, one object per declaration
 * whether reached from a class definition or an object, located at the
 * property (decision A19-D86). */
static vpiHandle dyn_event_array_ranges(const llg_vpi_shape_t* shape, uint32_t class_index,
                                        uint32_t field) {
    uintptr_t* items = (uintptr_t*)vpi_calloc(1u, sizeof(*items));
    uint32_t declaring = dyn_declaring_class(class_index, field);
    const llg_vpi_class_field_t* entry = dyn_field(class_index, field);
    llg_vpi_range_origin_t origin = {
        RANGE_PROPERTY, (uint64_t)declaring << 32 | field, 0u, LLG_VPI_NONE,
        dyn_class_located_scope(declaring), entry ? entry->line : 0u, entry ? entry->file : NULL };
    vpiHandle range = make_range(&origin, 0u, shape->left, shape->right);
    if (!range) {
        free(items);
        return NULL;
    }
    items[0] = (uintptr_t)range;
    return make_list_iterator(vpiRange, items, 1u);
}

/* 37.17 details 4 and 7: vpiReg and vpiVarSelect iterate the elements of
 * an array variable, whatever their type (decision A19-D83). */
static int32_t dyn_element_type_matches(const llg_vpi_shape_t* element, int32_t type) {
    return type == vpiReg || type == vpiVarSelect || type == vpiMemoryWord || type == element->type;
}

/* The shape of the innermost elements of an array of shape `array`: the
 * element shape, through every container nested in it. */
static LLG_OUTLYING const llg_vpi_shape_t* dyn_leaf_shape(const llg_vpi_shape_t* array) {
    const llg_vpi_shape_t* shape = array ? dyn_shape(array->element) : NULL;
    for (uint32_t depth = 0; shape && shape->shape == LLG_VPI_SHAPE_CONTAINER && depth < 64u; ++depth)
        shape = dyn_shape(shape->element);
    return shape;
}

/* A growing handle list (list iterators). */
typedef struct {
    uintptr_t* items;
    uint32_t count;
    uint32_t capacity;
    int32_t failed;
} dyn_list_t;

static LLG_OUTLYING void dyn_list_push(dyn_list_t* list, vpiHandle handle) {
    if (!handle) {
        list->failed = 1;
        return;
    }
    if (list->failed) {
        vpi_release_handle(handle);
        return;
    }
    if (list->count == list->capacity) {
        uint32_t capacity = list->capacity ? list->capacity * 2u : 8u;
        list->items =
            (uintptr_t*)vpi_realloc(list->items, (size_t)capacity * sizeof(*list->items));
        list->capacity = capacity;
    }
    list->items[list->count++] = (uintptr_t)handle;
}

/* The iterator of a finished list, or NULL (an empty list, or a failure
 * whose handles are released). */
static LLG_OUTLYING vpiHandle dyn_list_iterator(int32_t type, dyn_list_t* list) {
    if (list->failed || !list->count) {
        for (uint32_t i = 0; i < list->count; ++i) vpi_release_handle((vpiHandle)list->items[i]);
        free(list->items);
        return NULL;
    }
    return make_list_iterator(type, list->items, list->count);
}

static LLG_OUTLYING vpiHandle dyn_record_members(const dyn_place_t* place, uint32_t field);
static LLG_OUTLYING void dyn_collect_leaves(dyn_list_t* list, const dyn_place_t* top, uint32_t field,
                                            const dyn_place_t* level, dyn_step_t* steps,
                                            uint32_t depth);

/* vpiRange of a fixed-size array whose shape carries its ranges (37.17
 * detail 4), leftmost first, from `origin`. */
static LLG_OUTLYING vpiHandle dyn_shape_ranges(const llg_vpi_shape_t* shape,
                                               const llg_vpi_range_origin_t* origin) {
    dyn_list_t list = {0};
    for (uint32_t d = 0; d < shape->dim_count; ++d)
        dyn_list_push(&list, make_range(origin, d, shape->dims[2u * d], shape->dims[2u * d + 1u]));
    return dyn_list_iterator(vpiRange, &list);
}

/* vpiRange, vpiReg and vpiVarSelect of an array member of a structure
 * (37.17 details 4 and 7, 37.24): its declared ranges, or its innermost
 * elements, each named by its path. The ranges take the source location of
 * the declaration holding the structure (37.3.3), which owns no separate
 * range object (decision A19-D86). */
static LLG_OUTLYING vpiHandle dyn_iterate_member_array(const llg_vpi_slot_t* slot, int32_t type) {
    const llg_vpi_shape_t* shape = dyn_element_shape(slot);
    if (type == vpiRange) {
        if (!shape->dim_count) {
            vpi_error("LLG_VPI_RELATION", "this array has no declared range");
            return NULL;
        }
        uint32_t line = 0;
        const char* file = NULL;
        if (!dyn_location(slot, &line, &file)) {
            line = 0;
            file = NULL;
        }
        llg_vpi_range_origin_t origin = {
            RANGE_MEMBER, 0u, 0u, LLG_VPI_NONE, dyn_constant_scope(slot), line, file };
        return dyn_shape_ranges(shape, &origin);
    }
    const llg_vpi_shape_t* element = dyn_leaf_shape(shape);
    if (!element || !dyn_element_type_matches(element, type)) {
        vpi_error("LLG_VPI_RELATION", "unsupported VPI iteration for this dynamic object");
        return NULL;
    }
    if (shape->key == LLG_VPI_KEY_OTHER) {
        vpi_error("LLG_VPI_UNSUPPORTED",
                  "llg does not iterate associative arrays with a wildcard index");
        return NULL;
    }
    dyn_place_t top, level;
    if (!dyn_member_array(slot, &level) ||
        !dyn_container_place(slot->object, slot->pointer, (uint32_t)slot->aux, slot->right, &top))
        return NULL;
    uint32_t count = 1u + slot->span;
    if (count >= 64u) {
        vpi_error("LLG_VPI_LIMIT", "this element path is too deep");
        return NULL;
    }
    dyn_step_t steps[64];
    for (uint32_t i = 0; i < count; ++i) steps[i] = dyn_step_at(slot, i);
    dyn_list_t list = {0};
    dyn_collect_leaves(&list, &top, (uint32_t)slot->aux, &level, steps, count);
    return dyn_list_iterator(type, &list);
}

/* vpiRange of a fixed-size array static property (37.17 detail 4): its
 * declared ranges, leftmost first, located at the property (decision
 * A19-D86). */
static LLG_OUTLYING vpiHandle dyn_static_ranges(uint32_t field) {
    const llg_vpi_class_static_t* entry = dyn_static(field);
    if (!entry || !entry->dim_count) {
        vpi_error("LLG_VPI_RELATION", "this array has no declared range");
        return NULL;
    }
    llg_vpi_range_origin_t origin = {
        RANGE_PROPERTY, (uint64_t)entry->class_index << 32 | field, 0u, LLG_VPI_NONE,
        dyn_class_located_scope(entry->class_index), entry->field.line, entry->field.file };
    dyn_list_t list = {0};
    for (uint32_t d = 0; d < entry->dim_count; ++d)
        dyn_list_push(&list, make_range(&origin, d, entry->dims[2u * d], entry->dims[2u * d + 1u]));
    return dyn_list_iterator(vpiRange, &list);
}

/* Append the innermost elements of `level` (a container at depth
 * `depth` of the path `steps`), in index or key order. */
static LLG_OUTLYING void dyn_collect_leaves(dyn_list_t* list, const dyn_place_t* top, uint32_t field,
                               const dyn_place_t* level, dyn_step_t* steps, uint32_t depth) {
    if (depth >= 64u) {
        list->failed = 1;
        vpi_error("LLG_VPI_LIMIT", "this array nests too deeply for VPI");
        return;
    }
    const llg_vpi_shape_t* element = dyn_shape(level->shape->element);
    size_t size = dyn_container_size(level);
    for (size_t position = 0; position < size && !list->failed; ++position) {
        dyn_step_t* step = &steps[depth];
        if (level->shape->array_type == vpiAssocArray) {
            if (!dyn_assoc_step(level, position, step)) {
                list->failed = 1;
                return;
            }
        } else {
            memset(step, 0, sizeof(*step));
            step->at = DYN_AT_INDEX;
            step->element = position;
        }
        if (element->shape != LLG_VPI_SHAPE_CONTAINER) {
            dyn_list_push(list, make_dyn_path(top, field, steps, depth + 1u));
            continue;
        }
        int32_t exists;
        llg_value_t* value = dyn_locate_step(level, step, &exists);
        dyn_place_t inner;
        if (!exists || !dyn_nested_level(value, element, &inner)) {
            list->failed = 1;
            vpi_error("LLG_VPI_MODEL", "a nested array disagrees with its layout");
            return;
        }
        dyn_collect_leaves(list, top, field, &inner, steps, depth + 1u);
    }
}

/* vpiReg / vpiVarSelect of an array whose elements hold containers: the
 * innermost elements, each named by all its indices (`qq[0][1]`), in
 * index and key order; an element whose nested container is empty adds
 * none. Listed at once: a nested walk has no position to resume from. */
static LLG_OUTLYING vpiHandle dyn_iterate_leaves(const dyn_place_t* place, uint32_t field, int32_t type) {
    dyn_list_t list = {0};
    dyn_step_t steps[64];
    dyn_collect_leaves(&list, place, field, place, steps, 0);
    return dyn_list_iterator(type, &list);
}

/* The number of innermost elements of an array whose elements may hold
 * containers (37.17 detail 9: the variables in the array). */
static LLG_OUTLYING size_t dyn_leaf_count(const dyn_place_t* level, uint32_t depth) {
    const llg_vpi_shape_t* element = dyn_shape(level->shape->element);
    size_t size = dyn_container_size(level);
    if (!element || element->shape != LLG_VPI_SHAPE_CONTAINER || depth >= 64u) return size;
    size_t total = 0;
    for (size_t position = 0; position < size; ++position) {
        dyn_step_t step;
        if (level->shape->array_type == vpiAssocArray) {
            if (!dyn_assoc_step(level, position, &step)) continue;
        } else {
            memset(&step, 0, sizeof(step));
            step.at = DYN_AT_INDEX;
            step.element = position;
        }
        int32_t exists;
        llg_value_t* value = dyn_locate_step(level, &step, &exists);
        dyn_place_t inner;
        if (exists && dyn_nested_level(value, element, &inner)) total += dyn_leaf_count(&inner, depth + 1u);
    }
    return total;
}

/* vpiMember of a structure element or member: its members in declaration
 * order (37.24). */
static LLG_OUTLYING vpiHandle dyn_iterate_members(const llg_vpi_slot_t* slot, const llg_vpi_shape_t* shape) {
    dyn_list_t list = {0};
    for (uint32_t m = 0; m < shape->member_count && !list.failed; ++m) {
        dyn_step_t step;
        memset(&step, 0, sizeof(step));
        step.at = DYN_AT_MEMBER;
        step.element = m;
        dyn_list_push(&list, dyn_extend(slot, &step));
    }
    return dyn_list_iterator(vpiMember, &list);
}

/* vpiIndex of an element of a multidimensional or nested array (37.17
 * details 5 and 18): one constant per index, innermost first; a string key
 * is a string constant (decision A19-D23). */
static LLG_OUTLYING vpiHandle dyn_iterate_indices(const llg_vpi_slot_t* slot) {
    dyn_target_t target;
    if (!dyn_resolve(slot, &target)) return NULL;
    uint32_t scope = dyn_constant_scope(slot);
    dyn_list_t list = {0};
    /* The indices in the element's own array: after the last member
     * selection, if any (37.24). */
    uint32_t first = dyn_tail_first(slot);
    dyn_place_t level;
    if (!dyn_tail_level(slot, first, &level)) return NULL;
    const llg_vpi_shape_t* shape = NULL;
    llg_value_t* value = NULL;
    for (uint32_t i = first; i <= slot->span && !list.failed; ++i) {
        dyn_step_t step = dyn_step_at(slot, i);
        if (step.at == DYN_AT_MEMBER) break;
        if (i > first && !dyn_nested_level(value, shape, &level)) break;
        if (step.at == DYN_AT_STRING) {
            dyn_list_push(&list, make_string_constant(step.key, step.length, scope));
        } else if (step.at == DYN_AT_INTEGRAL) {
            sv4_t key = dyn_integral_key(&level, step.element);
            int64_t number = sv4_to_i64(key);
            if (!llg_sv4_signed(key) && llg_sv4_width(key) < 64u) number = (int64_t)sv4_to_u64(key);
            sv4_destroy(&key);
            dyn_list_push(&list, dyn_constant(number, scope));
        } else {
            int64_t indices[DYN_MAX_DIMS];
            uint32_t count = dyn_declared_indices(&level, step.element, indices);
            for (uint32_t d = 0; d < count; ++d) dyn_list_push(&list, dyn_constant(indices[d], scope));
        }
        shape = dyn_shape(level.shape->element);
        int32_t exists;
        value = dyn_locate_step(&level, &step, &exists);
    }
    /* 37.17 detail 5: the element's own index first, then outward, as a
     * static array element gives them (decision A19-D28). */
    for (uint32_t i = 0; !list.failed && i < list.count / 2u; ++i) {
        uintptr_t swapped = list.items[i];
        list.items[i] = list.items[list.count - 1u - i];
        list.items[list.count - 1u - i] = swapped;
    }
    return dyn_list_iterator(vpiIndex, &list);
}

static vpiHandle dyn_iterate(int32_t type, llg_vpi_slot_t* slot, int32_t* handled) {
    *handled = 1;
    /* 37.12 detail 4: a class declares no import (A.1.9 class_item), so its
     * definition, typespec and objects import nothing. */
    if (type == vpiImport &&
        (slot->kind == SLOT_CLASSOBJ || slot->kind == SLOT_CLASSDEFN || slot->kind == SLOT_CLASSTS))
        return NULL;
    switch (slot->kind) {
        case SLOT_CLASSOBJ: {
            uint32_t class_index;
            if (!dyn_object_class(slot->pointer, &class_index)) break;
            if (type == vpiVariables || type == vpiReg || type == vpiRegArray)
                /* 37.29 detail 6: every property, inherited ones first. */
                return dyn_iterate_fields(slot->pointer, class_index, 0,
                                          type == vpiVariables ? 0 : type);
            if (dyn_class_iteration(type)) {
                if (dyn_refuse_dying(slot->pointer, "vpi_iterate")) return NULL;
                /* 37.29 details 6 and 8: the items of the object's class; its
                 * constraints are the object's (37.30). */
                return dyn_iterate_items(class_index, type, 0,
                                         type == vpiConstraint ? slot->pointer : NULL);
            }
            /* 37.29 details 6 and 7: the object's event, event array and
             * virtual interface properties, inherited ones first. */
            if (type == vpiNamedEvent || type == vpiNamedEventArray || type == vpiInterfaceDecl)
                return dyn_iterate_fields(slot->pointer, class_index, 0, type);
            break;
        }
        case SLOT_CLASSITEM:
            if (slot->pointer && dyn_refuse_dying(slot->pointer, "vpi_iterate")) return NULL;
            return dyn_item_iterate(type, slot);
        case SLOT_CLASSDEFN:
        case SLOT_CLASSTS: {
            uint32_t class_index = (uint32_t)slot->aux;
            if (type == vpiVariables || type == vpiReg || type == vpiRegArray)
                /* The properties the class declares (37.27 detail 4: base
                 * classes through vpiExtends); they have no value. */
                return dyn_iterate_fields(NULL, class_index, dyn_inherited_fields(class_index),
                                          type == vpiVariables ? 0 : type);
            /* 37.27, 37.28 detail 5: the items the class declares. */
            if (dyn_class_iteration(type)) return dyn_iterate_items(class_index, type, 0, NULL);
            /* 37.27, 37.28: the event and event array properties the class
             * declares; they name no synchronization object. */
            if (type == vpiNamedEvent || (type == vpiNamedEventArray && slot->kind == SLOT_CLASSDEFN) ||
                (type == vpiInterfaceDecl && slot->kind == SLOT_CLASSDEFN))
                return dyn_iterate_fields(NULL, class_index, dyn_inherited_fields(class_index), type);
            if (type == vpiDerivedClasses && slot->kind == SLOT_CLASSDEFN) {
                uint32_t count = 0;
                for (uint32_t i = 0; i < dyn_class_count(); ++i)
                    if (dyn_class(i)->base == class_index + 1u) ++count;
                vpiHandle iterator = dyn_iterator(DYN_ITER_DERIVED, count);
                llg_vpi_slot_t* state = iterator ? decode_handle(iterator) : NULL;
                if (state) state->right = class_index;
                return iterator;
            }
            break;
        }
        case SLOT_DYNELEM:
        case SLOT_PROPERTY:
        case SLOT_OBJECT: {
            dyn_place_t place;
            if (slot->kind == SLOT_DYNELEM && type == vpiIndex) {
                /* 37.17 details 5 and 18: the indices that select the
                 * element, leftmost first. */
                if (dyn_index_count(slot) > 1u || dyn_multi_dimensional(slot))
                    return dyn_iterate_indices(slot);
                int32_t index_handled;
                vpiHandle index = dyn_handle(vpiIndex, slot, &index_handled);
                if (!index) return NULL;
                uintptr_t* items = (uintptr_t*)vpi_calloc(1u, sizeof(*items));
                items[0] = (uintptr_t)index;
                return make_list_iterator(vpiIndex, items, 1u);
            }
            if (dyn_is_member_array(slot)) {
                if (dyn_refuse_dying(slot->pointer, "vpi_iterate")) return NULL;
                return dyn_iterate_member_array(slot, type);
            }
            if (slot->kind == SLOT_DYNELEM && type == vpiMember) {
                /* 37.24: the members of an unpacked structure element. */
                if (dyn_refuse_dying(slot->pointer, "vpi_iterate")) return NULL;
                const llg_vpi_shape_t* shape = dyn_element_shape(slot);
                if (shape && shape->shape == LLG_VPI_SHAPE_RECORD) return dyn_iterate_members(slot, shape);
            }
            if (slot->kind == SLOT_DYNELEM || !dyn_place(slot, &place)) {
                if (slot->kind == SLOT_OBJECT) {
                    *handled = 0;
                    return NULL;
                }
                break;
            }
            if (dyn_refuse_dying(place.owner, "vpi_iterate")) return NULL;
            /* 37.25 detail 1 (named event array): an event property is not
             * part of an array, so it has no indices. */
            if (type == vpiIndex && place.shape->shape == LLG_VPI_SHAPE_EVENT) return NULL;
            if (dyn_is_event_array(place.shape)) {
                /* 37.25 detail 2 (decision A19-D69): the range and the
                 * events of a named event array. */
                if (type == vpiRange)
                    return dyn_event_array_ranges(place.shape, (uint32_t)slot->right,
                                                  (uint32_t)slot->aux);
                if (type != vpiNamedEvent) {
                    vpi_error("LLG_VPI_RELATION",
                              "a named event array iterates vpiRange and vpiNamedEvent");
                    return NULL;
                }
            }
            if (place.shape->shape == LLG_VPI_SHAPE_RECORD && slot->kind == SLOT_PROPERTY &&
                type == vpiMember) {
                /* 37.24: the members of a structure property (A19-D89). */
                if (!place.storage) {
                    vpi_error("LLG_VPI_RELATION",
                              "a property of a class definition has no members (37.27 detail 2)");
                    return NULL;
                }
                return dyn_record_members(&place, (uint32_t)slot->aux);
            }
            if (place.shape->shape != LLG_VPI_SHAPE_CONTAINER) {
                if (slot->kind == SLOT_OBJECT) {
                    *handled = 0;
                    return NULL;
                }
                break;
            }
            /* A fixed-size array's ranges are its row's (37.17 detail 6). */
            if (dyn_fixed_dim_count(&place) && slot->kind == SLOT_OBJECT && type == vpiRange) {
                *handled = 0;
                return NULL;
            }
            if (place.row == DYN_STATIC_ROW && type == vpiRange) return dyn_static_ranges((uint32_t)slot->aux);
            if (type == vpiRange && place.shape->dim_count && slot->kind == SLOT_PROPERTY) {
                /* A fixed-size class instance property: the ranges its
                 * shape carries, located at the property (A19-D86). */
                uint32_t declaring = dyn_declaring_class((uint32_t)slot->right, (uint32_t)slot->aux);
                const llg_vpi_class_field_t* entry = dyn_field((uint32_t)slot->right, (uint32_t)slot->aux);
                llg_vpi_range_origin_t origin = {
                    RANGE_PROPERTY, (uint64_t)declaring << 32 | (uint32_t)slot->aux, 0u, LLG_VPI_NONE,
                    dyn_class_located_scope(declaring), entry->line, entry->file };
                return dyn_shape_ranges(place.shape, &origin);
            }
            const llg_vpi_shape_t* element = dyn_leaf_shape(place.shape);
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
            uint32_t field = slot->kind == SLOT_PROPERTY ? (uint32_t)slot->aux : 0u;
            /* 37.17 detail 4: a container nested in the elements is a
             * further dimension; the elements are the innermost ones. */
            if (dyn_shape(place.shape->element)->shape == LLG_VPI_SHAPE_CONTAINER)
                return dyn_iterate_leaves(&place, field, type);
            size_t size = dyn_container_size(&place);
            if (size > UINT32_MAX) size = UINT32_MAX;
            vpiHandle iterator = dyn_iterator(DYN_ITER_ELEMENTS, (uint32_t)size);
            llg_vpi_slot_t* state = iterator ? decode_handle(iterator) : NULL;
            if (!state) return NULL;
            if (place.owner) {
                live_get(place.owner)->named = 1;
            }
            state->object = place.row;
            state->right = place.serial;
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
                if (dyn_field_listed(class_index, field, slot->aux))
                    return make_property(slot->pointer, class_index, field);
            }
            /* Then the static properties (dyn_visible_fields). */
            while (layout) {
                uint32_t field = dyn_class_static(class_index, (int32_t)slot->lsb,
                                                  (uint32_t)(slot->left - (int64_t)layout->field_count));
                if (field == LLG_VPI_NONE) break;
                ++slot->left;
                if (dyn_field_listed(class_index, field, slot->aux)) return make_static_property(field);
            }
            return NULL;
        }
        case DYN_ITER_ITEMS: {
            uint32_t class_index = (uint32_t)slot->right;
            const llg_vpi_class_t* layout = dyn_class(class_index);
            while (layout && slot->left < (int64_t)layout->item_count) {
                uint32_t item = (uint32_t)slot->left++;
                const llg_vpi_class_item_t* found = dyn_item(class_index, item);
                if (!dyn_item_selected(found, slot->aux, slot->element)) continue;
                if (slot->pointer && found->type == vpiConstraint)
                    return object_item_handle(slot->pointer, class_index, item);
                return item_handle(class_index, item);
            }
            return NULL;
        }
        case DYN_ITER_CLASSES: {
            while (slot->left < (int64_t)dyn_class_count()) {
                uint32_t candidate = (uint32_t)slot->left++;
                if (dyn_class_in_scope(candidate, (uint32_t)slot->right))
                    return defn_handle(candidate);
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
            if (!dyn_container_place(slot->object, slot->pointer, field, slot->right, &place)) return NULL;
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

static PLI_INT64 dyn_size(const dyn_place_t* place, int32_t* ok) {
    *ok = 1;
    const llg_vpi_shape_t* shape = place->shape;
    if (shape->shape == LLG_VPI_SHAPE_STRING && place->storage)
        /* 37.17 detail 9: the characters the string holds. */
        return (PLI_INT64)((const llg_string_t*)place->storage)->len;
    /* 37.17 detail 9: the variables in the array, the innermost elements
     * of nested containers. */
    if (shape->shape == LLG_VPI_SHAPE_CONTAINER && place->storage)
        return (PLI_INT64)dyn_leaf_count(place, 0);
    if (shape->shape == LLG_VPI_SHAPE_PACKED) return shape->width;
    /* A chandle reads as a 64-bit image (decision A19-D21). */
    if (shape->shape == LLG_VPI_SHAPE_CHANDLE) return 64;
    /* 37.17 detail 9: the number of fields of an unpacked structure. */
    if (shape->shape == LLG_VPI_SHAPE_RECORD) return shape->member_count;
    *ok = 0;
    return 0;
}

/* vpiRandType, vpiVisibility and vpiIsRandomized (37.17 details 15, 16
 * and 24) of a dynamic variable or a class property. A variable that is
 * not a class member is public and not random; a property of a class
 * definition belongs to no object, so it is not active for randomization
 * (decision A19-D36). */
static PLI_INT64 dyn_random_property(int32_t property, const llg_vpi_slot_t* slot) {
    const llg_vpi_class_field_t* field =
        slot->kind == SLOT_PROPERTY ? dyn_field((uint32_t)slot->right, (uint32_t)slot->aux) : NULL;
    if (property == vpiRandType) return field ? field->rand : vpiNotRand;
    if (property == vpiVisibility) return field ? field->visibility : vpiPublicVis;
    if (!field || field->rand == vpiNotRand || !slot->pointer) return 0;
    if (!field->mode) return 1;
    uint32_t kind = 0;
    const sv4_t* off = (const sv4_t*)g_dyn.tables->field(slot->pointer, field->mode - 1u, &kind);
    if (!off || kind != LLG_VPI_FIELD_PACKED) {
        vpi_error("LLG_VPI_MODEL", "the rand_mode state disagrees with the class layout");
        return vpiUndefined;
    }
    /* 18.8: the hidden off bit is 0 while the variable is active. */
    return sv4_to_u64(*off) == 0;
}

/* vpi_get64(property, slot); `*handled` 0 when llg_vpi.c should answer. */
/* Whether `slot` is an `event` class property (37.25). Its place's storage
 * is the handle slot naming the synchronization object (NULL for a property
 * reached from a class definition). */
static int32_t dyn_event_property(const llg_vpi_slot_t* slot, dyn_place_t* place) {
    return slot->kind == SLOT_PROPERTY && dyn_place(slot, place) &&
           place->shape->shape == LLG_VPI_SHAPE_EVENT;
}

/* The event handle of an event property of a live object; 0 when the
 * property has no object (a class definition's) or the object is dying. */
static int32_t dyn_property_event(const llg_vpi_slot_t* slot, llg_event_t* event, const char* what) {
    dyn_place_t place;
    if (!dyn_event_property(slot, &place)) return 0;
    if (!place.storage) {
        vpi_error("LLG_VPI_RELATION",
                  "an event property of a class definition names no event (37.27 detail 2)");
        return 0;
    }
    if (dyn_refuse_dying(place.owner, what)) return 0;
    event->object = *(llg_event_object_t* const*)place.storage;
    return 1;
}

/* 37.25 properties of an event property; others are wrong-object queries. */
static PLI_INT64 dyn_event_get(int32_t property, const dyn_place_t* place) {
    switch (property) {
        case vpiArray:
        case vpiArrayMember:
            return 0;
        case vpiAutomatic:
            /* A static event property is one event of the class. */
            return place->row != DYN_STATIC_ROW;
        case vpiAllocScheme:
            return place->owner ? vpiDynamicScheme : vpiOtherScheme;
        default:
            unsupported_property();
            return vpiUndefined;
    }
}

/* 37.25 properties of a named event array property, as a module-level
 * array answers them (decision A19-D69); others are wrong-object queries. */
static PLI_INT64 dyn_event_array_get(int32_t property, const dyn_place_t* place) {
    const llg_vpi_shape_t* shape = place->shape;
    switch (property) {
        case vpiSize:
            /* The declared size, also for a class definition's property. */
            return (shape->left > shape->right ? shape->left - shape->right
                                               : shape->right - shape->left) + 1;
        case vpiAutomatic:
            return 1;
        case vpiAllocScheme:
            return place->owner ? vpiDynamicScheme : vpiOtherScheme;
        default:
            unsupported_property();
            return vpiUndefined;
    }
}

/* Whether element handle `slot` is an event of a named event array
 * property. */
static int32_t dyn_event_element(const llg_vpi_slot_t* slot) {
    if (slot->kind != SLOT_DYNELEM) return 0;
    const llg_vpi_shape_t* shape = dyn_element_shape(slot);
    return shape && shape->shape == LLG_VPI_SHAPE_EVENT;
}

/* The event an element of a named event array property names. */
static int32_t dyn_element_event(const llg_vpi_slot_t* slot, llg_event_t* event, const char* what) {
    if (slot->object == DYN_DEFN_ROW) {
        vpi_error("LLG_VPI_VALUE",
                  "an event of a property of a class definition names no event (37.27 detail 2)");
        return 0;
    }
    dyn_place_t place;
    if (!dyn_container_place(slot->object, slot->pointer, (uint32_t)slot->aux, slot->right, &place)) {
        vpi_error("LLG_VPI_HANDLE", "the array of this element no longer exists");
        return 0;
    }
    if (dyn_refuse_dying(place.owner, what)) return 0;
    int32_t exists;
    llg_value_t* element = dyn_locate(&place, slot, &exists);
    if (!exists || !element || !element->desc || element->desc->kind != LLG_VALUE_EVENT) {
        vpi_error("LLG_VPI_INDEX", "this element no longer exists (decision A19-D23)");
        return 0;
    }
    event->object = (llg_event_object_t*)element->value.handle;
    return 1;
}

static PLI_INT64 dyn_get(int32_t property, llg_vpi_slot_t* slot, int32_t* handled) {
    *handled = 1;
    if (property == vpiType) return dyn_type(slot);
    if (property == vpiLineNo && (slot->kind == SLOT_PROPERTY || slot->kind == SLOT_DYNELEM ||
                                  slot->kind == SLOT_EXTENDS)) {
        uint32_t line = 0;
        const char* file = NULL;
        if (dyn_location(slot, &line, &file)) return line;
        unsupported_property();
        return vpiUndefined;
    }
    {
        dyn_place_t place;
        if (dyn_event_property(slot, &place)) return dyn_event_get(property, &place);
        if (slot->kind == SLOT_PROPERTY && dyn_place(slot, &place) && dyn_is_event_array(place.shape))
            return dyn_event_array_get(property, &place);
    }
    switch (slot->kind) {
        case SLOT_CLASSDEFN:
        case SLOT_CLASSTS:
            if (property == vpiClassType) return vpiUserDefinedClass;
            /* 37.3.7: class methods are automatic by default. */
            if (property == vpiAutomatic) return 1;
            if (property == vpiVirtual && slot->kind == SLOT_CLASSDEFN)
                return (dyn_class((uint32_t)slot->aux)->flags & LLG_VPI_CLASS_VIRTUAL) != 0;
            /* 37.3.3: a class definition and its typespec are on the
             * class declaration's line. */
            if (property == vpiLineNo && dyn_class((uint32_t)slot->aux)->line)
                return dyn_class((uint32_t)slot->aux)->line;
            break;
        case SLOT_CLASSITEM: {
            int32_t ok;
            if (slot->pointer && property == vpiIsConstraintEnabled &&
                dyn_refuse_dying(slot->pointer, "vpi_get"))
                return vpiUndefined;
            PLI_INT64 value = dyn_item_get(property, slot, &ok);
            if (ok) return value;
            break;
        }
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
            /* 37.17 details 2 and 17: a record member is a member of its
             * structure, not of the array. */
            if (property == vpiArrayMember) return !dyn_is_member(slot);
            if (property == vpiAllocScheme) return vpiDynamicScheme;
            /* An element is automatic with its array: of an automatic or
             * of an instance property; one of a module or static property
             * array is static (37.3.7, decision A19-D87). */
            if (property == vpiAutomatic)
                return slot->pointer != NULL || slot->object == DYN_AUTO_ROW ||
                       slot->object == DYN_DEFN_ROW;
            /* 37.25: an event of a named event array property has the
             * properties of a named event only. */
            if (shape->shape == LLG_VPI_SHAPE_EVENT) {
                if (property == vpiArray) return 0;
                break;
            }
            if (property == vpiStructUnionMember) return dyn_is_member(slot);
            if (property == vpiSigned) return shape->is_signed;
            if (shape->shape == LLG_VPI_SHAPE_RECORD) {
                /* 37.17 detail 9: the number of fields of an unpacked
                 * structure; it is not packed (37.24) and has no vector
                 * value (detail 20). */
                if (property == vpiSize) return shape->member_count;
                if (property == vpiPacked || property == vpiVector || property == vpiScalar)
                    return 0;
            }
            if (shape->shape == LLG_VPI_SHAPE_CONTAINER && dyn_is_member_array(slot)) {
                /* 37.17 details 2, 9 and 21: an array member of a
                 * structure is an array variable. */
                if (property == vpiArrayType) return shape->array_type;
                if (property == vpiArray) return 1;
                if (property == vpiSize) {
                    dyn_place_t level;
                    if (!dyn_member_array(slot, &level)) return vpiUndefined;
                    return (PLI_INT64)dyn_leaf_count(&level, 0);
                }
            }
            if (property == vpiSize) {
                if (shape->shape == LLG_VPI_SHAPE_PACKED) return shape->width;
                /* A chandle reads as a 64-bit image (decision A19-D21). */
                if (shape->shape == LLG_VPI_SHAPE_CHANDLE) return 64;
                if (shape->shape == LLG_VPI_SHAPE_STRING) {
                    s_vpi_value value;
                    value.format = vpiStringVal;
                    dyn_element_get(slot, &value);
                    return g_vpi.error_pending ? vpiUndefined : (PLI_INT64)strlen(value.value.str);
                }
            }
            if (property == vpiObjId && shape->shape == LLG_VPI_SHAPE_CLASS) {
                int32_t ok;
                void* object = dyn_class_target(slot, &ok);
                if (!ok) break;
                return object ? (PLI_INT64)live_get(object)->id : 0;
            }
            break;
        }
        case SLOT_PROPERTY:
        case SLOT_OBJECT: {
            dyn_place_t place;
            if (!dyn_place(slot, &place)) break;
            const llg_vpi_shape_t* shape = place.shape;
            if (shape->shape == LLG_VPI_SHAPE_VIF) {
                /* 37.15: a reference object for a virtual interface. */
                if (property == vpiVirtual) return 1;
                break;
            }
            switch (property) {
                case vpiSize: {
                    int32_t ok;
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
                    return object ? (PLI_INT64)live_get(object)->id : 0;
                }
                case vpiSigned:
                    return shape->shape == LLG_VPI_SHAPE_PACKED ? shape->is_signed : 0;
                case vpiArrayMember:
                    return 0;
                case vpiArray:
                    /* Deprecated (37.17 detail 2): TRUE for an array. */
                    return shape->shape == LLG_VPI_SHAPE_CONTAINER;
                case vpiScalar:
                case vpiVector:
                    /* 37.17 detail 20: FALSE for string, chandle and class
                     * variables. */
                    if (shape->shape == LLG_VPI_SHAPE_STRING ||
                        shape->shape == LLG_VPI_SHAPE_CHANDLE ||
                        shape->shape == LLG_VPI_SHAPE_CLASS)
                        return 0;
                    break;
                case vpiStructUnionMember:
                    /* 37.17 detail 17: a member of a structure variable. */
                    return slot->kind == SLOT_OBJECT &&
                           (g_vpi.objects[slot->object].flags & LLG_VPI_OBJ_MEMBER) != 0;
                case vpiAutomatic:
                    /* 37.3.7: a static property is a static variable. */
                    return slot->kind == SLOT_PROPERTY && place.row != DYN_STATIC_ROW;
                case vpiAllocScheme:
                    return slot->kind == SLOT_PROPERTY && place.owner ? vpiDynamicScheme
                                                                      : vpiOtherScheme;
                case vpiLineNo:
                    if (slot->kind == SLOT_OBJECT) return g_vpi.objects[slot->object].line;
                    break;
                case vpiRandType:
                case vpiVisibility:
                case vpiIsRandomized:
                    return dyn_random_property(property, slot);
                case vpiConstantVariable:
                    /* 37.17: declared const (6.20.6, 8.5). */
                    if (slot->kind == SLOT_OBJECT)
                        return (g_vpi.objects[slot->object].flags & LLG_VPI_OBJ_CONST) != 0;
                    return dyn_field((uint32_t)slot->right, (uint32_t)slot->aux)->is_const != 0;
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
    int32_t length = vsnprintf(g_vpi.str_buffer, sizeof(g_vpi.str_buffer), format, arguments);
    va_end(arguments);
    if (length < 0 || (size_t)length >= sizeof(g_vpi.str_buffer)) {
        vpi_error("LLG_VPI_LIMIT", "the VPI name is too long");
        return NULL;
    }
    return g_vpi.str_buffer;
}

/* A growing name (element paths). */
typedef struct {
    char* data;
    size_t length;
    size_t capacity;
} dyn_name_t;

static LLG_OUTLYING void dyn_name_append(dyn_name_t* name, const char* text, size_t length) {
    if (name->length + length + 1u > name->capacity) {
        size_t capacity = name->capacity ? name->capacity : 64u;
        while (capacity < name->length + length + 1u) capacity *= 2u;
        char* grown = (char*)vpi_realloc(name->data, capacity);
        name->data = grown;
        name->capacity = capacity;
    }
    if (length) memcpy(name->data + name->length, text, length);
    name->length += length;
    name->data[name->length] = '\0';
}

static LLG_OUTLYING void dyn_name_index(dyn_name_t* name, int64_t index, int32_t is_signed) {
    char text[32];
    int32_t length = is_signed ? snprintf(text, sizeof(text), "[%lld]", (long long)index)
                           : snprintf(text, sizeof(text), "[%llu]", (unsigned long long)index);
    dyn_name_append(name, text, (size_t)length);
}

/* `[index]`, `[i][j]` (a fixed-size array of more dimensions), `[key]` or
 * `["key"]` of selection `step` in container `place`. A quote in a string
 * key is written `\"`, so the name reads back by vpi_handle_by_name
 * (decision A19-D81). */
static LLG_OUTLYING void dyn_name_step(dyn_name_t* name, const dyn_place_t* place, const dyn_step_t* step) {
    if (step->at == DYN_AT_STRING) {
        dyn_name_append(name, "[\"", 2u);
        for (uint32_t i = 0; i < step->length; ++i) {
            if (step->key[i] == '"') dyn_name_append(name, "\\", 1u);
            dyn_name_append(name, &step->key[i], 1u);
        }
        dyn_name_append(name, "\"]", 2u);
        return;
    }
    if (step->at == DYN_AT_INTEGRAL) {
        sv4_t key = dyn_integral_key(place, step->element);
        if (llg_sv4_signed(key)) dyn_name_index(name, sv4_to_i64(key), 1);
        else dyn_name_index(name, (int64_t)sv4_to_u64(key), 0);
        sv4_destroy(&key);
        return;
    }
    int64_t indices[DYN_MAX_DIMS];
    uint32_t count = dyn_declared_indices(place, step->element, indices);
    for (uint32_t d = 0; d < count; ++d) dyn_name_index(name, indices[d], 1);
}

/* The vpiName of an automatic named `name`: a block local's ends its block
 * path (`blk.q` names `q`, decision A19-D88). */
static LLG_OUTLYING const char* dyn_auto_leaf(const char* name) {
    const char* dot = strrchr(name, '.');
    return dot ? dot + 1 : name;
}

/* `base` followed by the selections of `slot`'s path from `first` on
 * (`.member` for a member), resolving each nested container (its keys'
 * type) from the array `top`. */
static LLG_OUTLYING int32_t dyn_name_path(dyn_name_t* name, const llg_vpi_slot_t* slot, const dyn_place_t* top,
                         uint32_t first, const char* base) {
    dyn_name_append(name, base, strlen(base));
    dyn_place_t level = *top;
    const llg_vpi_shape_t* shape = NULL;
    llg_value_t* value = NULL;
    if (top->shape->shape == LLG_VPI_SHAPE_RECORD) {
        shape = top->shape;
        value = (llg_value_t*)top->storage;
    }
    uint32_t count = 1u + slot->span;
    for (uint32_t i = 0; i < count; ++i) {
        dyn_step_t step = dyn_step_at(slot, i);
        if (step.at == DYN_AT_MEMBER) {
            const llg_vpi_member_t* member = dyn_member(shape, step.element);
            if (!member) return 0;
            if (i >= first) {
                dyn_name_append(name, ".", 1u);
                dyn_name_append(name, member->name, strlen(member->name));
            }
            shape = dyn_shape(member->shape);
            value = value && value->desc && value->desc->kind == LLG_VALUE_AGGREGATE &&
                            step.element < value->desc->member_count
                        ? &value->value.items[step.element]
                        : NULL;
            continue;
        }
        if (i && !dyn_nested_level(value, shape, &level)) {
            vpi_error("LLG_VPI_INDEX", "this element no longer exists (decision A19-D23)");
            return 0;
        }
        if (i >= first) dyn_name_step(name, &level, &step);
        shape = dyn_shape(level.shape->element);
        int32_t exists = 1;
        value = i + 1u < count ? dyn_locate_step(&level, &step, &exists) : NULL;
        if (!exists) {
            vpi_error("LLG_VPI_INDEX", "this element no longer exists (decision A19-D23)");
            return 0;
        }
    }
    return 1;
}

/* 37.17 detail 25: the full name of a static property, through its class
 * definition (`top.Packet::Id`), whether a class variable or the class
 * definition reaches it. Owned by the caller; NULL with an error. */
static LLG_OUTLYING char* dyn_static_full_name(uint32_t field) {
    const llg_vpi_class_static_t* entry = dyn_static(field);
    const llg_vpi_class_t* layout = entry ? dyn_class(entry->class_index) : NULL;
    if (!layout) return NULL;
    size_t length = strlen(layout->full_name) + 2u + strlen(entry->field.name) + 1u;
    char* text = (char*)vpi_malloc(length);
    snprintf(text, length, "%s::%s", layout->full_name, entry->field.name);
    return text;
}

/* vpiName (`full` 0) or vpiFullName of an element: the array's name and
 * every index of its path (`q[1]`, `qq[0][1]`, `tb.t.q[1]`); a record
 * member is named by its member name, its full name continues the
 * element's (`tb.rq[0].a`). An element of a class property has no full
 * name (37.17 detail 25). */
static char* dyn_element_name(const llg_vpi_slot_t* slot, int32_t full) {
    if (slot->object == DYN_DEFN_ROW) {
        const llg_vpi_class_field_t* field = dyn_field((uint32_t)slot->right, (uint32_t)slot->aux);
        const llg_vpi_shape_t* array = field && field->name ? dyn_shape(field->shape) : NULL;
        if (full || !dyn_is_event_array(array)) {
            vpi_error("LLG_VPI_PROPERTY",
                      "an element of a class property has no vpiFullName (37.17 detail 25)");
            return NULL;
        }
        int64_t left = array->left, right = array->right;
        int64_t index = left >= right ? left - (int64_t)slot->element : left + (int64_t)slot->element;
        return dyn_text("%s[%lld]", field->name, (long long)index);
    }
    dyn_place_t place;
    if (!dyn_container_place(slot->object, slot->pointer, (uint32_t)slot->aux, slot->right,
                             &place))
        return NULL;
    if (!full && dyn_is_member(slot)) {
        const llg_vpi_member_t* member = dyn_member(dyn_path_shape(slot, place.shape, slot->span),
                                                    dyn_step_at(slot, slot->span).element);
        if (!member) return NULL;
        return dyn_text("%s", member->name);
    }
    if (full && place.owner) {
        vpi_error("LLG_VPI_PROPERTY",
                  "an element of a class property has no vpiFullName (37.17 detail 25)");
        return NULL;
    }
    dyn_name_t name = {0};
    uint32_t first = full ? 0u : dyn_tail_first(slot);
    if (first) {
        /* An element of an array member of a structure: the member's name
         * and the element's indices in it (`fa[1]`). */
        const llg_vpi_member_t* member = dyn_member(dyn_path_shape(slot, place.shape, first - 1u),
                                                    dyn_step_at(slot, first - 1u).element);
        if (!member) return NULL;
        int32_t named = dyn_name_path(&name, slot, &place, first, member->name);
        char* text = named ? dyn_text("%s", name.data) : NULL;
        free(name.data);
        return text;
    }
    char* base = NULL;
    if (!full) {
        base = NULL;
    } else if (place.row == DYN_STATIC_ROW) {
        if (!(base = dyn_static_full_name((uint32_t)slot->aux))) return NULL;
    } else if (place.row == DYN_AUTO_ROW) {
        /* An automatic's element: the frame's name, the variable, the index
         * (`tb.t.q[1]`). */
        const char* frame = rts_auto_frame_name((uint64_t)place.serial);
        if (!frame) return NULL;
        size_t length = strlen(frame) + 1u + strlen(place.name) + 1u;
        base = (char*)vpi_malloc(length);
        snprintf(base, length, "%s.%s", frame, place.name);
    }
    int32_t ok = dyn_name_path(&name, slot, &place, 0,
                           !full ? (place.row == DYN_AUTO_ROW ? dyn_auto_leaf(place.name) : place.name)
                                 : base ? base : g_vpi.objects[place.row].full_name);
    free(base);
    char* text = ok ? dyn_text("%s", name.data) : NULL;
    free(name.data);
    return text;
}

/* vpi_get_str(property, slot); `*handled` 0 when llg_vpi.c should answer. */
static const char* dyn_get_str(int32_t property, llg_vpi_slot_t* slot, int32_t* handled) {
    *handled = 1;
    if (property == vpiType) {
        const char* name = dyn_type_name(dyn_type(slot));
        if (name) return name;
        vpi_error("LLG_VPI_PROPERTY", "unknown VPI object type");
        return NULL;
    }
    if (property == vpiFile && (slot->kind == SLOT_PROPERTY || slot->kind == SLOT_DYNELEM ||
                                slot->kind == SLOT_EXTENDS)) {
        uint32_t line = 0;
        const char* file = NULL;
        if (dyn_location(slot, &line, &file)) return file;
        vpi_error("LLG_VPI_PROPERTY", "this object has no source location");
        return NULL;
    }
    switch (slot->kind) {
        case SLOT_CLASSDEFN:
        case SLOT_CLASSTS:
            if (property == vpiName) return dyn_class((uint32_t)slot->aux)->name;
            /* 37.10 detail 5, 37.12. */
            if (property == vpiFullName) return dyn_class((uint32_t)slot->aux)->full_name;
            if (property == vpiFile && dyn_class((uint32_t)slot->aux)->line)
                return dyn_class((uint32_t)slot->aux)->file;
            break;
        case SLOT_CLASSITEM:
            return dyn_item_get_str(property, slot);
        case SLOT_PROPERTY:
            if (property == vpiName) return dyn_field((uint32_t)slot->right, (uint32_t)slot->aux)->name;
            if (property == vpiFullName) {
                /* 37.17 detail 25. */
                char* text = dyn_static((uint32_t)slot->aux) ? dyn_static_full_name((uint32_t)slot->aux)
                                                             : NULL;
                if (!text) {
                    vpi_error("LLG_VPI_PROPERTY", "a non-static class property has no vpiFullName");
                    return NULL;
                }
                const char* result = dyn_text("%s", text);
                free(text);
                return result;
            }
            break;
        case SLOT_DYNELEM:
            if (property == vpiName || property == vpiFullName)
                return dyn_element_name(slot, property == vpiFullName);
            break;
        case SLOT_BUILTINOBJ:
            /* The built-in class's name, like a class object's (A19-D64). */
            if (property == vpiName)
                return slot->aux == LLG_VPI_BUILTIN_MAILBOX ? "mailbox" : "semaphore";
            break;
        case SLOT_CLASSOBJ:
            /* 37.29: a class object is named by its class (decision
             * A19-D64); it has no full name, being in no scope. */
            if (property == vpiName) {
                uint32_t class_index;
                if (dyn_refuse_dying(slot->pointer, "vpi_get_str")) return NULL;
                if (dyn_object_class(slot->pointer, &class_index))
                    return dyn_class(class_index)->name;
            }
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
        case SLOT_CLASSITEM:
            dyn_item_value(slot, output);
            return;
        case SLOT_DYNELEM:
            if (dyn_event_element(slot)) {
                vpi_error("LLG_VPI_VALUE", "a named event has no value (37.25)");
                return;
            }
            dyn_element_get(slot, output);
            return;
        case SLOT_OBJECT:
        case SLOT_PROPERTY: {
            dyn_place_t place;
            if (!dyn_place(slot, &place)) return;
            if (place.shape->shape == LLG_VPI_SHAPE_EVENT) {
                vpi_error("LLG_VPI_VALUE", "a named event has no value (37.25)");
                return;
            }
            if (dyn_is_event_array(place.shape)) {
                vpi_error("LLG_VPI_VALUE", "a named event array has no value (37.25)");
                return;
            }
            if (place.shape->shape == LLG_VPI_SHAPE_VIF) {
                vpi_error("LLG_VPI_VALUE", "a virtual interface has no value (37.15)");
                return;
            }
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
static void dyn_put_value(llg_vpi_slot_t* slot, p_vpi_value input, int32_t mode) {
    {
        dyn_place_t place;
        if (dyn_event_property(slot, &place)) {
            /* 38.34: a put toggles the named event; value_p may be NULL
             * (vpi_put_value already refused read-only regions). */
            llg_event_t event;
            if (mode != vpiNoDelay) {
                vpi_error("LLG_VPI_UNSUPPORTED", "llg triggers a named event only with vpiNoDelay");
                return;
            }
            if (dyn_property_event(slot, &event, "vpi_put_value")) llg_event_trigger(&event);
            return;
        }
        if (dyn_event_element(slot)) {
            /* 38.34 for an event of a named event array property, as for a
             * module-level array's (decision A19-D69). */
            llg_event_t event;
            if (mode != vpiNoDelay) {
                vpi_error("LLG_VPI_UNSUPPORTED", "llg triggers a named event only with vpiNoDelay");
                return;
            }
            if (dyn_element_event(slot, &event, "vpi_put_value")) llg_event_trigger(&event);
            return;
        }
        if (slot->kind == SLOT_PROPERTY && dyn_place(slot, &place) && dyn_is_event_array(place.shape)) {
            vpi_error("LLG_VPI_VALUE", "a named event array cannot receive a value (37.25)");
            return;
        }
    }
    if (mode != vpiNoDelay) {
        vpi_error("LLG_VPI_FLAGS",
                  "llg writes class properties and array elements with vpiNoDelay only and "
                  "forces no string, property or element (decision A19-D23)");
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
    const llg_vpi_class_static_t* entry =
        slot->kind == SLOT_PROPERTY ? dyn_static((uint32_t)slot->aux) : NULL;
    sv4_t* dependency = slot->kind == SLOT_OBJECT && place.row != LLG_VPI_NONE
                            ? g_vpi.objects[place.row].dependency
                            : entry ? entry->dependency : NULL;
    dyn_write(place.shape, place.storage, dependency, input);
}

/* vpi_compare_objects of two dynamic handles: 1 same object, 0 different,
 * -1 when either is not a dynamic handle. */
static int32_t dyn_compare(const llg_vpi_slot_t* a, uint32_t a_index, const llg_vpi_slot_t* b,
                       uint32_t b_index) {
    (void)a_index;
    (void)b_index;
    int32_t a_dynamic = a->kind == SLOT_CLASSOBJ || a->kind == SLOT_PROPERTY || a->kind == SLOT_DYNELEM;
    int32_t b_dynamic = b->kind == SLOT_CLASSOBJ || b->kind == SLOT_PROPERTY || b->kind == SLOT_DYNELEM;
    if (!a_dynamic && !b_dynamic) return -1;
    if (a->kind != b->kind) return 0;
    switch (a->kind) {
        case SLOT_CLASSOBJ:
        case SLOT_BUILTINOBJ:
            return a->pointer == b->pointer;
        case SLOT_PROPERTY:
            /* A declaration's property (no storage) is one object per
             * class and field, distinct from every object's property. */
            if (!a->pointer && !b->pointer) return a->right == b->right && a->aux == b->aux;
            if (!a->pointer || !b->pointer) return 0;
            return a->pointer == b->pointer && a->aux == b->aux;
        default:
            if (a->object != b->object || a->pointer != b->pointer || a->aux != b->aux ||
                a->span != b->span || (a->object == DYN_AUTO_ROW && a->right != b->right))
                return 0;
            for (uint32_t i = 0; i <= a->span; ++i) {
                dyn_step_t x = dyn_step_at(a, i), y = dyn_step_at(b, i);
                if (x.at != y.at) return 0;
                if (x.at == DYN_AT_STRING) {
                    if (x.length != y.length || (x.length && memcmp(x.key, y.key, x.length) != 0))
                        return 0;
                } else if (x.element != y.element) {
                    return 0;
                }
            }
            return 1;
    }
}

/* ── names and indices ──────────────────────────────────────────────────── */

/* The method `name` (`length` bytes) class `class_index` declares, or
 * NULL. */
static LLG_OUTLYING vpiHandle dyn_method_named(uint32_t class_index, const char* name, size_t length) {
    const llg_vpi_class_t* layout = dyn_class(class_index);
    for (uint32_t k = 0; layout && k < layout->item_count; ++k) {
        const llg_vpi_class_item_t* item = dyn_item(class_index, k);
        if (dyn_item_is_method(item) && strlen(item->name) == length &&
            memcmp(item->name, name, length) == 0)
            return item_handle(class_index, k);
    }
    return NULL;
}

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
            /* A property with no VPI form (a virtual interface) names no
             * object, as in vpiVariables; an event property is a named
             * event. */
            return dyn_field_listed(class_index, i, 0) ||
                           dyn_field_listed(class_index, i, vpiNamedEvent)
                       ? make_property(object, class_index, i)
                       : NULL;
    }
    /* A method of the object's class or of a base it inherits from
     * (37.37: `top.p.get`); a derived class's declaration hides a base's. */
    for (uint32_t current = class_index; current != LLG_VPI_NONE;) {
        vpiHandle method = dyn_method_named(current, name, length);
        if (method) return method;
        uint32_t base = dyn_class(current)->base;
        current = base ? base - 1u : LLG_VPI_NONE;
    }
    /* A static property through an object (37.17 detail 25: `p.Id`); a
     * derived class's declaration hides its base's. */
    uint32_t found = LLG_VPI_NONE;
    for (uint32_t k = 0;; ++k) {
        uint32_t field = dyn_class_static(class_index, 1, k);
        if (field == LLG_VPI_NONE) break;
        const char* candidate = dyn_field(class_index, field)->name;
        if (strlen(candidate) == length && memcmp(candidate, name, length) == 0) found = field;
    }
    return found != LLG_VPI_NONE && (dyn_field_listed(class_index, found, 0) ||
                                     dyn_field_listed(class_index, found, vpiNamedEvent))
               ? make_static_property(found) : NULL;
}

/* The selection of the element of container `place` at `index` (a
 * dynamic array or queue index, the declared index of a one-dimensional
 * fixed-size array, an existing integral key of an associative array); 0,
 * with an error unless `quiet`, when there is none. */
static LLG_OUTLYING int32_t dyn_select_step(const dyn_place_t* place, int64_t index, dyn_step_t* step, int32_t quiet) {
    const llg_vpi_shape_t* shape = place->shape;
    memset(step, 0, sizeof(*step));
    if (shape->array_type == vpiAssocArray) {
        if (shape->key != LLG_VPI_KEY_INTEGRAL) {
            if (!quiet)
                vpi_error("LLG_VPI_INDEX", "llg selects associative elements by integral keys only");
            return 0;
        }
        if (!place->storage) {
            if (!quiet) vpi_error("LLG_VPI_INDEX", "the associative array has no such key");
            return 0;
        }
        sv4_t key = dyn_integral_key(place, (uint64_t)index);
        int32_t exists = shape->values
            ? llg_assoc_value_exists_integral((const llg_assoc_value_t*)place->storage, key)
            : llg_assoc_exists_integral((const llg_assoc_t*)place->storage, key);
        int64_t stored = llg_sv4_signed(key) ? sv4_to_i64(key) : (int64_t)sv4_to_u64(key);
        sv4_destroy(&key);
        if (!exists || stored != index) {
            if (!quiet) vpi_error("LLG_VPI_INDEX", "the associative array has no such key");
            return 0;
        }
        step->at = DYN_AT_INTEGRAL;
        step->element = (uint64_t)index;
        return 1;
    }
    int64_t left, right;
    if (dyn_fixed_range(place, &left, &right)) {
        /* A fixed-size array: the declared index (37.17). */
        int64_t low = left < right ? left : right, high = left < right ? right : left;
        if (index < low || index > high) {
            if (!quiet) vpi_error("LLG_VPI_INDEX", "index outside the array range");
            return 0;
        }
        index = left >= right ? left - index : index - left;
    }
    if (index < 0 || (uint64_t)index >= dyn_container_size(place)) {
        if (!quiet) vpi_error("LLG_VPI_INDEX", "array index is outside the current size");
        return 0;
    }
    step->at = DYN_AT_INDEX;
    step->element = (uint64_t)index;
    return 1;
}

/* An element of container `place` at `index` (dynamic array, queue, or an
 * integral key of an associative array). */
static LLG_OUTLYING vpiHandle dyn_element_by_index(const dyn_place_t* place, uint32_t field, int64_t index,
                                      int32_t quiet) {
    if (!place->storage && !place->owner && dyn_is_event_array(place->shape)) {
        /* An event of a class definition's event array property: named and
         * typed, with no event object (37.25, 37.27 detail 2). */
        int64_t left = place->shape->left, right = place->shape->right;
        int64_t low = left < right ? left : right, high = left < right ? right : left;
        if (index < low || index > high) {
            if (!quiet) vpi_error("LLG_VPI_INDEX", "index outside the array range");
            return NULL;
        }
        vpiHandle handle;
        llg_vpi_slot_t* slot = alloc_slot(SLOT_DYNELEM, &handle);
        if (!slot) return NULL;
        slot->object = DYN_DEFN_ROW;
        slot->right = (int64_t)place->class_index;
        slot->aux = (int32_t)field;
        slot->lsb = DYN_AT_INDEX;
        slot->element = (uint64_t)(left >= right ? left - index : index - left);
        return handle;
    }
    if (!place->storage) {
        if (!quiet)
            vpi_error("LLG_VPI_INDEX",
                      "a property of a class definition has no elements (37.27 detail 2)");
        return NULL;
    }
    if (dyn_fixed_dim_count(place) > 1u) {
        if (!quiet)
            vpi_error("LLG_VPI_INDEX",
                      "use vpi_handle_by_multi_index for a multidimensional array");
        return NULL;
    }
    if (dyn_shape(place->shape->element)->shape == LLG_VPI_SHAPE_CONTAINER) {
        /* 37.17 detail 4: the elements of an array whose elements hold
         * containers are its innermost ones; the 2009 object model has
         * no subarray (decision A20-D23). */
        if (!quiet)
            vpi_error("LLG_VPI_INDEX",
                      "an element of a nested array needs every index; use vpi_handle_by_name");
        return NULL;
    }
    dyn_step_t step;
    if (!dyn_select_step(place, index, &step, quiet)) return NULL;
    return make_dyn_path(place, field, &step, 1u);
}

/* The element of a fixed-size array `place` at its declared `indices`,
 * one per dimension (vpi_handle_by_multi_index), or of a one-dimensional
 * array at its one index. */
static LLG_OUTLYING vpiHandle dyn_element_by_indices(const dyn_place_t* place, uint32_t field,
                                        const int64_t* indices, uint32_t count, int32_t quiet) {
    uint32_t dims = dyn_fixed_dim_count(place);
    if (count == 1u && dims <= 1u) return dyn_element_by_index(place, field, indices[0], quiet);
    if (dims < 2u) {
        if (!quiet)
            vpi_error("LLG_VPI_INDEX",
                      dyn_shape(place->shape->element)->shape == LLG_VPI_SHAPE_CONTAINER
                          ? "llg selects the elements of a nested array by name"
                          : "this array has one dimension");
        return NULL;
    }
    if (count != dims) {
        /* Decision A20-D23: no subarray objects. */
        if (!quiet) vpi_error("LLG_VPI_INDEX", "give one index per dimension of the array");
        return NULL;
    }
    if (!place->storage) {
        if (!quiet)
            vpi_error("LLG_VPI_INDEX",
                      "a property of a class definition has no elements (37.27 detail 2)");
        return NULL;
    }
    uint64_t offset;
    if (!dyn_fixed_offset(place, indices, &offset, quiet)) return NULL;
    return make_dyn_element(place, field, DYN_AT_INDEX, offset, NULL, 0);
}

/* The element of array member `slot` of a structure (37.24) at its
 * declared `indices`, one per dimension (a dynamic array or queue index or
 * an integral key for the others); NULL, with an error unless `quiet`. */
static LLG_OUTLYING vpiHandle dyn_member_element(llg_vpi_slot_t* slot, const int64_t* indices,
                                                 uint32_t count, int32_t quiet) {
    dyn_place_t level;
    if (!dyn_member_array(slot, &level)) return NULL;
    if (dyn_shape(level.shape->element)->shape == LLG_VPI_SHAPE_CONTAINER) {
        /* 37.17 detail 4: no subarray (decision A20-D23). */
        if (!quiet)
            vpi_error("LLG_VPI_INDEX",
                      "an element of a nested array needs every index; use vpi_handle_by_name");
        return NULL;
    }
    uint32_t dims = dyn_fixed_dim_count(&level);
    dyn_step_t step;
    memset(&step, 0, sizeof(step));
    if (dims > 1u) {
        if (count != dims) {
            if (!quiet) vpi_error("LLG_VPI_INDEX", "give one index per dimension of the array");
            return NULL;
        }
        uint64_t offset;
        if (!dyn_fixed_offset(&level, indices, &offset, quiet)) return NULL;
        step.at = DYN_AT_INDEX;
        step.element = offset;
    } else {
        if (count != 1u) {
            if (!quiet) vpi_error("LLG_VPI_INDEX", "this array has one dimension");
            return NULL;
        }
        if (!dyn_select_step(&level, indices[0], &step, quiet)) return NULL;
    }
    return dyn_extend(slot, &step);
}

static LLG_OUTLYING vpiHandle dyn_handle_by_multi_index(llg_vpi_slot_t* slot, const int64_t* indices,
                                           uint32_t count) {
    dyn_place_t place;
    if (dyn_is_member_array(slot)) {
        if (dyn_refuse_dying(slot->pointer, "vpi_handle_by_multi_index")) return NULL;
        return dyn_member_element(slot, indices, count, 0);
    }
    if (slot->kind == SLOT_DYNELEM || !dyn_place(slot, &place) ||
        place.shape->shape != LLG_VPI_SHAPE_CONTAINER) {
        vpi_error("LLG_VPI_INDEX", "object has no access by index");
        return NULL;
    }
    if (dyn_refuse_dying(place.owner, "vpi_handle_by_multi_index")) return NULL;
    if (dyn_is_event_array(place.shape) && count != 1u) {
        vpi_error("LLG_VPI_INDEX", "a named event array property has one dimension");
        return NULL;
    }
    return dyn_element_by_indices(&place, slot->kind == SLOT_PROPERTY ? (uint32_t)slot->aux : 0u,
                                  indices, count, 0);
}

/* A `"key"` index of a name, as element names spell it (dyn_element_name):
 * `\"` stands for a quote and any other backslash for itself. On success
 * `*cursor` is past the closing quote and `*key` (malloc'd, `*length`
 * bytes) holds the key. */
static int32_t dyn_parse_key(const char** cursor, char** key, size_t* length) {
    const char* p = *cursor;
    if (*p != '"') return 0;
    size_t size = 0;
    const char* q = ++p;
    for (; *q && *q != '"'; ++q, ++size)
        if (q[0] == '\\' && q[1] == '"') ++q;
    if (*q != '"') return 0;
    char* text = (char*)vpi_malloc(size + 1u);
    size_t n = 0;
    for (; p < q; ++p) {
        if (p[0] == '\\' && p[1] == '"') ++p;
        text[n++] = *p;
    }
    text[n] = '\0';
    *key = text;
    *length = n;
    *cursor = q + 1;
    return 1;
}

static vpiHandle dyn_handle_by_index(llg_vpi_slot_t* slot, PLI_INT32 index) {
    dyn_place_t place;
    if (dyn_is_member_array(slot)) {
        if (dyn_refuse_dying(slot->pointer, "vpi_handle_by_index")) return NULL;
        int64_t indices[1] = {index};
        return dyn_member_element(slot, indices, 1u, 0);
    }
    if (slot->kind == SLOT_DYNELEM || !dyn_place(slot, &place) ||
        place.shape->shape != LLG_VPI_SHAPE_CONTAINER) {
        vpi_error("LLG_VPI_INDEX", "object has no access by index");
        return NULL;
    }
    if (dyn_refuse_dying(place.owner, "vpi_handle_by_index")) return NULL;
    return dyn_element_by_index(&place, slot->kind == SLOT_PROPERTY ? (uint32_t)slot->aux : 0u,
                                index, 0);
}

/* The selection of the existing string key `key` (`length` bytes) of a
 * string-keyed associative array `place`; 0 when it is absent. */
static LLG_OUTLYING int32_t dyn_key_step(const dyn_place_t* place, const char* key, size_t length, dyn_step_t* step) {
    const llg_vpi_shape_t* shape = place->shape;
    if (!place->storage || shape->array_type != vpiAssocArray || shape->key != LLG_VPI_KEY_STRING ||
        length > LLG_VPI_MAX_ELEMENT_KEY)
        return 0;
    int32_t exists = shape->values
        ? llg_assoc_value_exists_string((const llg_assoc_value_t*)place->storage, key, length)
        : llg_assoc_exists_string((const llg_assoc_t*)place->storage, key, length);
    memset(step, 0, sizeof(*step));
    step->at = DYN_AT_STRING;
    step->key = key;
    step->length = (uint32_t)length;
    return exists;
}

/* The selection a `[index]` or `["key"]` at `*cursor` names in `place`;
 * on success `*cursor` is past the `]` and `*key` holds a parsed string
 * key the step points into (the caller frees it). */
static LLG_OUTLYING int32_t dyn_parse_step(const dyn_place_t* place, const char** cursor, dyn_step_t* step,
                          char** key) {
    const char* p = *cursor;
    *key = NULL;
    if (*p != '[') return 0;
    ++p;
    int32_t found = 0;
    if (*p == '"') {
        /* A string key (decision A19-D81). */
        size_t length = 0;
        if (dyn_parse_key(&p, key, &length) && *p == ']')
            found = dyn_key_step(place, *key, length, step);
    } else {
        int64_t index = 0;
        if (parse_index(&p, &index) && *p == ']') found = dyn_select_step(place, index, step, 1);
    }
    if (!found) return 0;
    *cursor = p + 1;
    return 1;
}

/* `[i]` after an element whose value holds a container: the element of
 * that container (37.17 detail 4), or NULL; fewer indices than a fixed-size
 * array member has dimensions set `*partial`. */
static LLG_OUTLYING vpiHandle dyn_suffix_nested(const llg_vpi_slot_t* slot, const char** cursor,
                                                int32_t* partial) {
    dyn_target_t target;
    dyn_place_t level;
    if (!dyn_resolve(slot, &target) || target.packed ||
        !dyn_nested_level(target.value, target.shape, &level))
        return NULL;
    uint32_t dims = dyn_fixed_dim_count(&level);
    if (dims > 1u) {
        /* A fixed-size array member of more dimensions: one index each. */
        int64_t indices[DYN_MAX_DIMS];
        const char* q = *cursor;
        for (uint32_t n = 0; n < dims; ++n) {
            const char* r = q + 1;
            if (*q != '[' || !parse_index(&r, &indices[n]) || *r != ']') {
                /* Fewer indices than dimensions: a subarray (A20-D23). */
                if (n && (*q == '\0' || *q == '.')) *partial = 1;
                return NULL;
            }
            q = r + 1;
        }
        uint64_t offset;
        if (!dyn_fixed_offset(&level, indices, &offset, 1)) return NULL;
        dyn_step_t step;
        memset(&step, 0, sizeof(step));
        step.at = DYN_AT_INDEX;
        step.element = offset;
        *cursor = q;
        return dyn_extend(slot, &step);
    }
    dyn_step_t step;
    char* key = NULL;
    vpiHandle next = dyn_parse_step(&level, cursor, &step, &key) ? dyn_extend(slot, &step) : NULL;
    free(key);
    return next;
}

/* `.name` after an element or member holding an unpacked structure: that
 * member (37.24), or NULL. */
static LLG_OUTLYING vpiHandle dyn_suffix_member(const llg_vpi_slot_t* slot, const char* name, size_t length) {
    const llg_vpi_shape_t* shape = dyn_element_shape(slot);
    if (!shape || shape->shape != LLG_VPI_SHAPE_RECORD) return NULL;
    for (uint32_t m = 0; m < shape->member_count; ++m) {
        const char* candidate = dyn_member(shape, m)->name;
        if (strlen(candidate) != length || memcmp(candidate, name, length) != 0) continue;
        dyn_step_t step;
        memset(&step, 0, sizeof(step));
        step.at = DYN_AT_MEMBER;
        step.element = m;
        return dyn_extend(slot, &step);
    }
    return NULL;
}

/* The element of container `place` (property or automatic `field`) that
 * the selections at `*cursor` name: `[i]`, `["key"]`, or one index per
 * dimension of a fixed-size array of more dimensions (`[i][j]`); on
 * success `*cursor` is past them. Fewer indices than dimensions set
 * `*partial` (a subarray, decision A20-D23). */
static LLG_OUTLYING vpiHandle dyn_first_suffix(const dyn_place_t* place, uint32_t field, const char** cursor,
                                  int32_t* partial) {
    uint32_t dims = dyn_fixed_dim_count(place);
    if (dims > 1u) {
        int64_t indices[DYN_MAX_DIMS];
        uint32_t n = 0;
        const char* q = *cursor;
        while (n < dims && *q == '[') {
            const char* r = q + 1;
            if (!parse_index(&r, &indices[n]) || *r != ']') break;
            q = r + 1;
            ++n;
        }
        if (n == dims) {
            vpiHandle element = dyn_element_by_indices(place, field, indices, dims, 1);
            if (element) *cursor = q;
            return element;
        }
        if (n && (*q == '\0' || *q == '.')) *partial = 1;
        return NULL;
    }
    dyn_step_t step;
    char* key = NULL;
    vpiHandle element = dyn_parse_step(place, cursor, &step, &key)
                            ? make_dyn_path(place, field, &step, 1u) : NULL;
    free(key);
    return element;
}

/* Member `name` (`length` bytes) of the record value at `place` (an
 * automatic or property `field`): its path handle, or NULL when the record
 * has no member of that name. */
static LLG_OUTLYING vpiHandle dyn_record_member(const dyn_place_t* place, uint32_t field, const char* name,
                                                size_t length) {
    for (uint32_t m = 0; m < place->shape->member_count; ++m) {
        const llg_vpi_member_t* member = dyn_member(place->shape, m);
        if (strlen(member->name) != length || strncmp(member->name, name, length) != 0) continue;
        dyn_step_t step;
        memset(&step, 0, sizeof(step));
        step.at = DYN_AT_MEMBER;
        step.element = m;
        return make_dyn_path(place, field, &step, 1u);
    }
    return NULL;
}

/* Continue name `p` from `current` (a handle the walk owns unless it is
 * `keep`): `[i]` into a container nested in an element, `.name` a member
 * of a structure element or a property of the object a class variable,
 * property or element refers to (37.29 detail 9: `top.p.Id`). A name that
 * denotes nothing is NULL; one that names a subarray is NULL with an
 * error (decision A20-D23). */
static LLG_OUTLYING vpiHandle dyn_walk_suffix(vpiHandle current, vpiHandle keep, const char* p, int32_t partial) {
    int32_t unserved_select = 0, unserved_member = 0;
    while (current && *p) {
        llg_vpi_slot_t* slot = decode_handle(current);
        vpiHandle next = NULL;
        if (*p == '[') {
            dyn_place_t place;
            if (slot->kind == SLOT_DYNELEM) {
                const llg_vpi_shape_t* shape = dyn_element_shape(slot);
                if (shape && shape->shape == LLG_VPI_SHAPE_PACKED)
                    unserved_select = 1;
                else
                    next = dyn_suffix_nested(slot, &p, &partial);
            } else if (dyn_place(slot, &place)) {
                if (place.shape->shape == LLG_VPI_SHAPE_CONTAINER)
                    next = dyn_first_suffix(&place, slot->kind == SLOT_PROPERTY ? (uint32_t)slot->aux : 0u,
                                            &p, &partial);
                else if (slot->kind == SLOT_PROPERTY && place.shape->shape == LLG_VPI_SHAPE_PACKED)
                    unserved_select = 1;
            }
        } else if (*p == '.') {
            const char* name = ++p;
            while (*p && *p != '.' && *p != '[') ++p;
            if (slot->kind == SLOT_DYNELEM) next = dyn_suffix_member(slot, name, (size_t)(p - name));
            if (!next && slot->kind == SLOT_PROPERTY) {
                /* A member of a structure property (decision A19-D89). */
                dyn_place_t place;
                if (dyn_place(slot, &place) && place.shape->shape == LLG_VPI_SHAPE_RECORD && place.storage)
                    next = dyn_record_member(&place, (uint32_t)slot->aux, name, (size_t)(p - name));
            }
            if (!next) {
                int32_t ok;
                void* object = dyn_class_target(decode_handle(current), &ok);
                if (ok && object) next = dyn_property_by_name(object, name, (size_t)(p - name));
            }
            if (!next) {
                /* A structure kept as one packed value has no member objects. */
                slot = decode_handle(current);
                dyn_place_t place;
                const llg_vpi_shape_t* shape = slot->kind == SLOT_DYNELEM ? dyn_element_shape(slot)
                                               : dyn_place(slot, &place)  ? place.shape
                                                                          : NULL;
                if (shape && shape->shape == LLG_VPI_SHAPE_PACKED &&
                    (shape->type == vpiStructVar || shape->type == vpiUnionVar))
                    unserved_member = 1;
            }
        }
        if (current != keep) vpi_release_handle(current);
        current = next;
    }
    if (current) {
        /* A container nested in an element is a dimension, not an object. */
        llg_vpi_slot_t* slot = decode_handle(current);
        const llg_vpi_shape_t* shape =
            slot && slot->kind == SLOT_DYNELEM && !dyn_is_member(slot) ? dyn_element_shape(slot) : NULL;
        if (shape && shape->shape == LLG_VPI_SHAPE_CONTAINER) {
            if (current != keep) vpi_release_handle(current);
            current = NULL;
            partial = 1;
        }
    }
    if (partial)
        vpi_error("LLG_VPI_INDEX",
                  "a name with fewer indices than its array has dimensions names no object "
                  "(decision A20-D23)");
    else if (unserved_select && !current)
        /* Raised after the walk released its handles, which clears errors. */
        vpi_error("LLG_VPI_UNSUPPORTED",
                  "llg serves no bit or part select of a dynamic element, member or property "
                  "(decision A19-D89)");
    else if (unserved_member && !current)
        vpi_error("LLG_VPI_UNSUPPORTED",
                  "llg serves no member of a structure or union kept as one packed value in a "
                  "class property or dynamic element (decision A19-D89)");
    return current;
}

/* vpi_handle_by_name of a static property by its full name (37.17 detail
 * 25: `top.Packet::Id`), or of an element or member after it, or of a
 * class method (`top.C::get`); NULL when none has that name. */
static LLG_OUTLYING vpiHandle dyn_static_by_name(const char* name) {
    for (uint32_t i = 0; g_dyn.tables && i < g_dyn.tables->static_count; ++i) {
        char* full = dyn_static_full_name(DYN_STATIC_FIELD + i);
        if (!full) return NULL;
        size_t length = strlen(full);
        int32_t match = strncmp(name, full, length) == 0 &&
                    (name[length] == '\0' || name[length] == '[' || name[length] == '.');
        free(full);
        if (!match) continue;
        vpiHandle property = make_static_property(DYN_STATIC_FIELD + i);
        return property ? dyn_walk_suffix(property, NULL, name + length, 0) : NULL;
    }
    /* A method by its full name (37.37 detail 5: `top.C::get`). A
     * parameterized class lists one definition per specialization under
     * one name (decision A19-D36): the first one answers. */
    for (uint32_t c = 0; c < dyn_class_count(); ++c) {
        const char* full = dyn_class(c)->full_name;
        size_t length = strlen(full);
        if (strncmp(name, full, length) != 0 || strncmp(name + length, "::", 2) != 0) continue;
        vpiHandle method =
            dyn_method_named(c, name + length + 2u, strlen(name + length + 2u));
        if (method) return method;
    }
    return NULL;
}

/* `suffix` after dynamic row `row` (dyn_walk_suffix). */
static vpiHandle dyn_by_suffix(uint32_t row, const char* suffix) {
    vpiHandle start = object_handle(row);
    return dyn_walk_suffix(start, start, suffix, 0);
}

/* vpiMember of the record automatic `place` (automatic `field` of its
 * frame): one element handle per member, in declaration order (37.24). */
static LLG_OUTLYING vpiHandle dyn_record_members(const dyn_place_t* place, uint32_t field) {
    dyn_list_t list = {0};
    for (uint32_t m = 0; m < place->shape->member_count && !list.failed; ++m) {
        dyn_step_t step;
        memset(&step, 0, sizeof(step));
        step.at = DYN_AT_MEMBER;
        step.element = m;
        dyn_list_push(&list, make_dyn_path(place, field, &step, 1u));
    }
    return dyn_list_iterator(vpiMember, &list);
}

/* `suffix` (starting with `.`) after the record automatic `place`: a
 * member, then an element or member below it; NULL when it names none. */
static LLG_OUTLYING vpiHandle dyn_record_suffix(const dyn_place_t* place, uint32_t field,
                                                const char* suffix) {
    const char* name = suffix + 1;
    size_t length = strcspn(name, ".[");
    vpiHandle handle = dyn_record_member(place, field, name, length);
    return handle ? dyn_walk_suffix(handle, NULL, name + length, 0) : NULL;
}

/* `suffix` (starting with `[`) after the container automatic `place`
 * (automatic `field` of its frame): its element, a nested element, a
 * member (decisions A19-D82, A19-D83). */
static LLG_OUTLYING vpiHandle dyn_auto_suffix(const dyn_place_t* place, uint32_t field, const char* suffix) {
    int32_t partial = 0;
    vpiHandle element = dyn_first_suffix(place, field, &suffix, &partial);
    if (!element) {
        if (partial)
            vpi_error("LLG_VPI_INDEX",
                      "a name with fewer indices than its array has dimensions names no object "
                      "(decision A20-D23)");
        return NULL;
    }
    return dyn_walk_suffix(element, NULL, suffix, 0);
}

/* ── callbacks ──────────────────────────────────────────────────────────── */

/* cbValueChange on a class variable: its value is opaque, so the callback
 * runs with a NULL value when the variable refers to another object or to
 * none (38.36.1). On a chandle variable it reports the new pointer as
 * vpi_get_value reads it (decision A19-D21). Both watch the variable's
 * change marker, which a handle store toggles once per check when the
 * value differs (T1-D11). Strings and arrays are excluded by the clause.
 * Returns -1 when `slot` is not a dynamic object. */
static int32_t dyn_bind_value_cb(struct llg_vpi_cb* cb, llg_vpi_slot_t* slot) {
    if (!dyn_slot(slot)) return -1;
    llg_vpi_model_object_t* object =
        slot->kind == SLOT_OBJECT ? row_object(slot->object) : NULL;
    const llg_vpi_shape_t* shape = object ? dyn_shape(object->shape - 1u) : NULL;
    int32_t handle = shape && shape->shape == LLG_VPI_SHAPE_CHANDLE;
    if (!shape || (shape->shape != LLG_VPI_SHAPE_CLASS && !handle) || !object->dependency) {
        vpi_error("LLG_VPI_CALLBACK",
                  "cbValueChange applies to class and chandle variables of a scope, not to "
                  "strings, arrays, class objects or their properties (38.36.1)");
        return 0;
    }
    int32_t format = cb->value.format;
    if (handle && format != vpiSuppressVal &&
        (format < vpiBinStrVal || format > vpiObjTypeVal || format == vpiStrengthVal)) {
        /* Table 38-5: vpiStrengthVal is for scalar objects only. */
        vpi_error("LLG_VPI_CALLBACK", "unsupported cbValueChange value format for a chandle");
        return 0;
    }
    llg_vpi_watch_t* watch = (llg_vpi_watch_t*)vpi_calloc(1, sizeof(*watch));
    watch->key = object->dependency;
    watch->cb = cb;
    watch->element = LLG_VPI_NO_ELEMENT;
    watch->event = handle ? LLG_VPI_WATCH_HANDLE : LLG_VPI_WATCH_OPAQUE;
    cb->watches = watch;
    cb->watch_total = 1;
    cb->target.object = slot->object;
    cb->target.element = LLG_VPI_NO_ELEMENT;
    cb->scope = object;
    cb->value_type = handle ? vpiChandleVar : vpiClassVar;
    return 1;
}

/* The shape marker a container publishes on each resize, or NULL. */
static sv4_t* dyn_container_shape_marker(const dyn_place_t* place) {
    const llg_vpi_shape_t* shape = place->shape;
    void* storage = place->storage;
    if (!storage || shape->inline_value) return NULL;
    switch (shape->array_type) {
        case vpiDynamicArray:
            return shape->values ? ((llg_dyn_value_array_t*)storage)->shape_dependency
                                 : ((llg_dyn_array_t*)storage)->shape_dependency;
        case vpiQueueArray:
            return shape->values ? ((llg_queue_value_array_t*)storage)->shape_dependency
                                 : ((llg_queue_t*)storage)->shape_dependency;
        case vpiAssocArray:
            return shape->values ? ((llg_assoc_value_t*)storage)->shape_dependency
                                 : ((llg_assoc_t*)storage)->shape_dependency;
        default: return NULL;
    }
}

/* cbSizeChange (decision A19-D91): on a dynamic array, queue or associative
 * array variable of a scope, with vpiSuppressVal as industry-standard
 * simulators accept it; the record watches the container's shape marker and
 * runs after a check that sees a new size (37.17 detail 14). A string var,
 * another value format, a class property, a fixed array of containers and a
 * nested container are refused, so that no accepted record stays silent. */
static int32_t dyn_bind_size_cb(struct llg_vpi_cb* cb, llg_vpi_slot_t* slot, const s_cb_data* data) {
    if (data->value && data->value->format != vpiSuppressVal) {
        vpi_error("LLG_VPI_CALLBACK",
                  "cbSizeChange takes vpiSuppressVal; read the new size with vpiSize "
                  "(decision A19-D91)");
        return 0;
    }
    dyn_place_t place;
    if (slot->kind != SLOT_OBJECT || !dyn_slot(slot) || !dyn_place(slot, &place) ||
        place.shape->shape != LLG_VPI_SHAPE_CONTAINER) {
        vpi_error("LLG_VPI_CALLBACK",
                  "cbSizeChange applies to dynamic array, queue and associative array "
                  "variables of a module, interface, program or package (37.17 detail 14, "
                  "decision A19-D91)");
        return 0;
    }
    sv4_t* marker = dyn_container_shape_marker(&place);
    if (!marker) {
        vpi_error("LLG_VPI_CALLBACK",
                  "cbSizeChange on a fixed array of containers is not supported by llg; "
                  "decision A19-D91");
        return 0;
    }
    llg_vpi_watch_t* watch = (llg_vpi_watch_t*)vpi_calloc(1, sizeof(*watch));
    watch->key = marker;
    watch->cb = cb;
    watch->element = LLG_VPI_NO_ELEMENT;
    watch->event = LLG_VPI_WATCH_SIZE;
    cb->watches = watch;
    cb->watch_total = 1;
    cb->target.object = slot->object;
    cb->target.element = LLG_VPI_NO_ELEMENT;
    cb->scope = row_object(slot->object);
    cb->value_type = place.shape->array_type;
    cb->last_size = dyn_container_size(&place);
    return 1;
}

/* Whether the container of a cbSizeChange record has a new size; records
 * it. */
static int32_t dyn_size_cb_changed(struct llg_vpi_cb* cb) {
    llg_vpi_model_object_t* object = row_object(cb->target.object);
    dyn_place_t place;
    memset(&place, 0, sizeof(place));
    place.shape = dyn_shape(object->shape - 1u);
    place.storage = object->storage;
    if (!place.shape) return 0;
    uint64_t size = dyn_container_size(&place);
    if (size == cb->last_size) return 0;
    cb->last_size = size;
    return 1;
}

/* The value of a cbValueChange record on a chandle variable: the variable
 * as it holds now, in the record's format. */
static LLG_OUTLYING void dyn_handle_cb_value(const struct llg_vpi_cb* cb, p_vpi_value output) {
    const llg_vpi_model_object_t* object = row_object(cb->target.object);
    dyn_read(dyn_shape(object->shape - 1u), object->storage, output);
}

/* cbCreateObj on a class typespec; cbReclaimObj on a class typespec or a
 * class object; cbEndOfObject on a class object (38.36.1). The record keeps
 * the class in target.object and a watched object's address in
 * target.element. */
static int32_t dyn_bind_object_cb(struct llg_vpi_cb* cb, const s_cb_data* data) {
    llg_vpi_slot_t* slot = checked_handle(data->obj);
    if (!slot) return 0;
    int32_t reason = data->reason;
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
    int32_t reason;
    uint32_t class_index;
    const void* object;
} dyn_cb_match_t;

/* A typespec record matches objects of exactly its class (decision
 * A19-D22); an object record matches its object. */
static int32_t matches_object_cb(const llg_vpi_cb_t* cb, const void* context) {
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
static void dyn_run_object_cbs(int32_t reason, void* object, uint32_t class_index) {
    dyn_cb_match_t match = { reason, class_index, object };
    llg_vpi_cb_t* buffer[LLG_VPI_DISPATCH_INLINE];
    size_t count;
    llg_vpi_cb_t** items = collect_cbs(g_vpi.cbs, matches_object_cb, &match, buffer, &count);
    if (count) {
        vpiHandle handle = make_object(object);
        if (handle) {
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
            llg_vpi_slot_t* slot = decode_handle(handle);
            if (slot) release_slot(slot, handle_index(handle));
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
    int32_t named = 0;
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
            entry->dying = 1;
            dyn_run_object_cbs(cbReclaimObj, objects[i], class_index);
            named |= entry->named;
        }
        dispatch_end();
    }
    if (named) {
        for (size_t i = g_vpi.static_count; i < g_vpi.slot_count; ++i) {
            llg_vpi_slot_t* slot = slot_at(i);
            int32_t bound = slot->kind == SLOT_CLASSOBJ || slot->kind == SLOT_BUILTINOBJ ||
                        slot->kind == SLOT_PROPERTY ||
                        slot->kind == SLOT_DYNELEM || slot->kind == SLOT_CLASSITEM ||
                        (slot->kind == SLOT_ITERATOR && slot->iter_kind == ITER_DYN);
            if (bound && slot->pointer && dyn_is_dying(slot->pointer))
                release_slot(slot, (uint32_t)i);
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
