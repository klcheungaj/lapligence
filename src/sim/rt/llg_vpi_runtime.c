// llg_vpi_runtime.c — VPI runtime structures (IEEE 1800-2009 37.32 UDP
// definitions, 37.54 vpiUse, 37.75 callback iteration, 37.76 the time queue,
// 37.79 iterator vpiUse, 38.23 vpi_iterate(vpiUserSystf, NULL)). Embedded
// right after llg_vpi_dynamic.c in the same translation unit: it uses the
// private slot, handle and catalog helpers of llg_vpi.c. Nothing here runs
// unless an application asks for one of these objects.

/* ── slots ──────────────────────────────────────────────────────────────── */

static int rts_slot(const llg_vpi_slot_t* slot) {
    return slot->kind == SLOT_TIMEQ || slot->kind == SLOT_UDPDEFN || slot->kind == SLOT_UDPIO ||
           slot->kind == SLOT_TABLEENTRY;
}

static int rts_type(const llg_vpi_slot_t* slot) {
    switch (slot->kind) {
        case SLOT_TIMEQ: return vpiTimeQueue;
        case SLOT_UDPDEFN: return vpiUdpDefn;
        case SLOT_UDPIO: return vpiIODecl;
        case SLOT_TABLEENTRY: return vpiTableEntry;
        default: return vpiUndefined;
    }
}

static int rts_identity(const llg_vpi_slot_t* slot, uint64_t identity[4]) {
    if (!rts_slot(slot)) return 0;
    identity[0] = 2u + slot->kind;
    identity[1] = (uint64_t)(uint32_t)slot->aux;
    identity[2] = (uint64_t)slot->left;
    identity[3] = 0;
    return 1;
}

static vpiHandle rts_make(llg_vpi_slot_kind_t kind, uint32_t aux, int64_t left) {
    uint32_t index = alloc_slot(kind);
    if (index == LLG_VPI_NONE) return NULL;
    g_vpi.slots[index].aux = (int32_t)aux;
    g_vpi.slots[index].left = left;
    return encode_handle(index);
}

/* A list iterator over `count` handles, or NULL with an error when the
 * list could not be built. */
static vpiHandle rts_list(int type, uintptr_t* items, uint32_t count, int failed) {
    if (failed) {
        free(items);
        vpi_error("LLG_VPI_NOMEM", "VPI iterator allocation failed");
        return NULL;
    }
    return make_list_iterator(type, items, count);
}

typedef struct {
    uintptr_t* items;
    uint32_t count;
    uint32_t capacity;
    int failed;
} rts_handles_t;

static void rts_push(rts_handles_t* list, vpiHandle handle) {
    if (list->failed || !handle) return;
    if (list->count == list->capacity) {
        uint32_t capacity = list->capacity ? list->capacity * 2u : 8u;
        uintptr_t* grown = (uintptr_t*)realloc(list->items, capacity * sizeof(*grown));
        if (!grown) {
            list->failed = 1;
            return;
        }
        list->items = grown;
        list->capacity = capacity;
    }
    list->items[list->count++] = (uintptr_t)handle;
}

/* ── UDP definitions (37.32) ────────────────────────────────────────────── */

static uint32_t rts_udp_count(void) { return g_vpi.code ? g_vpi.code->udp_count : 0u; }

static const llg_vpi_udp_t* rts_udp(int32_t index) {
    if (index < 0 || (uint32_t)index >= rts_udp_count()) return NULL;
    return &g_vpi.code->udps[index];
}

/* The definition a UDP instance row names by its definition name. */
static vpiHandle rts_udp_of(const llg_vpi_model_object_t* object) {
    for (uint32_t i = 0; object && object->definition_name && i < rts_udp_count(); ++i)
        if (strcmp(g_vpi.code->udps[i].name, object->definition_name) == 0)
            return rts_make(SLOT_UDPDEFN, i, 0);
    vpi_error("LLG_VPI_UNSUPPORTED",
              "UDP definitions are objects only in models generated with --vpi or --dpi-lib");
    return NULL;
}

