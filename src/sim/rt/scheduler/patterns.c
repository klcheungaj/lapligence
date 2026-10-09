// ── Assignment-pattern formatting (`%p`/`%0p`, SV 21.2.1.7) ──────────────────
//
// A generated model describes each formatted type with immutable
// llg_pattern_type_t tables and names the storage form of the root value.
// The walker only reads: it never mutates, retains or frees its input, and
// every temporary it creates is released before it returns.
//
// Class handles print their properties as a named pattern, following the
// object's dynamic class. Each walk keeps the chain of objects being
// printed; a handle to an object already on that chain prints `(cycle)` and
// nesting deeper than LLG_PATTERN_MAX_DEPTH objects prints `(...)`, so cyclic
// graphs terminate. Only objects reachable from a live value are visited, so
// the walk never reaches a collected object; a reclaimed object kept by
// LLG_GC_VERIFY prints `(reclaimed)` instead of being read. Other handles
// print `null` or a fixed word (`chandle`, `event`, `interface`, `process`).
// Output stops at LLG_PATTERN_OUTPUT_LIMIT bytes with a warning.

static const llg_pattern_class_t* g_pattern_classes;
static uint32_t g_pattern_class_count;
static llg_pattern_object_fn g_pattern_object;
static llg_pattern_field_fn g_pattern_field;

void llg_pattern_set_classes(const llg_pattern_class_t* classes, uint32_t count,
                             llg_pattern_object_fn object,
                             llg_pattern_field_fn field) {
    g_pattern_classes = classes;
    g_pattern_class_count = count;
    g_pattern_object = object;
    g_pattern_field = field;
}

typedef struct {
    const llg_pattern_type_t* types;
    uint32_t type_count;
    char* data;
    size_t len;
    size_t cap;
    int truncated;
    int abbreviated;
    size_t depth;
    const void* chain[LLG_PATTERN_MAX_DEPTH];
} llg_pattern_out_t;

// Type `index` of the walked table; LLG_PATTERN_NO_TYPE (or an index outside
// the table) selects the storage's own generic form.
static const llg_pattern_type_t* llg_pattern_at(const llg_pattern_out_t* out,
                                                uint32_t index) {
    return index < out->type_count ? &out->types[index] : NULL;
}

static void llg_pattern_put(llg_pattern_out_t* out, const char* text, size_t n) {
    if (out->truncated) return;
    if (n > LLG_PATTERN_OUTPUT_LIMIT - out->len) {
        n = LLG_PATTERN_OUTPUT_LIMIT - out->len;
        out->truncated = 1;
    }
    if (n > out->cap - out->len) {
        size_t cap = out->cap ? out->cap : 64u;
        while (cap - out->len < n) cap = cap > LLG_PATTERN_OUTPUT_LIMIT / 2u
                                             ? LLG_PATTERN_OUTPUT_LIMIT
                                             : cap * 2u;
        char* grown = realloc(out->data, cap);
        if (!grown) llg_fatal_allocation("pattern text", cap, 1);
        out->data = grown;
        out->cap = cap;
    }
    if (n) memcpy(out->data + out->len, text, n);
    out->len += n;
}

static void llg_pattern_text(llg_pattern_out_t* out, const char* text) {
    llg_pattern_put(out, text, strlen(text));
}

// Element separator: `, ` in the full form and `,` in the `%0p` form.
static void llg_pattern_separator(llg_pattern_out_t* out) {
    llg_pattern_text(out, out->abbreviated ? "," : ", ");
}

static void llg_pattern_label(llg_pattern_out_t* out, const char* name) {
    if (out->abbreviated || !name) return;
    llg_pattern_text(out, name);
    llg_pattern_put(out, ":", 1);
}

static void llg_pattern_scalar(llg_pattern_out_t* out, sv4_t value) {
    char inline_text[160];
    size_t cap = (size_t)llg_sv4_width(value) + 72u;
    char* text = cap <= sizeof(inline_text)
                     ? inline_text
                     : llg_checked_malloc(cap, 1, "pattern scalar");
    size_t len = llg_format_pattern_packed(value, text, cap);
    llg_pattern_put(out, text, len);
    if (text != inline_text) free(text);
}

