// llg_vpi_runtime.c — VPI runtime structures (IEEE 1800-2009 37.32 UDP
// definitions, 37.54 vpiUse, 37.75 callback iteration, 37.76 the time queue,
// 37.79 iterator vpiUse, 38.23 vpi_iterate(vpiUserSystf, NULL)) and 37.39
// frames and 37.40 threads with their 38.36.1 callbacks. Embedded
// right after llg_vpi_dynamic.c in the same translation unit: it uses the
// private slot, handle and catalog helpers of llg_vpi.c. Nothing here runs
// unless an application asks for one of these objects.

/* ── slots ──────────────────────────────────────────────────────────────── */

static int32_t rts_slot(const llg_vpi_slot_t* slot) {
    return slot->kind == SLOT_TIMEQ || slot->kind == SLOT_UDPDEFN || slot->kind == SLOT_UDPIO ||
           slot->kind == SLOT_TABLEENTRY || slot->kind == SLOT_UDPINIT || slot->kind == SLOT_THREAD ||
           slot->kind == SLOT_FRAME || slot->kind == SLOT_AUTO;
}

static int32_t rts_auto_type(const llg_vpi_slot_t* slot);

static int32_t rts_type(const llg_vpi_slot_t* slot) {
    switch (slot->kind) {
        case SLOT_TIMEQ: return vpiTimeQueue;
        case SLOT_UDPDEFN: return vpiUdpDefn;
        case SLOT_UDPIO: return vpiIODecl;
        case SLOT_TABLEENTRY: return vpiTableEntry;
        case SLOT_UDPINIT:
            return slot->left == 0 ? vpiInitial : slot->left == 1 ? vpiAssignment : vpiConstant;
        case SLOT_THREAD: return vpiThread;
        case SLOT_FRAME: return vpiFrame;
        case SLOT_AUTO: return rts_auto_type(slot);
        default: return vpiUndefined;
    }
}

static int32_t rts_identity(const llg_vpi_slot_t* slot, uint64_t identity[4]) {
    if (!rts_slot(slot)) return 0;
    identity[0] = 2u + slot->kind;
    identity[1] = (uint64_t)(uint32_t)slot->aux;
    identity[2] = (uint64_t)slot->left;
    identity[3] = slot->kind == SLOT_FRAME || slot->kind == SLOT_AUTO ? slot->element : 0u;
    /* An automatic: its index, and the element and member it names. */
    if (slot->kind == SLOT_AUTO) {
        identity[0] += ((uint64_t)slot->right << 8) + ((uint64_t)slot->count << 32);
        identity[1] |= (uint64_t)slot->lsb << 32;
        /* An element of an array member; frame serials stay below 2^40. */
        identity[3] ^= (uint64_t)slot->span << 40;
    }
    return 1;
}

static vpiHandle rts_make(llg_vpi_slot_kind_t kind, uint32_t aux, int64_t left) {
    uint32_t index = alloc_slot(kind);
    if (index == LLG_VPI_NONE) return NULL;
    slot_at(index)->aux = (int32_t)aux;
    slot_at(index)->left = left;
    return encode_handle(index);
}

/* A list iterator over `count` handles, or NULL with an error when the
 * list could not be built. */
static vpiHandle rts_list(int32_t type, uintptr_t* items, uint32_t count, int32_t failed) {
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
    int32_t failed;
} rts_handles_t;

static void rts_push(rts_handles_t* list, vpiHandle handle) {
    if (list->failed || !handle) return;
    if (list->count == list->capacity) {
        uint32_t capacity = list->capacity ? list->capacity * 2u : 8u;
        uintptr_t* grown = (uintptr_t*)vpi_realloc(list->items, capacity * sizeof(*grown));
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
            (s_vpi_vecval*)vpi_realloc(g_vpi.value_vector, words * sizeof(*grown));
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
        PLI_UINT32 pair = ((PLI_UINT32)(uint8_t)symbols[i][0] << 8) |
                          (PLI_UINT32)(uint8_t)symbols[i][1];
        PLI_UINT32* half = (i % 4u) < 2u ? &word->aval : &word->bval;
        uint32_t shift = (i % 2u) == 0 ? 16u : 0u;
        *half |= pair << shift;
    }
    output->value.vector = g_vpi.value_vector;
}

/* ── time queue (37.76) ─────────────────────────────────────────────────── */

/* The absolute time a pending time callback runs at. */
static int32_t rts_cb_time(const llg_vpi_cb_t* cb, uint64_t* time) {
    if (cb->kind != CB_TIME || cb->removed) return 0;
    int32_t ok;
    uint64_t ticks = ticks_from_time(&cb->time, cb->scope, &ok);
    if (!ok || ticks > UINT64_MAX - cb->registered_at) return 0;
    *time = cb->registered_at + ticks;
    return 1;
}