/* The symbols of table entry `text`: one per input field (an edge `(vw)`
 * is one two-character symbol), then the state and output fields. */
static uint32_t rts_entry_symbols(const char* text, char symbols[][2], uint32_t capacity) {
    uint32_t count = 0;
    for (const char* c = text; *c; ++c) {
        if (*c == ':') continue;
        char first = *c, second = 0;
        if (*c == '(' && c[1] && c[2]) {
            first = c[1];
            second = c[2];
            c += 3; /* the closing parenthesis */
        }
        if (count < capacity) {
            symbols[count][0] = first;
            symbols[count][1] = second;
        }
        ++count;
    }
    return count;
}

/* 38.15: a table entry reads as its decompiled text or as its symbols,
 * two bytes each, four to an s_vpi_vecval: the first two in aval (most
 * significant byte first), the next two in bval (decision A19-D38). */
static void rts_entry_value(const llg_vpi_slot_t* slot, p_vpi_value output) {
    const llg_vpi_udp_t* udp = rts_udp(slot->aux);
    if (!udp || slot->left < 0 || (uint64_t)slot->left >= udp->entry_count) {
        vpi_error("LLG_VPI_HANDLE", "invalid UDP table entry");
        return;
    }
    const char* text = udp->entries[slot->left];
    if (output->format == vpiStringVal) {
        size_t length = strlen(text);
        char* buffer = text_buffer(length + 1u);
        if (!buffer) {
            vpi_error("LLG_VPI_NOMEM", "VPI value allocation failed");
            return;
        }
        memcpy(buffer, text, length + 1u);
        output->value.str = buffer;
        return;
    }
    if (output->format != vpiVectorVal) {
        vpi_error("LLG_VPI_VALUE",
                  "a UDP table entry reads only as vpiStringVal or vpiVectorVal (37.32 detail 1)");
        return;
    }
    char symbols[64][2];
    uint32_t count = rts_entry_symbols(text, symbols, 64u);
    if (count > 64u) count = 64u;
    size_t words = count ? (count - 1u) / 4u + 1u : 1u;
    if (words > g_vpi.value_vector_capacity) {
        s_vpi_vecval* grown =
            (s_vpi_vecval*)realloc(g_vpi.value_vector, words * sizeof(*grown));
        if (!grown) {
            vpi_error("LLG_VPI_NOMEM", "VPI vector allocation failed");
            return;
        }
        g_vpi.value_vector = grown;
        g_vpi.value_vector_capacity = words;
    }
    memset(g_vpi.value_vector, 0, words * sizeof(*g_vpi.value_vector));
    for (uint32_t i = 0; i < count; ++i) {
        s_vpi_vecval* word = &g_vpi.value_vector[i / 4u];
        PLI_UINT32 pair = ((PLI_UINT32)(unsigned char)symbols[i][0] << 8) |
                          (PLI_UINT32)(unsigned char)symbols[i][1];
        PLI_UINT32* half = (i % 4u) < 2u ? &word->aval : &word->bval;
        unsigned shift = (i % 2u) == 0 ? 16u : 0u;
        *half |= pair << shift;
    }
    output->value.vector = g_vpi.value_vector;
}

/* ── time queue (37.76) ─────────────────────────────────────────────────── */

/* The absolute time a pending time callback runs at. */
static int rts_cb_time(const llg_vpi_cb_t* cb, uint64_t* time) {
    if (cb->kind != CB_TIME || cb->removed) return 0;
    int ok;
    uint64_t ticks = ticks_from_time(&cb->time, cb->scope, &ok);
    if (!ok || ticks > UINT64_MAX - cb->registered_at) return 0;
    *time = cb->registered_at + ticks;
    return 1;
}

static vpiHandle rts_time_queues(void) {
    size_t count = 0;
    int current = 0;
    uint64_t* times = llg_rt_pending_times(&count, &current);
    if (!times && count) return NULL;
    rts_handles_t list = {0};
    /* 37.76 details 1 and 3: increasing time; the current slot only while
     * events precede its read-only synchronization. */
    if (current) rts_push(&list, rts_make(SLOT_TIMEQ, 0u, (int64_t)llg_time()));
    for (size_t i = 0; i < count; ++i)
        rts_push(&list, rts_make(SLOT_TIMEQ, 0u, (int64_t)times[i]));
    free(times);
    return rts_list(vpiTimeQueue, list.items, list.count, list.failed);
}