static int llg_pattern_enum_match(const llg_pattern_enum_t* member, sv4_t value) {
    int limbs = llg_sv4_nlimbs(llg_sv4_width(value));
    for (int i = 0; i < limbs; ++i) {
        uint64_t mask = llg_sv4_limb_mask(llg_sv4_width(value), i);
        if (((llg_sv4_word(value, i, LLG_SV4_BITS) ^ member->words[i]) & mask) ||
            ((llg_sv4_word(value, i, LLG_SV4_X) ^ member->words[limbs + i]) & mask) ||
            ((llg_sv4_word(value, i, LLG_SV4_Z) ^ member->words[2 * limbs + i]) & mask))
            return 0;
    }
    return 1;
}

// A packed leaf: an enum prints its member name when the value is one of
// its members and its base-type value otherwise.
static void llg_pattern_packed_leaf(llg_pattern_out_t* out,
                                    const llg_pattern_type_t* type, sv4_t value) {
    if (type && type->enums && llg_sv4_width(value) == type->flat_width) {
        for (size_t i = 0; i < type->count; ++i) {
            if (llg_pattern_enum_match(&type->enums[i], value)) {
                llg_pattern_text(out, type->enums[i].name);
                return;
            }
        }
    }
    llg_pattern_scalar(out, value);
}

static void llg_pattern_real(llg_pattern_out_t* out, double value) {
    // `%f` of the largest double needs 317 characters.
    char text[512];
    size_t len = llg_format_pattern_real(value, text, sizeof(text));
    llg_pattern_put(out, text, len);
}

static void llg_pattern_string(llg_pattern_out_t* out, const char* data,
                               size_t length) {
    char inline_text[256];
    if (length > (SIZE_MAX - 3u) / 4u)
        llg_fatal_allocation("pattern string", length, 4u);
    size_t cap = length * 4u + 3u;
    char* text = cap <= sizeof(inline_text)
                     ? inline_text
                     : llg_checked_malloc(cap, 1, "pattern string");
    size_t len = llg_format_pattern_string(data, length, text, cap);
    llg_pattern_put(out, text, len);
    if (text != inline_text) free(text);
}

static uint64_t llg_pattern_extent(int32_t left, int32_t right) {
    return (uint64_t)(left > right ? (int64_t)left - right : (int64_t)right - left) + 1u;
}

static void llg_pattern_value(llg_pattern_out_t* out,
                              const llg_pattern_type_t* type,
                              const llg_value_t* value);
static void llg_pattern_handle(llg_pattern_out_t* out,
                               const llg_pattern_type_t* type, void* handle);
static void llg_pattern_container(llg_pattern_out_t* out,
                                  const llg_pattern_type_t* type,
                                  uint8_t storage, const void* container);

// A real leaf of a flattened aggregate is its IEEE image (64 bits, or 32 bits
// for shortreal), as packed aggregate transport stores it.
static double llg_pattern_flat_real(sv4_t bits, int shortreal) {
    uint64_t word = llg_sv4_word(bits, 0, LLG_SV4_BITS);
    if (shortreal) {
        uint32_t image = (uint32_t)word;
        float result;
        memcpy(&result, &image, sizeof(result));
        return (double)result;
    }
    double result;
    memcpy(&result, &word, sizeof(result));
    return result;
}

// One element of a flattened value: bits [lsb + width - 1 : lsb] of `whole`,
// carrying the element's own signedness.
static sv4_t llg_pattern_slice(sv4_t whole, uint32_t lsb, uint32_t width,
                               int is_signed) {
    sv4_t slice = sv4_part_select(whole, (int64_t)lsb + width - 1, (int64_t)lsb);
    if (!is_signed) return slice;
    sv4_t result = sv4_resize(slice, width, 1);
    sv4_destroy(&slice);
    return result;
}