static vpiHandle rts_time_queues(void) {
    size_t count = 0;
    int32_t current = 0;
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
static int32_t rts_callback_owner(int32_t object_type) {
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
static int32_t rts_same_object(vpiHandle a, vpiHandle b) {
    uint32_t ai = LLG_VPI_NONE, bi = LLG_VPI_NONE;
    llg_vpi_slot_t* first = decode_handle(a, &ai);
    llg_vpi_slot_t* second = decode_handle(b, &bi);
    if (!first || !second) return 0;
    int32_t dynamic = dyn_compare(first, ai, second, bi);
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
    int32_t queue = slot && slot->kind == SLOT_TIMEQ;
    for (llg_vpi_cb_t* cb = g_vpi.cbs; cb; cb = cb->next) {
        if (cb->removed) continue;
        int32_t listed;
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
static int32_t rts_code_parents(void) {
    if (g_vpi.code_parents) return 1;
    (void)code_nodes();
    if (!g_vpi.code || !g_vpi.code->node_count) return 0;
    uint32_t count = g_vpi.code->node_count;
    uint32_t* parents = (uint32_t*)vpi_calloc(count, sizeof(*parents));
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

/* The statement a frame that made call row `row` is running (decision
 * A19-D75): the statement holding the call that sits directly in a
 * sequential or parallel block or is the body of the task, function or
 * process, as `if (c) t();` is for its call `t()`. A call that is itself a
 * statement (a task call, or a function call written as one) in a block is
 * that statement. */
static uint32_t rts_call_statement(uint32_t row) {
    const llg_vpi_code_t* node = code_at(row);
    if (!node || !rts_code_parents()) return LLG_VPI_NONE;
    uint32_t statement =
        code_is_statement(node->type) || node->role == vpiStmt ? row : LLG_VPI_NONE;
    for (uint32_t parent = g_vpi.code_parents[row]; parent; parent = g_vpi.code_parents[row]) {
        row = parent - 1u;
        node = code_at(row);
        if (!node) break;
        if (node->type == vpiBegin || node->type == vpiNamedBegin || node->type == vpiFork ||
            node->type == vpiNamedFork)
            break;
        if (code_is_statement(node->type) && !(node->flags & LLG_VPI_CODE_HIDDEN))
            statement = row;
    }
    return statement;
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
    int32_t one_bit = 0;
    int32_t spanned = slot->kind != SLOT_CODE && slot_span(slot, &query, &one_bit);
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
static int32_t rts_simple_expr(int32_t object_type) {
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

/* ── threads and frames (37.39, 37.40, 38.36.1) ─────────────────────────── */

/* A thread is a live procedural process, named by (registry slot, process
 * identity); a frame is one task or function activation on the frame stack
 * the bridge keeps per process (llg_vpi_frame_enter/exit, emitted only in
 * catalog models). Handles copy those names and are checked at every use,
 * so a handle to an ended thread or frame fails with an error and nothing
 * points into scheduler or model memory (ADV-019-A02). */

int32_t llg_vpi_frames_on;
uint32_t llg_vpi_next_origin;

#define RTS_ROW_UNRESOLVED (LLG_VPI_NONE - 1u)

/* One record per framed call while an application is loaded: kept at 32
 * bytes in a 64-byte aligned array, so no record straddles a cache line.
 * A 48-byte record cost deep recursion about 4% (task recursion of depth
 * 200; 24 and 40 bytes straddle lines too). The automatics table lives in
 * the activation's address array (LLG_VPI_AUTOS_TABLE), not here. */
typedef struct {
    uint64_t serial;   /* unique per run; the frame handle's name */
    const char* name;  /* the subprogram's full name (generated literal) */
    void** slots;      /* the activation's address array, or NULL */
    uint32_t row;      /* its task or function row, resolved on demand */
    uint32_t origin;   /* the call that activated it: code row + 1, 0 for none */
} rts_frame_t;

#define RTS_FRAME_ALIGN 64u

typedef struct {
    uint64_t identity; /* process identity the frames belong to */
    rts_frame_t* frames; /* RTS_FRAME_ALIGN aligned inside `block` */
    void* block;
    uint32_t depth;
    uint32_t capacity;
} rts_stack_t;

static struct {
    rts_stack_t* stacks; /* [registry slot + 1]; [0] outside any process */
    uint32_t stack_count;
    uint64_t next_serial;
    uint32_t start_thread_cbs; /* live cbStartOfThread records */
    uint32_t start_frame_cbs;  /* live cbStartOfFrame records */
    uint32_t end_frame_cbs;    /* live cbEndOfFrame records */
    int32_t hooked;
    /* The thread whose cbEndOfThread callbacks run: its handles stay
     * valid until they return. */
    int32_t dying;
    int32_t dying_slot;
    uint64_t dying_identity;
} rts;

static void rts_thread_event(int32_t slot, uint64_t identity, int32_t event);

static void rts_enable(void) {
    if (!g_vpi.objects || rts.hooked) return;
    rts.hooked = 1;
    llg_vpi_frames_on = 1;
    llg_rt_set_thread_hook(rts_thread_event);
}

static rts_stack_t* rts_stack(int32_t slot, uint64_t identity, int32_t create) {
    if (slot < -1) return NULL;
    uint32_t index = (uint32_t)(slot + 1);
    if (index >= rts.stack_count) {
        if (!create) return NULL;
        uint32_t count = rts.stack_count ? rts.stack_count : 16u;
        while (count <= index) count *= 2u;
        rts_stack_t* grown = (rts_stack_t*)vpi_realloc(rts.stacks, count * sizeof(*grown));
        if (!grown) return NULL;
        memset(grown + rts.stack_count, 0, (count - rts.stack_count) * sizeof(*grown));
        rts.stacks = grown;
        rts.stack_count = count;
    }
    rts_stack_t* stack = &rts.stacks[index];
    if (stack->identity != identity) {
        /* Frames of an ended process that never returned. */
        if (!create) return NULL;
        stack->identity = identity;
        stack->depth = 0;
    }
    return stack;
}

static int32_t rts_thread_valid(int32_t slot, uint64_t identity) {
    if (rts.dying && slot == rts.dying_slot && identity == rts.dying_identity) return 1;
    return llg_rt_thread_live(slot, identity) == 2;
}

/* The frame named by (slot, identity, serial) and its depth, or NULL once
 * that frame ended. */
static rts_frame_t* rts_frame_find(int32_t slot, uint64_t identity, uint64_t serial,
                                   uint32_t* depth) {
    if (slot >= 0 && !(rts.dying && slot == rts.dying_slot && identity == rts.dying_identity) &&
        !llg_rt_thread_live(slot, identity))
        return NULL;
    rts_stack_t* stack = rts_stack(slot, identity, 0);
    if (!stack) return NULL;
    for (uint32_t i = stack->depth; i-- > 0;) {
        if (stack->frames[i].serial == serial) {
            if (depth) *depth = i;
            return &stack->frames[i];
        }
    }
    return NULL;
}

static uint32_t rts_frame_row(rts_frame_t* frame) {
    if (frame->row == RTS_ROW_UNRESOLVED) {
        /* Resolved once the catalog is indexed (a registration alone does
         * not index it). */
        if (!g_vpi.indexed) return LLG_VPI_NONE;
        uint32_t row = find_row(frame->name);
        const llg_vpi_model_object_t* object = row_object(row);
        frame->row = object && (object->type == vpiTask || object->type == vpiFunction)
                         ? row
                         : LLG_VPI_NONE;
    }
    return frame->row;
}

/* A frame's automatics table (the activation's address array holds it)
 * and its row count (up to the terminating row with no name). */
static const llg_vpi_auto_t* rts_auto_table(const rts_frame_t* frame) {
    return frame->slots ? (const llg_vpi_auto_t*)frame->slots[LLG_VPI_AUTOS_TABLE] : NULL;
}

static uint32_t rts_auto_count(const rts_frame_t* frame) {
    const llg_vpi_auto_t* table = rts_auto_table(frame);
    uint32_t count = 0;
    while (table && table[count].name) ++count;
    return count;
}

/* The vpiName of automatic `entry`: a block local's name ends its block
 * path (decision A19-D88). */
static LLG_OUTLYING const char* rts_auto_name(const llg_vpi_auto_t* entry) {
    if (!(entry->flags & LLG_VPI_AUTO_BLOCK)) return entry->name;
    const char* dot = strrchr(entry->name, '.');
    return dot ? dot + 1 : entry->name;
}

/* The table entry of an automatic-variable handle while its frame lives,
 * with the frame; NULL once the frame ended. */
static const llg_vpi_auto_t* rts_auto(const llg_vpi_slot_t* slot, rts_frame_t** frame_out) {
    rts_frame_t* frame = rts_frame_find(slot->aux, (uint64_t)slot->left, slot->element, NULL);
    if (!frame || slot->right < 0 || (uint64_t)slot->right >= rts_auto_count(frame)) return NULL;
    if (frame_out) *frame_out = frame;
    return &rts_auto_table(frame)[slot->right];
}

static int32_t rts_is_part(const llg_vpi_slot_t* slot);
typedef struct rts_part rts_part_t;
static int32_t rts_part_type(const llg_vpi_slot_t* slot);

static int32_t rts_auto_type(const llg_vpi_slot_t* slot) {
    const llg_vpi_auto_t* entry = rts_auto(slot, NULL);
    if (entry && rts_is_part(slot)) return rts_part_type(slot);
    return entry ? entry->type : vpiUndefined;
}

static vpiHandle rts_thread_handle(int32_t slot, uint64_t identity) {
    return rts_make(SLOT_THREAD, (uint32_t)slot, (int64_t)identity);
}

static vpiHandle rts_frame_handle(int32_t slot, uint64_t identity, uint64_t serial) {
    vpiHandle handle = rts_make(SLOT_FRAME, (uint32_t)slot, (int64_t)identity);
    llg_vpi_slot_t* frame = handle ? decode_handle(handle, NULL) : NULL;
    if (frame) frame->element = serial;
    return handle;
}

/* The innermost frame of (slot, identity), or NULL. */
static vpiHandle rts_top_frame(int32_t slot, uint64_t identity) {
    rts_stack_t* stack = rts_stack(slot, identity, 0);
    if (!stack || !stack->depth) return NULL;
    return rts_frame_handle(slot, identity, stack->frames[stack->depth - 1].serial);
}

/* A thread or frame handle whose object still exists; otherwise an error. */
static int32_t rts_alive(const llg_vpi_slot_t* slot) {
    int32_t slot_index = slot->aux;
    uint64_t identity = (uint64_t)slot->left;
    if (slot->kind == SLOT_THREAD) {
        if (rts_thread_valid(slot_index, identity)) return 1;
        vpi_error("LLG_VPI_HANDLE", "this thread has ended; its handle is no longer valid");
        return 0;
    }
    if (slot->kind == SLOT_FRAME) {
        if (rts_frame_find(slot_index, identity, slot->element, NULL)) return 1;
        vpi_error("LLG_VPI_HANDLE", "this frame has ended; its handle is no longer valid");
        return 0;
    }
    if (slot->kind == SLOT_AUTO) {
        if (rts_auto(slot, NULL)) return 1;
        vpi_error("LLG_VPI_HANDLE",
                  "the frame of this automatic variable has ended; its handle is no longer valid");
        return 0;
    }
    return 1;
}

static int32_t rts_runtime_slot(const llg_vpi_slot_t* slot) {
    return slot->kind == SLOT_THREAD || slot->kind == SLOT_FRAME || slot->kind == SLOT_AUTO;
}

/* The automatic variables of a frame (37.39 detail 1), in table order. */
static vpiHandle rts_automatics(const llg_vpi_slot_t* frame_slot) {
    int32_t thread_slot = frame_slot->aux;
    int64_t identity = frame_slot->left;
    uint64_t serial = frame_slot->element;
    rts_frame_t* frame = rts_frame_find(thread_slot, (uint64_t)identity, serial, NULL);
    uint32_t count = frame ? rts_auto_count(frame) : 0u;
    if (!count) return NULL;
    const llg_vpi_auto_t* table = rts_auto_table(frame);
    rts_handles_t list = {0};
    for (uint32_t i = 0; i < count; ++i) {
        /* A block's locals are its variables, not the frame's. */
        if (table[i].flags & LLG_VPI_AUTO_BLOCK) continue;
        vpiHandle handle = rts_make(SLOT_AUTO, (uint32_t)thread_slot, identity);
        llg_vpi_slot_t* entry = handle ? decode_handle(handle, NULL) : NULL;
        if (entry) {
            entry->element = serial;
            entry->right = (int64_t)i;
        }
        rts_push(&list, handle);
    }
    return rts_list(vpiAutomatics, list.items, list.count, list.failed);
}

/* ── parts of automatics: elements of fixed-size arrays, members ── */

/* A SLOT_AUTO handle names a part of its automatic variable when `count`
 * (element offset + 1, row-major from the leftmost element) or `lsb` (a
 * typespec member node + 1) is set: an element of a fixed-size array kept
 * as one packed image (LLG_VPI_AUTO_IMAGE), as real cells
 * (LLG_VPI_AUTO_REALS) or as keyed cells (LLG_VPI_AUTO_FIXED), a member of
 * a structure or union kept as a packed image (an unpacked one with
 * LLG_VPI_AUTO_IMAGE, a packed one by its value shape), or a member of such
 * an element (decisions A19-D83, A19-D88). With `lsb` naming an unpacked
 * array member, `span` (element offset + 1, row-major) names one of its
 * elements. Parts are read and written through the variable's storage;
 * nothing is kept per part. */
static int32_t rts_is_part(const llg_vpi_slot_t* slot) { return slot->count || slot->lsb || slot->span; }

struct rts_part {
    const llg_vpi_auto_t* entry;
    rts_frame_t* frame;
    void* storage;                      /* the address-array entry, or NULL */
    const llg_vpi_typespec_t* typespec; /* the typespec of the part's value */
    const llg_vpi_member_node_t* node;  /* a member's node, else NULL */
    const llg_vpi_shape_t* shape;       /* an element's shape, else NULL */
    uint64_t element;                   /* element offset, or LLG_VPI_NO_ELEMENT */
    uint64_t member_element;            /* element of an array member, or LLG_VPI_NO_ELEMENT */
    uint32_t bit;                       /* image bits of a packed part */
    uint32_t width;
    int32_t type;
};

/* The element count of an unpacked array typespec, 0 without ranges. */
static LLG_OUTLYING uint64_t rts_typespec_elements(const llg_vpi_typespec_t* typespec) {
    if (!typespec || typespec->type != vpiArrayTypespec || !typespec->range_count) return 0;
    const int64_t* ranges = &g_vpi.types->ranges[2u * typespec->first_range];
    uint64_t count = 1;
    for (uint32_t d = 0; d < typespec->range_count; ++d) {
        int64_t left = ranges[2u * d], right = ranges[2u * d + 1u];
        count *= (uint64_t)(left > right ? left - right : right - left) + 1u;
    }
    return count;
}

/* The variable type of a packed element typespec (37.17). */
static LLG_OUTLYING int32_t rts_typespec_var_type(const llg_vpi_typespec_t* typespec) {
    switch (typespec ? typespec->type : 0) {
        case vpiIntTypespec: return vpiIntVar;
        case vpiByteTypespec: return vpiByteVar;
        case vpiShortIntTypespec: return vpiShortIntVar;
        case vpiLongIntTypespec: return vpiLongIntVar;
        case vpiIntegerTypespec: return vpiIntegerVar;
        case vpiTimeTypespec: return vpiTimeVar;
        case vpiBitTypespec: return vpiBitVar;
        case vpiLogicTypespec: return vpiLogicVar;
        case vpiEnumTypespec: return vpiEnumVar;
        case vpiPackedArrayTypespec: return vpiPackedArrayVar;
        case vpiStructTypespec: return vpiStructVar;
        case vpiUnionTypespec: return vpiUnionVar;
        default: return vpiUndefined;
    }
}

/* The array typespec's leaf element: every unpacked dimension removed. */
static const llg_vpi_typespec_t* rts_leaf_typespec(const llg_vpi_typespec_t* typespec) {
    for (uint32_t guard = 0; typespec && typespec->type == vpiArrayTypespec && guard < 64u; ++guard)
        typespec = typespec_ref(typespec->element);
    return typespec;
}

#define RTS_AUTO_CELLS (LLG_VPI_AUTO_IMAGE | LLG_VPI_AUTO_REALS | LLG_VPI_AUTO_FIXED)

/* Whether automatic `entry` is a fixed-size array whose elements llg
 * serves (37.17 detail 4): one packed image, real cells or keyed cells. */
static int32_t rts_auto_fixed(const llg_vpi_auto_t* entry) {
    return entry->type == vpiArrayVar && entry->array_type == vpiStaticArray &&
           (entry->flags & RTS_AUTO_CELLS) && !(entry->flags & LLG_VPI_AUTO_NO_VALUE);
}

/* The value shape of automatic `entry`: none for a fixed-size array, whose
 * row names its element shape. */
static const llg_vpi_shape_t* rts_auto_value_shape(const llg_vpi_auto_t* entry) {
    if (!entry->shape || (entry->type == vpiArrayVar && (entry->flags & RTS_AUTO_CELLS)))
        return NULL;
    return dyn_shape(entry->shape - 1u);
}

/* The typespec member nodes a part's value holds (a structure or union kept
 * as a packed image); 0 when it has no member objects. */
static uint32_t rts_part_members(const rts_part_t* part, uint32_t* first) {
    /* An element of an array member lists no members of its own. */
    if (part->member_element != LLG_VPI_NO_ELEMENT) return 0;
    if (part->node) {
        *first = part->node->first_child;
        return part->node->child_count;
    }
    const llg_vpi_typespec_t* typespec = part->typespec;
    if (!is_aggregate_typespec(typespec)) return 0;
    if (part->element == LLG_VPI_NO_ELEMENT) {
        /* The variable itself: an image, or a packed structure value. */
        const llg_vpi_shape_t* shape = rts_auto_value_shape(part->entry);
        if (!(part->entry->flags & LLG_VPI_AUTO_IMAGE) &&
            !(shape && shape->shape == LLG_VPI_SHAPE_PACKED))
            return 0;
    } else if (!(part->entry->flags & (LLG_VPI_AUTO_IMAGE | LLG_VPI_AUTO_FIXED))) {
        return 0;
    }
    *first = typespec->first_node;
    return typespec->node_count;
}

/* Resolve a part handle (or the variable itself) while its frame lives. */
static int32_t rts_part(const llg_vpi_slot_t* slot, rts_part_t* part) {
    memset(part, 0, sizeof(*part));
    part->element = LLG_VPI_NO_ELEMENT;
    part->member_element = LLG_VPI_NO_ELEMENT;
    part->entry = rts_auto(slot, &part->frame);
    if (!part->entry) {
        vpi_error("LLG_VPI_HANDLE",
                  "the frame of this automatic variable has ended; its handle is no longer valid");
        return 0;
    }
    const llg_vpi_auto_t* entry = part->entry;
    if (!(entry->flags & LLG_VPI_AUTO_NO_VALUE))
        part->storage = part->frame->slots[LLG_VPI_AUTOS_FIRST + (uint64_t)slot->right];
    const llg_vpi_typespec_t* typespec = typespec_ref(entry->typespec);
    part->typespec = typespec;
    part->type = entry->type;
    const llg_vpi_shape_t* stored = rts_auto_value_shape(entry);
    if (stored && stored->shape == LLG_VPI_SHAPE_PACKED) part->width = stored->width;
    else if (entry->flags & LLG_VPI_AUTO_IMAGE && typespec) part->width = typespec->width;
    uint32_t base = 0;
    if (slot->count) {
        if (!rts_auto_fixed(entry) || slot->count > entry->size) return 0;
        uint64_t offset = slot->count - 1u;
        part->element = offset;
        part->typespec = rts_leaf_typespec(typespec);
        part->shape = entry->shape ? dyn_shape(entry->shape - 1u) : NULL;
        part->width = 0;
        if (entry->flags & LLG_VPI_AUTO_IMAGE) {
            uint32_t width = typespec && entry->size ? typespec->width / entry->size : 0u;
            base = (uint32_t)((entry->size - 1u - offset) * width);
            part->bit = base;
            part->width = width;
        } else if (entry->flags & LLG_VPI_AUTO_FIXED) {
            /* The element's own cell is the image its parts lie in. */
            part->width = part->typespec ? part->typespec->width : 0u;
        }
        if (part->shape) part->type = part->shape->type;
        else if (part->typespec && part->typespec->type == vpiStructTypespec) part->type = vpiStructVar;
        else if (part->typespec && part->typespec->type == vpiUnionTypespec) part->type = vpiUnionVar;
        else part->type = vpiUndefined;
    }
    if (slot->lsb) {
        const llg_vpi_member_node_t* node = node_at(slot->lsb - 1u);
        if (!node) return 0;
        part->node = node;
        part->typespec = typespec_at(node->typespec);
        part->shape = NULL;
        part->type = node->type;
        part->bit = base + node->lsb;
        part->width = node->width;
    }
    if (slot->span) {
        /* An element of an unpacked array member: its bits in the image. */
        const llg_vpi_typespec_t* array = part->typespec;
        uint64_t count = rts_typespec_elements(array);
        if (!part->node || part->node->type != vpiRegArray || !count || slot->span > count ||
            part->node->width % count)
            return 0;
        uint64_t offset = slot->span - 1u;
        uint32_t width = (uint32_t)(part->node->width / count);
        part->member_element = offset;
        part->typespec = rts_leaf_typespec(array);
        part->type = rts_typespec_var_type(part->typespec);
        part->bit += (uint32_t)((count - 1u - offset) * width);
        part->width = width;
    }
    return 1;
}

/* Whether a part's typespec is an unpacked structure or union: no value of
 * its own (37.17 detail 11, 37.24 detail 1). */
static int32_t rts_part_unpacked_aggregate(const rts_part_t* part) {
    return is_aggregate_typespec(part->typespec) && !(part->typespec->flags & LLG_VPI_TS_PACKED);
}

static int32_t rts_part_type(const llg_vpi_slot_t* slot) {
    rts_part_t part;
    return rts_part(slot, &part) ? part.type : vpiUndefined;
}

/* A new handle to part (`count`, `member`, `span`) of automatic `base`. */
static LLG_OUTLYING vpiHandle rts_part_make_at(const llg_vpi_slot_t* base, uint32_t count, uint32_t member,
                                  uint32_t span) {
    vpiHandle handle = rts_make(SLOT_AUTO, (uint32_t)base->aux, base->left);
    llg_vpi_slot_t* slot = handle ? decode_handle(handle, NULL) : NULL;
    if (slot) {
        slot->element = base->element;
        slot->right = base->right;
        slot->count = count;
        slot->lsb = member;
        slot->span = span;
    }
    return handle;
}

/* A new handle to part (`count`, `member`) of automatic `base`. */
static vpiHandle rts_part_make(const llg_vpi_slot_t* base, uint32_t count, uint32_t member) {
    return rts_part_make_at(base, count, member, 0u);
}

/* Whether a part is an unpacked array member (not one of its elements). */
static LLG_OUTLYING int32_t rts_part_array_member(const rts_part_t* part) {
    return part->node && part->node->type == vpiRegArray &&
           part->member_element == LLG_VPI_NO_ELEMENT;
}

/* The element of array member part `slot` at `offset`, or NULL with an
 * error. */
static LLG_OUTLYING vpiHandle rts_member_element(const llg_vpi_slot_t* slot, const rts_part_t* part,
                                    uint64_t offset) {
    if (offset >= rts_typespec_elements(part->typespec)) {
        vpi_error("LLG_VPI_INDEX", "array index is outside the declared range");
        return NULL;
    }
    return rts_part_make_at(slot, slot->count, slot->lsb, (uint32_t)(offset + 1u));
}

/* The row-major offset of the declared `indices` in array typespec
 * `typespec`, or 0 with an error. */
static LLG_OUTLYING int32_t rts_typespec_offset(const llg_vpi_typespec_t* typespec, const int64_t* indices,
                               uint32_t count, uint64_t* offset) {
    if (!typespec || typespec->type != vpiArrayTypespec || count != typespec->range_count) {
        vpi_error("LLG_VPI_INDEX", "wrong number of indices for the array");
        return 0;
    }
    const int64_t* ranges = &g_vpi.types->ranges[2u * typespec->first_range];
    uint64_t flat = 0;
    for (uint32_t d = 0; d < count; ++d) {
        int64_t left = ranges[2u * d], right = ranges[2u * d + 1u];
        int64_t low = left < right ? left : right, high = left < right ? right : left;
        if (indices[d] < low || indices[d] > high) {
            vpi_error("LLG_VPI_INDEX", "array index is outside the declared range");
            return 0;
        }
        flat = flat * ((uint64_t)(high - low) + 1u) +
               (uint64_t)(left >= right ? left - indices[d] : indices[d] - left);
    }
    *offset = flat;
    return 1;
}

/* The declared indices of element `offset` of array typespec `typespec`
 * (leftmost first); the dimension count. */
static LLG_OUTLYING uint32_t rts_typespec_indices(const llg_vpi_typespec_t* typespec, uint64_t offset,
                                     int64_t* indices, uint32_t capacity) {
    if (!typespec || !typespec->range_count || typespec->range_count > capacity) return 0;
    const int64_t* ranges = &g_vpi.types->ranges[2u * typespec->first_range];
    for (uint32_t d = typespec->range_count; d-- > 0;) {
        int64_t left = ranges[2u * d], right = ranges[2u * d + 1u];
        uint64_t extent = (uint64_t)(left > right ? left - right : right - left) + 1u;
        uint64_t index = offset % extent;
        offset /= extent;
        indices[d] = left >= right ? left - (int64_t)index : left + (int64_t)index;
    }
    return typespec->range_count;
}

/* The packed shape a part's value reads and writes through. */
static llg_vpi_shape_t rts_part_packed_shape(const rts_part_t* part) {
    llg_vpi_shape_t shape;
    if (part->shape && part->shape->shape == LLG_VPI_SHAPE_PACKED) return *part->shape;
    memset(&shape, 0, sizeof(shape));
    shape.shape = LLG_VPI_SHAPE_PACKED;
    shape.type = part->type;
    shape.width = part->width;
    if (part->member_element != LLG_VPI_NO_ELEMENT && part->typespec) {
        shape.is_signed = (part->typespec->flags & LLG_VPI_TS_SIGNED) != 0;
        shape.two_state = (part->typespec->flags & LLG_VPI_TS_TWO_STATE) != 0;
    } else if (part->node) {
        shape.is_signed = (part->node->flags & LLG_VPI_TS_SIGNED) != 0;
        shape.two_state = (part->node->flags & LLG_VPI_TS_TWO_STATE) != 0;
    }
    return shape;
}

/* The whole packed image a part lies in (an owned copy): the variable's
 * image, or the element's cell of an array kept in keyed cells. */
static int32_t rts_part_image(const rts_part_t* part, sv4_t* image) {
    if (!part->storage) {
        vpi_error("LLG_VPI_VALUE", "this automatic variable is not declared yet in its frame");
        return 0;
    }
    if (part->entry->flags & LLG_VPI_AUTO_FIXED) {
        const llg_fixed_array_t* cells = (const llg_fixed_array_t*)part->storage;
        const llg_vpi_typespec_t* leaf = rts_leaf_typespec(typespec_ref(part->entry->typespec));
        if (part->element == LLG_VPI_NO_ELEMENT || part->element >= cells->total || !leaf ||
            (uint64_t)part->bit + part->width > leaf->width) {
            vpi_error("LLG_VPI_MODEL", "the storage of this automatic variable disagrees with its type");
            return 0;
        }
        *image = sv4_clone(llg_fixed_array_peek(cells, part->element));
        if (llg_sv4_width(*image) != leaf->width) {
            sv4_destroy(image);
            vpi_error("LLG_VPI_MODEL", "the storage of this automatic variable disagrees with its type");
            return 0;
        }
        return 1;
    }
    *image = (part->entry->flags & LLG_VPI_AUTO_REF) ? llg_rt_ref_read((const llg_ref_t*)part->storage)
                                                     : sv4_clone((const sv4_t*)part->storage);
    const llg_vpi_typespec_t* typespec = typespec_ref(part->entry->typespec);
    const llg_vpi_shape_t* stored = rts_auto_value_shape(part->entry);
    uint32_t expected = stored && stored->shape == LLG_VPI_SHAPE_PACKED ? stored->width
                        : typespec ? typespec->width : 0u;
    if (!expected || llg_sv4_width(*image) != expected ||
        (uint64_t)part->bit + part->width > expected) {
        sv4_destroy(image);
        vpi_error("LLG_VPI_MODEL", "the storage of this automatic variable disagrees with its type");
        return 0;
    }
    return 1;
}

/* Whether a part has a value llg reads, else an error. */
static int32_t rts_part_has_value(const rts_part_t* part) {
    if (rts_part_unpacked_aggregate(part)) {
        vpi_error("LLG_VPI_VALUE",
                  "an unpacked structure or union has no value; access its members (37.17 detail 11)");
        return 0;
    }
    if (part->node && part->member_element == LLG_VPI_NO_ELEMENT &&
        (part->node->type == vpiRegArray || !part->width)) {
        vpi_error("LLG_VPI_VALUE", part->node->type == vpiRegArray
                                       ? "an array variable has no value; access its elements (37.17 detail 11)"
                                       : "llg serves no value of this member of an automatic variable");
        return 0;
    }
    if (part->element != LLG_VPI_NO_ELEMENT && !part->node && !part->shape) {
        vpi_error("LLG_VPI_VALUE", "this element has no value llg exposes");
        return 0;
    }
    return 1;
}

static void rts_part_value(const llg_vpi_slot_t* slot, p_vpi_value output) {
    rts_part_t part;
    if (!rts_part(slot, &part) || !rts_part_has_value(&part)) return;
    if (part.entry->flags & LLG_VPI_AUTO_REALS) {
        if (!part.storage) {
            vpi_error("LLG_VPI_VALUE", "this automatic variable is not declared yet in its frame");
            return;
        }
        dyn_read(part.shape, (double*)part.storage + part.element, output);
        return;
    }
    sv4_t image;
    if (!rts_part_image(&part, &image)) return;
    llg_vpi_shape_t shape = rts_part_packed_shape(&part);
    sv4_t value = sv4_part_select(image, (int64_t)part.bit + part.width - 1, (int64_t)part.bit);
    llg_sv4_set_signed(&value, 0);
    dyn_read(&shape, &value, output);
    sv4_destroy(&value);
    sv4_destroy(&image);
}

/* vpi_put_value (vpiNoDelay) of a part: the variable's storage changes at
 * once, through its `ref` descriptor for a `ref` formal. */
static void rts_part_put(const llg_vpi_slot_t* slot, const s_vpi_value* input) {
    rts_part_t part;
    if (!rts_part(slot, &part) || !rts_part_has_value(&part)) return;
    if (part.entry->flags & LLG_VPI_AUTO_REALS) {
        double value;
        if (!part.storage) {
            vpi_error("LLG_VPI_VALUE", "this automatic variable is not declared yet in its frame");
            return;
        }
        if (dyn_real_input(part.shape, input, &value)) llg_ba_d((double*)part.storage + part.element, value);
        return;
    }
    llg_vpi_shape_t shape = rts_part_packed_shape(&part);
    sv4_t value;
    if (!dyn_packed_input(&shape, input, &value)) return;
    sv4_t image;
    if (!rts_part_image(&part, &image)) {
        sv4_destroy(&value);
        return;
    }
    sv4_part_select_set(&image, (int64_t)part.bit + part.width - 1, (int64_t)part.bit, value);
    if (part.entry->flags & LLG_VPI_AUTO_REF) llg_ref_write((llg_ref_t*)part.storage, image);
    else if (part.entry->flags & LLG_VPI_AUTO_FIXED)
        llg_ba(llg_fixed_array_cell((llg_fixed_array_t*)part.storage, part.element), image);
    else llg_ba((sv4_t*)part.storage, image);
    sv4_destroy(&image);
    sv4_destroy(&value);
}

/* The declared indices of element `offset` of a fixed-size array automatic
 * (leftmost dimension first); the dimension count, 0 without ranges. */
static uint32_t rts_element_indices(const llg_vpi_auto_t* entry, uint64_t offset, int64_t* indices,
                                    uint32_t capacity) {
    const llg_vpi_typespec_t* typespec = typespec_ref(entry->typespec);
    if (!typespec || !typespec->range_count || typespec->range_count > capacity) return 0;
    const int64_t* ranges = &g_vpi.types->ranges[2u * typespec->first_range];
    for (uint32_t d = typespec->range_count; d-- > 0;) {
        int64_t left = ranges[2u * d], right = ranges[2u * d + 1u];
        uint64_t extent = (uint64_t)(left > right ? left - right : right - left) + 1u;
        uint64_t index = offset % extent;
        offset /= extent;
        indices[d] = left >= right ? left - (int64_t)index : left + (int64_t)index;
    }
    return typespec->range_count;
}

/* The element offset of the declared indices, or 0 with an error. */
static int32_t rts_element_offset(const llg_vpi_auto_t* entry, const int64_t* indices, uint32_t count,
                              uint64_t* offset) {
    const llg_vpi_typespec_t* typespec = typespec_ref(entry->typespec);
    if (!typespec || !typespec->range_count) {
        vpi_error("LLG_VPI_INDEX", "this array has no declared range");
        return 0;
    }
    if (count != typespec->range_count) {
        vpi_error("LLG_VPI_INDEX", typespec->range_count > 1u && count == 1u
                                       ? "use vpi_handle_by_multi_index for a multidimensional array"
                                       : "wrong number of indices for the array");
        return 0;
    }
    const int64_t* ranges = &g_vpi.types->ranges[2u * typespec->first_range];
    uint64_t flat = 0;
    for (uint32_t d = 0; d < count; ++d) {
        int64_t left = ranges[2u * d], right = ranges[2u * d + 1u];
        int64_t low = left < right ? left : right, high = left < right ? right : left;
        if (indices[d] < low || indices[d] > high) {
            vpi_error("LLG_VPI_INDEX", "array index is outside the declared range");
            return 0;
        }
        uint64_t extent = (uint64_t)(high - low) + 1u;
        flat = flat * extent + (uint64_t)(left >= right ? left - indices[d] : indices[d] - left);
    }
    if (flat >= entry->size) {
        vpi_error("LLG_VPI_INDEX", "array index is outside the declared range");
        return 0;
    }
    *offset = flat;
    return 1;
}

/* The element of a fixed-size array automatic at the declared indices. */
static vpiHandle rts_fixed_element(const llg_vpi_slot_t* slot, const int64_t* indices, uint32_t count) {
    if (!rts_alive(slot)) return NULL;
    const llg_vpi_auto_t* entry = rts_auto(slot, NULL);
    uint64_t offset;
    if (rts_is_part(slot) || !rts_auto_fixed(entry)) {
        vpi_error("LLG_VPI_INDEX", "object has no access by index");
        return NULL;
    }
    if (!rts_element_offset(entry, indices, count, &offset)) return NULL;
    return rts_part_make(slot, (uint32_t)(offset + 1u), 0u);
}

/* vpi_handle_by_multi_index of an automatic: an element of a fixed-size
 * array kept as an image or as a container. */
static vpiHandle rts_auto_by_multi_index(const llg_vpi_slot_t* slot, const int64_t* indices,
                                         uint32_t count) {
    dyn_place_t place;
    if (!rts_is_part(slot) && slot->right >= 0 &&
        rts_auto_container(slot->element, (uint32_t)slot->right, &place)) {
        if (!rts_alive(slot)) return NULL;
        return dyn_element_by_indices(&place, (uint32_t)slot->right, indices, count, 0);
    }
    rts_part_t part;
    if (rts_is_part(slot) && rts_alive(slot) && rts_part(slot, &part) &&
        rts_part_array_member(&part)) {
        uint64_t offset;
        return rts_typespec_offset(part.typespec, indices, count, &offset)
                   ? rts_member_element(slot, &part, offset)
                   : NULL;
    }
    return rts_fixed_element(slot, indices, count);
}

/* `[i]...` of an element of an array member, appended to `out`. */
static LLG_OUTLYING int32_t rts_member_index_path(const rts_part_t* part, char* out, size_t size, size_t* used) {
    int64_t indices[16];
    uint32_t dims = rts_typespec_indices(typespec_at(part->node->typespec), part->member_element,
                                         indices, 16u);
    for (uint32_t d = 0; d < dims; ++d) {
        int32_t written = snprintf(out + *used, size - *used, "[%lld]", (long long)indices[d]);
        if (written < 0 || (size_t)written >= size - *used) return 0;
        *used += (size_t)written;
    }
    return 1;
}

/* `name[i]...` of an element, `.member` of a member, appended to `out`. */
static int32_t rts_part_path(const llg_vpi_slot_t* slot, const rts_part_t* part, char* out, size_t size,
                         size_t* used) {
    int32_t written;
    if (part->element != LLG_VPI_NO_ELEMENT) {
        int64_t indices[16];
        uint32_t dims = rts_element_indices(part->entry, part->element, indices, 16u);
        for (uint32_t d = 0; d < dims; ++d) {
            written = snprintf(out + *used, size - *used, "[%lld]", (long long)indices[d]);
            if (written < 0 || (size_t)written >= size - *used) return 0;
            *used += (size_t)written;
        }
    }
    if (slot->lsb) {
        /* The member chain, outermost first. */
        uint32_t chain[64];
        uint32_t depth = 0;
        for (uint32_t node = slot->lsb; node && depth < 64u; node = node_at(node - 1u)->parent)
            chain[depth++] = node - 1u;
        while (depth-- > 0) {
            written = snprintf(out + *used, size - *used, ".%s", node_at(chain[depth])->name);
            if (written < 0 || (size_t)written >= size - *used) return 0;
            *used += (size_t)written;
        }
    }
    if (part->member_element != LLG_VPI_NO_ELEMENT &&
        !rts_member_index_path(part, out, size, used))
        return 0;
    return 1;
}

static const char* rts_part_str(int32_t property, const llg_vpi_slot_t* slot) {
    rts_part_t part;
    if (!rts_part(slot, &part)) return NULL;
    char* out = g_vpi.str_buffer;
    size_t size = sizeof(g_vpi.str_buffer), used = 0;
    int32_t written;
    if (property == vpiName && part.node && part.member_element != LLG_VPI_NO_ELEMENT) {
        written = snprintf(out, size, "%s", part.node->name);
        used = written < 0 ? size : (size_t)written;
        if (used >= size || !rts_member_index_path(&part, out, size, &used)) {
            vpi_error("LLG_VPI_LIMIT", "the VPI name is too long");
            return NULL;
        }
        return out;
    }
    if (property == vpiName && part.node) return part.node->name;
    if (property == vpiName)
        written = snprintf(out, size, "%s", rts_auto_name(part.entry));
    else if (property == vpiFullName)
        written = snprintf(out, size, "%s.%s", part.frame->name, part.entry->name);
    else if (property == vpiFile) {
        const llg_vpi_model_object_t* object = row_object(rts_frame_row(part.frame));
        if (object) return object->file;
        vpi_error("LLG_VPI_PROPERTY", "this object has no such string property");
        return NULL;
    } else {
        vpi_error("LLG_VPI_PROPERTY", "this object has no such string property");
        return NULL;
    }
    if (written < 0 || (size_t)written >= size) {
        vpi_error("LLG_VPI_LIMIT", "the VPI name is too long");
        return NULL;
    }
    used = (size_t)written;
    if (!rts_part_path(slot, &part, out, size, &used)) {
        vpi_error("LLG_VPI_LIMIT", "the VPI name is too long");
        return NULL;
    }
    return out;
}

static PLI_INT64 rts_part_get(int32_t property, const llg_vpi_slot_t* slot) {
    rts_part_t part;
    if (!rts_part(slot, &part)) return vpiUndefined;
    int32_t element = !part.node || part.member_element != LLG_VPI_NO_ELEMENT;
    int32_t packed = !rts_part_unpacked_aggregate(&part) && part.width && !rts_part_array_member(&part);
    int32_t real = (part.entry->flags & LLG_VPI_AUTO_REALS) != 0;
    switch (property) {
        /* 37.3.7: a part of an automatic variable is non-static too. */
        case vpiAutomatic: return 1;
        case vpiArrayMember: return element;
        case vpiStructUnionMember: return !element;
        case vpiAllocScheme: return vpiAutomaticScheme;
        case vpiLineNo: return part.entry->line;
        case vpiArray: return rts_part_array_member(&part);
        case vpiArrayType:
            /* An unpacked array member of a structure is a fixed array. */
            if (rts_part_array_member(&part)) return vpiStaticArray;
            break;
        case vpiConstantVariable: return 0;
        case vpiSize:
            /* 37.17 detail 9: an array member's element count. */
            if (rts_part_array_member(&part)) return (PLI_INT64)rts_typespec_elements(part.typespec);
            if (rts_part_unpacked_aggregate(&part)) return part.typespec->member_count;
            if (real) return part.shape && part.shape->shortreal ? 32 : 64;
            if (part.width) return part.width;
            break;
        case vpiSigned:
            if (real || rts_part_unpacked_aggregate(&part)) return 0;
            if (part.shape) return part.shape->is_signed;
            if (part.node && part.member_element == LLG_VPI_NO_ELEMENT)
                return (part.node->flags & LLG_VPI_TS_SIGNED) != 0;
            return part.typespec && (part.typespec->flags & LLG_VPI_TS_SIGNED) != 0;
        case vpiScalar:
        case vpiVector:
            if (real || rts_part_unpacked_aggregate(&part)) return 0;
            if (packed) return property == vpiScalar ? part.width == 1u : part.width != 1u;
            break;
        default: break;
    }
    return unsupported_property();
}

static vpiHandle rts_part_handle(int32_t type, const llg_vpi_slot_t* slot) {
    rts_part_t part;
    if (!rts_part(slot, &part)) return NULL;
    switch (type) {
        case vpiParent:
            /* A member's parent is the value holding it, an element's its
             * array (37.17 details 13, 25). */
            if (part.member_element != LLG_VPI_NO_ELEMENT) return rts_part_make(slot, slot->count, slot->lsb);
            if (part.node && part.node->parent) return rts_part_make(slot, slot->count, part.node->parent);
            if (part.node && slot->count) return rts_part_make(slot, slot->count, 0u);
            return rts_part_make(slot, 0u, 0u);
        case vpiScope: {
            uint32_t row = rts_frame_row(part.frame);
            return row != LLG_VPI_NONE ? object_handle(row) : NULL;
        }
        case vpiTypespec: return declared_typespec_handle(part.typespec, LLG_VPI_NONE, 0);
        default: break;
    }
    vpi_error("LLG_VPI_RELATION", "this object has no such relation");
    return NULL;
}

/* vpiMember of a structure kept as a packed image, its elements or members
 * (37.17 detail 10); NULL with an error for other parts. */
static vpiHandle rts_part_member_list(const llg_vpi_slot_t* slot, const rts_part_t* part) {
    uint32_t first = 0;
    uint32_t count = rts_part_members(part, &first);
    if (!count) {
        vpi_error("LLG_VPI_RELATION", is_aggregate_typespec(part->typespec)
                                          ? "llg serves no members of this automatic structure"
                                          : "vpiMember iterates a structure or union variable");
        return NULL;
    }
    rts_handles_t list = {0};
    for (uint32_t i = 0; i < count; ++i) rts_push(&list, rts_part_make(slot, slot->count, first + i + 1u));
    return rts_list(vpiMember, list.items, list.count, list.failed);
}

/* vpiRange of a fixed-size array automatic (37.17 detail 6): its declared
 * ranges, leftmost first; a range of an automatic has no owning object. It
 * is one object per declared dimension of the subprogram's variable,
 * located at the variable and its bounds at the subprogram (decision
 * A19-D86). */
static vpiHandle rts_auto_ranges(const llg_vpi_auto_t* entry, rts_frame_t* frame,
                                 const llg_vpi_typespec_t* typespec) {
    if (!typespec || !typespec->range_count) {
        vpi_error("LLG_VPI_RELATION", "this array has no declared range");
        return NULL;
    }
    uint32_t row = frame ? rts_frame_row(frame) : LLG_VPI_NONE;
    const llg_vpi_model_object_t* subprogram = row_object(row);
    llg_vpi_range_origin_t origin = {
        RANGE_AUTO, (uint64_t)(uintptr_t)entry, 0u, LLG_VPI_NONE,
        subprogram ? located_scope(subprogram) : LLG_VPI_NONE,
        subprogram && subprogram->file ? entry->line : 0u, subprogram ? subprogram->file : NULL };
    rts_handles_t list = {0};
    for (uint32_t d = 0; d < typespec->range_count; ++d) {
        const int64_t* bounds = &g_vpi.types->ranges[2u * (typespec->first_range + d)];
        vpiHandle range = make_range(&origin, d, bounds[0], bounds[1]);
        if (!range) {
            list.failed = 1;
            break;
        }
        rts_push(&list, range);
    }
    return rts_list(vpiRange, list.items, list.count, list.failed);
}

/* vpiReg / vpiVarSelect of a fixed-size array automatic: its elements, in
 * row-major order (37.17 details 4 and 7). */
static vpiHandle rts_fixed_elements(int32_t type, const llg_vpi_slot_t* slot, const rts_part_t* part) {
    const llg_vpi_shape_t* element = part->entry->shape ? dyn_shape(part->entry->shape - 1u) : NULL;
    if (type != vpiVarSelect && !(type == vpiReg && element) &&
        !(element && type == element->type)) {
        vpi_error("LLG_VPI_RELATION", element ? "this array has no elements of the requested type"
                                              : "the elements of an array of structures are vpiVarSelect objects");
        return NULL;
    }
    if (!part->storage) {
        vpi_error("LLG_VPI_RELATION", "this automatic variable is not declared yet in its frame");
        return NULL;
    }
    rts_handles_t list = {0};
    for (uint32_t i = 0; i < part->entry->size; ++i) rts_push(&list, rts_part_make(slot, i + 1u, 0u));
    return rts_list(type, list.items, list.count, list.failed);
}

/* The live frame with `serial` on any thread, with its thread (VPI queries
 * only: a scan of the frame stacks). */
static rts_frame_t* rts_frame_by_serial(uint64_t serial, int32_t* thread_slot, uint64_t* identity) {
    for (uint32_t i = 0; i < rts.stack_count; ++i) {
        rts_stack_t* stack = &rts.stacks[i];
        if (!stack->depth) continue;
        rts_frame_t* frame = rts_frame_find((int32_t)i - 1, stack->identity, serial, NULL);
        if (frame) {
            if (thread_slot) *thread_slot = (int32_t)i - 1;
            if (identity) *identity = stack->identity;
            return frame;
        }
    }
    return NULL;
}

/* The container or record automatic `index` of frame `serial` as a
 * dynamic-layer place (its elements or members, decisions A19-D82,
 * A19-D88); 0 once the frame has ended or when the variable keeps
 * neither. */
static LLG_OUTLYING int32_t rts_auto_value_place(uint64_t serial, uint32_t index, dyn_place_t* place) {
    rts_frame_t* frame = rts_frame_by_serial(serial, NULL, NULL);
    if (!frame || index >= rts_auto_count(frame)) return 0;
    const llg_vpi_auto_t* entry = &rts_auto_table(frame)[index];
    const llg_vpi_shape_t* shape = rts_auto_value_shape(entry);
    void* storage = (entry->flags & LLG_VPI_AUTO_NO_VALUE)
                        ? NULL
                        : frame->slots[LLG_VPI_AUTOS_FIRST + index];
    if (!shape || !storage ||
        (shape->shape != LLG_VPI_SHAPE_CONTAINER && shape->shape != LLG_VPI_SHAPE_RECORD))
        return 0;
    place->shape = shape;
    place->storage = storage;
    place->row = DYN_AUTO_ROW;
    place->name = entry->name;
    place->serial = (int64_t)serial;
    if (shape->array_type == vpiStaticArray) {
        /* A fixed-size array of strings, handles or structures kept as
         * values: its declared ranges. */
        const llg_vpi_typespec_t* typespec = typespec_ref(entry->typespec);
        if (!typespec || !typespec->range_count || typespec->range_count > DYN_MAX_DIMS) return 0;
        place->dims = &g_vpi.types->ranges[2u * typespec->first_range];
        place->dim_count = typespec->range_count;
    }
    return 1;
}

/* rts_auto_value_place of a container automatic only. */
static int32_t rts_auto_container(uint64_t serial, uint32_t index, dyn_place_t* place) {
    return rts_auto_value_place(serial, index, place) &&
           place->shape->shape == LLG_VPI_SHAPE_CONTAINER;
}

static int32_t rts_auto_location(uint64_t serial, uint32_t index, uint32_t* line, const char** file) {
    rts_frame_t* frame = rts_frame_by_serial(serial, NULL, NULL);
    if (!frame || index >= rts_auto_count(frame)) return 0;
    const llg_vpi_model_object_t* subprogram = row_object(rts_frame_row(frame));
    *line = rts_auto_table(frame)[index].line;
    *file = subprogram ? subprogram->file : NULL;
    return *line && *file;
}

static uint32_t rts_auto_scope(uint64_t serial) {
    rts_frame_t* frame = rts_frame_by_serial(serial, NULL, NULL);
    return frame ? located_scope(row_object(rts_frame_row(frame))) : LLG_VPI_NONE;
}

static const char* rts_auto_frame_name(uint64_t serial) {
    rts_frame_t* frame = rts_frame_by_serial(serial, NULL, NULL);
    if (!frame) vpi_error("LLG_VPI_HANDLE", "the frame of this element has ended");
    return frame ? frame->name : NULL;
}

/* The automatic-variable handle of entry `index` of frame `serial`. */
static vpiHandle rts_auto_handle_by_serial(uint64_t serial, uint32_t index) {
    int32_t thread_slot = 0;
    uint64_t identity = 0;
    if (!rts_frame_by_serial(serial, &thread_slot, &identity)) {
        vpi_error("LLG_VPI_HANDLE", "the frame of this element has ended");
        return NULL;
    }
    vpiHandle handle = rts_make(SLOT_AUTO, (uint32_t)thread_slot, (int64_t)identity);
    llg_vpi_slot_t* entry = handle ? decode_handle(handle, NULL) : NULL;
    if (entry) {
        entry->element = serial;
        entry->right = (int64_t)index;
    }
    return handle;
}

/* The container place of an automatic-variable handle, else an error. */
static int32_t rts_auto_elements(const llg_vpi_slot_t* slot, dyn_place_t* place) {
    memset(place, 0, sizeof(*place));
    if (slot->right >= 0 && rts_auto_container(slot->element, (uint32_t)slot->right, place))
        return 1;
    rts_frame_t* frame = NULL;
    const llg_vpi_auto_t* entry = rts_auto(slot, &frame);
    vpi_error("LLG_VPI_RELATION",
              entry && entry->type == vpiArrayVar
                  ? (entry->flags & LLG_VPI_AUTO_LOCAL) && entry->shape &&
                            !(entry->flags & LLG_VPI_AUTO_NO_VALUE)
                        ? "this automatic variable is not declared yet in its frame"
                        : "llg serves no elements of this automatic array (its storage is a "
                          "descriptor-backed value)"
                  : "this object has no elements");
    return 0;
}

/* vpi_iterate(vpiReg / vpiVarSelect ...) of a dynamic array, queue or
 * associative array automatic: its elements (37.17 detail 4). */
static LLG_OUTLYING vpiHandle rts_auto_iterate(int32_t type, const llg_vpi_slot_t* slot) {
    rts_part_t part;
    dyn_place_t record;
    if (type == vpiMember && !rts_is_part(slot) && slot->right >= 0 &&
        rts_auto_value_place(slot->element, (uint32_t)slot->right, &record) &&
        record.shape->shape == LLG_VPI_SHAPE_RECORD)
        /* An unpacked structure kept as a record value (37.24 detail 1). */
        return dyn_record_members(&record, (uint32_t)slot->right);
    if (type == vpiMember || rts_is_part(slot)) {
        if (!rts_part(slot, &part)) return NULL;
        if (type == vpiMember) return rts_part_member_list(slot, &part);
        if (rts_part_array_member(&part)) {
            /* An unpacked array member: its ranges and its elements in
             * row-major order (37.17 details 4 and 6). */
            if (type == vpiRange) return rts_auto_ranges(part.entry, part.frame, part.typespec);
            const llg_vpi_typespec_t* leaf = rts_leaf_typespec(part.typespec);
            if (type == vpiReg || type == vpiVarSelect || type == rts_typespec_var_type(leaf)) {
                llg_vpi_slot_t base = *slot;
                uint64_t count = rts_typespec_elements(part.typespec);
                rts_handles_t list = {0};
                for (uint64_t i = 0; i < count; ++i)
                    rts_push(&list, rts_part_make_at(&base, base.count, base.lsb, (uint32_t)(i + 1u)));
                return rts_list(type, list.items, list.count, list.failed);
            }
        }
        vpi_error("LLG_VPI_RELATION", "this object has no such iteration");
        return NULL;
    }
    rts_frame_t* frame = NULL;
    const llg_vpi_auto_t* entry = rts_auto(slot, &frame);
    if (type == vpiRange && entry && entry->type == vpiArrayVar &&
        entry->array_type == vpiStaticArray)
        return rts_auto_ranges(entry, frame, typespec_ref(entry->typespec));
    if (rts_auto_fixed(entry)) {
        if (!rts_part(slot, &part)) return NULL;
        return rts_fixed_elements(type, slot, &part);
    }
    dyn_place_t place;
    if (!rts_auto_elements(slot, &place)) return NULL;
    const llg_vpi_shape_t* element = dyn_leaf_shape(place.shape);
    if (!element || !dyn_element_type_matches(element, type)) {
        vpi_error("LLG_VPI_RELATION", "this object has no such iteration");
        return NULL;
    }
    if (place.shape->key == LLG_VPI_KEY_OTHER) {
        vpi_error("LLG_VPI_UNSUPPORTED", "llg does not iterate associative arrays with a wildcard index");
        return NULL;
    }
    /* 37.17 detail 4: the innermost elements of nested containers. */
    if (dyn_shape(place.shape->element)->shape == LLG_VPI_SHAPE_CONTAINER)
        return dyn_iterate_leaves(&place, (uint32_t)slot->right, type);
    size_t size = dyn_container_size(&place);
    if (size > UINT32_MAX) size = UINT32_MAX;
    vpiHandle iterator = dyn_iterator(DYN_ITER_ELEMENTS, (uint32_t)size);
    llg_vpi_slot_t* state = iterator ? decode_handle(iterator, NULL) : NULL;
    if (!state) return NULL;
    state->object = DYN_AUTO_ROW;
    state->right = place.serial;
    state->pointer = NULL;
    state->lsb = (uint32_t)slot->right;
    return iterator;
}

/* vpi_handle_by_index of a container automatic: the element at `index`. */
static vpiHandle rts_auto_by_index(const llg_vpi_slot_t* slot, PLI_INT32 index) {
    if (!rts_alive(slot)) return NULL;
    rts_part_t part;
    if (rts_is_part(slot) && rts_part(slot, &part) && rts_part_array_member(&part)) {
        int64_t value = index;
        uint64_t offset;
        return rts_typespec_offset(part.typespec, &value, 1u, &offset)
                   ? rts_member_element(slot, &part, offset)
                   : NULL;
    }
    if (rts_is_part(slot) || rts_auto_fixed(rts_auto(slot, NULL))) {
        int64_t value = index;
        return rts_fixed_element(slot, &value, 1u);
    }
    dyn_place_t place;
    if (!rts_auto_elements(slot, &place)) return NULL;
    return dyn_element_by_index(&place, (uint32_t)slot->right, index, 0);
}

/* The place of an automatic variable (37.39 detail 1): its value shape and
 * its storage in the activation. The storage is NULL for a variable listed
 * without a value and for a body local whose declaration has not run; a
 * packed `ref` formal's is its reference descriptor. */
static const llg_vpi_auto_t* rts_auto_place(const llg_vpi_slot_t* slot, dyn_place_t* place) {
    rts_frame_t* frame = NULL;
    const llg_vpi_auto_t* entry = rts_auto(slot, &frame);
    memset(place, 0, sizeof(*place));
    place->row = LLG_VPI_NONE;
    if (!entry) return NULL;
    place->shape = rts_auto_value_shape(entry);
    place->name = entry->name;
    if (!(entry->flags & LLG_VPI_AUTO_NO_VALUE)) place->storage = frame->slots[LLG_VPI_AUTOS_FIRST + (uint64_t)slot->right];
    return entry;
}

/* The storage of an automatic that has a value now, else an error. */
static const llg_vpi_auto_t* rts_auto_storage(const llg_vpi_slot_t* slot, dyn_place_t* place) {
    const llg_vpi_auto_t* entry = rts_auto_place(slot, place);
    if (!entry) return NULL;
    if (place->shape && place->shape->shape == LLG_VPI_SHAPE_CONTAINER) {
        vpi_error("LLG_VPI_VALUE",
                  "an array variable has no value; access its elements (37.17 detail 11)");
        return NULL;
    }
    if (place->shape && place->shape->shape == LLG_VPI_SHAPE_RECORD) {
        vpi_error("LLG_VPI_VALUE",
                  "an unpacked structure or union has no value; access its members (37.17 detail 11)");
        return NULL;
    }
    if (!place->shape || (entry->flags & LLG_VPI_AUTO_NO_VALUE)) {
        vpi_error("LLG_VPI_VALUE",
                  entry->type == vpiArrayVar
                      ? "an array variable has no value; access its elements (37.17 detail 11)"
                  : entry->type == vpiStructVar || entry->type == vpiUnionVar
                      ? "an unpacked structure or union has no value; access its members (37.17 "
                        "detail 11)"
                      : "llg keeps no value of this automatic variable");
        return NULL;
    }
    if (!place->storage) {
        vpi_error("LLG_VPI_VALUE", "this automatic variable is not declared yet in its frame");
        return NULL;
    }
    return entry;
}

static void rts_auto_value(llg_vpi_slot_t* slot, p_vpi_value output) {
    if (rts_is_part(slot)) {
        rts_part_value(slot, output);
        return;
    }
    dyn_place_t place;
    const llg_vpi_auto_t* entry = rts_auto_storage(slot, &place);
    if (!entry) return;
    if (entry->flags & LLG_VPI_AUTO_REF) {
        /* A packed `ref` formal reads its actual (13.5.2). */
        llg_vpi_datum_t datum;
        memset(&datum, 0, sizeof(datum));
        datum.packed = llg_rt_ref_read((const llg_ref_t*)place.storage);
        fill_value(&datum, place.shape->is_signed, place.shape->type, output);
        datum_destroy(&datum);
        return;
    }
    dyn_read(place.shape, place.storage, output);
}

/* 37.39 details 2-3: an automatic variable takes only a vpiNoDelay value. */
static void rts_auto_put(llg_vpi_slot_t* slot, const s_vpi_value* input, int32_t mode) {
    if (!rts_alive(slot)) return;
    if (mode != vpiNoDelay) {
        vpi_error("LLG_VPI_FLAGS",
                  "an automatic variable takes only a vpiNoDelay value (37.39 detail 3)");
        return;
    }
    if (rts_is_part(slot)) {
        rts_part_put(slot, input);
        return;
    }
    dyn_place_t place;
    const llg_vpi_auto_t* entry = rts_auto_storage(slot, &place);
    if (!entry) return;
    if (entry->flags & LLG_VPI_AUTO_REF) {
        sv4_t value;
        if (!dyn_packed_input(place.shape, input, &value)) return;
        llg_ref_write((llg_ref_t*)place.storage, value);
        sv4_destroy(&value);
        return;
    }
    if (place.shape->shape == LLG_VPI_SHAPE_CLASS) {
        vpi_error("LLG_VPI_VALUE", "a class variable cannot receive a value through VPI");
        return;
    }
    dyn_write(place.shape, place.storage, NULL, input);
}

static PLI_INT64 rts_auto_get(int32_t property, const llg_vpi_slot_t* slot) {
    if (rts_is_part(slot)) return rts_part_get(property, slot);
    dyn_place_t place;
    const llg_vpi_auto_t* entry = rts_auto_place(slot, &place);
    const llg_vpi_shape_t* shape = place.shape;
    switch (property) {
        case vpiAutomatic: return 1;
        case vpiLineNo: return entry->line;
        case vpiArray: return entry->type == vpiArrayVar;
        case vpiArrayMember:
        case vpiConstantVariable:
        case vpiStructUnionMember: return 0;
        case vpiAllocScheme: return vpiAutomaticScheme;
        case vpiArrayType:
            if (entry->array_type) return entry->array_type;
            break;
        case vpiSigned:
            if (!shape && entry->type == vpiArrayVar)
                return (entry->flags & LLG_VPI_AUTO_SIGNED) != 0;
            if (shape && shape->shape == LLG_VPI_SHAPE_PACKED) return shape->is_signed;
            if (shape && (shape->shape == LLG_VPI_SHAPE_REAL || shape->shape == LLG_VPI_SHAPE_STRING ||
                          shape->shape == LLG_VPI_SHAPE_CHANDLE))
                return 0;
            if (shape && shape->shape == LLG_VPI_SHAPE_CONTAINER) {
                const llg_vpi_shape_t* element = dyn_shape(shape->element);
                if (element && element->shape == LLG_VPI_SHAPE_PACKED) return element->is_signed;
            }
            break;
        case vpiSize:
            if (!shape) {
                if (entry->type == vpiArrayVar) return entry->size;
                /* 37.17 detail 9: an unpacked structure or union reports
                 * its number of fields, as a static one does. */
                const llg_vpi_typespec_t* typespec = typespec_ref(entry->typespec);
                if (is_aggregate_typespec(typespec)) return typespec->member_count;
                break;
            }
            if (shape->shape == LLG_VPI_SHAPE_CONTAINER || shape->shape == LLG_VPI_SHAPE_STRING) {
                if (!place.storage) {
                    vpi_error("LLG_VPI_VALUE", "this automatic variable is not declared yet in its frame");
                    return vpiUndefined;
                }
                int32_t ok;
                PLI_INT64 size = dyn_size(&place, &ok);
                if (ok) return size;
                break;
            }
            if (shape->shape == LLG_VPI_SHAPE_PACKED) return shape->width;
            if (shape->shape == LLG_VPI_SHAPE_CHANDLE) return 64;
            if (shape->shape == LLG_VPI_SHAPE_REAL) return shape->shortreal ? 32 : 64;
            /* 37.17 detail 9: an unpacked structure's number of fields. */
            if (shape->shape == LLG_VPI_SHAPE_RECORD) return shape->member_count;
            break;
        case vpiScalar:
        case vpiVector:
            if (shape && shape->shape == LLG_VPI_SHAPE_PACKED)
                return property == vpiScalar ? shape->width == 1u : shape->width != 1u;
            /* An array or unpacked structure is no vector (37.17 detail 11). */
            if (shape || entry->type == vpiArrayVar || entry->type == vpiStructVar ||
                entry->type == vpiUnionVar)
                return 0;
            break;
        case vpiObjId:
            if (shape && shape->shape == LLG_VPI_SHAPE_CLASS && place.storage) {
                void* object = *(void**)place.storage;
                llg_vpi_live_t* live = object ? live_get(object) : NULL;
                return live ? (PLI_INT64)live->id : 0;
            }
            break;
        default: break;
    }
    return unsupported_property();
}

/* vpi_handle from an automatic variable: its subprogram (vpiScope), its
 * typespec and, for a class variable, the object it holds. 37.39 gives an
 * automatic no parent object (NULL, as for other simulators). */
static vpiHandle rts_auto_handle(int32_t type, const llg_vpi_slot_t* slot) {
    if (rts_is_part(slot)) return rts_part_handle(type, slot);
    rts_frame_t* frame = NULL;
    const llg_vpi_auto_t* entry = rts_auto(slot, &frame);
    switch (type) {
        case vpiScope: {
            uint32_t row = rts_frame_row(frame);
            if (row != LLG_VPI_NONE) return object_handle(row);
            return NULL;
        }
        case vpiParent: return NULL;
        case vpiTypespec:
            return declared_typespec_handle(typespec_ref(entry->typespec), LLG_VPI_NONE, 0);
        case vpiClassObj: {
            dyn_place_t place;
            if (!rts_auto_place(slot, &place) || !place.shape ||
                place.shape->shape != LLG_VPI_SHAPE_CLASS)
                break;
            if (!place.storage) {
                vpi_error("LLG_VPI_VALUE", "this automatic variable is not declared yet in its frame");
                return NULL;
            }
            void* object = *(void**)place.storage;
            return object ? make_shape_object(place.shape, object) : NULL;
        }
        default: break;
    }
    vpi_error("LLG_VPI_RELATION", "this object has no such relation");
    return NULL;
}

/* ── automatics by name ── */

/* The live frame an automatic's name `name` reaches: the innermost frame of
 * the running thread whose full name prefixes it (then a `.`), else the
 * most recently entered such frame of any thread. *rest receives the text
 * after the `.`. */
static rts_frame_t* rts_frame_for_name(const char* name, const char** rest) {
    uint64_t current_identity = 0;
    int32_t current = llg_rt_current_thread(&current_identity);
    rts_frame_t* best = NULL;
    for (int32_t pass = 0; pass < 2 && !best; ++pass) {
        for (uint32_t i = 0; i < rts.stack_count; ++i) {
            rts_stack_t* stack = &rts.stacks[i];
            int32_t thread_slot = (int32_t)i - 1;
            if (!stack->depth) continue;
            int32_t mine = thread_slot == current && stack->identity == current_identity;
            if (pass == 0 ? !mine : mine) continue;
            if (thread_slot >= 0 && !rts_thread_valid(thread_slot, stack->identity)) continue;
            for (uint32_t d = stack->depth; d-- > 0;) {
                rts_frame_t* frame = &stack->frames[d];
                size_t length = strlen(frame->name);
                if (!rts_auto_table(frame) || strncmp(name, frame->name, length) != 0 ||
                    name[length] != '.')
                    continue;
                if (!best || frame->serial > best->serial) best = frame;
                /* The innermost match of this thread wins over its outer ones. */
                break;
            }
        }
    }
    if (best) *rest = name + strlen(best->name) + 1u;
    return best;
}

static vpiHandle rts_auto_by_name(const char* name);

/* The entry of the block local of `frame` whose block path `rest` starts
 * with (then ends, or a `.` or `[` follows), the longest one, while its
 * block has bound it; LLG_VPI_NONE without one. */
static LLG_OUTLYING uint32_t rts_auto_block_by_name(const rts_frame_t* frame, const char* rest) {
    const llg_vpi_auto_t* table = rts_auto_table(frame);
    uint32_t found = LLG_VPI_NONE;
    size_t best = 0;
    for (uint32_t i = 0; table[i].name; ++i) {
        if (!(table[i].flags & LLG_VPI_AUTO_BLOCK)) continue;
        size_t length = strlen(table[i].name);
        if (length <= best || strncmp(table[i].name, rest, length) != 0) continue;
        char next = rest[length];
        if (next != '\0' && next != '.' && next != '[') continue;
        if (!frame->slots[LLG_VPI_AUTOS_FIRST + i]) continue;
        found = i;
        best = length;
    }
    return found;
}

/* A statement scope's declarations (37.12) with, while a frame of its
 * subprogram runs that has bound them, the block locals the scope declares
 * (decision A19-D88): those of the innermost such frame of the running
 * thread, else of the most recently entered one. */
static LLG_OUTLYING vpiHandle rts_block_scope_iterate(int32_t type, uint32_t row) {
    const char* rest = NULL;
    rts_frame_t* frame = llg_vpi_frames_on && rts.stack_count
                             ? rts_frame_for_name(g_vpi.objects[row].full_name, &rest)
                             : NULL;
    if (!frame) return iterate_children(type, row);
    const llg_vpi_auto_t* table = rts_auto_table(frame);
    size_t prefix = strlen(rest);
    rts_handles_t list = {0};
    for (uint32_t child = g_vpi.first_child[row]; child; child = g_vpi.next_sibling[child - 1u])
        if (child_matches(&g_vpi.objects[child - 1u], type))
            rts_push(&list, object_handle(child - 1u));
    uint64_t serial = frame->serial;
    for (uint32_t i = 0; table[i].name; ++i) {
        const llg_vpi_auto_t* entry = &table[i];
        if (!(entry->flags & LLG_VPI_AUTO_BLOCK) || strncmp(entry->name, rest, prefix) != 0 ||
            entry->name[prefix] != '.' || strchr(entry->name + prefix + 1u, '.') ||
            !frame->slots[LLG_VPI_AUTOS_FIRST + i])
            continue;
        llg_vpi_model_object_t shown;
        memset(&shown, 0, sizeof(shown));
        shown.type = entry->type;
        if (!child_matches(&shown, type)) continue;
        rts_push(&list, rts_auto_handle_by_serial(serial, i));
    }
    return rts_list(type, list.items, list.count, list.failed);
}

/* vpi_handle_by_name relative to an automatic variable or a part of it (a
 * member name in a structure); NULL without an error when it has none. */
static vpiHandle rts_auto_by_scoped_name(const llg_vpi_slot_t* scope, const char* name) {
    if (!rts_alive(scope)) return NULL;
    int32_t handled = 0;
    const char* base = rts_get_str(vpiFullName, (llg_vpi_slot_t*)scope, &handled);
    if (!base) return NULL;
    size_t length = strlen(base) + 1u + strlen(name) + 1u;
    char* full = (char*)vpi_malloc(length);
    if (!full) {
        vpi_error("LLG_VPI_NOMEM", "VPI name allocation failed");
        return NULL;
    }
    snprintf(full, length, "%s.%s", base, name);
    vpiHandle result = rts_auto_by_name(full);
    free(full);
    return result;
}

/* vpi_handle_by_name of an automatic variable of a live frame (37.39), an
 * element of it (`tb.t.arr[1]`, `tb.t.q[0]`) or a member (`tb.t.r.a`,
 * `tb.t.pa[1].h`), decision A19-D83; NULL without an error when no live
 * frame has it. */
static vpiHandle rts_auto_by_name(const char* name) {
    if (!llg_vpi_frames_on || !rts.stack_count) return NULL;
    const char* rest = NULL;
    rts_frame_t* frame = rts_frame_for_name(name, &rest);
    if (!frame) return NULL;
    size_t length = strcspn(rest, ".[");
    const llg_vpi_auto_t* table = rts_auto_table(frame);
    uint32_t index = 0;
    while (table[index].name &&
           ((table[index].flags & LLG_VPI_AUTO_BLOCK) || strlen(table[index].name) != length ||
            strncmp(table[index].name, rest, length) != 0))
        ++index;
    if (!table[index].name) {
        /* A block local by its block path (`blk.x`), while its block runs
         * (decision A19-D88). */
        index = rts_auto_block_by_name(frame, rest);
        if (index == LLG_VPI_NONE) return NULL;
        length = strlen(table[index].name);
    }
    const char* p = rest + length;
    dyn_place_t place;
    if (*p == '[' && rts_auto_container(frame->serial, index, &place))
        /* A container automatic: its elements, nested elements and
         * structure members are dynamic elements. */
        return dyn_auto_suffix(&place, index, p);
    if (*p == '.' && rts_auto_value_place(frame->serial, index, &place) &&
        place.shape->shape == LLG_VPI_SHAPE_RECORD)
        /* A record automatic: its members are dynamic elements. */
        return dyn_record_suffix(&place, index, p);
    vpiHandle handle = rts_auto_handle_by_serial(frame->serial, index);
    if (*p == '[') {
        int64_t indices[16];
        uint32_t count = 0;
        while (*p == '[' && count < 16u) {
            char* end = NULL;
            long long value = strtoll(p + 1, &end, 10);
            if (!end || end == p + 1 || *end != ']') {
                release_handle(handle);
                return NULL;
            }
            indices[count++] = (int64_t)value;
            p = end + 1;
        }
        llg_vpi_slot_t* slot = handle ? decode_handle(handle, NULL) : NULL;
        vpiHandle element = NULL;
        if (slot && *p != '[') {
            if (rts_auto_fixed(rts_auto(slot, NULL)))
                element = rts_auto_by_multi_index(slot, indices, count);
            else if (count == 1u && indices[0] >= INT32_MIN && indices[0] <= INT32_MAX)
                element = rts_auto_by_index(slot, (PLI_INT32)indices[0]);
        }
        release_handle(handle);
        handle = element;
    }
    while (handle && *p == '.') {
        const char* member = ++p;
        p += strcspn(p, ".[");
        llg_vpi_slot_t* slot = decode_handle(handle, NULL);
        rts_part_t part;
        uint32_t first = 0, count = 0, found = 0;
        if (slot && slot->kind == SLOT_AUTO && rts_part(slot, &part)) count = rts_part_members(&part, &first);
        for (uint32_t i = 0; i < count && !found; ++i) {
            const char* candidate = node_at(first + i)->name;
            if (strlen(candidate) == (size_t)(p - member) &&
                strncmp(candidate, member, (size_t)(p - member)) == 0)
                found = first + i + 1u;
        }
        vpiHandle next = found ? rts_part_make(slot, slot->count, found) : NULL;
        release_handle(handle);
        handle = next;
        if (handle && *p == '[') {
            /* An element of an unpacked array member: all its indices. */
            int64_t indices[16];
            uint32_t given = 0;
            while (*p == '[' && given < 16u) {
                char* end = NULL;
                long long value = strtoll(p + 1, &end, 10);
                if (!end || end == p + 1 || *end != ']') break;
                indices[given++] = (int64_t)value;
                p = end + 1;
            }
            vpiHandle element = *p == '[' ? NULL : rts_auto_by_multi_index(decode_handle(handle, NULL), indices, given);
            release_handle(handle);
            handle = element;
        }
    }
    if (handle && *p) {
        release_handle(handle);
        return NULL;
    }
    return handle;
}

/* ── frame and thread callbacks ── */

static int32_t rts_cb_reason(int32_t reason) {
    return reason == cbStartOfFrame || reason == cbEndOfFrame || reason == cbStartOfThread ||
           reason == cbEndOfThread || reason == cbEnterThread;
}

/* 38.36.1: cbStartOfFrame is placed on a task or function (the frame does
 * not exist yet), cbEndOfFrame on a frame, cbEndOfThread and cbEnterThread
 * on a thread; cbStartOfThread takes no object. */
static int32_t rts_bind_cb(llg_vpi_cb_t* cb, const s_cb_data* data) {
    int32_t reason = data->reason;
    cb->thread_slot = -1;
    if (reason == cbStartOfThread) {
        if (data->obj) {
            vpi_error("LLG_VPI_CALLBACK", "cbStartOfThread takes no object");
            return 0;
        }
        return 1;
    }
    if (!data->obj) {
        vpi_error("LLG_VPI_CALLBACK", reason == cbStartOfFrame ? "cbStartOfFrame needs a task or function handle"
                                      : reason == cbEndOfFrame ? "cbEndOfFrame needs a frame handle"
                                                               : "this thread callback needs a thread handle");
        return 0;
    }
    llg_vpi_slot_t* slot = checked_handle(data->obj, NULL);
    if (!slot) return 0;
    if (reason == cbStartOfFrame) {
        const llg_vpi_model_object_t* object =
            slot->kind == SLOT_OBJECT ? row_object(slot->object) : NULL;
        if (!object || (object->type != vpiTask && object->type != vpiFunction)) {
            vpi_error("LLG_VPI_CALLBACK", "cbStartOfFrame needs a task or function handle");
            return 0;
        }
        cb->target.object = slot->object;
        cb->target.element = LLG_VPI_NO_ELEMENT;
        return 1;
    }
    int32_t wanted = reason == cbEndOfFrame ? SLOT_FRAME : SLOT_THREAD;
    if (slot->kind != wanted) {
        vpi_error("LLG_VPI_CALLBACK", reason == cbEndOfFrame ? "cbEndOfFrame needs a frame handle"
                                                             : "this thread callback needs a thread handle");
        return 0;
    }
    if (!rts_alive(slot)) return 0;
    cb->thread_slot = slot->aux;
    cb->thread_identity = (uint64_t)slot->left;
    cb->frame_serial = slot->kind == SLOT_FRAME ? slot->element : 0u;
    return 1;
}

static void rts_cb_added(const llg_vpi_cb_t* cb) {
    switch (cb->data.reason) {
        case cbStartOfThread: ++rts.start_thread_cbs; break;
        case cbStartOfFrame: ++rts.start_frame_cbs; break;
        case cbEndOfFrame: ++rts.end_frame_cbs; break;
        case cbEnterThread:
            llg_rt_thread_watch_enter(cb->thread_slot, cb->thread_identity, 1);
            break;
        default: break;
    }
}

static void rts_cb_removed(const llg_vpi_cb_t* cb) {
    switch (cb->data.reason) {
        case cbStartOfThread: --rts.start_thread_cbs; break;
        case cbStartOfFrame: --rts.start_frame_cbs; break;
        case cbEndOfFrame: --rts.end_frame_cbs; break;
        case cbEnterThread: {
            int32_t watched = 0;
            for (const llg_vpi_cb_t* other = g_vpi.cbs; other && !watched; other = other->next)
                watched = !other->removed && other->kind == CB_RUNTIME &&
                          other->data.reason == cbEnterThread &&
                          other->thread_slot == cb->thread_slot &&
                          other->thread_identity == cb->thread_identity;
            if (!watched) llg_rt_thread_watch_enter(cb->thread_slot, cb->thread_identity, 0);
            break;
        }
        default: break;
    }
}

typedef struct {
    int32_t reason;
    int32_t slot;
    uint64_t identity;
    uint64_t serial;
    uint32_t row;
} rts_cb_match_t;

static int32_t rts_matches(const llg_vpi_cb_t* cb, const void* context) {
    const rts_cb_match_t* match = (const rts_cb_match_t*)context;
    if (cb->kind != CB_RUNTIME || cb->data.reason != match->reason) return 0;
    switch (match->reason) {
        case cbStartOfThread: return 1;
        case cbStartOfFrame: return cb->target.object == match->row;
        case cbEndOfFrame:
            return cb->thread_slot == match->slot && cb->thread_identity == match->identity &&
                   cb->frame_serial == match->serial;
        default: return cb->thread_slot == match->slot && cb->thread_identity == match->identity;
    }
}

/* Every live record on the thread (or, with `serial`, only on that frame)
 * is released with it (37.2.2 item 2). */
static int32_t rts_on_ended(const llg_vpi_cb_t* cb, const void* context) {
    const rts_cb_match_t* match = (const rts_cb_match_t*)context;
    return cb->kind == CB_RUNTIME && cb->data.reason != cbStartOfThread &&
           cb->data.reason != cbStartOfFrame && cb->thread_slot == match->slot &&
           cb->thread_identity == match->identity &&
           (!match->serial || cb->frame_serial == match->serial);
}

static void rts_dispatch(const rts_cb_match_t* match) {
    llg_vpi_cb_t* buffer[LLG_VPI_DISPATCH_INLINE];
    size_t count;
    dispatch_begin();
    llg_vpi_cb_t** items = collect_cbs(g_vpi.cbs, rts_matches, match, buffer, &count);
    for (size_t i = 0; i < count; ++i) {
        llg_vpi_cb_t* cb = items[i];
        if (cb->removed) continue;
        s_cb_data data;
        s_vpi_time time;
        base_data(cb, &data);
        data.time = callback_time(cb->time.type, NULL, llg_time(), &time);
        invoke_cb(cb, &data);
    }
    if (items != buffer) free(items);
    dispatch_end();
}

static void rts_release(const rts_cb_match_t* match) {
    llg_vpi_cb_t* buffer[LLG_VPI_DISPATCH_INLINE];
    size_t count;
    dispatch_begin();
    llg_vpi_cb_t** items = collect_cbs(g_vpi.cbs, rts_on_ended, match, buffer, &count);
    for (size_t i = 0; i < count; ++i) vpi_cb_retire(items[i]);
    if (items != buffer) free(items);
    dispatch_end();
}

/* The innermost frame of (slot, identity) ends: cbEndOfFrame runs while
 * its handles are still valid, then the frame and its records go. */
static void rts_pop_frame(int32_t slot, uint64_t identity) {
    rts_stack_t* stack = rts_stack(slot, identity, 0);
    if (!stack || !stack->depth) return;
    uint64_t serial = stack->frames[stack->depth - 1].serial;
    if (rts.end_frame_cbs) {
        rts_cb_match_t match = {cbEndOfFrame, slot, identity, serial, 0};
        rts_dispatch(&match);
        rts_release(&match);
        stack = rts_stack(slot, identity, 0);
        if (!stack || !stack->depth) return;
    }
    stack->depth--;
}

/* Grow a frame stack to `capacity` records, keeping RTS_FRAME_ALIGN. */
static int32_t rts_grow(rts_stack_t* stack, uint32_t capacity) {
    if ((uint64_t)capacity * sizeof(rts_frame_t) > (uint64_t)(SIZE_MAX - RTS_FRAME_ALIGN)) return 0;
    void* block = vpi_malloc((size_t)capacity * sizeof(rts_frame_t) + RTS_FRAME_ALIGN);
    if (!block) return 0;
    uintptr_t address = ((uintptr_t)block + RTS_FRAME_ALIGN - 1u) & ~(uintptr_t)(RTS_FRAME_ALIGN - 1u);
    rts_frame_t* frames = (rts_frame_t*)(void*)address;
    if (stack->depth) memcpy(frames, stack->frames, stack->depth * sizeof(*frames));
    free(stack->block);
    stack->block = block;
    stack->frames = frames;
    stack->capacity = capacity;
    return 1;
}

static uint64_t rts_enter(const char* full_name, void** slots) {
    uint32_t origin = llg_vpi_next_origin;
    llg_vpi_next_origin = 0u;
    uint64_t identity;
    int32_t slot = llg_rt_current_thread(&identity);
    rts_stack_t* stack = rts_stack(slot, identity, 1);
    if (!stack) return 0;
    if (stack->depth == stack->capacity && !rts_grow(stack, stack->capacity ? stack->capacity * 2u : 8u))
        return 0;
    rts_frame_t* frame = &stack->frames[stack->depth++];
    frame->serial = ++rts.next_serial;
    frame->name = full_name;
    frame->slots = slots;
    frame->row = RTS_ROW_UNRESOLVED;
    frame->origin = origin;
    uint64_t serial = frame->serial;
    if (rts.start_frame_cbs) {
        uint32_t row = rts_frame_row(frame);
        if (row != LLG_VPI_NONE) {
            rts_cb_match_t match = {cbStartOfFrame, slot, identity, serial, row};
            rts_dispatch(&match);
        }
    }
    return serial;
}

uint64_t llg_vpi_frame_enter(const char* full_name) {
    return rts_enter(full_name, NULL);
}

uint64_t llg_vpi_frame_enter_autos(const char* full_name, const llg_vpi_auto_t* autos,
                                   void** slots) {
    /* The model zeroes the array, so body locals have no address until
     * their declarations run. */
    slots[LLG_VPI_AUTOS_TABLE] = (void*)autos;
    return rts_enter(full_name, slots);
}

void llg_vpi_frame_exit(uint64_t serial) {
    uint64_t identity;
    int32_t slot = llg_rt_current_thread(&identity);
    for (;;) {
        rts_stack_t* stack = rts_stack(slot, identity, 0);
        if (!stack || !stack->depth || stack->frames[stack->depth - 1].serial < serial) return;
        rts_pop_frame(slot, identity);
    }
}

static void rts_thread_event(int32_t slot, uint64_t identity, int32_t event) {
    if (event == LLG_RT_THREAD_START) {
        if (rts.start_thread_cbs) {
            rts_cb_match_t match = {cbStartOfThread, slot, identity, 0, 0};
            rts_dispatch(&match);
        }
        return;
    }
    if (event == LLG_RT_THREAD_ENTER) {
        rts_cb_match_t match = {cbEnterThread, slot, identity, 0, 0};
        rts_dispatch(&match);
        return;
    }
    /* 38.36.1: the frames of an ending thread have ended before
     * cbEndOfThread runs. */
    int32_t was_dying = rts.dying, old_slot = rts.dying_slot;
    uint64_t old_identity = rts.dying_identity;
    rts.dying = 1;
    rts.dying_slot = slot;
    rts.dying_identity = identity;
    for (;;) {
        rts_stack_t* stack = rts_stack(slot, identity, 0);
        if (!stack || !stack->depth) break;
        rts_pop_frame(slot, identity);
    }
    rts_cb_match_t match = {cbEndOfThread, slot, identity, 0, 0};
    rts_dispatch(&match);
    rts_release(&match);
    rts.dying = was_dying;
    rts.dying_slot = old_slot;
    rts.dying_identity = old_identity;
    rts_stack_t* stack = rts_stack(slot, identity, 0);
    if (stack) stack->identity = 0;
}

/* ── frame and thread queries ── */

static PLI_INT64 rts_runtime_get(int32_t property, llg_vpi_slot_t* slot) {
    if (slot->kind == SLOT_THREAD) {
        if (property == vpiActive) return 1;
    } else {
        uint32_t depth = 0;
        rts_frame_t* frame = rts_frame_find(slot->aux, (uint64_t)slot->left, slot->element, &depth);
        rts_stack_t* stack = rts_stack(slot->aux, (uint64_t)slot->left, 0);
        /* 37.39 detail 4: one active frame per thread, the innermost. */
        if (property == vpiActive) return stack && depth + 1u == stack->depth;
        if (property == vpiLineNo) {
            const llg_vpi_model_object_t* object = row_object(rts_frame_row(frame));
            return object ? object->line : 0;
        }
    }
    return unsupported_property();
}

static const char* rts_runtime_get_str(int32_t property, llg_vpi_slot_t* slot) {
    if (slot->kind == SLOT_FRAME) {
        rts_frame_t* frame = rts_frame_find(slot->aux, (uint64_t)slot->left, slot->element, NULL);
        if (property == vpiFullName) return frame->name;
        if (property == vpiFile) {
            const llg_vpi_model_object_t* object = row_object(rts_frame_row(frame));
            if (object) return object->file;
        }
    }
    vpi_error("LLG_VPI_PROPERTY", "this object has no such string property");
    return NULL;
}

static vpiHandle rts_runtime_handle(int32_t type, llg_vpi_slot_t* slot) {
    if (slot->kind == SLOT_AUTO) return rts_auto_handle(type, slot);
    int32_t thread_slot = slot->aux;
    uint64_t identity = (uint64_t)slot->left;
    if (slot->kind == SLOT_THREAD) {
        if (type == vpiParent) {
            int32_t parent_slot;
            uint64_t parent_identity;
            if (!llg_rt_thread_parent(thread_slot, identity, &parent_slot, &parent_identity))
                return NULL;
            return rts_thread_handle(parent_slot, parent_identity);
        }
        /* The statement that started a thread is not recorded; an initial
         * or always procedure has none. */
        if (type == vpiOrigin) return NULL;
        if (type == vpiFrame) return rts_top_frame(thread_slot, identity);
    } else {
        uint32_t depth = 0;
        rts_frame_find(thread_slot, identity, slot->element, &depth);
        rts_stack_t* stack = rts_stack(thread_slot, identity, 0);
        if (type == vpiParent) {
            if (!depth) return NULL;
            return rts_frame_handle(thread_slot, identity, stack->frames[depth - 1].serial);
        }
        if (type == vpiThread) {
            if (thread_slot < 0 || !rts_thread_valid(thread_slot, identity)) return NULL;
            return rts_thread_handle(thread_slot, identity);
        }
        if (type == vpiStmt) {
            /* 37.39 detail 4: the active statement of the innermost frame of
             * the running thread while it runs a system task or function
             * call is that call. A frame that called another runs the
             * statement holding the call that activated it (decision
             * A19-D75); a suspended innermost frame names none llg records. */
            if (depth + 1u < stack->depth) {
                uint32_t origin = stack->frames[depth + 1u].origin;
                uint32_t statement = origin ? rts_call_statement(origin - 1u) : LLG_VPI_NONE;
                return statement == LLG_VPI_NONE ? NULL : code_handle(statement);
            }
            uint64_t current;
            int32_t current_slot = llg_rt_current_thread(&current);
            if (!g_vpi.active_call || current_slot != thread_slot || current != identity)
                return NULL;
            /* A handle of its own: releasing it leaves the call's
             * vpiSysTfCall handle valid. */
            uint32_t index = alloc_slot(SLOT_CALL);
            if (index == LLG_VPI_NONE) return NULL;
            slot_at(index)->pointer = g_vpi.active_call->site;
            return encode_handle(index);
        }
        if (type == vpiOrigin) {
            /* 37.39: the task, function or method call that activated the
             * frame; NULL for one no source call activated (a DPI export
             * called from C, a function a continuous assignment evaluates). */
            rts_frame_t* frame = &stack->frames[depth];
            if (!frame->origin || !code_at(frame->origin - 1u)) return NULL;
            return code_handle(frame->origin - 1u);
        }
    }
    vpi_error("LLG_VPI_RELATION", "this object has no such relation");
    return NULL;
}

static vpiHandle rts_thread_list(int32_t parent_slot, uint64_t parent_identity) {
    int32_t* slots;
    uint64_t* identities;
    size_t count = llg_rt_threads(parent_slot, parent_identity, &slots, &identities);
    rts_handles_t list = {0};
    for (size_t i = 0; i < count; ++i) rts_push(&list, rts_thread_handle(slots[i], identities[i]));
    free(slots);
    free(identities);
    return rts_list(vpiThread, list.items, list.count, list.failed);
}

/* 37.25 detail 1: the threads waiting on a named event. */
static vpiHandle rts_event_waiters(const llg_event_t* event) {
    int32_t* slots;
    uint64_t* identities;
    size_t count = llg_rt_event_waiters(event, &slots, &identities);
    rts_handles_t list = {0};
    for (size_t i = 0; i < count; ++i) rts_push(&list, rts_thread_handle(slots[i], identities[i]));
    free(slots);
    free(identities);
    return rts_list(vpiThread, list.items, list.count, list.failed);
}

static void rts_runtime_shutdown(void) {
    if (rts.hooked) llg_rt_set_thread_hook(NULL);
    for (uint32_t i = 0; i < rts.stack_count; ++i) free(rts.stacks[i].block);
    free(rts.stacks);
    memset(&rts, 0, sizeof(rts));
    llg_vpi_frames_on = 0;
}

/* ── statement processes (37.56, 37.58) ──────────────────────────────────── */

/* The catalog row + 1 of the process whose body is root code row `row`,
 * indexed on the first vpiProcess query; a root owned by a task, a function
 * or nothing has 0. */
static uint32_t* rts_code_owners;

/* The atomic statements of 37.56: every statement object except blocks,
 * repeat controls and concurrent assertions. A function call, though in
 * the tf call class, is an expression and has no process (decision
 * A19-D62). */
static int32_t rts_atomic_statement(int32_t type) {
    switch (type) {
        case vpiBegin: case vpiFork: case vpiNamedBegin: case vpiNamedFork:
        case vpiRepeatControl: case vpiAssert: case vpiAssume: case vpiCover:
            return 0;
        default:
            return code_is_statement(type);
    }
}

static int32_t rts_process_owners(void) {
    if (rts_code_owners) return 1;
    if (!rts_code_parents()) return 0;
    uint32_t* owners = (uint32_t*)vpi_calloc(g_vpi.code->node_count, sizeof(*owners));
    if (!owners) return 0;
    for (size_t row = 0; row < g_vpi.object_count; ++row) {
        const llg_vpi_model_object_t* object = &g_vpi.objects[row];
        int32_t process = object->type == vpiInitial || object->type == vpiAlways ||
                      object->type == vpiFinal;
        if (!process || !object->code || object->code > g_vpi.code->node_count) continue;
        /* A body may be a link to a shared root. */
        owners[object->code - 1u] = (uint32_t)row + 1u;
        owners[code_visible(object->code - 1u)] = (uint32_t)row + 1u;
    }
    rts_code_owners = owners;
    return 1;
}

/* vpiProcess of an atomic statement (decision A19-D62): the initial,
 * always or final construct whose body contains it; NULL without an error
 * for a statement of a task or function, which no process owns. `handled`
 * stays 0 for other objects. */
static vpiHandle rts_statement_process(const llg_vpi_slot_t* slot, int32_t* handled) {
    uint32_t row = LLG_VPI_NONE;
    if (slot->kind == SLOT_CODE) {
        row = code_visible((uint32_t)slot->aux);
        const llg_vpi_code_t* node = code_at(row);
        if (!node || (node->flags & LLG_VPI_CODE_ALIAS) || !rts_atomic_statement(node->type))
            return NULL;
    } else if (slot->kind == SLOT_OBJECT) {
        /* A for or foreach statement that declares its loop variables is a
         * scope (37.12 details 2 and 3): a catalog row with its code. */
        const llg_vpi_model_object_t* object = row_object(slot->object);
        if (!object || (object->type != vpiFor && object->type != vpiForeachStmt) ||
            !object->code || !g_vpi.code || object->code > g_vpi.code->node_count)
            return NULL;
        row = object->code - 1u;
    } else {
        return NULL;
    }
    *handled = 1;
    if (!rts_process_owners()) {
        vpi_error("LLG_VPI_NOMEM", "VPI statement process index allocation failed");
        return NULL;
    }
    while (g_vpi.code_parents[row]) row = g_vpi.code_parents[row] - 1u;
    uint32_t owner = rts_code_owners[row];
    return owner ? object_handle(owner - 1u) : NULL;
}

/* ── dispatch ───────────────────────────────────────────────────────────── */

static PLI_INT64 rts_get(int32_t property, llg_vpi_slot_t* slot, int32_t* handled) {
    *handled = 1;
    if (rts_runtime_slot(slot)) {
        if (!rts_alive(slot)) return vpiUndefined;
        if (property == vpiType) {
            *handled = 0;
            return 0;
        }
        if (slot->kind == SLOT_AUTO) return rts_auto_get(property, slot);
        switch (property) {
            case vpiActive:
            case vpiLineNo: return rts_runtime_get(property, slot);
            default: return unsupported_property();
        }
    }
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
        case SLOT_UDPINIT:
            /* 37.32: `initial q = 1'b1;` is an initial process whose
             * statement is a blocking assignment of a one-bit binary
             * constant (decision A19-D68). */
            if (property == vpiLineNo) return udp->line;
            if (slot->left == 1 && property == vpiOpType) return vpiAssignmentOp;
            if (slot->left == 1 && property == vpiBlocking) return 1;
            if (slot->left == 2 && property == vpiConstType) return vpiBinaryConst;
            if (slot->left == 2 && property == vpiSize) return 1;
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

static const char* rts_get_str(int32_t property, llg_vpi_slot_t* slot, int32_t* handled) {
    *handled = 1;
    if (rts_runtime_slot(slot)) {
        if (!rts_alive(slot)) return NULL;
        /* The type names of runtime objects include the variable types of
         * automatics. */
        if (property == vpiType) return dyn_type_name(rts_type(slot));
        if (slot->kind == SLOT_AUTO && rts_is_part(slot)) return rts_part_str(property, slot);
        if (slot->kind == SLOT_AUTO) {
            rts_frame_t* frame = NULL;
            const llg_vpi_auto_t* entry = rts_auto(slot, &frame);
            if (property == vpiName) return rts_auto_name(entry);
            if (property == vpiFullName) {
                snprintf(g_vpi.str_buffer, sizeof(g_vpi.str_buffer), "%s.%s", frame->name,
                         entry->name);
                return g_vpi.str_buffer;
            }
            if (property == vpiFile) {
                const llg_vpi_model_object_t* object = row_object(rts_frame_row(frame));
                if (object) return object->file;
            }
            vpi_error("LLG_VPI_PROPERTY", "this object has no such string property");
            return NULL;
        }
        switch (property) {
            case vpiFullName:
            case vpiFile: return rts_runtime_get_str(property, slot);
            default: break;
        }
        vpi_error("LLG_VPI_PROPERTY", "this object has no such string property");
        return NULL;
    }
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
    if (slot->kind == SLOT_AUTO) {
        if (rts_alive(slot)) rts_auto_value(slot, output);
        return;
    }
    if (slot->kind == SLOT_TABLEENTRY) {
        rts_entry_value(slot, output);
        return;
    }
    const llg_vpi_udp_t* udp = rts_udp(slot->aux);
    if (slot->kind == SLOT_UDPINIT && slot->left == 2 && udp) {
        llg_vpi_datum_t datum = {0};
        datum.packed = udp->initial == 'x' ? sv4_x(1u, 0) : sv4_from_u64(udp->initial == '1', 1u, 0);
        fill_value(&datum, 0, vpiConstant, output);
        datum_destroy(&datum);
        return;
    }
    vpi_error("LLG_VPI_VALUE", "this object has no value");
}

/* vpi_handle for the runtime structures; *handled is 0 for a query the
 * other layers answer. */
static vpiHandle rts_handle(int32_t type, vpiHandle reference, int32_t* handled) {
    *handled = 0;
    if (!reference) {
        if (type != vpiFrame) return NULL;
        /* 37.39 detail 4: the active frame of the running thread. */
        *handled = 1;
        if (!vpi_ready()) return NULL;
        uint64_t identity;
        int32_t slot = llg_rt_current_thread(&identity);
        return rts_top_frame(slot, identity);
    }
    llg_vpi_slot_t* slot = decode_handle(reference, NULL);
    if (!slot) return NULL;
    if (rts_runtime_slot(slot)) {
        *handled = 1;
        if (!rts_alive(slot)) return NULL;
        if (slot->kind == SLOT_AUTO) return rts_auto_handle(type, slot);
        switch (type) {
            case vpiParent:
            case vpiOrigin:
            case vpiFrame:
            case vpiThread:
            case vpiStmt: return rts_runtime_handle(type, slot);
            default: break;
        }
        vpi_error("LLG_VPI_RELATION", "this object has no such relation");
        return NULL;
    }
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
    if (type == vpiProcess) {
        vpiHandle process = rts_statement_process(slot, handled);
        if (*handled) return process;
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
        /* 37.32: NULL without an error for a definition with no initial
         * statement. */
        const llg_vpi_udp_t* udp = rts_udp(slot->aux);
        *handled = 1;
        return udp && udp->initial ? rts_make(SLOT_UDPINIT, (uint32_t)slot->aux, 0) : NULL;
    }
    if (slot->kind == SLOT_UDPINIT) {
        *handled = 1;
        if (slot->left == 0 && type == vpiStmt) return rts_make(SLOT_UDPINIT, (uint32_t)slot->aux, 1);
        if (slot->left == 1 && type == vpiRhs) return rts_make(SLOT_UDPINIT, (uint32_t)slot->aux, 2);
        /* The assigned output is the definition's output declaration. */
        if (slot->left == 1 && type == vpiLhs) return rts_make(SLOT_UDPIO, (uint32_t)slot->aux, 0);
    }
    if (rts_slot(slot)) {
        *handled = 1;
        vpi_error("LLG_VPI_RELATION", "this object has no such relation");
    }
    return NULL;
}

static vpiHandle rts_iterate(int32_t type, vpiHandle reference, int32_t* handled) {
    *handled = 1;
    if (!reference) {
        if (type == vpiUserSystf) {
            /* 37.38 detail 6: every registration, in registration order. */
            rts_handles_t list = {0};
            for (int32_t i = 0; i < g_vpi.registration_count; ++i)
                rts_push(&list, encode_handle(g_vpi.registrations[i]->slot));
            return rts_list(vpiUserSystf, list.items, list.count, list.failed);
        }
        if (type == vpiCallback) return rts_callbacks(NULL, NULL);
        if (type == vpiTimeQueue) return rts_time_queues();
        if (type == vpiThread) return rts_thread_list(-1, 0);
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
    if (rts_runtime_slot(slot)) {
        if (!rts_alive(slot)) return NULL;
        if (slot->kind == SLOT_THREAD && type == vpiThread)
            return rts_thread_list(slot->aux, (uint64_t)slot->left);
        if (slot->kind == SLOT_FRAME && type == vpiAutomatics) return rts_automatics(slot);
        if (slot->kind == SLOT_AUTO) return rts_auto_iterate(type, slot);
        vpi_error("LLG_VPI_RELATION", "this object has no such iteration");
        return NULL;
    }
    if (type == vpiWaitingProcesses && slot->kind == SLOT_OBJECT) {
        const llg_vpi_model_object_t* object = row_object(slot->object);
        if (object && object->type == vpiNamedEvent && object->event)
            return rts_event_waiters(object->event);
    }
    if (type == vpiWaitingProcesses && slot->kind == SLOT_BUILTINOBJ) {
        /* 37.29 detail 3: the threads waiting on a mailbox or semaphore. */
        int32_t* slots;
        uint64_t* identities;
        size_t count = llg_rt_resource_waiters(slot->pointer, &slots, &identities);
        rts_handles_t list = {0};
        for (size_t i = 0; i < count; ++i)
            rts_push(&list, rts_thread_handle(slots[i], identities[i]));
        free(slots);
        free(identities);
        return rts_list(vpiThread, list.items, list.count, list.failed);
    }
    if (type == vpiWaitingProcesses && slot->kind == SLOT_PROPERTY) {
        /* 37.25 detail 1 for an event property of a class object. */
        dyn_place_t place;
        if (dyn_event_property(slot, &place)) {
            llg_event_t event;
            return dyn_property_event(slot, &event, "vpiWaitingProcesses") ? rts_event_waiters(&event)
                                                                           : NULL;
        }
    }
    if (type == vpiWaitingProcesses && slot->kind == SLOT_DYNELEM && dyn_event_element(slot)) {
        /* 37.25 detail 1 for an event of a named event array property. */
        llg_event_t event;
        return dyn_element_event(slot, &event, "vpiWaitingProcesses") ? rts_event_waiters(&event)
                                                                      : NULL;
    }
    int32_t object_type = handle_type(slot);
    if (type == vpiCallback && rts_callback_owner(object_type)) return rts_callbacks(reference, slot);
    if (type == vpiUse && rts_simple_expr(object_type)) return rts_uses(slot);
    if (slot->kind == SLOT_UDPDEFN) {
        uint32_t defn = (uint32_t)slot->aux;
        const llg_vpi_udp_t* udp = rts_udp(slot->aux);
        rts_handles_t list = {0};
        if (type == vpiIODecl) {
            for (uint32_t i = 0; i < udp->port_count; ++i)
                rts_push(&list, rts_make(SLOT_UDPIO, defn, (int64_t)i));
            return rts_list(vpiIODecl, list.items, list.count, list.failed);
        }
        if (type == vpiTableEntry) {
            for (uint32_t i = 0; i < udp->entry_count; ++i)
                rts_push(&list, rts_make(SLOT_TABLEENTRY, defn, (int64_t)i));
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

static void rts_shutdown(void) {
    rts_runtime_shutdown();
    free(rts_code_owners);
    rts_code_owners = NULL;
}