/* ── callback iteration (37.75) ─────────────────────────────────────────── */

/* The objects callbacks are placed on: prim terms, expressions, time
 * queues and statements (scopes included), the callback_owner class of
 * the catalog. */
static int rts_callback_owner(int object_type) {
    switch (object_type) {
        case vpiAssignStmt: case vpiAssignment: case vpiBegin: case vpiBitSelect:
        case vpiBitVar: case vpiBreak: case vpiByteVar: case vpiCase: case vpiChandleVar:
        case vpiClassDefn: case vpiClassObj: case vpiClassTypespec: case vpiClassVar:
        case vpiClockingBlock: case vpiConstant: case vpiContinue: case vpiDeassign:
        case vpiDelayControl: case vpiDisable: case vpiDisableFork: case vpiDoWhile:
        case vpiEnumNet: case vpiEnumVar: case vpiEventControl: case vpiEventStmt:
        case vpiExpectStmt: case vpiFor: case vpiForce: case vpiForeachStmt: case vpiForever:
        case vpiFork: case vpiFuncCall: case vpiFunction: case vpiGenScope: case vpiIf:
        case vpiIfElse: case vpiImmediateAssert: case vpiImmediateAssume:
        case vpiImmediateCover: case vpiIndexedPartSelect: case vpiIntVar:
        case vpiIntegerNet: case vpiIntegerVar: case vpiInterface: case vpiLetExpr:
        case vpiLongIntVar: case vpiMethodFuncCall: case vpiMethodTaskCall: case vpiModule:
        case vpiNamedBegin: case vpiNamedFork: case vpiNet: case vpiNetArray: case vpiNetBit:
        case vpiNullStmt: case vpiOperation: case vpiOrderedWait: case vpiPackage:
        case vpiPackedArrayNet: case vpiPackedArrayVar: case vpiParameter: case vpiPartSelect:
        case vpiPrimTerm: case vpiProgram: case vpiRealVar: case vpiRefObj: case vpiReg:
        case vpiRegArray: case vpiRegBit: case vpiRelease: case vpiRepeat: case vpiReturnStmt:
        case vpiShortIntVar: case vpiShortRealVar: case vpiSpecParam: case vpiStringVar:
        case vpiStructNet: case vpiStructVar: case vpiSysFuncCall: case vpiSysTaskCall:
        case vpiTask: case vpiTaskCall: case vpiTimeNet: case vpiTimeQueue: case vpiTimeVar:
        case vpiUnionVar: case vpiVarSelect: case vpiWait: case vpiWaitFork: case vpiWhile:
            return 1;
        default:
            return 0;
    }
}

/* Whether two handles name the same object, without raising errors. */
static int rts_same_object(vpiHandle a, vpiHandle b) {
    uint32_t ai, bi;
    llg_vpi_slot_t* first = decode_handle(a, &ai);
    llg_vpi_slot_t* second = decode_handle(b, &bi);
    if (!first || !second) return 0;
    int dynamic = dyn_compare(first, ai, second, bi);
    if (dynamic >= 0) return dynamic;
    uint64_t x[4], y[4];
    handle_identity(first, ai, x);
    handle_identity(second, bi, y);
    return memcmp(x, y, sizeof(x)) == 0;
}

/* 37.75: the live callbacks placed on `reference`, or with a NULL
 * reference those related to none of the owner objects (detail 2): time
 * callbacks belong to their time queue. */