static void llg_pattern_flat(llg_pattern_out_t* out,
                             const llg_pattern_type_t* type, sv4_t whole,
                             uint32_t lsb);

static void llg_pattern_flat_array(llg_pattern_out_t* out,
                                   const llg_pattern_type_t* type, size_t dim,
                                   sv4_t whole, uint32_t msb) {
    uint64_t extent = llg_pattern_extent(type->bounds[2 * dim], type->bounds[2 * dim + 1]);
    uint64_t stride = llg_pattern_at(out, type->element)->flat_width;
    for (size_t inner = dim + 1; inner < type->count; ++inner)
        stride *= llg_pattern_extent(type->bounds[2 * inner], type->bounds[2 * inner + 1]);
    llg_pattern_put(out, "'{", 2);
    for (uint64_t i = 0; i < extent && !out->truncated; ++i) {
        if (i) llg_pattern_separator(out);
        uint32_t element_msb = (uint32_t)(msb - i * stride);
        if (dim + 1 < type->count)
            llg_pattern_flat_array(out, type, dim + 1, whole, element_msb);
        else
            llg_pattern_flat(out, llg_pattern_at(out, type->element), whole,
                             element_msb - (uint32_t)stride);
    }
    llg_pattern_put(out, "}", 1);
}

// Walk a value stored as packed bits: the first member or element occupies
// the most significant bits, as in an aggregate's packed transport.
static void llg_pattern_flat(llg_pattern_out_t* out,
                             const llg_pattern_type_t* type, sv4_t whole,
                             uint32_t lsb) {
    if (!type) {
        llg_pattern_scalar(out, whole);
        return;
    }
    switch (type->kind) {
    case LLG_PATTERN_PACKED_STRUCT:
    case LLG_PATTERN_STRUCT: {
        uint32_t msb = lsb + type->flat_width;
        llg_pattern_put(out, "'{", 2);
        for (size_t i = 0; i < type->count && !out->truncated; ++i) {
            const llg_pattern_member_t* member = &type->members[i];
            if (i) llg_pattern_separator(out);
            llg_pattern_label(out, member->name);
            const llg_pattern_type_t* member_type = llg_pattern_at(out, member->type);
            msb -= member_type->flat_width;
            llg_pattern_flat(out, member_type, whole, msb);
        }
        llg_pattern_put(out, "}", 1);
        return;
    }
    case LLG_PATTERN_UNION:
        // Only the first declared member is printed (SV 21.2.1.7); it is
        // aligned to the least significant bits of the overlay.
        if (type->count) {
            llg_pattern_put(out, "'{", 2);
            llg_pattern_label(out, type->members[0].name);
            llg_pattern_flat(out, llg_pattern_at(out, type->members[0].type), whole, lsb);
            llg_pattern_put(out, "}", 1);
        }
        return;
    case LLG_PATTERN_FIXED_ARRAY:
        llg_pattern_flat_array(out, type, 0, whole, lsb + type->flat_width);
        return;
    case LLG_PATTERN_REAL: {
        sv4_t bits = llg_pattern_slice(whole, lsb, type->flat_width, 0);
        llg_pattern_real(out, llg_pattern_flat_real(bits, type->shortreal));
        sv4_destroy(&bits);
        return;
    }
    default: {
        if (lsb == 0 && type->flat_width == llg_sv4_width(whole) &&
            (int)llg_sv4_signed(whole) == (int)type->signed_flag) {
            llg_pattern_packed_leaf(out, type, whole);
            return;
        }
        sv4_t element = llg_pattern_slice(whole, lsb, type->flat_width, type->signed_flag);
        llg_pattern_packed_leaf(out, type, element);
        sv4_destroy(&element);
        return;
    }
    }
}

static const llg_pattern_type_t* llg_pattern_member_type(
    const llg_pattern_out_t* out, const llg_pattern_type_t* type, size_t index) {
    if (!type) return NULL;
    if ((type->kind == LLG_PATTERN_STRUCT || type->kind == LLG_PATTERN_UNION) &&
        index < type->count)
        return llg_pattern_at(out, type->members[index].type);
    if (type->kind == LLG_PATTERN_FIXED_ARRAY || type->kind == LLG_PATTERN_QUEUE ||
        type->kind == LLG_PATTERN_DYNAMIC || type->kind == LLG_PATTERN_ASSOC)
        return llg_pattern_at(out, type->element);
    return NULL;
}

// Items of a fixed array stored row-major in one item list.
static void llg_pattern_value_items(llg_pattern_out_t* out,
                                    const llg_pattern_type_t* type, size_t dim,
                                    const llg_value_t* items, size_t count) {
    size_t dims = type && type->kind == LLG_PATTERN_FIXED_ARRAY ? type->count : 1u;
    uint64_t extent = type && type->kind == LLG_PATTERN_FIXED_ARRAY
                          ? llg_pattern_extent(type->bounds[2 * dim], type->bounds[2 * dim + 1])
                          : count;
    size_t stride = extent ? count / (size_t)extent : 0;
    llg_pattern_put(out, "'{", 2);
    for (uint64_t i = 0; i < extent && !out->truncated; ++i) {
        if (i) llg_pattern_separator(out);
        if (dim + 1 < dims)
            llg_pattern_value_items(out, type, dim + 1, items + i * stride, stride);
        else
            llg_pattern_value(out, type ? llg_pattern_at(out, type->element) : NULL,
                              &items[i]);
    }
    llg_pattern_put(out, "}", 1);
}

static void llg_pattern_value(llg_pattern_out_t* out,
                              const llg_pattern_type_t* type,
                              const llg_value_t* value) {
    if (!value || !value->desc) {
        llg_pattern_text(out, "null");
        return;
    }
    const llg_value_desc_t* desc = value->desc;
    switch (desc->kind) {
    case LLG_VALUE_PACKED:
        if (type && type->kind != LLG_PATTERN_PACKED &&
            llg_sv4_width(value->value.packed) == type->flat_width)
            llg_pattern_flat(out, type, value->value.packed, 0);
        else
            llg_pattern_packed_leaf(out, type, value->value.packed);
        return;
    case LLG_VALUE_REAL:
        llg_pattern_real(out, value->value.real);
        return;
    case LLG_VALUE_STRING:
        llg_pattern_string(out, value->value.string.data, value->value.string.len);
        return;
    case LLG_VALUE_AGGREGATE: {
        int named = type && (type->kind == LLG_PATTERN_STRUCT ||
                             type->kind == LLG_PATTERN_UNION) &&
                    type->count == desc->member_count;
        if (named && type->kind == LLG_PATTERN_UNION) {
            llg_pattern_put(out, "'{", 2);
            if (desc->item_count) {
                llg_pattern_label(out, type->members[0].name);
                llg_pattern_value(out, llg_pattern_at(out, type->members[0].type),
                                  &value->value.items[0]);
            }
            llg_pattern_put(out, "}", 1);
            return;
        }
        llg_pattern_put(out, "'{", 2);
        for (size_t i = 0; i < desc->item_count && !out->truncated; ++i) {
            if (i) llg_pattern_separator(out);
            if (named) llg_pattern_label(out, type->members[i].name);
            llg_pattern_value(out, named ? llg_pattern_at(out, type->members[i].type) : NULL,
                              &value->value.items[i]);
        }
        llg_pattern_put(out, "}", 1);
        return;
    }
    case LLG_VALUE_FIXED_ARRAY: {
        const llg_pattern_type_t* array =
            type && type->kind == LLG_PATTERN_FIXED_ARRAY ? type : NULL;
        size_t total = 1;
        for (size_t d = 0; array && d < array->count; ++d)
            total *= (size_t)llg_pattern_extent(array->bounds[2 * d], array->bounds[2 * d + 1]);
        if (!array || total != desc->item_count) {
            llg_pattern_value_items(out, NULL, 0, value->value.items, desc->item_count);
            return;
        }
        llg_pattern_value_items(out, array, 0, value->value.items, desc->item_count);
        return;
    }
    case LLG_VALUE_CONTAINER:
        llg_pattern_container(out, type, LLG_PATTERN_FROM_NESTED_CONTAINER,
                              value->value.container);
        return;
    case LLG_VALUE_EVENT:
        llg_pattern_text(out, value->value.handle ? "event" : "null");
        return;
    case LLG_VALUE_PROCESS:
        llg_pattern_text(out, value->value.handle ? "process" : "null");
        return;
    default:
        llg_pattern_handle(out, type, value->value.handle);
        return;
    }
}