static vpiHandle rts_callbacks(vpiHandle reference, const llg_vpi_slot_t* slot) {
    rts_handles_t list = {0};
    int queue = slot && slot->kind == SLOT_TIMEQ;
    for (llg_vpi_cb_t* cb = g_vpi.cbs; cb; cb = cb->next) {
        if (cb->removed) continue;
        int listed;
        if (!reference) {
            listed = cb->kind != CB_TIME && !cb->data.obj;
        } else if (queue) {
            uint64_t time;
            listed = rts_cb_time(cb, &time) && time == (uint64_t)slot->left;
        } else {
            listed = cb->kind != CB_TIME && cb->data.obj &&
                     rts_same_object(cb->data.obj, reference);
        }
        if (listed) rts_push(&list, encode_handle(cb->slot));
    }
    return rts_list(vpiCallback, list.items, list.count, list.failed);
}

/* ── vpiUse (37.54) ─────────────────────────────────────────────────────── */

/* The parent of every code row, built on the first vpiUse query. */
static int rts_code_parents(void) {
    if (g_vpi.code_parents) return 1;
    if (!g_vpi.code || !g_vpi.code->node_count) return 0;
    uint32_t count = g_vpi.code->node_count;
    uint32_t* parents = (uint32_t*)calloc(count, sizeof(*parents));
    if (!parents) return 0;
    for (uint32_t row = 0; row < count; ++row) {
        const llg_vpi_code_t* node = &g_vpi.code->nodes[row];
        if (node->flags & LLG_VPI_CODE_LINK) continue;
        for (uint32_t child = 0; child < node->count; ++child)
            if (node->first + child < count) parents[node->first + child] = row + 1u;
    }
    g_vpi.code_parents = parents;
    return 1;
}

/* The nearest statement that contains code row `row`, or LLG_VPI_NONE. */
static uint32_t rts_enclosing_statement(uint32_t row) {
    for (uint32_t parent = g_vpi.code_parents[row]; parent; parent = g_vpi.code_parents[row]) {
        row = parent - 1u;
        const llg_vpi_code_t* node = code_at(row);
        if (node && code_is_statement(node->type) && !(node->flags & LLG_VPI_CODE_HIDDEN))
            return row;
    }
    return LLG_VPI_NONE;
}

/* 37.54: the statements, continuous assignments, terminals and ports that
 * use a net, variable, parameter or select of one (decision A19-D39). A
 * select reports every use of its parent object that covers its bits. */
static vpiHandle rts_uses(const llg_vpi_slot_t* slot) {
    uint32_t row = slot->object;
    if (slot->kind == SLOT_CODE) {
        const llg_vpi_code_t* node = code_at(code_visible((uint32_t)slot->aux));
        row = node && node->object >= 0 ? (uint32_t)node->object : LLG_VPI_NONE;
    }
    if (row == LLG_VPI_NONE || !row_object(row)) return NULL;
    llg_vpi_link_items_t items;
    memset(&items, 0, sizeof(items));
    llg_vpi_span_t query;
    int one_bit = 0;
    int spanned = slot->kind != SLOT_CODE && slot_span(slot, &query, &one_bit);
    if (links_ready()) {
        for (size_t i = links_of(row); i < g_vpi.link_count && g_vpi.links[i].row == row; ++i) {
            const llg_vpi_link_t* link = &g_vpi.links[i];
            uint32_t from, to;
            if (spanned && !link_overlap(link, &query, &from, &to)) continue;
            /* A force is a statement: the code rows below report it. */
            if (link->kind == LINK_FORCE) continue;
            if (link->kind == LINK_PORT_HIGH || link->kind == LINK_CONT_ASSIGN)
                items_add(&items, SLOT_OBJECT, link->source, 0u);
            else if (link->kind == LINK_PRIM_TERM)
                items_add(&items, SLOT_PRIMTERM, link->source, link->aux);
            else if (link->kind == LINK_TERM)
                items_add(&items, SLOT_TERM, link->source, link->aux);
            else
                items_add(&items, SLOT_OBJECT, link->source, 0u);
        }
    }
    if (rts_code_parents()) {
        for (uint32_t code = 0; code < g_vpi.code->node_count; ++code) {
            const llg_vpi_code_t* node = &g_vpi.code->nodes[code];
            if (node->object < 0 || (uint32_t)node->object != row ||
                !(node->flags & LLG_VPI_CODE_ALIAS))
                continue;
            uint32_t statement = rts_enclosing_statement(code);
            if (statement != LLG_VPI_NONE) items_add(&items, SLOT_CODE, statement, 0u);
        }
    }
    if (items.count > 1) {
        qsort(items.items, items.count, sizeof(*items.items), compare_link_items);
        size_t kept = 1;
        for (size_t i = 1; i < items.count; ++i)
            if (compare_link_items(&items.items[kept - 1u], &items.items[i]) != 0)
                items.items[kept++] = items.items[i];
        items.count = kept;
    }
    return link_items_iterator(vpiUse, &items);
}

/* The simple expressions of 37.54. */
static int rts_simple_expr(int object_type) {
    switch (object_type) {
        case vpiBitSelect: case vpiBitVar: case vpiByteVar: case vpiChandleVar:
        case vpiClassVar: case vpiEnumNet: case vpiEnumVar: case vpiIntVar:
        case vpiIntegerNet: case vpiIntegerVar: case vpiLongIntVar: case vpiNet:
        case vpiNetArray: case vpiNetBit: case vpiPackedArrayNet: case vpiPackedArrayVar:
        case vpiParameter: case vpiRealVar: case vpiRefObj: case vpiReg: case vpiRegArray:
        case vpiRegBit: case vpiShortIntVar: case vpiShortRealVar: case vpiSpecParam:
        case vpiStringVar: case vpiStructNet: case vpiStructVar: case vpiTimeNet:
        case vpiTimeVar: case vpiUnionVar: case vpiVarSelect:
            return 1;
        default:
            return 0;
    }
}

/* ── dispatch ───────────────────────────────────────────────────────────── */

static PLI_INT64 rts_get(int property, llg_vpi_slot_t* slot, int* handled) {
    *handled = 1;
    const llg_vpi_udp_t* udp = rts_udp(slot->aux);
    switch (slot->kind) {
        case SLOT_UDPDEFN:
            switch (property) {
                case vpiSize: return udp->input_count;
                case vpiProtected: return 0;
                case vpiPrimType: return udp->sequential ? vpiSeqPrim : vpiCombPrim;
                case vpiLineNo: return udp->line;
                default: break;
            }
            break;
        case SLOT_UDPIO:
            switch (property) {
                case vpiDirection: return udp->directions[slot->left];
                case vpiSize:
                case vpiScalar: return 1;
                case vpiVector:
                case vpiSigned: return 0;
                case vpiLineNo: return udp->line;
                default: break;
            }
            break;
        case SLOT_TABLEENTRY:
            switch (property) {
                case vpiSize: {
                    char symbols[1][2];
                    return rts_entry_symbols(udp->entries[slot->left], symbols, 0u);
                }
                case vpiLineNo: return udp->entry_lines[slot->left];
                default: break;
            }
            break;
        default:
            break;
    }
    if (property == vpiType) {
        *handled = 0;
        return 0;
    }
    return unsupported_property();
}

static const char* rts_get_str(int property, llg_vpi_slot_t* slot, int* handled) {
    *handled = 1;
    const llg_vpi_udp_t* udp = rts_udp(slot->aux);
    switch (property) {
        case vpiDefName:
            if (slot->kind == SLOT_UDPDEFN) return udp->name;
            break;
        case vpiName:
            if (slot->kind == SLOT_UDPIO) return udp->ports[slot->left];
            break;
        case vpiFile:
            if (slot->kind != SLOT_TIMEQ) return udp->file;
            break;
        case vpiType:
            *handled = 0;
            return NULL;
        default:
            break;
    }
    vpi_error("LLG_VPI_PROPERTY", "this object has no such string property");
    return NULL;
}

static void rts_get_value(llg_vpi_slot_t* slot, p_vpi_value output) {
    if (slot->kind == SLOT_TABLEENTRY) {
        rts_entry_value(slot, output);
        return;
    }
    vpi_error("LLG_VPI_VALUE", "this object has no value");
}

/* vpi_handle for the runtime structures; *handled is 0 for a query the
 * other layers answer. */