static void llg_pattern_assoc_key(llg_pattern_out_t* out, uint8_t key_kind,
                                  const sv4_t* integral, const unsigned char* text,
                                  size_t length) {
    if (key_kind == LLG_ASSOC_STRING)
        llg_pattern_string(out, (const char*)text, length);
    else
        llg_pattern_scalar(out, *integral);
}

static void llg_pattern_container(llg_pattern_out_t* out,
                                  const llg_pattern_type_t* type,
                                  uint8_t storage, const void* container) {
    const llg_pattern_type_t* element = llg_pattern_member_type(out, type, 0);
    int assoc = type && type->kind == LLG_PATTERN_ASSOC;
    int queue = type && type->kind == LLG_PATTERN_QUEUE;
    if (container && storage != LLG_PATTERN_FROM_PACKED_CONTAINER && !assoc &&
        type && type->kind == LLG_PATTERN_FIXED_ARRAY) {
        // A fixed array of native elements uses dynamic-array storage.
        const llg_dyn_value_array_t* array = container;
        llg_pattern_value_items(out, type, 0, array->data, array->size);
        return;
    }
    llg_pattern_put(out, "'{", 2);
    if (!container) {
        llg_pattern_put(out, "}", 1);
        return;
    }
    if (storage == LLG_PATTERN_FROM_PACKED_CONTAINER) {
        if (assoc) {
            const llg_assoc_t* array = container;
            for (size_t i = 0; i < array->size && !out->truncated; ++i) {
                const llg_assoc_entry_t* entry = &array->entries[i];
                if (i) llg_pattern_separator(out);
                llg_pattern_assoc_key(out, array->key_kind, &entry->integral_key,
                                      entry->string_key, entry->string_length);
                llg_pattern_put(out, ":", 1);
                llg_pattern_flat(out, element, entry->value, 0);
            }
        } else {
            const sv4_t* data = queue ? ((const llg_queue_t*)container)->data
                                      : ((const llg_dyn_array_t*)container)->data;
            size_t size = queue ? ((const llg_queue_t*)container)->size
                                : ((const llg_dyn_array_t*)container)->size;
            for (size_t i = 0; i < size && !out->truncated; ++i) {
                if (i) llg_pattern_separator(out);
                llg_pattern_flat(out, element, data[i], 0);
            }
        }
    } else if (assoc) {
        const llg_assoc_value_t* array = container;
        for (size_t i = 0; i < array->size && !out->truncated; ++i) {
            const llg_assoc_value_entry_t* entry = &array->entries[i];
            if (i) llg_pattern_separator(out);
            llg_pattern_assoc_key(out, array->key_kind, &entry->integral_key,
                                  entry->string_key, entry->string_length);
            llg_pattern_put(out, ":", 1);
            llg_pattern_value(out, element, &entry->value);
        }
    } else {
        // Value queues keep their own storage; value dynamic arrays and the
        // containers nested in a value (always dynamic-array storage) keep
        // their elements in order.
        const llg_value_t* data;
        size_t size;
        if (queue && storage == LLG_PATTERN_FROM_VALUE_CONTAINER) {
            data = ((const llg_queue_value_array_t*)container)->data;
            size = ((const llg_queue_value_array_t*)container)->size;
        } else {
            data = ((const llg_dyn_value_array_t*)container)->data;
            size = ((const llg_dyn_value_array_t*)container)->size;
        }
        for (size_t i = 0; i < size && !out->truncated; ++i) {
            if (i) llg_pattern_separator(out);
            llg_pattern_value(out, element, &data[i]);
        }
    }
    llg_pattern_put(out, "}", 1);
}