static vpiHandle rts_handle(int type, vpiHandle reference, int* handled) {
    *handled = 0;
    if (!reference) return NULL;
    llg_vpi_slot_t* slot = decode_handle(reference, NULL);
    if (!slot) return NULL;
    if (type == vpiUse && slot->kind == SLOT_ITERATOR) {
        /* 37.79 details 1 and 2: the reference used to create the iterator,
         * NULL for a NULL reference. */
        *handled = 1;
        if (!slot->use) return NULL;
        if (!decode_handle((vpiHandle)slot->use, NULL)) {
            vpi_error("LLG_VPI_HANDLE", "the reference handle of this iterator was released");
            return NULL;
        }
        return (vpiHandle)slot->use;
    }
    if (type == vpiUdpDefn) {
        if (slot->kind == SLOT_UDPIO) {
            *handled = 1;
            return rts_make(SLOT_UDPDEFN, (uint32_t)slot->aux, 0);
        }
        const llg_vpi_model_object_t* object =
            slot->kind == SLOT_OBJECT ? row_object(slot->object) : NULL;
        if (object && object->type == vpiUdp) {
            *handled = 1;
            return rts_udp_of(object);
        }
    }
    if (type == vpiInitial && slot->kind == SLOT_UDPDEFN) {
        *handled = 1;
        vpi_error("LLG_VPI_UNSUPPORTED",
                  "the initial statement of a UDP is kept only as its value, not as an object");
        return NULL;
    }
    if (rts_slot(slot)) {
        *handled = 1;
        vpi_error("LLG_VPI_RELATION", "this object has no such relation");
    }
    return NULL;
}

static vpiHandle rts_iterate(int type, vpiHandle reference, int* handled) {
    *handled = 1;
    if (!reference) {
        if (type == vpiUserSystf) {
            /* 37.38 detail 6: every registration, in registration order. */
            rts_handles_t list = {0};
            for (int i = 0; i < g_vpi.registration_count; ++i)
                rts_push(&list, encode_handle(g_vpi.registrations[i]->slot));
            return rts_list(vpiUserSystf, list.items, list.count, list.failed);
        }
        if (type == vpiCallback) return rts_callbacks(NULL, NULL);
        if (type == vpiTimeQueue) return rts_time_queues();
        if (type == vpiUdpDefn) {
            rts_handles_t list = {0};
            for (uint32_t i = 0; i < rts_udp_count(); ++i)
                rts_push(&list, rts_make(SLOT_UDPDEFN, i, 0));
            return rts_list(vpiUdpDefn, list.items, list.count, list.failed);
        }
        *handled = 0;
        return NULL;
    }
    llg_vpi_slot_t* slot = decode_handle(reference, NULL);
    if (!slot) {
        *handled = 0;
        return NULL;
    }
    int object_type = handle_type(slot);
    if (type == vpiCallback && rts_callback_owner(object_type)) return rts_callbacks(reference, slot);
    if (type == vpiUse && rts_simple_expr(object_type)) return rts_uses(slot);
    if (slot->kind == SLOT_UDPDEFN) {
        const llg_vpi_udp_t* udp = rts_udp(slot->aux);
        rts_handles_t list = {0};
        if (type == vpiIODecl) {
            for (uint32_t i = 0; i < udp->port_count; ++i)
                rts_push(&list, rts_make(SLOT_UDPIO, (uint32_t)slot->aux, (int64_t)i));
            return rts_list(vpiIODecl, list.items, list.count, list.failed);
        }
        if (type == vpiTableEntry) {
            for (uint32_t i = 0; i < udp->entry_count; ++i)
                rts_push(&list, rts_make(SLOT_TABLEENTRY, (uint32_t)slot->aux, (int64_t)i));
            return rts_list(vpiTableEntry, list.items, list.count, list.failed);
        }
    }
    if (rts_slot(slot)) {
        vpi_error("LLG_VPI_RELATION", "this object has no such iteration");
        return NULL;
    }
    *handled = 0;
    return NULL;
}

static void rts_shutdown(void) {}