static void llg_pattern_field(llg_pattern_out_t* out,
                              const llg_pattern_field_t* field,
                              const void* storage) {
    switch (field->storage) {
    case LLG_PATTERN_FROM_PACKED:
        llg_pattern_flat(out, llg_pattern_at(out, field->type), *(const sv4_t*)storage, 0);
        return;
    case LLG_PATTERN_FROM_REAL:
        llg_pattern_real(out, *(const double*)storage);
        return;
    case LLG_PATTERN_FROM_STRING: {
        const llg_string_t* text = storage;
        llg_pattern_string(out, text->data, text->len);
        return;
    }
    case LLG_PATTERN_FROM_HANDLE:
        llg_pattern_handle(out, llg_pattern_at(out, field->type), *(void* const*)storage);
        return;
    case LLG_PATTERN_FROM_VALUE:
        llg_pattern_value(out, llg_pattern_at(out, field->type), storage);
        return;
    default:
        llg_pattern_container(out, llg_pattern_at(out, field->type), field->storage, storage);
        return;
    }
}

static void llg_pattern_object(llg_pattern_out_t* out, void* handle) {
    uint32_t class_id = 0;
    size_t count = 0;
    if (!g_pattern_object || !g_pattern_object(handle, &class_id, &count) ||
        class_id >= g_pattern_class_count) {
        llg_pattern_text(out, "(reclaimed)");
        return;
    }
    for (size_t i = 0; i < out->depth; ++i) {
        if (out->chain[i] == handle) {
            llg_pattern_text(out, "(cycle)");
            return;
        }
    }
    if (out->depth == LLG_PATTERN_MAX_DEPTH) {
        llg_pattern_text(out, "(...)");
        return;
    }
    const llg_pattern_class_t* layout = &g_pattern_classes[class_id];
    if (layout->count < count) count = layout->count;
    out->chain[out->depth++] = handle;
    llg_pattern_put(out, "'{", 2);
    for (size_t i = 0; i < count && !out->truncated; ++i) {
        if (i) llg_pattern_separator(out);
        llg_pattern_label(out, layout->fields[i].name);
        llg_pattern_field(out, &layout->fields[i], g_pattern_field(handle, i));
    }
    llg_pattern_put(out, "}", 1);
    out->depth--;
}

static void llg_pattern_handle(llg_pattern_out_t* out,
                               const llg_pattern_type_t* type, void* handle) {
    if (!handle) {
        llg_pattern_text(out, "null");
        return;
    }
    switch (type ? type->kind : LLG_PATTERN_CHANDLE) {
    case LLG_PATTERN_CLASS:
        llg_pattern_object(out, handle);
        return;
    case LLG_PATTERN_EVENT:
        llg_pattern_text(out, "event");
        return;
    case LLG_PATTERN_VIRTUAL_INTERFACE:
        llg_pattern_text(out, "interface");
        return;
    case LLG_PATTERN_PROCESS:
        llg_pattern_text(out, "process");
        return;
    default:
        llg_pattern_text(out, "chandle");
        return;
    }
}

llg_string_t llg_pattern_format(const llg_pattern_type_t* types,
                                uint32_t type_count, uint32_t type,
                                uint8_t storage, const void* source,
                                int abbreviated) {
    llg_pattern_out_t out;
    memset(&out, 0, sizeof(out));
    out.types = types;
    out.type_count = type_count;
    out.abbreviated = abbreviated;
    llg_pattern_field_t root = {NULL, type, storage};
    llg_pattern_field(&out, &root, source);
    if (out.truncated)
        fprintf(stderr,
                "llg: warning: %%p output truncated at %u characters\n",
                (unsigned)LLG_PATTERN_OUTPUT_LIMIT);
    llg_string_t result = llg_string_bytes(out.data, out.len);
    free(out.data);
    return result;
}
