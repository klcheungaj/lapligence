// llg_vpi.c — VPI object model, value access and system-task bridge for
// generated models (IEEE 1800-2009 clauses 36-38, Annex K/M).
//
// Handles are opaque encodings of (slot index, generation): the runtime never
// dereferences an application-supplied handle, so a stale, released or
// malformed handle is a checked vpi_chk_error() error instead of a
// use-after-free. Catalog objects live in static model storage (the generated
// catalog and its value cells). Handles into class objects (llg_vpi_dynamic.c)
// never pin them: the collector reports each reclaimed object and every handle
// into it is released first.

#include "llg_vpi.h"
#include "sv_vpi_user.h"

#include <math.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "llg_platform_native.h"

/* Tunables. Catalog rows and live dynamic handles share the slot index
 * space, whose width is LLG_VPI_INDEX_BITS. */
#define LLG_VPI_MAX_OBJECTS (1u << 20)
#define LLG_VPI_MAX_LIVE_HANDLES (1u << 21)
/* Fired one-shot callback handles kept valid until removed (decision
 * S43-D4). Past this count the oldest one is freed and its handle goes
 * stale, which bounds the memory of applications that never remove them. */
#define LLG_VPI_MAX_FIRED_CALLBACKS (1u << 16)
/* Elements a cbValueChange on a whole unpacked array may watch; each costs
 * one watch entry here and one runtime watch row. */
#define LLG_VPI_MAX_ARRAY_WATCH_ELEMENTS (1u << 20)
#define LLG_VPI_MAX_REGISTRATIONS 4096
#define LLG_VPI_MAX_STARTUP_ROUTINES 1024
#define LLG_VPI_MAX_ARGS 256
#define LLG_VPI_MAX_PLUGINS 16
#define LLG_VPI_MAX_CALL_DEPTH 64

#define LLG_VPI_INDEX_BITS (UINTPTR_MAX > UINT32_MAX ? 32u : 22u)
#define LLG_VPI_INDEX_MASK ((((uintptr_t)1) << LLG_VPI_INDEX_BITS) - 1u)
#define LLG_VPI_GENERATION_MASK \
    ((uint32_t)(UINTPTR_MAX >> LLG_VPI_INDEX_BITS) & UINT32_MAX)
#define LLG_VPI_NONE UINT32_MAX
#define LLG_VPI_NO_ELEMENT UINT64_MAX
#define VPI_EMPTY ((sv4_t)SV4_EMPTY)

typedef enum {
    SLOT_FREE = 0,
    SLOT_OBJECT,    /* catalog row */
    SLOT_ELEMENT,   /* array element: object + element */
    SLOT_SELECT,    /* bit/part-select of an object or element */
    SLOT_CONSTANT,  /* integer constant (range bound, index), or a vpiStringConst
                     * with its characters in `items`/`count` */
    SLOT_RANGE,     /* one unpacked range of an array */
    SLOT_ITERATOR,
    SLOT_SYSTF,
    SLOT_CALL,
    SLOT_ARGUMENT,  /* expression or literal argument snapshot */
    SLOT_CALLBACK,
    SLOT_FIRED_CB,  /* one-shot callback that ran or cannot occur: llg_vpi_cb_done_t */
    SLOT_EVENT,     /* vpiSchedEvent */
    SLOT_ASSERTION, /* assertion coverage item `left` (39.3) */
    SLOT_MEMBER,    /* struct/union member: object + element + member node `aux` */
    SLOT_TYPESPEC,  /* typespec `aux`: a static slot, or a dynamic one owned by
                     * declaration row `object` (member node `left` - 1) */
    SLOT_TSMEMBER,  /* typespec member `aux` (static slot) */
    SLOT_ENUMCONST, /* enum constant `aux` (static slot) */
    SLOT_CONN,      /* constant or expression connection `aux` of row `object` */
    SLOT_PRIMTERM,  /* terminal `aux` of primitive row `object` */
    SLOT_PORTBIT,   /* bit `aux` (from the LSB) of port row `object` */
    SLOT_REFOBJ,    /* interface port `object` reference: `aux` 0 low, 1 high */
    SLOT_TERM,      /* path term or tchk term: connection `aux` of row `object` */
    SLOT_STMT,      /* force/release statement `aux` (vpiForce, vpiRelease) of a
                     * running cbForce/cbRelease dispatch: llg_vpi_stmt_t */
    /* Dynamic objects (llg_vpi_dynamic.c). */
    SLOT_CLASSOBJ,  /* class object `pointer` */
    SLOT_PROPERTY,  /* property `aux` of class `right`: of object `pointer`, or of the
                     * class definition when `pointer` is NULL */
    SLOT_DYNELEM,   /* element of the container row `object` or of property `aux` of
                     * object `pointer`, named by index or key (`lsb`, `element`, `items`) */
    SLOT_CLASSDEFN, /* class definition `aux` (static slot) */
    SLOT_CLASSTS,   /* class typespec `aux` (static slot) */
    SLOT_EXTENDS,   /* extends object of class `aux` (static slot) */
    SLOT_CLASSITEM, /* declared item `aux` of class `right` (static slot): a method, an
                     * argument, a parameter or a constraint */
    SLOT_CODE,      /* expression or statement object: code row `aux` */
    SLOT_COLLECTION, /* vpiCollection: `count` members in `items`, each a
                      * catalog row << 1 or an assertion item << 1 | 1 */
    SLOT_FSM,        /* FSM `left` of the FSM coverage table (40.5.3) */
    SLOT_FSMSTATE,   /* state `aux` of FSM `left` (40.5.3) */
    SLOT_IMP,        /* vpiInterModPath (37.33): from port row `left` (bit `right`, -1:
                      * the whole port) to load port row `object` (bit `aux`, -1: whole) */
    SLOT_CABIT,      /* bit `aux` (vpiOffset, from the LSB) of continuous assignment
                      * row `object` (vpiContAssignBit, 37.43) */
    /* Runtime structures (llg_vpi_runtime.c). */
    SLOT_TIMEQ,      /* time queue of simulation time `left` (37.76) */
    SLOT_UDPDEFN,    /* UDP definition `aux` (static slot, 37.32) */
    SLOT_UDPIO,      /* port `left` of UDP definition `aux` */
    SLOT_TABLEENTRY, /* table entry `left` of UDP definition `aux` */
    SLOT_THREAD,     /* thread: registry slot `aux`, process identity `left` (37.40) */
    SLOT_FRAME,      /* frame `element` of the frame stack of (`aux`, `left`) (37.39) */
} llg_vpi_slot_kind_t;

enum {
    LLG_VPI_PHASE_STARTUP = 0, /* startup routines and compiletf */
    LLG_VPI_PHASE_COMPILED,    /* cbEndOfCompile ran */
    LLG_VPI_PHASE_RUNNING,     /* cbStartOfSimulation ran */
    LLG_VPI_PHASE_ENDED,       /* cbEndOfSimulation ran */
};

typedef enum {
    ITER_LIST = 0,  /* precomputed handle list */
    ITER_ELEMENTS,  /* array elements, created on scan */
    ITER_BITS,      /* vector bits, created on scan */
    ITER_ARGUMENTS, /* arguments of the active call, created on scan */
    ITER_MEMBERS,   /* member nodes [left, left + count), created on scan */
    ITER_DYN,       /* properties, container elements, derived classes (llg_vpi_dynamic.c) */
    ITER_CODE,      /* code rows [left, left + count) whose role is `aux` */
} llg_vpi_iter_kind_t;

struct llg_vpi_callsite;
struct llg_vpi_registration;
struct llg_vpi_event;
struct llg_vpi_cb;
struct llg_vpi_watch;

typedef struct {
    uint32_t generation;
    uint8_t kind;
    uint8_t iter_kind;
    uint8_t owned;            /* invalidated when its call invocation ends */
    uint8_t pad;
    uint32_t object;          /* catalog row */
    uint32_t next_owned;      /* call-owned list: slot index + 1 */
    uint64_t element;         /* element index or LLG_VPI_NO_ELEMENT */
    uint32_t lsb;             /* select storage offset */
    uint32_t span;            /* select width; 0 selects the whole value */
    int64_t left;             /* select bounds, constant value, range bounds */
    int64_t right;
    int32_t aux;              /* iterator type, constant type, argument index */
    uint32_t count;           /* iterator length */
    uint32_t position;        /* iterator cursor */
    uintptr_t* items;         /* ITER_LIST handles */
    void* pointer;            /* registration, callsite, event, callback */
    uintptr_t use;            /* an iterator's reference handle (vpiUse, 37.79) */
} llg_vpi_slot_t;

typedef struct llg_vpi_registration {
    s_vpi_systf_data data;
    char* name;
    uint32_t slot;            /* its vpiUserSystf handle */
    int size_known;
    uint32_t size;
} llg_vpi_registration_t;

struct llg_vpi_call;

typedef struct llg_vpi_callsite {
    uint64_t id;
    const char* name;
    llg_vpi_registration_t* registration;
    uint64_t time_unit_fs;
    uint32_t result_width;
    int8_t result_signed;
    int8_t result_real;
    int valid;
    int arg_count;
    llg_vpi_compile_arg_t* args;
    llg_vpi_model_object_t* scope;
    const char* file;
    int line;
    void* userdata;
    uint32_t call_slot;       /* persistent vpiSysTfCall handle: index + 1 */
    struct llg_vpi_call* active;
} llg_vpi_callsite_t;

typedef struct llg_vpi_call {
    llg_vpi_callsite_t* site;
    llg_vpi_arg_t* args;
    int arg_count;
    sv4_t return_value;
    double real_return;
    int has_real_return;
    int is_function;
    uint32_t owned;           /* first call-owned slot: index + 1 */
    struct llg_vpi_call* previous;
} llg_vpi_call_t;

typedef struct {
    uint32_t object;
    uint64_t element;
    uint32_t lsb;
    uint32_t span;            /* 0: the whole value */
    int8_t is_signed;         /* the value read through this target */
    int8_t two_state;         /* writes convert x/z to 0 */
} llg_vpi_target_t;

typedef struct llg_vpi_event {
    llg_vpi_target_t target;
    int is_real;
    sv4_t value;
    double real;
    uint64_t time;
    llg_delayed_call_id_t id; /* the carrier process while pending */
    uint32_t slot;            /* vpiSchedEvent handle: index + 1, or 0 */
    int pending;
    struct llg_vpi_event* next;
    struct llg_vpi_event** prev_link;
} llg_vpi_event_t;

/* The force or release statement a cbForce/cbRelease dispatch reports. It
 * lives on the dispatching stack; its SLOT_STMT handle is released before
 * the dispatch returns. */
typedef struct {
    int release;
    const llg_force_part_t* parts;
    int n_parts;
    double* real;
} llg_vpi_stmt_t;

/* A catalog row that owns storage, by address (cbForce/cbRelease reverse
 * lookup from a force part to the object it writes). */
typedef struct {
    uintptr_t begin;
    uint64_t count;           /* elements */
    size_t size;              /* bytes per element */
    uint32_t row;
} llg_vpi_storage_t;

/* A value read from a VPI-visible source, independently owned. */
typedef struct {
    int is_real;
    double real;
    sv4_t packed;
} llg_vpi_datum_t;

typedef struct {
    const char* design_name;
    llg_vpi_model_object_t* objects;
    size_t object_count;
    uint32_t model_generation;
    int indexed;              /* catalog validated and indexed */
    uint32_t* sorted;         /* name-ordered rows (ports excluded) */
    size_t sorted_count;
    uint32_t* first_child;    /* row + 1 */
    uint32_t* next_sibling;   /* row + 1 */
    uint64_t* element_counts;
    const llg_vpi_type_tables_t* types; /* typespec tables, or NULL */
    const llg_vpi_code_tables_t* code;  /* expression and statement tables, or NULL */
    char* decompiled;             /* vpiDecompile result, overwritten by each call */
    size_t decompiled_capacity;
    uint32_t typespec_base;   /* static typespec slots follow the catalog rows */
    uint32_t tsmember_base;   /* then typespec members */
    uint32_t enumconst_base;  /* then enum constants */
    uint32_t static_count;    /* slots below this index are never freed */
    llg_vpi_slot_t* slots;
    size_t slot_count;
    size_t slot_capacity;
    uint32_t free_slots;      /* free list: index + 1 through next_owned */
    size_t live_dynamic;
    llg_vpi_registration_t** registrations;
    int registration_count;
    int registration_capacity;
    struct llg_vpi_cb* cbs;       /* live callbacks, registration order */
    struct llg_vpi_cb** cbs_tail;
    struct llg_vpi_cb* retired;   /* removed while a dispatch was running */
    struct llg_vpi_watch** watch_buckets; /* cbValueChange entries by storage key */
    size_t watch_capacity;
    size_t watch_count;
    uint64_t cb_sequence;
    uint32_t fired_head;          /* fired callback slots, oldest first: index + 1 */
    uint32_t fired_tail;
    uint32_t fired_count;
    int dispatch_depth;
    int next_time_pending;        /* cbNextSimTime records waiting */
    int phase;                    /* LLG_VPI_PHASE_* */
    llg_vpi_callsite_t** sites;   /* dense generated site ids */
    size_t site_capacity;
    size_t site_count;
    llg_vpi_call_t* active_call;
    int call_depth;
    llg_vpi_event_t* events;      /* pending scheduled puts */
    int initialized;          /* llg_vpi_model_init ran */
    int startup_called;
    int running;
    int starting;             /* cbEndOfCompile/cbStartOfSimulation are running */
    int failed;
    int error_pending;
    int error_state;
    int error_level;
    int error_line;
    char error_message[512];
    char error_code[64];
    char error_product[32];
    llg_dl_t plugin_handles[LLG_VPI_MAX_PLUGINS];
    int plugin_count;
    s_vpi_vecval* value_vector;   /* simulator-owned vpi_get_value results */
    size_t value_vector_capacity;
    s_vpi_vecval* scratch;        /* internal conversion words */
    size_t scratch_capacity;
    char* value_text;
    size_t value_text_capacity;
    s_vpi_strengthval* value_strength;
    size_t value_strength_capacity;
    s_vpi_time value_time;
    char str_buffer[1024];        /* vpi_get_str result for composed names */
    char* mcd_name;               /* vpi_mcd_name result, overwritten by each call */
    size_t mcd_name_capacity;
    void* array_values;           /* vpi_get_value_array result, overwritten by each call */
    size_t array_values_capacity;
    uint32_t force_cbs;           /* live cbForce/cbRelease/cbAssign/cbDeassign records */
    uint32_t disable_cbs;         /* live cbDisable records */
    uint32_t assertion_cbs;       /* live vpi_register_assertion_cb records */
    int assertion_sys_started;    /* cbAssertionSysInitialized ran (39.4.1) */
    int assertion_sys_scheduled;  /* its time-zero Active event is queued */
    int assertions_ended;         /* vpiAssertionSysEnd or the end of the run */
    uint32_t tchk_cbs;            /* live cbTchkViolation records */
    int quiet_force;              /* vpi_put_value force/release: not a statement */
    llg_vpi_storage_t* storage;   /* rows with storage by address, built on demand */
    size_t storage_count;
    struct llg_vpi_link* links;   /* connectivity links by referenced row, built on demand */
    size_t link_count;
    size_t link_capacity;
    uint32_t* port_links;         /* the high-connection links, by port row */
    size_t port_link_count;
    int links_built;
    uintptr_t iterate_ref;        /* reference of the running vpi_iterate call */
    uint32_t* code_parents;       /* parent code row + 1 of each code row, built on demand */
} llg_vpi_state_t;

static llg_vpi_state_t g_vpi;
static uint32_t g_vpi_generation;

/* Dynamic objects: llg_vpi_dynamic.c, appended to this translation unit. */
struct llg_vpi_cb;
static int dyn_tables_valid(void);
static size_t dyn_static_slot_count(void);
static void dyn_static_slots(uint32_t base);
static int dyn_row_valid(const llg_vpi_model_object_t* object);
static int dyn_slot(const llg_vpi_slot_t* slot);
static vpiHandle dyn_iterate_class_defns(uint32_t scope);
static int dyn_is_class_scope(const llg_vpi_slot_t* slot);
static int dyn_type(const llg_vpi_slot_t* slot);
static vpiHandle dyn_handle(int type, llg_vpi_slot_t* slot, int* handled);
static vpiHandle dyn_iterate(int type, llg_vpi_slot_t* slot, int* handled);
static vpiHandle dyn_scan(llg_vpi_slot_t* slot, uint32_t position);
static PLI_INT64 dyn_get(int property, llg_vpi_slot_t* slot, int* handled);
static const char* dyn_get_str(int property, llg_vpi_slot_t* slot, int* handled);
static void dyn_get_value(llg_vpi_slot_t* slot, p_vpi_value output);
static void dyn_put_value(llg_vpi_slot_t* slot, p_vpi_value input, int mode);
static int dyn_compare(const llg_vpi_slot_t* a, uint32_t a_index, const llg_vpi_slot_t* b,
                       uint32_t b_index);
static vpiHandle dyn_handle_by_index(llg_vpi_slot_t* slot, PLI_INT32 index);
static vpiHandle dyn_by_suffix(uint32_t row, const char* suffix);
static int dyn_bind_value_cb(struct llg_vpi_cb* cb, llg_vpi_slot_t* slot);
static int dyn_bind_object_cb(struct llg_vpi_cb* cb, const s_cb_data* data);
static void dyn_object_cb_added(void);
static void dyn_object_cb_removed(void);
static void dyn_shutdown(void);
/* Runtime structures: llg_vpi_runtime.c, appended after the dynamic layer. */
static int rts_slot(const llg_vpi_slot_t* slot);
static int rts_type(const llg_vpi_slot_t* slot);
static int rts_identity(const llg_vpi_slot_t* slot, uint64_t identity[4]);
static PLI_INT64 rts_get(int property, llg_vpi_slot_t* slot, int* handled);
static const char* rts_get_str(int property, llg_vpi_slot_t* slot, int* handled);
static vpiHandle rts_handle(int type, vpiHandle reference, int* handled);
static vpiHandle rts_iterate(int type, vpiHandle reference, int* handled);
static void rts_get_value(llg_vpi_slot_t* slot, p_vpi_value output);
static void rts_shutdown(void);
static int rts_cb_reason(int reason);
static int rts_bind_cb(struct llg_vpi_cb* cb, const s_cb_data* data);
static void rts_cb_added(const struct llg_vpi_cb* cb);
static void rts_cb_removed(const struct llg_vpi_cb* cb);
static void rts_enable(void);
static int code_tables_valid(const llg_vpi_code_tables_t* t);
static PLI_INT32 unsupported_property(void);

/* ── errors ─────────────────────────────────────────────────────────────── */

static void vpi_set_error(int state, int level, const char* code,
                          const char* message) {
    g_vpi.error_pending = 1;
    g_vpi.error_state = state;
    g_vpi.error_level = level;
    g_vpi.error_line = 0;
    snprintf(g_vpi.error_code, sizeof(g_vpi.error_code), "%s",
             code ? code : "LLG_VPI");
    snprintf(g_vpi.error_message, sizeof(g_vpi.error_message), "%s",
             message ? message : "VPI operation failed");
}

static void vpi_set_errorf(int state, int level, const char* code,
                           const char* format, ...) {
    va_list ap;
    g_vpi.error_pending = 1;
    g_vpi.error_state = state;
    g_vpi.error_level = level;
    g_vpi.error_line = 0;
    snprintf(g_vpi.error_code, sizeof(g_vpi.error_code), "%s",
             code ? code : "LLG_VPI");
    va_start(ap, format);
    vsnprintf(g_vpi.error_message, sizeof(g_vpi.error_message), format, ap);
    va_end(ap);
}

static int vpi_state(void) { return g_vpi.running ? vpiRun : vpiPLI; }

static void vpi_error(const char* code, const char* message) {
    vpi_set_error(vpi_state(), vpiError, code, message);
}

/* 38.2: the error status is reset by every VPI routine except
 * vpi_chk_error(). Every public entry point calls this first. */
static void vpi_enter(void) {
    g_vpi.error_pending = 0;
    g_vpi.iterate_ref = 0;
}

static void vpi_fail_runtime(const char* message) {
    g_vpi.failed = 1;
    vpi_set_error(vpiRun, vpiError, "LLG_VPI_RUNTIME", message);
    fprintf(stderr, "llg: VPI error: %s\n", message);
    llg_rt_request_finish();
}

static char* vpi_strdup(const char* text) {
    if (!text) return NULL;
    size_t length = strlen(text);
    char* copy = (char*)malloc(length + 1);
    if (!copy) return NULL;
    memcpy(copy, text, length + 1);
    return copy;
}

/* ── catalog indexing ───────────────────────────────────────────────────── */

static int model_pointer_index(const llg_vpi_model_object_t* pointer,
                               uint32_t* index) {
    if (!pointer || !g_vpi.objects) return 0;
    uintptr_t begin = (uintptr_t)g_vpi.objects;
    uintptr_t address = (uintptr_t)pointer;
    uintptr_t bytes = (uintptr_t)(g_vpi.object_count * sizeof(*g_vpi.objects));
    if (address < begin || address - begin >= bytes ||
        (address - begin) % sizeof(*g_vpi.objects) != 0)
        return 0;
    *index = (uint32_t)((address - begin) / sizeof(*g_vpi.objects));
    return 1;
}

/* A catalog row of type vpiBegin, vpiFork, vpiFor or vpiForeachStmt is an
 * unnamed block, for or foreach that is a scope (37.12 details 1-3,
 * decision A19-D27); other such statements are code objects only. */
static int is_statement_scope_type(int type) {
    return type == vpiBegin || type == vpiFork || type == vpiFor || type == vpiForeachStmt;
}

/* Module, interface and program instances and packages (37.10). */
static int is_instance_type(int type) {
    return type == vpiModule || type == vpiInterface || type == vpiProgram || type == vpiPackage;
}

static int is_scope_type(int type) {
    return is_instance_type(type) || type == vpiGenScope || type == vpiClockingBlock ||
           type == vpiNamedBegin || type == vpiNamedFork || type == vpiTask ||
           type == vpiFunction || is_statement_scope_type(type);
}

/* The iterations 37.12 gives every scope that llg answers; a begin or fork
 * that is not a scope answers them with no objects (decision A19-D27). */
static int is_scope_iteration(int type) {
    switch (type) {
        case vpiPropertyDecl: case vpiSequenceDecl: case vpiNamedEvent: case vpiVariables:
        case vpiReg: case vpiRegArray: case vpiParameter: case vpiInternalScope:
            return 1;
        default:
            return 0;
    }
}

/* A parameter or specparam: a named constant (37.26). */
static int is_parameter_type(int type) {
    return type == vpiParameter || type == vpiSpecParam;
}

/* Declaration objects without a value (37.13, 37.80, decision A19-D42). */
static int is_declaration_type(int type) {
    return type == vpiGenScopeArray || type == vpiGenVar || type == vpiIODecl;
}

static int is_process_type(int type) {
    return type == vpiInitial || type == vpiAlways || type == vpiFinal;
}

static int is_primitive_type(int type) {
    return type == vpiGate || type == vpiSwitch || type == vpiUdp;
}

static int is_instance_array_type(int type) {
    return type == vpiModuleArray || type == vpiInterfaceArray || type == vpiProgramArray ||
           type == vpiGateArray || type == vpiSwitchArray || type == vpiUdpArray;
}

/* Structure objects without a value of their own (37.7, 37.11, 37.25,
 * 37.31, 37.43, 37.58). */
static int is_structure_type(int type) {
    return is_process_type(type) || is_primitive_type(type) || is_instance_array_type(type) ||
           type == vpiContAssign || type == vpiNamedEvent || type == vpiModport ||
           type == vpiModPath || type == vpiTchk || type == vpiClockingIODecl;
}

static int is_array_type(int type) {
    return type == vpiRegArray || type == vpiNetArray;
}

static int is_variable_type(int type) {
    switch (type) {
        case vpiReg: case vpiIntegerVar: case vpiTimeVar: case vpiRealVar:
        case vpiLongIntVar: case vpiShortIntVar: case vpiIntVar:
        case vpiShortRealVar: case vpiByteVar: case vpiEnumVar:
        case vpiStructVar: case vpiUnionVar: case vpiBitVar:
        case vpiPackedArrayVar:
            return 1;
        default:
            return 0;
    }
}

/* Objects whose value lives in model storage (not parameters or scopes). */
/* The catalog objects 37.10, 37.16, 37.17, 37.25, 37.31 and 37.80 give the
 * vpiArray and vpiArrayMember properties: instances, nets, variables,
 * named events, primitives and generate scopes. */
static int has_array_properties(const llg_vpi_model_object_t* object) {
    if (!object) return 0;
    int type = object->type;
    return is_instance_type(type) || type == vpiNet || type == vpiNamedEvent ||
           type == vpiGenScope || is_primitive_type(type) || is_array_type(type) ||
           is_variable_type(type);
}

static int has_storage(const llg_vpi_model_object_t* object) {
    return object->is_real ? object->real != NULL : object->packed != NULL;
}

static int model_object_valid(const llg_vpi_model_object_t* object) {
    uint32_t parent = 0, target = 0;
    if (!object->name || !object->full_name ||
        object->width >= LLG_SUPPORTED_WIDTH_LIMIT)
        return 0;
    if (object->typespec &&
        (!g_vpi.types || object->typespec > g_vpi.types->typespec_count))
        return 0;
    if (object->flags & LLG_VPI_OBJ_DYNAMIC) return dyn_row_valid(object);
    if (object->storage || object->dependency || object->shape) return 0;
    if (object->flags & LLG_VPI_OBJ_RECORD)
        return (object->type == vpiStructVar || object->type == vpiUnionVar) &&
               !object->packed && !object->real;
    if ((object->flags & LLG_VPI_OBJ_SLICE) && (object->is_real || !object->packed))
        return 0;
    if (object->parent && !model_pointer_index(object->parent, &parent)) return 0;
    if (object->target && !model_pointer_index(object->target, &target)) return 0;
    if (object->dim_count && !object->dims) return 0;
    if (object->instance &&
        (!object->instance->library || !object->instance->cell ||
         (object->type != vpiModule && object->type != vpiInterface &&
          object->type != vpiProgram)))
        return 0;
    if (object->conn_count && !object->conns) return 0;
    for (uint32_t i = 0; i < object->conn_count; ++i) {
        const llg_vpi_compile_arg_t* ref = &object->conns[i].ref;
        if (ref->object >= 0 && (uint64_t)ref->object >= g_vpi.object_count) return 0;
        if (ref->ref_kind == LLG_VPI_ARG_CONSTANT && !object->conns[i].value) return 0;
        if (object->conns[i].condition > object->conn_count) return 0;
    }
    if (object->timing) {
        const llg_vpi_timing_t* tm = object->timing;
        if ((tm->op_count && !tm->ops) || (tm->declared_count && !tm->declared))
            return 0;
        for (uint32_t i = 0; i < tm->op_count; ++i) {
            const llg_sdf_op_t* op = &tm->ops[i];
            int valid = op->kind == LLG_SDF_PATH ? op->path != NULL
                        : op->kind == LLG_SDF_GATE ? op->gate != NULL
                        : op->kind == LLG_SDF_CHECK ? op->check != NULL && op->checks != NULL
                        : op->kind == LLG_SDF_PAIR ? op->checks != NULL : 0;
            if (!valid || (object->type == vpiTchk && op->first >= 2u)) return 0;
        }
    }
    if (object->type == vpiModPath || object->type == vpiTchk) return object->timing != NULL;
    if (is_structure_type(object->type)) {
        if (object->packed || object->real) return 0;
        if (object->type == vpiNamedEvent) return object->event != NULL;
        if (is_instance_array_type(object->type)) return object->dim_count > 0;
        return 1;
    }
    if (is_scope_type(object->type))
        return !object->packed && !object->real;
    if (is_parameter_type(object->type))
        return object->is_real || object->constant;
    if (object->type == vpiPort) return 1;
    if (is_declaration_type(object->type)) return !object->packed && !object->real;
    if (is_array_type(object->type))
        return object->dim_count > 0 &&
               (object->is_real ? object->real != NULL && object->width == 0
                                : object->packed != NULL && object->width != 0);
    if (object->type == vpiNet || is_variable_type(object->type))
        return object->is_real ? object->real != NULL && object->width == 0
                               : object->packed != NULL && object->width != 0;
    return 0;
}

/* Every index of the borrowed typespec tables is in range, and every child
 * window of a member node lies in the node table. */
static int types_valid(const llg_vpi_type_tables_t* t) {
    if (!t) return 1;
    if ((t->typespec_count && !t->typespecs) || (t->range_count && !t->ranges) ||
        (t->member_count && !t->members) || (t->const_count && !t->consts) ||
        (t->node_count && !t->nodes))
        return 0;
    for (uint32_t i = 0; i < t->typespec_count; ++i) {
        const llg_vpi_typespec_t* ts = &t->typespecs[i];
        if (ts->alias > t->typespec_count || ts->element > t->typespec_count ||
            ts->base > t->typespec_count ||
            ts->first_range > t->range_count || ts->range_count > t->range_count - ts->first_range ||
            ts->first_member > t->member_count ||
            ts->member_count > t->member_count - ts->first_member ||
            ts->first_const > t->const_count || ts->const_count > t->const_count - ts->first_const ||
            ts->first_node > t->node_count || ts->node_count > t->node_count - ts->first_node)
            return 0;
    }
    for (uint32_t i = 0; i < t->member_count; ++i)
        if (!t->members[i].name || t->members[i].typespec >= t->typespec_count) return 0;
    for (uint32_t i = 0; i < t->const_count; ++i)
        if (!t->consts[i].name || !t->consts[i].value) return 0;
    for (uint32_t i = 0; i < t->node_count; ++i) {
        const llg_vpi_member_node_t* node = &t->nodes[i];
        if (!node->name || node->typespec >= t->typespec_count || node->parent > t->node_count ||
            node->first_child > t->node_count ||
            node->child_count > t->node_count - node->first_child ||
            node->width >= LLG_SUPPORTED_WIDTH_LIMIT || node->lsb >= LLG_SUPPORTED_WIDTH_LIMIT)
            return 0;
    }
    return 1;
}

static int compare_rows(const void* left, const void* right) {
    uint32_t a = *(const uint32_t*)left, b = *(const uint32_t*)right;
    return strcmp(g_vpi.objects[a].full_name, g_vpi.objects[b].full_name);
}

static uint64_t dims_element_count(const llg_vpi_model_object_t* object) {
    uint64_t count = 1;
    for (uint32_t d = 0; d < object->dim_count; ++d) {
        int64_t left = object->dims[2 * d], right = object->dims[2 * d + 1];
        uint64_t extent = (uint64_t)(left > right ? left - right : right - left) + 1u;
        if (extent && count > UINT64_MAX / extent) return 0;
        count *= extent;
    }
    return count;
}

static uint32_t next_generation(uint32_t generation) {
    generation = (generation + 1u) & LLG_VPI_GENERATION_MASK;
    return generation ? generation : 1u;
}

/* Validate and index the borrowed catalog. Runs on the first VPI use, so a
 * model whose run loads no application never walks its catalog. */
static int vpi_ready(void) {
    if (g_vpi.indexed) return 1;
    if (!g_vpi.initialized) {
        vpi_set_error(vpiPLI, vpiError, "LLG_VPI_MODEL", "VPI used without model metadata");
        return 0;
    }
    size_t count = g_vpi.object_count;
    if (!types_valid(g_vpi.types) || !dyn_tables_valid()) {
        vpi_set_error(vpiPLI, vpiError, "LLG_VPI_MODEL", "invalid VPI typespec tables");
        g_vpi.failed = 1;
        return 0;
    }
    if (!code_tables_valid(g_vpi.code)) {
        vpi_set_error(vpiPLI, vpiError, "LLG_VPI_MODEL", "invalid VPI expression and statement tables");
        g_vpi.failed = 1;
        return 0;
    }
    for (size_t i = 0; i < count; ++i) {
        if (!model_object_valid(&g_vpi.objects[i])) {
            vpi_set_errorf(vpiPLI, vpiError, "LLG_VPI_MODEL",
                           "invalid VPI model object at index %zu", i);
            g_vpi.failed = 1;
            return 0;
        }
    }
    /* Typespecs, their members and enum constants are static slots after
     * the catalog rows, so each has one handle for the whole run. */
    const llg_vpi_type_tables_t* types = g_vpi.types;
    size_t typespecs = types ? types->typespec_count : 0u;
    size_t tsmembers = types ? types->member_count : 0u;
    size_t enumconsts = types ? types->const_count : 0u;
    size_t static_count = count + typespecs + tsmembers + enumconsts + dyn_static_slot_count();
    if (static_count >= LLG_VPI_INDEX_MASK - 1u) {
        vpi_set_error(vpiPLI, vpiError, "LLG_VPI_LIMIT", "VPI catalog exceeds the handle index space");
        g_vpi.failed = 1;
        return 0;
    }
    size_t capacity = static_count + 64u;
    g_vpi.slots = (llg_vpi_slot_t*)calloc(capacity, sizeof(*g_vpi.slots));
    /* One extra row keeps the allocations non-empty for a model built
     * without a catalog (no `--vpi`), whose applications see no objects. */
    g_vpi.sorted = (uint32_t*)malloc((count + 1u) * sizeof(*g_vpi.sorted));
    g_vpi.first_child = (uint32_t*)calloc(count + 1u, sizeof(*g_vpi.first_child));
    g_vpi.next_sibling = (uint32_t*)calloc(count + 1u, sizeof(*g_vpi.next_sibling));
    g_vpi.element_counts = (uint64_t*)calloc(count + 1u, sizeof(*g_vpi.element_counts));
    if (!g_vpi.slots || !g_vpi.sorted || !g_vpi.first_child ||
        !g_vpi.next_sibling || !g_vpi.element_counts) {
        free(g_vpi.slots);
        free(g_vpi.sorted);
        free(g_vpi.first_child);
        free(g_vpi.next_sibling);
        free(g_vpi.element_counts);
        g_vpi.slots = NULL;
        g_vpi.sorted = NULL;
        g_vpi.first_child = g_vpi.next_sibling = NULL;
        g_vpi.element_counts = NULL;
        vpi_set_error(vpiPLI, vpiError, "LLG_VPI_NOMEM", "VPI catalog index allocation failed");
        return 0;
    }
    g_vpi.slot_capacity = capacity;
    g_vpi.slot_count = static_count;
    g_vpi.typespec_base = (uint32_t)count;
    g_vpi.tsmember_base = (uint32_t)(count + typespecs);
    g_vpi.enumconst_base = (uint32_t)(count + typespecs + tsmembers);
    g_vpi.static_count = (uint32_t)static_count;
    for (size_t i = count; i < static_count; ++i) {
        llg_vpi_slot_t* slot = &g_vpi.slots[i];
        slot->generation = g_vpi.model_generation;
        slot->object = LLG_VPI_NONE;
        slot->element = LLG_VPI_NO_ELEMENT;
        if (i < g_vpi.tsmember_base) {
            slot->kind = SLOT_TYPESPEC;
            slot->aux = (int32_t)(i - count);
        } else if (i < g_vpi.enumconst_base) {
            slot->kind = SLOT_TSMEMBER;
            slot->aux = (int32_t)(i - g_vpi.tsmember_base);
        } else {
            slot->kind = SLOT_ENUMCONST;
            slot->aux = (int32_t)(i - g_vpi.enumconst_base);
        }
    }
    /* Class definitions, typespecs and extends objects follow. */
    dyn_static_slots((uint32_t)(count + typespecs + tsmembers + enumconsts));
    g_vpi.sorted_count = 0;
    /* Children keep catalog order: link in reverse. */
    for (size_t i = count; i-- > 0;) {
        llg_vpi_model_object_t* object = &g_vpi.objects[i];
        llg_vpi_slot_t* slot = &g_vpi.slots[i];
        slot->generation = g_vpi.model_generation;
        slot->kind = SLOT_OBJECT;
        slot->object = (uint32_t)i;
        slot->element = LLG_VPI_NO_ELEMENT;
        uint32_t parent;
        if (model_pointer_index(object->parent, &parent)) {
            g_vpi.next_sibling[i] = g_vpi.first_child[parent];
            g_vpi.first_child[parent] = (uint32_t)i + 1u;
        }
        if (is_array_type(object->type) || is_instance_array_type(object->type))
            g_vpi.element_counts[i] = dims_element_count(object);
    }
    /* Ports live in their own namespace and unnamed objects have no name. */
    for (size_t i = 0; i < count; ++i)
        if (g_vpi.objects[i].type != vpiPort && g_vpi.objects[i].type != vpiIODecl &&
            !(g_vpi.objects[i].flags & LLG_VPI_OBJ_UNNAMED))
            g_vpi.sorted[g_vpi.sorted_count++] = (uint32_t)i;
    qsort(g_vpi.sorted, g_vpi.sorted_count, sizeof(*g_vpi.sorted), compare_rows);
    g_vpi.indexed = 1;
    return 1;
}

/* ── handle slots ───────────────────────────────────────────────────────── */

static vpiHandle encode_handle(uint32_t index) {
    uintptr_t value = ((uintptr_t)g_vpi.slots[index].generation << LLG_VPI_INDEX_BITS) |
                      ((uintptr_t)index + 1u);
    return (vpiHandle)value;
}

/* Decode without dereferencing the application's pointer value. */
static llg_vpi_slot_t* decode_handle(vpiHandle handle, uint32_t* index_out) {
    if (!handle || !g_vpi.indexed) return NULL;
    uintptr_t value = (uintptr_t)handle;
    uintptr_t index = value & LLG_VPI_INDEX_MASK;
    uint32_t generation = (uint32_t)(value >> LLG_VPI_INDEX_BITS);
    if (index == 0 || index - 1u >= g_vpi.slot_count) return NULL;
    llg_vpi_slot_t* slot = &g_vpi.slots[index - 1u];
    if (slot->kind == SLOT_FREE || slot->generation != generation) return NULL;
    if (index_out) *index_out = (uint32_t)(index - 1u);
    return slot;
}

static llg_vpi_slot_t* checked_handle(vpiHandle handle, uint32_t* index) {
    llg_vpi_slot_t* slot = decode_handle(handle, index);
    if (!slot) vpi_error("LLG_VPI_HANDLE", "invalid, released or stale VPI handle");
    return slot;
}

/* Allocate a dynamic slot; returns its index or LLG_VPI_NONE. The slot
 * array may move, so callers re-derive slot pointers after allocating. */
static uint32_t alloc_slot(llg_vpi_slot_kind_t kind) {
    if (!vpi_ready()) return LLG_VPI_NONE;
    if (g_vpi.live_dynamic >= LLG_VPI_MAX_LIVE_HANDLES) {
        vpi_error("LLG_VPI_LIMIT", "too many live VPI handles; release unused handles");
        return LLG_VPI_NONE;
    }
    uint32_t index;
    if (g_vpi.free_slots) {
        index = g_vpi.free_slots - 1u;
        g_vpi.free_slots = g_vpi.slots[index].next_owned;
    } else {
        if (g_vpi.slot_count >= LLG_VPI_INDEX_MASK - 1u) {
            vpi_error("LLG_VPI_LIMIT", "VPI handle index space exhausted");
            return LLG_VPI_NONE;
        }
        if (g_vpi.slot_count == g_vpi.slot_capacity) {
            size_t capacity = g_vpi.slot_capacity * 2u;
            llg_vpi_slot_t* grown =
                (llg_vpi_slot_t*)realloc(g_vpi.slots, capacity * sizeof(*grown));
            if (!grown) {
                vpi_error("LLG_VPI_NOMEM", "VPI handle allocation failed");
                return LLG_VPI_NONE;
            }
            memset(grown + g_vpi.slot_capacity, 0,
                   (capacity - g_vpi.slot_capacity) * sizeof(*grown));
            g_vpi.slots = grown;
            g_vpi.slot_capacity = capacity;
        }
        index = (uint32_t)g_vpi.slot_count++;
    }
    llg_vpi_slot_t* slot = &g_vpi.slots[index];
    uint32_t generation = next_generation(slot->generation);
    if (generation == g_vpi.model_generation) generation = next_generation(generation);
    memset(slot, 0, sizeof(*slot));
    slot->generation = generation;
    slot->kind = (uint8_t)kind;
    slot->object = LLG_VPI_NONE;
    slot->element = LLG_VPI_NO_ELEMENT;
    /* 37.79 detail 1: an iterator remembers the reference it was made from. */
    if (kind == SLOT_ITERATOR) slot->use = g_vpi.iterate_ref;
    ++g_vpi.live_dynamic;
    return index;
}

static void release_slot(uint32_t index) {
    llg_vpi_slot_t* slot = &g_vpi.slots[index];
    if (slot->kind == SLOT_FREE || index < g_vpi.static_count) return;
    free(slot->items);
    uint32_t generation = slot->generation;
    memset(slot, 0, sizeof(*slot));
    /* Bump on release too, so the old encoding is stale immediately. */
    slot->generation = next_generation(generation);
    slot->kind = SLOT_FREE;
    slot->next_owned = g_vpi.free_slots;
    g_vpi.free_slots = index + 1u;
    if (g_vpi.live_dynamic) --g_vpi.live_dynamic;
}

/* Register a slot as owned by the active invocation. */
static void own_by_call(uint32_t index, llg_vpi_call_t* call) {
    if (!call) return;
    g_vpi.slots[index].owned = 1;
    g_vpi.slots[index].next_owned = call->owned;
    call->owned = index + 1u;
}

static void release_call_slots(llg_vpi_call_t* call) {
    uint32_t cursor = call->owned;
    while (cursor) {
        uint32_t index = cursor - 1u;
        cursor = g_vpi.slots[index].next_owned;
        if (g_vpi.slots[index].kind != SLOT_FREE && g_vpi.slots[index].owned)
            release_slot(index);
    }
    call->owned = 0;
}

static vpiHandle object_handle(uint32_t row) {
    if (row == LLG_VPI_NONE || row >= g_vpi.object_count) return NULL;
    return encode_handle(row);
}

static vpiHandle make_element(uint32_t row, uint64_t element, llg_vpi_call_t* owner) {
    uint32_t index = alloc_slot(SLOT_ELEMENT);
    if (index == LLG_VPI_NONE) return NULL;
    g_vpi.slots[index].object = row;
    g_vpi.slots[index].element = element;
    own_by_call(index, owner);
    return encode_handle(index);
}

static vpiHandle make_select(uint32_t row, uint64_t element, uint32_t lsb, uint32_t width,
                             int64_t left, int64_t right, llg_vpi_call_t* owner) {
    uint32_t index = alloc_slot(SLOT_SELECT);
    if (index == LLG_VPI_NONE) return NULL;
    llg_vpi_slot_t* slot = &g_vpi.slots[index];
    slot->object = row;
    slot->element = element;
    slot->lsb = lsb;
    slot->span = width;
    slot->left = left;
    slot->right = right;
    own_by_call(index, owner);
    return encode_handle(index);
}

static vpiHandle make_constant(int64_t value, int const_type) {
    uint32_t index = alloc_slot(SLOT_CONSTANT);
    if (index == LLG_VPI_NONE) return NULL;
    g_vpi.slots[index].left = value;
    g_vpi.slots[index].aux = const_type;
    return encode_handle(index);
}

/* A vpiStringConst holding a copy of `length` bytes in `items`/`count`
 * (an associative array's string key, decision A19-D23). */
static vpiHandle make_string_constant(const char* bytes, uint32_t length) {
    char* copy = (char*)malloc((size_t)length + 1u);
    if (!copy) {
        vpi_error("LLG_VPI_NOMEM", "VPI constant allocation failed");
        return NULL;
    }
    if (length) memcpy(copy, bytes, length);
    copy[length] = '\0';
    uint32_t index = alloc_slot(SLOT_CONSTANT);
    if (index == LLG_VPI_NONE) {
        free(copy);
        return NULL;
    }
    g_vpi.slots[index].aux = vpiStringConst;
    g_vpi.slots[index].items = (uintptr_t*)(void*)copy;
    g_vpi.slots[index].count = length;
    return encode_handle(index);
}

static int is_string_constant(const llg_vpi_slot_t* slot) {
    return slot->kind == SLOT_CONSTANT && slot->aux == vpiStringConst && slot->items;
}

/* ── catalog helpers ────────────────────────────────────────────────────── */

static llg_vpi_model_object_t* row_object(uint32_t row) {
    return row < g_vpi.object_count ? &g_vpi.objects[row] : NULL;
}

static uint32_t row_of(const llg_vpi_model_object_t* object) {
    uint32_t row;
    return model_pointer_index(object, &row) ? row : LLG_VPI_NONE;
}

static uint32_t find_row(const char* full_name) {
    size_t low = 0, high = g_vpi.sorted_count;
    while (low < high) {
        size_t middle = low + (high - low) / 2u;
        int order = strcmp(g_vpi.objects[g_vpi.sorted[middle]].full_name, full_name);
        if (order == 0) return g_vpi.sorted[middle];
        if (order < 0) low = middle + 1u;
        else high = middle;
    }
    return LLG_VPI_NONE;
}

/* Element vpiType of an array or the type of an element/value object. */
static int value_type_of(const llg_vpi_model_object_t* object, uint64_t element) {
    if (element != LLG_VPI_NO_ELEMENT && is_array_type(object->type))
        return object->subtype ? object->subtype : (object->is_net ? vpiNet : vpiReg);
    return object->type;
}

/* Index range `[left:right]` of the outermost unpacked dimension. */
static int array_dim(const llg_vpi_model_object_t* object, uint32_t dim,
                     int64_t* left, int64_t* right) {
    if (dim >= object->dim_count) return 0;
    *left = object->dims[2 * dim];
    *right = object->dims[2 * dim + 1];
    return 1;
}

static int index_offset(int64_t left, int64_t right, int64_t index, uint64_t* offset) {
    int64_t low = left < right ? left : right, high = left < right ? right : left;
    if (index < low || index > high) return 0;
    *offset = (uint64_t)(left >= right ? left - index : index - left);
    return 1;
}

/* Packed range of an element or whole value (MSB index first). */
static void packed_range(const llg_vpi_model_object_t* object, int64_t* left, int64_t* right) {
    if (object->flags & LLG_VPI_OBJ_RANGE) {
        *left = object->left;
        *right = object->right;
    } else {
        *left = object->width ? (int64_t)object->width - 1 : 0;
        *right = 0;
    }
}

/* Storage offset of packed index `index` (SV index, not offset). */
static int packed_offset(const llg_vpi_model_object_t* object, int64_t index, uint32_t* offset) {
    int64_t left, right;
    packed_range(object, &left, &right);
    int64_t low = left < right ? left : right, high = left < right ? right : left;
    if (index < low || index > high) return 0;
    *offset = (uint32_t)(left >= right ? index - right : right - index);
    return 1;
}

/* ── typespecs and members (37.17, 37.23, 37.24) ────────────────────────── */

static const llg_vpi_typespec_t* typespec_at(uint32_t index) {
    const llg_vpi_type_tables_t* t = g_vpi.types;
    return t && index < t->typespec_count ? &t->typespecs[index] : NULL;
}

/* An optional typespec reference (index + 1, 0: none). */
static const llg_vpi_typespec_t* typespec_ref(uint32_t reference) {
    return reference ? typespec_at(reference - 1u) : NULL;
}

static uint32_t typespec_index(const llg_vpi_typespec_t* typespec) {
    return (uint32_t)(typespec - g_vpi.types->typespecs);
}

static const llg_vpi_member_node_t* node_at(uint32_t index) {
    const llg_vpi_type_tables_t* t = g_vpi.types;
    return t && index < t->node_count ? &t->nodes[index] : NULL;
}

static vpiHandle typespec_handle(const llg_vpi_typespec_t* typespec) {
    return typespec ? encode_handle(g_vpi.typespec_base + typespec_index(typespec)) : NULL;
}

/* The typespec of a declaration (decision A19-D4): a structure, union or
 * enum type is one object (its static slot), any other typespec belongs to
 * the declaration that reaches it, catalog row `row` (and member node
 * `member` + 1 of a structure member, else 0). The interned table stays
 * shared; such a handle is a dynamic slot whose identity adds the owner. */
static vpiHandle declared_typespec_handle(const llg_vpi_typespec_t* typespec, uint32_t row,
                                          int64_t member) {
    if (!typespec) return NULL;
    if (row == LLG_VPI_NONE || typespec->type == vpiStructTypespec ||
        typespec->type == vpiUnionTypespec || typespec->type == vpiEnumTypespec)
        return typespec_handle(typespec);
    uint32_t index = alloc_slot(SLOT_TYPESPEC);
    if (index == LLG_VPI_NONE) return NULL;
    llg_vpi_slot_t* slot = &g_vpi.slots[index];
    slot->aux = (int32_t)typespec_index(typespec);
    slot->object = row;
    slot->left = member;
    return encode_handle(index);
}

/* The typespec of an element value: its array typespec without every
 * unpacked dimension. */
static const llg_vpi_typespec_t* element_typespec(const llg_vpi_model_object_t* object) {
    const llg_vpi_typespec_t* typespec = typespec_ref(object->typespec);
    for (uint32_t guard = 0; typespec && typespec->type == vpiArrayTypespec && guard <= object->dim_count;
         ++guard)
        typespec = typespec_ref(typespec->element);
    return typespec;
}

/* The typespec of the value a row, element or member handle names. */
static const llg_vpi_typespec_t* value_typespec(const llg_vpi_slot_t* slot) {
    llg_vpi_model_object_t* object = row_object(slot->object);
    switch (slot->kind) {
        case SLOT_OBJECT:
            return object ? typespec_ref(object->typespec) : NULL;
        case SLOT_ELEMENT:
            return element_typespec(object);
        case SLOT_MEMBER: {
            const llg_vpi_member_node_t* node = node_at((uint32_t)slot->aux);
            return node ? typespec_at(node->typespec) : NULL;
        }
        default:
            return NULL;
    }
}

static int is_aggregate_typespec(const llg_vpi_typespec_t* typespec) {
    return typespec && (typespec->type == vpiStructTypespec || typespec->type == vpiUnionTypespec);
}

/* 37.24 detail 1: an element or member that is a whole unpacked structure
 * or union has no value even when its storage is a packed image. */
static int whole_unpacked_aggregate(const llg_vpi_slot_t* slot) {
    if (slot->kind != SLOT_ELEMENT && slot->kind != SLOT_MEMBER) return 0;
    const llg_vpi_typespec_t* typespec = value_typespec(slot);
    return is_aggregate_typespec(typespec) && (typespec->flags & LLG_VPI_TS_UNPACKED);
}

static vpiHandle make_member(uint32_t row, uint64_t element, uint32_t node_index) {
    const llg_vpi_member_node_t* node = node_at(node_index);
    if (!node) return NULL;
    uint32_t index = alloc_slot(SLOT_MEMBER);
    if (index == LLG_VPI_NONE) return NULL;
    llg_vpi_slot_t* slot = &g_vpi.slots[index];
    slot->object = row;
    slot->element = element;
    slot->aux = (int32_t)node_index;
    slot->lsb = node->lsb;
    slot->span = node->width;
    return encode_handle(index);
}

/* ── value storage ──────────────────────────────────────────────────────── */

static sv4_t* packed_cell(const llg_vpi_target_t* target) {
    llg_vpi_model_object_t* object = row_object(target->object);
    if (!object || object->is_real || !object->packed) return NULL;
    if (target->element == LLG_VPI_NO_ELEMENT) return object->packed;
    return object->packed + target->element;
}

static double* real_cell(const llg_vpi_target_t* target) {
    llg_vpi_model_object_t* object = row_object(target->object);
    if (!object || !object->is_real || !object->real) return NULL;
    if (target->element == LLG_VPI_NO_ELEMENT) return object->real;
    return object->real + target->element;
}

static void datum_destroy(llg_vpi_datum_t* datum) {
    if (!datum->is_real) sv4_destroy(&datum->packed);
}

/* Scratch vecval words for internal conversions, valid until the next use. */
static s_vpi_vecval* scratch_words(size_t words) {
    if (words == 0) words = 1;
    if (words > g_vpi.scratch_capacity) {
        s_vpi_vecval* grown = (s_vpi_vecval*)realloc(g_vpi.scratch, words * sizeof(*grown));
        if (!grown) return NULL;
        g_vpi.scratch = grown;
        g_vpi.scratch_capacity = words;
    }
    memset(g_vpi.scratch, 0, words * sizeof(*g_vpi.scratch));
    return g_vpi.scratch;
}

static size_t word_count(uint32_t width) { return width ? ((size_t)width - 1u) / 32u + 1u : 1u; }

static void export_words(sv4_t value, s_vpi_vecval* words) {
    _Static_assert(offsetof(s_vpi_vecval, bval) == sizeof(uint32_t), "VPI vector layout");
    llg_sv4_export_vpi32(value, words, word_count(llg_sv4_width(value)), sizeof(*words));
}

/* A value from four-state digit text (MSB first), or from string text. */
static sv4_t digits_value(const char* text, uint32_t width, int is_signed, int string) {
    width = width ? width : 1u;
    sv4_t value = sv4_zero(width, (int8_t)is_signed);
    s_vpi_vecval* words = scratch_words(word_count(width));
    if (!words) return value;
    if (!text) text = "";
    size_t length = strlen(text);
    if (string) {
        for (size_t i = 0; i < length && i * 8u < width; ++i) {
            unsigned char ch = (unsigned char)text[length - 1u - i];
            for (unsigned bit = 0; bit < 8u && i * 8u + bit < width; ++bit) {
                size_t position = i * 8u + bit;
                if ((ch >> bit) & 1u) words[position / 32u].aval |= 1u << (position % 32u);
            }
        }
    } else {
        for (size_t i = 0; i < length && i < width; ++i) {
            char digit = text[length - 1u - i];
            uint32_t mask = 1u << (i % 32u);
            if (digit == '1' || digit == 'x') words[i / 32u].aval |= mask;
            if (digit == 'x' || digit == 'z') words[i / 32u].bval |= mask;
        }
    }
    llg_sv4_import_vpi32(&value, words, word_count(width), sizeof(*words));
    return value;
}

/* Parameter value from its four-state digit text. */
static sv4_t parameter_value(const llg_vpi_model_object_t* object) {
    return digits_value(object->constant, object->width, object->is_signed,
                        (object->flags & LLG_VPI_OBJ_TEXT) != 0);
}

/* Read the whole value (object or element) of a target, then its select. */
static int read_target(const llg_vpi_target_t* target, llg_vpi_datum_t* out) {
    llg_vpi_model_object_t* object = row_object(target->object);
    memset(out, 0, sizeof(*out));
    out->packed = VPI_EMPTY;
    if (!object) return 0;
    if (is_parameter_type(object->type)) {
        if (object->is_real) {
            out->is_real = 1;
            out->real = object->real_constant;
            return 1;
        }
        out->packed = parameter_value(object);
    } else if (object->is_real) {
        double* cell = real_cell(target);
        if (!cell) return 0;
        out->is_real = 1;
        out->real = *cell;
        return 1;
    } else {
        sv4_t* cell = packed_cell(target);
        if (!cell) return 0;
        out->packed = sv4_clone(cell);
    }
    if (target->span) {
        sv4_t selected = sv4_part_select(out->packed, (int64_t)target->lsb + target->span - 1,
                                         (int64_t)target->lsb);
        llg_sv4_set_signed(&selected, 0);
        sv4_replace(&out->packed, selected);
    }
    return 1;
}

static int target_writable(const llg_vpi_target_t* target, const char** reason) {
    llg_vpi_model_object_t* object = row_object(target->object);
    if (!object || !has_storage(object)) {
        *reason = "VPI object has no writable value";
        return 0;
    }
    if (object->type == vpiNetArray) {
        *reason = "elements of a net array are published from their nets and are read-only";
        return 0;
    }
    if (is_array_type(object->type) && target->element == LLG_VPI_NO_ELEMENT) {
        *reason = "an unpacked array has no value; write its elements";
        return 0;
    }
    return 1;
}

/* Deposit a value through the storage write APIs: waiters, dependency marks
 * and active forces apply exactly as for a procedural blocking write. */
static void write_target(const llg_vpi_target_t* target, const llg_vpi_datum_t* datum) {
    llg_vpi_model_object_t* object = row_object(target->object);
    if (object->is_real) {
        double value = datum->is_real ? datum->real : sv4_to_real(datum->packed);
        if (object->flags & LLG_VPI_OBJ_SHORTREAL) value = (double)(float)value;
        llg_ba_d(real_cell(target), value);
        return;
    }
    sv4_t* cell = packed_cell(target);
    if (target->span) {
        sv4_t updated = sv4_clone(cell);
        sv4_t part = datum->is_real ? sv4_from_real(datum->real, target->span, target->is_signed)
                                    : sv4_resize(datum->packed, target->span, target->is_signed);
        /* A two-state member of a four-state structure stores 0 for x/z. */
        if (target->two_state) sv4_replace(&part, sv4_to_two_state(part));
        sv4_part_select_set(&updated, (int64_t)target->lsb + target->span - 1,
                            (int64_t)target->lsb, part);
        if (object->flags & LLG_VPI_OBJ_TWO_STATE)
            sv4_replace(&updated, sv4_to_two_state(updated));
        llg_ba(cell, updated);
        sv4_destroy(&part);
        sv4_destroy(&updated);
        return;
    }
    sv4_t value = datum->is_real ? sv4_from_real(datum->real, object->width, object->is_signed)
                                 : sv4_resize(datum->packed, object->width, object->is_signed);
    if (object->flags & LLG_VPI_OBJ_TWO_STATE) sv4_replace(&value, sv4_to_two_state(value));
    llg_ba(cell, value);
    sv4_destroy(&value);
}

/* The force/release descriptor of a packed target (one storage part). */
static llg_force_part_t force_part(const llg_vpi_target_t* target) {
    llg_vpi_model_object_t* object = row_object(target->object);
    llg_force_part_t part;
    memset(&part, 0, sizeof(part));
    part.target = packed_cell(target);
    part.net = target->element == LLG_VPI_NO_ELEMENT ? object->net : NULL;
    uint32_t lsb = target->span ? target->lsb : 0u;
    uint32_t width = target->span ? target->span : object->width;
    part.left = (int64_t)lsb + width - 1;
    part.right = (int64_t)lsb;
    part.width = width;
    part.value_lsb = 0;
    part.two_state = (object->flags & LLG_VPI_OBJ_TWO_STATE) != 0;
    return part;
}

/* ── handle → value source ──────────────────────────────────────────────── */

/* Resolve a value-bearing handle to its storage target. Argument snapshots,
 * constants and calls have no target; see read_handle. */
static int handle_target(const llg_vpi_slot_t* slot, llg_vpi_target_t* target) {
    llg_vpi_model_object_t* object = row_object(slot->object);
    target->object = slot->object;
    target->element = LLG_VPI_NO_ELEMENT;
    target->lsb = 0;
    target->span = 0;
    target->is_signed = object ? object->is_signed : 0;
    target->two_state = 0;
    switch (slot->kind) {
        case SLOT_OBJECT:
            if (!object || !(has_storage(object) || is_parameter_type(object->type)) ||
                is_array_type(object->type))
                return 0;
            /* An unpacked union member is the low bits of the shared extent. */
            if (object->flags & LLG_VPI_OBJ_SLICE) target->span = object->width;
            return 1;
        case SLOT_ELEMENT:
            target->element = slot->element;
            return 1;
        case SLOT_SELECT:
            target->element = slot->element;
            target->lsb = slot->lsb;
            target->span = slot->span;
            target->is_signed = 0;
            return 1;
        case SLOT_MEMBER: {
            const llg_vpi_member_node_t* node = node_at((uint32_t)slot->aux);
            /* An unpacked array member has no value of its own (37.17). */
            if (!node || node->type == vpiRegArray || !node->width) return 0;
            target->element = slot->element;
            target->lsb = slot->lsb;
            target->span = slot->span;
            target->is_signed = (node->flags & LLG_VPI_TS_SIGNED) != 0;
            target->two_state = (node->flags & LLG_VPI_TS_TWO_STATE) != 0;
            return 1;
        }
        default:
            return 0;
    }
}

static llg_vpi_call_t* slot_call(const llg_vpi_slot_t* slot) {
    if (slot->kind != SLOT_CALL && slot->kind != SLOT_ARGUMENT) return NULL;
    llg_vpi_callsite_t* site = (llg_vpi_callsite_t*)slot->pointer;
    return site ? site->active : NULL;
}

static int read_conn(uint32_t row, uint32_t index, llg_vpi_datum_t* out, int* is_signed);
static int code_value(uint32_t index, llg_vpi_datum_t* out, int* is_signed);
static int handle_type(const llg_vpi_slot_t* slot);
static vpiHandle code_handle(uint32_t index);
static int code_is_unscoped_block(uint32_t index);
static const llg_vpi_code_t* code_at(uint32_t index);
static uint32_t code_visible(uint32_t index);
static uint32_t code_child(const llg_vpi_code_t* node, int relation);
static const llg_fcov_fsm_t* slot_fsm(const llg_vpi_slot_t* slot);

/* The width and sign of the value a handle reads, or 0 when it has none. */
static int read_handle(const llg_vpi_slot_t* slot, llg_vpi_datum_t* out, int* is_signed) {
    llg_vpi_target_t target;
    *is_signed = 0;
    memset(out, 0, sizeof(*out));
    out->packed = VPI_EMPTY;
    if (handle_target(slot, &target)) {
        *is_signed = target.is_signed;
        return read_target(&target, out);
    }
    switch (slot->kind) {
        case SLOT_OBJECT: {
            llg_vpi_model_object_t* object = row_object(slot->object);
            if (!object) return 0;
            if (is_primitive_type(object->type)) {
                /* 37.31: a primitive's value is its output terminal's. */
                for (uint32_t i = 0; i < object->conn_count; ++i)
                    if (object->conns[i].direction == vpiOutput)
                        return read_conn(slot->object, i, out, is_signed);
                return 0;
            }
            if (object->type == vpiContAssign) {
                /* The value the assignment drives: its right-hand side
                 * converted to the left-hand side size (decision A19-D5). */
                if (!read_conn(slot->object, 1u, out, is_signed)) return 0;
                if (out->is_real || !object->width) return 1;
                /* `*is_signed` is a 0/1 flag; the resize takes an `int8_t`. */
                sv4_t sized =
                    sv4_resize(out->packed, object->width, (int8_t)(*is_signed != 0));
                sv4_replace(&out->packed, sized);
                *is_signed = 0;
                llg_sv4_set_signed(&out->packed, 0);
                return 1;
            }
            return 0;
        }
        case SLOT_CONN:
        case SLOT_PRIMTERM:
            return read_conn(slot->object, (uint32_t)slot->aux, out, is_signed);
        case SLOT_CABIT: {
            /* 37.43: bit `vpiOffset` of the value the assignment drives. */
            llg_vpi_slot_t whole = *slot;
            whole.kind = SLOT_OBJECT;
            if (!read_handle(&whole, out, is_signed)) return 0;
            if (out->is_real || (uint32_t)slot->aux >= llg_sv4_width(out->packed)) return 0;
            sv4_replace(&out->packed, sv4_bit_select(out->packed, (uint64_t)slot->aux));
            *is_signed = 0;
            return 1;
        }
        case SLOT_CODE:
            return code_value((uint32_t)slot->aux, out, is_signed);
        case SLOT_FSMSTATE: {
            /* The value of the state's parameter, as wide as the state
             * expression (40.5.3). */
            const llg_fcov_fsm_t* fsm = slot_fsm(slot);
            if (!fsm) return 0;
            out->packed = sv4_from_i64((int64_t)fsm->states[slot->aux].value, fsm->size);
            return 1;
        }
        case SLOT_CONSTANT:
            if (is_string_constant(slot)) {
                /* 8 bits per character, the first most significant. */
                uint32_t width = slot->count ? slot->count * 8u : 8u;
                out->packed = llg_string_to_packed(
                    llg_string_bytes((const char*)slot->items, slot->count), width, 0);
                *is_signed = 0;
                return 1;
            }
            out->packed = sv4_from_i64(slot->left, 32);
            llg_sv4_set_signed(&out->packed, 1);
            *is_signed = 1;
            return 1;
        case SLOT_ENUMCONST: {
            /* An enum constant has the value of its enum's base type. */
            const llg_vpi_enum_const_t* constant = &g_vpi.types->consts[slot->aux];
            const llg_vpi_typespec_t* owner = NULL;
            for (uint32_t i = 0; i < g_vpi.types->typespec_count && !owner; ++i) {
                const llg_vpi_typespec_t* typespec = &g_vpi.types->typespecs[i];
                if ((uint32_t)slot->aux >= typespec->first_const &&
                    (uint32_t)slot->aux - typespec->first_const < typespec->const_count)
                    owner = typespec;
            }
            if (!owner) return 0;
            *is_signed = (owner->flags & LLG_VPI_TS_SIGNED) != 0;
            out->packed = digits_value(constant->value, owner->width, *is_signed, 0);
            return 1;
        }
        case SLOT_ARGUMENT: {
            llg_vpi_call_t* call = slot_call(slot);
            if (!call || slot->aux < 0 || slot->aux >= call->arg_count) return 0;
            llg_vpi_arg_t* argument = &call->args[slot->aux];
            if (argument->is_real) {
                out->is_real = 1;
                out->real = argument->real;
            } else {
                out->packed = sv4_clone(&argument->packed);
                *is_signed = argument->is_signed;
            }
            return 1;
        }
        case SLOT_CALL: {
            llg_vpi_call_t* call = slot_call(slot);
            if (!call || !call->is_function) return 0;
            if (call->has_real_return) {
                out->is_real = 1;
                out->real = call->real_return;
            } else {
                out->packed = sv4_clone(&call->return_value);
                *is_signed = llg_sv4_signed(call->return_value);
            }
            return 1;
        }
        default:
            return 0;
    }
}

/* ── structure objects (37.5-37.15, 37.25, 37.31, 37.43, 37.58) ────────── */

static vpiHandle make_list_iterator(int type, uintptr_t* items, uint32_t count);

static const llg_vpi_conn_t* row_conn(uint32_t row, uint32_t index) {
    llg_vpi_model_object_t* object = row_object(row);
    return object && index < object->conn_count ? &object->conns[index] : NULL;
}

static vpiHandle make_aux_slot(llg_vpi_slot_kind_t kind, uint32_t row, uint32_t aux) {
    uint32_t index = alloc_slot(kind);
    if (index == LLG_VPI_NONE) return NULL;
    g_vpi.slots[index].object = row;
    g_vpi.slots[index].aux = (int32_t)aux;
    return encode_handle(index);
}

/* The handle of connection `index` of `row`: the catalog object, select or
 * element it names, else a constant or expression handle of its own. */
static vpiHandle conn_handle(uint32_t row, uint32_t index) {
    const llg_vpi_conn_t* conn = row_conn(row, index);
    if (!conn) return NULL;
    const llg_vpi_compile_arg_t* ref = &conn->ref;
    if (ref->object >= 0) {
        uint32_t target = (uint32_t)ref->object;
        switch (ref->ref_kind) {
            case LLG_VPI_ARG_OBJECT:
                return object_handle(target);
            case LLG_VPI_ARG_SELECT:
                return make_select(target, LLG_VPI_NO_ELEMENT, (uint32_t)ref->offset,
                                   ref->select_width, ref->left, ref->right, NULL);
            case LLG_VPI_ARG_ELEMENT:
                if (ref->offset < g_vpi.element_counts[target])
                    return make_element(target, ref->offset, NULL);
                break;
            default:
                break;
        }
    }
    /* An expression or literal of a model generated for VPI applications
     * is an expression object (37.55). */
    if ((ref->ref_kind == LLG_VPI_ARG_EXPRESSION || ref->ref_kind == LLG_VPI_ARG_CONSTANT) &&
        ref->code && g_vpi.code)
        return code_handle(ref->code - 1u);
    return make_aux_slot(SLOT_CONN, row, index);
}

/* Read connection `index` of `row` as its conn_handle reads: an
 * expression of a model generated for VPI applications evaluates its
 * operation tree; another opaque expression has no readable value. */
static int read_conn(uint32_t row, uint32_t index, llg_vpi_datum_t* out, int* is_signed) {
    memset(out, 0, sizeof(*out));
    out->packed = VPI_EMPTY;
    *is_signed = 0;
    const llg_vpi_conn_t* conn = row_conn(row, index);
    if (!conn) return 0;
    const llg_vpi_compile_arg_t* ref = &conn->ref;
    if (ref->ref_kind == LLG_VPI_ARG_CONSTANT) {
        *is_signed = ref->is_signed;
        out->packed = digits_value(conn->value, ref->width, ref->is_signed, 0);
        return 1;
    }
    if (ref->ref_kind == LLG_VPI_ARG_EXPRESSION && ref->code && g_vpi.code)
        return code_value(ref->code - 1u, out, is_signed);
    if (ref->object < 0) return 0;
    llg_vpi_slot_t view;
    memset(&view, 0, sizeof(view));
    view.object = (uint32_t)ref->object;
    view.element = LLG_VPI_NO_ELEMENT;
    switch (ref->ref_kind) {
        case LLG_VPI_ARG_OBJECT:
            view.kind = SLOT_OBJECT;
            break;
        case LLG_VPI_ARG_SELECT:
            view.kind = SLOT_SELECT;
            view.lsb = (uint32_t)ref->offset;
            view.span = ref->select_width;
            view.left = ref->left;
            view.right = ref->right;
            break;
        case LLG_VPI_ARG_ELEMENT:
            if (ref->offset >= g_vpi.element_counts[view.object]) return 0;
            view.kind = SLOT_ELEMENT;
            view.element = ref->offset;
            break;
        default:
            return 0;
    }
    return read_handle(&view, out, is_signed);
}

/* Length of `name` without its trailing `[index]` groups. */
static size_t name_base_length(const char* name, int64_t* last_index, int* groups) {
    size_t length = strlen(name);
    *groups = 0;
    while (length > 1 && name[length - 1] == ']') {
        size_t open = length - 1;
        while (open > 0 && name[open] != '[') --open;
        if (name[open] != '[' || open == 0) break;
        if (*groups == 0) *last_index = strtoll(name + open + 1, NULL, 10);
        ++*groups;
        length = open;
    }
    return length;
}

/* The instance array row an instance or primitive row is an element of, or
 * LLG_VPI_NONE. */
static uint32_t instance_array_of(uint32_t row, int64_t* index, int* dims) {
    llg_vpi_model_object_t* object = row_object(row);
    if (!object || object->flags & LLG_VPI_OBJ_UNNAMED) return LLG_VPI_NONE;
    int64_t last = 0;
    int groups;
    size_t base = name_base_length(object->name, &last, &groups);
    if (!groups) return LLG_VPI_NONE;
    const char* scope = object->parent ? object->parent->full_name : "";
    size_t scope_length = strlen(scope);
    size_t total = scope_length + 1u + base + 1u;
    char* full = (char*)malloc(total);
    if (!full) return LLG_VPI_NONE;
    if (scope_length) snprintf(full, total, "%s.%.*s", scope, (int)base, object->name);
    else snprintf(full, total, "%.*s", (int)base, object->name);
    uint32_t array = find_row(full);
    free(full);
    if (array == LLG_VPI_NONE || !is_instance_array_type(g_vpi.objects[array].type) ||
        g_vpi.objects[array].dim_count != (uint32_t)groups)
        return LLG_VPI_NONE;
    if (index) *index = last;
    if (dims) *dims = groups;
    return array;
}

/* Element `indices` (one per dimension) of instance array `row`. */
static uint32_t instance_element(uint32_t row, const int64_t* indices, uint32_t count) {
    llg_vpi_model_object_t* array = row_object(row);
    if (!array || count != array->dim_count) return LLG_VPI_NONE;
    size_t total = strlen(array->full_name) + 1u + 24u * count;
    char* full = (char*)malloc(total);
    if (!full) return LLG_VPI_NONE;
    size_t at = (size_t)snprintf(full, total, "%s", array->full_name);
    for (uint32_t d = 0; d < count && at < total; ++d) {
        uint64_t offset;
        if (!index_offset(array->dims[2u * d], array->dims[2u * d + 1u], indices[d], &offset)) {
            free(full);
            return LLG_VPI_NONE;
        }
        at += (size_t)snprintf(full + at, total - at, "[%lld]", (long long)indices[d]);
    }
    uint32_t element = find_row(full);
    free(full);
    return element;
}

/* The elements of instance array `row` in declared order, leftmost index
 * of each dimension first, as a list iterator. */
static vpiHandle iterate_instance_elements(uint32_t row, int type) {
    llg_vpi_model_object_t* array = row_object(row);
    uint64_t total = g_vpi.element_counts[row];
    if (!total || total > UINT32_MAX || array->dim_count > 16) return NULL;
    uintptr_t* items = (uintptr_t*)calloc((size_t)total, sizeof(*items));
    if (!items) {
        vpi_error("LLG_VPI_NOMEM", "VPI iterator allocation failed");
        return NULL;
    }
    uint32_t count = 0;
    for (uint64_t flat = 0; flat < total; ++flat) {
        int64_t indices[16];
        uint64_t rest = flat;
        for (uint32_t d = array->dim_count; d-- > 0;) {
            int64_t left = array->dims[2u * d], right = array->dims[2u * d + 1u];
            uint64_t extent = (uint64_t)(left > right ? left - right : right - left) + 1u;
            uint64_t offset = rest % extent;
            rest /= extent;
            indices[d] = left >= right ? left - (int64_t)offset : left + (int64_t)offset;
        }
        uint32_t element = instance_element(row, indices, array->dim_count);
        if (element != LLG_VPI_NONE) items[count++] = (uintptr_t)object_handle(element);
    }
    return make_list_iterator(type, items, count);
}

/* vpiLowConn/vpiHighConn of a port bit (37.14): the same bit of the
 * connected object or constant select. */
static vpiHandle port_bit_conn(const llg_vpi_slot_t* slot, int high) {
    llg_vpi_model_object_t* port = row_object(slot->object);
    uint32_t bit = (uint32_t)slot->aux, base = 0, width, row;
    if (!high) {
        if (!port->target) return NULL;
        row = row_of(port->target);
        width = port->target->width;
    } else {
        const llg_vpi_conn_t* conn = row_conn(slot->object, 0u);
        if (!conn) return NULL;
        if (conn->ref.object < 0 || (conn->ref.ref_kind != LLG_VPI_ARG_OBJECT &&
                                     conn->ref.ref_kind != LLG_VPI_ARG_SELECT)) {
            vpi_error("LLG_VPI_UNSUPPORTED",
                      "the high connection of this port is an expression without bits");
            return NULL;
        }
        row = (uint32_t)conn->ref.object;
        if (conn->ref.ref_kind == LLG_VPI_ARG_SELECT) {
            base = (uint32_t)conn->ref.offset;
            width = conn->ref.select_width;
        } else {
            width = g_vpi.objects[row].width;
        }
    }
    llg_vpi_model_object_t* target = row_object(row);
    if (!target || target->is_real || !target->packed || is_array_type(target->type) ||
        target->type == vpiPackedArrayVar || bit >= width) {
        vpi_error("LLG_VPI_UNSUPPORTED", "the connection of this port bit has no matching bit");
        return NULL;
    }
    uint32_t offset = base + bit;
    int64_t left, right;
    packed_range(target, &left, &right);
    int64_t index = left >= right ? right + (int64_t)offset : right - (int64_t)offset;
    return make_select(row, LLG_VPI_NO_ELEMENT, offset, 1, index, index, NULL);
}

/* vpiRange of an unpacked array or instance array: one range handle per
 * dimension, leftmost first. */
static vpiHandle iterate_dims(uint32_t row, int type) {
    llg_vpi_model_object_t* object = row_object(row);
    uintptr_t* items = (uintptr_t*)calloc(object->dim_count + 1u, sizeof(*items));
    if (!items) return NULL;
    uint32_t count = 0;
    for (uint32_t d = 0; d < object->dim_count; ++d) {
        uint32_t range = alloc_slot(SLOT_RANGE);
        if (range == LLG_VPI_NONE) break;
        g_vpi.slots[range].object = row;
        g_vpi.slots[range].aux = (int32_t)d;
        array_dim(object, d, &g_vpi.slots[range].left, &g_vpi.slots[range].right);
        items[count++] = (uintptr_t)encode_handle(range);
    }
    return make_list_iterator(type, items, count);
}

/* Whether a net, variable, bit or named event handle has the vpiIndex
 * relations of 37.16, 37.17 and 37.25 (an expression object answers its
 * own). */
static int has_index_relation(const llg_vpi_slot_t* slot) {
    if (slot->kind != SLOT_OBJECT && slot->kind != SLOT_ELEMENT &&
        !(slot->kind == SLOT_SELECT && slot->span == 1))
        return 0;
    int type = handle_type(slot);
    return is_variable_type(type) || type == vpiRegArray || type == vpiNet ||
           type == vpiNetBit || type == vpiRegBit || type == vpiNamedEvent ||
           type == vpiStringVar || type == vpiChandleVar || type == vpiClassVar;
}

/* The ranges of the packed dimensions of a value (37.16 detail 23, 37.17
 * detail 4), leftmost first: its typespec's ranges, or its one declared
 * range; NULL for a scalar. A range's vpiParent is the declaration. */
static vpiHandle iterate_packed_ranges(const llg_vpi_slot_t* slot) {
    const llg_vpi_model_object_t* object = row_object(slot->object);
    const llg_vpi_typespec_t* typespec = value_typespec(slot);
    uint32_t count = 0;
    const int64_t* bounds = NULL;
    int64_t single[2];
    if (typespec && (typespec->type == vpiLogicTypespec || typespec->type == vpiBitTypespec ||
                     typespec->type == vpiPackedArrayTypespec)) {
        count = typespec->range_count;
        bounds = count ? &g_vpi.types->ranges[2u * typespec->first_range] : NULL;
    } else if (object && (object->flags & LLG_VPI_OBJ_RANGE)) {
        single[0] = object->left;
        single[1] = object->right;
        bounds = single;
        count = 1;
    }
    if (!count) return NULL;
    /* alloc_slot may move the slot table: keep no slot pointer across it. */
    uint32_t row = slot->object;
    uintptr_t* items = (uintptr_t*)calloc(count, sizeof(*items));
    if (!items) {
        vpi_error("LLG_VPI_NOMEM", "VPI iterator allocation failed");
        return NULL;
    }
    uint32_t made = 0;
    for (uint32_t i = 0; i < count; ++i) {
        uint32_t range = alloc_slot(SLOT_RANGE);
        if (range == LLG_VPI_NONE) break;
        g_vpi.slots[range].object = row;
        g_vpi.slots[range].left = bounds[2u * i];
        g_vpi.slots[range].right = bounds[2u * i + 1u];
        g_vpi.slots[range].aux = (int32_t)i;
        items[made++] = (uintptr_t)encode_handle(range);
    }
    return make_list_iterator(vpiRange, items, made);
}

/* The vpiIndex iteration of a net, variable or named event (37.16 details
 * 10 and 22, 37.17 details 5 and 13, 37.18 detail 6, 37.25 detail 1): the
 * index of a bit, then the indices of the array element that holds it,
 * innermost first. Outside an array the text makes the iteration NULL for a
 * net or named event, which return NULL without an error, as do a whole
 * variable array and a packed array var (no packed parent); it
 * defines no iteration for another whole variable or for a bit of a vector
 * outside an array, which raise a vpiError (decision A19-D28). */
static vpiHandle iterate_indices(const llg_vpi_slot_t* slot) {
    const llg_vpi_model_object_t* object = row_object(slot->object);
    if (!object) return NULL;
    int bit = slot->kind == SLOT_SELECT && slot->span == 1;
    uint32_t dims = slot->element != LLG_VPI_NO_ELEMENT && slot->kind != SLOT_OBJECT &&
                            is_array_type(object->type)
                        ? object->dim_count : 0u;
    uint32_t count = dims + (bit ? 1u : 0u);
    if (!dims) {
        int type = bit ? 0 : handle_type(slot);
        if (type == vpiNet || type == vpiNamedEvent || type == vpiPackedArrayVar ||
            type == vpiRegArray)
            return NULL;
        vpi_error("LLG_VPI_RELATION", bit ? "vpiIndex iteration of a bit outside an array"
                                          : "vpiIndex iteration of a variable outside an array");
        return NULL;
    }
    /* make_constant may move the slot table: keep no slot pointer across it. */
    int64_t bit_index = slot->left;
    uint64_t rest = slot->element;
    uintptr_t* items = (uintptr_t*)calloc(count, sizeof(*items));
    if (!items) {
        vpi_error("LLG_VPI_NOMEM", "VPI iterator allocation failed");
        return NULL;
    }
    uint32_t made = 0;
    if (bit) {
        vpiHandle index = make_constant(bit_index, vpiDecConst);
        if (index) items[made++] = (uintptr_t)index;
    }
    /* Row-major element number: the last dimension varies fastest. */
    for (uint32_t d = dims; d-- > 0;) {
        int64_t left = 0, right = 0;
        array_dim(object, d, &left, &right);
        uint64_t extent = (uint64_t)(left >= right ? left - right : right - left) + 1u;
        uint64_t offset = rest % extent;
        rest /= extent;
        int64_t value = left >= right ? left - (int64_t)offset : left + (int64_t)offset;
        vpiHandle index = make_constant(value, vpiDecConst);
        if (index) items[made++] = (uintptr_t)index;
    }
    return make_list_iterator(vpiIndex, items, made);
}

/* A list iterator of `count` auxiliary handles of `kind` on `row`, from aux
 * count - 1 down to 0 when `descending` (port bits, most significant
 * first like vpiBit of a vector). */
static vpiHandle iterate_aux(llg_vpi_slot_kind_t kind, uint32_t row, uint32_t count, int type,
                             int descending) {
    if (!count) return NULL;
    uintptr_t* items = (uintptr_t*)calloc(count, sizeof(*items));
    if (!items) {
        vpi_error("LLG_VPI_NOMEM", "VPI iterator allocation failed");
        return NULL;
    }
    uint32_t made = 0;
    for (uint32_t i = 0; i < count; ++i) {
        vpiHandle handle = make_aux_slot(kind, row, descending ? count - 1u - i : i);
        if (!handle) break;
        items[made++] = (uintptr_t)handle;
    }
    return make_list_iterator(type, items, made);
}

/* ── module paths and timing checks (37.35, 37.36) ──────────────────────── */

/* How the model was generated (llg_vpi_model_delays); kept apart from
 * g_vpi, which model initialization clears. */
static int g_vpi_delays_writable;
static int g_vpi_delay_select = 1;

void llg_vpi_model_delays(int writable, int delay_select) {
    g_vpi_delays_writable = writable != 0;
    g_vpi_delay_select = delay_select >= 0 && delay_select <= 2 ? delay_select : 1;
}

static uint64_t time_unit_of(const llg_vpi_model_object_t* object);

static int is_term_role(int role) {
    return role == LLG_VPI_ROLE_PATH_IN || role == LLG_VPI_ROLE_PATH_OUT ||
           role == LLG_VPI_ROLE_PATH_DATA || role == LLG_VPI_ROLE_TCHK_REF ||
           role == LLG_VPI_ROLE_TCHK_DATA;
}

/* The first connection of `row` with `role`, or LLG_VPI_NONE. */
static uint32_t role_conn(uint32_t row, int role) {
    llg_vpi_model_object_t* object = row_object(row);
    if (!object) return LLG_VPI_NONE;
    for (uint32_t i = 0; i < object->conn_count; ++i)
        if (object->conns[i].role == role) return i;
    return LLG_VPI_NONE;
}

/* The path term or tchk term of `row` with `role`; NULL when it has none. */
static vpiHandle role_term(uint32_t row, int role) {
    uint32_t conn = role_conn(row, role);
    return conn == LLG_VPI_NONE ? NULL : make_aux_slot(SLOT_TERM, row, conn);
}

/* The connections of `row` whose role is in the bit set `roles`, in
 * order: terminals as term handles, other entries as their expressions. */
static vpiHandle iterate_roles(uint32_t row, int type, unsigned roles) {
    llg_vpi_model_object_t* object = row_object(row);
    if (!object || !object->conn_count) return NULL;
    uintptr_t* items = (uintptr_t*)calloc(object->conn_count, sizeof(*items));
    if (!items) {
        vpi_error("LLG_VPI_NOMEM", "VPI iterator allocation failed");
        return NULL;
    }
    uint32_t made = 0;
    for (uint32_t i = 0; i < object->conn_count; ++i) {
        int role = object->conns[i].role;
        if (role < 0 || role > 31 || !(roles >> role & 1u)) continue;
        vpiHandle handle = is_term_role(role) ? make_aux_slot(SLOT_TERM, row, i)
                                              : conn_handle(row, i);
        if (!handle) break;
        items[made++] = (uintptr_t)handle;
    }
    return make_list_iterator(type, items, made);
}

/* The delay object kinds of vpi_get_delays/vpi_put_delays. */
enum {
    DELAYS_NONE = 0,
    DELAYS_PATH,
    DELAYS_TCHK,
    DELAYS_PRIMITIVE,
    DELAYS_CONT_ASSIGN,
    DELAYS_NET,     /* a net, net bit or net array element (37.16) */
    DELAYS_PORT,    /* module input port delays of a port or port bit (37.14) */
    DELAYS_CONTROL, /* the delay of a delay control (37.63) */
};

/* What a delay record addresses: the kind, the object holding the delays
 * (a port bit's and an intermodule path's load port), the bits of a port
 * (`bit`, `n_bits`) and the delay expression of a delay control. */
typedef struct {
    int kind;
    llg_vpi_model_object_t* row;
    uint32_t bit;
    uint32_t n_bits;
    uint32_t code;
    int imp;
} llg_vpi_delays_t;

static int delay_kind(const llg_vpi_model_object_t* object) {
    if (!object) return DELAYS_NONE;
    if (object->is_net) return DELAYS_NET;
    if (object->type == vpiPort) return DELAYS_PORT;
    if (!object->timing) return DELAYS_NONE;
    if (object->type == vpiModPath) return DELAYS_PATH;
    if (object->type == vpiTchk) return DELAYS_TCHK;
    if (is_primitive_type(object->type)) return DELAYS_PRIMITIVE;
    if (object->type == vpiContAssign) return DELAYS_CONT_ASSIGN;
    return DELAYS_NONE;
}

/* `ticks` in the time format of `type` for `scope` (38.10). */
static void delay_time(int64_t ticks, int type, const llg_vpi_model_object_t* scope,
                       s_vpi_time* out) {
    out->type = type;
    out->high = 0;
    out->low = 0;
    out->real = 0.0;
    if (type == vpiSimTime) {
        uint64_t value = (uint64_t)ticks;
        out->high = (PLI_UINT32)(value >> 32);
        out->low = (PLI_UINT32)value;
        return;
    }
    uint64_t precision = llg_time_precision_fs();
    uint64_t unit = time_unit_of(scope);
    if (!precision) precision = 1;
    if (!unit) unit = precision;
    out->real = (double)((long double)ticks * precision / unit);
}

/* A delay value in design-precision ticks; 0 when it cannot be one. */
static int delay_ticks(const s_vpi_time* time, int type, const llg_vpi_model_object_t* scope,
                       int64_t* out) {
    if (type == vpiSimTime) {
        *out = (int64_t)(((uint64_t)time->high << 32) | time->low);
        return 1;
    }
    uint64_t precision = llg_time_precision_fs();
    uint64_t unit = time_unit_of(scope);
    if (!precision) precision = 1;
    if (!unit) unit = precision;
    double ticks = time->real * (double)unit / (double)precision;
    if (!(ticks > -9.2e18 && ticks < 9.2e18)) return 0;
    *out = (int64_t)llround(ticks);
    return 1;
}

/* The op of `tm` holding limit `limit` of a timing check, or NULL. */
static const llg_sdf_op_t* limit_op(const llg_vpi_timing_t* tm, uint32_t limit) {
    for (uint32_t i = 0; i < tm->op_count; ++i)
        if (tm->ops[i].first == limit) return &tm->ops[i];
    return NULL;
}

static int code_type(const llg_vpi_code_t* node);

/* The scheduler ticks of the value of delay expression `code`, which `scope`
 * evaluates in its time unit as a delay does (9.4.1: x and z are 0, a
 * negative integral value is an unsigned time; a real one is rounded). */
static int code_delay_ticks(uint32_t code, const llg_vpi_model_object_t* scope, int64_t* out) {
    llg_vpi_datum_t value;
    int is_signed;
    if (!code_value(code, &value, &is_signed)) return 0;
    uint64_t precision = llg_time_precision_fs();
    uint64_t unit = time_unit_of(scope);
    if (!precision) precision = 1;
    if (!unit || unit < precision) unit = precision;
    uint64_t ticks = value.is_real ? sv4_real_delay_ticks(value.real, unit / precision, 1u)
                                   : sv4_delay_ticks(value.packed, unit / precision);
    sv4_destroy(&value.packed);
    *out = ticks > (uint64_t)INT64_MAX ? INT64_MAX : (int64_t)ticks;
    return 1;
}

/* The current values of the runtime-valued delays of `object` (its
 * vpiDelay expressions, 37.3.4), as a driver evaluates them: one value
 * serves every transition, two give turn-off as the smaller (28.16). */
static int evaluate_delays(const llg_vpi_model_object_t* object, int64_t out[3]) {
    const llg_vpi_timing_t* tm = object->timing;
    if (!tm || !tm->delay || !g_vpi.code) return 0;
    uint32_t root = tm->delay - 1u;
    const llg_vpi_code_t* node = code_at(root);
    if (!node) return 0;
    uint32_t codes[3];
    uint32_t count = 0;
    if (code_type(node) == vpiOperation && node->subtype == vpiListOp) {
        for (uint32_t i = 0; i < node->count && count < 3u; ++i)
            codes[count++] = node->first + i;
    } else {
        codes[count++] = root;
    }
    for (uint32_t i = 0; i < count; ++i)
        if (!code_delay_ticks(codes[i], object, &out[i])) return 0;
    if (count == 1) out[1] = out[0];
    if (count < 3) out[2] = count == 1 ? out[0] : out[0] < out[1] ? out[0] : out[1];
    return 1;
}

/* The gate delay table entries of a runtime-valued primitive of a VPI
 * model (llg_gate_delays_refresh): the delays in use, then the values last
 * evaluated; NULL for any other primitive. */
static uint64_t* runtime_gate(const llg_vpi_model_object_t* object) {
    const llg_vpi_timing_t* tm = object->timing;
    if (!tm || tm->declared_count || !tm->count || !tm->op_count || !tm->ops[0].gate) return NULL;
    return tm->ops[0].gate;
}

/* Interconnect path transition indices (llg_module_path_t.delays). */
enum { DELAY_T01 = 0, DELAY_T10 = 1, DELAY_T0Z = 2 };

/* The interconnect load of bit `bit` of port `port`: its SDF load, or one
 * the runtime builds on the port's own net when `create` (NULL when the
 * port has none and cannot take one). */
static llg_sdf_load_t* port_load(const llg_vpi_model_object_t* port, uint32_t bit, int create) {
    const llg_vpi_timing_t* tm = port->timing;
    for (uint32_t i = 0; tm && i < tm->op_count; ++i)
        if (tm->ops[i].kind == LLG_SDF_LOAD && tm->ops[i].first == bit) return tm->ops[i].load;
    const llg_vpi_model_object_t* net = port->target;
    if (!net || !net->net || net->packed != &net->net->resolved ||
        net->net->width != port->width)
        return NULL;
    llg_net_t* resolved = net->net;
    if (!create) {
        /* A stage the runtime built earlier answers; none reads 0. */
        if (!resolved->path_dest || bit >= resolved->path_dest->n_paths) return NULL;
        const llg_module_path_t* path = &resolved->path_dest->paths[bit];
        if (!path->interconnect) return NULL;
    }
    return llg_sdf_port_load(resolved, bit);
}

/* Current value `index` of the delays or limits of `delays`: its runtime
 * value when it has one, else as declared. Path delays are transitions in
 * the order of llg_module_path_t.delays (01, 10, 0z, z1, 1z, z0, ...), so
 * the first one, two, three or six are those of a declaration with as many
 * values; primitive, continuous assignment, net and port delays are rise,
 * fall and turn-off. */
static int64_t current_delay(const llg_vpi_delays_t* delays, uint32_t index) {
    const llg_vpi_model_object_t* object = delays->row;
    const llg_vpi_timing_t* tm = object->timing;
    int64_t evaluated[3];
    switch (delays->kind) {
        case DELAYS_PATH:
            if (tm->op_count && tm->ops[0].path) return (int64_t)tm->ops[0].path->delays[index];
            break;
        case DELAYS_PRIMITIVE: {
            uint64_t* gate = runtime_gate(object);
            /* A runtime-valued gate that has not evaluated its delays yet
             * reads its expressions. */
            if (gate && gate[3] == UINT64_MAX)
                return index < 3u && evaluate_delays(object, evaluated) ? evaluated[index] : 0;
            if (tm->op_count && tm->ops[0].gate) return (int64_t)tm->ops[0].gate[index];
            break;
        }
        case DELAYS_TCHK: {
            const llg_sdf_op_t* op = limit_op(tm, index);
            int second = op && (op->flags & LLG_SDF_SECOND);
            if (op && op->kind == LLG_SDF_CHECK && op->check)
                return second ? op->check->limit2 : op->check->limit;
            if (op && op->kind == LLG_SDF_PAIR && op->checks && op->checks->pairs &&
                op->index < op->checks->n_pairs)
                return second ? op->checks->pairs[op->index].after
                              : op->checks->pairs[op->index].before;
            break;
        }
        case DELAYS_NET: {
            /* The delay the net group uses (a runtime-valued one as last
             * evaluated); a net without a delay reads 0. */
            const llg_net_t* net = object->net;
            if (net && net->net_delayed && index < 3u) {
                uint64_t value = index == 0u   ? net->propagation_rise
                                 : index == 1u ? net->propagation_fall
                                               : net->propagation_turn_off;
                return value > (uint64_t)INT64_MAX ? INT64_MAX : (int64_t)value;
            }
            break;
        }
        case DELAYS_PORT: {
            llg_sdf_load_t* load = port_load(object, delays->bit, 0);
            if (!load || index >= 3u) return 0;
            static const int transitions[3] = {DELAY_T01, DELAY_T10, DELAY_T0Z};
            uint64_t value = load->path->delays[transitions[index]];
            return value > (uint64_t)INT64_MAX ? INT64_MAX : (int64_t)value;
        }
        case DELAYS_CONTROL: {
            int64_t ticks = 0;
            return code_delay_ticks(delays->code, object, &ticks) ? ticks : 0;
        }
        default:
            break;
    }
    if (tm && index < tm->declared_count) return tm->declared[index];
    /* A runtime-valued delay without a runtime table reads its
     * expressions. */
    if (tm && !tm->declared_count && tm->count && index < 3u && evaluate_delays(object, evaluated))
        return evaluated[index];
    return 0;
}

/* The port row and bits of a port or port bit handle. */
static int port_bits(const llg_vpi_slot_t* slot, llg_vpi_delays_t* out) {
    out->row = row_object(slot->object);
    if (!out->row || out->row->type != vpiPort) return 0;
    out->bit = slot->kind == SLOT_PORTBIT ? (uint32_t)slot->aux : 0u;
    out->n_bits = slot->kind == SLOT_PORTBIT ? 1u : out->row->width;
    return 1;
}

/* Validate the delay record against the object (38.10, 38.32): the object
 * kind, time format, number of delays and flags. Returns the object's
 * delay kind with what it addresses in `out`, or DELAYS_NONE with the
 * error recorded. */
static int delay_record(vpiHandle handle, p_vpi_delay delay_p, const char* routine,
                        llg_vpi_delays_t* out) {
    memset(out, 0, sizeof(*out));
    llg_vpi_slot_t* slot = checked_handle(handle, NULL);
    if (!slot) return DELAYS_NONE;
    if (!delay_p || !delay_p->da) {
        vpi_set_errorf(vpi_state(), vpiError, "LLG_VPI_ARGUMENT",
                       "%s needs a delay record with an s_vpi_time array", routine);
        return DELAYS_NONE;
    }
    int read = strcmp(routine, "vpi_get_delays") == 0;
    int kind = DELAYS_NONE;
    switch (slot->kind) {
        case SLOT_OBJECT:
            out->row = row_object(slot->object);
            kind = delay_kind(out->row);
            if (kind == DELAYS_PORT && !port_bits(slot, out)) kind = DELAYS_NONE;
            /* 37.16 lists only vpi_get_delays for nets. */
            if (kind == DELAYS_NET && !read) kind = DELAYS_NONE;
            break;
        case SLOT_CABIT:
            /* A continuous assignment bit reads its assignment's delays
             * (37.43). */
            out->row = row_object(slot->object);
            if (read) kind = delay_kind(out->row);
            break;
        case SLOT_SELECT:
        case SLOT_ELEMENT:
            /* A net bit or a net array element reads its net's delays. */
            out->row = row_object(slot->object);
            if (read && out->row && out->row->is_net &&
                (slot->kind == SLOT_ELEMENT || slot->span == 1) &&
                handle_type(slot) != vpiPartSelect)
                kind = DELAYS_NET;
            break;
        case SLOT_PORTBIT:
            if (port_bits(slot, out)) kind = DELAYS_PORT;
            break;
        case SLOT_IMP:
            /* An intermodule path has its load port's delays (decision
             * A20-D18). */
            out->row = row_object(slot->object);
            out->bit = slot->aux < 0 ? 0u : (uint32_t)slot->aux;
            out->n_bits = slot->aux < 0 ? (out->row ? out->row->width : 0u) : 1u;
            out->imp = 1;
            kind = out->row ? DELAYS_PORT : DELAYS_NONE;
            break;
        case SLOT_CODE: {
            const llg_vpi_code_t* node = code_at((uint32_t)slot->aux);
            if (read && node && code_type(node) == vpiDelayControl) {
                out->code = code_child(node, vpiDelay);
                out->row = node->scope ? row_object(node->scope - 1u) : NULL;
                if (out->code != LLG_VPI_NONE) kind = DELAYS_CONTROL;
            }
            break;
        }
        default:
            break;
    }
    if (kind == DELAYS_NONE) {
        vpi_set_errorf(vpi_state(), vpiError, "LLG_VPI_UNSUPPORTED",
                       read ? "%s applies to module paths, timing checks, primitives, continuous "
                              "assignments, nets, ports, intermodule paths and delay controls"
                            : "%s applies to module paths, timing checks, primitives, ports and "
                              "intermodule paths",
                       routine);
        return DELAYS_NONE;
    }
    if (kind == DELAYS_PORT) {
        /* 37.14: module input port delays; detail 2: none for an interface
         * port. */
        const llg_vpi_model_object_t* port = out->row;
        if (port->subtype != vpiInput && port->subtype != vpiInout) {
            vpi_set_errorf(vpi_state(), vpiError, "LLG_VPI_DELAYS",
                           "%s: only input and inout ports have module input port delays",
                           routine);
            return DELAYS_NONE;
        }
        if (port->detail == vpiInterfacePort || port->detail == vpiModportPort ||
            out->n_bits == 0u || out->bit + out->n_bits > port->width) {
            vpi_set_errorf(vpi_state(), vpiError, "LLG_VPI_DELAYS",
                           "%s: an interface port has no delays (37.14 detail 2)", routine);
            return DELAYS_NONE;
        }
    }
    if (delay_p->time_type != vpiScaledRealTime && delay_p->time_type != vpiSimTime) {
        vpi_set_errorf(vpi_state(), vpiError, "LLG_VPI_TIME",
                       "%s needs time_type vpiScaledRealTime or vpiSimTime", routine);
        return DELAYS_NONE;
    }
    int n = delay_p->no_of_delays;
    int legal;
    const char* what;
    const char* counts;
    switch (kind) {
        case DELAYS_PATH:
            legal = n == 1 || n == 2 || n == 3 || n == 6 || n == 12;
            what = "module path";
            counts = "1, 2, 3, 6 or 12";
            break;
        case DELAYS_TCHK:
            legal = n == (int)out->row->timing->count;
            what = "timing check";
            counts = "the number of limits of the check";
            break;
        case DELAYS_CONTROL:
            legal = n == 1;
            what = "delay control";
            counts = "1";
            break;
        default:
            legal = n == 2 || n == 3;
            what = kind == DELAYS_PRIMITIVE      ? "primitive"
                   : kind == DELAYS_CONT_ASSIGN ? "continuous assignment"
                   : kind == DELAYS_NET          ? "net"
                   : out->imp                    ? "intermodule path"
                                                 : "port";
            counts = "2 or 3";
            break;
    }
    if (!legal) {
        vpi_set_errorf(vpi_state(), vpiError, "LLG_VPI_DELAYS",
                       "%s: %d delays do not suit this %s (%s)", routine, n, what, counts);
        return DELAYS_NONE;
    }
    if (delay_p->pulsere_flag && kind != DELAYS_PATH && kind != DELAYS_PRIMITIVE &&
        kind != DELAYS_CONT_ASSIGN) {
        vpi_set_errorf(vpi_state(), vpiError, "LLG_VPI_DELAYS",
                       "%s: a %s has no pulse limits", routine, what);
        return DELAYS_NONE;
    }
    out->kind = kind;
    return kind;
}

/* Position in delay_p->da of value `which` (0 delay, 1 reject, 2 error) of
 * delay `k`, min/typ/max operand `m` (Tables 38-2 and 38-4). */
static size_t delay_slot(const s_vpi_delay* delay_p, uint32_t k, uint32_t which, uint32_t m) {
    size_t operands = delay_p->mtm_flag ? 3u : 1u;
    size_t per = operands * (delay_p->pulsere_flag ? 3u : 1u);
    return (size_t)k * per + which * operands + (delay_p->mtm_flag ? m : 0u);
}

void vpi_get_delays(vpiHandle object, p_vpi_delay delay_p) {
    vpi_enter();
    if (!vpi_ready()) return;
    llg_vpi_delays_t delays;
    int kind = delay_record(object, delay_p, "vpi_get_delays", &delays);
    if (kind == DELAYS_NONE) return;
    llg_vpi_model_object_t* row = delays.row;
    uint32_t n = (uint32_t)delay_p->no_of_delays;
    for (uint32_t k = 0; k < n; ++k) {
        int64_t value[3];
        value[0] = current_delay(&delays, k);
        value[1] = value[0];
        value[2] = value[0];
        if (kind == DELAYS_PATH && row->timing->op_count && row->timing->ops[0].path) {
            uint64_t reject, error;
            llg_module_path_pulse_limits(row->timing->ops[0].path, k, &reject, &error);
            value[1] = (int64_t)reject;
            value[2] = (int64_t)error;
        }
        uint32_t classes = delay_p->pulsere_flag ? 3u : 1u;
        for (uint32_t which = 0; which < classes; ++which)
            for (uint32_t m = 0; m < (delay_p->mtm_flag ? 3u : 1u); ++m)
                delay_time(value[which], delay_p->time_type, row,
                           &delay_p->da[delay_slot(delay_p, k, which, m)]);
    }
}

/* `n` values in the order of llg_module_path_t.delays, expanded to the
 * twelve transitions as a declaration with as many delays (Tables 30-2 and
 * 30-3, `expand_path_delays` in the IR). */
static void expand_path_values(const int64_t* values, uint32_t n, int64_t* out) {
    enum { T01, T10, T0Z, TZ1, T1Z, TZ0, T0X, TX1, T1X, TX0, TXZ, TZX };
    if (n == 12) {
        memcpy(out, values, 12u * sizeof(*out));
        return;
    }
    int64_t r = values[0], f = n >= 2 ? values[1] : r;
    switch (n) {
        case 1:
            for (int t = 0; t < 6; ++t) out[t] = r;
            break;
        case 2:
        case 3: {
            int64_t z = n == 3 ? values[2] : 0;
            out[T01] = r;
            out[T10] = f;
            out[T0Z] = n == 3 ? z : r;
            out[TZ1] = r;
            out[T1Z] = n == 3 ? z : f;
            out[TZ0] = f;
            break;
        }
        default:
            memcpy(out, values, 6u * sizeof(*out));
            break;
    }
#define LLG_MIN(a, b) ((a) < (b) ? (a) : (b))
#define LLG_MAX(a, b) ((a) > (b) ? (a) : (b))
    out[T0X] = LLG_MIN(out[T0Z], out[T01]);
    out[T1X] = LLG_MIN(out[T1Z], out[T10]);
    out[TZX] = LLG_MIN(out[TZ1], out[TZ0]);
    out[TX0] = LLG_MAX(out[TZ0], out[T10]);
    out[TX1] = LLG_MAX(out[TZ1], out[T01]);
    out[TXZ] = LLG_MAX(out[T1Z], out[T0Z]);
#undef LLG_MIN
#undef LLG_MAX
}

/* Write the module input port delays (rise, fall, turn-off) of the bits of
 * `delays` through their interconnect loads, as an SDF PORT entry does
 * (32.4.4): every source entry of the load takes them, and the load's
 * interconnect path the result. */
static void put_port_delays(const llg_vpi_delays_t* delays, const int64_t triple[3], int append) {
    llg_sdf_op_t* ops = (llg_sdf_op_t*)calloc(delays->n_bits, sizeof(*ops));
    if (!ops) {
        vpi_error("LLG_VPI_NOMEM", "vpi_put_delays: out of memory");
        return;
    }
    /* Rise, fall and turn-off as a three-value path delay (Table 30-2). */
    int64_t values[12];
    expand_path_values(triple, 3u, values);
    for (uint32_t i = 0; i < delays->n_bits; ++i) {
        llg_sdf_load_t* load = port_load(delays->row, delays->bit + i, 1);
        if (!load) {
            free(ops);
            vpi_error("LLG_VPI_DELAYS",
                      "vpi_put_delays: llg delays only an input or inout port connected to a "
                      "net of its own that no module path drives");
            return;
        }
        ops[i].kind = LLG_SDF_LOAD;
        ops[i].flags = (uint8_t)((append ? LLG_SDF_INCREMENT : 0u) | 3u << LLG_SDF_MIPD_SHIFT);
        ops[i].delay = 0x0fffu;
        ops[i].load = load;
    }
    llg_sdf_annotation_t annotation = {ops, delays->n_bits, values};
    llg_sdf_apply(&annotation);
    free(ops);
}

void vpi_put_delays(vpiHandle object, p_vpi_delay delay_p) {
    vpi_enter();
    if (!vpi_ready()) return;
    llg_vpi_delays_t delays;
    int kind = delay_record(object, delay_p, "vpi_put_delays", &delays);
    if (kind == DELAYS_NONE) return;
    llg_vpi_model_object_t* row = delays.row;
    const llg_vpi_timing_t* tm = row->timing;
    if (kind == DELAYS_CONT_ASSIGN) {
        vpi_error("LLG_VPI_DELAYS",
                  "vpi_put_delays: the delays of a continuous assignment are read-only (37.43)");
        return;
    }
    if (!g_vpi_delays_writable) {
        vpi_error("LLG_VPI_DELAYS",
                  "vpi_put_delays: the model was generated without --vpi or --dpi-lib, so its "
                  "delays are not writable");
        return;
    }
    if (kind != DELAYS_PORT && !tm->op_count) {
        vpi_error("LLG_VPI_DELAYS",
                  kind == DELAYS_PRIMITIVE
                      ? "vpi_put_delays: this primitive has no delays to write"
                      : kind == DELAYS_TCHK
                      ? "vpi_put_delays: this timing check is not simulated (timing checks "
                        "are off, or a duplicate or a window that cannot report was dropped)"
                      : "vpi_put_delays: this module path is not simulated");
        return;
    }
    if (delay_p->pulsere_flag && kind == DELAYS_PRIMITIVE) {
        vpi_error("LLG_VPI_DELAYS",
                  "vpi_put_delays: primitive delays are inertial; llg keeps no pulse limits "
                  "for them");
        return;
    }
    uint32_t n = (uint32_t)delay_p->no_of_delays;
    uint32_t classes = delay_p->pulsere_flag ? 3u : 1u;
    uint32_t operand = delay_p->mtm_flag ? (uint32_t)g_vpi_delay_select : 0u;
    int append = delay_p->append_flag != 0;
    int64_t given[3][12] = {{0}};
    for (uint32_t which = 0; which < classes; ++which) {
        for (uint32_t k = 0; k < n; ++k) {
            const s_vpi_time* time = &delay_p->da[delay_slot(delay_p, k, which, operand)];
            if (!delay_ticks(time, delay_p->time_type, row, &given[which][k])) {
                vpi_error("LLG_VPI_DELAYS", "vpi_put_delays: a delay value is out of range");
                return;
            }
            if (!append && given[which][k] < 0 && kind != DELAYS_TCHK) {
                vpi_error("LLG_VPI_DELAYS",
                          "vpi_put_delays: module path, primitive and port delays are not "
                          "negative");
                return;
            }
        }
    }
    if (kind == DELAYS_TCHK) {
        /* Absolute limits, the stored ops take them by limit index. A
         * negative limit counts as 0 except where the check keeps its sign
         * (31.9.4 for `$setuphold`/`$recrem` with negative timing checks, the
         * `$nochange` offsets). */
        int64_t limits[2] = {0, 0};
        for (uint32_t k = 0; k < n && k < 2u; ++k) {
            int64_t value = given[0][k];
            if (append) {
                int64_t current = current_delay(&delays, k);
                value = value > 0 && current > INT64_MAX - value ? INT64_MAX
                        : value < 0 && current < INT64_MIN - value ? INT64_MIN
                                                                   : current + value;
            }
            const llg_sdf_op_t* op = limit_op(tm, k);
            int signed_limit = row->detail == vpiNoChange || (op && op->kind == LLG_SDF_PAIR);
            if (value < 0 && !signed_limit) value = 0;
            limits[k] = value;
        }
        llg_sdf_annotation_t annotation = {tm->ops, tm->op_count, limits};
        llg_sdf_apply(&annotation);
        return;
    }
    /* Two primitive or port delays: turn-off is the smaller (28.16). */
    int64_t triple[3] = {given[0][0], given[0][1],
                         n == 3 ? given[0][2]
                                : (given[0][0] < given[0][1] ? given[0][0] : given[0][1])};
    if (kind == DELAYS_PORT) {
        put_port_delays(&delays, triple, append);
        return;
    }
    uint64_t* gate = kind == DELAYS_PRIMITIVE ? runtime_gate(row) : NULL;
    if (gate && gate[3] == UINT64_MAX) {
        /* A runtime-valued gate that has not evaluated its delays: they
         * start from its expressions' current values, which its first
         * evaluation then keeps (llg_gate_delays_refresh). */
        int64_t evaluated[3];
        if (evaluate_delays(row, evaluated)) {
            for (uint32_t i = 0; i < 3u; ++i)
                gate[i] = gate[3u + i] = (uint64_t)evaluated[i];
        } else {
            gate[0] = gate[1] = gate[2] = 0;
        }
    }
    llg_sdf_op_t* ops = (llg_sdf_op_t*)calloc(tm->op_count, sizeof(*ops));
    if (!ops) {
        vpi_error("LLG_VPI_NOMEM", "vpi_put_delays: out of memory");
        return;
    }
    int64_t values[3 * 12] = {0};
    if (kind == DELAYS_PATH) {
        for (uint32_t which = 0; which < classes; ++which)
            expand_path_values(given[which], n, values + 12u * which);
    } else {
        memcpy(values, triple, sizeof(triple));
    }
    for (uint32_t i = 0; i < tm->op_count; ++i) {
        ops[i] = tm->ops[i];
        ops[i].first = 0;
        ops[i].derive = 0;
        ops[i].flags = (uint8_t)(append ? LLG_SDF_INCREMENT : 0u);
        if (kind == DELAYS_PATH) {
            ops[i].delay = 0x0fffu;
            ops[i].reject = delay_p->pulsere_flag ? 0x0fffu : 0u;
            ops[i].error = ops[i].reject;
            /* 38.32: new delays alone leave the pulse limits as they were. */
            if (!delay_p->pulsere_flag) ops[i].flags |= LLG_SDF_KEEP_LIMITS;
        } else {
            ops[i].delay = 0x7u;
            ops[i].reject = 0;
            ops[i].error = 0;
        }
    }
    llg_sdf_annotation_t annotation = {ops, tm->op_count, values};
    llg_sdf_apply(&annotation);
    free(ops);
}

/* ── intermodule paths (37.33, 38.22) ─────────────────────────────────── */

/* The port row and bit (-1: the whole port) of a port or port bit handle,
 * or 0. */
static int port_reference(vpiHandle handle, uint32_t* row, int32_t* bit) {
    llg_vpi_slot_t* slot = checked_handle(handle, NULL);
    if (!slot) return 0;
    const llg_vpi_model_object_t* object = row_object(slot->object);
    if (!object || object->type != vpiPort ||
        (slot->kind != SLOT_OBJECT && slot->kind != SLOT_PORTBIT))
        return 0;
    *row = slot->object;
    *bit = slot->kind == SLOT_PORTBIT ? slot->aux : -1;
    return 1;
}

/* 38.22: the intermodule path from output (or inout) port `ref1` to input
 * (or inout) port `ref2`, both ports or port bits of the same size; its
 * delays are those of its load port (decision A20-D18). */
static vpiHandle intermodule_path(PLI_INT32 type, vpiHandle ref1, vpiHandle ref2) {
    if (type != vpiInterModPath) {
        vpi_error("LLG_VPI_RELATION", "vpi_handle_multi gives only vpiInterModPath objects");
        return NULL;
    }
    uint32_t source, load;
    int32_t source_bit, load_bit;
    if (!port_reference(ref1, &source, &source_bit) || !port_reference(ref2, &load, &load_bit)) {
        if (!vpi_chk_error(NULL))
            vpi_error("LLG_VPI_RELATION",
                      "vpi_handle_multi(vpiInterModPath) takes an output port and an input "
                      "port or port bit");
        return NULL;
    }
    const llg_vpi_model_object_t* from = row_object(source);
    const llg_vpi_model_object_t* to = row_object(load);
    uint32_t from_size = source_bit < 0 ? from->width : 1u;
    uint32_t to_size = load_bit < 0 ? to->width : 1u;
    if ((from->subtype != vpiOutput && from->subtype != vpiInout) ||
        (to->subtype != vpiInput && to->subtype != vpiInout) ||
        (source == load && source_bit == load_bit)) {
        vpi_error("LLG_VPI_RELATION",
                  "vpi_handle_multi(vpiInterModPath) takes an output port and an input port, "
                  "in that order");
        return NULL;
    }
    if (from_size != to_size) {
        vpi_error("LLG_VPI_RELATION",
                  "the ports of an intermodule path have the same size (38.22)");
        return NULL;
    }
    uint32_t index = alloc_slot(SLOT_IMP);
    if (index == LLG_VPI_NONE) return NULL;
    g_vpi.slots[index].object = load;
    g_vpi.slots[index].aux = load_bit;
    g_vpi.slots[index].left = source;
    g_vpi.slots[index].right = source_bit;
    return encode_handle(index);
}

/* 37.33: the ports of an intermodule path, its source first. */
static vpiHandle iterate_imp_ports(const llg_vpi_slot_t* slot) {
    uintptr_t* items = (uintptr_t*)calloc(2u, sizeof(*items));
    if (!items) {
        vpi_error("LLG_VPI_NOMEM", "VPI iterator allocation failed");
        return NULL;
    }
    /* make_aux_slot may move the slot table: copy the fields first. */
    uint32_t source = (uint32_t)slot->left, load = slot->object;
    int32_t source_bit = (int32_t)slot->right, load_bit = slot->aux;
    uint32_t made = 0;
    vpiHandle first = source_bit < 0 ? object_handle(source)
                                     : make_aux_slot(SLOT_PORTBIT, source, (uint32_t)source_bit);
    if (first) items[made++] = (uintptr_t)first;
    vpiHandle second = load_bit < 0 ? object_handle(load)
                                    : make_aux_slot(SLOT_PORTBIT, load, (uint32_t)load_bit);
    if (second) items[made++] = (uintptr_t)second;
    return make_list_iterator(vpiPort, items, made);
}

/* ── cbTchkViolation (38.36.1) ─────────────────────────────────────────── */

/* Timing check rows by the runtime check they own, built at the first
 * violation a record may report (pair rows exist only once the model
 * started). */
typedef struct {
    const llg_timing_check_t* check;
    uint32_t row;
} llg_vpi_tchk_row_t;

static llg_vpi_tchk_row_t* g_vpi_tchk_rows;
static size_t g_vpi_tchk_row_count;
static int g_vpi_tchk_rows_built;

static int compare_tchk_rows(const void* left, const void* right) {
    uintptr_t a = (uintptr_t)((const llg_vpi_tchk_row_t*)left)->check;
    uintptr_t b = (uintptr_t)((const llg_vpi_tchk_row_t*)right)->check;
    return a < b ? -1 : a > b;
}

static void tchk_rows_build(void) {
    g_vpi_tchk_rows_built = 1;
    size_t capacity = 0;
    for (size_t row = 0; row < g_vpi.object_count; ++row) {
        const llg_vpi_model_object_t* object = &g_vpi.objects[row];
        if (object->type == vpiTchk && object->timing) capacity += 2u * object->timing->op_count;
    }
    if (!capacity) return;
    g_vpi_tchk_rows = (llg_vpi_tchk_row_t*)calloc(capacity, sizeof(*g_vpi_tchk_rows));
    if (!g_vpi_tchk_rows) return;
    size_t count = 0;
    for (size_t row = 0; row < g_vpi.object_count; ++row) {
        const llg_vpi_model_object_t* object = &g_vpi.objects[row];
        if (object->type != vpiTchk || !object->timing) continue;
        for (uint32_t i = 0; i < object->timing->op_count; ++i) {
            const llg_sdf_op_t* op = &object->timing->ops[i];
            if (op->kind == LLG_SDF_CHECK && op->check) {
                g_vpi_tchk_rows[count++] = (llg_vpi_tchk_row_t){op->check, (uint32_t)row};
            } else if (op->kind == LLG_SDF_PAIR && op->checks && op->checks->pairs &&
                       op->index < op->checks->n_pairs) {
                const llg_timing_pair_t* pair = &op->checks->pairs[op->index];
                int32_t rows[2] = {pair->before_row, pair->after_row};
                for (int part = 0; part < 2; ++part)
                    if (rows[part] >= 0 && (uint32_t)rows[part] < op->checks->n_checks)
                        g_vpi_tchk_rows[count++] =
                            (llg_vpi_tchk_row_t){&op->checks->checks[rows[part]], (uint32_t)row};
            }
        }
    }
    qsort(g_vpi_tchk_rows, count, sizeof(*g_vpi_tchk_rows), compare_tchk_rows);
    g_vpi_tchk_row_count = count;
}

static uint32_t tchk_row_of(const llg_timing_check_t* check) {
    if (!g_vpi_tchk_rows_built) tchk_rows_build();
    size_t low = 0, high = g_vpi_tchk_row_count;
    while (low < high) {
        size_t middle = low + (high - low) / 2u;
        if ((uintptr_t)g_vpi_tchk_rows[middle].check < (uintptr_t)check) low = middle + 1u;
        else high = middle;
    }
    return low < g_vpi_tchk_row_count && g_vpi_tchk_rows[low].check == check
               ? g_vpi_tchk_rows[low].row : LLG_VPI_NONE;
}

static void tchk_rows_release(void) {
    free(g_vpi_tchk_rows);
    g_vpi_tchk_rows = NULL;
    g_vpi_tchk_row_count = 0;
    g_vpi_tchk_rows_built = 0;
}

/* ── value formats (Table 38-3) ─────────────────────────────────────────── */

static char* text_buffer(size_t need) {
    if (need > g_vpi.value_text_capacity) {
        char* grown = (char*)realloc(g_vpi.value_text, need);
        if (!grown) return NULL;
        g_vpi.value_text = grown;
        g_vpi.value_text_capacity = need;
    }
    return g_vpi.value_text;
}

static unsigned word_state(const s_vpi_vecval* words, uint32_t bit) {
    unsigned a = (words[bit / 32u].aval >> (bit % 32u)) & 1u;
    unsigned b = (words[bit / 32u].bval >> (bit % 32u)) & 1u;
    return b ? (a ? 2u : 3u) : a; /* 0, 1, 2 = x, 3 = z */
}

/* Binary, octal or hex digits, most significant first. A digit is x/z when
 * all of its bits are x/z, and X/Z when only some are (Table 38-3). */
static char* format_radix(const s_vpi_vecval* words, uint32_t width, unsigned bits) {
    size_t digits = ((size_t)width + bits - 1u) / bits;
    char* text = text_buffer(digits + 1u);
    if (!text) return NULL;
    for (size_t d = 0; d < digits; ++d) {
        uint32_t low = (uint32_t)(d * bits);
        unsigned value = 0, xs = 0, zs = 0, count = 0;
        for (unsigned bit = 0; bit < bits && low + bit < width; ++bit, ++count) {
            unsigned state = word_state(words, low + bit);
            if (state == 1) value |= 1u << bit;
            if (state == 2) ++xs;
            if (state == 3) ++zs;
        }
        char ch;
        if (xs == count) ch = 'x';
        else if (zs == count) ch = 'z';
        else if (xs) ch = bits == 1 ? 'x' : 'X';
        else if (zs) ch = bits == 1 ? 'z' : 'Z';
        else ch = "0123456789abcdef"[value];
        text[digits - 1u - d] = ch;
    }
    text[digits] = '\0';
    return text;
}

/* Field width of a vpiDecStrVal string (decision S42-D6): the number of
 * decimal digits of the largest unsigned value of `width` bits, as %d sizes
 * its field (21.2.1.3), except that a signed object of 2 to 32 bits takes
 * the 11 characters of a 32-bit integer and a negative constant adds one
 * character for its sign. floor(width * log10(2)) + 1 counts the digits of
 * 2^width - 1 (2^width is never a power of ten); in double precision the
 * product is exact enough for every supported width, whose distance from an
 * integer stays far above the rounding error. */
static size_t decimal_field_width(uint32_t width, int is_signed, int constant, int negative) {
    size_t digits = (size_t)((double)width * 0.30102999566398119521) + 1u;
    if (constant) return digits + (negative ? 1u : 0u);
    if (is_signed && width >= 2u && width <= 32u) return 11u;
    return digits;
}

/* Decimal digits of a value, right-justified in its field width; signed
 * values print a '-' sign. A value with any x or z bit reads as 0 (decision
 * S42-D6; Table 38-3 lists no x/z digits for vpiDecStrVal). */
static char* format_decimal(const s_vpi_vecval* source, uint32_t width, int is_signed,
                            int constant) {
    size_t count = word_count(width);
    int unknown = 0;
    for (size_t i = 0; i < count && !unknown; ++i) unknown = source[i].bval != 0;
    uint32_t* magnitude = (uint32_t*)calloc(count, sizeof(uint32_t));
    if (!magnitude) return NULL;
    if (!unknown)
        for (size_t i = 0; i < count; ++i) magnitude[i] = source[i].aval;
    uint32_t top_bits = width % 32u;
    if (top_bits) magnitude[count - 1u] &= (1u << top_bits) - 1u;
    int negative = !unknown && is_signed && width && word_state(source, width - 1u) == 1u;
    if (negative) {
        if (top_bits) magnitude[count - 1u] |= ~((1u << top_bits) - 1u);
        uint64_t carry = 1;
        for (size_t i = 0; i < count; ++i) {
            uint64_t sum = (uint64_t)(uint32_t)~magnitude[i] + carry;
            magnitude[i] = (uint32_t)sum;
            carry = sum >> 32;
        }
        if (top_bits) magnitude[count - 1u] &= (1u << top_bits) - 1u;
    }
    /* Each 32-bit word needs at most 10 decimal digits. */
    size_t capacity = count * 10u + 3u;
    char* digits = (char*)malloc(capacity);
    if (!digits) {
        free(magnitude);
        return NULL;
    }
    size_t length = 0;
    size_t active = count;
    do {
        uint64_t remainder = 0;
        for (size_t i = active; i-- > 0;) {
            uint64_t current = (remainder << 32) | magnitude[i];
            magnitude[i] = (uint32_t)(current / 1000000000u);
            remainder = current % 1000000000u;
        }
        while (active && magnitude[active - 1u] == 0) --active;
        for (unsigned k = 0; k < 9u; ++k) {
            digits[length++] = (char)('0' + remainder % 10u);
            remainder /= 10u;
            if (!active && !remainder) break;
        }
    } while (active);
    while (length > 1 && digits[length - 1u] == '0') --length;
    size_t used = length + (negative ? 1u : 0u);
    size_t field = decimal_field_width(width, is_signed, constant, negative);
    size_t pad = field > used ? field - used : 0u;
    char* text = text_buffer(pad + used + 1u);
    if (text) {
        size_t at = 0;
        while (at < pad) text[at++] = ' ';
        if (negative) text[at++] = '-';
        for (size_t i = length; i-- > 0;) text[at++] = digits[i];
        text[at] = '\0';
    }
    free(digits);
    free(magnitude);
    return text;
}

/* Each 8-bit group from the most significant end is one character; zero
 * groups (and X/Z bits, read as 0) produce no character. */
static char* format_string(const s_vpi_vecval* words, uint32_t width) {
    size_t groups = ((size_t)width + 7u) / 8u;
    char* text = text_buffer(groups + 1u);
    if (!text) return NULL;
    size_t at = 0;
    for (size_t group = groups; group-- > 0;) {
        unsigned ch = 0;
        for (unsigned bit = 0; bit < 8u; ++bit) {
            uint32_t position = (uint32_t)(group * 8u + bit);
            if (position < width && word_state(words, position) == 1u) ch |= 1u << bit;
        }
        if (ch) text[at++] = (char)ch;
    }
    text[at] = '\0';
    return text;
}

static int32_t low_int(const s_vpi_vecval* words, uint32_t width, int is_signed) {
    uint32_t value = words[0].aval & ~words[0].bval;
    if (width < 32u) {
        uint32_t mask = (1u << width) - 1u;
        value &= mask;
        if (is_signed && width && ((value >> (width - 1u)) & 1u)) value |= ~mask;
    }
    return (int32_t)value;
}

static uint64_t low_u64(const s_vpi_vecval* words, uint32_t width) {
    uint64_t value = words[0].aval & ~words[0].bval;
    if (width > 32u) value |= (uint64_t)(words[1].aval & ~words[1].bval) << 32;
    if (width < 64u) value &= (UINT64_C(1) << width) - 1u;
    return value;
}

/* The format vpiObjTypeVal resolves to for a handle (38.15). */
static int natural_format(int type, uint32_t width, int is_real) {
    if (is_real) return vpiRealVal;
    switch (type) {
        case vpiIntegerVar: case vpiIntVar: case vpiShortIntVar: case vpiByteVar:
        case vpiConstant:
            return vpiIntVal;
        case vpiTimeVar:
            return vpiTimeVal;
        default:
            return width == 1 ? vpiScalarVal : vpiVectorVal;
    }
}

static void fill_value(llg_vpi_datum_t* datum, int is_signed, int type, p_vpi_value output) {
    int format = output->format;
    if (format == vpiObjTypeVal) {
        format = natural_format(type, datum->is_real ? 0 : llg_sv4_width(datum->packed),
                                datum->is_real);
        output->format = format;
    }
    if (format == vpiSuppressVal) return;
    if (datum->is_real) {
        switch (format) {
            case vpiRealVal:
                output->value.real = datum->real;
                return;
            case vpiIntVal: {
                double rounded = datum->real != datum->real ? 0.0 : round(datum->real);
                if (rounded > 2147483647.0) rounded = 2147483647.0;
                if (rounded < -2147483648.0) rounded = -2147483648.0;
                output->value.integer = (PLI_INT32)rounded;
                return;
            }
            case vpiBinStrVal:
            case vpiOctStrVal:
            case vpiDecStrVal:
            case vpiHexStrVal:
            case vpiStringVal:
            case vpiVectorVal: {
                /* Decision S42-D9: the string and vector formats read the
                 * real truncated toward zero as a signed 64-bit value. */
                double truncated = datum->real != datum->real ? 0.0 : trunc(datum->real);
                llg_vpi_datum_t integral;
                memset(&integral, 0, sizeof(integral));
                integral.packed = sv4_from_real(truncated, 64, 1);
                fill_value(&integral, 1, vpiLongIntVar, output);
                datum_destroy(&integral);
                return;
            }
            default:
                vpi_error("LLG_VPI_FORMAT",
                          "a real value has no scalar, time or strength representation");
                return;
        }
    }
    uint32_t width = llg_sv4_width(datum->packed);
    if (width == 0) {
        vpi_error("LLG_VPI_VALUE", "VPI value has no bits");
        return;
    }
    size_t count = word_count(width);
    s_vpi_vecval* words = scratch_words(count);
    if (!words) {
        vpi_error("LLG_VPI_NOMEM", "VPI value conversion allocation failed");
        return;
    }
    export_words(datum->packed, words);
    switch (format) {
        case vpiBinStrVal:
        case vpiOctStrVal:
        case vpiHexStrVal: {
            unsigned bits = format == vpiBinStrVal ? 1u : format == vpiOctStrVal ? 3u : 4u;
            char* text = format_radix(words, width, bits);
            if (!text) vpi_error("LLG_VPI_NOMEM", "VPI text allocation failed");
            else output->value.str = (PLI_BYTE8*)text;
            return;
        }
        case vpiDecStrVal: {
            char* text = format_decimal(words, width, is_signed, type == vpiConstant);
            if (!text) vpi_error("LLG_VPI_NOMEM", "VPI text allocation failed");
            else output->value.str = (PLI_BYTE8*)text;
            return;
        }
        case vpiStringVal: {
            char* text = format_string(words, width);
            if (!text) vpi_error("LLG_VPI_NOMEM", "VPI text allocation failed");
            else output->value.str = (PLI_BYTE8*)text;
            return;
        }
        case vpiScalarVal: {
            /* A vector reads as its least significant bit (decision S42-D13). */
            static const PLI_INT32 scalar[4] = {vpi0, vpi1, vpiX, vpiZ};
            output->value.scalar = scalar[word_state(words, 0)];
            return;
        }
        case vpiIntVal:
            output->value.integer = low_int(words, width, is_signed);
            return;
        case vpiRealVal: {
            sv4_t known = sv4_to_two_state(datum->packed);
            output->value.real = sv4_to_real(known);
            sv4_destroy(&known);
            return;
        }
        case vpiTimeVal: {
            uint64_t value = low_u64(words, width);
            g_vpi.value_time.type = vpiSimTime;
            g_vpi.value_time.high = (PLI_UINT32)(value >> 32);
            g_vpi.value_time.low = (PLI_UINT32)value;
            g_vpi.value_time.real = 0.0;
            output->value.time = &g_vpi.value_time;
            return;
        }
        case vpiVectorVal: {
            if (count > g_vpi.value_vector_capacity) {
                s_vpi_vecval* grown =
                    (s_vpi_vecval*)realloc(g_vpi.value_vector, count * sizeof(*grown));
                if (!grown) {
                    vpi_error("LLG_VPI_NOMEM", "VPI vector allocation failed");
                    return;
                }
                g_vpi.value_vector = grown;
                g_vpi.value_vector_capacity = count;
            }
            memcpy(g_vpi.value_vector, words, count * sizeof(*words));
            output->value.vector = g_vpi.value_vector;
            return;
        }
        case vpiStrengthVal: {
            if (width > g_vpi.value_strength_capacity) {
                s_vpi_strengthval* grown = (s_vpi_strengthval*)realloc(
                    g_vpi.value_strength, (size_t)width * sizeof(*grown));
                if (!grown) {
                    vpi_error("LLG_VPI_NOMEM", "VPI strength allocation failed");
                    return;
                }
                g_vpi.value_strength = grown;
                g_vpi.value_strength_capacity = width;
            }
            static const PLI_INT32 logic[4] = {vpi0, vpi1, vpiX, vpiZ};
            for (uint32_t bit = 0; bit < width; ++bit) {
                unsigned state = word_state(words, bit);
                g_vpi.value_strength[bit].logic = logic[state];
                g_vpi.value_strength[bit].s0 = state == 3 ? vpiHiZ : vpiStrongDrive;
                g_vpi.value_strength[bit].s1 = state == 3 ? vpiHiZ : vpiStrongDrive;
            }
            output->value.strength = g_vpi.value_strength;
            return;
        }
        default:
            vpi_error("LLG_VPI_FORMAT", "unsupported VPI value format");
            return;
    }
}

/* Parse radix digits (with x/z and `_`) into `width` bits; extra high
 * digits are discarded and missing ones are zero. */
static int parse_radix(const char* text, unsigned bits, s_vpi_vecval* words, uint32_t width) {
    size_t length = strlen(text);
    uint32_t position = 0;
    int digits = 0;
    for (size_t i = length; i-- > 0;) {
        char ch = text[i];
        if (ch == '_') continue;
        unsigned value = 0, state = 0;
        if (ch >= '0' && ch <= '9') value = (unsigned)(ch - '0');
        else if (ch >= 'a' && ch <= 'f') value = (unsigned)(ch - 'a' + 10);
        else if (ch >= 'A' && ch <= 'F') value = (unsigned)(ch - 'A' + 10);
        else if (ch == 'x' || ch == 'X') state = 2;
        else if (ch == 'z' || ch == 'Z' || ch == '?') state = 3;
        else return 0;
        if (!state && value >= (1u << bits)) return 0;
        for (unsigned bit = 0; bit < bits; ++bit, ++position) {
            if (position >= width) continue;
            uint32_t mask = 1u << (position % 32u);
            unsigned b = state ? state : ((value >> bit) & 1u);
            if (b == 1 || b == 2) words[position / 32u].aval |= mask;
            if (b == 2 || b == 3) words[position / 32u].bval |= mask;
        }
        ++digits;
    }
    return digits > 0;
}

/* Parse an optionally signed decimal string, or a lone x/z. */
static int parse_decimal(const char* text, s_vpi_vecval* words, uint32_t width) {
    size_t count = word_count(width);
    if ((text[0] == 'x' || text[0] == 'X' || text[0] == 'z' || text[0] == 'Z') && !text[1]) {
        int z = text[0] == 'z' || text[0] == 'Z';
        for (size_t i = 0; i < count; ++i) {
            words[i].aval = z ? 0u : UINT32_MAX;
            words[i].bval = UINT32_MAX;
        }
        return 1;
    }
    int negative = 0;
    if (*text == '-' || *text == '+') negative = *text++ == '-';
    int digits = 0;
    for (; *text; ++text) {
        if (*text == '_') continue;
        if (*text < '0' || *text > '9') return 0;
        uint64_t carry = (uint64_t)(*text - '0');
        for (size_t i = 0; i < count; ++i) {
            uint64_t product = (uint64_t)words[i].aval * 10u + carry;
            words[i].aval = (uint32_t)product;
            carry = product >> 32;
        }
        ++digits;
    }
    if (!digits) return 0;
    if (negative) {
        uint64_t carry = 1;
        for (size_t i = 0; i < count; ++i) {
            uint64_t sum = (uint64_t)(uint32_t)~words[i].aval + carry;
            words[i].aval = (uint32_t)sum;
            carry = sum >> 32;
        }
    }
    return 1;
}

/* Convert an application value to a datum for a destination of `width`
 * bits (`real` for a real destination). */
static int value_from_vpi(const s_vpi_value* input, uint32_t width, int is_signed,
                          int real, llg_vpi_datum_t* out) {
    memset(out, 0, sizeof(*out));
    out->packed = VPI_EMPTY;
    if (!input) {
        vpi_error("LLG_VPI_ARGUMENT", "null s_vpi_value");
        return 0;
    }
    if (input->format == vpiRealVal) {
        out->is_real = 1;
        out->real = input->value.real;
        return 1;
    }
    if (real) {
        if (input->format == vpiStringVal) {
            vpi_error("LLG_VPI_FORMAT", "vpiStringVal cannot be put to a real object (38.34)");
            return 0;
        }
        if (input->format == vpiIntVal) {
            out->is_real = 1;
            out->real = (double)input->value.integer;
            return 1;
        }
        width = 64;
        is_signed = 1;
    }
    if (width == 0 || width >= LLG_SUPPORTED_WIDTH_LIMIT) {
        vpi_error("LLG_VPI_VALUE", "invalid destination width");
        return 0;
    }
    size_t count = word_count(width);
    s_vpi_vecval* words = scratch_words(count);
    if (!words) {
        vpi_error("LLG_VPI_NOMEM", "VPI value conversion allocation failed");
        return 0;
    }
    switch (input->format) {
        case vpiBinStrVal:
        case vpiOctStrVal:
        case vpiHexStrVal: {
            unsigned bits = input->format == vpiBinStrVal ? 1u : input->format == vpiOctStrVal ? 3u : 4u;
            if (!input->value.str || !parse_radix(input->value.str, bits, words, width)) {
                vpi_error("LLG_VPI_VALUE", "malformed binary/octal/hex VPI string value");
                return 0;
            }
            break;
        }
        case vpiDecStrVal:
            if (!input->value.str || !parse_decimal(input->value.str, words, width)) {
                vpi_error("LLG_VPI_VALUE", "malformed decimal VPI string value");
                return 0;
            }
            break;
        case vpiScalarVal: {
            /* A vector takes the scalar zero-extended, as an assignment of a
             * one-bit unsigned value (decision S42-D13). A real has no
             * scalar representation (S42-D9). */
            if (real) {
                vpi_error("LLG_VPI_FORMAT", "vpiScalarVal cannot be put to a real object");
                return 0;
            }
            switch (input->value.scalar) {
                case vpi0: case vpiL: break;
                case vpi1: case vpiH: words[0].aval = 1; break;
                case vpiX: case vpiDontCare: words[0].aval = 1; words[0].bval = 1; break;
                case vpiZ: words[0].bval = 1; break;
                default:
                    vpi_error("LLG_VPI_VALUE", "invalid vpiScalarVal code");
                    return 0;
            }
            break;
        }
        case vpiIntVal: {
            int32_t value = input->value.integer;
            words[0].aval = (uint32_t)value;
            for (size_t i = 1; i < count; ++i) words[i].aval = value < 0 ? UINT32_MAX : 0u;
            break;
        }
        case vpiStringVal: {
            const char* text = input->value.str;
            if (!text) {
                vpi_error("LLG_VPI_VALUE", "null vpiStringVal text");
                return 0;
            }
            size_t length = strlen(text);
            for (size_t i = 0; i < length; ++i) {
                unsigned char ch = (unsigned char)text[length - 1u - i];
                for (unsigned bit = 0; bit < 8u; ++bit) {
                    size_t position = i * 8u + bit;
                    if (position < width && ((ch >> bit) & 1u))
                        words[position / 32u].aval |= 1u << (position % 32u);
                }
            }
            break;
        }
        case vpiVectorVal:
            if (!input->value.vector) {
                vpi_error("LLG_VPI_VALUE", "null vpiVectorVal array");
                return 0;
            }
            memcpy(words, input->value.vector, count * sizeof(*words));
            break;
        case vpiTimeVal:
            if (!input->value.time) {
                vpi_error("LLG_VPI_VALUE", "null vpiTimeVal structure");
                return 0;
            }
            words[0].aval = input->value.time->low;
            if (count > 1) words[1].aval = input->value.time->high;
            break;
        case vpiStrengthVal:
            /* 38.34: vpiStrengthVal is illegal for a vector object. */
            if (width != 1) {
                vpi_error("LLG_VPI_FORMAT", "vpiStrengthVal put to a vector object (38.34)");
                return 0;
            }
            if (!input->value.strength) {
                vpi_error("LLG_VPI_VALUE", "null vpiStrengthVal array");
                return 0;
            }
            for (uint32_t bit = 0; bit < width; ++bit) {
                uint32_t mask = 1u << (bit % 32u);
                switch (input->value.strength[bit].logic) {
                    case vpi0: break;
                    case vpi1: words[bit / 32u].aval |= mask; break;
                    case vpiZ: words[bit / 32u].bval |= mask; break;
                    default: words[bit / 32u].aval |= mask; words[bit / 32u].bval |= mask; break;
                }
            }
            break;
        default:
            vpi_error("LLG_VPI_FORMAT", "unsupported VPI value format for vpi_put_value");
            return 0;
    }
    uint32_t top = width % 32u;
    if (top) {
        words[count - 1u].aval &= (1u << top) - 1u;
        words[count - 1u].bval &= (1u << top) - 1u;
    }
    out->packed = sv4_zero(width, (int8_t)is_signed);
    llg_sv4_import_vpi32(&out->packed, words, count, sizeof(*words));
    if (real) {
        sv4_t known = sv4_to_two_state(out->packed);
        out->real = sv4_to_real(known);
        out->is_real = 1;
        sv4_destroy(&known);
        sv4_destroy(&out->packed);
    }
    return 1;
}

/* ── model lifecycle ────────────────────────────────────────────────────── */

int llg_vpi_model_init(const char* design_name,
                       llg_vpi_model_object_t* objects, size_t object_count) {
    if (g_vpi.initialized || (!objects && object_count) || (objects && !object_count) ||
        object_count > LLG_VPI_MAX_OBJECTS) {
        vpi_set_error(vpiCompile, vpiError, "LLG_VPI_MODEL", "invalid or duplicate VPI model metadata");
        return 0;
    }
    memset(&g_vpi, 0, sizeof(g_vpi));
    g_vpi_generation = next_generation(g_vpi_generation);
    g_vpi.model_generation = g_vpi_generation;
    g_vpi.design_name = design_name ? design_name : "llg";
    g_vpi.objects = objects;
    g_vpi.object_count = object_count;
    g_vpi.initialized = 1;
    snprintf(g_vpi.error_product, sizeof(g_vpi.error_product), "lapligence");
    return 1;
}

int llg_vpi_model_types(const llg_vpi_type_tables_t* tables) {
    if (!g_vpi.initialized || g_vpi.indexed) {
        vpi_set_error(vpiCompile, vpiError, "LLG_VPI_MODEL",
                      "VPI typespec tables registered outside model initialization");
        return 0;
    }
    g_vpi.types = tables;
    return 1;
}

static int run_startup_table(void (*const* startup)(void), const char* origin) {
    rts_enable();
    for (int i = 0; i < LLG_VPI_MAX_STARTUP_ROUTINES; ++i) {
        if (!startup[i]) return 1;
        startup[i]();
    }
    vpi_set_errorf(vpiPLI, vpiError, "LLG_VPI_PLUGIN", "VPI startup table of `%s` exceeds %d entries",
                   origin, LLG_VPI_MAX_STARTUP_ROUTINES);
    return 0;
}

static int load_one_plugin(const char* path) {
    if (!path || !*path || g_vpi.plugin_count >= LLG_VPI_MAX_PLUGINS) {
        vpi_set_error(vpiPLI, vpiError, "LLG_VPI_PLUGIN", "empty or excessive VPI plugin path");
        return 0;
    }
    llg_dl_t module = llg_dl_open(path);
    if (!module) {
        const char* loader_error = llg_dl_error();
        if (loader_error)
            vpi_set_errorf(vpiPLI, vpiError, "LLG_VPI_PLUGIN", "cannot load VPI plugin `%s`: %s",
                           path, loader_error);
        else
            vpi_set_errorf(vpiPLI, vpiError, "LLG_VPI_PLUGIN", "cannot load VPI plugin `%s`", path);
        return 0;
    }
    void (**startup)(void) = (void (**)(void))llg_dl_symbol(module, "vlog_startup_routines");
    if (!startup) {
        vpi_set_errorf(vpiPLI, vpiError, "LLG_VPI_PLUGIN", "VPI plugin `%s` exports no startup table", path);
        llg_dl_close(module);
        return 0;
    }
    g_vpi.plugin_handles[g_vpi.plugin_count++] = module;
    if (!g_vpi.object_count && g_vpi.plugin_count == 1)
        fprintf(stderr, "llg: warning: the model was built without the VPI object catalog; "
                        "rebuild it with --vpi to reach design objects from `%s`\n", path);
    if (!run_startup_table((void (*const*)(void))startup, path)) return 0;
    return !g_vpi.failed;
}

int llg_vpi_startup_static(void (*const* routines)(void)) {
    if (!g_vpi.initialized) {
        vpi_set_error(vpiCompile, vpiError, "LLG_VPI_MODEL", "VPI startup ran without model metadata");
        return 0;
    }
    if (!routines) return 1;
    return run_startup_table(routines, "linked application") && !g_vpi.failed;
}

int llg_vpi_startup(void) {
    if (!g_vpi.initialized) {
        vpi_set_error(vpiCompile, vpiError, "LLG_VPI_MODEL", "VPI startup ran without model metadata");
        return 0;
    }
    if (g_vpi.startup_called) return !g_vpi.failed;
    g_vpi.startup_called = 1;
    const char* paths = getenv("LLG_VPI_PLUGIN");
    if (paths && *paths) {
        char buffer[4096];
        size_t length = strlen(paths);
        if (length >= sizeof(buffer)) {
            vpi_set_error(vpiPLI, vpiError, "LLG_VPI_PLUGIN", "LLG_VPI_PLUGIN exceeds 4095 bytes");
            return 0;
        }
        memcpy(buffer, paths, length + 1);
        char* cursor = buffer;
        while (cursor && *cursor) {
            char* next = strchr(cursor, LLG_PATH_LIST_SEPARATOR);
            if (next) *next++ = '\0';
            if (!load_one_plugin(cursor)) {
                fprintf(stderr, "llg: VPI startup failed: %s\n", g_vpi.error_message);
                return 0;
            }
            cursor = next;
        }
    }
    return !g_vpi.failed;
}

/* ── registration ───────────────────────────────────────────────────────── */

static int legal_systf_name(const char* name) {
    if (!name || name[0] != '$' || !name[1]) return 0;
    for (const char* p = name + 1; *p; ++p) {
        char ch = *p;
        if (!((ch >= 'a' && ch <= 'z') || (ch >= 'A' && ch <= 'Z') ||
              (ch >= '0' && ch <= '9') || ch == '_' || ch == '$'))
            return 0;
    }
    return 1;
}

static llg_vpi_registration_t* find_registration(const char* name) {
    if (!name) return NULL;
    for (int i = 0; i < g_vpi.registration_count; ++i)
        if (strcmp(g_vpi.registrations[i]->name, name) == 0) return g_vpi.registrations[i];
    return NULL;
}

vpiHandle vpi_register_systf(p_vpi_systf_data data) {
    vpi_enter();
    if (!data || !legal_systf_name((const char*)data->tfname) ||
        (data->type != vpiSysTask && data->type != vpiSysFunc)) {
        vpi_set_error(vpiCompile, vpiError, "LLG_VPI_REGISTRATION",
                      "invalid VPI system task/function registration (38.37)");
        return NULL;
    }
    if (data->type == vpiSysFunc && data->sysfunctype != vpiIntFunc &&
        data->sysfunctype != vpiRealFunc && data->sysfunctype != vpiTimeFunc &&
        data->sysfunctype != vpiSizedFunc && data->sysfunctype != vpiSizedSignedFunc) {
        vpi_set_error(vpiCompile, vpiError, "LLG_VPI_REGISTRATION",
                      "unsupported VPI system-function result type");
        return NULL;
    }
    if (find_registration((const char*)data->tfname)) {
        vpi_set_errorf(vpiCompile, vpiError, "LLG_VPI_DUPLICATE",
                       "VPI system task/function `%s` registered twice", data->tfname);
        return NULL;
    }
    if (g_vpi.registration_count >= LLG_VPI_MAX_REGISTRATIONS || !vpi_ready()) {
        if (!g_vpi.error_pending)
            vpi_set_error(vpiCompile, vpiError, "LLG_VPI_LIMIT", "too many VPI registrations");
        return NULL;
    }
    if (g_vpi.registration_count == g_vpi.registration_capacity) {
        int capacity = g_vpi.registration_capacity ? g_vpi.registration_capacity * 2 : 16;
        llg_vpi_registration_t** grown = (llg_vpi_registration_t**)realloc(
            g_vpi.registrations, (size_t)capacity * sizeof(*grown));
        if (!grown) {
            vpi_set_error(vpiCompile, vpiError, "LLG_VPI_NOMEM", "VPI registration allocation failed");
            return NULL;
        }
        g_vpi.registrations = grown;
        g_vpi.registration_capacity = capacity;
    }
    llg_vpi_registration_t* registration =
        (llg_vpi_registration_t*)calloc(1, sizeof(*registration));
    char* name = vpi_strdup((const char*)data->tfname);
    uint32_t slot = registration && name ? alloc_slot(SLOT_SYSTF) : LLG_VPI_NONE;
    if (slot == LLG_VPI_NONE) {
        free(registration);
        free(name);
        vpi_set_error(vpiCompile, vpiError, "LLG_VPI_NOMEM", "VPI registration allocation failed");
        return NULL;
    }
    registration->data = *data;
    registration->name = name;
    registration->data.tfname = (PLI_BYTE8*)name;
    registration->slot = slot;
    g_vpi.slots[slot].pointer = registration;
    g_vpi.registrations[g_vpi.registration_count++] = registration;
    return encode_handle(slot);
}

void vpi_get_systf_info(vpiHandle object, p_vpi_systf_data data) {
    vpi_enter();
    llg_vpi_slot_t* slot = checked_handle(object, NULL);
    if (!slot) return;
    if (slot->kind != SLOT_SYSTF) {
        vpi_error("LLG_VPI_HANDLE", "vpi_get_systf_info needs a vpiUserSystf handle");
        return;
    }
    if (!data) {
        vpi_error("LLG_VPI_ARGUMENT", "null vpi_get_systf_info output");
        return;
    }
    *data = ((llg_vpi_registration_t*)slot->pointer)->data;
}

/* ── lookup by name and index ───────────────────────────────────────────── */

static vpiHandle select_from_suffix(uint32_t row, const char* suffix);
static vpiHandle assertion_by_name(const char* name, vpiHandle scope);

/* Resolve `name` with optional trailing `[i]`, `[i][j]` or `[l:r]` selects
 * after a catalog name. Catalog names themselves may contain brackets
 * (generate blocks), so the longest catalog prefix wins. */
/* `*selected` reports whether `name` names an object followed by a select
 * that does not resolve (an index outside the declared range). */
static vpiHandle member_from_suffix(uint32_t row, const char* suffix);

static vpiHandle lookup_full_name(const char* name, int* selected) {
    *selected = 0;
    uint32_t row = find_row(name);
    if (row != LLG_VPI_NONE) return object_handle(row);
    size_t length = strlen(name);
    char* prefix = (char*)malloc(length + 1u);
    if (!prefix) return NULL;
    memcpy(prefix, name, length + 1u);
    vpiHandle found = NULL;
    for (size_t cut = length; cut-- > 0 && !found;) {
        if (name[cut] != '[' && name[cut] != '.') continue;
        prefix[cut] = '\0';
        row = find_row(prefix);
        if (row != LLG_VPI_NONE) {
            /* A member suffix that names nothing is an ordinary negative
             * answer (S42-D2); only a bad select of an object is an error. */
            if (g_vpi.objects[row].flags & LLG_VPI_OBJ_DYNAMIC) {
                found = dyn_by_suffix(row, name + cut);
            } else if (strchr(name + cut, '.')) {
                found = member_from_suffix(row, name + cut);
            } else {
                *selected = 1;
                found = select_from_suffix(row, name + cut);
            }
        }
        prefix[cut] = name[cut];
    }
    free(prefix);
    return found;
}

static int parse_index(const char** cursor, int64_t* value) {
    const char* p = *cursor;
    int negative = 0;
    if (*p == '-') {
        negative = 1;
        ++p;
    }
    if (*p < '0' || *p > '9') return 0;
    int64_t result = 0;
    while (*p >= '0' && *p <= '9') {
        if (result > (INT64_MAX - 9) / 10) return 0;
        result = result * 10 + (*p++ - '0');
    }
    *value = negative ? -result : result;
    *cursor = p;
    return 1;
}

static vpiHandle element_by_indices(uint32_t row, const int64_t* indices, int count, int quiet);
static PLI_INT32 release_handle(vpiHandle handle);
/* How bit_by_index reports a failure: every error, none, or every error
 * except an index outside the declared range. */
enum { BIT_ERRORS = 0, BIT_QUIET = 1, BIT_QUIET_RANGE = 2 };
static vpiHandle bit_by_index(uint32_t row, uint64_t element, int64_t index, int quiet);

static vpiHandle select_from_suffix(uint32_t row, const char* suffix) {
    int64_t indices[16];
    int count = 0;
    const char* p = suffix;
    while (*p == '[' && count < 16) {
        ++p;
        int64_t left;
        if (!parse_index(&p, &left)) return NULL;
        if (*p == ':') {
            ++p;
            int64_t right;
            if (!parse_index(&p, &right) || *p != ']' || p[1] != '\0') return NULL;
            /* A part-select of a whole vector or element. */
            llg_vpi_model_object_t* object = row_object(row);
            uint64_t element = LLG_VPI_NO_ELEMENT;
            if (is_array_type(object->type)) {
                if ((uint32_t)count != object->dim_count) return NULL;
                vpiHandle base = element_by_indices(row, indices, count, 1);
                llg_vpi_slot_t* slot = decode_handle(base, NULL);
                if (!slot) return NULL;
                element = slot->element;
                vpi_release_handle(base);
            } else if (count) {
                return NULL;
            }
            if (object->is_real) return NULL;
            uint32_t a, b;
            if (!packed_offset(object, left, &a) || !packed_offset(object, right, &b))
                return NULL;
            int64_t declared_left, declared_right;
            packed_range(object, &declared_left, &declared_right);
            /* The select must keep the declared direction (11.5.1). */
            if ((declared_left >= declared_right) != (left >= right) && left != right)
                return NULL;
            return make_select(row, element, a < b ? a : b, (a > b ? a - b : b - a) + 1u,
                               left, right, NULL);
        }
        if (*p != ']') return NULL;
        ++p;
        indices[count++] = left;
    }
    if (*p != '\0' || count == 0) return NULL;
    llg_vpi_model_object_t* object = row_object(row);
    if (is_array_type(object->type)) {
        if ((uint32_t)count == object->dim_count)
            return element_by_indices(row, indices, count, 1);
        if ((uint32_t)count == object->dim_count + 1u) {
            vpiHandle base = element_by_indices(row, indices, count - 1, 1);
            llg_vpi_slot_t* slot = decode_handle(base, NULL);
            if (!slot) return NULL;
            uint64_t element = slot->element;
            vpi_release_handle(base);
            return bit_by_index(row, element, indices[count - 1], BIT_QUIET);
        }
        return NULL;
    }
    return count == 1 ? bit_by_index(row, LLG_VPI_NO_ELEMENT, indices[0], BIT_QUIET) : NULL;
}

/* `[i]...` element indices (an array row) then `.member` steps through the
 * member nodes of the value's typespec: `tb.s.a.b`, `tb.arr[1].a`. */
static vpiHandle member_from_suffix(uint32_t row, const char* suffix) {
    llg_vpi_model_object_t* object = row_object(row);
    const char* p = suffix;
    uint64_t element = LLG_VPI_NO_ELEMENT;
    if (*p == '[') {
        int64_t indices[16];
        int count = 0;
        while (*p == '[' && count < 16) {
            ++p;
            if (!parse_index(&p, &indices[count]) || *p != ']') return NULL;
            ++p;
            ++count;
        }
        if (*p != '.' || !is_array_type(object->type)) return NULL;
        vpiHandle base = element_by_indices(row, indices, count, 1);
        uint32_t base_index;
        llg_vpi_slot_t* slot = decode_handle(base, &base_index);
        if (!slot) return NULL;
        element = slot->element;
        release_slot(base_index);
    }
    const llg_vpi_typespec_t* typespec = element == LLG_VPI_NO_ELEMENT
        ? typespec_ref(object->typespec) : element_typespec(object);
    if (!is_aggregate_typespec(typespec) || (object->flags & LLG_VPI_OBJ_RECORD) ||
        object->is_net || (element == LLG_VPI_NO_ELEMENT && !has_storage(object)))
        return NULL;
    uint32_t first = typespec->first_node, count = typespec->node_count, node = LLG_VPI_NONE;
    while (*p == '.') {
        const char* segment = ++p;
        while (*p && *p != '.') ++p;
        size_t length = (size_t)(p - segment);
        node = LLG_VPI_NONE;
        for (uint32_t i = 0; i < count; ++i) {
            const char* candidate = node_at(first + i)->name;
            if (strlen(candidate) == length && memcmp(candidate, segment, length) == 0) {
                node = first + i;
                break;
            }
        }
        if (node == LLG_VPI_NONE) return NULL;
        first = node_at(node)->first_child;
        count = node_at(node)->child_count;
    }
    if (*p || node == LLG_VPI_NONE) return NULL;
    return make_member(row, element, node);
}

vpiHandle vpi_handle_by_name(PLI_BYTE8* name, vpiHandle scope) {
    vpi_enter();
    if (!vpi_ready()) return NULL;
    if (!name || !*name) {
        vpi_error("LLG_VPI_ARGUMENT", "empty VPI object name");
        return NULL;
    }
    vpiHandle result = NULL;
    int selected = 0;
    if (scope) {
        llg_vpi_slot_t* slot = checked_handle(scope, NULL);
        if (!slot) return NULL;
        llg_vpi_model_object_t* parent =
            slot->kind == SLOT_OBJECT ? row_object(slot->object) : NULL;
        if (!parent || !is_scope_type(parent->type)) {
            vpi_error("LLG_VPI_SCOPE", "VPI name lookup scope is not a scope object");
            return NULL;
        }
        size_t length = strlen(parent->full_name) + 1u + strlen((const char*)name) + 1u;
        char* full = (char*)malloc(length + 1u);
        if (!full) {
            vpi_error("LLG_VPI_NOMEM", "VPI name allocation failed");
            return NULL;
        }
        /* 37.10 detail 5: `::` follows a package name. */
        snprintf(full, length + 1u, "%s%s%s", parent->full_name,
                 parent->type == vpiPackage ? "::" : ".", (const char*)name);
        result = lookup_full_name(full, &selected);
        free(full);
    } else {
        result = lookup_full_name((const char*)name, &selected);
    }
    /* 37.10 detail 6: no object within a compilation unit is found by
     * name (`$unit` itself is). */
    if (result) {
        llg_vpi_slot_t* found = decode_handle(result, NULL);
        const llg_vpi_model_object_t* row = found ? row_object(found->object) : NULL;
        for (const llg_vpi_model_object_t* up = row ? row->parent : NULL; up; up = up->parent)
            if (up->type == vpiPackage && (up->flags & LLG_VPI_OBJ_QUALIFIER)) {
                release_handle(result);
                return NULL;
            }
    }
    /* A labelled assertion is also a named scope of the catalog; the name
     * denotes the assertion (39.3.1). */
    if (!g_vpi.error_pending) {
        llg_vpi_slot_t* found = result ? decode_handle(result, NULL) : NULL;
        llg_vpi_model_object_t* row = found ? row_object(found->object) : NULL;
        if (!result || (row && row->type == vpiNamedBegin && found->kind == SLOT_OBJECT)) {
            vpiHandle assertion = assertion_by_name((const char*)name, scope);
            if (assertion) result = assertion;
        }
    }
    /* A name that denotes no object is an ordinary negative answer: NULL
     * with no error status (decision S42-D2). A select outside the range of
     * an existing object is an error (decision S42-D11). */
    if (!result && selected && !g_vpi.error_pending)
        vpi_set_errorf(vpi_state(), vpiError, "LLG_VPI_NOT_FOUND",
                       "VPI object `%s` has no such element or select", (const char*)name);
    return result;
}

static vpiHandle element_by_indices(uint32_t row, const int64_t* indices, int count, int quiet) {
    llg_vpi_model_object_t* object = row_object(row);
    if (!is_array_type(object->type) || count <= 0 || (uint32_t)count != object->dim_count) {
        if (!quiet) vpi_error("LLG_VPI_INDEX", "wrong number of indices for the array");
        return NULL;
    }
    uint64_t flat = 0;
    for (int d = 0; d < count; ++d) {
        int64_t left, right;
        uint64_t offset;
        if (!array_dim(object, (uint32_t)d, &left, &right) ||
            !index_offset(left, right, indices[d], &offset)) {
            if (!quiet) vpi_error("LLG_VPI_INDEX", "array index is outside the declared range");
            return NULL;
        }
        uint64_t extent = (uint64_t)(left > right ? left - right : right - left) + 1u;
        flat = flat * extent + offset;
    }
    return make_element(row, flat, NULL);
}

static vpiHandle bit_by_index(uint32_t row, uint64_t element, int64_t index, int quiet) {
    llg_vpi_model_object_t* object = row_object(row);
    if (object->is_real || !object->packed ||
        object->type == vpiPackedArrayVar || object->width < 1) {
        if (quiet != BIT_QUIET)
            vpi_error("LLG_VPI_INDEX", "object has no bit-selectable packed value");
        return NULL;
    }
    uint32_t offset;
    if (!packed_offset(object, index, &offset)) {
        if (quiet == BIT_ERRORS)
            vpi_error("LLG_VPI_INDEX", "bit index is outside the declared range");
        return NULL;
    }
    return make_select(row, element, offset, 1, index, index, NULL);
}

/* ── declaration objects (37.13, 37.80, decision A19-D42) ──────────────── */

/* The generate scopes of a generate scope array: the siblings that name it. */
static uint32_t gen_scope_count(uint32_t array) {
    const llg_vpi_model_object_t* object = &g_vpi.objects[array];
    uint32_t parent = object->parent ? row_of(object->parent) : LLG_VPI_NONE, count = 0;
    if (parent == LLG_VPI_NONE) return 0;
    for (uint32_t child = g_vpi.first_child[parent]; child; child = g_vpi.next_sibling[child - 1u])
        count += g_vpi.objects[child - 1u].type == vpiGenScope &&
                 g_vpi.objects[child - 1u].target == object;
    return count;
}

static vpiHandle iterate_gen_scopes(uint32_t array) {
    const llg_vpi_model_object_t* object = &g_vpi.objects[array];
    uint32_t parent = object->parent ? row_of(object->parent) : LLG_VPI_NONE;
    uint32_t count = gen_scope_count(array), used = 0;
    if (count == 0 || parent == LLG_VPI_NONE) return NULL;
    uintptr_t* items = (uintptr_t*)malloc(count * sizeof(*items));
    if (!items) {
        vpi_error("LLG_VPI_NOMEM", "VPI iterator allocation failed");
        return NULL;
    }
    for (uint32_t child = g_vpi.first_child[parent]; child && used < count;
         child = g_vpi.next_sibling[child - 1u])
        if (g_vpi.objects[child - 1u].type == vpiGenScope &&
            g_vpi.objects[child - 1u].target == object)
            items[used++] = (uintptr_t)object_handle(child - 1u);
    return make_list_iterator(vpiGenScope, items, used);
}

/* 37.80: the generate scope `array[index]`; NULL when the loop generated
 * none for that index. */
static vpiHandle gen_scope_by_index(uint32_t array, int64_t index) {
    char name[1024];
    int length = snprintf(name, sizeof(name), "%s[%lld]", g_vpi.objects[array].full_name,
                          (long long)index);
    if (length < 0 || (size_t)length >= sizeof(name)) return NULL;
    uint32_t row = find_row(name);
    if (row == LLG_VPI_NONE || g_vpi.objects[row].target != &g_vpi.objects[array]) return NULL;
    return object_handle(row);
}

/* The one-to-one relations of a generate scope array, genvar, io
 * declaration or generate scope that the generic relations do not give;
 * *handled is 0 for the others. */
static vpiHandle declaration_relation(int type, uint32_t row, int* handled) {
    const llg_vpi_model_object_t* object = &g_vpi.objects[row];
    const llg_vpi_model_object_t* parent = object->parent;
    *handled = 1;
    switch (object->type) {
        case vpiGenScope:
            if (type == vpiGenScopeArray)
                return object->target && object->target->type == vpiGenScopeArray
                           ? object_handle(row_of(object->target))
                           : NULL;
            break;
        case vpiGenScopeArray:
            if (type == vpiGenVar)
                return object->target ? object_handle(row_of(object->target)) : NULL;
            break;
        case vpiIODecl:
            switch (type) {
                case vpiExpr: {
                    /* The port's declaration, the task or function
                     * argument, or the interface member of a modport port. */
                    uint32_t target =
                        object->target ? row_of(object->target) : find_row(object->full_name);
                    return target == LLG_VPI_NONE ? NULL : object_handle(target);
                }
                case vpiTaskFunc:
                    return parent && (parent->type == vpiTask || parent->type == vpiFunction)
                               ? object_handle(row_of(parent))
                               : NULL;
                case vpiModport:
                    return parent && parent->type == vpiModport ? object_handle(row_of(parent))
                                                                : NULL;
                case vpiUdpDefn:
                    vpi_error("LLG_VPI_UNSUPPORTED", "UDP definitions are not objects");
                    return NULL;
                case vpiLeftRange:
                case vpiRightRange:
                    /* A scalar or a value without a packed range has none. */
                    if (!(object->flags & LLG_VPI_OBJ_RANGE)) return NULL;
                    break;
                default:
                    break;
            }
            break;
        default:
            break;
    }
    *handled = 0;
    return NULL;
}

vpiHandle vpi_handle_by_index(vpiHandle object, PLI_INT32 index) {
    vpi_enter();
    llg_vpi_slot_t* slot = checked_handle(object, NULL);
    if (!slot) return NULL;
    if (dyn_slot(slot)) return dyn_handle_by_index(slot, index);
    int64_t value = index;
    if (slot->kind == SLOT_OBJECT) {
        llg_vpi_model_object_t* base = row_object(slot->object);
        if (base->type == vpiGenScopeArray) return gen_scope_by_index(slot->object, value);
        if (is_instance_array_type(base->type)) {
            /* 37.11: an instance of the array; NULL outside its range. */
            if (base->dim_count != 1) {
                vpi_error("LLG_VPI_INDEX",
                          "use vpi_handle_by_multi_index for a multidimensional array");
                return NULL;
            }
            return object_handle(instance_element(slot->object, &value, 1u));
        }
        if (is_array_type(base->type)) {
            if (base->dim_count != 1) {
                vpi_error("LLG_VPI_INDEX",
                          "use vpi_handle_by_multi_index for a multidimensional array");
                return NULL;
            }
            return element_by_indices(slot->object, &value, 1, 0);
        }
        if (is_parameter_type(base->type) || !has_storage(base)) {
            vpi_error("LLG_VPI_INDEX", "object has no access by index");
            return NULL;
        }
        /* A bit index outside a vector's range is a plain NULL answer; one
         * outside an element's range stays an error (decision S42-D7). */
        return bit_by_index(slot->object, LLG_VPI_NO_ELEMENT, value, BIT_QUIET_RANGE);
    }
    if (slot->kind == SLOT_ELEMENT)
        return bit_by_index(slot->object, slot->element, value, BIT_ERRORS);
    vpi_error("LLG_VPI_INDEX", "object has no access by index");
    return NULL;
}

vpiHandle vpi_handle_by_multi_index(vpiHandle object, PLI_INT32 count, PLI_INT32* indices) {
    vpi_enter();
    llg_vpi_slot_t* slot = checked_handle(object, NULL);
    if (!slot) return NULL;
    if (count <= 0 || count > 16 || !indices) {
        vpi_error("LLG_VPI_ARGUMENT", "invalid index array");
        return NULL;
    }
    int64_t values[16];
    for (int i = 0; i < count; ++i) values[i] = indices[i];
    if (slot->kind != SLOT_OBJECT) {
        if (slot->kind == SLOT_ELEMENT && count == 1)
            return bit_by_index(slot->object, slot->element, values[0], BIT_ERRORS);
        vpi_error("LLG_VPI_INDEX", "object has no access by index");
        return NULL;
    }
    uint32_t row = slot->object;
    llg_vpi_model_object_t* base = row_object(row);
    if (base->type == vpiGenScopeArray) {
        if (count == 1) return gen_scope_by_index(row, values[0]);
        vpi_error("LLG_VPI_INDEX", "a generate scope array has one dimension");
        return NULL;
    }
    if (is_instance_array_type(base->type))
        return object_handle(instance_element(row, values, (uint32_t)count));
    if (!is_array_type(base->type)) {
        if (count == 1) return bit_by_index(row, LLG_VPI_NO_ELEMENT, values[0], BIT_QUIET_RANGE);
        vpi_error("LLG_VPI_INDEX", "too many indices for a packed object");
        return NULL;
    }
    if ((uint32_t)count == base->dim_count) return element_by_indices(row, values, count, 0);
    if ((uint32_t)count == base->dim_count + 1u) {
        vpiHandle element = element_by_indices(row, values, count - 1, 0);
        uint32_t element_index;
        llg_vpi_slot_t* element_slot = decode_handle(element, &element_index);
        if (!element_slot) return NULL;
        uint64_t flat = element_slot->element;
        release_slot(element_index);
        return bit_by_index(row, flat, values[count - 1], BIT_ERRORS);
    }
    vpi_error("LLG_VPI_INDEX", "index count does not match the array dimensions");
    return NULL;
}

/* ── one-to-one relations ───────────────────────────────────────────────── */

/* ── assertions and coverage (39.3, 40.5) ───────────────────────────────── */

static vpiHandle make_list_iterator(int type, uintptr_t* items, uint32_t count);
static vpiHandle iterate_collection(const llg_vpi_slot_t* slot);
static int item_in_instance(size_t index, int32_t scope);

static const llg_acov_item_t* slot_item(const llg_vpi_slot_t* slot) {
    if (!slot || slot->kind != SLOT_ASSERTION || slot->left < 0) return NULL;
    return llg_acov_item_at((size_t)slot->left);
}

/* The 39.3.2 assertion type of an item. */
static int assertion_type(const llg_acov_item_t* item) {
    if (!item) return vpiUndefined;
    int immediate = item->form == LLG_ACOV_IMMEDIATE || item->form == LLG_ACOV_DEFERRED;
    switch (item->kind) {
        case LLG_ASSERTION_ASSERT: return immediate ? vpiImmediateAssert : vpiAssert;
        case LLG_ASSERTION_ASSUME: return immediate ? vpiImmediateAssume : vpiAssume;
        default: return immediate ? vpiImmediateCover : vpiCover;
    }
}

static vpiHandle assertion_handle(size_t item) {
    uint32_t index = alloc_slot(SLOT_ASSERTION);
    if (index == LLG_VPI_NONE) return NULL;
    g_vpi.slots[index].left = (int64_t)item;
    return encode_handle(index);
}

/* The coverage instance of a module or interface handle, or -1. */
static int32_t slot_instance(const llg_vpi_slot_t* slot) {
    llg_vpi_model_object_t* object = row_object(slot->object);
    if (slot->kind != SLOT_OBJECT || !object ||
        (object->type != vpiModule && object->type != vpiInterface &&
         object->type != vpiProgram))
        return -1;
    return llg_acov_instance_named(object->full_name);
}

/* A package (or `$unit`) handle: an instance that holds no assertions,
 * FSMs or coverage of its own (16.14.5: no assertion statement in a
 * package). */
static int is_package_slot(const llg_vpi_slot_t* slot) {
    llg_vpi_model_object_t* object = row_object(slot->object);
    return slot->kind == SLOT_OBJECT && object && object->type == vpiPackage;
}

/* vpi_iterate(vpiSequenceDecl | vpiPropertyDecl, scope): the declarations
 * of a scope (37.12), in source order. The declarations are roots of the
 * expression object table; the walk is linear in that table. */
static vpiHandle iterate_declarations(int type, vpiHandle reference) {
    llg_vpi_slot_t* slot = reference ? checked_handle(reference, NULL) : NULL;
    if (!slot) {
        if (!reference)
            vpi_error("LLG_VPI_RELATION", "sequence and property declarations iterate from a scope");
        return NULL;
    }
    const llg_vpi_model_object_t* scope = slot->kind == SLOT_OBJECT ? row_object(slot->object) : NULL;
    if (slot->kind == SLOT_CODE && code_is_unscoped_block((uint32_t)slot->aux)) return NULL;
    if (dyn_is_class_scope(slot)) return NULL;
    if (!scope || !is_scope_type(scope->type)) {
        vpi_error("LLG_VPI_RELATION", "sequence and property declarations iterate from a scope");
        return NULL;
    }
    uint32_t total = g_vpi.code ? g_vpi.code->node_count : 0u, count = 0;
    for (uint32_t i = 0; i < total; ++i) {
        const llg_vpi_code_t* node = &g_vpi.code->nodes[i];
        if (!node->role && node->type == type && node->scope == slot->object + 1u) ++count;
    }
    if (!count) return NULL;
    uintptr_t* items = (uintptr_t*)calloc(count, sizeof(*items));
    if (!items) {
        vpi_error("LLG_VPI_NOMEM", "VPI iterator allocation failed");
        return NULL;
    }
    uint32_t object = slot->object;
    count = 0;
    for (uint32_t i = 0; i < total; ++i) {
        const llg_vpi_code_t* node = &g_vpi.code->nodes[i];
        if (node->role || node->type != type || node->scope != object + 1u) continue;
        vpiHandle handle = code_handle(i);
        if (!handle) break;
        items[count++] = (uintptr_t)handle;
    }
    return make_list_iterator(type, items, count);
}

/* The catalog row + 1 of the scope that declares item `item` (a module,
 * generate scope or named block: the scope of its statement's entry), or 0
 * in a model without expression objects. */
static uint32_t item_scope(const llg_acov_item_t* item) {
    const llg_vpi_code_t* node = item->code ? code_at(item->code - 1u) : NULL;
    return node ? node->scope : 0u;
}

/* The items declared in scope row `row`: every item at any depth below a
 * generate scope (37.80 vpiAssertion), or the concurrent assertions
 * declared directly in a scope (37.12 vpiConcurrentAssertions); decision
 * A19-D31. */
static vpiHandle iterate_scope_assertions(uint32_t row, int concurrent) {
    if (!g_vpi.code) {
        vpi_error("LLG_VPI_UNSUPPORTED",
                  "assertions iterate from a scope only in models generated with --vpi or "
                  "--dpi-lib");
        return NULL;
    }
    size_t total = llg_acov_item_count(), count = 0;
    uintptr_t* items = (uintptr_t*)calloc(total ? total : 1u, sizeof(*items));
    if (!items) {
        vpi_error("LLG_VPI_NOMEM", "VPI iterator allocation failed");
        return NULL;
    }
    const llg_vpi_model_object_t* scope = row_object(row);
    for (size_t i = 0; i < total; ++i) {
        const llg_acov_item_t* item = llg_acov_item_at(i);
        uint32_t declared = item_scope(item);
        if (!declared) continue;
        if (concurrent) {
            if (declared - 1u != row ||
                (item->form != LLG_ACOV_CONCURRENT && item->form != LLG_ACOV_COVER_SEQUENCE))
                continue;
        } else {
            const llg_vpi_model_object_t* object = row_object(declared - 1u);
            while (object && object != scope) object = object->parent;
            if (!object) continue;
        }
        vpiHandle handle = assertion_handle(i);
        if (!handle) break;
        items[count++] = (uintptr_t)handle;
    }
    return make_list_iterator(concurrent ? vpiConcurrentAssertions : vpiAssertion, items,
                              (uint32_t)count);
}

/* vpi_iterate(vpiConcurrentAssertions, scope) (37.12). */
static vpiHandle iterate_concurrent_assertions(vpiHandle reference) {
    llg_vpi_slot_t* slot = reference ? checked_handle(reference, NULL) : NULL;
    if (!slot) {
        if (!reference)
            vpi_error("LLG_VPI_RELATION", "vpiConcurrentAssertions iterates from a scope");
        return NULL;
    }
    /* A block that is not a scope declares nothing (decision A19-D27). */
    if (slot->kind == SLOT_CODE && code_is_unscoped_block((uint32_t)slot->aux)) return NULL;
    if (dyn_is_class_scope(slot)) return NULL;
    const llg_vpi_model_object_t* scope = slot->kind == SLOT_OBJECT ? row_object(slot->object) : NULL;
    if (!scope || !is_scope_type(scope->type)) {
        vpi_error("LLG_VPI_RELATION", "vpiConcurrentAssertions iterates from a scope");
        return NULL;
    }
    if (is_package_slot(slot)) return NULL;
    return iterate_scope_assertions(slot->object, 1);
}

/* vpi_iterate(vpiClassDefn, NULL | instance) (37.10): the class
 * definitions declared in an instance; from NULL, all of them (decision
 * A19-D36). */
static vpiHandle iterate_class_defns(vpiHandle reference) {
    if (!reference) return dyn_iterate_class_defns(LLG_VPI_NONE);
    llg_vpi_slot_t* slot = checked_handle(reference, NULL);
    if (!slot) return NULL;
    const llg_vpi_model_object_t* object = slot->kind == SLOT_OBJECT ? row_object(slot->object) : NULL;
    if (!object || !is_instance_type(object->type)) {
        vpi_error("LLG_VPI_RELATION", "class definitions iterate from an instance or NULL");
        return NULL;
    }
    return dyn_iterate_class_defns(slot->object);
}

/* vpi_iterate(vpiAssertion, NULL | instance | generate scope): every item,
 * the items of one instance (39.3.1) or of a generate scope (37.80). */
static vpiHandle iterate_assertions(vpiHandle reference) {
    int32_t instance = -1;
    if (reference) {
        llg_vpi_slot_t* slot = checked_handle(reference, NULL);
        if (!slot) return NULL;
        llg_vpi_model_object_t* object = row_object(slot->object);
        if (slot->kind == SLOT_OBJECT && object && object->type == vpiGenScope)
            return iterate_scope_assertions(slot->object, 0);
        if (slot->kind != SLOT_OBJECT || !object ||
            (object->type != vpiModule && object->type != vpiInterface &&
             object->type != vpiProgram && object->type != vpiPackage)) {
            vpi_error("LLG_VPI_RELATION", "vpiAssertion iterates from NULL or an instance");
            return NULL;
        }
        instance = slot_instance(slot);
        if (instance < 0) return NULL;
    }
    size_t total = llg_acov_item_count(), count = 0;
    uintptr_t* items = (uintptr_t*)calloc(total ? total : 1u, sizeof(*items));
    if (!items) {
        vpi_error("LLG_VPI_NOMEM", "VPI iterator allocation failed");
        return NULL;
    }
    for (size_t i = 0; i < total; ++i) {
        if (reference && llg_acov_item_at(i)->instance != instance) continue;
        vpiHandle handle = assertion_handle(i);
        if (!handle) break;
        items[count++] = (uintptr_t)handle;
    }
    return make_list_iterator(vpiAssertion, items, (uint32_t)count);
}

/* A named assertion by its full name, or `name` below `scope` (39.3.1:
 * unnamed assertions cannot be found by name). */
static vpiHandle assertion_by_name(const char* name, vpiHandle scope) {
    const char* prefix = NULL;
    if (scope) {
        llg_vpi_slot_t* slot = decode_handle(scope, NULL);
        llg_vpi_model_object_t* parent = slot ? row_object(slot->object) : NULL;
        if (!parent) return NULL;
        prefix = parent->full_name;
    }
    size_t prefix_length = prefix ? strlen(prefix) : 0u;
    for (size_t i = 0; i < llg_acov_item_count(); ++i) {
        const char* full = llg_acov_item_at(i)->full_name;
        if (!full[0]) continue;
        if (prefix) {
            if (strncmp(full, prefix, prefix_length) != 0 || full[prefix_length] != '.' ||
                strcmp(full + prefix_length + 1u, name) != 0)
                continue;
        } else if (strcmp(full, name) != 0) {
            continue;
        }
        return assertion_handle(i);
    }
    return NULL;
}

/* The one-to-one relations of 37.46 (concurrent) and 37.51 (immediate)
 * assertions. */
static int assertion_relation_applies(int type, int relation) {
    switch (type) {
        case vpiAssert:
        case vpiAssume:
            return relation == vpiClockingEvent || relation == vpiDisableCondition ||
                   relation == vpiProperty || relation == vpiStmt || relation == vpiElseStmt;
        case vpiCover:
            return relation == vpiClockingEvent || relation == vpiDisableCondition ||
                   relation == vpiProperty || relation == vpiStmt;
        case vpiImmediateAssert:
        case vpiImmediateAssume: return relation == vpiExpr || relation == vpiStmt || relation == vpiElseStmt;
        case vpiImmediateCover: return relation == vpiExpr || relation == vpiStmt;
        default: return 0;
    }
}

static vpiHandle assertion_relation(int type, const llg_vpi_slot_t* slot) {
    const llg_acov_item_t* item = slot_item(slot);
    if (item && (type == vpiScope || type == vpiModule || type == vpiInstance)) {
        /* The owning instance (37.45 vpiInstance); NULL for an assertion
         * owned by $root. */
        const llg_acov_instance_t* owner =
            item->instance >= 0 ? llg_acov_instance_at((size_t)item->instance) : NULL;
        if (!owner) return NULL;
        uint32_t row = find_row(owner->path);
        if (row != LLG_VPI_NONE) return object_handle(row);
        vpi_error("LLG_VPI_NOT_FOUND", "the assertion's instance is not in the VPI catalog");
        return NULL;
    }
    if (item && assertion_relation_applies(assertion_type(item), type)) {
        const llg_vpi_code_t* node = item->code ? code_at(item->code - 1u) : NULL;
        if (!node) {
            vpi_error("LLG_VPI_UNSUPPORTED",
                      "this model has no expression objects: generate it with --vpi");
            return NULL;
        }
        /* A relation the statement does not have (no disable iff, no else
         * action) is NULL without an error. */
        uint32_t child = code_child(node, type);
        return child == LLG_VPI_NONE ? NULL : code_handle(child);
    }
    vpi_error("LLG_VPI_RELATION", "unsupported VPI one-to-one relation for this object");
    return NULL;
}

/* The expression a failing attempt of `item` reports as failExpr (39.4.2,
 * decision A17-D4): the property expression of a concurrent assertion (the
 * property instance, or the vpiPropertyExpr of its property specification)
 * or the condition of an immediate assertion; LLG_VPI_NONE without
 * expression objects. */
static uint32_t assertion_fail_code(const llg_acov_item_t* item) {
    const llg_vpi_code_t* node = item && item->code ? code_at(item->code - 1u) : NULL;
    if (!node) return LLG_VPI_NONE;
    uint32_t child = code_child(node, vpiExpr);
    if (child != LLG_VPI_NONE) return code_visible(child);
    child = code_child(node, vpiProperty);
    const llg_vpi_code_t* property = child == LLG_VPI_NONE ? NULL : code_at(child);
    if (property && property->type == vpiPropertySpec) child = code_child(property, vpiPropertyExpr);
    return child == LLG_VPI_NONE ? LLG_VPI_NONE : code_visible(child);
}

/* An integer property of an assertion handle (39.3.2, 40.5.3). */
static int assertion_property(const llg_vpi_slot_t* slot, int property, uint64_t* value) {
    const llg_acov_item_t* item = slot_item(slot);
    if (!item) return 0;
    switch (property) {
        case vpiIsDeferred: *value = item->form == LLG_ACOV_DEFERRED; return 1;
        /* The statement's source position (39.3.2). */
        case vpiLineNo:
        case vpiStartLine:
            if (!item->line) return 0;
            *value = item->line;
            return 1;
        case vpiColumn:
            if (!item->column) return 0;
            *value = item->column;
            return 1;
        case vpiEndLine:
            if (!item->end_line) return 0;
            *value = item->end_line;
            return 1;
        case vpiEndColumn:
            if (!item->end_column) return 0;
            *value = item->end_column;
            return 1;
        case vpiIsCoverSequence:
            if (assertion_type(item) != vpiCover) return 0;
            *value = item->form == LLG_ACOV_COVER_SEQUENCE;
            return 1;
        case vpiIsClockInferred: {
            /* 37.46: concurrent assertions; the entry records how the clock
             * was found. */
            const llg_vpi_code_t* node = item->code ? code_at(item->code - 1u) : NULL;
            if (item->form != LLG_ACOV_CONCURRENT && item->form != LLG_ACOV_COVER_SEQUENCE)
                return 0;
            if (!node) return 0;
            *value = (node->flags & LLG_VPI_CODE_INFERRED) != 0;
            return 1;
        }
        case vpiCovered:
            /* Attempted, succeeded at least once and never failed (40.5.3). */
            *value = item->attempts && item->successes && !item->failures;
            return 1;
        case vpiCoverMax: *value = 1; return 1;
        case vpiCoveredCount:
        case vpiAssertSuccessCovered: *value = item->successes; return 1;
        case vpiAssertAttemptCovered: *value = item->attempts; return 1;
        case vpiAssertFailureCovered: *value = item->failures; return 1;
        case vpiAssertVacuousSuccessCovered: *value = item->vacuous; return 1;
        case vpiAssertDisableCovered: *value = item->disabled; return 1;
        case vpiAssertKillCovered: *value = item->killed; return 1;
        default: return 0;
    }
}

static int is_coverage_type_property(int property) {
    return property >= vpiAssertCoverage && property <= vpiToggleCoverage;
}

/* vpi_get(<coverage type>, instance): the covered items of that type owned
 * by the instance (40.5.3, S46-D10). */
static PLI_INT32 instance_coverage(const llg_vpi_slot_t* slot, int property) {
    if (property != vpiAssertCoverage && property != vpiFsmStateCoverage) {
        vpi_error("LLG_VPI_UNSUPPORTED",
                  "statement and toggle coverage are not implemented (S46-D5)");
        return vpiUndefined;
    }
    if (is_package_slot(slot)) return 0;
    int32_t instance = slot_instance(slot);
    if (instance < 0) {
        vpi_error("LLG_VPI_RELATION", "coverage properties need an instance");
        return vpiUndefined;
    }
    /* FSM state coverage counts the reached states of the instance's FSMs. */
    size_t covered = property == vpiFsmStateCoverage ? llg_fcov_covered_in(instance)
                                                     : llg_acov_covered_in(instance);
    return covered > INT32_MAX ? INT32_MAX : (PLI_INT32)covered;
}

/* The 40.3.1 coverage type of a VPI coverage type property, or -1. */
static int sv_coverage_type(int type) {
    return is_coverage_type_property(type) ? 20 + (type - vpiAssertCoverage) : -1;
}

/* vpi_control(vpiCoverageStart/Stop/Reset/Check, type, instance or
 * assertion): the SV_COV_* status of $coverage_control (40.5.4, S46-D10). */
static PLI_INT32 coverage_control(int operation, int type, vpiHandle object) {
    llg_vpi_slot_t* slot = checked_handle(object, NULL);
    if (!slot) return SV_COV_ERROR;
    int control = operation == vpiCoverageStart ? 0
                  : operation == vpiCoverageStop ? 1
                  : operation == vpiCoverageReset ? 2 : 3;
    int sv_type = sv_coverage_type(type);
    if (sv_type < 0) {
        vpi_error("LLG_VPI_ARGUMENT", "unknown VPI coverage type");
        return SV_COV_ERROR;
    }
    if (slot->kind == SLOT_ASSERTION)
        return llg_coverage_control_vpi(control, sv_type, -1, slot->left);
    if (slot->kind == SLOT_FSM || slot->kind == SLOT_FSMSTATE) {
        vpi_error("LLG_VPI_RELATION",
                  "FSM coverage is controlled only per instance (40.5.4, decision A17-D13)");
        return SV_COV_ERROR;
    }
    int32_t instance = slot_instance(slot);
    if (instance < 0) {
        vpi_error("LLG_VPI_RELATION", "coverage control needs an instance or an assertion");
        return SV_COV_ERROR;
    }
    return llg_coverage_control_vpi(control, sv_type, instance, -1);
}

/* vpi_control(vpiCoverageSave/Merge, type, name) (40.5.4). */
static PLI_INT32 coverage_file_control(int operation, int type, const char* name) {
    int sv_type = sv_coverage_type(type);
    if (sv_type < 0 || !name) {
        vpi_error("LLG_VPI_ARGUMENT", "coverage save and merge need a coverage type and a name");
        return SV_COV_ERROR;
    }
    return llg_coverage_file_vpi(operation == vpiCoverageMerge, sv_type, name);
}

/* ── FSM objects (40.5.3, decision A17-D13) ──────────────────────────────────
 *
 * An FSM handle (vpiFsm) names one row of the FSM coverage table, a state
 * handle (vpiFsmHandle) one state of it. Both read the live counters of
 * coverage/fsm.c. */

static const llg_fcov_fsm_t* slot_fsm(const llg_vpi_slot_t* slot) {
    if (!slot || (slot->kind != SLOT_FSM && slot->kind != SLOT_FSMSTATE) || slot->left < 0)
        return NULL;
    const llg_fcov_fsm_t* fsm = llg_fcov_at((size_t)slot->left);
    if (fsm && slot->kind == SLOT_FSMSTATE && (slot->aux < 0 || (uint32_t)slot->aux >= fsm->n_states))
        return NULL;
    return fsm;
}

static vpiHandle fsm_handle(llg_vpi_slot_kind_t kind, size_t fsm, uint32_t state) {
    uint32_t index = alloc_slot(kind);
    if (index == LLG_VPI_NONE) return NULL;
    g_vpi.slots[index].left = (int64_t)fsm;
    g_vpi.slots[index].aux = (int32_t)state;
    return encode_handle(index);
}

/* vpi_iterate(vpiFsm, instance): the FSMs the instance itself declares. */
static vpiHandle iterate_fsms(vpiHandle reference) {
    llg_vpi_slot_t* slot = reference ? checked_handle(reference, NULL) : NULL;
    if (reference && !slot) return NULL;
    if (slot && is_package_slot(slot)) return NULL;
    int32_t instance = slot ? slot_instance(slot) : -1;
    if (instance < 0) {
        vpi_error("LLG_VPI_RELATION", "vpiFsm iterates from a module, interface or program instance");
        return NULL;
    }
    size_t total = llg_fcov_count(), count = 0;
    uintptr_t* items = (uintptr_t*)calloc(total ? total : 1u, sizeof(*items));
    if (!items) {
        vpi_error("LLG_VPI_NOMEM", "VPI iterator allocation failed");
        return NULL;
    }
    for (size_t i = 0; i < total; ++i) {
        if (llg_fcov_at(i)->instance != instance) continue;
        vpiHandle handle = fsm_handle(SLOT_FSM, i, 0);
        if (!handle) break;
        items[count++] = (uintptr_t)handle;
    }
    return make_list_iterator(vpiFsm, items, (uint32_t)count);
}

/* vpi_iterate(vpiFsmStates, fsm): its states in declaration order. */
static vpiHandle iterate_fsm_states(vpiHandle reference) {
    llg_vpi_slot_t* slot = checked_handle(reference, NULL);
    if (!slot) return NULL;
    const llg_fcov_fsm_t* fsm = slot->kind == SLOT_FSM ? slot_fsm(slot) : NULL;
    if (!fsm) {
        vpi_error("LLG_VPI_RELATION", "vpiFsmStates iterates from an FSM handle");
        return NULL;
    }
    uintptr_t* items = (uintptr_t*)calloc(fsm->n_states ? fsm->n_states : 1u, sizeof(*items));
    if (!items) {
        vpi_error("LLG_VPI_NOMEM", "VPI iterator allocation failed");
        return NULL;
    }
    uint32_t count = 0;
    for (uint32_t i = 0; i < fsm->n_states; ++i) {
        vpiHandle handle = fsm_handle(SLOT_FSMSTATE, (size_t)slot->left, i);
        if (!handle) break;
        items[count++] = (uintptr_t)handle;
    }
    return make_list_iterator(vpiFsmHandle, items, count);
}

/* vpi_handle(vpiFsmStateExpression | vpiScope | vpiModule, fsm) and
 * vpi_handle(vpiFsm, state). */
static vpiHandle fsm_relation(int type, const llg_vpi_slot_t* slot) {
    const llg_fcov_fsm_t* fsm = slot_fsm(slot);
    if (!fsm) {
        vpi_error("LLG_VPI_HANDLE", "the FSM handle is no longer valid");
        return NULL;
    }
    if (slot->kind == SLOT_FSMSTATE) {
        if (type == vpiFsm) return fsm_handle(SLOT_FSM, (size_t)slot->left, 0);
    } else if (type == vpiScope || type == vpiModule) {
        const llg_acov_instance_t* owner = llg_acov_instance_at((size_t)fsm->instance);
        uint32_t row = owner ? find_row(owner->path) : LLG_VPI_NONE;
        if (row != LLG_VPI_NONE) return object_handle(row);
        vpi_error("LLG_VPI_NOT_FOUND", "the FSM's instance is not in the VPI catalog");
        return NULL;
    } else if (type == vpiFsmStateExpression) {
        if (!fsm->expression) {
            /* A concatenation of state signals is no VPI object of llg. */
            vpi_error("LLG_VPI_UNSUPPORTED",
                      "the state expression of this FSM is a concatenation, which has no VPI "
                      "object (decision A17-D13)");
            return NULL;
        }
        uint32_t row = find_row(fsm->expression);
        if (row == LLG_VPI_NONE) {
            vpi_error("LLG_VPI_NOT_FOUND", "the FSM's state signal is not in the VPI catalog");
            return NULL;
        }
        if (!fsm->select) return object_handle(row);
        char suffix[64];
        snprintf(suffix, sizeof suffix, "[%lld:%lld]", (long long)fsm->select_left,
                 (long long)fsm->select_right);
        vpiHandle select = select_from_suffix(row, suffix);
        if (!select)
            vpi_error("LLG_VPI_NOT_FOUND", "the FSM's state part-select has no VPI object");
        return select;
    }
    vpi_error("LLG_VPI_RELATION", "unsupported VPI one-to-one relation for this object");
    return NULL;
}

/* The integer properties of an FSM or state handle (40.5.3): vpiCovered is
 * the number of reached states (0 or 1 for a state), vpiCoverMax the number
 * of states (1 for a state) and vpiCoveredCount the visits. */
static int fsm_property(const llg_vpi_slot_t* slot, int property, uint64_t* value) {
    const llg_fcov_fsm_t* fsm = slot_fsm(slot);
    if (!fsm) return 0;
    if (slot->kind == SLOT_FSMSTATE) {
        uint64_t visits = fsm->visits[slot->aux];
        switch (property) {
            case vpiCovered: *value = visits != 0; return 1;
            case vpiCoverMax: *value = 1; return 1;
            case vpiCoveredCount: *value = visits; return 1;
            case vpiSize: *value = fsm->size; return 1;
            default: return 0;
        }
    }
    uint64_t reached = 0, visits = 0;
    for (uint32_t i = 0; i < fsm->n_states; ++i) {
        reached += fsm->visits[i] != 0;
        visits = fsm->visits[i] > UINT64_MAX - visits ? UINT64_MAX : visits + fsm->visits[i];
    }
    switch (property) {
        case vpiCovered: *value = reached; return 1;
        case vpiCoverMax: *value = fsm->n_states; return 1;
        case vpiCoveredCount: *value = visits; return 1;
        case vpiLineNo: *value = fsm->line; return 1;
        default: return 0;
    }
}

static vpiHandle call_handle_for(llg_vpi_callsite_t* site) {
    if (site->call_slot) {
        uint32_t index = site->call_slot - 1u;
        if (g_vpi.slots[index].kind == SLOT_CALL && g_vpi.slots[index].pointer == site)
            return encode_handle(index);
    }
    uint32_t index = alloc_slot(SLOT_CALL);
    if (index == LLG_VPI_NONE) return NULL;
    g_vpi.slots[index].pointer = site;
    site->call_slot = index + 1u;
    return encode_handle(index);
}

static uint32_t enclosing_module(uint32_t row) {
    llg_vpi_model_object_t* object = row_object(row);
    while (object && object->type != vpiModule && object->type != vpiInterface)
        object = object->parent;
    return object ? row_of(object) : LLG_VPI_NONE;
}

/* The immediate package, module, interface or program instance of `row`
 * (37.10 detail 3), `row` itself included. */
static uint32_t enclosing_instance(uint32_t row) {
    llg_vpi_model_object_t* object = row_object(row);
    while (object && !is_instance_type(object->type)) object = object->parent;
    return object ? row_of(object) : LLG_VPI_NONE;
}

/* Bounds of a range-bearing handle: `vpiLeftRange` (left != 0). */
static int handle_bound(const llg_vpi_slot_t* slot, int left, int64_t* value) {
    llg_vpi_model_object_t* object = row_object(slot->object);
    switch (slot->kind) {
        case SLOT_OBJECT:
            if (!object) return 0;
            if (is_array_type(object->type) || is_instance_array_type(object->type)) {
                int64_t l, r;
                if (!array_dim(object, 0, &l, &r)) return 0;
                *value = left ? l : r;
                return 1;
            }
            if (object->is_real || !(object->flags & LLG_VPI_OBJ_RANGE)) return 0;
            *value = left ? object->left : object->right;
            return 1;
        case SLOT_ELEMENT:
            if (object->is_real || !(object->flags & LLG_VPI_OBJ_RANGE)) return 0;
            *value = left ? object->left : object->right;
            return 1;
        case SLOT_SELECT:
            if (slot->span < 2) return 0;
            *value = left ? slot->left : slot->right;
            return 1;
        case SLOT_RANGE:
            *value = left ? slot->left : slot->right;
            return 1;
        case SLOT_MEMBER:
        case SLOT_TYPESPEC: {
            /* 37.23 detail 9: the leftmost packed or unpacked dimension. */
            const llg_vpi_typespec_t* typespec = slot->kind == SLOT_TYPESPEC
                ? typespec_at((uint32_t)slot->aux) : value_typespec(slot);
            if (!typespec || !typespec->range_count) return 0;
            const int64_t* range = &g_vpi.types->ranges[2u * typespec->first_range];
            *value = left ? range[0] : range[1];
            return 1;
        }
        default:
            return 0;
    }
}

/* ── force and release statements (38.36.1) ─────────────────────────────── */

static int compare_storage(const void* left, const void* right) {
    const llg_vpi_storage_t *a = (const llg_vpi_storage_t*)left, *b = (const llg_vpi_storage_t*)right;
    return a->begin < b->begin ? -1 : a->begin > b->begin ? 1 : 0;
}

/* Index the rows that own storage (variables, nets, arrays and members kept
 * as their own signals; not union slices, which share one extent) by
 * address, once. */
static int storage_index(void) {
    if (g_vpi.storage || !g_vpi.object_count) return 1;
    llg_vpi_storage_t* entries =
        (llg_vpi_storage_t*)calloc(g_vpi.object_count, sizeof(*entries));
    if (!entries) {
        vpi_error("LLG_VPI_NOMEM", "VPI storage index allocation failed");
        return 0;
    }
    size_t count = 0;
    for (uint32_t row = 0; row < g_vpi.object_count; ++row) {
        const llg_vpi_model_object_t* object = &g_vpi.objects[row];
        if (!has_storage(object) || (object->flags & LLG_VPI_OBJ_SLICE) ||
            !(object->type == vpiNet || is_variable_type(object->type) ||
              is_array_type(object->type)))
            continue;
        llg_vpi_storage_t* entry = &entries[count++];
        entry->begin = object->is_real ? (uintptr_t)object->real : (uintptr_t)object->packed;
        entry->count = is_array_type(object->type) ? g_vpi.element_counts[row] : 1u;
        entry->size = object->is_real ? sizeof(double) : sizeof(sv4_t);
        entry->row = row;
    }
    qsort(entries, count, sizeof(*entries), compare_storage);
    g_vpi.storage = entries;
    g_vpi.storage_count = count;
    return 1;
}

/* The row and element whose storage is `address`, or 0. */
static int storage_row(const void* address, uint32_t* row, uint64_t* element) {
    uintptr_t key = (uintptr_t)address;
    size_t low = 0, high = g_vpi.storage_count;
    while (low < high) {
        size_t middle = low + (high - low) / 2u;
        if (g_vpi.storage[middle].begin <= key) low = middle + 1u;
        else high = middle;
    }
    if (!low) return 0;
    const llg_vpi_storage_t* entry = &g_vpi.storage[low - 1u];
    uintptr_t offset = key - entry->begin;
    if (offset % entry->size || offset / entry->size >= entry->count) return 0;
    *row = entry->row;
    *element = is_array_type(g_vpi.objects[entry->row].type) ? offset / entry->size
                                                             : LLG_VPI_NO_ELEMENT;
    return 1;
}

/* The row and element a force part writes. A net array element is its own
 * net; it is found through the alias that publishes the net, bit for bit,
 * into the array cell. */
static int part_location(const llg_force_part_t* part, uint32_t* row, uint64_t* element) {
    if (storage_row(part->target, row, element)) return 1;
    const llg_net_t* net = part->net;
    if (!net) return 0;
    for (int i = 0; i < net->n_aliases; ++i) {
        const llg_net_alias_t* alias = net->aliases[i];
        if (alias->publication_target && alias->n_parts == 1 && alias->parts[0].net == net &&
            alias->parts[0].signal_bit == 0 && alias->parts[0].group_bit == 0 &&
            alias->parts[0].bit_count == alias->width &&
            alias->width == llg_sv4_width(*part->target) &&
            storage_row(alias->publication_target, row, element))
            return 1;
    }
    return 0;
}

/* Declared packed index of storage bit `offset` (inverse of packed_offset). */
static int64_t declared_index(const llg_vpi_model_object_t* object, uint32_t offset) {
    int64_t left, right;
    packed_range(object, &left, &right);
    return left >= right ? right + (int64_t)offset : right - (int64_t)offset;
}

/* The one object a statement writes: a whole variable, net, element or
 * member, or a contiguous range of one (its parts may split it bit by bit).
 * 0 for a concatenation or storage without a catalog object. */
static int statement_target(const llg_vpi_stmt_t* stmt, uint32_t* row, uint64_t* element,
                            uint32_t* lsb, uint32_t* width) {
    if (stmt->real) {
        *lsb = *width = 0;
        return storage_row(stmt->real, row, element);
    }
    if (stmt->n_parts <= 0 || !part_location(&stmt->parts[0], row, element)) return 0;
    int64_t low = stmt->parts[0].right, high = stmt->parts[0].left;
    int64_t shift = (int64_t)stmt->parts[0].value_lsb - low;
    uint64_t bits = 0;
    for (int i = 0; i < stmt->n_parts; ++i) {
        const llg_force_part_t* part = &stmt->parts[i];
        uint32_t part_row;
        uint64_t part_element;
        if (!part_location(part, &part_row, &part_element) || part_row != *row ||
            part_element != *element || (int64_t)part->value_lsb - part->right != shift)
            return 0;
        if (part->right < low) low = part->right;
        if (part->left > high) high = part->left;
        bits += part->width;
    }
    if (bits != (uint64_t)(high - low + 1)) return 0;
    *lsb = (uint32_t)low;
    *width = (uint32_t)bits;
    return 1;
}

/* A handle for the object a statement writes, or NULL (no error). */
static vpiHandle statement_lhs(const llg_vpi_stmt_t* stmt) {
    uint32_t row, lsb, width;
    uint64_t element;
    if (!statement_target(stmt, &row, &element, &lsb, &width)) return NULL;
    const llg_vpi_model_object_t* object = row_object(row);
    if (stmt->real || (lsb == 0 && width == object->width))
        return element == LLG_VPI_NO_ELEMENT ? object_handle(row) : make_element(row, element, NULL);
    return make_select(row, element, lsb, width, declared_index(object, lsb + width - 1u),
                       declared_index(object, lsb), NULL);
}

/* 38.36.1: vpiLhs of the reported statement. A statement whose left-hand
 * side is a concatenation has no expression object in llg. */
/* ── expression and statement objects (37.54-37.74) ────────────────────── */

/* Expression evaluation recurses once per operand level; deeper trees (or
 * corrupt tables) stop with an error instead of exhausting the stack. */
#define LLG_VPI_MAX_EVAL_DEPTH 4096

static const llg_vpi_code_t* code_at(uint32_t index) {
    return g_vpi.code && index < g_vpi.code->node_count ? &g_vpi.code->nodes[index] : NULL;
}

static int code_is_select(int type) {
    return type == vpiBitSelect || type == vpiPartSelect || type == vpiIndexedPartSelect;
}

/* The selects named by their text (decision A19-D24). */
static int code_is_named_select(int type) {
    return code_is_select(type) || type == vpiVarSelect;
}

static int code_is_statement(int type) {
    switch (type) {
        case vpiBegin: case vpiFork: case vpiNamedBegin: case vpiNamedFork:
        case vpiAssignment: case vpiIf: case vpiIfElse: case vpiCase: case vpiFor:
        case vpiWhile: case vpiRepeat: case vpiDoWhile: case vpiForever: case vpiForeachStmt:
        case vpiDelayControl: case vpiEventControl: case vpiRepeatControl: case vpiEventStmt:
        case vpiWait: case vpiOrderedWait: case vpiWaitFork: case vpiDisable:
        case vpiDisableFork: case vpiReturnStmt: case vpiBreak: case vpiContinue:
        case vpiNullStmt: case vpiForce: case vpiRelease: case vpiAssignStmt: case vpiDeassign:
        case vpiTaskCall: case vpiSysTaskCall: case vpiMethodTaskCall:
        case vpiImmediateAssert: case vpiImmediateAssume: case vpiImmediateCover:
        case vpiAssert: case vpiAssume: case vpiCover:
            return 1;
        default:
            return 0;
    }
}

/* The sequence and property objects of 37.47-37.50 that are neither
 * statements nor expressions with a value. */
static int code_is_assertion_object(int type) {
    switch (type) {
        case vpiPropertySpec: case vpiPropertyInst: case vpiSequenceInst:
        case vpiPropertyDecl: case vpiSequenceDecl: case vpiPropFormalDecl:
        case vpiSeqFormalDecl:
            return 1;
        default:
            return 0;
    }
}

/* The vpiType of an entry: an alias has its catalog object's type. */
static int code_type(const llg_vpi_code_t* node) {
    if (node->type) return node->type;
    return node->object >= 0 ? g_vpi.objects[node->object].type : vpiRefObj;
}

/* A begin or fork code object: one without declarations, which is not a
 * scope (37.12 detail 1); a scope is its catalog row. */
static int code_is_unscoped_block(uint32_t index) {
    const llg_vpi_code_t* node = code_at(index);
    if (!node) return 0;
    int type = code_type(node);
    return type == vpiBegin || type == vpiFork;
}

/* The objects of a constraint (37.30, 37.34) other than expressions. */
static int code_is_constraint_object(int type) {
    switch (type) {
        case vpiConstraint: case vpiConstraintOrdering: case vpiDistribution: case vpiDistItem:
        case vpiImplication: case vpiConstrIf: case vpiConstrIfElse:
        /* A value range `[low:high]` (37.30 vpiValueRange, 11.4.13). */
        case vpiRange:
            return 1;
        default:
            return 0;
    }
}

static int code_is_expression(const llg_vpi_code_t* node) {
    int type = code_type(node);
    return !code_is_statement(type) && type != vpiCaseItem && !code_is_assertion_object(type) &&
           !code_is_constraint_object(type);
}

static const char* code_text(const llg_vpi_code_t* node) {
    if (code_is_select(node->type) || !node->data || node->data > g_vpi.code->text_count)
        return NULL;
    return g_vpi.code->texts[node->data - 1u];
}

static int code_range(const llg_vpi_code_t* node, int64_t* left, int64_t* right) {
    if (!code_is_select(node->type) || !node->data || node->data > g_vpi.code->range_count)
        return 0;
    *left = g_vpi.code->ranges[2u * (node->data - 1u)];
    *right = g_vpi.code->ranges[2u * (node->data - 1u) + 1u];
    return 1;
}

static int code_tables_valid(const llg_vpi_code_tables_t* t) {
    uint32_t nodes = t ? t->node_count : 0u;
    for (size_t row = 0; row < g_vpi.object_count; ++row) {
        const llg_vpi_model_object_t* object = &g_vpi.objects[row];
        if (object->code > nodes) return 0;
        for (uint32_t i = 0; i < object->conn_count; ++i)
            if (object->conns[i].ref.code > nodes) return 0;
    }
    if (!t) return 1;
    if ((t->node_count && !t->nodes) || (t->text_count && !t->texts) ||
        (t->file_count && !t->files) || (t->range_count && !t->ranges) ||
        (t->span_count && !t->spans) ||
        t->node_count >= (uint32_t)INT32_MAX)
        return 0;
    uint32_t typespecs = g_vpi.types ? g_vpi.types->typespec_count : 0u;
    for (uint32_t i = 0; i < t->node_count; ++i) {
        const llg_vpi_code_t* node = &t->nodes[i];
        /* Children follow their parent, so every walk terminates. */
        if (node->count && (node->first <= i || node->first > t->node_count ||
                            node->count > t->node_count - node->first))
            return 0;
        if (node->object >= 0 && (size_t)node->object >= g_vpi.object_count) return 0;
        if (node->scope > g_vpi.object_count || node->file > t->file_count ||
            node->typespec > typespecs || node->span > t->span_count)
            return 0;
        if (node->data > (code_is_select(node->type) ? t->range_count : t->text_count)) return 0;
        if (node->data && !code_is_select(node->type) && !t->texts[node->data - 1u]) return 0;
        if (node->file && !t->files[node->file - 1u]) return 0;
        /* A link names a root without children of its own. */
        if ((node->flags & LLG_VPI_CODE_LINK) &&
            (node->count || node->first >= t->node_count ||
             (t->nodes[node->first].flags & LLG_VPI_CODE_LINK)))
            return 0;
    }
    return 1;
}

int llg_vpi_model_code(const llg_vpi_code_tables_t* tables) {
    g_vpi.code = tables;
    return 1;
}

/* The entry an application sees for entry `index`: implicit conversions
 * are not objects of their own. */
static uint32_t code_visible(uint32_t index) {
    const llg_vpi_code_t* node = code_at(index);
    if (node && (node->flags & LLG_VPI_CODE_LINK)) return node->first;
    while (node && (node->flags & LLG_VPI_CODE_HIDDEN) && node->count) {
        index = node->first;
        node = code_at(index);
    }
    return index;
}

/* The handle of entry `index`: the catalog object it stands for, else a
 * handle of its own. */
static vpiHandle code_handle(uint32_t index) {
    index = code_visible(index);
    const llg_vpi_code_t* node = code_at(index);
    if (!node) return NULL;
    if ((node->flags & LLG_VPI_CODE_ALIAS) && node->object >= 0)
        return object_handle((uint32_t)node->object);
    return make_aux_slot(SLOT_CODE, LLG_VPI_NONE, index);
}

/* The first child of `node` reached by `relation`, or LLG_VPI_NONE. */
static uint32_t code_child(const llg_vpi_code_t* node, int relation) {
    for (uint32_t i = 0; i < node->count; ++i)
        if (g_vpi.code->nodes[node->first + i].role == relation) return node->first + i;
    return LLG_VPI_NONE;
}

/* Whether 37.54-37.74 give objects of `type` the one-to-one `relation`
 * (a missing optional target is then NULL without an error). */
static int code_relation_applies(int type, int relation) {
    switch (type) {
        case vpiAssignment:
            return relation == vpiLhs || relation == vpiRhs || relation == vpiDelayControl ||
                   relation == vpiEventControl || relation == vpiRepeatControl;
        case vpiIf: case vpiWhile: case vpiRepeat: case vpiDoWhile: case vpiWait:
        case vpiEventControl:
            return relation == vpiCondition || relation == vpiStmt;
        case vpiIfElse:
            return relation == vpiCondition || relation == vpiStmt || relation == vpiElseStmt;
        case vpiCase: return relation == vpiCondition;
        case vpiCaseItem: case vpiForever: return relation == vpiStmt;
        case vpiFor:
            return relation == vpiForInitStmt || relation == vpiCondition ||
                   relation == vpiForIncStmt || relation == vpiStmt;
        case vpiForeachStmt: return relation == vpiVariables || relation == vpiStmt;
        case vpiOrderedWait: return relation == vpiStmt || relation == vpiElseStmt;
        case vpiDelayControl: return relation == vpiDelay || relation == vpiStmt;
        case vpiRepeatControl: return relation == vpiExpr || relation == vpiEventControl;
        case vpiEventStmt: return relation == vpiNamedEvent;
        case vpiDisable: return relation == vpiExpr;
        case vpiReturnStmt: return relation == vpiCondition;
        case vpiForce: case vpiAssignStmt: return relation == vpiLhs || relation == vpiRhs;
        case vpiRelease: case vpiDeassign: return relation == vpiLhs;
        case vpiFuncCall: return relation == vpiFunction;
        case vpiTaskCall: return relation == vpiTask;
        case vpiMethodFuncCall:
            return relation == vpiFunction || relation == vpiPrefix || relation == vpiWith;
        case vpiMethodTaskCall:
            return relation == vpiTask || relation == vpiPrefix || relation == vpiWith;
        case vpiBitSelect: case vpiVarSelect:
            return relation == vpiParent || relation == vpiIndex;
        case vpiPartSelect:
            return relation == vpiParent || relation == vpiLeftRange || relation == vpiRightRange;
        case vpiIndexedPartSelect:
            return relation == vpiParent || relation == vpiBaseExpr || relation == vpiWidthExpr;
        case vpiPropertySpec:
            return relation == vpiClockingEvent || relation == vpiDisableCondition ||
                   relation == vpiPropertyExpr;
        case vpiSequenceInst: return relation == vpiSequenceDecl;
        case vpiPropertyInst: return relation == vpiPropertyDecl || relation == vpiDisableCondition;
        /* 37.30, 37.34. */
        case vpiDistribution: return relation == vpiExpr;
        case vpiDistItem: return relation == vpiValueRange || relation == vpiWeight;
        case vpiImplication: case vpiConstrIf: case vpiConstrIfElse:
            return relation == vpiCondition;
        case vpiRange: return relation == vpiLeftRange || relation == vpiRightRange;
        default:
            return 0;
    }
}

/* Whether objects of `type` iterate `relation`. */
static int code_iteration_applies(int type, int relation) {
    switch (type) {
        case vpiOperation: return relation == vpiOperand;
        case vpiBegin: case vpiFork: case vpiNamedBegin: case vpiNamedFork:
            return relation == vpiStmt;
        case vpiCase: return relation == vpiCaseItem;
        case vpiCaseItem: return relation == vpiExpr;
        case vpiFor: return relation == vpiForInitStmt || relation == vpiForIncStmt;
        case vpiFuncCall: case vpiTaskCall: case vpiSysFuncCall: case vpiSysTaskCall:
        case vpiMethodFuncCall: case vpiMethodTaskCall:
            return relation == vpiArgument;
        case vpiVarSelect: return relation == vpiIndex;
        case vpiForeachStmt: return relation == vpiLoopVars;
        case vpiOrderedWait: return relation == vpiCondition;
        case vpiSequenceInst: case vpiPropertyInst: return relation == vpiArgument;
        case vpiSequenceDecl: return relation == vpiSeqFormalDecl;
        case vpiPropertyDecl: return relation == vpiPropFormalDecl;
        /* 37.30, 37.34. */
        case vpiConstraint: return relation == vpiConstraintItem;
        case vpiDistribution: return relation == vpiDistItem;
        case vpiConstraintOrdering: return relation == vpiSolveBefore || relation == vpiSolveAfter;
        case vpiImplication: case vpiConstrIf: return relation == vpiConstraintExpr;
        case vpiConstrIfElse: return relation == vpiConstraintExpr || relation == vpiElseConst;
        default:
            return 0;
    }
}

static void code_opaque_error(void) {
    vpi_error("LLG_VPI_UNSUPPORTED", "llg does not decompose this expression or statement");
}

/* The relations of 37.47 and 37.49 llg does not keep: a declaration's body
 * and variables are expanded into each instance, and a formal's default
 * appears only as the argument of an instance that omits it. */
static int code_assertion_unkept(int type, int relation) {
    switch (type) {
        case vpiPropertyDecl:
            return relation == vpiPropertySpec || relation == vpiVariables;
        case vpiSequenceDecl:
            return relation == vpiExpr || relation == vpiVariables;
        case vpiPropFormalDecl:
        case vpiSeqFormalDecl:
            return relation == vpiExpr;
        default:
            return 0;
    }
}

/* The objects of the 37.45 assertion class among expression and statement
 * objects: sequence and property instances and assertion statements. */
static int code_is_assertion(int type) {
    switch (type) {
        case vpiSequenceInst: case vpiPropertyInst: case vpiAssert: case vpiAssume:
        case vpiCover: case vpiImmediateAssert: case vpiImmediateAssume: case vpiImmediateCover:
            return 1;
        default:
            return 0;
    }
}

static vpiHandle code_relation(int type, uint32_t index) {
    const llg_vpi_code_t* node = code_at(index);
    if (!node) return NULL;
    if (type == vpiScope && code_is_constraint_object(node->type)) {
        /* 37.30 and 37.34 give constraint objects no scope. */
        vpi_error("LLG_VPI_RELATION", "constraint objects and value ranges have no vpiScope (37.30, 37.34)");
        return NULL;
    }
    if (type == vpiScope) return node->scope ? object_handle(node->scope - 1u) : NULL;
    if ((type == vpiInstance || type == vpiModule) && code_is_assertion(node->type)) {
        /* 37.10 details 2 and 3, 37.45: the instance and the module that
         * hold the assertion (decision A19-D31). */
        uint32_t row = node->scope ? (type == vpiInstance ? enclosing_instance(node->scope - 1u)
                                                          : enclosing_module(node->scope - 1u))
                                   : LLG_VPI_NONE;
        return row == LLG_VPI_NONE ? NULL : object_handle(row);
    }
    if (code_assertion_unkept(node->type, type)) {
        vpi_error("LLG_VPI_UNSUPPORTED",
                  "llg keeps no declaration body, variables or formal default of a sequence or "
                  "property: each instance holds its expanded body and its actual arguments");
        return NULL;
    }
    if (type == vpiTypespec &&
        (node->type == vpiPropFormalDecl || node->type == vpiSeqFormalDecl)) {
        /* 37.47 and 37.49 detail 2: NULL for an untyped formal. */
        return node->typespec ? typespec_handle(typespec_at(node->typespec - 1u)) : NULL;
    }
    if (type == vpiTypespec) {
        /* 37.55 detail 5: casts, simple expressions, selects and calls;
         * other operations and constants have none (decision A19-D32). */
        if (node->typespec) return typespec_handle(typespec_at(node->typespec - 1u));
        vpi_error("LLG_VPI_RELATION",
                  "llg gives casts, simple expressions, selects and calls a typespec, not other "
                  "operations or constants (decision A19-D32)");
        return NULL;
    }
    if (type == vpiUserSystf && (node->type == vpiSysFuncCall || node->type == vpiSysTaskCall)) {
        /* 37.38: the registration of a user-defined system task or function
         * a call names; NULL for a built-in one (decision A19-D32). */
        const char* name = code_text(node);
        llg_vpi_registration_t* registration =
            (node->flags & LLG_VPI_CODE_USER) && name ? find_registration(name) : NULL;
        return registration ? encode_handle(registration->slot) : NULL;
    }
    if (node->flags & LLG_VPI_CODE_OPAQUE) {
        code_opaque_error();
        return NULL;
    }
    /* A child kept for a one-to-many relation (an operand, an argument, a
     * block statement) is not a one-to-one relation of its owner. */
    if (!code_relation_applies(code_type(node), type)) {
        vpi_error("LLG_VPI_RELATION", "unsupported VPI one-to-one relation for this object");
        return NULL;
    }
    uint32_t child = code_child(node, type);
    if (child == LLG_VPI_NONE && (node->flags & LLG_VPI_CODE_USER) &&
        (type == vpiFunction || type == vpiTask) &&
        (node->type == vpiMethodFuncCall || node->type == vpiMethodTaskCall)) {
        /* 37.38 detail 11 gives NULL only for a built-in method. */
        vpi_error("LLG_VPI_UNSUPPORTED", "class methods are not VPI objects in llg");
        return NULL;
    }
    return child != LLG_VPI_NONE ? code_handle(child) : NULL;
}

/* Iterate the children of entry `index` reached by `relation`. */
static vpiHandle iterate_code(uint32_t index, int relation) {
    const llg_vpi_code_t* node = code_at(index);
    if (!node) return NULL;
    if (code_assertion_unkept(node->type, relation)) {
        vpi_error("LLG_VPI_UNSUPPORTED",
                  "llg keeps no variables of a sequence or property declaration");
        return NULL;
    }
    if (node->flags & LLG_VPI_CODE_OPAQUE) {
        code_opaque_error();
        return NULL;
    }
    if (!code_iteration_applies(code_type(node), relation)) {
        /* A block that is not a scope declares nothing (decision A19-D27). */
        if (!(node->flags & LLG_VPI_CODE_ALIAS) && code_is_unscoped_block(index) &&
            is_scope_iteration(relation))
            return NULL;
        vpi_error("LLG_VPI_RELATION", "unsupported VPI one-to-many relation for this object");
        return NULL;
    }
    if (code_child(node, relation) == LLG_VPI_NONE) return NULL;
    uint32_t slot_index = alloc_slot(SLOT_ITERATOR);
    if (slot_index == LLG_VPI_NONE) return NULL;
    llg_vpi_slot_t* iterator = &g_vpi.slots[slot_index];
    iterator->iter_kind = ITER_CODE;
    iterator->left = node->first;
    iterator->count = node->count;
    iterator->aux = relation;
    return encode_handle(slot_index);
}

static vpiHandle scan_code(uint32_t slot_index) {
    llg_vpi_slot_t* iterator = &g_vpi.slots[slot_index];
    while (iterator->position < iterator->count) {
        uint32_t child = (uint32_t)iterator->left + iterator->position++;
        if (g_vpi.code->nodes[child].role == iterator->aux) return code_handle(child);
    }
    release_slot(slot_index);
    return NULL;
}

/* The expression object behind a call argument handle, or LLG_VPI_NONE. */
static uint32_t argument_code(const llg_vpi_slot_t* slot) {
    if (slot->kind != SLOT_ARGUMENT || !g_vpi.code) return LLG_VPI_NONE;
    const llg_vpi_callsite_t* site = (const llg_vpi_callsite_t*)slot->pointer;
    if (slot->aux < 0 || slot->aux >= site->arg_count) return LLG_VPI_NONE;
    uint32_t code = site->args[slot->aux].code;
    return code && code - 1u < g_vpi.code->node_count ? code_visible(code - 1u) : LLG_VPI_NONE;
}

/* ── expression values ── */

static int code_eval(uint32_t index, llg_vpi_datum_t* out, int depth);

static void datum_set_packed(llg_vpi_datum_t* out, sv4_t value) {
    sv4_replace(&out->packed, value);
    out->is_real = 0;
}

/* Truth of a datum: 1 true, 0 false, -1 unknown (11.4.7). */
static int datum_truth(const llg_vpi_datum_t* datum) {
    if (datum->is_real) return datum->real != 0.0;
    sv4_t any = sv4_reduce_or(datum->packed);
    int truth = sv4_is_unknown(any) ? -1 : sv4_to_bool(any);
    sv4_destroy(&any);
    return truth;
}

static double datum_real(const llg_vpi_datum_t* datum) {
    return datum->is_real ? datum->real : sv4_to_real(datum->packed);
}

/* Bits [lsb, lsb + width) of `value`; bits outside it read as x (11.5.1). */
static sv4_t select_bits(sv4_t value, int64_t lsb, uint32_t width) {
    uint32_t have = llg_sv4_width(value);
    if (lsb >= 0 && (uint64_t)lsb + width <= have)
        return sv4_part_select(value, lsb + (int64_t)width - 1, lsb);
    sv4_t result = sv4_x(width, 0);
    for (uint32_t i = 0; i < width; ++i) {
        int64_t bit = lsb + (int64_t)i;
        if (bit < 0 || (uint64_t)bit >= have) continue;
        sv4_t one = sv4_bit_select(value, (uint64_t)bit);
        sv4_bit_select_set(&result, i, one);
        sv4_destroy(&one);
    }
    return result;
}

static int code_index(uint32_t index, int64_t* value, int depth) {
    llg_vpi_datum_t datum;
    memset(&datum, 0, sizeof(datum));
    datum.packed = VPI_EMPTY;
    int known = code_eval(index, &datum, depth) && !datum.is_real &&
                sv4_to_index_i64(datum.packed, value);
    datum_destroy(&datum);
    return known;
}

/* A bit, part or indexed part select of a packed value (11.5.1). */
static int eval_select(const llg_vpi_code_t* node, llg_vpi_datum_t* out, int depth) {
    int64_t left, right;
    uint32_t parent = code_child(node, vpiParent);
    if (parent == LLG_VPI_NONE || !code_range(node, &left, &right)) return 0;
    llg_vpi_datum_t base;
    memset(&base, 0, sizeof(base));
    base.packed = VPI_EMPTY;
    if (!code_eval(parent, &base, depth) || base.is_real) {
        datum_destroy(&base);
        return 0;
    }
    uint64_t extent = (uint64_t)(left >= right ? left - right : right - left) + 1u;
    uint64_t stride = llg_sv4_width(base.packed) / extent;
    if (!stride) stride = 1;
    int64_t low = 0, high = 0;
    int known = 1;
    uint32_t width = node->width ? node->width : 1u;
    if (node->type == vpiBitSelect) {
        known = code_index(code_child(node, vpiIndex), &low, depth);
        high = low;
    } else if (node->type == vpiPartSelect) {
        known = code_index(code_child(node, vpiLeftRange), &high, depth) &&
                code_index(code_child(node, vpiRightRange), &low, depth);
    } else {
        int64_t start = 0, count = 0;
        known = code_index(code_child(node, vpiBaseExpr), &start, depth) &&
                code_index(code_child(node, vpiWidthExpr), &count, depth) && count > 0;
        low = start;
        high = node->subtype == vpiNegIndexed ? start - count + 1 : start + count - 1;
    }
    /* Offsets count elements from the `right` end of the dimension, so
     * ascending and descending ranges map alike; bits outside the value
     * read as x. */
    int64_t from_low = left >= right ? low - right : right - low;
    int64_t from_high = left >= right ? high - right : right - high;
    int64_t from = from_low < from_high ? from_low : from_high;
    int64_t limit = (int64_t)1 << 40;
    if (from <= -limit || from >= limit) known = 0;
    int64_t lsb = known ? from * (int64_t)stride : 0;
    sv4_t value = known ? select_bits(base.packed, lsb, width) : sv4_x(width, 0);
    datum_destroy(&base);
    datum_set_packed(out, value);
    return 1;
}

/* An element of a catalog array, by its indices (37.19). */
static int eval_var_select(const llg_vpi_code_t* node, llg_vpi_datum_t* out, int depth) {
    uint32_t parent = code_child(node, vpiParent);
    const llg_vpi_code_t* array = parent == LLG_VPI_NONE ? NULL : code_at(parent);
    if (!array || !(array->flags & LLG_VPI_CODE_ALIAS) || array->object < 0) return 0;
    llg_vpi_model_object_t* object = row_object((uint32_t)array->object);
    if (!object || !is_array_type(object->type)) return 0;
    uint64_t element = 0;
    uint32_t dim = 0;
    int in_range = 1;
    for (uint32_t i = 0; i < node->count; ++i) {
        uint32_t child = node->first + i;
        if (g_vpi.code->nodes[child].role != vpiIndex) continue;
        int64_t left, right, value;
        /* Set even when `code_index` fails and `index_offset` is skipped:
         * MSVC (C4701) and Clang cannot see that `in_range` then guards it. */
        uint64_t offset = 0;
        if (!array_dim(object, dim++, &left, &right)) return 0;
        uint64_t extent = (uint64_t)(left >= right ? left - right : right - left) + 1u;
        if (!code_index(child, &value, depth) || !index_offset(left, right, value, &offset))
            in_range = 0;
        element = element * extent + (in_range ? offset : 0u);
    }
    if (dim != object->dim_count) return 0;
    if (!in_range || element >= g_vpi.element_counts[array->object]) {
        /* An index outside the array or with x/z bits reads the default. */
        if (object->is_real) {
            out->is_real = 1;
            out->real = 0.0;
        } else {
            datum_set_packed(out, sv4_x(object->width ? object->width : 1u, 0));
        }
        return 1;
    }
    llg_vpi_slot_t view;
    memset(&view, 0, sizeof(view));
    view.kind = SLOT_ELEMENT;
    view.object = (uint32_t)array->object;
    view.element = element;
    int is_signed = 0;
    datum_destroy(out);
    return read_handle(&view, out, &is_signed);
}

/* The operands of an operation, evaluated; returns how many (at most
 * `capacity`), or -1 when one has no value. */
static int eval_operands(const llg_vpi_code_t* node, llg_vpi_datum_t* operands, int capacity,
                         int depth) {
    if (node->count > (uint32_t)capacity) return -1;
    int count = 0;
    for (uint32_t i = 0; i < node->count && count < capacity; ++i) {
        memset(&operands[count], 0, sizeof(operands[count]));
        operands[count].packed = VPI_EMPTY;
        if (!code_eval(node->first + i, &operands[count], depth)) {
            for (int j = 0; j <= count; ++j) datum_destroy(&operands[j]);
            return -1;
        }
        ++count;
    }
    return count;
}

static int eval_real_operation(int op, llg_vpi_datum_t* operands, int count,
                               llg_vpi_datum_t* out) {
    double a = count > 0 ? datum_real(&operands[0]) : 0.0;
    double b = count > 1 ? datum_real(&operands[1]) : 0.0;
    out->is_real = 1;
    switch (op) {
        case vpiMinusOp: out->real = -a; return count == 1;
        case vpiPlusOp: out->real = a; return count == 1;
        case vpiAddOp: out->real = a + b; return count == 2;
        case vpiSubOp: out->real = a - b; return count == 2;
        case vpiMultOp: out->real = a * b; return count == 2;
        case vpiDivOp: out->real = a / b; return count == 2;
        case vpiPowerOp: out->real = pow(a, b); return count == 2;
        default: return 0;
    }
}

/* Integral and comparison operations (11.4). Operand types are already the
 * frontend's: implicit conversions are entries of their own. */
static int eval_operation(const llg_vpi_code_t* node, llg_vpi_datum_t* out, int depth) {
    llg_vpi_datum_t operands[3];
    for (int i = 0; i < 3; ++i) {
        memset(&operands[i], 0, sizeof(operands[i]));
        operands[i].packed = VPI_EMPTY;
    }
    int op = node->subtype;
    if (op == vpiConcatOp || op == vpiMultiConcatOp) {
        /* Self-determined operands, joined most significant first. */
        uint32_t first = node->first, count = node->count;
        uint64_t repeat = 1;
        if (op == vpiMultiConcatOp) {
            int64_t times;
            if (!count || !code_index(first, &times, depth) || times < 0) return 0;
            repeat = (uint64_t)times;
            ++first;
            --count;
        }
        sv4_t joined = VPI_EMPTY;
        for (uint32_t i = 0; i < count; ++i) {
            llg_vpi_datum_t part;
            memset(&part, 0, sizeof(part));
            part.packed = VPI_EMPTY;
            if (!code_eval(first + i, &part, depth) || part.is_real) {
                datum_destroy(&part);
                sv4_destroy(&joined);
                return 0;
            }
            sv4_t next = i ? sv4_concat(joined, part.packed) : sv4_clone(&part.packed);
            sv4_replace(&joined, next);
            datum_destroy(&part);
        }
        if (op == vpiMultiConcatOp) sv4_replace(&joined, sv4_repeat(joined, repeat));
        datum_set_packed(out, joined);
        return 1;
    }
    if (op == vpiConditionOp) {
        if (node->count != 3 || !code_eval(node->first, &operands[0], depth)) return 0;
        int truth = datum_truth(&operands[0]);
        datum_destroy(&operands[0]);
        if (truth >= 0) return code_eval(node->first + (truth ? 1u : 2u), out, depth);
        if (node->flags & LLG_VPI_CODE_REAL) return 0;
        /* An unknown condition merges the arms bit by bit (11.4.11). */
        if (eval_operands(node, operands, 3, depth) != 3) return 0;
        sv4_t select = sv4_x(1, 0);
        datum_set_packed(out, sv4_mux(select, operands[1].packed, operands[2].packed));
        sv4_destroy(&select);
        for (int i = 0; i < 3; ++i) datum_destroy(&operands[i]);
        return 1;
    }
    int count = eval_operands(node, operands, 2, depth);
    if (count < 0) return 0;
    int ok = 1;
    int real = count > 0 && (operands[0].is_real || (count > 1 && operands[1].is_real));
    if (node->flags & LLG_VPI_CODE_REAL) {
        ok = eval_real_operation(op, operands, count, out);
    } else if (real) {
        /* Comparisons and logical operators of real operands. */
        double a = datum_real(&operands[0]), b = count > 1 ? datum_real(&operands[1]) : 0.0;
        int result;
        switch (op) {
            case vpiEqOp: result = a == b; break;
            case vpiNeqOp: result = a != b; break;
            case vpiGtOp: result = a > b; break;
            case vpiGeOp: result = a >= b; break;
            case vpiLtOp: result = a < b; break;
            case vpiLeOp: result = a <= b; break;
            case vpiNotOp: result = a == 0.0; break;
            case vpiLogAndOp: result = a != 0.0 && b != 0.0; break;
            case vpiLogOrOp: result = a != 0.0 || b != 0.0; break;
            default: ok = 0; result = 0; break;
        }
        if (ok) datum_set_packed(out, sv4_from_u64((uint64_t)result, 1, 0));
    } else if (count == 1) {
        sv4_t a = operands[0].packed;
        switch (op) {
            case vpiMinusOp: datum_set_packed(out, sv4_neg(a)); break;
            case vpiPlusOp: datum_set_packed(out, sv4_clone(&a)); break;
            case vpiNotOp: datum_set_packed(out, sv4_lognot(a)); break;
            case vpiBitNegOp: datum_set_packed(out, sv4_bitneg(a)); break;
            case vpiUnaryAndOp: datum_set_packed(out, sv4_reduce_and(a)); break;
            case vpiUnaryNandOp: datum_set_packed(out, sv4_reduce_nand(a)); break;
            case vpiUnaryOrOp: datum_set_packed(out, sv4_reduce_or(a)); break;
            case vpiUnaryNorOp: datum_set_packed(out, sv4_reduce_nor(a)); break;
            case vpiUnaryXorOp: datum_set_packed(out, sv4_reduce_xor(a)); break;
            case vpiUnaryXNorOp: datum_set_packed(out, sv4_reduce_xnor(a)); break;
            default: ok = 0; break;
        }
    } else if (count == 2) {
        sv4_t a = operands[0].packed, b = operands[1].packed;
        switch (op) {
            case vpiAddOp: datum_set_packed(out, sv4_add(a, b)); break;
            case vpiSubOp: datum_set_packed(out, sv4_sub(a, b)); break;
            case vpiMultOp: datum_set_packed(out, sv4_mul(a, b)); break;
            case vpiDivOp: datum_set_packed(out, sv4_div(a, b)); break;
            case vpiModOp: datum_set_packed(out, sv4_mod(a, b)); break;
            case vpiPowerOp: datum_set_packed(out, sv4_pow(a, b)); break;
            case vpiEqOp: datum_set_packed(out, sv4_eq(a, b)); break;
            case vpiNeqOp: datum_set_packed(out, sv4_neq(a, b)); break;
            case vpiCaseEqOp: datum_set_packed(out, sv4_case_eq(a, b)); break;
            case vpiCaseNeqOp: datum_set_packed(out, sv4_case_neq(a, b)); break;
            case vpiWildEqOp: datum_set_packed(out, sv4_wild_eq(a, b)); break;
            case vpiWildNeqOp: datum_set_packed(out, sv4_wild_neq(a, b)); break;
            case vpiGtOp: datum_set_packed(out, sv4_gt(a, b)); break;
            case vpiGeOp: datum_set_packed(out, sv4_ge(a, b)); break;
            case vpiLtOp: datum_set_packed(out, sv4_lt(a, b)); break;
            case vpiLeOp: datum_set_packed(out, sv4_le(a, b)); break;
            case vpiLShiftOp: datum_set_packed(out, sv4_shl(a, b)); break;
            case vpiRShiftOp: datum_set_packed(out, sv4_shr(a, b)); break;
            case vpiArithLShiftOp: datum_set_packed(out, sv4_ashl(a, b)); break;
            case vpiArithRShiftOp: datum_set_packed(out, sv4_ashr(a, b)); break;
            case vpiLogAndOp: datum_set_packed(out, sv4_logand(a, b)); break;
            case vpiLogOrOp: datum_set_packed(out, sv4_logor(a, b)); break;
            case vpiImplyOp: datum_set_packed(out, sv4_logimpl(a, b)); break;
            case vpiBitAndOp: datum_set_packed(out, sv4_and(a, b)); break;
            case vpiBitOrOp: datum_set_packed(out, sv4_or(a, b)); break;
            case vpiBitXorOp: datum_set_packed(out, sv4_xor(a, b)); break;
            case vpiBitXNorOp: datum_set_packed(out, sv4_xnor(a, b)); break;
            default: ok = 0; break;
        }
    } else {
        ok = 0;
    }
    for (int i = 0; i < count; ++i) datum_destroy(&operands[i]);
    return ok;
}

/* A conversion, implicit or a cast: the operand's value as the entry's
 * type (6.24). */
static int eval_conversion(const llg_vpi_code_t* node, llg_vpi_datum_t* out, int depth) {
    if (node->count != 1) return 0;
    llg_vpi_datum_t operand;
    memset(&operand, 0, sizeof(operand));
    operand.packed = VPI_EMPTY;
    if (!code_eval(node->first, &operand, depth)) {
        datum_destroy(&operand);
        return 0;
    }
    int8_t is_signed = (node->flags & LLG_VPI_CODE_SIGNED) != 0;
    int ok = 1;
    if (node->flags & LLG_VPI_CODE_REAL) {
        out->real = datum_real(&operand);
        out->is_real = 1;
    } else if (!node->width) {
        ok = 0;
    } else if (operand.is_real) {
        datum_set_packed(out, sv4_from_real(operand.real, node->width, is_signed));
    } else {
        datum_set_packed(out, sv4_cast(operand.packed, node->width, is_signed));
    }
    if (ok && !out->is_real && (node->flags & LLG_VPI_CODE_TWO_STATE))
        sv4_replace(&out->packed, sv4_to_two_state(out->packed));
    datum_destroy(&operand);
    return ok;
}

/* Evaluate entry `index` into `out` (an initialized datum): its current
 * value as the frontend typed it. Returns 0 when it has none. */
static int code_eval(uint32_t index, llg_vpi_datum_t* out, int depth) {
    const llg_vpi_code_t* node = code_at(index);
    if (!node || depth > LLG_VPI_MAX_EVAL_DEPTH || (node->flags & LLG_VPI_CODE_OPAQUE)) return 0;
    ++depth;
    int ok;
    if (node->flags & LLG_VPI_CODE_ALIAS) {
        if (node->object < 0) return 0;
        llg_vpi_slot_t view;
        memset(&view, 0, sizeof(view));
        view.kind = SLOT_OBJECT;
        view.object = (uint32_t)node->object;
        view.element = LLG_VPI_NO_ELEMENT;
        int is_signed = 0;
        datum_destroy(out);
        return read_handle(&view, out, &is_signed);
    }
    switch (node->type) {
        case vpiConstant: {
            const char* text = code_text(node);
            if (!text || node->subtype == vpiNullConst || node->subtype == vpiUnboundedConst)
                return 0;
            if (node->flags & LLG_VPI_CODE_REAL) {
                out->is_real = 1;
                out->real = strtod(text, NULL);
                return 1;
            }
            datum_set_packed(out, digits_value(text, node->width,
                                               (node->flags & LLG_VPI_CODE_SIGNED) != 0,
                                               (node->flags & LLG_VPI_CODE_STRING) != 0));
            return 1;
        }
        case vpiOperation:
            ok = node->subtype == vpiCastOp ? eval_conversion(node, out, depth)
                                            : eval_operation(node, out, depth);
            break;
        case vpiBitSelect:
        case vpiPartSelect:
        case vpiIndexedPartSelect:
            ok = eval_select(node, out, depth);
            break;
        case vpiVarSelect:
            return eval_var_select(node, out, depth);
        default:
            return 0;
    }
    /* Results carry the entry's own size and sign. */
    if (ok && !out->is_real && node->width) {
        int8_t is_signed = (node->flags & LLG_VPI_CODE_SIGNED) != 0;
        if (llg_sv4_width(out->packed) != node->width)
            sv4_replace(&out->packed, sv4_resize(out->packed, node->width, is_signed));
        llg_sv4_set_signed(&out->packed, is_signed);
    }
    return ok;
}

static int code_value(uint32_t index, llg_vpi_datum_t* out, int* is_signed) {
    memset(out, 0, sizeof(*out));
    out->packed = VPI_EMPTY;
    *is_signed = 0;
    if (!code_eval(index, out, 0)) return 0;
    *is_signed = !out->is_real && llg_sv4_signed(out->packed);
    return 1;
}

/* ── decompilation (37.55 detail 2) ── */

typedef struct {
    char* text;
    size_t length;
    size_t capacity;
    int failed;
} llg_vpi_text_t;

static void text_append(llg_vpi_text_t* text, const char* part, size_t length) {
    if (text->failed) return;
    if (text->length + length + 1u > text->capacity) {
        size_t capacity = text->capacity ? text->capacity : 64u;
        while (capacity < text->length + length + 1u) capacity *= 2u;
        char* grown = (char*)realloc(text->text, capacity);
        if (!grown) {
            text->failed = 1;
            return;
        }
        text->text = grown;
        text->capacity = capacity;
    }
    memcpy(text->text + text->length, part, length);
    text->length += length;
    text->text[text->length] = '\0';
}

static void text_put(llg_vpi_text_t* text, const char* part) {
    text_append(text, part, strlen(part));
}

/* Operator spelling and precedence (11.3.2, Table 11-2): larger binds
 * tighter; 0 for an operation printed without an infix operator. */
static const char* operator_text(int op, int unary, int* precedence) {
    if (unary) {
        *precedence = 14;
        switch (op) {
            case vpiMinusOp: return "-";
            case vpiPlusOp: return "+";
            case vpiNotOp: return "!";
            case vpiBitNegOp: return "~";
            case vpiUnaryAndOp: return "&";
            case vpiUnaryNandOp: return "~&";
            case vpiUnaryOrOp: return "|";
            case vpiUnaryNorOp: return "~|";
            case vpiUnaryXorOp: return "^";
            case vpiUnaryXNorOp: return "~^";
            case vpiPreIncOp: return "++";
            case vpiPreDecOp: return "--";
            case vpiPosedgeOp: *precedence = 1; return "posedge";
            case vpiNegedgeOp: *precedence = 1; return "negedge";
            default: break;
        }
    }
    switch (op) {
        case vpiPowerOp: *precedence = 13; return "**";
        case vpiMultOp: *precedence = 12; return "*";
        case vpiDivOp: *precedence = 12; return "/";
        case vpiModOp: *precedence = 12; return "%";
        case vpiAddOp: *precedence = 11; return "+";
        case vpiSubOp: *precedence = 11; return "-";
        case vpiLShiftOp: *precedence = 10; return "<<";
        case vpiRShiftOp: *precedence = 10; return ">>";
        case vpiArithLShiftOp: *precedence = 10; return "<<<";
        case vpiArithRShiftOp: *precedence = 10; return ">>>";
        case vpiLtOp: *precedence = 9; return "<";
        case vpiLeOp: *precedence = 9; return "<=";
        case vpiGtOp: *precedence = 9; return ">";
        case vpiGeOp: *precedence = 9; return ">=";
        case vpiInsideOp: *precedence = 9; return "inside";
        case vpiEqOp: *precedence = 8; return "==";
        case vpiNeqOp: *precedence = 8; return "!=";
        case vpiCaseEqOp: *precedence = 8; return "===";
        case vpiCaseNeqOp: *precedence = 8; return "!==";
        case vpiWildEqOp: *precedence = 8; return "==?";
        case vpiWildNeqOp: *precedence = 8; return "!=?";
        case vpiBitAndOp: *precedence = 7; return "&";
        case vpiBitXorOp: *precedence = 6; return "^";
        case vpiBitXNorOp: *precedence = 6; return "~^";
        case vpiBitOrOp: *precedence = 5; return "|";
        case vpiLogAndOp: *precedence = 4; return "&&";
        case vpiLogOrOp: *precedence = 3; return "||";
        case vpiConditionOp: *precedence = 2; return "?";
        case vpiImplyOp: *precedence = 1; return "->";
        case vpiEventOrOp: *precedence = 1; return "or";
        case vpiIffOp: *precedence = 1; return "iff";
        default: *precedence = 15; return NULL;
    }
}

/* The precedence an expression prints with (15: a primary). */
static int code_precedence(uint32_t index) {
    const llg_vpi_code_t* node = code_at(code_visible(index));
    if (!node || node->type != vpiOperation || (node->flags & LLG_VPI_CODE_OPAQUE)) return 15;
    int precedence = 15;
    switch (node->subtype) {
        case vpiPostIncOp: case vpiPostDecOp: case vpiConcatOp: case vpiMultiConcatOp:
        case vpiCastOp: case vpiAssignmentPatternOp: case vpiMultiAssignmentPatternOp:
        case vpiNullOp: case vpiMinTypMaxOp: case vpiListOp:
            return 15;
        default:
            break;
    }
    operator_text(node->subtype, node->count == 1, &precedence);
    return precedence;
}

static void decompile(llg_vpi_text_t* text, uint32_t index, int depth);

/* Operand `index` at a position that binds at least `minimum`. */
static void decompile_operand(llg_vpi_text_t* text, uint32_t index, int minimum, int depth) {
    int parenthesize = code_precedence(index) < minimum;
    if (parenthesize) text_put(text, "(");
    decompile(text, index, depth);
    if (parenthesize) text_put(text, ")");
}

/* The children reached by `relation`, separated by ", ". */
static void decompile_list(llg_vpi_text_t* text, const llg_vpi_code_t* node, int relation,
                           uint32_t skip, int depth) {
    int first = 1;
    for (uint32_t i = skip; i < node->count; ++i) {
        if (g_vpi.code->nodes[node->first + i].role != relation) continue;
        if (!first) text_put(text, ", ");
        first = 0;
        decompile(text, node->first + i, depth);
    }
}

/* A constant as a literal of its vpiConstType: unsized signed 32-bit
 * decimals and unbased unsized literals as written, other integers sized in their radix (binary when
 * a hexadecimal or octal digit would mix known and unknown bits). */
static void decompile_constant(llg_vpi_text_t* text, const llg_vpi_code_t* node) {
    const char* digits = code_text(node);
    char buffer[64];
    if (node->subtype == vpiNullConst) {
        text_put(text, "null");
        return;
    }
    if (!digits) return;
    if ((node->flags & LLG_VPI_CODE_FILL) && digits[0]) {
        /* Every digit is the fill; the literal prints as written, with a
         * lower-case x or z. */
        buffer[0] = '\'';
        buffer[1] = (char)(digits[0] == 'X' ? 'x' : digits[0] == 'Z' ? 'z' : digits[0]);
        buffer[2] = '\0';
        text_put(text, buffer);
        return;
    }
    if (node->flags & LLG_VPI_CODE_STRING) {
        text_put(text, "\"");
        for (const char* c = digits; *c; ++c) {
            if (*c == '"' || *c == '\\') text_put(text, "\\");
            if (*c == '\n') {
                text_put(text, "\\n");
                continue;
            }
            text_append(text, c, 1);
        }
        text_put(text, "\"");
        return;
    }
    if ((node->flags & (LLG_VPI_CODE_REAL | LLG_VPI_CODE_OPAQUE)) ||
        node->subtype == vpiUnboundedConst) {
        text_put(text, digits);
        return;
    }
    size_t width = strlen(digits);
    int is_signed = (node->flags & LLG_VPI_CODE_SIGNED) != 0;
    int known = strspn(digits, "01") == width;
    int radix = node->subtype == vpiHexConst ? 16 : node->subtype == vpiOctConst ? 8
                : node->subtype == vpiDecConst || node->subtype == vpiIntConst ? 10 : 2;
    if (radix == 10 && known && width <= 64) {
        uint64_t value = 0;
        for (size_t i = 0; i < width; ++i) value = value << 1 | (uint64_t)(digits[i] == '1');
        if (width == 32 && is_signed) {
            snprintf(buffer, sizeof(buffer), "%lld", (long long)(int32_t)(uint32_t)value);
            text_put(text, buffer);
            return;
        }
        snprintf(buffer, sizeof(buffer), "%zu'%sd%llu", width, is_signed ? "s" : "",
                 (unsigned long long)value);
        text_put(text, buffer);
        return;
    }
    if (radix == 10) radix = 2;
    unsigned bits = radix == 16 ? 4u : radix == 8 ? 3u : 1u;
    /* Groups of digits must be all known or all one unknown state. */
    for (size_t end = width; bits > 1 && end > 0;) {
        size_t start = end > bits ? end - bits : 0;
        size_t known_bits = 0, x = 0, z = 0;
        for (size_t i = start; i < end; ++i) {
            known_bits += digits[i] == '0' || digits[i] == '1';
            x += digits[i] == 'x';
            z += digits[i] == 'z';
        }
        if (known_bits != end - start && x != end - start && z != end - start) bits = 1;
        end = start;
    }
    snprintf(buffer, sizeof(buffer), "%zu'%s%c", width, is_signed ? "s" : "",
             bits == 4 ? 'h' : bits == 3 ? 'o' : 'b');
    text_put(text, buffer);
    size_t groups = (width + bits - 1u) / bits;
    for (size_t group = groups; group-- > 0;) {
        size_t end = width - group * bits;
        size_t start = end > bits ? end - bits : 0;
        unsigned value = 0;
        char digit = 0;
        for (size_t i = start; i < end; ++i) {
            if (digits[i] == 'x' || digits[i] == 'z') digit = digits[i];
            value = value << 1 | (unsigned)(digits[i] == '1');
        }
        if (!digit) digit = "0123456789abcdef"[value];
        text_append(text, &digit, 1);
    }
}

/* The keyword of a built-in casting type (6.24.1), or NULL. */
static const char* cast_keyword(int typespec_type) {
    switch (typespec_type) {
        case vpiIntTypespec: return "int";
        case vpiShortIntTypespec: return "shortint";
        case vpiLongIntTypespec: return "longint";
        case vpiByteTypespec: return "byte";
        case vpiIntegerTypespec: return "integer";
        case vpiTimeTypespec: return "time";
        case vpiRealTypespec: return "real";
        case vpiShortRealTypespec: return "shortreal";
        case vpiStringTypespec: return "string";
        default: return NULL;
    }
}

static void decompile_operation(llg_vpi_text_t* text, const llg_vpi_code_t* node, int depth) {
    int op = node->subtype, precedence = 15;
    uint32_t first = node->first;
    switch (op) {
        case vpiConcatOp:
            text_put(text, "{");
            decompile_list(text, node, vpiOperand, 0, depth);
            text_put(text, "}");
            return;
        case vpiMultiConcatOp:
            text_put(text, "{");
            if (node->count) decompile(text, first, depth);
            text_put(text, "{");
            decompile_list(text, node, vpiOperand, 1, depth);
            text_put(text, "}}");
            return;
        case vpiAssignmentPatternOp:
            text_put(text, "'{");
            decompile_list(text, node, vpiOperand, 0, depth);
            text_put(text, "}");
            return;
        case vpiMultiAssignmentPatternOp:
            text_put(text, "'{");
            if (node->count) decompile(text, first, depth);
            text_put(text, "{");
            decompile_list(text, node, vpiOperand, 1, depth);
            text_put(text, "}}");
            return;
        case vpiCastOp: {
            const char* size = code_text(node);
            const llg_vpi_typespec_t* typespec =
                node->typespec ? typespec_at(node->typespec - 1u) : NULL;
            const char* keyword = typespec ? cast_keyword(typespec->type) : NULL;
            if (size) {
                text_put(text, size);
            } else if (typespec && typespec->name) {
                text_put(text, typespec->name);
            } else if (keyword) {
                text_put(text, keyword);
            } else if (node->count && !(node->flags & LLG_VPI_CODE_REAL)) {
                /* An unnamed vector target: a signing cast keeps the
                 * operand's size and a size cast its signedness (6.24.1). */
                const llg_vpi_code_t* argument = code_at(code_visible(first));
                int is_signed = (node->flags & LLG_VPI_CODE_SIGNED) != 0;
                int sign_changes = !(argument->flags & LLG_VPI_CODE_REAL) &&
                                   is_signed != ((argument->flags & LLG_VPI_CODE_SIGNED) != 0);
                int size_changes = (argument->flags & LLG_VPI_CODE_REAL) ||
                                   argument->width != node->width || !sign_changes;
                char buffer[48];
                if (sign_changes && size_changes) {
                    snprintf(buffer, sizeof(buffer), "%s'(%u", is_signed ? "signed" : "unsigned",
                             node->width);
                    text_put(text, buffer);
                    text_put(text, "'(");
                    decompile(text, first, depth);
                    text_put(text, "))");
                    return;
                }
                if (size_changes) {
                    snprintf(buffer, sizeof(buffer), "%u", node->width);
                    text_put(text, buffer);
                } else {
                    text_put(text, is_signed ? "signed" : "unsigned");
                }
            } else {
                text->failed = 1;
                return;
            }
            text_put(text, "'(");
            if (node->count) decompile(text, first, depth);
            text_put(text, ")");
            return;
        }
        case vpiPostIncOp:
        case vpiPostDecOp:
            if (node->count) decompile_operand(text, first, 15, depth);
            text_put(text, op == vpiPostIncOp ? "++" : "--");
            return;
        case vpiMinTypMaxOp:
            for (uint32_t i = 0; i < node->count; ++i) {
                if (i) text_put(text, ":");
                decompile(text, first + i, depth);
            }
            return;
        case vpiNullOp:
            return;
        case vpiListOp:
            decompile_list(text, node, vpiOperand, 0, depth);
            return;
        case vpiInsideOp:
            /* 11.4.13: `expression inside { open_range_list }`. */
            if (!node->count) return;
            decompile_operand(text, first, 9, depth);
            text_put(text, " inside {");
            decompile_list(text, node, vpiOperand, 1, depth);
            text_put(text, "}");
            return;
        case vpiConditionOp:
            if (node->count != 3) return;
            /* Right associative: only the condition needs a tighter binding. */
            decompile_operand(text, first, 3, depth);
            text_put(text, " ? ");
            decompile_operand(text, first + 1u, 2, depth);
            text_put(text, " : ");
            decompile_operand(text, first + 2u, 2, depth);
            return;
        default:
            break;
    }
    const char* spelling = operator_text(op, node->count == 1, &precedence);
    if (!spelling) return;
    if (node->count == 1) {
        text_put(text, spelling);
        text_put(text, " ");
        decompile_operand(text, first, precedence, depth);
        return;
    }
    /* Binary operators associate left to right except implication; an
     * equal-precedence operand on the other side keeps its parentheses.
     * An event or list is flat: its edges need none. */
    int right_associative = op == vpiImplyOp;
    for (uint32_t i = 0; i < node->count; ++i) {
        if (i) {
            text_put(text, " ");
            text_put(text, spelling);
            text_put(text, " ");
        }
        int tighter = op != vpiEventOrOp && (i == 0) == right_associative;
        decompile_operand(text, first + i, precedence + tighter, depth);
    }
}

static void decompile(llg_vpi_text_t* text, uint32_t index, int depth) {
    index = code_visible(index);
    const llg_vpi_code_t* node = code_at(index);
    if (!node || depth > LLG_VPI_MAX_EVAL_DEPTH) {
        text->failed = 1;
        return;
    }
    ++depth;
    const char* name = code_text(node);
    if (node->flags & LLG_VPI_CODE_ALIAS) {
        if (name) {
            text_put(text, name);
        } else if (node->object >= 0) {
            text_put(text, g_vpi.objects[node->object].name);
        }
        return;
    }
    switch (node->type) {
        case vpiConstant:
            decompile_constant(text, node);
            return;
        case vpiOperation:
            if (node->flags & LLG_VPI_CODE_OPAQUE) {
                text->failed = 1;
                return;
            }
            decompile_operation(text, node, depth);
            return;
        case vpiBitSelect:
        case vpiPartSelect:
        case vpiIndexedPartSelect:
        case vpiVarSelect: {
            uint32_t parent = code_child(node, vpiParent);
            if (parent == LLG_VPI_NONE) {
                text->failed = 1;
                return;
            }
            decompile_operand(text, parent, 15, depth);
            text_put(text, "[");
            if (node->type == vpiPartSelect) {
                decompile(text, code_child(node, vpiLeftRange), depth);
                text_put(text, ":");
                decompile(text, code_child(node, vpiRightRange), depth);
            } else if (node->type == vpiIndexedPartSelect) {
                decompile(text, code_child(node, vpiBaseExpr), depth);
                text_put(text, node->subtype == vpiNegIndexed ? " -: " : " +: ");
                decompile(text, code_child(node, vpiWidthExpr), depth);
            } else {
                int first = 1;
                for (uint32_t i = 0; i < node->count; ++i) {
                    if (g_vpi.code->nodes[node->first + i].role != vpiIndex) continue;
                    if (!first) text_put(text, "][");
                    first = 0;
                    decompile(text, node->first + i, depth);
                }
            }
            text_put(text, "]");
            return;
        }
        case vpiRange:
            /* A value range of a dist item or an inside operator. */
            text_put(text, "[");
            decompile(text, code_child(node, vpiLeftRange), depth);
            text_put(text, ":");
            decompile(text, code_child(node, vpiRightRange), depth);
            text_put(text, "]");
            return;
        case vpiFuncCall:
        case vpiTaskCall:
        case vpiSysFuncCall:
        case vpiSysTaskCall:
            if (name) text_put(text, name);
            text_put(text, "(");
            decompile_list(text, node, vpiArgument, 0, depth);
            text_put(text, ")");
            return;
        case vpiMethodFuncCall:
        case vpiMethodTaskCall: {
            /* `prefix.name(arguments)`, then the `with` expression. */
            uint32_t prefix = code_child(node, vpiPrefix);
            if (prefix != LLG_VPI_NONE) {
                decompile_operand(text, prefix, 15, depth);
                text_put(text, ".");
            }
            if (name) text_put(text, name);
            text_put(text, "(");
            decompile_list(text, node, vpiArgument, 0, depth);
            text_put(text, ")");
            uint32_t with = code_child(node, vpiWith);
            if (with != LLG_VPI_NONE) {
                text_put(text, " with (");
                decompile(text, with, depth);
                text_put(text, ")");
            }
            return;
        }
        default:
            if (node->flags & LLG_VPI_CODE_OPAQUE && name && !code_is_statement(node->type)) {
                /* A reference to an object outside the catalog. */
                text_put(text, name);
                return;
            }
            text->failed = 1;
            return;
    }
}

static char* code_decompile(uint32_t index) {
    llg_vpi_text_t text = {g_vpi.decompiled, 0, g_vpi.decompiled_capacity, 0};
    text_put(&text, "");
    decompile(&text, index, 0);
    g_vpi.decompiled = text.text;
    g_vpi.decompiled_capacity = text.capacity;
    if (text.failed) {
        vpi_error("LLG_VPI_UNSUPPORTED", "llg cannot decompile this expression");
        return NULL;
    }
    return text.text;
}

/* ── properties ── */

static PLI_INT32 code_property(uint32_t index, int property) {
    const llg_vpi_code_t* node = code_at(index);
    int type = code_type(node);
    int expression = code_is_expression(node);
    switch (property) {
        case vpiType: return type;
        case vpiLineNo: return (PLI_INT32)node->line;
        case vpiStartLine:
        case vpiColumn:
        case vpiEndLine:
        case vpiEndColumn: {
            /* 37.45: the location of an assertion object (A19-D31). */
            if (!node->span) break;
            const uint32_t* span = &g_vpi.code->spans[3u * (node->span - 1u)];
            uint32_t value = property == vpiStartLine ? node->line
                             : property == vpiColumn  ? span[0]
                             : property == vpiEndLine ? span[1]
                                                      : span[2];
            return (PLI_INT32)value;
        }
        case vpiSize:
            if (!expression) break;
            /* A real expression reports its IEEE 754 storage (S42-D8). */
            if (node->flags & LLG_VPI_CODE_REAL) return 64;
            if (!node->width && (node->flags & LLG_VPI_CODE_OPAQUE)) break;
            return (PLI_INT32)node->width;
        case vpiSigned:
            if (!expression) break;
            return (node->flags & LLG_VPI_CODE_SIGNED) != 0;
        case vpiScalar:
        case vpiVector:
            if (!expression || (node->flags & LLG_VPI_CODE_REAL) || !node->width) break;
            return property == vpiScalar ? node->width == 1 : node->width > 1;
        case vpiOpType:
            if (type == vpiAssignment && node->subtype) return node->subtype;
            if (type != vpiOperation || (node->flags & LLG_VPI_CODE_OPAQUE)) break;
            return node->subtype;
        case vpiConstType:
            if (type != vpiConstant) break;
            return node->subtype;
        case vpiBlocking:
            if (type != vpiAssignment && type != vpiEventStmt) break;
            return (node->flags & LLG_VPI_CODE_BLOCKING) != 0;
        case vpiCaseType:
            if (type != vpiCase || (node->flags & LLG_VPI_CODE_OPAQUE)) break;
            return node->subtype;
        case vpiQualifier:
            if (type != vpiIf && type != vpiIfElse && type != vpiCase) break;
            return node->qualifier;
        case vpiConstantSelect:
            if (!code_is_select(type) && type != vpiVarSelect) break;
            return (node->flags & LLG_VPI_CODE_CONSTANT) != 0;
        case vpiIndexedPartSelectType:
            if (type != vpiIndexedPartSelect) break;
            return node->subtype;
        case vpiLocalVarDecls:
            if (type != vpiFor) break;
            return (node->flags & LLG_VPI_CODE_LOCALS) != 0;
        case vpiUserDefn:
            /* Annex K: a user-defined system task or function; a task,
             * function or method call is none (decision A19-D32). */
            if (type == vpiFuncCall || type == vpiTaskCall || type == vpiMethodFuncCall ||
                type == vpiMethodTaskCall)
                return 0;
            if (type != vpiSysFuncCall && type != vpiSysTaskCall) break;
            return (node->flags & LLG_VPI_CODE_USER) != 0;
        case vpiDirection:
            if (type != vpiPropFormalDecl && type != vpiSeqFormalDecl) break;
            return node->subtype;
        case vpiDistType:
            /* 37.30: vpiEqualDist for `:=`, vpiDivDist for `:/`. */
            if (type != vpiDistItem) break;
            return node->subtype;
        case vpiFuncType: {
            if (type == vpiFuncCall && node->object >= 0) {
                const llg_vpi_model_object_t* callee = &g_vpi.objects[node->object];
                if (callee->detail) return callee->detail;
            }
            if (type == vpiSysFuncCall && (node->flags & LLG_VPI_CODE_USER)) {
                const char* name = code_text(node);
                llg_vpi_registration_t* registration = name ? find_registration(name) : NULL;
                if (registration && registration->data.type == vpiSysFunc)
                    return registration->data.sysfunctype;
            }
            break;
        }
        default:
            break;
    }
    return unsupported_property();
}

static PLI_BYTE8* code_string(uint32_t index, int property) {
    const llg_vpi_code_t* node = code_at(index);
    int type = code_type(node);
    switch (property) {
        case vpiName: {
            /* Calls, references outside the catalog and named blocks
             * outside a process have names; statements carry no label. */
            if (code_is_assertion_object(type) && type != vpiPropertySpec)
                return (PLI_BYTE8*)code_text(node);
            if (type == vpiFuncCall || type == vpiTaskCall || type == vpiSysFuncCall ||
                type == vpiSysTaskCall || type == vpiMethodFuncCall ||
                type == vpiMethodTaskCall || type == vpiNamedBegin || type == vpiNamedFork ||
                ((node->flags & LLG_VPI_CODE_OPAQUE) && code_is_expression(node) &&
                 type != vpiOperation && type != vpiConstant))
                return (PLI_BYTE8*)code_text(node);
            /* A select is named by its text (decision A19-D24). */
            if (code_is_named_select(type)) return (PLI_BYTE8*)code_decompile(index);
            if (code_is_statement(type)) return NULL;
            break;
        }
        case vpiFullName: {
            /* 37.47, 37.49: a declaration is named within its scope. */
            const char* name = code_text(node);
            if ((type == vpiPropertyDecl || type == vpiSequenceDecl) && node->scope && name) {
                snprintf(g_vpi.str_buffer, sizeof(g_vpi.str_buffer), "%s.%s",
                         g_vpi.objects[node->scope - 1u].full_name, name);
                return (PLI_BYTE8*)g_vpi.str_buffer;
            }
            /* A select: its scope's name and its text. */
            if (code_is_named_select(type)) {
                llg_vpi_text_t text = {g_vpi.decompiled, 0, g_vpi.decompiled_capacity, 0};
                text_put(&text, "");
                if (node->scope) {
                    text_put(&text, g_vpi.objects[node->scope - 1u].full_name);
                    text_put(&text, ".");
                }
                decompile(&text, index, 0);
                g_vpi.decompiled = text.text;
                g_vpi.decompiled_capacity = text.capacity;
                if (text.failed) {
                    vpi_error("LLG_VPI_UNSUPPORTED", "llg cannot decompile this expression");
                    return NULL;
                }
                return (PLI_BYTE8*)text.text;
            }
            break;
        }
        case vpiFile:
            return node->file ? (PLI_BYTE8*)g_vpi.code->files[node->file - 1u] : NULL;
        case vpiDecompile:
            if (code_is_expression(node) || type == vpiSysTaskCall || type == vpiTaskCall ||
                type == vpiMethodTaskCall)
                return (PLI_BYTE8*)code_decompile(index);
            break;
        default:
            break;
    }
    vpi_error("LLG_VPI_UNSUPPORTED", "unsupported VPI string property for this object");
    return NULL;
}

/* The statement object row `object` (an unnamed block, for or foreach that
 * is a scope) stands for: its code row, or LLG_VPI_NONE after an error. */
static uint32_t statement_scope_code(const llg_vpi_model_object_t* object) {
    if (!g_vpi.code || !object->code) {
        vpi_error("LLG_VPI_UNSUPPORTED",
                  "statements are objects only in models generated with --vpi or --dpi-lib");
        return LLG_VPI_NONE;
    }
    return object->code - 1u;
}

/* The clocking relations of 37.5, 37.6, 37.9 and 37.44 of catalog row
 * `row`; `*handled` is 0 when `type` is not one of them for that row. */
static vpiHandle clocking_relation(int type, uint32_t row, int* handled) {
    llg_vpi_model_object_t* object = row_object(row);
    *handled = 1;
    if (object->type == vpiModule || object->type == vpiInterface || object->type == vpiProgram) {
        if (type == vpiDefaultClocking)
            return object->target ? object_handle(row_of(object->target)) : NULL;
        if (type == vpiDefaultDisableIff) return object->conn_count ? conn_handle(row, 0u) : NULL;
        /* The global clocking declared in the instance (decision A19-D30). */
        if (type == vpiGlobalClocking && object->type != vpiProgram) {
            for (uint32_t child = g_vpi.first_child[row]; child;
                 child = g_vpi.next_sibling[child - 1u])
                if (g_vpi.objects[child - 1u].type == vpiClockingBlock &&
                    (g_vpi.objects[child - 1u].flags & LLG_VPI_OBJ_QUALIFIER))
                    return object_handle(child - 1u);
            return NULL;
        }
    }
    if (object->type == vpiClockingBlock || object->type == vpiClockingIODecl) {
        if (type == vpiClockingEvent || type == vpiInputSkew || type == vpiOutputSkew) {
            if (type == vpiClockingEvent && object->type != vpiClockingBlock) {
                vpi_error("LLG_VPI_RELATION", "a clocking io declaration has no clocking event");
                return NULL;
            }
            uint32_t code = statement_scope_code(object);
            if (code == LLG_VPI_NONE) return NULL;
            uint32_t child = code_child(code_at(code), type);
            return child == LLG_VPI_NONE ? NULL : code_handle(child);
        }
        if (type == vpiExpr && object->type == vpiClockingIODecl)
            return object->conn_count ? conn_handle(row, 0u) : NULL;
        if (type == vpiClockingBlock && object->type == vpiClockingIODecl)
            return object->parent ? object_handle(row_of(object->parent)) : NULL;
        /* 37.44 gives a clocking block or io declaration no module, and an
         * io declaration no instance or parent (decision A19-D30). */
        if (type == vpiModule ||
            (object->type == vpiClockingIODecl && (type == vpiInstance || type == vpiParent))) {
            vpi_error("LLG_VPI_RELATION", "unsupported VPI one-to-one relation for this object");
            return NULL;
        }
    }
    *handled = 0;
    return NULL;
}

static vpiHandle statement_relation(int type, const llg_vpi_slot_t* slot) {
    const llg_vpi_stmt_t* stmt = (const llg_vpi_stmt_t*)slot->pointer;
    if (type == vpiLhs) {
        vpiHandle lhs = statement_lhs(stmt);
        if (!lhs && !g_vpi.error_pending)
            vpi_error("LLG_VPI_UNSUPPORTED",
                      "this left-hand side is a concatenation or has no catalog object; llg "
                      "has no expression objects");
        return lhs;
    }
    vpi_error("LLG_VPI_UNSUPPORTED",
              "a force or release statement has only the vpiLhs relation in llg");
    return NULL;
}

static vpiHandle cont_assign_bit_lhs(uint32_t row, uint32_t bit);
static int net_handle(const llg_vpi_slot_t* slot);

vpiHandle vpi_handle(PLI_INT32 type, vpiHandle ref_handle) {
    vpi_enter();
    if (type == vpiSysTfCall || type == vpiSysFuncCall || type == vpiSysTaskCall) {
        if (ref_handle) {
            vpi_error("LLG_VPI_RELATION", "vpiSysTfCall takes a NULL reference handle");
            return NULL;
        }
        if (!g_vpi.active_call) {
            vpi_error("LLG_VPI_RELATION", "no user-defined system task or function is active");
            return NULL;
        }
        return call_handle_for(g_vpi.active_call->site);
    }
    {
        int handled;
        vpiHandle result = rts_handle(type, ref_handle, &handled);
        if (handled) return result;
    }
    uint32_t index;
    llg_vpi_slot_t* slot = checked_handle(ref_handle, &index);
    if (!slot) return NULL;
    if (slot->kind == SLOT_ASSERTION) return assertion_relation(type, slot);
    if (slot->kind == SLOT_FSM || slot->kind == SLOT_FSMSTATE) return fsm_relation(type, slot);
    if (slot->kind == SLOT_STMT) return statement_relation(type, slot);
    if (slot->kind == SLOT_IMP) {
        vpi_error("LLG_VPI_RELATION", "an intermodule path has no one-to-one relations (37.33)");
        return NULL;
    }
    if (dyn_slot(slot)) {
        int handled;
        vpiHandle result = dyn_handle(type, slot, &handled);
        if (handled) return result;
    }
    if (slot->kind == SLOT_CODE) return code_relation(type, (uint32_t)slot->aux);
    uint32_t argument = argument_code(slot);
    if (argument != LLG_VPI_NONE) return code_relation(type, argument);
    llg_vpi_model_object_t* object = row_object(slot->object);
    /* 37.37: a task or function that is not a class method has no class
     * defn. */
    if (type == vpiClassDefn && slot->kind == SLOT_OBJECT && object &&
        (object->type == vpiTask || object->type == vpiFunction))
        return NULL;
    if (slot->kind == SLOT_OBJECT && object && is_statement_scope_type(object->type)) {
        /* A statement scope's relations are its statement's (37.12, 37.69,
         * 37.70); vpiScope is the enclosing scope. */
        uint32_t code = statement_scope_code(object);
        return code == LLG_VPI_NONE ? NULL : code_relation(type, code);
    }
    if (slot->kind == SLOT_OBJECT && object) {
        int handled;
        vpiHandle result = clocking_relation(type, slot->object, &handled);
        if (handled) return result;
    }
    if (slot->kind == SLOT_OBJECT && object) {
        int handled;
        vpiHandle result = declaration_relation(type, slot->object, &handled);
        if (handled) return result;
    }
    switch (type) {
        case vpiStmt:
            /* 37.37, 37.58: the statement of a process, task or function
             * (NULL for an empty body, 37.12 detail 5). */
            if (slot->kind == SLOT_OBJECT && object &&
                (is_process_type(object->type) || object->type == vpiTask ||
                 object->type == vpiFunction)) {
                if (!g_vpi.code) {
                    vpi_error("LLG_VPI_UNSUPPORTED",
                              "statements are objects only in models generated with --vpi or "
                              "--dpi-lib");
                    return NULL;
                }
                return object->code ? code_handle(object->code - 1u) : NULL;
            }
            break;
        case vpiTypespec: {
            if ((slot->kind == SLOT_SELECT && slot->span == 1) || slot->kind == SLOT_PORTBIT) {
                /* 37.55 detail 5: a bit of a packed value has the scalar
                 * typespec at the end of the element chain of its vector;
                 * a port bit (37.14) that of its port. */
                const llg_vpi_typespec_t* typespec =
                    slot->kind == SLOT_PORTBIT || slot->element == LLG_VPI_NO_ELEMENT
                                                          ? typespec_ref(object->typespec)
                                                          : element_typespec(object);
                for (uint32_t guard = 0; typespec && typespec->element && guard < 64u; ++guard)
                    typespec = typespec_ref(typespec->element);
                if (typespec && typespec->range_count == 0u &&
                    (typespec->type == vpiLogicTypespec || typespec->type == vpiBitTypespec))
                    return typespec_handle(typespec);
                vpi_error("LLG_VPI_UNSUPPORTED", "llg exposes no typespec for the type of this object");
                return NULL;
            }
            if (slot->kind != SLOT_OBJECT && slot->kind != SLOT_ELEMENT &&
                slot->kind != SLOT_MEMBER && slot->kind != SLOT_TSMEMBER)
                break;
            if (slot->kind == SLOT_OBJECT && object && is_scope_type(object->type)) break;
            if (slot->kind == SLOT_TSMEMBER) {
                const llg_vpi_typespec_t* typespec =
                    typespec_at(g_vpi.types->members[slot->aux].typespec);
                if (typespec) return typespec_handle(typespec);
            } else {
                const llg_vpi_typespec_t* typespec = value_typespec(slot);
                if (typespec)
                    return declared_typespec_handle(
                        typespec, slot->object,
                        slot->kind == SLOT_MEMBER ? (int64_t)slot->aux + 1 : 0);
            }
            vpi_error("LLG_VPI_UNSUPPORTED", "llg exposes no typespec for the type of this object");
            return NULL;
        }
        case vpiTypedefAlias:
        case vpiElemTypespec:
        case vpiBaseTypespec: {
            if (slot->kind != SLOT_TYPESPEC) break;
            const llg_vpi_typespec_t* typespec = typespec_at((uint32_t)slot->aux);
            if (type == vpiBaseTypespec && typespec->type != vpiEnumTypespec) break;
            /* 37.23 detail 8 defines vpiElemTypespec for array, packed
             * array, bit and logic typespecs and makes it NULL for struct,
             * union and enum typespecs; other typespecs do not have it. */
            if (type == vpiElemTypespec && typespec->type != vpiArrayTypespec &&
                typespec->type != vpiPackedArrayTypespec && typespec->type != vpiBitTypespec &&
                typespec->type != vpiLogicTypespec && !is_aggregate_typespec(typespec) &&
                typespec->type != vpiEnumTypespec)
                break;
            /* A typespec without the relation yields NULL with no error
             * (37.23 details 1 and 8). */
            uint32_t reference = type == vpiTypedefAlias ? typespec->alias
                                 : type == vpiElemTypespec ? typespec->element : typespec->base;
            /* A declaration's typespec owns the typespecs below it. */
            return declared_typespec_handle(typespec_ref(reference), slot->object, slot->left);
        }
        case vpiParent:
            if (slot->kind == SLOT_PRIMTERM || slot->kind == SLOT_PORTBIT ||
                slot->kind == SLOT_TERM || slot->kind == SLOT_CABIT)
                return object_handle(slot->object);
            if (slot->kind == SLOT_MEMBER) {
                /* 37.17 detail 26: the enclosing member, else the struct or
                 * union variable (or array element) that holds the member. */
                const llg_vpi_member_node_t* node = node_at((uint32_t)slot->aux);
                if (node && node->parent)
                    return make_member(slot->object, slot->element, node->parent - 1u);
                return slot->element == LLG_VPI_NO_ELEMENT
                           ? object_handle(slot->object)
                           : make_element(slot->object, slot->element, NULL);
            }
            /* 37.9 and 37.10 give programs and packages no parent
             * (decision A19-D29). */
            if (slot->kind == SLOT_OBJECT && object &&
                (object->type == vpiPackage || object->type == vpiProgram))
                break;
            if (slot->kind == SLOT_OBJECT)
                return object && object->parent ? object_handle(row_of(object->parent)) : NULL;
            /* A typespec's range has no owning object. */
            if (slot->kind == SLOT_ELEMENT ||
                (slot->kind == SLOT_RANGE && slot->object != LLG_VPI_NONE))
                return object_handle(slot->object);
            if (slot->kind == SLOT_SELECT)
                return slot->element == LLG_VPI_NO_ELEMENT ? object_handle(slot->object)
                                                           : make_element(slot->object, slot->element, NULL);
            break;
        case vpiScope:
            if (slot->kind == SLOT_CALL) {
                llg_vpi_callsite_t* site = (llg_vpi_callsite_t*)slot->pointer;
                return site->scope ? object_handle(row_of(site->scope)) : NULL;
            }
            if (object) {
                llg_vpi_model_object_t* scope = slot->kind == SLOT_OBJECT && is_scope_type(object->type)
                                                    ? object->parent : object;
                while (scope && !is_scope_type(scope->type)) scope = scope->parent;
                return scope ? object_handle(row_of(scope)) : NULL;
            }
            break;
        case vpiModule: {
            uint32_t start = LLG_VPI_NONE;
            if (slot->kind == SLOT_CALL) {
                llg_vpi_callsite_t* site = (llg_vpi_callsite_t*)slot->pointer;
                start = site->scope ? row_of(site->scope) : LLG_VPI_NONE;
            } else if (object) {
                if (slot->kind == SLOT_OBJECT && object->type == vpiPackage) break;
                start = slot->kind == SLOT_OBJECT && object->type == vpiModule && object->parent
                            ? row_of(object->parent) : slot->object;
            }
            if (start == LLG_VPI_NONE && slot->kind != SLOT_CALL && !object) break;
            uint32_t module = start == LLG_VPI_NONE ? LLG_VPI_NONE : enclosing_module(start);
            return module == LLG_VPI_NONE ? NULL : object_handle(module);
        }
        case vpiLeftRange:
        case vpiRightRange: {
            int64_t value;
            if (!handle_bound(slot, type == vpiLeftRange, &value)) break;
            return make_constant(value, vpiDecConst);
        }
        case vpiIndex:
            if (slot->kind == SLOT_OBJECT && object &&
                (object->type == vpiModule || object->type == vpiInterface ||
                 object->type == vpiProgram || is_primitive_type(object->type))) {
                /* 37.5 detail 2, 37.6 and 37.9 detail 1, 37.31 detail 4:
                 * NULL outside an array. */
                int64_t value = 0;
                int dims = 0;
                if (instance_array_of(slot->object, &value, &dims) == LLG_VPI_NONE) return NULL;
                if (dims == 1) return make_constant(value, vpiDecConst);
                vpi_error("LLG_VPI_UNSUPPORTED",
                          "vpiIndex of an element of a multidimensional instance array");
                return NULL;
            }
            if (slot->kind == SLOT_SELECT && slot->span == 1)
                return make_constant(slot->left, vpiDecConst);
            if (slot->kind == SLOT_ELEMENT && object->dim_count == 1) {
                int64_t left, right;
                array_dim(object, 0, &left, &right);
                int64_t value = left >= right ? left - (int64_t)slot->element
                                              : left + (int64_t)slot->element;
                return make_constant(value, vpiDecConst);
            }
            /* A whole net, variable or named event is not an array element
             * and has no index: the text defines none (decision A19-D28). */
            if (slot->kind == SLOT_OBJECT && has_index_relation(slot)) {
                vpi_error("LLG_VPI_RELATION", "vpiIndex of an object outside an array");
                return NULL;
            }
            break;
        case vpiInstanceArray:
        case vpiModuleArray:
        case vpiPrimitiveArray:
            /* 37.5, 37.6, 37.9, 37.31: the instance array an element belongs to.
             * The text defines no answer outside an array: NULL, with a
             * vpiError for a module or interface (decision A19-D28). */
            if (slot->kind == SLOT_OBJECT && object &&
                (type == vpiPrimitiveArray ? is_primitive_type(object->type)
                 : type == vpiModuleArray  ? object->type == vpiModule
                                           : object->type == vpiModule ||
                                                 object->type == vpiInterface ||
                                                 object->type == vpiProgram)) {
                uint32_t array = instance_array_of(slot->object, NULL, NULL);
                if (array != LLG_VPI_NONE) return object_handle(array);
                if (type == vpiPrimitiveArray) return NULL;
                vpi_error("LLG_VPI_RELATION", "the instance is not an element of an array");
                return NULL;
            }
            break;
        case vpiLowConn:
        case vpiHighConn: {
            int high = type == vpiHighConn;
            if (slot->kind == SLOT_OBJECT && object && object->type == vpiPort) {
                /* 37.14 detail 5, 37.15: both connections of an interface
                 * or modport port are reference objects. */
                if (object->detail) return make_aux_slot(SLOT_REFOBJ, slot->object, (uint32_t)high);
                /* Detail 10: an unconnected or null port gives NULL. */
                if (high) return object->conn_count ? conn_handle(slot->object, 0u) : NULL;
                if (!object->target) return NULL;
                return object_handle(row_of(object->target));
            }
            if (slot->kind == SLOT_PORTBIT) return port_bit_conn(slot, high);
            break;
        }
        case vpiLhs:
        case vpiRhs:
            if (slot->kind == SLOT_OBJECT && object && object->type == vpiContAssign)
                return conn_handle(slot->object, type == vpiLhs ? 0u : 1u);
            /* 37.43: a bit's target is that bit of the assignment's target;
             * its right-hand side is the assignment's (decision A19-D34). */
            if (slot->kind == SLOT_CABIT)
                return type == vpiLhs ? cont_assign_bit_lhs(slot->object, (uint32_t)slot->aux)
                                      : conn_handle(slot->object, 1u);
            break;
        case vpiSimNet:
            /* 37.16 detail 17: models generated for VPI applications keep
             * every net of its own (no collapsing), so the simulated net of
             * a net is itself (decision A19-D34). */
            if (net_handle(slot)) {
                if (slot->kind == SLOT_OBJECT) return object_handle(slot->object);
                if (slot->kind == SLOT_ELEMENT) return make_element(slot->object, slot->element, NULL);
                return make_select(slot->object, slot->element, slot->lsb, slot->span, slot->left,
                                   slot->right, NULL);
            }
            break;
        case vpiPrimitive:
            /* 37.31: a terminal's primitive. */
            if (slot->kind == SLOT_PRIMTERM) return object_handle(slot->object);
            break;
        case vpiModPath:
        case vpiTchk:
            /* 37.35, 37.36: the path or timing check of a terminal. */
            if (slot->kind == SLOT_TERM && handle_type(slot) == (type == vpiModPath ? vpiPathTerm
                                                                                    : vpiTchkTerm))
                return object_handle(slot->object);
            break;
        case vpiExpr:
            if (slot->kind == SLOT_PRIMTERM || slot->kind == SLOT_TERM)
                return conn_handle(slot->object, (uint32_t)slot->aux);
            /* 37.17 detail 8, 37.26 detail 3: the initialization expression
             * of a variable, the default expression of a value parameter
             * (decision A19-D32); NULL without one. */
            if (slot->kind == SLOT_OBJECT && object &&
                (is_variable_type(object->type) || object->type == vpiRegArray ||
                 object->type == vpiParameter || (object->flags & LLG_VPI_OBJ_DYNAMIC))) {
                if (!g_vpi.code) {
                    vpi_error("LLG_VPI_UNSUPPORTED",
                              "expressions are objects only in models generated with --vpi or "
                              "--dpi-lib");
                    return NULL;
                }
                return object->code ? code_handle(object->code - 1u) : NULL;
            }
            break;
        case vpiCondition:
            /* 37.35, 37.36: the condition of a state-dependent path or of a
             * timing check terminal; NULL without one (and for `ifnone`). */
            if (slot->kind == SLOT_OBJECT && object && object->type == vpiModPath) {
                uint32_t conn = role_conn(slot->object, LLG_VPI_ROLE_CONDITION);
                return conn == LLG_VPI_NONE ? NULL : conn_handle(slot->object, conn);
            }
            if (slot->kind == SLOT_TERM && handle_type(slot) == vpiTchkTerm) {
                const llg_vpi_conn_t* conn = row_conn(slot->object, (uint32_t)slot->aux);
                return conn && conn->condition ? conn_handle(slot->object, conn->condition - 1u)
                                               : NULL;
            }
            break;
        case vpiModDataPathIn:
            if (slot->kind == SLOT_OBJECT && object && object->type == vpiModPath)
                return role_term(slot->object, LLG_VPI_ROLE_PATH_DATA);
            break;
        case vpiDelay:
            /* 37.3.4: the delay expressions the source gives a primitive or
             * primitive array, a net (a bit or element: its net's), a
             * continuous assignment (or one of its bits), a module path or
             * a timing check: the expression, a vpiListOp operation of
             * several, or NULL without (decision A20-D16). */
            if (!object) break;
            if (slot->kind == SLOT_OBJECT
                    ? is_primitive_type(object->type) || object->type == vpiGateArray ||
                          object->type == vpiSwitchArray || object->type == vpiUdpArray ||
                          object->type == vpiContAssign || object->type == vpiModPath ||
                          object->type == vpiTchk || object->is_net
                    : slot->kind == SLOT_CABIT ||
                          (object->is_net && (slot->kind == SLOT_ELEMENT ||
                                              slot->kind == SLOT_MEMBER ||
                                              (slot->kind == SLOT_SELECT && slot->span == 1)))) {
                if (!g_vpi.code) {
                    vpi_error("LLG_VPI_UNSUPPORTED",
                              "delay expressions are objects only in models generated with "
                              "--vpi or --dpi-lib");
                    return NULL;
                }
                return object->timing && object->timing->delay
                           ? code_handle(object->timing->delay - 1u)
                           : NULL;
            }
            break;
        case vpiTchkRefTerm:
        case vpiTchkDataTerm:
            if (slot->kind == SLOT_OBJECT && object && object->type == vpiTchk)
                return role_term(slot->object, type == vpiTchkRefTerm ? LLG_VPI_ROLE_TCHK_REF
                                                                      : LLG_VPI_ROLE_TCHK_DATA);
            break;
        case vpiTchkNotifier:
            if (slot->kind == SLOT_OBJECT && object && object->type == vpiTchk) {
                uint32_t conn = role_conn(slot->object, LLG_VPI_ROLE_NOTIFIER);
                return conn == LLG_VPI_NONE ? NULL : conn_handle(slot->object, conn);
            }
            break;
        case vpiActual:
            if (slot->kind == SLOT_REFOBJ) return object_handle(row_of(object->target));
            break;
        case vpiInstance:
            /* 37.10 detail 3: the immediate package, module, interface or
             * program instance; an instance's is the one it is declared
             * in (NULL at the top level). */
            if (object && slot->kind != SLOT_CALL) {
                uint32_t start = slot->object;
                if (slot->kind == SLOT_OBJECT && is_instance_type(object->type)) {
                    if (!object->parent) return NULL;
                    start = row_of(object->parent);
                }
                uint32_t instance = enclosing_instance(start);
                return instance == LLG_VPI_NONE ? NULL : object_handle(instance);
            }
            break;
        case vpiUserSystf:
            if (slot->kind == SLOT_CALL) {
                llg_vpi_callsite_t* site = (llg_vpi_callsite_t*)slot->pointer;
                return site->registration ? encode_handle(site->registration->slot) : NULL;
            }
            break;
        default:
            break;
    }
    vpi_error("LLG_VPI_RELATION", "unsupported VPI one-to-one relation for this object");
    return NULL;
}

/* ── connectivity (37.16, 37.17, 37.21, 37.42, 37.43) ───────────────────── */

/* What references a net or variable: built on the first connectivity query
 * from the connections of the catalog rows and the force statements of the
 * code tables, so a model whose applications never ask pays nothing. */
enum {
    LINK_CONT_ASSIGN = 0, /* lhs (driver) or rhs (load) of a continuous assignment */
    LINK_PRIM_TERM,       /* a primitive terminal: `aux` is its index */
    LINK_PORT_HIGH,       /* the high connection of a port */
    LINK_PORT_LOW,        /* the low connection of a port */
    LINK_FORCE,           /* lhs (driver) or rhs (load) of the force statement `source` (code row) */
    LINK_TERM,            /* a module path or timing check terminal: connection `aux` */
};
#define LINK_DRIVER 0x1u
#define LINK_LOAD 0x2u
#define LINK_MAPPED 0x4u  /* bit `lsb` is bit `offset` of the lhs or port value */

typedef struct llg_vpi_link {
    uint32_t row;         /* the net or variable referenced */
    uint64_t element;     /* its element, or LLG_VPI_NO_ELEMENT: the whole row */
    uint32_t lsb;         /* the referenced bits, as storage offsets */
    uint32_t width;
    uint32_t offset;      /* LINK_MAPPED: the bit of the source value of bit `lsb` */
    uint32_t source;      /* catalog row; the code row of a force */
    uint32_t aux;
    uint8_t kind;
    uint8_t flags;        /* LINK_DRIVER, LINK_LOAD, LINK_MAPPED */
} llg_vpi_link_t;

/* Bits of a net or variable a connection references (a span of a link). */
typedef struct {
    uint32_t row;
    uint64_t element;
    uint32_t lsb;
    uint32_t width;
    uint32_t offset;
    int mapped;
} llg_vpi_span_t;

typedef struct {
    llg_vpi_span_t* items;
    size_t count;
    size_t capacity;
    int failed;
} llg_vpi_spans_t;

/* Tunable: the bit spans one query may visit through port connections. */
#define LLG_VPI_MAX_GROUP_SPANS (1u << 16)
#define LLG_VPI_MAX_SPAN_DEPTH 64

static void spans_push(llg_vpi_spans_t* list, uint32_t row, uint64_t element, uint32_t lsb,
                       uint32_t width, uint32_t offset, int mapped) {
    if (list->failed || !width) return;
    if (list->count == list->capacity) {
        size_t capacity = list->capacity ? list->capacity * 2u : 8u;
        llg_vpi_span_t* grown =
            (llg_vpi_span_t*)realloc(list->items, capacity * sizeof(*grown));
        if (!grown) {
            list->failed = 1;
            return;
        }
        list->items = grown;
        list->capacity = capacity;
    }
    llg_vpi_span_t* span = &list->items[list->count++];
    span->row = row;
    span->element = element;
    span->lsb = lsb;
    span->width = width;
    span->offset = offset;
    span->mapped = mapped;
}

/* A net or variable row whose bits connections can reference. */
static int is_connectable(const llg_vpi_model_object_t* object) {
    return object && (object->type == vpiNet || is_variable_type(object->type) ||
                      is_array_type(object->type));
}

/* Bits of a value of `row` (one element of an array); a real is one unit. */
static uint32_t value_bits(const llg_vpi_model_object_t* object) {
    return object->is_real || !object->width ? 1u : object->width;
}

/* The one span an lvalue-shaped expression object references: a whole net
 * or variable, an element of an array, or a select of either; a select with
 * a variable index references its whole prefix. 0 for anything else. */
static int code_span(uint32_t index, llg_vpi_span_t* span, int depth) {
    if (depth > LLG_VPI_MAX_SPAN_DEPTH) return 0;
    const llg_vpi_code_t* node = code_at(code_visible(index));
    if (!node || (node->flags & LLG_VPI_CODE_OPAQUE)) return 0;
    if (node->flags & LLG_VPI_CODE_ALIAS) {
        llg_vpi_model_object_t* object = row_object((uint32_t)node->object);
        if (node->object < 0 || !is_connectable(object)) return 0;
        span->row = (uint32_t)node->object;
        span->element = LLG_VPI_NO_ELEMENT;
        span->lsb = 0;
        span->width = value_bits(object);
        return 1;
    }
    if (node->type == vpiVarSelect) {
        uint32_t parent = code_child(node, vpiParent);
        const llg_vpi_code_t* array = parent == LLG_VPI_NONE ? NULL : code_at(parent);
        if (!array || !(array->flags & LLG_VPI_CODE_ALIAS) || array->object < 0) return 0;
        llg_vpi_model_object_t* object = row_object((uint32_t)array->object);
        if (!object || !is_array_type(object->type)) return 0;
        span->row = (uint32_t)array->object;
        span->element = LLG_VPI_NO_ELEMENT;
        span->lsb = 0;
        span->width = value_bits(object);
        if (!(node->flags & LLG_VPI_CODE_CONSTANT)) return 1;
        uint64_t element = 0;
        uint32_t dim = 0;
        for (uint32_t i = 0; i < node->count; ++i) {
            uint32_t child = node->first + i;
            if (g_vpi.code->nodes[child].role != vpiIndex) continue;
            int64_t left, right, value;
            uint64_t offset = 0;
            if (!array_dim(object, dim++, &left, &right)) return 1;
            uint64_t extent = (uint64_t)(left >= right ? left - right : right - left) + 1u;
            if (!code_index(child, &value, depth + 1) || !index_offset(left, right, value, &offset))
                return 1;
            element = element * extent + offset;
        }
        if (dim == object->dim_count && element < g_vpi.element_counts[span->row])
            span->element = element;
        return 1;
    }
    if (!code_is_select(node->type)) return 0;
    uint32_t parent = code_child(node, vpiParent);
    int64_t left, right;
    if (parent == LLG_VPI_NONE || !code_span(parent, span, depth + 1)) return 0;
    if (!(node->flags & LLG_VPI_CODE_CONSTANT) || !code_range(node, &left, &right)) return 1;
    uint64_t extent = (uint64_t)(left >= right ? left - right : right - left) + 1u;
    uint64_t stride = span->width / extent;
    if (!stride) stride = 1;
    int64_t low = 0, high = 0;
    int known;
    if (node->type == vpiBitSelect) {
        known = code_index(code_child(node, vpiIndex), &low, depth + 1);
        high = low;
    } else if (node->type == vpiPartSelect) {
        known = code_index(code_child(node, vpiLeftRange), &high, depth + 1) &&
                code_index(code_child(node, vpiRightRange), &low, depth + 1);
    } else {
        int64_t start = 0, count = 0;
        known = code_index(code_child(node, vpiBaseExpr), &start, depth + 1) &&
                code_index(code_child(node, vpiWidthExpr), &count, depth + 1) && count > 0;
        low = start;
        high = node->subtype == vpiNegIndexed ? start - count + 1 : start + count - 1;
    }
    int64_t from_low = left >= right ? low - right : right - low;
    int64_t from_high = left >= right ? high - right : right - high;
    int64_t from = from_low < from_high ? from_low : from_high;
    int64_t to = from_low < from_high ? from_high : from_low;
    if (!known || from < 0 || (uint64_t)to >= extent) return 1;
    uint64_t lsb = (uint64_t)from * stride, width = (uint64_t)(to - from + 1) * stride;
    if (lsb + width > span->width) return 1;
    span->lsb += (uint32_t)lsb;
    span->width = (uint32_t)width;
    return 1;
}

/* The spans the expression object `index` references. With `lhs`, it is an
 * assignment target or port value: each span records the bit of that value
 * its first bit is (`offset`, a concatenation's last operand at 0);
 * otherwise every net or variable it reads, index expressions included. */
static void code_spans(uint32_t index, int lhs, uint32_t offset, llg_vpi_spans_t* out,
                       int depth) {
    if (depth > LLG_VPI_MAX_SPAN_DEPTH) return;
    index = code_visible(index);
    const llg_vpi_code_t* node = code_at(index);
    if (!node || (node->flags & (LLG_VPI_CODE_OPAQUE | LLG_VPI_CODE_LINK))) return;
    llg_vpi_span_t span;
    if (code_span(index, &span, depth)) {
        spans_push(out, span.row, span.element, span.lsb, span.width, offset, lhs);
        if (lhs) return;
        /* A select reads its index expressions as well. */
        for (uint32_t i = 0; i < node->count; ++i) {
            uint32_t child = node->first + i;
            int role = g_vpi.code->nodes[child].role;
            if (role != vpiParent) code_spans(child, 0, 0, out, depth + 1);
            else if (code_is_select(node->type)) code_spans(child, 0, 0, out, depth + 1);
        }
        return;
    }
    if (node->flags & LLG_VPI_CODE_ALIAS) return;
    if (lhs && node->type == vpiOperation && node->subtype == vpiConcatOp) {
        /* Operands most significant first: the last one starts at `offset`. */
        uint32_t at = offset;
        for (uint32_t i = node->count; i-- > 0;) {
            uint32_t child = node->first + i;
            const llg_vpi_code_t* operand = code_at(code_visible(child));
            code_spans(child, 1, at, out, depth + 1);
            at += operand && operand->width ? operand->width : 1u;
        }
        return;
    }
    /* Any other expression (a port connected to `!x`) only reads its
     * operands: they are unmapped load spans even on the target side. */
    for (uint32_t i = 0; i < node->count; ++i) code_spans(node->first + i, 0, 0, out, depth + 1);
}

/* The spans connection `index` of `row` references (see code_spans). */
static void conn_spans(uint32_t row, uint32_t index, int lhs, llg_vpi_spans_t* out) {
    const llg_vpi_conn_t* conn = row_conn(row, index);
    if (!conn) return;
    const llg_vpi_compile_arg_t* ref = &conn->ref;
    llg_vpi_model_object_t* object =
        ref->object >= 0 ? row_object((uint32_t)ref->object) : NULL;
    switch (ref->ref_kind) {
        case LLG_VPI_ARG_OBJECT:
            if (is_connectable(object))
                spans_push(out, (uint32_t)ref->object, LLG_VPI_NO_ELEMENT, 0, value_bits(object),
                           0, lhs);
            return;
        case LLG_VPI_ARG_SELECT:
            if (is_connectable(object))
                spans_push(out, (uint32_t)ref->object, LLG_VPI_NO_ELEMENT, (uint32_t)ref->offset,
                           ref->select_width, 0, lhs);
            return;
        case LLG_VPI_ARG_ELEMENT:
            if (is_connectable(object) && ref->offset < g_vpi.element_counts[ref->object])
                spans_push(out, (uint32_t)ref->object, ref->offset, 0, value_bits(object), 0, lhs);
            return;
        default:
            if (ref->code && g_vpi.code) code_spans(ref->code - 1u, lhs, 0, out, 0);
            return;
    }
}

static int compare_links(const void* left, const void* right) {
    const llg_vpi_link_t *a = (const llg_vpi_link_t*)left, *b = (const llg_vpi_link_t*)right;
    if (a->row != b->row) return a->row < b->row ? -1 : 1;
    if (a->element != b->element) return a->element < b->element ? -1 : 1;
    if (a->lsb != b->lsb) return a->lsb < b->lsb ? -1 : 1;
    if (a->source != b->source) return a->source < b->source ? -1 : 1;
    return a->aux < b->aux ? -1 : a->aux > b->aux;
}

/* Append one link per span of `spans`, then empty it. */
static int links_add(llg_vpi_spans_t* spans, uint8_t kind, uint8_t roles, uint32_t source,
                     uint32_t aux) {
    if (spans->failed) return 0;
    if (g_vpi.link_count + spans->count > g_vpi.link_capacity) {
        size_t capacity = g_vpi.link_capacity ? g_vpi.link_capacity : 64u;
        while (capacity < g_vpi.link_count + spans->count) capacity *= 2u;
        llg_vpi_link_t* grown =
            (llg_vpi_link_t*)realloc(g_vpi.links, capacity * sizeof(*grown));
        if (!grown) return 0;
        g_vpi.links = grown;
        g_vpi.link_capacity = capacity;
    }
    for (size_t i = 0; i < spans->count; ++i) {
        const llg_vpi_span_t* span = &spans->items[i];
        llg_vpi_link_t* link = &g_vpi.links[g_vpi.link_count++];
        link->row = span->row;
        link->element = span->element;
        link->lsb = span->lsb;
        link->width = span->width;
        link->offset = span->offset;
        link->source = source;
        link->aux = aux;
        link->kind = kind;
        /* An unmapped span of a port's high connection is read by the port
         * expression whatever the port direction. */
        uint8_t span_roles = kind == LINK_PORT_HIGH && !span->mapped ? (uint8_t)LINK_LOAD : roles;
        link->flags = (uint8_t)(span_roles | (span->mapped ? LINK_MAPPED : 0u));
    }
    spans->count = 0;
    return 1;
}

static uint8_t direction_roles(int direction, int high) {
    if (direction == vpiInout) return LINK_DRIVER | LINK_LOAD;
    if (direction == vpiInput) return high ? LINK_LOAD : LINK_DRIVER;
    if (direction == vpiOutput) return high ? LINK_DRIVER : LINK_LOAD;
    return 0;
}

static int compare_port_links(const void* left, const void* right) {
    const llg_vpi_link_t* a = &g_vpi.links[*(const uint32_t*)left];
    const llg_vpi_link_t* b = &g_vpi.links[*(const uint32_t*)right];
    return a->source < b->source ? -1 : a->source > b->source;
}

/* Build the link table once. */
static int links_ready(void) {
    if (g_vpi.links_built) return 1;
    llg_vpi_spans_t spans;
    memset(&spans, 0, sizeof(spans));
    int ok = 1;
    for (uint32_t row = 0; ok && row < g_vpi.object_count; ++row) {
        const llg_vpi_model_object_t* object = &g_vpi.objects[row];
        if (object->type == vpiContAssign && object->conn_count >= 2) {
            conn_spans(row, 0u, 1, &spans);
            ok = links_add(&spans, LINK_CONT_ASSIGN, LINK_DRIVER, row, 0u);
            conn_spans(row, 1u, 0, &spans);
            ok = ok && links_add(&spans, LINK_CONT_ASSIGN, LINK_LOAD, row, 1u);
        } else if (is_primitive_type(object->type)) {
            for (uint32_t i = 0; ok && i < object->conn_count; ++i) {
                conn_spans(row, i, 0, &spans);
                int direction = object->conns[i].direction;
                uint8_t roles = direction == vpiInout    ? (uint8_t)(LINK_DRIVER | LINK_LOAD)
                                : direction == vpiOutput ? (uint8_t)LINK_DRIVER
                                                         : (uint8_t)LINK_LOAD;
                ok = links_add(&spans, LINK_PRIM_TERM, roles, row, i);
            }
        } else if (object->type == vpiPort && !object->detail) {
            if (object->conn_count) {
                conn_spans(row, 0u, 1, &spans);
                ok = links_add(&spans, LINK_PORT_HIGH, direction_roles(object->subtype, 1), row, 0u);
            }
            const llg_vpi_model_object_t* low = object->target;
            if (ok && is_connectable(low))
                spans_push(&spans, row_of(low), LLG_VPI_NO_ELEMENT, 0, value_bits(low), 0, 1);
            ok = ok && links_add(&spans, LINK_PORT_LOW, direction_roles(object->subtype, 0), row, 0u);
        } else if (object->type == vpiModPath || object->type == vpiTchk) {
            for (uint32_t i = 0; ok && i < object->conn_count; ++i) {
                if (!is_term_role(object->conns[i].role)) continue;
                conn_spans(row, i, 0, &spans);
                ok = links_add(&spans, LINK_TERM, 0u, row, i);
            }
        }
    }
    for (uint32_t index = 0; ok && g_vpi.code && index < g_vpi.code->node_count; ++index) {
        const llg_vpi_code_t* node = &g_vpi.code->nodes[index];
        if (node->type != vpiForce) continue;
        uint32_t lhs = code_child(node, vpiLhs), rhs = code_child(node, vpiRhs);
        if (lhs != LLG_VPI_NONE) code_spans(lhs, 1, 0, &spans, 0);
        ok = links_add(&spans, LINK_FORCE, LINK_DRIVER, index, 0u);
        if (ok && rhs != LLG_VPI_NONE) code_spans(rhs, 0, 0, &spans, 0);
        ok = ok && links_add(&spans, LINK_FORCE, LINK_LOAD, index, 1u);
    }
    free(spans.items);
    if (ok && g_vpi.link_count)
        qsort(g_vpi.links, g_vpi.link_count, sizeof(*g_vpi.links), compare_links);
    /* The high-connection links of each port, by port row. */
    size_t ports = 0;
    for (size_t i = 0; ok && i < g_vpi.link_count; ++i)
        ports += g_vpi.links[i].kind == LINK_PORT_HIGH;
    if (ok && ports) {
        g_vpi.port_links = (uint32_t*)malloc(ports * sizeof(*g_vpi.port_links));
        if (!g_vpi.port_links) ok = 0;
        for (size_t i = 0; ok && i < g_vpi.link_count; ++i)
            if (g_vpi.links[i].kind == LINK_PORT_HIGH)
                g_vpi.port_links[g_vpi.port_link_count++] = (uint32_t)i;
        if (ok)
            qsort(g_vpi.port_links, g_vpi.port_link_count, sizeof(*g_vpi.port_links),
                  compare_port_links);
    }
    if (!ok) {
        free(g_vpi.links);
        free(g_vpi.port_links);
        g_vpi.links = NULL;
        g_vpi.port_links = NULL;
        g_vpi.link_count = g_vpi.link_capacity = g_vpi.port_link_count = 0;
        vpi_error("LLG_VPI_NOMEM", "VPI connectivity index allocation failed");
        return 0;
    }
    g_vpi.links_built = 1;
    return 1;
}

/* The first link on `row`, or link_count. */
static size_t links_of(uint32_t row) {
    size_t low = 0, high = g_vpi.link_count;
    while (low < high) {
        size_t middle = low + (high - low) / 2u;
        if (g_vpi.links[middle].row < row) low = middle + 1u;
        else high = middle;
    }
    return low;
}

/* The bits a link and a span both cover, as [*from, *to). */
static int link_overlap(const llg_vpi_link_t* link, const llg_vpi_span_t* span, uint32_t* from,
                        uint32_t* to) {
    if (link->row != span->row) return 0;
    if (link->element != span->element && link->element != LLG_VPI_NO_ELEMENT &&
        span->element != LLG_VPI_NO_ELEMENT)
        return 0;
    uint32_t a = link->lsb > span->lsb ? link->lsb : span->lsb;
    uint64_t link_end = (uint64_t)link->lsb + link->width;
    uint64_t span_end = (uint64_t)span->lsb + span->width;
    uint64_t b = link_end < span_end ? link_end : span_end;
    if (a >= b) return 0;
    *from = a;
    *to = (uint32_t)b;
    return 1;
}

static int is_net_row(uint32_t row) {
    const llg_vpi_model_object_t* object = row_object(row);
    return object && object->is_net;
}

/* Whether a port connects a net on both sides bit for bit, so a driver
 * or load on one side drives or loads the other (23.3.3.7). */
static int port_link_transparent(const llg_vpi_link_t* link) {
    const llg_vpi_model_object_t* port = row_object(link->source);
    if (!port || !port->target || !port->target->is_net) return 0;
    if (link->kind == LINK_PORT_HIGH) return (link->flags & LINK_MAPPED) && is_net_row(link->row);
    /* A low connection is transparent when every high connection is. */
    size_t low = 0, high = g_vpi.port_link_count, found = 0;
    while (low < high) {
        size_t middle = low + (high - low) / 2u;
        if (g_vpi.links[g_vpi.port_links[middle]].source < link->source) low = middle + 1u;
        else high = middle;
    }
    for (size_t i = low; i < g_vpi.port_link_count; ++i) {
        const llg_vpi_link_t* other = &g_vpi.links[g_vpi.port_links[i]];
        if (other->source != link->source) break;
        if (!(other->flags & LINK_MAPPED) || !is_net_row(other->row)) return 0;
        ++found;
    }
    return found != 0;
}

/* Append a span unless one already listed covers it. */
static void group_push(llg_vpi_spans_t* group, uint32_t row, uint64_t element, uint32_t lsb,
                       uint32_t width) {
    for (size_t i = 0; i < group->count; ++i) {
        const llg_vpi_span_t* span = &group->items[i];
        if (span->row == row && span->element == element && span->lsb <= lsb &&
            (uint64_t)span->lsb + span->width >= (uint64_t)lsb + width)
            return;
    }
    if (group->count >= LLG_VPI_MAX_GROUP_SPANS) return;
    spans_push(group, row, element, lsb, width, 0, 0);
}

/* The bits of the nets a query span is connected to through ports whose
 * both sides are nets (the simulated net of 23.3.3.7), the span first. */
static void net_group(const llg_vpi_span_t* start, llg_vpi_spans_t* group) {
    spans_push(group, start->row, start->element, start->lsb, start->width, 0, 0);
    if (!is_net_row(start->row)) return;
    for (size_t at = 0; at < group->count && !group->failed; ++at) {
        llg_vpi_span_t span = group->items[at];
        for (size_t i = links_of(span.row); i < g_vpi.link_count; ++i) {
            const llg_vpi_link_t* link = &g_vpi.links[i];
            uint32_t from, to;
            if (link->row != span.row) break;
            if ((link->kind != LINK_PORT_HIGH && link->kind != LINK_PORT_LOW) ||
                !link_overlap(link, &span, &from, &to) || !port_link_transparent(link))
                continue;
            const llg_vpi_model_object_t* port = row_object(link->source);
            /* Port value bits [first, last). */
            uint32_t first = link->offset + (from - link->lsb), last = first + (to - from);
            if (link->kind == LINK_PORT_HIGH) {
                group_push(group, row_of(port->target), LLG_VPI_NO_ELEMENT, first, last - first);
                continue;
            }
            size_t low = 0, high = g_vpi.port_link_count;
            while (low < high) {
                size_t middle = low + (high - low) / 2u;
                if (g_vpi.links[g_vpi.port_links[middle]].source < link->source) low = middle + 1u;
                else high = middle;
            }
            for (size_t k = low; k < g_vpi.port_link_count; ++k) {
                const llg_vpi_link_t* outer = &g_vpi.links[g_vpi.port_links[k]];
                if (outer->source != link->source) break;
                uint32_t a = first > outer->offset ? first : outer->offset;
                uint64_t outer_end = (uint64_t)outer->offset + outer->width;
                uint32_t b = (uint64_t)last < outer_end ? last : (uint32_t)outer_end;
                if (a < b)
                    group_push(group, outer->row, outer->element,
                               outer->lsb + (a - outer->offset), b - a);
            }
        }
    }
}

/* The live force bindings installed by force statements, as spans: one list
 * per binding, `starts[i]` its first span. */
typedef struct {
    llg_vpi_spans_t spans;
    size_t* starts;
    size_t count;
} llg_vpi_forces_t;

static void forces_collect(llg_vpi_forces_t* forces) {
    memset(forces, 0, sizeof(*forces));
    if (!storage_index()) return;
    size_t capacity = 0;
    const llg_force_part_t* parts;
    int n_parts;
    double* real;
    for (int index = 0; llg_force_entry_view(index, &parts, &n_parts, &real); ++index) {
        if (!parts && !real) continue;
        if (forces->count == capacity) {
            capacity = capacity ? capacity * 2u : 4u;
            size_t* grown = (size_t*)realloc(forces->starts, (capacity + 1u) * sizeof(*grown));
            if (!grown) {
                forces->spans.failed = 1;
                return;
            }
            forces->starts = grown;
        }
        forces->starts[forces->count++] = forces->spans.count;
        uint32_t row;
        uint64_t element;
        if (real) {
            if (storage_row(real, &row, &element)) spans_push(&forces->spans, row, element, 0, 1, 0, 0);
            continue;
        }
        for (int i = 0; i < n_parts; ++i) {
            const llg_force_part_t* part = &parts[i];
            if (!part_location(part, &row, &element)) continue;
            int64_t low = part->right < part->left ? part->right : part->left;
            if (low < 0) continue;
            spans_push(&forces->spans, row, element, (uint32_t)low, part->width, 0, 0);
        }
    }
    if (forces->starts) forces->starts[forces->count] = forces->spans.count;
}

/* Whether two span lists cover the same bits (each a set of disjoint spans). */
static int same_bits(const llg_vpi_span_t* a, size_t a_count, const llg_vpi_span_t* b,
                     size_t b_count) {
    uint64_t a_bits = 0, b_bits = 0;
    for (size_t i = 0; i < a_count; ++i) a_bits += a[i].width;
    for (size_t i = 0; i < b_count; ++i) b_bits += b[i].width;
    if (a_bits != b_bits || !a_count) return 0;
    for (size_t i = 0; i < a_count; ++i) {
        uint64_t covered = 0;
        for (size_t k = 0; k < b_count; ++k) {
            if (a[i].row != b[k].row || a[i].element != b[k].element) continue;
            uint32_t from = a[i].lsb > b[k].lsb ? a[i].lsb : b[k].lsb;
            uint64_t a_end = (uint64_t)a[i].lsb + a[i].width, b_end = (uint64_t)b[k].lsb + b[k].width;
            uint64_t to = a_end < b_end ? a_end : b_end;
            if (from < to) covered += to - from;
        }
        if (covered != a[i].width) return 0;
    }
    return 1;
}

/* 37.16 details 11-12: a force statement counts while a binding it
 * installed is in effect, which is a live binding of exactly its target. */
static int force_active(uint32_t code, const llg_vpi_forces_t* forces) {
    const llg_vpi_code_t* node = code_at(code);
    uint32_t lhs = node ? code_child(node, vpiLhs) : LLG_VPI_NONE;
    if (lhs == LLG_VPI_NONE || !forces->count) return 0;
    llg_vpi_spans_t target;
    memset(&target, 0, sizeof(target));
    code_spans(lhs, 1, 0, &target, 0);
    int active = 0;
    for (size_t i = 0; !target.failed && !active && i < forces->count; ++i)
        active = same_bits(&forces->spans.items[forces->starts[i]],
                           forces->starts[i + 1] - forces->starts[i], target.items, target.count);
    free(target.items);
    return active;
}

/* The objects a connectivity iteration returns, deduplicated. */
typedef struct {
    uint8_t kind;      /* llg_vpi_slot_kind_t of the handle, SLOT_CODE for a force */
    uint32_t row;
    uint32_t aux;
} llg_vpi_link_item_t;

typedef struct {
    llg_vpi_link_item_t* items;
    size_t count;
    size_t capacity;
    int failed;
} llg_vpi_link_items_t;

static void items_add(llg_vpi_link_items_t* list, uint8_t kind, uint32_t row, uint32_t aux) {
    for (size_t i = 0; i < list->count; ++i)
        if (list->items[i].kind == kind && list->items[i].row == row && list->items[i].aux == aux)
            return;
    if (list->failed) return;
    if (list->count == list->capacity) {
        size_t capacity = list->capacity ? list->capacity * 2u : 8u;
        llg_vpi_link_item_t* grown =
            (llg_vpi_link_item_t*)realloc(list->items, capacity * sizeof(*grown));
        if (!grown) {
            list->failed = 1;
            return;
        }
        list->items = grown;
        list->capacity = capacity;
    }
    list->items[list->count].kind = kind;
    list->items[list->count].row = row;
    list->items[list->count].aux = aux;
    ++list->count;
}

/* The span of a net or variable handle and whether it is one bit (a
 * scalar or a bit select); 0 for another handle. */
static int slot_span(const llg_vpi_slot_t* slot, llg_vpi_span_t* span, int* bit) {
    const llg_vpi_model_object_t* object = row_object(slot->object);
    if (!is_connectable(object) || (object->flags & LLG_VPI_OBJ_DYNAMIC)) return 0;
    span->row = slot->object;
    span->element = LLG_VPI_NO_ELEMENT;
    span->lsb = 0;
    span->width = value_bits(object);
    span->offset = 0;
    span->mapped = 0;
    switch (slot->kind) {
        case SLOT_OBJECT:
            *bit = !is_array_type(object->type) && !object->is_real && object->width == 1;
            return 1;
        case SLOT_ELEMENT:
            span->element = slot->element;
            *bit = !object->is_real && object->width == 1;
            return 1;
        case SLOT_SELECT:
            /* A bit select is a net or variable bit; a part select is an
             * expression without connectivity relations. */
            if (slot->span != 1) return 0;
            span->element = slot->element;
            span->lsb = slot->lsb;
            span->width = 1;
            *bit = 1;
            return 1;
        case SLOT_MEMBER:
            if (!slot->span) return 0;
            span->element = slot->element;
            span->lsb = slot->lsb;
            span->width = slot->span;
            *bit = slot->span == 1;
            return 1;
        default:
            return 0;
    }
}

/* The instance a link's source sits in: a port's high connection is made in
 * the instance that instantiates the port's module. */
static uint32_t link_instance(const llg_vpi_link_t* link) {
    if (link->kind == LINK_FORCE) {
        const llg_vpi_code_t* node = code_at(link->source);
        return node && node->scope ? enclosing_instance(node->scope - 1u) : LLG_VPI_NONE;
    }
    if (link->kind == LINK_PORT_HIGH) {
        const llg_vpi_model_object_t* port = row_object(link->source);
        const llg_vpi_model_object_t* owner = port ? port->parent : NULL;
        return owner && owner->parent ? enclosing_instance(row_of(owner->parent)) : LLG_VPI_NONE;
    }
    return enclosing_instance(link->source);
}

/* Whether the target of continuous assignment `row` is a concatenation. */
static int lhs_is_concat(uint32_t row) {
    const llg_vpi_conn_t* conn = row_conn(row, 0u);
    if (!conn || conn->ref.ref_kind == LLG_VPI_ARG_OBJECT ||
        conn->ref.ref_kind == LLG_VPI_ARG_SELECT || conn->ref.ref_kind == LLG_VPI_ARG_ELEMENT ||
        !conn->ref.code || !g_vpi.code)
        return 0;
    const llg_vpi_code_t* node = code_at(code_visible(conn->ref.code - 1u));
    return node && node->type == vpiOperation && node->subtype == vpiConcatOp;
}

/* The handle item of a link reached through bit `bit` of a one-bit query
 * (`one_bit`; `select_bit` when it is a bit of a vector): a continuous
 * assignment to a vector or a part of one drives such a bit through one
 * of its bits (37.43), a vector port connects a one-bit query through a
 * port bit (37.16 detail 7). */
static int link_item(const llg_vpi_link_t* link, int one_bit, int select_bit, uint32_t bit,
                     llg_vpi_link_items_t* items) {
    const llg_vpi_model_object_t* source = row_object(link->source);
    uint32_t value_bit = link->offset + (bit - link->lsb);
    switch (link->kind) {
        case LINK_CONT_ASSIGN:
            if (select_bit && (link->flags & LINK_DRIVER) && (link->flags & LINK_MAPPED) &&
                !source->is_real && value_bit < source->width && !lhs_is_concat(link->source))
                items_add(items, SLOT_CABIT, link->source, value_bit);
            else
                items_add(items, SLOT_OBJECT, link->source, 0u);
            return 1;
        case LINK_PRIM_TERM:
            items_add(items, SLOT_PRIMTERM, link->source, link->aux);
            return 1;
        case LINK_PORT_HIGH:
        case LINK_PORT_LOW:
            if (link->flags & LINK_MAPPED) {
                /* 37.16 detail 8: bits beyond the port connect nothing. */
                if (value_bit >= source->width) return 0;
                if (one_bit && source->width > 1) {
                    items_add(items, SLOT_PORTBIT, link->source, value_bit);
                    return 1;
                }
            }
            items_add(items, SLOT_OBJECT, link->source, 0u);
            return 1;
        case LINK_FORCE:
            items_add(items, SLOT_CODE, link->source, 0u);
            return 1;
        default:
            items_add(items, SLOT_TERM, link->source, link->aux);
            return 1;
    }
}

static int compare_link_items(const void* left, const void* right) {
    const llg_vpi_link_item_t *a = (const llg_vpi_link_item_t*)left,
                              *b = (const llg_vpi_link_item_t*)right;
    int a_code = a->kind == SLOT_CODE, b_code = b->kind == SLOT_CODE;
    if (a_code != b_code) return a_code - b_code;
    if (a->row != b->row) return a->row < b->row ? -1 : 1;
    if (a->kind != b->kind) return a->kind < b->kind ? -1 : 1;
    return a->aux < b->aux ? -1 : a->aux > b->aux;
}

static vpiHandle link_items_iterator(int type, llg_vpi_link_items_t* items) {
    if (items->failed) {
        free(items->items);
        vpi_error("LLG_VPI_NOMEM", "VPI iterator allocation failed");
        return NULL;
    }
    if (!items->count) {
        free(items->items);
        return NULL;
    }
    /* Catalog order (declaration order), force statements last. */
    qsort(items->items, items->count, sizeof(*items->items), compare_link_items);
    uintptr_t* handles = (uintptr_t*)calloc(items->count, sizeof(*handles));
    if (!handles) {
        free(items->items);
        vpi_error("LLG_VPI_NOMEM", "VPI iterator allocation failed");
        return NULL;
    }
    uint32_t made = 0;
    for (size_t i = 0; i < items->count; ++i) {
        const llg_vpi_link_item_t* item = &items->items[i];
        vpiHandle handle = item->kind == SLOT_OBJECT ? object_handle(item->row)
                           : item->kind == SLOT_CODE ? code_handle(item->row)
                                                     : make_aux_slot((llg_vpi_slot_kind_t)item->kind,
                                                                     item->row, item->aux);
        if (handle) handles[made++] = (uintptr_t)handle;
    }
    free(items->items);
    return make_list_iterator(type, handles, made);
}

/* A net, net bit, net array or element of one (the nets of 37.16). */
static int net_handle(const llg_vpi_slot_t* slot) {
    const llg_vpi_model_object_t* object = row_object(slot->object);
    if (!object || !object->is_net || (object->type != vpiNet && object->type != vpiNetArray))
        return 0;
    return slot->kind == SLOT_OBJECT || slot->kind == SLOT_ELEMENT ||
           (slot->kind == SLOT_SELECT && slot->span == 1);
}

/* Whether an iteration is one of the connectivity relations of a net or
 * variable (37.16, 37.17). */
static int is_connectivity_iteration(int type) {
    return type == vpiDriver || type == vpiLoad || type == vpiLocalDriver ||
           type == vpiLocalLoad || type == vpiPortInst || type == vpiPort ||
           type == vpiPrimTerm || type == vpiContAssign || type == vpiPathTerm ||
           type == vpiTchkTerm;
}

/* vpiDriver, vpiLoad, vpiLocalDriver, vpiLocalLoad, vpiPortInst, vpiPort,
 * vpiPrimTerm, vpiContAssign, vpiPathTerm and vpiTchkTerm of a net, net
 * bit, net array, variable, variable bit, element or member (decisions
 * A19-D33, A19-D34). *handled is 0 for another handle. */
static vpiHandle iterate_connectivity(int type, const llg_vpi_slot_t* slot, int* handled) {
    llg_vpi_span_t query;
    int one_bit = 0;
    *handled = slot_span(slot, &query, &one_bit);
    const llg_vpi_model_object_t* object = row_object(slot->object);
    if (!*handled && slot->kind == SLOT_OBJECT && object && (object->flags & LLG_VPI_OBJ_DYNAMIC) &&
        type != vpiLocalDriver && type != vpiLocalLoad) {
        /* The index covers packed and real values: a string, handle or
         * container variable reports no connection. */
        *handled = 1;
        return NULL;
    }
    if (!*handled) return NULL;
    int select_bit = one_bit && (slot->kind == SLOT_SELECT || slot->kind == SLOT_MEMBER);
    /* 37.16 gives local drivers and loads to nets only. */
    if ((type == vpiLocalDriver || type == vpiLocalLoad) && !object->is_net) {
        vpi_error("LLG_VPI_RELATION", "local drivers and loads are relations of nets (37.16)");
        return NULL;
    }
    /* 37.16 detail 5: assignments and terminals only from scalars and bits. */
    if ((type == vpiContAssign || type == vpiPrimTerm) && !one_bit) return NULL;
    if (!links_ready()) return NULL;
    int local = type == vpiLocalDriver || type == vpiLocalLoad;
    uint8_t want = type == vpiDriver || type == vpiLocalDriver ? (uint8_t)LINK_DRIVER
                   : type == vpiLoad || type == vpiLocalLoad   ? (uint8_t)LINK_LOAD
                                                               : (uint8_t)0;
    llg_vpi_spans_t scope;
    memset(&scope, 0, sizeof(scope));
    /* 37.16 detail 4 and detail 19: assignments, terminals and the drivers
     * and loads of a net reach across the ports that join it to other nets. */
    if (!local && (want || type == vpiContAssign || type == vpiPrimTerm))
        net_group(&query, &scope);
    else
        spans_push(&scope, query.row, query.element, query.lsb, query.width, 0, 0);
    llg_vpi_forces_t forces;
    memset(&forces, 0, sizeof(forces));
    int forces_known = 0;
    uint32_t instance = local ? enclosing_instance(query.row) : LLG_VPI_NONE;
    llg_vpi_link_items_t items;
    memset(&items, 0, sizeof(items));
    for (size_t s = 0; s < scope.count && !scope.failed; ++s) {
        llg_vpi_span_t span = scope.items[s];
        for (size_t i = links_of(span.row); i < g_vpi.link_count; ++i) {
            const llg_vpi_link_t* link = &g_vpi.links[i];
            uint32_t from, to;
            if (link->row != span.row) break;
            if (!link_overlap(link, &span, &from, &to)) continue;
            int port = link->kind == LINK_PORT_HIGH || link->kind == LINK_PORT_LOW;
            switch (type) {
                case vpiPortInst:
                    if (link->kind != LINK_PORT_HIGH) continue;
                    break;
                case vpiPort:
                    if (link->kind != LINK_PORT_LOW) continue;
                    break;
                case vpiPrimTerm:
                    if (link->kind != LINK_PRIM_TERM) continue;
                    break;
                case vpiContAssign:
                    if (link->kind != LINK_CONT_ASSIGN) continue;
                    break;
                case vpiPathTerm:
                case vpiTchkTerm: {
                    if (link->kind != LINK_TERM) continue;
                    const llg_vpi_conn_t* conn = row_conn(link->source, link->aux);
                    int tchk = conn->role == LLG_VPI_ROLE_TCHK_REF ||
                               conn->role == LLG_VPI_ROLE_TCHK_DATA;
                    if (tchk != (type == vpiTchkTerm)) continue;
                    break;
                }
                default:
                    if (!(link->flags & want) || link->kind == LINK_TERM) continue;
                    /* 37.21: a variable's drivers are ports, forces and
                     * continuous assignments; its loads have no ports. */
                    if (!object->is_net &&
                        (link->kind == LINK_PRIM_TERM || (port && want == LINK_LOAD)))
                        continue;
                    /* A port joining two nets is not a driver or load of
                     * their simulated net (37.16 detail 13). */
                    if (!local && port && port_link_transparent(link)) continue;
                    if (local && link_instance(link) != instance) continue;
                    if (link->kind == LINK_FORCE) {
                        if (!forces_known) {
                            forces_collect(&forces);
                            forces_known = 1;
                        }
                        if (!force_active(link->source, &forces)) continue;
                    }
                    break;
            }
            link_item(link, one_bit, select_bit, from, &items);
        }
    }
    int failed = scope.failed || forces.spans.failed;
    free(scope.items);
    free(forces.spans.items);
    free(forces.starts);
    if (failed) items.failed = 1;
    return link_items_iterator(type, &items);
}

/* Whether a net or net bit is the target of a net declaration assignment
 * (37.16 vpiNetDeclAssign). */
static int net_decl_assigned(const llg_vpi_slot_t* slot) {
    llg_vpi_span_t query;
    int one_bit;
    if (!slot_span(slot, &query, &one_bit) || !links_ready()) return 0;
    for (size_t i = links_of(query.row); i < g_vpi.link_count; ++i) {
        const llg_vpi_link_t* link = &g_vpi.links[i];
        uint32_t from, to;
        if (link->row != query.row) break;
        if (link->kind == LINK_CONT_ASSIGN && (link->flags & LINK_DRIVER) &&
            (g_vpi.objects[link->source].flags & LLG_VPI_OBJ_QUALIFIER) &&
            link_overlap(link, &query, &from, &to))
            return 1;
    }
    return 0;
}

static int net_type_of(uint32_t row) {
    const llg_vpi_model_object_t* object = row_object(row);
    return object && object->type == vpiNet && object->subtype ? object->subtype : vpiWire;
}

/* Whether the internal net type dominates the external one across a port
 * (Table 23-1). */
static int internal_dominates(int internal, int external) {
    int w = external == vpiWire || external == vpiTri;
    int pulled = external == vpiWand || external == vpiTriAnd || external == vpiWor ||
                 external == vpiTriOr || external == vpiTriReg || external == vpiTri0 ||
                 external == vpiTri1;
    switch (internal) {
        case vpiWand: case vpiTriAnd: case vpiWor: case vpiTriOr: case vpiTriReg:
            return w;
        case vpiTri0: case vpiTri1:
            return w || external == vpiTriReg;
        case vpiUwire:
            return w || pulled;
        case vpiSupply0: case vpiSupply1:
            return w || pulled || external == vpiUwire;
        default:
            return 0;
    }
}

/* vpiResolvedNetType (37.16): the type of the simulated net a net belongs
 * to, each port joining two nets giving both the type Table 23-1 selects
 * (23.3.3.7, decision A19-D35). */
static int resolved_net_type(const llg_vpi_slot_t* slot) {
    llg_vpi_span_t query;
    int one_bit;
    int own = net_type_of(slot->object);
    if (!slot_span(slot, &query, &one_bit) || !links_ready()) return own;
    llg_vpi_spans_t group;
    memset(&group, 0, sizeof(group));
    net_group(&query, &group);
    size_t count = 0;
    uint32_t* rows = group.failed ? NULL : (uint32_t*)malloc((group.count + 1u) * sizeof(*rows));
    int* types = rows ? (int*)malloc((group.count + 1u) * sizeof(*types)) : NULL;
    if (!rows || !types) {
        free(group.items);
        free(rows);
        free(types);
        return own;
    }
    for (size_t at = 0; at < group.count; ++at) {
        size_t k = 0;
        while (k < count && rows[k] != group.items[at].row) ++k;
        if (k == count) {
            rows[count] = group.items[at].row;
            types[count++] = net_type_of(group.items[at].row);
        }
    }
    free(group.items);
    for (size_t pass = 0, changed = 1; changed && pass <= count; ++pass) {
        changed = 0;
        for (size_t r = 0; r < count; ++r) {
            for (size_t i = links_of(rows[r]); i < g_vpi.link_count; ++i) {
                const llg_vpi_link_t* link = &g_vpi.links[i];
                if (link->row != rows[r]) break;
                if (link->kind != LINK_PORT_LOW || !port_link_transparent(link)) continue;
                for (size_t k = 0; k < g_vpi.port_link_count; ++k) {
                    const llg_vpi_link_t* outer = &g_vpi.links[g_vpi.port_links[k]];
                    if (outer->source != link->source) continue;
                    size_t h = 0;
                    while (h < count && rows[h] != outer->row) ++h;
                    if (h == count || types[h] == types[r]) continue;
                    int type = internal_dominates(types[r], types[h]) ? types[r] : types[h];
                    types[r] = types[h] = type;
                    changed = 1;
                }
            }
        }
    }
    int resolved = own;
    for (size_t r = 0; r < count; ++r)
        if (rows[r] == slot->object) resolved = types[r];
    free(rows);
    free(types);
    return resolved;
}

/* vpiLhs of a continuous assignment bit (37.43): bit `bit` of the
 * assignment's target, or the whole target when it has no such bit. */
static vpiHandle cont_assign_bit_lhs(uint32_t row, uint32_t bit) {
    if (links_ready()) {
        for (size_t i = 0; i < g_vpi.link_count; ++i) {
            const llg_vpi_link_t* link = &g_vpi.links[i];
            if (link->kind != LINK_CONT_ASSIGN || link->source != row ||
                !(link->flags & LINK_DRIVER) || !(link->flags & LINK_MAPPED) ||
                bit < link->offset || bit - link->offset >= link->width)
                continue;
            llg_vpi_model_object_t* target = row_object(link->row);
            uint32_t offset = link->lsb + (bit - link->offset);
            if (target->is_real || value_bits(target) == 1u)
                return link->element == LLG_VPI_NO_ELEMENT
                           ? object_handle(link->row)
                           : make_element(link->row, link->element, NULL);
            int64_t index = declared_index(target, offset);
            return make_select(link->row, link->element, offset, 1, index, index, NULL);
        }
    }
    return conn_handle(row, 0u);
}

/* ── iteration ──────────────────────────────────────────────────────────── */

static int child_matches(const llg_vpi_model_object_t* child, int type) {
    switch (type) {
        case vpiVariables:
            /* 37.12: every variable, string, chandle, class and container
             * variables included (37.17). */
            return is_variable_type(child->type) || child->type == vpiRegArray ||
                   child->type == vpiStringVar || child->type == vpiChandleVar ||
                   child->type == vpiClassVar || child->type == vpiArrayVar;
        case vpiInternalScope:
            /* 37.12: the instances and clocking blocks below are scopes too
             * (decision A19-D30). */
            return (is_instance_type(child->type) && child->type != vpiPackage) ||
                   child->type == vpiGenScope || child->type == vpiClockingBlock ||
                   child->type == vpiNamedBegin ||
                   child->type == vpiNamedFork || child->type == vpiTask ||
                   child->type == vpiFunction || is_statement_scope_type(child->type);
        case vpiModule:
            return child->type == vpiModule;
        case vpiInstance:
            return is_instance_type(child->type);
        case vpiProcess:
            return is_process_type(child->type);
        case vpiTaskFunc:
            return child->type == vpiTask || child->type == vpiFunction;
        case vpiPrimitive:
            return is_primitive_type(child->type);
        case vpiPrimitiveArray:
            return child->type == vpiGateArray || child->type == vpiSwitchArray ||
                   child->type == vpiUdpArray;
        case vpiInstanceArray:
            return is_instance_array_type(child->type);
        default:
            return child->type == type;
    }
}

static int iteration_type_supported(int type) {
    switch (type) {
        case vpiModule: case vpiInterface: case vpiNet: case vpiReg:
        case vpiRealVar: case vpiIntegerVar: case vpiTimeVar: case vpiLongIntVar:
        case vpiShortIntVar: case vpiIntVar: case vpiShortRealVar: case vpiByteVar:
        case vpiEnumVar: case vpiStructVar: case vpiUnionVar: case vpiBitVar:
        case vpiPackedArrayVar: case vpiRegArray: case vpiNetArray:
        case vpiVariables: case vpiInternalScope: case vpiParameter: case vpiPort:
        case vpiGenScope: case vpiNamedBegin: case vpiNamedFork:
        case vpiNamedEvent: case vpiProcess: case vpiContAssign: case vpiTaskFunc:
        case vpiTask: case vpiFunction: case vpiPrimitive: case vpiGate: case vpiSwitch:
        case vpiUdp: case vpiModuleArray: case vpiInterfaceArray: case vpiPrimitiveArray:
        case vpiGateArray: case vpiSwitchArray: case vpiUdpArray: case vpiInstanceArray:
        case vpiModport: case vpiModPath: case vpiTchk: case vpiProgram: case vpiProgramArray:
        case vpiClockingBlock: case vpiClockingIODecl:
            return 1;
        default:
            return 0;
    }
}

/* The iterations of a package: the instance items and scope members of
 * 37.10 and 37.12, not the ports, processes, instances and structure of
 * modules and interfaces (26.2: a package declares no hierarchy). */
static int package_iteration(int type) {
    switch (type) {
        case vpiPort: case vpiProcess: case vpiContAssign: case vpiModule: case vpiModuleArray:
        case vpiInterface: case vpiInterfaceArray: case vpiPrimitive: case vpiPrimitiveArray:
        case vpiGate: case vpiSwitch: case vpiUdp: case vpiGateArray: case vpiSwitchArray:
        case vpiUdpArray: case vpiInstanceArray: case vpiModport: case vpiModPath: case vpiTchk:
        case vpiGenScope: case vpiClockingBlock:
            return 0;
        default:
            return 1;
    }
}

static vpiHandle make_list_iterator(int type, uintptr_t* items, uint32_t count) {
    if (count == 0) {
        free(items);
        return NULL;
    }
    uint32_t index = alloc_slot(SLOT_ITERATOR);
    if (index == LLG_VPI_NONE) {
        free(items);
        return NULL;
    }
    llg_vpi_slot_t* slot = &g_vpi.slots[index];
    slot->iter_kind = ITER_LIST;
    slot->items = items;
    slot->count = count;
    slot->aux = type;
    return encode_handle(index);
}

static int compare_ports(const void* left, const void* right) {
    uint32_t a = (uint32_t)(*(const uintptr_t*)left & LLG_VPI_INDEX_MASK) - 1u;
    uint32_t b = (uint32_t)(*(const uintptr_t*)right & LLG_VPI_INDEX_MASK) - 1u;
    uint32_t pa = g_vpi.objects[a].port_index, pb = g_vpi.objects[b].port_index;
    return pa < pb ? -1 : pa > pb;
}

static vpiHandle iterate_children(int type, uint32_t parent) {
    size_t capacity = 16, count = 0;
    uintptr_t* items = (uintptr_t*)malloc(capacity * sizeof(*items));
    if (!items) {
        vpi_error("LLG_VPI_NOMEM", "VPI iterator allocation failed");
        return NULL;
    }
    if (parent == LLG_VPI_NONE) {
        for (size_t row = 0; row < g_vpi.object_count; ++row) {
            llg_vpi_model_object_t* object = &g_vpi.objects[row];
            if (object->parent || !child_matches(object, type)) continue;
            if (count == capacity) {
                uintptr_t* grown = (uintptr_t*)realloc(items, capacity * 2u * sizeof(*items));
                if (!grown) break;
                items = grown;
                capacity *= 2u;
            }
            items[count++] = (uintptr_t)object_handle((uint32_t)row);
        }
    } else {
        for (uint32_t child = g_vpi.first_child[parent]; child;
             child = g_vpi.next_sibling[child - 1u]) {
            if (!child_matches(&g_vpi.objects[child - 1u], type)) continue;
            if (count == capacity) {
                uintptr_t* grown = (uintptr_t*)realloc(items, capacity * 2u * sizeof(*items));
                if (!grown) break;
                items = grown;
                capacity *= 2u;
            }
            items[count++] = (uintptr_t)object_handle(child - 1u);
        }
    }
    if ((type == vpiPort || type == vpiClockingIODecl || type == vpiIODecl) && count > 1)
        qsort(items, count, sizeof(*items), compare_ports);
    return make_list_iterator(type, items, (uint32_t)count);
}

/* vpiMember (37.24): the member rows of an unpacked structure or union kept
 * as member storage, in declaration order, or the member nodes of a packed
 * value, an element of an array of structures, or an enclosing member. */
static vpiHandle iterate_members(const llg_vpi_slot_t* slot) {
    llg_vpi_model_object_t* object = row_object(slot->object);
    const llg_vpi_typespec_t* typespec = value_typespec(slot);
    if (slot->kind == SLOT_OBJECT && object && (object->flags & LLG_VPI_OBJ_RECORD)) {
        uint32_t row = slot->object;
        /* Without a typespec the rows keep catalog order. */
        if (!typespec) {
            uint32_t children = 0;
            for (uint32_t child = g_vpi.first_child[row]; child; child = g_vpi.next_sibling[child - 1u])
                ++children;
            uintptr_t* items = (uintptr_t*)calloc(children + 1u, sizeof(*items));
            if (!items) {
                vpi_error("LLG_VPI_NOMEM", "VPI iterator allocation failed");
                return NULL;
            }
            uint32_t count = 0;
            for (uint32_t child = g_vpi.first_child[row]; child; child = g_vpi.next_sibling[child - 1u])
                if (g_vpi.objects[child - 1u].flags & LLG_VPI_OBJ_MEMBER)
                    items[count++] = (uintptr_t)object_handle(child - 1u);
            return make_list_iterator(vpiMember, items, count);
        }
        uintptr_t* items = (uintptr_t*)calloc(typespec->member_count + 1u, sizeof(*items));
        if (!items) {
            vpi_error("LLG_VPI_NOMEM", "VPI iterator allocation failed");
            return NULL;
        }
        uint32_t count = 0;
        size_t length = strlen(object->full_name);
        for (uint32_t i = 0; i < typespec->member_count; ++i) {
            const char* name = g_vpi.types->members[typespec->first_member + i].name;
            size_t total = length + 1u + strlen(name) + 1u;
            char* full = (char*)malloc(total);
            if (!full) break;
            snprintf(full, total, "%s.%s", object->full_name, name);
            uint32_t member = find_row(full);
            free(full);
            /* Members kept outside the catalog (unpacked arrays, strings,
             * handles) have no row. */
            if (member != LLG_VPI_NONE && row_object(member)->parent == object)
                items[count++] = (uintptr_t)object_handle(member);
        }
        return make_list_iterator(vpiMember, items, count);
    }
    if ((slot->kind != SLOT_OBJECT && slot->kind != SLOT_ELEMENT && slot->kind != SLOT_MEMBER) ||
        !is_aggregate_typespec(typespec)) {
        vpi_error("LLG_VPI_RELATION", "vpiMember iterates a structure or union variable");
        return NULL;
    }
    if (object && object->is_net) {
        /* Members of a struct net are nets (37.16); llg exposes none. */
        vpi_error("LLG_VPI_UNSUPPORTED", "members of a structure or union net are not VPI objects");
        return NULL;
    }
    uint32_t first = typespec->first_node, count = typespec->node_count;
    if (slot->kind == SLOT_MEMBER) {
        const llg_vpi_member_node_t* node = node_at((uint32_t)slot->aux);
        first = node->first_child;
        count = node->child_count;
    }
    if (!count) {
        if (typespec->member_count)
            vpi_error("LLG_VPI_UNSUPPORTED", "the members of this value are not VPI objects");
        return NULL;
    }
    uint32_t row = slot->object;
    uint64_t element = slot->element;
    uint32_t index = alloc_slot(SLOT_ITERATOR);
    if (index == LLG_VPI_NONE) return NULL;
    llg_vpi_slot_t* iterator = &g_vpi.slots[index];
    iterator->iter_kind = ITER_MEMBERS;
    iterator->object = row;
    iterator->element = element;
    iterator->left = first;
    iterator->count = count;
    iterator->aux = vpiMember;
    return encode_handle(index);
}

/* vpiTypespecMember, vpiEnumConst and vpiRange of a typespec (37.23). */
static vpiHandle iterate_typespec(const llg_vpi_slot_t* slot, int type) {
    const llg_vpi_typespec_t* typespec = typespec_at((uint32_t)slot->aux);
    uint32_t count, base;
    if (type == vpiTypespecMember && is_aggregate_typespec(typespec)) {
        count = typespec->member_count;
        base = g_vpi.tsmember_base + typespec->first_member;
    } else if (type == vpiEnumConst && typespec->type == vpiEnumTypespec) {
        count = typespec->const_count;
        base = g_vpi.enumconst_base + typespec->first_const;
    } else if (type == vpiRange && (typespec->type == vpiLogicTypespec ||
                                    typespec->type == vpiBitTypespec ||
                                    typespec->type == vpiPackedArrayTypespec ||
                                    typespec->type == vpiArrayTypespec)) {
        count = typespec->range_count;
        base = LLG_VPI_NONE;
    } else {
        vpi_error("LLG_VPI_RELATION", "unsupported VPI one-to-many relation for this typespec");
        return NULL;
    }
    if (!count) return NULL;
    uintptr_t* items = (uintptr_t*)calloc(count, sizeof(*items));
    if (!items) {
        vpi_error("LLG_VPI_NOMEM", "VPI iterator allocation failed");
        return NULL;
    }
    uint32_t made = 0;
    for (uint32_t i = 0; i < count; ++i) {
        if (base != LLG_VPI_NONE) {
            items[made++] = (uintptr_t)encode_handle(base + i);
            continue;
        }
        uint32_t range = alloc_slot(SLOT_RANGE);
        if (range == LLG_VPI_NONE) break;
        const int64_t* bounds = &g_vpi.types->ranges[2u * (typespec->first_range + i)];
        g_vpi.slots[range].left = bounds[0];
        g_vpi.slots[range].right = bounds[1];
        g_vpi.slots[range].aux = (int32_t)i;
        items[made++] = (uintptr_t)encode_handle(range);
    }
    return make_list_iterator(type, items, made);
}

vpiHandle vpi_iterate(PLI_INT32 type, vpiHandle ref_handle) {
    vpi_enter();
    if (!vpi_ready()) return NULL;
    g_vpi.iterate_ref = (uintptr_t)ref_handle;
    {
        int handled;
        vpiHandle result = rts_iterate(type, ref_handle, &handled);
        if (handled) return result;
    }
    if (type == vpiAssertion) return iterate_assertions(ref_handle);
    if (type == vpiConcurrentAssertions) return iterate_concurrent_assertions(ref_handle);
    if (type == vpiFsm) return iterate_fsms(ref_handle);
    if (type == vpiFsmStates) return iterate_fsm_states(ref_handle);
    if (type == vpiSequenceDecl || type == vpiPropertyDecl) return iterate_declarations(type, ref_handle);
    if (type == vpiClassDefn) return iterate_class_defns(ref_handle);
    if (!ref_handle) {
        /* 37.5 detail 1, 37.10: the top-level instances, packages and
         * `$unit` included. */
        if (type != vpiModule && type != vpiInterface && type != vpiProgram &&
            type != vpiPackage && type != vpiInstance) {
            vpi_error("LLG_VPI_UNSUPPORTED",
                      "only instances iterate from a NULL reference");
            return NULL;
        }
        return iterate_children(type, LLG_VPI_NONE);
    }
    uint32_t ref_index;
    llg_vpi_slot_t* slot = checked_handle(ref_handle, &ref_index);
    if (!slot) return NULL;
    if (slot->kind == SLOT_IMP) {
        if (type == vpiPort) return iterate_imp_ports(slot);
        vpi_error("LLG_VPI_RELATION", "an intermodule path iterates only its ports (37.33)");
        return NULL;
    }
    if (type == vpiArgument && slot->kind != SLOT_CODE) {
        if (slot->kind != SLOT_CALL) {
            vpi_error("LLG_VPI_RELATION", "vpiArgument iterates a system task or function call");
            return NULL;
        }
        llg_vpi_callsite_t* site = (llg_vpi_callsite_t*)slot->pointer;
        llg_vpi_call_t* call = site->active;
        if (!call) {
            vpi_error("LLG_VPI_RELATION", "arguments are available only while the call is active");
            return NULL;
        }
        if (call->arg_count <= 0) return NULL;
        uint32_t index = alloc_slot(SLOT_ITERATOR);
        if (index == LLG_VPI_NONE) return NULL;
        llg_vpi_slot_t* iterator = &g_vpi.slots[index];
        iterator->iter_kind = ITER_ARGUMENTS;
        iterator->pointer = site;
        iterator->count = (uint32_t)call->arg_count;
        iterator->aux = vpiArgument;
        own_by_call(index, call);
        return encode_handle(index);
    }
    if (type == vpiMember && slot->kind == SLOT_COLLECTION) return iterate_collection(slot);
    if (type == vpiIndex && has_index_relation(slot)) return iterate_indices(slot);
    if (dyn_slot(slot)) {
        int handled;
        vpiHandle result = dyn_iterate(type, slot, &handled);
        if (handled) return result;
    }
    if (type == vpiMember) return iterate_members(slot);
    if (slot->kind == SLOT_TYPESPEC) return iterate_typespec(slot, type);
    if (slot->kind == SLOT_CODE) return iterate_code((uint32_t)slot->aux, type);
    uint32_t argument = argument_code(slot);
    if (argument != LLG_VPI_NONE) return iterate_code(argument, type);
    llg_vpi_model_object_t* object = row_object(slot->object);
    if (is_connectivity_iteration(type)) {
        int handled;
        vpiHandle result = iterate_connectivity(type, slot, &handled);
        if (handled) return result;
    }
    if (type == vpiBit && slot->kind == SLOT_OBJECT && object && object->type == vpiContAssign)
        /* 37.43: the bits of an assignment to a vector, from vpiOffset 0
         * (decision A19-D34). */
        return object->width && !object->is_real
                   ? iterate_aux(SLOT_CABIT, slot->object, object->width, type, 0)
                   : NULL;
    if (slot->kind == SLOT_OBJECT && object && is_statement_scope_type(object->type)) {
        /* 37.12: a statement scope iterates its declarations; the other
         * iterations are its statement's (37.69, 37.70). */
        if (is_scope_iteration(type)) return iterate_children(type, slot->object);
        uint32_t code = statement_scope_code(object);
        return code == LLG_VPI_NONE ? NULL : iterate_code(code, type);
    }
    if (type == vpiStmt && slot->kind == SLOT_OBJECT && object &&
        (object->type == vpiNamedBegin || object->type == vpiNamedFork)) {
        /* 37.12: the statements of a named block. */
        if (!g_vpi.code) {
            vpi_error("LLG_VPI_UNSUPPORTED",
                      "statements are objects only in models generated with --vpi or --dpi-lib");
            return NULL;
        }
        return object->code ? iterate_code(object->code - 1u, type) : NULL;
    }
    if (slot->kind == SLOT_OBJECT && object && is_array_type(object->type) &&
        (type == vpiRange || type == vpiReg || type == vpiNet || type == vpiMemoryWord ||
         type == vpiVarSelect || type == object->subtype)) {
        if (type == vpiRange) return iterate_dims(slot->object, type);
        /* 37.16, 37.17: vpiNet iterates the elements of a net array and
         * vpiReg those of a variable array, whatever their type; vpiVarSelect
         * the elements of a variable array (37.17 detail 7, decision
         * A19-D28). */
        if (type != object->subtype && type != vpiMemoryWord &&
            !((type == vpiReg || type == vpiVarSelect) && object->type == vpiRegArray) &&
            !(type == vpiNet && object->type == vpiNetArray)) {
            vpi_error("LLG_VPI_RELATION", "this array has no elements of the requested type");
            return NULL;
        }
        uint32_t index = alloc_slot(SLOT_ITERATOR);
        if (index == LLG_VPI_NONE) return NULL;
        llg_vpi_slot_t* iterator = &g_vpi.slots[index];
        iterator->iter_kind = ITER_ELEMENTS;
        iterator->object = g_vpi.slots[ref_index].object;
        iterator->count = (uint32_t)(g_vpi.element_counts[iterator->object] > UINT32_MAX
                                         ? UINT32_MAX : g_vpi.element_counts[iterator->object]);
        iterator->aux = type;
        return encode_handle(index);
    }
    if (slot->kind == SLOT_OBJECT && object && is_instance_array_type(object->type)) {
        /* 37.11: the ranges and the instances of an instance array. */
        if (type == vpiRange) return iterate_dims(slot->object, type);
        if (type == object->detail || type == vpiInstance ||
            (type == vpiPrimitive && is_primitive_type(object->detail)))
            return iterate_instance_elements(slot->object, type);
        vpi_error("LLG_VPI_RELATION", "an instance array iterates vpiRange and its instances");
        return NULL;
    }
    if (slot->kind == SLOT_OBJECT && object && object->type == vpiModPath) {
        /* 37.35: the input, output and data terminals. */
        int role = type == vpiModPathIn ? LLG_VPI_ROLE_PATH_IN
                   : type == vpiModPathOut ? LLG_VPI_ROLE_PATH_OUT
                   : type == vpiModDataPathIn ? LLG_VPI_ROLE_PATH_DATA : -1;
        if (role >= 0) return iterate_roles(slot->object, type, 1u << role);
        vpi_error("LLG_VPI_RELATION",
                  "a module path iterates vpiModPathIn, vpiModPathOut and vpiModDataPathIn");
        return NULL;
    }
    if (type == vpiIODecl || type == vpiGenScopeArray || type == vpiSpecParam ||
        (slot->kind == SLOT_OBJECT && object && object->type == vpiGenScopeArray)) {
        /* 37.13: the io declarations of a module, task, function or
         * modport in declaration order; 37.80: the generate scope arrays of
         * a module or generate scope, the scopes of an array; 37.10: the
         * specparams of an instance. */
        int allowed = slot->kind == SLOT_OBJECT && object &&
                      (object->type == vpiGenScopeArray
                           ? type == vpiGenScope
                       : type == vpiIODecl
                           ? object->type == vpiModule || object->type == vpiTask ||
                                 object->type == vpiFunction || object->type == vpiModport
                       : type == vpiSpecParam
                           ? object->type == vpiModule || object->type == vpiInterface ||
                                 object->type == vpiProgram || object->type == vpiPackage
                           : object->type == vpiModule || object->type == vpiGenScope);
        if (!allowed) {
            vpi_error("LLG_VPI_RELATION", "unsupported VPI one-to-many relation for this object");
            return NULL;
        }
        if (object->type == vpiGenScopeArray) return iterate_gen_scopes(slot->object);
        return iterate_children(type, slot->object);
    }
    if (slot->kind == SLOT_OBJECT && object && object->type == vpiTchk) {
        /* 37.36 detail 2: the arguments in order, the events as tchk terms. */
        if (type == vpiExpr)
            return iterate_roles(slot->object, type,
                                 1u << LLG_VPI_ROLE_TCHK_REF | 1u << LLG_VPI_ROLE_TCHK_DATA |
                                     1u << LLG_VPI_ROLE_ARGUMENT | 1u << LLG_VPI_ROLE_NOTIFIER);
        vpi_error("LLG_VPI_RELATION", "a timing check iterates vpiExpr");
        return NULL;
    }
    if (slot->kind == SLOT_OBJECT && object && is_primitive_type(object->type)) {
        if (type == vpiPrimTerm) return iterate_aux(SLOT_PRIMTERM, slot->object, object->conn_count, type, 0);
        vpi_error("LLG_VPI_RELATION", "a primitive iterates vpiPrimTerm");
        return NULL;
    }
    if (type == vpiBit && slot->kind == SLOT_OBJECT && object && object->type == vpiPort) {
        /* 37.14: the bits of a port; an interface port has none. */
        if (object->detail) {
            vpi_error("LLG_VPI_RELATION", "an interface or modport port has no bits");
            return NULL;
        }
        return iterate_aux(SLOT_PORTBIT, slot->object, object->width, type, 1);
    }
    if (type == vpiRange && (slot->kind == SLOT_OBJECT || slot->kind == SLOT_ELEMENT) &&
        object && !(slot->kind == SLOT_OBJECT && is_array_type(object->type))) {
        /* 37.16 detail 23, 37.17 detail 4: the packed ranges of a logic or
         * bit vector or a packed array. */
        int value_type = handle_type(slot);
        if (value_type == vpiNet || value_type == vpiReg || value_type == vpiBitVar ||
            value_type == vpiPackedArrayVar || value_type == vpiIODecl)
            return iterate_packed_ranges(slot);
    }
    if (type == vpiBit && (slot->kind == SLOT_OBJECT || slot->kind == SLOT_ELEMENT) && object &&
        !object->is_real && object->packed &&
        !(slot->kind == SLOT_OBJECT && is_array_type(object->type))) {
        uint64_t element = slot->element;
        uint32_t row = slot->object;
        uint32_t index = alloc_slot(SLOT_ITERATOR);
        if (index == LLG_VPI_NONE) return NULL;
        llg_vpi_slot_t* iterator = &g_vpi.slots[index];
        iterator->iter_kind = ITER_BITS;
        iterator->object = row;
        iterator->element = element;
        /* The catalog never moves, so `object` survives the slot growth. */
        iterator->count = object->width;
        iterator->aux = vpiBit;
        return encode_handle(index);
    }
    if (slot->kind == SLOT_OBJECT && object && is_scope_type(object->type)) {
        if (!iteration_type_supported(type)) {
            vpi_error("LLG_VPI_UNSUPPORTED", "unsupported VPI iteration type for a scope");
            return NULL;
        }
        /* 37.10, 37.13: instances and generate scopes hold programs and
         * clocking blocks; 37.44: a clocking block holds io declarations
         * and its scope's declarations only. */
        if ((type == vpiProgram || type == vpiProgramArray || type == vpiClockingBlock) &&
            !is_instance_type(object->type) && object->type != vpiGenScope) {
            vpi_error("LLG_VPI_RELATION",
                      "only instances and generate scopes hold programs and clocking blocks");
            return NULL;
        }
        if ((type == vpiClockingIODecl) != (object->type == vpiClockingBlock) &&
            !(object->type == vpiClockingBlock && is_scope_iteration(type))) {
            vpi_error("LLG_VPI_RELATION", "unsupported VPI one-to-many relation for this object");
            return NULL;
        }
        if (object->type == vpiPackage && !package_iteration(type)) {
            vpi_error("LLG_VPI_RELATION",
                      "a package holds no ports, processes, instances or structure (37.10)");
            return NULL;
        }
        return iterate_children(type, slot->object);
    }
    vpi_error("LLG_VPI_UNSUPPORTED", "unsupported VPI one-to-many relation for this object");
    return NULL;
}

/* Handle for argument `position` of an active call. Object arguments are
 * the catalog objects themselves (writable); selects and elements become
 * call-owned handles; other expressions are call-owned value snapshots. */
static vpiHandle argument_handle(llg_vpi_callsite_t* site, llg_vpi_call_t* call, int position) {
    const llg_vpi_compile_arg_t* shape = position < site->arg_count ? &site->args[position] : NULL;
    if (shape && shape->object >= 0 && (uint64_t)shape->object < g_vpi.object_count) {
        uint32_t row = (uint32_t)shape->object;
        switch (shape->ref_kind) {
            case LLG_VPI_ARG_OBJECT:
                return object_handle(row);
            case LLG_VPI_ARG_SELECT:
                return make_select(row, LLG_VPI_NO_ELEMENT, (uint32_t)shape->offset,
                                   shape->select_width, shape->left, shape->right, call);
            case LLG_VPI_ARG_ELEMENT:
                if (shape->offset < g_vpi.element_counts[row])
                    return make_element(row, shape->offset, call);
                break;
            default:
                break;
        }
    }
    if (shape && shape->ref_kind == LLG_VPI_ARG_EXPRESSION && shape->code && g_vpi.code &&
        shape->code <= g_vpi.code->node_count) {
        /* A whole object converted to the formal's type is that object:
         * implicit conversions are not objects (decision A19-D12). */
        const llg_vpi_code_t* node = code_at(code_visible(shape->code - 1u));
        if ((node->flags & LLG_VPI_CODE_ALIAS) && node->object >= 0)
            return object_handle((uint32_t)node->object);
    }
    uint32_t index = alloc_slot(SLOT_ARGUMENT);
    if (index == LLG_VPI_NONE) return NULL;
    llg_vpi_slot_t* slot = &g_vpi.slots[index];
    slot->pointer = site;
    slot->aux = position;
    slot->count = shape && shape->ref_kind == LLG_VPI_ARG_CONSTANT ? 1u : 0u;
    slot->left = shape ? shape->const_type : 0;
    own_by_call(index, call);
    return encode_handle(index);
}

vpiHandle vpi_scan(vpiHandle iterator_handle) {
    vpi_enter();
    uint32_t index;
    llg_vpi_slot_t* slot = checked_handle(iterator_handle, &index);
    if (!slot) return NULL;
    if (slot->kind != SLOT_ITERATOR) {
        vpi_error("LLG_VPI_HANDLE", "vpi_scan needs an iterator handle");
        return NULL;
    }
    if (slot->iter_kind == ITER_CODE) return scan_code(index);
    if (slot->position >= slot->count) {
        /* 38.40: the iterator is freed once vpi_scan returns NULL. */
        release_slot(index);
        return NULL;
    }
    uint32_t position = slot->position++;
    switch (slot->iter_kind) {
        case ITER_LIST:
            return (vpiHandle)slot->items[position];
        case ITER_ELEMENTS:
            return make_element(slot->object, position, NULL);
        case ITER_BITS: {
            uint32_t row = slot->object;
            uint64_t element = slot->element;
            llg_vpi_model_object_t* object = row_object(row);
            int64_t left, right;
            packed_range(object, &left, &right);
            /* Bits are returned from the left (most significant) index. */
            uint32_t offset = object->width - 1u - position;
            int64_t sv_index = left >= right ? right + (int64_t)offset : right - (int64_t)offset;
            return make_select(row, element, offset, 1, sv_index, sv_index, NULL);
        }
        case ITER_MEMBERS:
            return make_member(slot->object, slot->element, (uint32_t)slot->left + position);
        case ITER_ARGUMENTS: {
            llg_vpi_callsite_t* site = (llg_vpi_callsite_t*)slot->pointer;
            if (!site->active) {
                vpi_error("LLG_VPI_RELATION", "the call of this argument iterator has returned");
                return NULL;
            }
            return argument_handle(site, site->active, (int)position);
        }
        case ITER_DYN:
            return dyn_scan(slot, position);
        default:
            return NULL;
    }
}

/* ── properties ─────────────────────────────────────────────────────────── */

static int log10_unit(uint64_t fs) {
    int exponent = -15;
    while (fs >= 10u && fs % 10u == 0) {
        fs /= 10u;
        ++exponent;
    }
    return exponent;
}

static uint64_t time_unit_of(const llg_vpi_model_object_t* object) {
    for (; object; object = object->parent)
        if (object->time_unit_fs) return object->time_unit_fs;
    return 0;
}

static int argument_type(const llg_vpi_slot_t* slot) {
    if (slot->count) return vpiConstant;
    return vpiOperation;
}

/* A catalog variable, or an element or bit of one: a bit or element shares
 * the declaration qualifiers of its variable (37.17 groups var bits with
 * the variables). */
static int is_static_variable_part(const llg_vpi_slot_t* slot,
                                   const llg_vpi_model_object_t* object) {
    if (!object || object->is_net) return 0;
    if (!is_variable_type(object->type) && object->type != vpiRegArray &&
        object->type != vpiStringVar && object->type != vpiChandleVar &&
        object->type != vpiArrayVar)
        return 0;
    return slot->kind == SLOT_OBJECT || slot->kind == SLOT_ELEMENT ||
           (slot->kind == SLOT_SELECT && slot->span == 1);
}

static int handle_type(const llg_vpi_slot_t* slot) {
    if (dyn_slot(slot)) return dyn_type(slot);
    if (rts_slot(slot)) return rts_type(slot);
    llg_vpi_model_object_t* object = row_object(slot->object);
    switch (slot->kind) {
        case SLOT_OBJECT: return object->type;
        case SLOT_ELEMENT: return value_type_of(object, slot->element);
        case SLOT_SELECT:
            if (slot->span != 1) return vpiPartSelect;
            return object->is_net ? vpiNetBit : vpiRegBit;
        case SLOT_CONSTANT: return vpiConstant;
        case SLOT_RANGE: return vpiRange;
        case SLOT_ITERATOR: return vpiIterator;
        case SLOT_SYSTF: return vpiUserSystf;
        case SLOT_CALL: {
            llg_vpi_callsite_t* site = (llg_vpi_callsite_t*)slot->pointer;
            return site->registration && site->registration->data.type == vpiSysFunc
                       ? vpiSysFuncCall : vpiSysTaskCall;
        }
        case SLOT_ARGUMENT: {
            uint32_t code = argument_code(slot);
            return code != LLG_VPI_NONE ? code_type(code_at(code)) : argument_type(slot);
        }
        case SLOT_CODE: return code_type(code_at((uint32_t)slot->aux));
        case SLOT_COLLECTION: return vpiCollection;
        case SLOT_CALLBACK:
        case SLOT_FIRED_CB: return vpiCallback;
        case SLOT_EVENT: return vpiSchedEvent;
        case SLOT_ASSERTION: return assertion_type(slot_item(slot));
        case SLOT_FSM: return vpiFsm;
        case SLOT_FSMSTATE: return vpiFsmHandle;
        case SLOT_MEMBER: {
            const llg_vpi_member_node_t* node = node_at((uint32_t)slot->aux);
            return node ? node->type : vpiUndefined;
        }
        case SLOT_TYPESPEC: return typespec_at((uint32_t)slot->aux)->type;
        case SLOT_TSMEMBER: return vpiTypespecMember;
        case SLOT_ENUMCONST: return vpiEnumConst;
        case SLOT_CONN: {
            const llg_vpi_conn_t* conn = row_conn(slot->object, (uint32_t)slot->aux);
            return conn && conn->ref.ref_kind == LLG_VPI_ARG_CONSTANT ? vpiConstant : vpiOperation;
        }
        case SLOT_PRIMTERM: return vpiPrimTerm;
        case SLOT_TERM: {
            const llg_vpi_conn_t* conn = row_conn(slot->object, (uint32_t)slot->aux);
            return conn && (conn->role == LLG_VPI_ROLE_TCHK_REF ||
                            conn->role == LLG_VPI_ROLE_TCHK_DATA)
                       ? vpiTchkTerm : vpiPathTerm;
        }
        case SLOT_PORTBIT: return vpiPortBit;
        case SLOT_CABIT: return vpiContAssignBit;
        case SLOT_REFOBJ: return vpiRefObj;
        case SLOT_STMT: return slot->aux;
        case SLOT_IMP: return vpiInterModPath;
        default: return vpiUndefined;
    }
}

/* Width of a value-bearing handle, or 0 when it has no packed width. */
static int handle_width(const llg_vpi_slot_t* slot, uint32_t* width, int* is_signed, int* real) {
    llg_vpi_model_object_t* object = row_object(slot->object);
    *is_signed = 0;
    *real = 0;
    switch (slot->kind) {
        case SLOT_OBJECT:
            if (!object || is_scope_type(object->type) || (object->flags & LLG_VPI_OBJ_RECORD))
                return 0;
            if (is_primitive_type(object->type)) {
                for (uint32_t i = 0; i < object->conn_count; ++i)
                    if (object->conns[i].direction == vpiOutput) {
                        *width = object->conns[i].ref.width;
                        return *width != 0;
                    }
                return 0;
            }
            if (object->type == vpiContAssign) {
                *width = object->width;
                return object->width != 0;
            }
            if (is_structure_type(object->type)) return 0;
            if (object->is_real) {
                *real = 1;
                return 1;
            }
            *width = object->width;
            *is_signed = object->is_signed;
            return 1;
        case SLOT_ELEMENT:
            if (object->is_real) {
                *real = 1;
                return 1;
            }
            *width = object->width;
            *is_signed = object->is_signed;
            return 1;
        case SLOT_SELECT:
            *width = slot->span;
            return 1;
        case SLOT_MEMBER: {
            const llg_vpi_member_node_t* node = node_at((uint32_t)slot->aux);
            if (!node || node->type == vpiRegArray || !node->width) return 0;
            *width = node->width;
            *is_signed = (node->flags & LLG_VPI_TS_SIGNED) != 0;
            return 1;
        }
        case SLOT_ENUMCONST: {
            llg_vpi_datum_t datum;
            if (!read_handle(slot, &datum, is_signed)) return 0;
            *width = llg_sv4_width(datum.packed);
            datum_destroy(&datum);
            return 1;
        }
        case SLOT_CONSTANT:
            *width = is_string_constant(slot) ? (slot->count ? slot->count * 8u : 8u) : 32u;
            *is_signed = !is_string_constant(slot);
            return 1;
        case SLOT_CODE: {
            const llg_vpi_code_t* node = code_at((uint32_t)slot->aux);
            if (!code_is_expression(node) || (node->flags & LLG_VPI_CODE_OPAQUE)) return 0;
            *real = (node->flags & LLG_VPI_CODE_REAL) != 0;
            *width = node->width;
            *is_signed = (node->flags & LLG_VPI_CODE_SIGNED) != 0;
            return *real || node->width != 0;
        }
        case SLOT_CONN:
        case SLOT_PRIMTERM: {
            const llg_vpi_conn_t* conn = row_conn(slot->object, (uint32_t)slot->aux);
            if (!conn || !conn->ref.width) return 0;
            *width = conn->ref.width;
            *is_signed = conn->ref.is_signed;
            return 1;
        }
        case SLOT_ARGUMENT: {
            llg_vpi_callsite_t* site = (llg_vpi_callsite_t*)slot->pointer;
            if (slot->aux < 0 || slot->aux >= site->arg_count) return 0;
            const llg_vpi_compile_arg_t* argument = &site->args[slot->aux];
            *real = argument->is_real;
            *width = argument->width;
            *is_signed = argument->is_signed;
            return 1;
        }
        case SLOT_CALL: {
            llg_vpi_callsite_t* site = (llg_vpi_callsite_t*)slot->pointer;
            if (!site->registration || site->registration->data.type != vpiSysFunc) return 0;
            *real = site->result_real;
            *width = site->result_width;
            *is_signed = site->result_signed;
            return 1;
        }
        default:
            return 0;
    }
}

static PLI_INT32 unsupported_property(void) {
    vpi_error("LLG_VPI_UNSUPPORTED", "unsupported VPI property for this object");
    return vpiUndefined;
}

/* A static catalog variable, variable array, element, bit or member of one,
 * or a named event: the objects whose vpiAutomatic is FALSE and whose
 * vpiAllocScheme is vpiOtherScheme (37.3.7). Nets have neither property. */
static int static_variable_slot(const llg_vpi_slot_t* slot,
                                const llg_vpi_model_object_t* object) {
    if (!object) return 0;
    switch (slot->kind) {
        case SLOT_OBJECT:
            return is_variable_type(object->type) || object->type == vpiRegArray ||
                   object->type == vpiNamedEvent;
        case SLOT_ELEMENT:
        case SLOT_MEMBER:
            return !object->is_net;
        case SLOT_SELECT:
            return slot->span == 1 && !object->is_net;
        default:
            return 0;
    }
}

/* vpiPacked of a struct/union typespec; unknown for a tagged union. */
static PLI_INT32 packed_property(const llg_vpi_typespec_t* typespec) {
    if (!is_aggregate_typespec(typespec) ||
        !(typespec->flags & (LLG_VPI_TS_PACKED | LLG_VPI_TS_UNPACKED)))
        return unsupported_property();
    return (typespec->flags & LLG_VPI_TS_PACKED) != 0;
}

/* The properties 37.23 gives typespecs; typespec members and enum
 * constants carry only their type and name. */
static PLI_INT32 typespec_property(const llg_vpi_slot_t* slot, int property) {
    if (slot->kind != SLOT_TYPESPEC) return unsupported_property();
    const llg_vpi_typespec_t* typespec = typespec_at((uint32_t)slot->aux);
    switch (property) {
        case vpiPacked:
            return packed_property(typespec);
        case vpiTagged:
            if (typespec->type != vpiUnionTypespec) return unsupported_property();
            return (typespec->flags & LLG_VPI_TS_TAGGED) != 0;
        case vpiVector:
            /* 37.23: packed array typespecs are always vectors. */
            if (typespec->type == vpiPackedArrayTypespec) return 1;
            if (typespec->type != vpiLogicTypespec && typespec->type != vpiBitTypespec)
                return unsupported_property();
            return typespec->range_count > 0;
        case vpiArrayType:
            if (typespec->type != vpiArrayTypespec) return unsupported_property();
            return vpiStaticArray;
        default:
            return unsupported_property();
    }
}

PLI_INT32 vpi_get(PLI_INT32 property, vpiHandle handle) {
    vpi_enter();
    if (!handle && (property == vpiTimeUnit || property == vpiTimePrecision)) {
        /* 38.6: a NULL object reports the simulation time unit. */
        uint64_t precision = llg_time_precision_fs();
        return log10_unit(precision ? precision : 1u);
    }
    uint32_t index;
    llg_vpi_slot_t* slot = checked_handle(handle, &index);
    if (!slot) return vpiUndefined;
    if (slot->kind == SLOT_ASSERTION && property != vpiType) {
        uint64_t value;
        if (!assertion_property(slot, property, &value)) return unsupported_property();
        return value > INT32_MAX ? INT32_MAX : (PLI_INT32)value;
    }
    if ((slot->kind == SLOT_FSM || slot->kind == SLOT_FSMSTATE) && property != vpiType) {
        uint64_t value;
        if (!fsm_property(slot, property, &value)) return unsupported_property();
        return value > INT32_MAX ? INT32_MAX : (PLI_INT32)value;
    }
    if (slot->kind == SLOT_STMT) return property == vpiType ? slot->aux : unsupported_property();
    if (slot->kind == SLOT_IMP) {
        /* 37.33: an intermodule path has no properties and no source. */
        if (property == vpiType) return vpiInterModPath;
        return property == vpiLineNo ? 0 : unsupported_property();
    }
    if (rts_slot(slot)) {
        int handled;
        PLI_INT64 value = rts_get(property, slot, &handled);
        if (handled) return value > INT32_MAX || value < INT32_MIN ? INT32_MAX : (PLI_INT32)value;
    }
    if (dyn_slot(slot)) {
        int handled;
        PLI_INT64 value = dyn_get(property, slot, &handled);
        if (handled) {
            if (value > INT32_MAX || value < INT32_MIN) {
                vpi_error("LLG_VPI_PROPERTY", "the property value needs vpi_get64");
                return vpiUndefined;
            }
            return (PLI_INT32)value;
        }
    }
    if (slot->kind == SLOT_CODE) return code_property((uint32_t)slot->aux, property);
    if (slot->kind == SLOT_OBJECT && row_object(slot->object) &&
        is_statement_scope_type(row_object(slot->object)->type)) {
        /* A statement scope's properties are its statement's. */
        uint32_t code = statement_scope_code(row_object(slot->object));
        return code == LLG_VPI_NONE ? vpiUndefined : code_property(code, property);
    }
    {
        /* An argument's expression object answers its own properties; its
         * value, size and sign are the call's snapshot. */
        uint32_t argument = argument_code(slot);
        if (argument != LLG_VPI_NONE &&
            (property == vpiOpType || property == vpiLineNo || property == vpiConstantSelect ||
             property == vpiIndexedPartSelectType || property == vpiUserDefn ||
             property == vpiFuncType))
            return code_property(argument, property);
    }
    if (is_coverage_type_property(property)) return instance_coverage(slot, property);
    if (slot->kind == SLOT_TYPESPEC || slot->kind == SLOT_TSMEMBER || slot->kind == SLOT_ENUMCONST) {
        if (property == vpiType) return handle_type(slot);
        return typespec_property(slot, property);
    }
    llg_vpi_model_object_t* object = row_object(slot->object);
    int type = handle_type(slot);
    uint32_t width = 0;
    int is_signed = 0, real = 0;
    int has_value = handle_width(slot, &width, &is_signed, &real);
    if (slot->kind == SLOT_OBJECT && object && is_declaration_type(object->type) &&
        property != vpiType && property != vpiLineNo) {
        /* 37.13, 37.80: an io declaration's direction and value shape, a
         * generate scope array's number of scopes; a genvar has none. */
        if (object->type == vpiGenScopeArray && property == vpiSize)
            return (PLI_INT32)gen_scope_count(slot->object);
        if (object->type != vpiIODecl) return unsupported_property();
        switch (property) {
            case vpiDirection: return object->subtype ? object->subtype : vpiNoDirection;
            case vpiSize: return (PLI_INT32)object->width;
            case vpiSigned: return object->is_signed != 0;
            case vpiScalar: return object->width == 1;
            case vpiVector: return object->width > 1;
            default: return unsupported_property();
        }
    }
    switch (property) {
        case vpiType:
            return type;
        case vpiSize: {
            /* 37.17 detail 9: an unpacked structure or union reports its
             * number of fields. */
            const llg_vpi_typespec_t* value_type = value_typespec(slot);
            if (is_aggregate_typespec(value_type) && (value_type->flags & LLG_VPI_TS_UNPACKED))
                return (PLI_INT32)value_type->member_count;
        }
            if (slot->kind == SLOT_OBJECT && object &&
                (is_array_type(object->type) || is_instance_array_type(object->type))) {
                uint64_t count = g_vpi.element_counts[slot->object];
                return count > INT32_MAX ? INT32_MAX : (PLI_INT32)count;
            }
            if (slot->kind == SLOT_OBJECT && object && is_primitive_type(object->type)) {
                /* 37.31 detail 1: the number of inputs. */
                PLI_INT32 inputs = 0;
                for (uint32_t i = 0; i < object->conn_count; ++i)
                    inputs += object->conns[i].direction == vpiInput;
                return inputs;
            }
            if (slot->kind == SLOT_OBJECT && object && object->type == vpiFunction)
                return (PLI_INT32)object->width; /* 37.37 detail 12: 0 for void */
            if (slot->kind == SLOT_PORTBIT) return 1;
            if (slot->kind == SLOT_OBJECT && object && object->type == vpiPort)
                return (PLI_INT32)object->width;
            if (slot->kind == SLOT_RANGE) {
                int64_t extent = slot->left > slot->right ? slot->left - slot->right
                                                          : slot->right - slot->left;
                return (PLI_INT32)(extent + 1);
            }
            if (slot->kind == SLOT_ITERATOR) return (PLI_INT32)slot->count;
            if (!has_value) return unsupported_property();
            /* 37.17 leaves vpiSize of a real undefined; it reports the bits of
             * its IEEE 754 storage (decision S42-D8). */
            if (real) return type == vpiShortRealVar ? 32 : 64;
            return (PLI_INT32)width;
        case vpiSigned:
            if (slot->kind == SLOT_OBJECT && object && object->type == vpiFunction && object->width)
                return object->is_signed != 0;
            if (!has_value && !(object && object->type == vpiPort)) return unsupported_property();
            return object && object->type == vpiPort ? object->is_signed != 0 : is_signed != 0;
        case vpiScalar:
        case vpiVector:
            /* 37.14 detail 6: a port is scalar or vector by its own size. */
            if (slot->kind == SLOT_OBJECT && object && object->type == vpiPort && !object->detail)
                return property == vpiScalar ? object->width == 1 : object->width > 1;
            if (slot->kind == SLOT_PORTBIT) return property == vpiScalar;
            /* 37.17 detail 20: FALSE for the other variables (real). */
            if (has_value && real && is_variable_type(type)) return 0;
            if (!has_value || real || is_structure_type(type)) return unsupported_property();
            return property == vpiScalar ? width == 1 : width > 1;
        case vpiArray:
            /* A net or variable bit is not an array (37.16, 37.17). */
            if (slot->kind == SLOT_SELECT && slot->span == 1) return 0;
            if (slot->kind != SLOT_OBJECT || !has_array_properties(object))
                return unsupported_property();
            return is_array_type(object->type);
        case vpiArrayMember:
            if (slot->kind == SLOT_OBJECT && object &&
                (object->type == vpiModule || object->type == vpiInterface ||
                 object->type == vpiProgram || is_primitive_type(object->type)))
                return instance_array_of(slot->object, NULL, NULL) != LLG_VPI_NONE;
            if (slot->kind == SLOT_OBJECT ? !has_array_properties(object)
                                          : slot->kind != SLOT_ELEMENT &&
                                                slot->kind != SLOT_SELECT &&
                                                slot->kind != SLOT_MEMBER)
                return unsupported_property();
            return slot->kind == SLOT_ELEMENT;
        case vpiArrayType:
            if (slot->kind != SLOT_OBJECT || !object || !is_array_type(object->type))
                return unsupported_property();
            return vpiStaticArray;
        case vpiStructUnionMember:
            if (slot->kind == SLOT_MEMBER) return 1;
            /* 37.16 detail 30, 37.17 detail 17: a bit is never a member. */
            if (slot->kind == SLOT_SELECT && slot->span == 1) return 0;
            if ((slot->kind == SLOT_OBJECT && object &&
                 (is_variable_type(object->type) || is_array_type(object->type))) ||
                slot->kind == SLOT_ELEMENT)
                return slot->kind == SLOT_OBJECT && (object->flags & LLG_VPI_OBJ_MEMBER) != 0;
            return unsupported_property();
        case vpiPacked:
            /* 37.18 detail 1: a packed array variable is packed. */
            if (type == vpiPackedArrayVar) return 1;
            return packed_property(value_typespec(slot));
        case vpiPackedArrayMember:
            /* 37.18 detail 4: TRUE only for a sub-element of a packed array,
             * which llg does not expose (decision A19-D28). */
            if (type == vpiPackedArrayVar || type == vpiStructVar || type == vpiUnionVar ||
                type == vpiEnumVar)
                return 0;
            return unsupported_property();
        case vpiConstantSelect:
            /* 37.16 detail 20, 37.17 detail 27: TRUE for a static object
             * without a parent or selected by constant indices, which every
             * catalog net, bit and element is (decision A19-D28). */
            if (type == vpiNet || type == vpiNetBit || type == vpiNetArray ||
                type == vpiRegBit || type == vpiPackedArrayVar || type == vpiStructVar ||
                type == vpiUnionVar || type == vpiEnumVar)
                return 1;
            return unsupported_property();
        case vpiIsMemory:
            if (slot->kind != SLOT_OBJECT || !object || !is_array_type(object->type))
                return unsupported_property();
            return object->type == vpiRegArray && object->dim_count == 1 &&
                   object->subtype == vpiReg;
        case vpiDirection:
            if ((slot->kind == SLOT_OBJECT || slot->kind == SLOT_PORTBIT) && object &&
                (object->type == vpiPort || object->type == vpiClockingIODecl))
                return object->subtype ? object->subtype : vpiNoDirection;
            if (slot->kind == SLOT_PRIMTERM || (slot->kind == SLOT_TERM && type == vpiPathTerm)) {
                const llg_vpi_conn_t* conn = row_conn(slot->object, (uint32_t)slot->aux);
                return conn && conn->direction ? conn->direction : vpiNoDirection;
            }
            return unsupported_property();
        case vpiEdge:
            if (slot->kind == SLOT_TERM) {
                const llg_vpi_conn_t* conn = row_conn(slot->object, (uint32_t)slot->aux);
                return conn ? conn->edge : vpiNoEdge;
            }
            return unsupported_property();
        case vpiPathType:
            if (slot->kind == SLOT_OBJECT && object && object->type == vpiModPath)
                return object->detail;
            return unsupported_property();
        case vpiPolarity:
        case vpiDataPolarity:
            if (slot->kind == SLOT_OBJECT && object && object->type == vpiModPath)
                return property == vpiPolarity ? object->timing->polarity
                                               : object->timing->data_polarity;
            return unsupported_property();
        case vpiModPathHasIfNone:
            if (slot->kind == SLOT_OBJECT && object && object->type == vpiModPath)
                return object->timing->has_ifnone != 0;
            return unsupported_property();
        case vpiTchkType:
            if (slot->kind == SLOT_OBJECT && object && object->type == vpiTchk)
                return object->detail;
            return unsupported_property();
        case vpiPortType:
            /* 37.14: a port bit has the type of its port. */
            if ((slot->kind == SLOT_OBJECT || slot->kind == SLOT_PORTBIT) && object &&
                object->type == vpiPort)
                return object->detail ? object->detail : vpiPort;
            return unsupported_property();
        case vpiTermIndex:
            if (slot->kind == SLOT_PRIMTERM) return slot->aux;
            return unsupported_property();
        case vpiAlwaysType:
            if (slot->kind == SLOT_OBJECT && object && object->type == vpiAlways && object->detail)
                return object->detail;
            return unsupported_property();
        case vpiPrimType:
            if (slot->kind == SLOT_OBJECT && object && is_primitive_type(object->type))
                return object->detail;
            return unsupported_property();
        case vpiNetDeclAssign:
            if ((slot->kind == SLOT_OBJECT || slot->kind == SLOT_CABIT) && object &&
                object->type == vpiContAssign)
                return (object->flags & LLG_VPI_OBJ_QUALIFIER) != 0;
            /* 37.16: a net assigned in its declaration (decision A19-D35). */
            if (net_handle(slot)) return net_decl_assigned(slot);
            return unsupported_property();
        case vpiOffset:
            /* 37.43 detail 3: 0 for the least significant bit. */
            if (slot->kind == SLOT_CABIT) return slot->aux;
            return unsupported_property();
        case vpiStrength0:
        case vpiStrength1:
            /* 37.31, 37.43: the drive strengths a primitive or continuous
             * assignment drives with; a net drives nothing of its own
             * (decision A19-D35). */
            if ((slot->kind == SLOT_OBJECT || slot->kind == SLOT_CABIT) && object &&
                (is_primitive_type(object->type) || object->type == vpiContAssign))
                return (object->strength >> (property == vpiStrength0 ? 0 : 8)) & 0xff;
            if (net_handle(slot)) return 0;
            return unsupported_property();
        case vpiExpanded:
            /* 6.9.2: "If the keyword scalared is used, ... the PLI shall
             * consider the net expanded"; llg keeps every other vector whole.
             * A net bit reports its parent's (37.16 detail 18). */
            if (net_handle(slot)) return (object->flags & LLG_VPI_OBJ_SCALARED) != 0;
            return unsupported_property();
        case vpiImplicitDecl:
            if (net_handle(slot)) return (object->flags & LLG_VPI_OBJ_IMPLICIT) != 0;
            return unsupported_property();
        case vpiExplicitScalared:
        case vpiExplicitVectored:
            /* 37.16: the `scalared` or `vectored` keyword of the declaration. */
            if (net_handle(slot))
                return (object->flags & (property == vpiExplicitScalared ? LLG_VPI_OBJ_SCALARED
                                                                         : LLG_VPI_OBJ_VECTORED)) != 0;
            return unsupported_property();
        case vpiConnByName:
        case vpiExplicitName:
            /* 37.14 detail 8: a port named in the module header, connected
             * by name in the instantiation. */
            if ((slot->kind == SLOT_OBJECT || slot->kind == SLOT_PORTBIT) && object &&
                object->type == vpiPort)
                return (object->flags & (property == vpiConnByName ? LLG_VPI_OBJ_BY_NAME
                                                                   : LLG_VPI_OBJ_EXPLICIT)) != 0;
            return unsupported_property();
        case vpiVirtual:
            /* 37.37: only class methods are virtual (decision A19-D36);
             * 37.15 detail 5: a ref obj of an interface port is not a
             * virtual interface. */
            if (slot->kind == SLOT_OBJECT && object &&
                (object->type == vpiTask || object->type == vpiFunction))
                return 0;
            if (slot->kind == SLOT_REFOBJ) return 0;
            return unsupported_property();
        case vpiMethod:
            /* Class methods are not catalog objects (37.37). */
            if (slot->kind == SLOT_OBJECT && object &&
                (object->type == vpiTask || object->type == vpiFunction))
                return 0;
            return unsupported_property();
        case vpiVisibility:
            if (slot->kind == SLOT_OBJECT && object &&
                (object->type == vpiTask || object->type == vpiFunction))
                return vpiPublicVis; /* 37.37 detail 4 */
            /* 37.17 detail 24: a variable that is not a class member. */
            if (is_static_variable_part(slot, object)) return vpiPublicVis;
            return unsupported_property();
        case vpiConstantVariable:
            /* 37.17: a variable declared const (6.20.6). */
            if (is_static_variable_part(slot, object))
                return (object->flags & LLG_VPI_OBJ_CONST) != 0;
            return unsupported_property();
        case vpiRandType:
        case vpiIsRandomized:
            /* 37.17 details 15 and 16: only class properties are declared
             * rand or randc; std::randomize() leaves a variable's type
             * vpiNotRand (decision A19-D36). */
            if (is_static_variable_part(slot, object))
                return property == vpiRandType ? vpiNotRand : 0;
            return unsupported_property();
        case vpiPortIndex:
            if (slot->kind == SLOT_OBJECT && object && object->type == vpiPort)
                return (PLI_INT32)object->port_index;
            return unsupported_property();
        case vpiResolvedNetType:
            if (net_handle(slot)) return resolved_net_type(slot);
            return unsupported_property();
        case vpiNetType:
            if (slot->kind == SLOT_OBJECT && object && object->type == vpiNet)
                return object->subtype ? object->subtype : vpiWire;
            if (slot->kind == SLOT_ELEMENT && object && object->type == vpiNetArray)
                return vpiWire;
            /* A net array and a net bit have their nets' type (37.16). */
            if (slot->kind == SLOT_OBJECT && object && object->type == vpiNetArray) return vpiWire;
            if (slot->kind == SLOT_SELECT && slot->span == 1 && object && object->is_net)
                return object->type == vpiNet && object->subtype ? object->subtype : vpiWire;
            return unsupported_property();
        case vpiInputEdge:
        case vpiOutputEdge:
            /* 37.44: the edge of the (default) skew, the clocking event's
             * for a skew without one (decision A19-D30). */
            if (slot->kind == SLOT_OBJECT && object &&
                (object->type == vpiClockingBlock || object->type == vpiClockingIODecl))
                return (object->detail >> (property == vpiInputEdge ? 0 : 8)) & 0xff;
            return unsupported_property();
        case vpiUnit:
            /* 37.10: TRUE for the compilation unit `$unit`. */
            if (slot->kind == SLOT_OBJECT && object && object->type == vpiPackage)
                return (object->flags & LLG_VPI_OBJ_QUALIFIER) != 0;
            return unsupported_property();
        case vpiTopModule:
            if (slot->kind == SLOT_OBJECT && object && object->type == vpiModule)
                return (object->flags & LLG_VPI_OBJ_TOP) || !object->parent;
            return unsupported_property();
        case vpiTop:
            /* 37.10: an instance at the top of the hierarchy. */
            if (slot->kind == SLOT_OBJECT && object && object->instance)
                return (object->flags & LLG_VPI_OBJ_TOP) || !object->parent;
            return unsupported_property();
        case vpiProtected:
            /* 37.10, 37.80: llg rejects protected envelopes (34), so no
             * instance or generate scope is protected. */
            if (slot->kind == SLOT_OBJECT && object &&
                (object->instance || object->type == vpiGenScope))
                return 0;
            return unsupported_property();
        case vpiLocalParam:
            if (slot->kind == SLOT_OBJECT && object && object->type == vpiParameter)
                return (object->flags & LLG_VPI_OBJ_LOCAL_PARAM) != 0;
            return unsupported_property();
        case vpiConstType:
            if (slot->kind == SLOT_CONSTANT) return slot->aux ? slot->aux : vpiDecConst;
            if (slot->kind == SLOT_ARGUMENT && slot->count)
                return slot->left ? (PLI_INT32)slot->left : vpiBinaryConst;
            if (slot->kind == SLOT_CONN && type == vpiConstant) {
                const llg_vpi_conn_t* conn = row_conn(slot->object, (uint32_t)slot->aux);
                return conn->ref.const_type ? conn->ref.const_type : vpiBinaryConst;
            }
            if (slot->kind == SLOT_OBJECT && object && is_parameter_type(object->type))
                return object->is_real ? vpiRealConst
                       : (object->flags & LLG_VPI_OBJ_TEXT) ? vpiStringConst : vpiBinaryConst;
            return unsupported_property();
        case vpiLineNo:
            /* A terminal, port bit or assignment bit is on its owner's line. */
            if ((slot->kind == SLOT_OBJECT || slot->kind == SLOT_CABIT ||
                 slot->kind == SLOT_PRIMTERM || slot->kind == SLOT_PORTBIT ||
                 slot->kind == SLOT_TERM) && object)
                return object->line;
            if (slot->kind == SLOT_CALL) return ((llg_vpi_callsite_t*)slot->pointer)->line;
            return unsupported_property();
        case vpiTimeUnit:
        case vpiTimePrecision:
            /* 37.44 gives a clocking block no time unit. */
            if (slot->kind == SLOT_OBJECT && object && is_scope_type(object->type) &&
                object->type != vpiClockingBlock) {
                uint64_t precision = llg_time_precision_fs();
                uint64_t unit = property == vpiTimeUnit ? time_unit_of(object) : precision;
                if (!unit) unit = precision ? precision : 1u;
                return log10_unit(unit);
            }
            return unsupported_property();
        case vpiCellInstance:
        case vpiDefNetType:
        case vpiDefLineNo:
        case vpiDefDelayMode:
        case vpiUnconnDrive:
            /* 37.10: the definition properties of an instance. */
            if (slot->kind != SLOT_OBJECT || !object || !object->instance)
                return unsupported_property();
            switch (property) {
                case vpiCellInstance: return object->instance->cell_define != 0;
                case vpiDefNetType: return object->instance->net_type;
                case vpiDefLineNo: return object->instance->def_line;
                /* llg rejects the delay-mode directives (decision A19-D41). */
                case vpiDefDelayMode: return vpiDelayModeNone;
                default: return object->instance->unconnected_drive;
            }
        case vpiAutomatic:
            if (slot->kind == SLOT_OBJECT && object &&
                (object->type == vpiTask || object->type == vpiFunction))
                return (object->flags & LLG_VPI_OBJ_QUALIFIER) != 0;
            /* 37.3.7: the default lifetime of an instance's subroutines. */
            if (slot->kind == SLOT_OBJECT && object && object->instance)
                return object->instance->automatic != 0;
            /* Automatic variables and events are not catalog objects. */
            if (static_variable_slot(slot, object)) return 0;
            return unsupported_property();
        case vpiAllocScheme:
            /* 37.3.7: an object allocated neither in a frame or thread nor in
             * dynamic memory reports vpiOtherScheme; every catalog variable,
             * part of one and named event is static (decision A19-D40). */
            if (static_variable_slot(slot, object)) return vpiOtherScheme;
            return unsupported_property();
        case vpiScheduled:
            if (slot->kind == SLOT_EVENT) {
                llg_vpi_event_t* event = (llg_vpi_event_t*)slot->pointer;
                return event && event->pending;
            }
            return unsupported_property();
        case vpiFuncType:
            if (slot->kind == SLOT_SYSTF) {
                llg_vpi_registration_t* registration = (llg_vpi_registration_t*)slot->pointer;
                return registration->data.type == vpiSysFunc ? registration->data.sysfunctype
                                                             : vpiSysTask;
            }
            if (slot->kind == SLOT_CALL) {
                llg_vpi_callsite_t* site = (llg_vpi_callsite_t*)slot->pointer;
                if (site->registration && site->registration->data.type == vpiSysFunc)
                    return site->registration->data.sysfunctype;
            }
            if (slot->kind == SLOT_OBJECT && object && object->type == vpiFunction &&
                object->detail)
                return object->detail;
            return unsupported_property();
        case vpiUserDefn:
            if (slot->kind == SLOT_CALL || slot->kind == SLOT_SYSTF) return 1;
            return unsupported_property();
        case vpiIteratorType:
            if (slot->kind == SLOT_ITERATOR) return slot->aux;
            return unsupported_property();
        default:
            return unsupported_property();
    }
}

PLI_INT64 vpi_get64(PLI_INT32 property, vpiHandle handle) {
    /* Assertion counters keep their full 64-bit value (40.5.3). */
    llg_vpi_slot_t* slot = decode_handle(handle, NULL);
    uint64_t value;
    if (slot && slot->kind == SLOT_ASSERTION && property != vpiType) {
        vpi_enter();
        if (!assertion_property(slot, property, &value)) return unsupported_property();
        return value > INT64_MAX ? INT64_MAX : (PLI_INT64)value;
    }
    if (slot && (slot->kind == SLOT_FSM || slot->kind == SLOT_FSMSTATE) && property != vpiType) {
        vpi_enter();
        if (!fsm_property(slot, property, &value)) return unsupported_property();
        return value > INT64_MAX ? INT64_MAX : (PLI_INT64)value;
    }
    /* vpiObjId is an int64 property (37.29). */
    if (slot && dyn_slot(slot)) {
        vpi_enter();
        int handled;
        PLI_INT64 result = dyn_get(property, slot, &handled);
        if (handled) return result;
    }
    return (PLI_INT64)vpi_get(property, handle);
}

static const char* type_name(int type) {
    switch (type) {
        case vpiModule: return "vpiModule";
        case vpiInterface: return "vpiInterface";
        case vpiGenScope: return "vpiGenScope";
        case vpiNamedBegin: return "vpiNamedBegin";
        case vpiForce: return "vpiForce";
        case vpiRelease: return "vpiRelease";
        case vpiNamedFork: return "vpiNamedFork";
        case vpiNet: return "vpiNet";
        case vpiReg: return "vpiReg";
        case vpiIntegerVar: return "vpiIntegerVar";
        case vpiTimeVar: return "vpiTimeVar";
        case vpiRealVar: return "vpiRealVar";
        case vpiLongIntVar: return "vpiLongIntVar";
        case vpiShortIntVar: return "vpiShortIntVar";
        case vpiIntVar: return "vpiIntVar";
        case vpiShortRealVar: return "vpiShortRealVar";
        case vpiByteVar: return "vpiByteVar";
        case vpiEnumVar: return "vpiEnumVar";
        case vpiStructVar: return "vpiStructVar";
        case vpiUnionVar: return "vpiUnionVar";
        case vpiBitVar: return "vpiBitVar";
        case vpiPackedArrayVar: return "vpiPackedArrayVar";
        case vpiRegArray: return "vpiRegArray";
        case vpiNetArray: return "vpiNetArray";
        case vpiParameter: return "vpiParameter";
        case vpiSpecParam: return "vpiSpecParam";
        case vpiGenScopeArray: return "vpiGenScopeArray";
        case vpiGenVar: return "vpiGenVar";
        case vpiIODecl: return "vpiIODecl";
        case vpiTimeQueue: return "vpiTimeQueue";
        case vpiUdpDefn: return "vpiUdpDefn";
        case vpiTableEntry: return "vpiTableEntry";
        case vpiThread: return "vpiThread";
        case vpiFrame: return "vpiFrame";
        case vpiPort: return "vpiPort";
        case vpiNetBit: return "vpiNetBit";
        case vpiRegBit: return "vpiRegBit";
        case vpiPartSelect: return "vpiPartSelect";
        case vpiConstant: return "vpiConstant";
        case vpiOperation: return "vpiOperation";
        case vpiRange: return "vpiRange";
        case vpiIterator: return "vpiIterator";
        case vpiUserSystf: return "vpiUserSystf";
        case vpiSysTaskCall: return "vpiSysTaskCall";
        case vpiSysFuncCall: return "vpiSysFuncCall";
        case vpiCallback: return "vpiCallback";
        case vpiSchedEvent: return "vpiSchedEvent";
        case vpiAssert: return "vpiAssert";
        case vpiAssume: return "vpiAssume";
        case vpiCover: return "vpiCover";
        case vpiImmediateAssert: return "vpiImmediateAssert";
        case vpiImmediateAssume: return "vpiImmediateAssume";
        case vpiImmediateCover: return "vpiImmediateCover";
        case vpiCollection: return "vpiCollection";
        case vpiFsm: return "vpiFsm";
        case vpiFsmHandle: return "vpiFsmHandle";
        case vpiPropertySpec: return "vpiPropertySpec";
        case vpiPropertyInst: return "vpiPropertyInst";
        case vpiSequenceInst: return "vpiSequenceInst";
        case vpiPropertyDecl: return "vpiPropertyDecl";
        case vpiSequenceDecl: return "vpiSequenceDecl";
        case vpiPropFormalDecl: return "vpiPropFormalDecl";
        case vpiSeqFormalDecl: return "vpiSeqFormalDecl";
        case vpiLongIntTypespec: return "vpiLongIntTypespec";
        case vpiShortRealTypespec: return "vpiShortRealTypespec";
        case vpiByteTypespec: return "vpiByteTypespec";
        case vpiShortIntTypespec: return "vpiShortIntTypespec";
        case vpiIntTypespec: return "vpiIntTypespec";
        case vpiStringTypespec: return "vpiStringTypespec";
        case vpiEnumTypespec: return "vpiEnumTypespec";
        case vpiEnumConst: return "vpiEnumConst";
        case vpiIntegerTypespec: return "vpiIntegerTypespec";
        case vpiTimeTypespec: return "vpiTimeTypespec";
        case vpiRealTypespec: return "vpiRealTypespec";
        case vpiStructTypespec: return "vpiStructTypespec";
        case vpiUnionTypespec: return "vpiUnionTypespec";
        case vpiBitTypespec: return "vpiBitTypespec";
        case vpiLogicTypespec: return "vpiLogicTypespec";
        case vpiArrayTypespec: return "vpiArrayTypespec";
        case vpiPackedArrayTypespec: return "vpiPackedArrayTypespec";
        case vpiTypespecMember: return "vpiTypespecMember";
        case vpiNamedEvent: return "vpiNamedEvent";
        case vpiInitial: return "vpiInitial";
        case vpiAlways: return "vpiAlways";
        case vpiFinal: return "vpiFinal";
        case vpiContAssign: return "vpiContAssign";
        case vpiContAssignBit: return "vpiContAssignBit";
        case vpiTask: return "vpiTask";
        case vpiFunction: return "vpiFunction";
        case vpiGate: return "vpiGate";
        case vpiSwitch: return "vpiSwitch";
        case vpiUdp: return "vpiUdp";
        case vpiPrimTerm: return "vpiPrimTerm";
        case vpiModuleArray: return "vpiModuleArray";
        case vpiInterfaceArray: return "vpiInterfaceArray";
        case vpiGateArray: return "vpiGateArray";
        case vpiSwitchArray: return "vpiSwitchArray";
        case vpiUdpArray: return "vpiUdpArray";
        case vpiModport: return "vpiModport";
        case vpiPortBit: return "vpiPortBit";
        case vpiRefObj: return "vpiRefObj";
        case vpiModPath: return "vpiModPath";
        case vpiInterModPath: return "vpiInterModPath";
        case vpiPathTerm: return "vpiPathTerm";
        case vpiTchk: return "vpiTchk";
        case vpiTchkTerm: return "vpiTchkTerm";
        case vpiAssignment: return "vpiAssignment";
        case vpiBegin: return "vpiBegin";
        case vpiCase: return "vpiCase";
        case vpiCaseItem: return "vpiCaseItem";
        case vpiConstraintOrdering: return "vpiConstraintOrdering";
        case vpiDistribution: return "vpiDistribution";
        case vpiDistItem: return "vpiDistItem";
        case vpiImplication: return "vpiImplication";
        case vpiConstrIf: return "vpiConstrIf";
        case vpiConstrIfElse: return "vpiConstrIfElse";
        case vpiDelayControl: return "vpiDelayControl";
        case vpiDisable: return "vpiDisable";
        case vpiEventControl: return "vpiEventControl";
        case vpiEventStmt: return "vpiEventStmt";
        case vpiFor: return "vpiFor";
        case vpiForever: return "vpiForever";
        case vpiFork: return "vpiFork";
        case vpiFuncCall: return "vpiFuncCall";
        case vpiIf: return "vpiIf";
        case vpiIfElse: return "vpiIfElse";
        case vpiNullStmt: return "vpiNullStmt";
        case vpiRepeat: return "vpiRepeat";
        case vpiRepeatControl: return "vpiRepeatControl";
        case vpiTaskCall: return "vpiTaskCall";
        case vpiVarSelect: return "vpiVarSelect";
        case vpiWait: return "vpiWait";
        case vpiWhile: return "vpiWhile";
        case vpiBitSelect: return "vpiBitSelect";
        case vpiIndexedPartSelect: return "vpiIndexedPartSelect";
        case vpiDoWhile: return "vpiDoWhile";
        case vpiOrderedWait: return "vpiOrderedWait";
        case vpiWaitFork: return "vpiWaitFork";
        case vpiDisableFork: return "vpiDisableFork";
        case vpiForeachStmt: return "vpiForeachStmt";
        case vpiReturnStmt: return "vpiReturnStmt";
        case vpiBreak: return "vpiBreak";
        case vpiContinue: return "vpiContinue";
        case vpiMethodFuncCall: return "vpiMethodFuncCall";
        case vpiMethodTaskCall: return "vpiMethodTaskCall";
        case vpiAssignStmt: return "vpiAssignStmt";
        case vpiDeassign: return "vpiDeassign";
        default: return NULL;
    }
}

/* Name of an element or select, composed in the vpi_get_str buffer. A
 * member's name is its own; its full name continues the containing value's
 * full name with every enclosing member (`tb.s.inner.x`, `tb.a[2].x`). */
static char* composed_name(const llg_vpi_slot_t* slot, int full) {
    llg_vpi_model_object_t* object = row_object(slot->object);
    char* buffer = g_vpi.str_buffer;
    size_t capacity = sizeof(g_vpi.str_buffer);
    if (slot->kind == SLOT_MEMBER && !full) {
        const llg_vpi_member_node_t* node = node_at((uint32_t)slot->aux);
        return node ? (char*)node->name : NULL;
    }
    int written = snprintf(buffer, capacity, "%s", full ? object->full_name : object->name);
    if (written < 0 || (size_t)written >= capacity) return NULL;
    size_t at = (size_t)written;
    if (slot->element != LLG_VPI_NO_ELEMENT) {
        uint64_t rest = slot->element;
        int64_t indices[16];
        uint32_t dims = object->dim_count < 16 ? object->dim_count : 16;
        for (uint32_t d = dims; d-- > 0;) {
            int64_t left, right;
            if (!array_dim(object, d, &left, &right)) return NULL;
            uint64_t extent = (uint64_t)(left > right ? left - right : right - left) + 1u;
            uint64_t offset = rest % extent;
            rest /= extent;
            indices[d] = left >= right ? left - (int64_t)offset : left + (int64_t)offset;
        }
        for (uint32_t d = 0; d < dims; ++d) {
            written = snprintf(buffer + at, capacity - at, "[%lld]", (long long)indices[d]);
            if (written < 0 || (size_t)written >= capacity - at) return NULL;
            at += (size_t)written;
        }
    }
    if (slot->kind == SLOT_MEMBER) {
        /* Enclosing members, outermost first: collect the chain upwards. */
        uint32_t chain[64];
        uint32_t depth = 0;
        for (uint32_t node = (uint32_t)slot->aux + 1u; node && depth < 64u;) {
            chain[depth++] = node - 1u;
            node = node_at(node - 1u)->parent;
        }
        while (depth-- > 0) {
            written = snprintf(buffer + at, capacity - at, ".%s", node_at(chain[depth])->name);
            if (written < 0 || (size_t)written >= capacity - at) return NULL;
            at += (size_t)written;
        }
        return buffer;
    }
    if (slot->kind == SLOT_SELECT) {
        if (slot->span == 1)
            written = snprintf(buffer + at, capacity - at, "[%lld]", (long long)slot->left);
        else
            written = snprintf(buffer + at, capacity - at, "[%lld:%lld]", (long long)slot->left,
                               (long long)slot->right);
        if (written < 0 || (size_t)written >= capacity - at) return NULL;
    }
    return buffer;
}

PLI_BYTE8* vpi_get_str(PLI_INT32 property, vpiHandle handle) {
    vpi_enter();
    llg_vpi_slot_t* slot = checked_handle(handle, NULL);
    if (!slot) return NULL;
    if (dyn_slot(slot)) {
        int handled;
        const char* text = dyn_get_str(property, slot, &handled);
        if (handled) return (PLI_BYTE8*)text;
    }
    if (rts_slot(slot)) {
        int handled;
        const char* text = rts_get_str(property, slot, &handled);
        if (handled) return (PLI_BYTE8*)text;
    }
    llg_vpi_model_object_t* object = row_object(slot->object);
    if (property == vpiType) {
        const char* name = type_name(handle_type(slot));
        if (name) return (PLI_BYTE8*)name;
    }
    if (slot->kind == SLOT_IMP) {
        if (property != vpiFile) unsupported_property();
        return NULL;
    }
    if (slot->kind == SLOT_CODE) return code_string((uint32_t)slot->aux, property);
    if (slot->kind == SLOT_OBJECT && object && is_statement_scope_type(object->type) &&
        property != vpiName && property != vpiFullName) {
        /* A statement scope has its scope's names (37.12) and its
         * statement's other strings. */
        uint32_t code = statement_scope_code(object);
        return code == LLG_VPI_NONE ? NULL : code_string(code, property);
    }
    {
        uint32_t argument = argument_code(slot);
        if (argument != LLG_VPI_NONE &&
            (property == vpiDecompile || property == vpiName || property == vpiFile ||
             (property == vpiFullName && code_is_named_select(code_type(code_at(argument))))))
            return code_string(argument, property);
    }
    if (slot->kind == SLOT_OBJECT && object && is_declaration_type(object->type) &&
        property != vpiName && property != vpiFile &&
        (property != vpiFullName || object->type == vpiIODecl)) {
        /* 37.13, 37.80: names only; an io declaration has no full name. */
        unsupported_property();
        return NULL;
    }
    switch (slot->kind) {
        case SLOT_OBJECT:
            /* Processes and continuous assignments have no name (37.43,
             * 37.58). A module path is never found by name but reports the
             * name it carries as both names (decision A19-D9). */
            if ((object->flags & LLG_VPI_OBJ_UNNAMED) &&
                (property == vpiName || property == vpiFullName)) {
                if (object->type == vpiModPath && object->name && object->name[0])
                    return (PLI_BYTE8*)object->name;
                break;
            }
            switch (property) {
                case vpiName: return (PLI_BYTE8*)object->name;
                case vpiFullName:
                    if (object->type == vpiPort) {
                        snprintf(g_vpi.str_buffer, sizeof(g_vpi.str_buffer), "%s",
                                 object->full_name);
                        return (PLI_BYTE8*)g_vpi.str_buffer;
                    }
                    return (PLI_BYTE8*)object->full_name;
                case vpiDefName:
                    if (object->definition_name) return (PLI_BYTE8*)object->definition_name;
                    break;
                case vpiFile:
                    if (object->file) return (PLI_BYTE8*)object->file;
                    break;
                case vpiDefFile:
                case vpiLibrary:
                case vpiCell:
                case vpiConfig: {
                    /* 37.10, 33.7: the definition and its binding. No
                     * configuration controls an unconfigured design's binding
                     * (decision A19-D41). */
                    const llg_vpi_instance_t* instance = object->instance;
                    const char* text = !instance                  ? NULL
                                       : property == vpiDefFile ? instance->def_file
                                       : property == vpiLibrary ? instance->library
                                       : property == vpiCell    ? instance->cell
                                                                : instance->config;
                    if (text) return (PLI_BYTE8*)text;
                    break;
                }
                case vpiDecompile:
                    /* 37.55: nets, variables (arrays, strings, chandles and
                     * class variables included) and parameters are simple
                     * expressions; each decompiles to its name (decision
                     * A19-D32). */
                    if (is_variable_type(object->type) || object->type == vpiNet ||
                        is_parameter_type(object->type) || is_array_type(object->type) ||
                        (object->flags & LLG_VPI_OBJ_DYNAMIC))
                        return (PLI_BYTE8*)object->name;
                    break;
                default:
                    break;
            }
            break;
        case SLOT_ELEMENT:
        case SLOT_SELECT:
        case SLOT_MEMBER:
            if (property == vpiDecompile) property = vpiName;
            if (property == vpiName || property == vpiFullName) {
                char* name = composed_name(slot, property == vpiFullName);
                if (name) return (PLI_BYTE8*)name;
            }
            if (property == vpiFile && object->file) return (PLI_BYTE8*)object->file;
            break;
        case SLOT_CABIT:
            if (property == vpiFile && object->file) return (PLI_BYTE8*)object->file;
            break;
        case SLOT_REFOBJ: {
            /* 37.15: the low reference is the port's formal, the high one
             * names the connected instance or modport. */
            const llg_vpi_model_object_t* actual = object->target;
            if (!actual) break;
            /* A reference object is a simple expression: it decompiles
             * to its name (37.55, decision A19-D32). */
            if (property == vpiName || property == vpiDecompile)
                return (PLI_BYTE8*)(slot->aux ? actual->name : object->name);
            if (property == vpiFullName)
                return (PLI_BYTE8*)(slot->aux ? actual->full_name : object->full_name);
            if (property == vpiDefName) {
                /* Detail 7: the interface definition or modport name. */
                if (actual->type == vpiModport) return (PLI_BYTE8*)actual->name;
                if (actual->definition_name) return (PLI_BYTE8*)actual->definition_name;
            }
            break;
        }
        case SLOT_CONSTANT:
            /* A string key decompiles as its characters. */
            if (is_string_constant(slot) && property == vpiDecompile)
                return (PLI_BYTE8*)slot->items;
            break;
        case SLOT_TYPESPEC:
            /* 37.23 detail 1: a type without a typedef has no name. */
            if (property == vpiName) return (PLI_BYTE8*)typespec_at((uint32_t)slot->aux)->name;
            break;
        case SLOT_TSMEMBER:
            if (property == vpiName) return (PLI_BYTE8*)g_vpi.types->members[slot->aux].name;
            break;
        case SLOT_ENUMCONST:
            if (property == vpiName) return (PLI_BYTE8*)g_vpi.types->consts[slot->aux].name;
            break;
        case SLOT_SYSTF:
            if (property == vpiName) return (PLI_BYTE8*)((llg_vpi_registration_t*)slot->pointer)->name;
            break;
        case SLOT_CALL: {
            llg_vpi_callsite_t* site = (llg_vpi_callsite_t*)slot->pointer;
            if (property == vpiName) return (PLI_BYTE8*)site->name;
            if (property == vpiFile && site->file) return (PLI_BYTE8*)site->file;
            break;
        }
        case SLOT_ASSERTION: {
            const llg_acov_item_t* item = slot_item(slot);
            if (!item) break;
            /* An unnamed assertion has no name (39.3.1). */
            if (property == vpiName) return item->name[0] ? (PLI_BYTE8*)item->name : NULL;
            if (property == vpiFullName)
                return item->full_name[0] ? (PLI_BYTE8*)item->full_name : NULL;
            if (property == vpiDefName) {
                const llg_acov_instance_t* owner = llg_acov_instance_at((size_t)item->instance);
                if (item->instance >= 0 && owner) return (PLI_BYTE8*)owner->definition;
            }
            if (property == vpiFile && item->file) return (PLI_BYTE8*)item->file;
            break;
        }
        case SLOT_FSM: {
            const llg_fcov_fsm_t* fsm = slot_fsm(slot);
            if (!fsm) break;
            if (property == vpiName) return (PLI_BYTE8*)fsm->name;
            if (property == vpiFullName) return (PLI_BYTE8*)fsm->full_name;
            if (property == vpiFile) return (PLI_BYTE8*)fsm->file;
            if (property == vpiDefName) {
                const llg_acov_instance_t* owner = llg_acov_instance_at((size_t)fsm->instance);
                if (owner) return (PLI_BYTE8*)owner->definition;
            }
            break;
        }
        case SLOT_FSMSTATE: {
            /* A state is named by its parameter (40.4.6). */
            const llg_fcov_fsm_t* fsm = slot_fsm(slot);
            if (fsm && property == vpiName) return (PLI_BYTE8*)fsm->states[slot->aux].name;
            break;
        }
        default:
            break;
    }
    vpi_error("LLG_VPI_UNSUPPORTED", "unsupported VPI string property for this object");
    return NULL;
}

/* ── values ─────────────────────────────────────────────────────────────── */

void vpi_get_value(vpiHandle handle, p_vpi_value output) {
    vpi_enter();
    if (!output) {
        vpi_error("LLG_VPI_ARGUMENT", "null vpi_get_value output");
        return;
    }
    llg_vpi_slot_t* slot = checked_handle(handle, NULL);
    if (!slot) return;
    if (dyn_slot(slot)) {
        dyn_get_value(slot, output);
        return;
    }
    if (rts_slot(slot)) {
        rts_get_value(slot, output);
        return;
    }
    if (whole_unpacked_aggregate(slot)) {
        vpi_error("LLG_VPI_VALUE",
                  "an entire unpacked structure or union has no value; access its members");
        return;
    }
    int type = handle_type(slot);
    llg_vpi_datum_t datum;
    int is_signed = 0;
    if (!read_handle(slot, &datum, &is_signed)) {
        datum_destroy(&datum);
        if (slot->kind == SLOT_CALL)
            vpi_error("LLG_VPI_VALUE", "a system task call, or an inactive call, has no value");
        else if (slot->kind == SLOT_ARGUMENT)
            vpi_error("LLG_VPI_VALUE", "the call of this argument has returned");
        else
            vpi_error("LLG_VPI_VALUE", "this VPI object has no value");
        return;
    }
    fill_value(&datum, is_signed, type, output);
    datum_destroy(&datum);
}

static uint64_t ticks_from_time(const s_vpi_time* time, const llg_vpi_model_object_t* scope,
                                int* ok) {
    *ok = 1;
    if (time->type == vpiSimTime) return ((uint64_t)time->high << 32) | time->low;
    if (time->type == vpiScaledRealTime) {
        uint64_t precision = llg_time_precision_fs();
        uint64_t unit = time_unit_of(scope);
        if (!precision) precision = 1;
        if (!unit) unit = precision;
        double ticks = time->real * (double)unit / (double)precision;
        if (!(ticks >= 0.0) || ticks > 18446744073709549568.0) {
            *ok = 0;
            return 0;
        }
        return (uint64_t)llround(ticks);
    }
    *ok = 0;
    return 0;
}

static void vpi_event_unlink(llg_vpi_event_t* event) {
    if (!event->prev_link) return;
    *event->prev_link = event->next;
    if (event->next) event->next->prev_link = event->prev_link;
    event->next = NULL;
    event->prev_link = NULL;
}

static void vpi_event_finish(llg_vpi_event_t* event) {
    event->pending = 0;
    vpi_event_unlink(event);
    if (!event->is_real) sv4_destroy(&event->value);
    if (event->slot) {
        /* The vpiSchedEvent handle keeps the record until it is released. */
        return;
    }
    free(event);
}

/* Runs in the Active turn of the event's carrier process. The event has
 * occurred before its write starts, so value-change callbacks of the write
 * see it as no longer scheduled, and a put or cancel they issue can neither
 * remove it nor free it twice. A handle may be released or cancelled from
 * such a callback, so the record is not touched after the write. */
static void vpi_event_fire(void* data) {
    llg_vpi_event_t* event = (llg_vpi_event_t*)data;
    event->pending = 0;
    vpi_event_unlink(event);
    llg_vpi_target_t target = event->target;
    llg_vpi_datum_t datum;
    datum.is_real = event->is_real;
    datum.real = event->real;
    datum.packed = event->value;
    event->value = VPI_EMPTY;
    /* The vpiSchedEvent handle keeps the record until it is released. */
    if (!event->slot) free(event);
    write_target(&target, &datum);
    if (!datum.is_real) sv4_destroy(&datum.packed);
}

static void vpi_event_cancel(llg_vpi_event_t* event) {
    if (event->pending) llg_cancel_delayed_call(event->id);
    vpi_event_finish(event);
}

static int same_object(const llg_vpi_target_t* a, const llg_vpi_target_t* b) {
    return a->object == b->object && a->element == b->element;
}

static vpiHandle schedule_put(const llg_vpi_target_t* target, llg_vpi_datum_t* datum,
                              uint64_t ticks, int mode, int return_event) {
    if (ticks > UINT64_MAX - llg_time()) {
        vpi_error("LLG_VPI_TIME", "the vpi_put_value delay passes the last simulation time");
        return NULL;
    }
    uint64_t when = llg_time() + ticks;
    for (llg_vpi_event_t* cursor = g_vpi.events; cursor;) {
        llg_vpi_event_t* next = cursor->next;
        if (same_object(&cursor->target, target) &&
            (mode == vpiInertialDelay || (mode == vpiTransportDelay && cursor->time > when)))
            vpi_event_cancel(cursor);
        cursor = next;
    }
    llg_vpi_event_t* event = (llg_vpi_event_t*)calloc(1, sizeof(*event));
    if (!event) {
        vpi_error("LLG_VPI_NOMEM", "VPI scheduled event allocation failed");
        return NULL;
    }
    event->target = *target;
    event->is_real = datum->is_real;
    event->real = datum->real;
    event->value = VPI_EMPTY;
    if (!datum->is_real) sv4_move(&event->value, &datum->packed);
    event->time = when;
    /* A delayed put is an Active-region event of its time slot, ordered with
     * process timer wakeups by when it was scheduled: a runtime carrier
     * process waits `#ticks` and then writes (decision S42-D5). */
    if (!llg_schedule_delayed_call("vpi_put_value delay", vpi_event_fire, event, ticks,
                                   &event->id)) {
        if (!event->is_real) sv4_destroy(&event->value);
        free(event);
        vpi_error("LLG_VPI_SCHEDULE", "the scheduler rejected the delayed VPI write");
        return NULL;
    }
    event->pending = 1;
    event->next = g_vpi.events;
    event->prev_link = &g_vpi.events;
    if (g_vpi.events) g_vpi.events->prev_link = &event->next;
    g_vpi.events = event;
    if (!return_event) return NULL;
    uint32_t index = alloc_slot(SLOT_EVENT);
    if (index == LLG_VPI_NONE) return NULL;
    g_vpi.slots[index].pointer = event;
    event->slot = index + 1u;
    return encode_handle(index);
}

vpiHandle vpi_put_value(vpiHandle handle, p_vpi_value input, p_vpi_time time,
                        PLI_INT32 flags) {
    vpi_enter();
    uint32_t index;
    llg_vpi_slot_t* slot = checked_handle(handle, &index);
    if (!slot) return NULL;
    int mode = flags & ~(vpiReturnEvent | vpiUserAllocFlag | vpiOneValue | vpiPropagateOff);
    int return_event = (flags & vpiReturnEvent) != 0;
    if (slot->kind == SLOT_EVENT) {
        if (mode != vpiCancelEvent) {
            vpi_error("LLG_VPI_FLAGS", "a vpiSchedEvent handle accepts only vpiCancelEvent");
            return NULL;
        }
        llg_vpi_event_t* event = (llg_vpi_event_t*)slot->pointer;
        /* 38.34: cancelling also frees the handle; an event that already
         * occurred is not an error. */
        event->slot = 0;
        if (event->pending) vpi_event_cancel(event);
        else free(event);
        release_slot(index);
        return NULL;
    }
    if (mode == vpiCancelEvent) {
        vpi_error("LLG_VPI_FLAGS", "vpiCancelEvent needs a vpiSchedEvent handle");
        return NULL;
    }
    if (slot->kind == SLOT_OBJECT && g_vpi.objects[slot->object].type == vpiNamedEvent) {
        /* 38.34: a put toggles the event; value_p may be NULL. */
        if (mode != vpiNoDelay) {
            vpi_error("LLG_VPI_UNSUPPORTED", "llg triggers a named event only with vpiNoDelay");
            return NULL;
        }
        if (llg_rt_running() && llg_region_is_read_only()) {
            vpi_error("LLG_VPI_VALUE", "a named event cannot be triggered from a read-only region");
            return NULL;
        }
        llg_event_trigger(g_vpi.objects[slot->object].event);
        return NULL;
    }
    if (slot->kind == SLOT_CALL) {
        llg_vpi_callsite_t* site = (llg_vpi_callsite_t*)slot->pointer;
        if (!site->registration || site->registration->data.type != vpiSysFunc) {
            vpi_error("LLG_VPI_VALUE", "a system task call has no value to put");
            return NULL;
        }
        if (mode != vpiNoDelay) {
            vpi_error("LLG_VPI_FLAGS", "system function values are put with vpiNoDelay (38.34)");
            return NULL;
        }
        llg_vpi_call_t* call = site->active;
        /* 38.34: a put to a system function that is not active is ignored. */
        if (!call) return NULL;
        llg_vpi_datum_t datum;
        if (!value_from_vpi(input, call->has_real_return ? 0 : llg_sv4_width(call->return_value),
                            call->has_real_return ? 0 : llg_sv4_signed(call->return_value),
                            call->has_real_return, &datum))
            return NULL;
        if (call->has_real_return) {
            call->real_return = datum.real;
        } else {
            sv4_t value = datum.is_real
                ? sv4_from_real(datum.real, llg_sv4_width(call->return_value),
                                llg_sv4_signed(call->return_value))
                : sv4_resize(datum.packed, llg_sv4_width(call->return_value),
                             llg_sv4_signed(call->return_value));
            sv4_replace(&call->return_value, value);
        }
        datum_destroy(&datum);
        return NULL;
    }
    /* 38.36.2: no write or event scheduling from cbReadOnlySynch or any
     * other read-only phase; the put is rejected with no state change. */
    if (g_vpi.phase == LLG_VPI_PHASE_RUNNING && llg_rt_running() && llg_region_is_read_only()) {
        vpi_set_errorf(vpiRun, vpiError, "LLG_VPI_READ_ONLY",
                       "vpi_put_value is not allowed in the read-only %s region",
                       llg_region_name(llg_current_region()));
        return NULL;
    }
    if (dyn_slot(slot)) {
        dyn_put_value(slot, input, mode);
        return NULL;
    }
    llg_vpi_target_t target;
    if (whole_unpacked_aggregate(slot)) {
        vpi_error("LLG_VPI_VALUE",
                  "an entire unpacked structure or union cannot receive a value; write its members");
        return NULL;
    }
    if (!handle_target(slot, &target)) {
        vpi_error("LLG_VPI_VALUE", slot->kind == SLOT_ARGUMENT
                      ? "this argument is an expression, not a writable object"
                      : "this VPI object cannot receive a value");
        return NULL;
    }
    llg_vpi_model_object_t* object = row_object(target.object);
    const char* reason = NULL;
    if (is_parameter_type(object->type)) {
        vpi_error("LLG_VPI_VALUE", "a parameter is a constant and cannot receive a value");
        return NULL;
    }
    if (!target_writable(&target, &reason)) {
        vpi_error("LLG_VPI_VALUE", reason);
        return NULL;
    }
    uint32_t width = target.span ? target.span : object->width;
    int is_signed = target.is_signed;
    switch (mode) {
        case vpiNoDelay:
        case vpiInertialDelay:
        case vpiTransportDelay:
        case vpiPureTransportDelay: {
            llg_vpi_datum_t datum;
            if (!value_from_vpi(input, object->is_real ? 0 : width, is_signed, object->is_real,
                                &datum))
                return NULL;
            if (mode == vpiNoDelay) {
                write_target(&target, &datum);
                datum_destroy(&datum);
                return NULL;
            }
            if (!time) {
                datum_destroy(&datum);
                vpi_error("LLG_VPI_TIME", "a delayed vpi_put_value needs a time");
                return NULL;
            }
            int ok;
            uint64_t ticks = ticks_from_time(time, object, &ok);
            if (!ok) {
                datum_destroy(&datum);
                vpi_error("LLG_VPI_TIME", "unsupported or negative vpi_put_value delay");
                return NULL;
            }
            vpiHandle event = schedule_put(&target, &datum, ticks, mode, return_event);
            datum_destroy(&datum);
            return event;
        }
        case vpiForceFlag: {
            llg_vpi_datum_t datum;
            if (!value_from_vpi(input, object->is_real ? 0 : width, is_signed, object->is_real,
                                &datum))
                return NULL;
            /* Not a force statement: no cbForce (decision A20-D9). */
            if (object->is_real) {
                llg_force_real_value(real_cell(&target), datum.real);
            } else {
                sv4_t value = datum.is_real ? sv4_from_real(datum.real, width, (int8_t)is_signed)
                                            : sv4_resize(datum.packed, width, (int8_t)is_signed);
                llg_force_part_t part = force_part(&target);
                llg_force_value_parts(&part, 1, value);
                sv4_destroy(&value);
            }
            datum_destroy(&datum);
            return NULL;
        }
        case vpiReleaseFlag: {
            /* Not a release statement: no cbRelease (decision A20-D9). */
            ++g_vpi.quiet_force;
            if (object->is_real) {
                llg_release_real(real_cell(&target));
            } else {
                llg_force_part_t part = force_part(&target);
                llg_release_parts(&part, 1, 0, 0);
            }
            --g_vpi.quiet_force;
            /* 38.34: value_p receives the value after the release. */
            if (input) {
                llg_vpi_datum_t datum;
                if (read_target(&target, &datum))
                    fill_value(&datum, is_signed, handle_type(slot), input);
                datum_destroy(&datum);
            }
            return NULL;
        }
        default:
            vpi_error("LLG_VPI_FLAGS", "unsupported vpi_put_value flags");
            return NULL;
    }
}

/* ── array values (38.16, 38.35) ────────────────────────────────────────── */

/* Whether `format` suits the elements of array row `object` (38.16, 38.35).
 * vpiIntVal, vpiTimeVal, vpiVectorVal and the raw formats read or write any
 * packed element; the small-integer and short real formats name the element
 * types each clause lists, which differ between get and put. */
static int array_format_suits(const llg_vpi_model_object_t* object, PLI_UINT32 format, int put) {
    int element = object->subtype;
    if (object->is_real) {
        if (format == vpiRealVal) return 1;
        return format == vpiShortRealVal && (object->flags & LLG_VPI_OBJ_SHORTREAL) != 0;
    }
    switch (format) {
        case vpiIntVal:
        case vpiTimeVal:
        case vpiVectorVal:
        case vpiRawFourStateVal:
        case vpiRawTwoStateVal:
            return 1;
        case vpiShortIntVal:
            return put ? element == vpiShortIntVar || element == vpiIntVar ||
                             element == vpiLongIntVar
                       : element == vpiShortIntVar || element == vpiByteVar;
        case vpiLongIntVal:
            return put ? element == vpiLongIntVar
                       : element == vpiLongIntVar || element == vpiShortIntVar ||
                             element == vpiByteVar;
        default:
            return 0;
    }
}

/* Bytes one element occupies in the application's buffer for `format`. */
static size_t array_element_bytes(PLI_UINT32 format, uint32_t width) {
    size_t groups = ((size_t)width + 7u) / 8u;
    switch (format) {
        case vpiIntVal: return sizeof(PLI_INT32);
        case vpiShortIntVal: return sizeof(PLI_INT16);
        case vpiLongIntVal: return sizeof(PLI_INT64);
        case vpiRealVal: return sizeof(double);
        case vpiShortRealVal: return sizeof(float);
        case vpiTimeVal: return sizeof(s_vpi_time);
        case vpiVectorVal: return word_count(width) * sizeof(s_vpi_vecval);
        case vpiRawFourStateVal: return groups * 2u;
        default: return groups; /* vpiRawTwoStateVal */
    }
}

/* Validate the array, format and section of a value-array call and return
 * the array row and the flat index of the first element. Every failure
 * records the error and returns 0 before any value moves. */
static int array_section(vpiHandle handle, const s_vpi_arrayvalue* values, const PLI_INT32* index,
                         PLI_UINT32 num, int put, uint32_t* row, uint64_t* first) {
    const char* routine = put ? "vpi_put_value_array" : "vpi_get_value_array";
    llg_vpi_slot_t* slot = checked_handle(handle, NULL);
    if (!slot) return 0;
    llg_vpi_model_object_t* object = slot->kind == SLOT_OBJECT ? row_object(slot->object) : NULL;
    if (!object || !is_array_type(object->type) || !has_storage(object)) {
        vpi_set_errorf(vpi_state(), vpiError, "LLG_VPI_HANDLE", "%s needs an unpacked array handle",
                       routine);
        return 0;
    }
    /* 38.16/38.35: elements of an unpacked type are not allowed. */
    const llg_vpi_typespec_t* element = element_typespec(object);
    if (is_aggregate_typespec(element) && (element->flags & LLG_VPI_TS_UNPACKED)) {
        vpi_set_errorf(vpi_state(), vpiError, "LLG_VPI_VALUE",
                       "%s: the elements of this array are unpacked structures or unions",
                       routine);
        return 0;
    }
    if (!array_format_suits(object, values->format, put)) {
        vpi_set_errorf(vpi_state(), vpiError, "LLG_VPI_FORMAT",
                       "%s: format %d is unsupported or does not suit the array elements",
                       routine, (int)values->format);
        return 0;
    }
    if (!index) {
        vpi_set_errorf(vpi_state(), vpiError, "LLG_VPI_INDEX", "%s needs index_p", routine);
        return 0;
    }
    uint64_t flat = 0;
    for (uint32_t d = 0; d < object->dim_count; ++d) {
        int64_t left = object->dims[2u * d], right = object->dims[2u * d + 1u];
        uint64_t offset;
        if (!index_offset(left, right, index[d], &offset)) {
            vpi_set_errorf(vpi_state(), vpiError, "LLG_VPI_INDEX",
                           "%s: index %d of dimension %u is outside [%lld:%lld]", routine,
                           (int)index[d], (unsigned)d + 1u, (long long)left, (long long)right);
            return 0;
        }
        uint64_t extent = (uint64_t)(left > right ? left - right : right - left) + 1u;
        flat = flat * extent + offset;
    }
    uint64_t count = g_vpi.element_counts[slot->object];
    if (num == 0 || num > count - flat) {
        vpi_set_errorf(vpi_state(), vpiError, "LLG_VPI_INDEX",
                       "%s: %u elements from the start index exceed the array", routine,
                       (unsigned)num);
        return 0;
    }
    *row = slot->object;
    *first = flat;
    return 1;
}

/* 38.16: values of `num` consecutive elements, fastest-varying index last.
 * Without vpiUserAllocFlag the values live in simulator storage that the
 * next call overwrites; on any error the value pointer becomes NULL. */
void vpi_get_value_array(vpiHandle handle, p_vpi_arrayvalue arrayvalue_p, PLI_INT32* index_p,
                         PLI_UINT32 num) {
    vpi_enter();
    if (!arrayvalue_p) {
        vpi_error("LLG_VPI_ARGUMENT", "null vpi_get_value_array arrayvalue_p");
        return;
    }
    int user = (arrayvalue_p->flags & vpiUserAllocFlag) != 0;
    void* buffer = user ? (void*)arrayvalue_p->value.rawvals : NULL;
    memset(&arrayvalue_p->value, 0, sizeof(arrayvalue_p->value));
    /* Decision A20-D5: only vpiUserAllocFlag is a get flag. */
    if (arrayvalue_p->flags & ~(PLI_UINT32)vpiUserAllocFlag) {
        vpi_error("LLG_VPI_FLAGS", "vpi_get_value_array accepts only vpiUserAllocFlag");
        return;
    }
    if (user && !buffer) {
        vpi_error("LLG_VPI_ARGUMENT", "vpiUserAllocFlag with a null value buffer");
        return;
    }
    uint32_t row;
    uint64_t first;
    if (!array_section(handle, arrayvalue_p, index_p, num, 0, &row, &first)) return;
    llg_vpi_model_object_t* object = row_object(row);
    PLI_UINT32 format = arrayvalue_p->format;
    uint32_t width = object->width;
    size_t unit = array_element_bytes(format, width);
    if (!user) {
        if (unit && (size_t)num > SIZE_MAX / unit) {
            vpi_error("LLG_VPI_NOMEM", "vpi_get_value_array section is too large");
            return;
        }
        size_t need = unit * (size_t)num;
        if (need > g_vpi.array_values_capacity) {
            void* grown = realloc(g_vpi.array_values, need);
            if (!grown) {
                vpi_error("LLG_VPI_NOMEM", "vpi_get_value_array allocation failed");
                return;
            }
            g_vpi.array_values = grown;
            g_vpi.array_values_capacity = need;
        }
        buffer = g_vpi.array_values;
    }
    unsigned char* out = (unsigned char*)buffer;
    size_t words = word_count(width);
    size_t groups = ((size_t)width + 7u) / 8u;
    for (PLI_UINT32 i = 0; i < num; ++i, out += unit) {
        uint64_t flat = first + i;
        if (object->is_real) {
            double value = object->real[flat];
            if (format == vpiShortRealVal) {
                float narrow = (float)value;
                memcpy(out, &narrow, sizeof(narrow));
            } else {
                memcpy(out, &value, sizeof(value));
            }
            continue;
        }
        s_vpi_vecval* bits = scratch_words(words);
        if (!bits) {
            vpi_error("LLG_VPI_NOMEM", "VPI value conversion allocation failed");
            return;
        }
        export_words(object->packed[flat], bits);
        switch (format) {
            case vpiIntVal: {
                PLI_INT32 value = low_int(bits, width, object->is_signed);
                memcpy(out, &value, sizeof(value));
                break;
            }
            case vpiShortIntVal: {
                PLI_INT16 value = (PLI_INT16)low_int(bits, width, object->is_signed);
                memcpy(out, &value, sizeof(value));
                break;
            }
            case vpiLongIntVal: {
                uint64_t value = low_u64(bits, width);
                if (object->is_signed && width < 64u && ((value >> (width - 1u)) & 1u))
                    value |= ~((UINT64_C(1) << width) - 1u);
                PLI_INT64 signed_value = (PLI_INT64)value;
                memcpy(out, &signed_value, sizeof(signed_value));
                break;
            }
            case vpiTimeVal: {
                uint64_t value = low_u64(bits, width);
                s_vpi_time time;
                memset(&time, 0, sizeof(time));
                time.type = vpiSimTime;
                time.high = (PLI_UINT32)(value >> 32);
                time.low = (PLI_UINT32)value;
                memcpy(out, &time, sizeof(time));
                break;
            }
            case vpiVectorVal:
                memcpy(out, bits, words * sizeof(*bits));
                break;
            default: {
                /* Raw formats: A then B byte groups, least significant byte
                 * first. The two-state form reads x and z as 0 (decision
                 * A20-D6). */
                int four = format == vpiRawFourStateVal;
                for (size_t group = 0; group < groups; ++group) {
                    unsigned shift = (unsigned)(group % 4u) * 8u;
                    uint32_t a = bits[group / 4u].aval >> shift;
                    uint32_t b = bits[group / 4u].bval >> shift;
                    out[group] = (unsigned char)(four ? a : a & ~b);
                    if (four) out[groups + group] = (unsigned char)b;
                }
                break;
            }
        }
    }
    arrayvalue_p->value.rawvals = (PLI_BYTE8*)buffer;
}

/* One element value from the application's buffer `in` (one element in
 * `format`) for array row `object`. */
static int array_element_datum(const llg_vpi_model_object_t* object, PLI_UINT32 format,
                               const unsigned char* in, llg_vpi_datum_t* out) {
    memset(out, 0, sizeof(*out));
    out->packed = VPI_EMPTY;
    if (object->is_real) {
        out->is_real = 1;
        if (format == vpiShortRealVal) {
            float narrow;
            memcpy(&narrow, in, sizeof(narrow));
            out->real = (double)narrow;
        } else {
            memcpy(&out->real, in, sizeof(out->real));
        }
        return 1;
    }
    uint32_t width = object->width;
    s_vpi_value value;
    memset(&value, 0, sizeof(value));
    switch (format) {
        case vpiIntVal:
            value.format = vpiIntVal;
            memcpy(&value.value.integer, in, sizeof(value.value.integer));
            return value_from_vpi(&value, width, object->is_signed, 0, out);
        case vpiShortIntVal: {
            PLI_INT16 narrow;
            memcpy(&narrow, in, sizeof(narrow));
            value.format = vpiIntVal;
            value.value.integer = narrow;
            return value_from_vpi(&value, width, object->is_signed, 0, out);
        }
        case vpiTimeVal: {
            s_vpi_time time;
            memcpy(&time, in, sizeof(time));
            value.format = vpiTimeVal;
            value.value.time = &time;
            return value_from_vpi(&value, width, object->is_signed, 0, out);
        }
        case vpiVectorVal:
            value.format = vpiVectorVal;
            value.value.vector = (p_vpi_vecval)(void*)in;
            return value_from_vpi(&value, width, object->is_signed, 0, out);
        default:
            break;
    }
    size_t count = word_count(width);
    s_vpi_vecval* words = scratch_words(count < 2u ? 2u : count);
    if (!words) {
        vpi_error("LLG_VPI_NOMEM", "VPI value conversion allocation failed");
        return 0;
    }
    if (format == vpiLongIntVal) {
        PLI_INT64 wide;
        memcpy(&wide, in, sizeof(wide));
        words[0].aval = (uint32_t)(uint64_t)wide;
        words[1].aval = (uint32_t)((uint64_t)wide >> 32);
        for (size_t i = 2; i < count; ++i) words[i].aval = wide < 0 ? UINT32_MAX : 0u;
    } else {
        /* Raw formats. 38.35: a two-state array ignores the B bytes of
         * vpiRawFourStateVal; vpiRawTwoStateVal has none (B is 0). */
        size_t groups = ((size_t)width + 7u) / 8u;
        int with_b = format == vpiRawFourStateVal && !(object->flags & LLG_VPI_OBJ_TWO_STATE);
        for (size_t group = 0; group < groups; ++group) {
            unsigned shift = (unsigned)(group % 4u) * 8u;
            words[group / 4u].aval |= (uint32_t)in[group] << shift;
            if (with_b) words[group / 4u].bval |= (uint32_t)in[groups + group] << shift;
        }
    }
    uint32_t top = width % 32u;
    if (top) {
        words[count - 1u].aval &= (1u << top) - 1u;
        words[count - 1u].bval &= (1u << top) - 1u;
    }
    out->packed = sv4_zero(width, object->is_signed);
    llg_sv4_import_vpi32(&out->packed, words, count, sizeof(*words));
    return 1;
}

/* 38.35: write `num` consecutive elements as vpi_put_value with vpiNoDelay
 * would, each through the procedural write path (forces still win).
 * vpiOneValue applies the first buffer element to the whole section.
 * vpiPropagateOff is accepted; llg still notifies every fanout, which the
 * clause's own caveat makes the always-correct result (decision A20-D7). */
void vpi_put_value_array(vpiHandle handle, p_vpi_arrayvalue arrayvalue_p, PLI_INT32* index_p,
                         PLI_UINT32 num) {
    vpi_enter();
    if (!arrayvalue_p) {
        vpi_error("LLG_VPI_ARGUMENT", "null vpi_put_value_array arrayvalue_p");
        return;
    }
    if (arrayvalue_p->flags & ~(PLI_UINT32)(vpiPropagateOff | vpiOneValue | vpiNoDelay)) {
        vpi_error("LLG_VPI_FLAGS",
                  "vpi_put_value_array accepts only vpiPropagateOff, vpiOneValue and vpiNoDelay");
        return;
    }
    if (!arrayvalue_p->value.rawvals) {
        vpi_error("LLG_VPI_ARGUMENT", "null vpi_put_value_array value buffer");
        return;
    }
    if (g_vpi.phase == LLG_VPI_PHASE_RUNNING && llg_rt_running() && llg_region_is_read_only()) {
        vpi_set_errorf(vpiRun, vpiError, "LLG_VPI_READ_ONLY",
                       "vpi_put_value_array is not allowed in the read-only %s region",
                       llg_region_name(llg_current_region()));
        return;
    }
    uint32_t row;
    uint64_t first;
    if (!array_section(handle, arrayvalue_p, index_p, num, 1, &row, &first)) return;
    llg_vpi_model_object_t* object = row_object(row);
    /* Decision A20-D8: as for vpi_put_value on a net array element. */
    if (object->type == vpiNetArray) {
        vpi_error("LLG_VPI_VALUE",
                  "elements of a net array are published from their nets and are read-only");
        return;
    }
    size_t unit = array_element_bytes(arrayvalue_p->format, object->width);
    int one = (arrayvalue_p->flags & vpiOneValue) != 0;
    const unsigned char* in = (const unsigned char*)arrayvalue_p->value.rawvals;
    for (PLI_UINT32 i = 0; i < num; ++i) {
        llg_vpi_datum_t datum;
        if (!array_element_datum(object, arrayvalue_p->format, one ? in : in + (size_t)i * unit,
                                 &datum))
            return;
        llg_vpi_target_t target;
        memset(&target, 0, sizeof(target));
        target.object = row;
        target.element = first + i;
        target.is_signed = object->is_signed;
        write_target(&target, &datum);
        datum_destroy(&datum);
    }
}

void vpi_get_time(vpiHandle object, p_vpi_time time) {
    vpi_enter();
    llg_vpi_model_object_t* scope = NULL;
    uint64_t unit = 0;
    /* A time queue reports its own time (37.76). */
    int queued = 0;
    uint64_t queue_time = 0;
    if (object) {
        llg_vpi_slot_t* slot = checked_handle(object, NULL);
        if (!slot) return;
        if (slot->kind == SLOT_TIMEQ) {
            queued = 1;
            queue_time = (uint64_t)slot->left;
        }
        scope = row_object(slot->object);
        if (slot->kind == SLOT_CALL || slot->kind == SLOT_ARGUMENT) {
            llg_vpi_callsite_t* site = (llg_vpi_callsite_t*)slot->pointer;
            unit = site->time_unit_fs;
        }
    }
    if (!time) {
        vpi_error("LLG_VPI_ARGUMENT", "null VPI time output");
        return;
    }
    if (time->type == vpiSuppressTime) return;
    if (time->type != vpiSimTime && time->type != vpiScaledRealTime) {
        vpi_error("LLG_VPI_TIME", "unsupported VPI time format");
        return;
    }
    /* cbEndOfSimulation may run after the scheduler state is released; the
     * current time is then the time the simulation ended. */
    uint64_t now = queued ? queue_time : llg_rt_end_time();
    if (time->type == vpiSimTime) {
        time->high = (PLI_UINT32)(now >> 32);
        time->low = (PLI_UINT32)now;
        return;
    }
    uint64_t precision = llg_time_precision_fs();
    if (!unit) unit = time_unit_of(scope);
    if (!precision) precision = 1;
    if (!unit) unit = precision;
    time->real = (double)((long double)now * precision / unit);
}

/* Identity of the simulation object a handle refers to (38.3). */
static int handle_identity(const llg_vpi_slot_t* slot, uint32_t index, uint64_t identity[4]) {
    llg_vpi_target_t target;
    if (rts_identity(slot, identity)) return 1;
    if (slot->kind == SLOT_CONN || slot->kind == SLOT_PRIMTERM || slot->kind == SLOT_PORTBIT ||
        slot->kind == SLOT_REFOBJ || slot->kind == SLOT_TERM || slot->kind == SLOT_CODE ||
        slot->kind == SLOT_CABIT) {
        identity[0] = 2u + slot->kind;
        identity[1] = slot->object;
        identity[2] = 0;
        identity[3] = (uint64_t)(uint32_t)slot->aux;
        return 1;
    }
    if (slot->kind == SLOT_TYPESPEC) {
        /* A typespec and the declaration that owns it (A19-D4). */
        identity[0] = 2u + slot->kind;
        identity[1] = (uint64_t)(uint32_t)slot->aux;
        identity[2] = slot->object;
        identity[3] = (uint64_t)slot->left;
        return 1;
    }
    if (slot->kind == SLOT_IMP) {
        identity[0] = 2u + slot->kind;
        identity[1] = slot->object;
        identity[2] = ((uint64_t)(uint32_t)slot->left << 32) | (uint32_t)slot->right;
        identity[3] = (uint64_t)(uint32_t)slot->aux;
        return 1;
    }
    if (slot->kind == SLOT_MEMBER) {
        /* Members of a union share bits; the member node tells them apart. */
        identity[0] = 2u + slot->kind;
        identity[1] = slot->object;
        identity[2] = slot->element;
        identity[3] = (uint64_t)(uint32_t)slot->aux;
        return 1;
    }
    if (handle_target(slot, &target) || slot->kind == SLOT_OBJECT) {
        identity[0] = 1;
        identity[1] = slot->object;
        identity[2] = slot->kind == SLOT_OBJECT ? LLG_VPI_NO_ELEMENT : slot->element;
        identity[3] = slot->kind == SLOT_SELECT ? ((uint64_t)slot->lsb << 32) | slot->span : 0;
        return 1;
    }
    identity[0] = 2u + slot->kind;
    identity[1] = slot->kind == SLOT_CALL || slot->kind == SLOT_SYSTF ||
                  slot->kind == SLOT_EVENT ? (uint64_t)(uintptr_t)slot->pointer
                  : slot->kind == SLOT_ASSERTION || slot->kind == SLOT_FSM ||
                    slot->kind == SLOT_FSMSTATE ? (uint64_t)slot->left : index;
    identity[2] = slot->kind == SLOT_CONSTANT ? (uint64_t)slot->left
                  : slot->kind == SLOT_FSMSTATE ? (uint64_t)(uint32_t)slot->aux : 0;
    identity[3] = 0;
    return 1;
}

PLI_INT32 vpi_compare_objects(vpiHandle object1, vpiHandle object2) {
    vpi_enter();
    uint32_t first_index, second_index;
    llg_vpi_slot_t* first = checked_handle(object1, &first_index);
    if (!first) return 0;
    llg_vpi_slot_t* second = checked_handle(object2, &second_index);
    if (!second) return 0;
    int dynamic = dyn_compare(first, first_index, second, second_index);
    if (dynamic >= 0) return dynamic;
    uint64_t a[4], b[4];
    handle_identity(first, first_index, a);
    handle_identity(second, second_index, b);
    if (first->kind == SLOT_CONSTANT || second->kind == SLOT_CONSTANT ||
        first->kind == SLOT_ARGUMENT || second->kind == SLOT_ARGUMENT ||
        first->kind == SLOT_ITERATOR || second->kind == SLOT_ITERATOR ||
        first->kind == SLOT_RANGE || second->kind == SLOT_RANGE)
        return first_index == second_index;
    return memcmp(a, b, sizeof(a)) == 0;
}

PLI_INT32 vpi_chk_error(p_vpi_error_info output) {
    /* 38.2: returns the severity level and leaves the error status as is. */
    if (!g_vpi.error_pending) return 0;
    if (output) {
        output->state = g_vpi.error_state;
        output->level = g_vpi.error_level;
        output->message = (PLI_BYTE8*)g_vpi.error_message;
        output->product = (PLI_BYTE8*)g_vpi.error_product;
        output->code = (PLI_BYTE8*)g_vpi.error_code;
        output->file = NULL;
        output->line = g_vpi.error_line;
    }
    return g_vpi.error_level;
}

static void vpi_cb_handle_released(struct llg_vpi_cb* cb);
static void fired_free(uint32_t index);

static PLI_INT32 release_handle(vpiHandle handle) {
    uint32_t index;
    llg_vpi_slot_t* slot = checked_handle(handle, &index);
    if (!slot) return 0;
    switch (slot->kind) {
        case SLOT_OBJECT:
        case SLOT_SYSTF:
            /* Catalog objects and registrations live for the whole run. */
            return 1;
        case SLOT_CALLBACK:
            /* Releasing a callback handle does not remove the callback; a
             * one-shot one then leaves no record behind when it ran. */
            vpi_cb_handle_released((struct llg_vpi_cb*)slot->pointer);
            return 1;
        case SLOT_FIRED_CB:
            /* The handle is the only reference to a fired callback. */
            fired_free(index);
            return 1;
        case SLOT_CALL: {
            llg_vpi_callsite_t* site = (llg_vpi_callsite_t*)slot->pointer;
            if (site->call_slot == index + 1u) site->call_slot = 0;
            release_slot(index);
            return 1;
        }
        case SLOT_EVENT: {
            llg_vpi_event_t* event = (llg_vpi_event_t*)slot->pointer;
            /* 38.34: releasing the handle does not affect the event. */
            if (event) {
                event->slot = 0;
                if (!event->pending) free(event);
            }
            release_slot(index);
            return 1;
        }
        default:
            release_slot(index);
            return 1;
    }
}

PLI_INT32 vpi_release_handle(vpiHandle handle) {
    vpi_enter();
    return release_handle(handle);
}

PLI_INT32 vpi_free_object(vpiHandle handle) {
    vpi_enter();
    return release_handle(handle);
}

static llg_vpi_callsite_t* userdata_site(vpiHandle handle) {
    llg_vpi_slot_t* slot = checked_handle(handle, NULL);
    if (!slot) return NULL;
    if (slot->kind != SLOT_CALL) {
        vpi_error("LLG_VPI_HANDLE", "user data belongs to a system task or function call handle");
        return NULL;
    }
    return (llg_vpi_callsite_t*)slot->pointer;
}

void* vpi_get_userdata(vpiHandle handle) {
    vpi_enter();
    llg_vpi_callsite_t* site = userdata_site(handle);
    return site ? site->userdata : NULL;
}

PLI_INT32 vpi_put_userdata(vpiHandle handle, void* userdata) {
    vpi_enter();
    llg_vpi_callsite_t* site = userdata_site(handle);
    if (!site) return 0;
    site->userdata = userdata;
    return 1;
}

/* ── simulation callbacks (SV 38.36, Table 4-1) ─────────────────────────── */

/* Callback records. A record is owned by the registry list from registration
 * until it is removed or, for a one-shot reason, retired after it ran (its
 * handle then names a compact fired copy, llg_vpi_cb_done_t). A
 * dispatch never frees a record: removal during a dispatch moves it to the
 * retired list, freed when the outermost dispatch returns, so callbacks may
 * register and remove any callback (including themselves) while running. */

typedef enum {
    CB_ACTION = 0, /* cbEndOfCompile, cbStartOfSimulation, cbEndOfSimulation */
    CB_TIME,       /* a region callback of one time slot */
    CB_NEXT_TIME,  /* cbNextSimTime */
    CB_VALUE,      /* cbValueChange */
    CB_FORCE,      /* cbForce, cbRelease, cbAssign, cbDeassign */
    CB_DISABLE,    /* cbDisable */
    CB_ASSERTION,  /* vpi_register_assertion_cb (39.4.2) */
    CB_ASSERT_SYS, /* cbAssertionSys* (39.4.1) */
    CB_TCHK,       /* cbTchkViolation */
    CB_OBJECT,     /* cbCreateObj, cbReclaimObj, cbEndOfObject (llg_vpi_dynamic.c) */
    CB_RUNTIME,    /* frame and thread reasons (llg_vpi_runtime.c) */
} llg_vpi_cb_kind_t;

typedef struct llg_vpi_cb {
    s_cb_data data;           /* registration; time/value point at the copies */
    s_vpi_time time;
    s_vpi_value value;        /* format only */
    uint32_t slot;            /* vpiCallback handle index */
    uint64_t sequence;
    uint64_t registered_at;   /* simulation time of the registration */
    llg_vpi_cb_kind_t kind;
    int removed;
    int handle_released;      /* vpi_release_handle ran: no record once fired */
    int scheduled;            /* a region callback is queued */
    int waiting;              /* cbNextSimTime not yet queued */
    llg_region_callback_id_t id;
    llg_vpi_target_t target;  /* cbValueChange object */
    int value_type;           /* vpiType that formats the value */
    int value_signed;
    int32_t index;            /* reported s_cb_data index */
    sv4_t last;               /* last value of a watched select */
    const llg_vpi_model_object_t* scope; /* time scaling object */
    struct llg_vpi_cb* next;
    struct llg_vpi_cb** prev_link;
    /* cbValueChange: one watch on the watched storage, or for a whole
     * unpacked array one per element (decision S43-D7). */
    struct llg_vpi_watch* watches;
    uint64_t watch_total;
    /* CB_FORCE: 0 every statement (NULL obj), 1 statements writing `target`,
     * 2 statements writing an element of the array row `target.object`. */
    int force_filter;
    /* CB_DISABLE on a system task or function call: its call site (the
     * record watches the scopes that enclose it), else NULL and the record
     * watches the row `target.object`. */
    llg_vpi_callsite_t* site;
    /* CB_RUNTIME on a thread or frame: the thread (registry slot, process
     * identity) and, for cbEndOfFrame, the frame's serial. */
    int thread_slot;
    uint64_t thread_identity;
    uint64_t frame_serial;
    /* CB_ASSERTION: the routine, its assertion item and whether the reason
     * is an attempt event (the item is then watched). data.obj is the
     * record's own assertion handle, data.cb_rtn NULL. */
    vpi_assertion_callback_func* assertion_rtn;
    uint32_t assertion_item;
    int assertion_attempt;
    /* The failExpr handle the record passes on cbAssertionFailure, owned
     * like data.obj; NULL until the first failure. */
    vpiHandle fail_expr;
    /* A record placed on a sequence or property instance: its code row + 1
     * (data.obj is then the record's instance handle), else 0. */
    uint32_t instance_code;
} llg_vpi_cb_t;

static void assertion_cb_release(llg_vpi_cb_t* cb);

/* What a watch key is: published storage whose value the callback reports,
 * a named event's llg_event_object_t, or the change marker of a value VPI
 * cannot read (a class variable): both of the latter report a NULL value. */
#define LLG_VPI_WATCH_VALUE 0
#define LLG_VPI_WATCH_EVENT 1
#define LLG_VPI_WATCH_OPAQUE 2

/* One watched storage address of a cbValueChange record. */
typedef struct llg_vpi_watch {
    const void* key;
    llg_vpi_cb_t* cb;
    uint64_t element;         /* array element of a whole-array record, else LLG_VPI_NO_ELEMENT */
    int event;                /* LLG_VPI_WATCH_* */
    struct llg_vpi_watch* next;
} llg_vpi_watch_t;

static void vpi_cb_free(llg_vpi_cb_t* cb) {
    sv4_destroy(&cb->last);
    free(cb->watches);
    free(cb);
}

/* What a one-shot callback keeps after it ran, or when it was registered
 * for an action that already occurred (decision S43-D4): its handle stays
 * valid for vpi_get_cb_info until vpi_remove_cb or vpi_release_handle frees
 * this compact copy of the registration. */
typedef struct {
    s_cb_data data;           /* time/value point at the copies below */
    s_vpi_time time;
    s_vpi_value value;        /* format only */
    uint32_t prev;            /* age list neighbours: slot index + 1 */
    uint32_t next;
} llg_vpi_cb_done_t;

static llg_vpi_cb_done_t* fired_at(uint32_t link) {
    return (llg_vpi_cb_done_t*)g_vpi.slots[link - 1u].pointer;
}

/* Free a fired callback's copy and its handle. */
static void fired_free(uint32_t index) {
    llg_vpi_cb_done_t* done = (llg_vpi_cb_done_t*)g_vpi.slots[index].pointer;
    if (done->prev) fired_at(done->prev)->next = done->next;
    else g_vpi.fired_head = done->next;
    if (done->next) fired_at(done->next)->prev = done->prev;
    else g_vpi.fired_tail = done->prev;
    --g_vpi.fired_count;
    free(done);
    release_slot(index);
}

static llg_vpi_cb_done_t* cb_done_new(const s_cb_data* data) {
    if (g_vpi.fired_count >= LLG_VPI_MAX_FIRED_CALLBACKS) fired_free(g_vpi.fired_head - 1u);
    llg_vpi_cb_done_t* done = (llg_vpi_cb_done_t*)calloc(1, sizeof(*done));
    if (!done) return NULL;
    done->data = *data;
    if (data->time) done->time = *data->time;
    if (data->value) done->value.format = data->value->format;
    done->data.time = data->time ? &done->time : NULL;
    done->data.value = data->value ? &done->value : NULL;
    return done;
}

/* Give slot `index` to `done`, the newest fired callback. */
static void fired_attach(uint32_t index, llg_vpi_cb_done_t* done) {
    g_vpi.slots[index].kind = SLOT_FIRED_CB;
    g_vpi.slots[index].pointer = done;
    done->prev = g_vpi.fired_tail;
    done->next = 0;
    if (g_vpi.fired_tail) fired_at(g_vpi.fired_tail)->next = index + 1u;
    else g_vpi.fired_head = index + 1u;
    g_vpi.fired_tail = index + 1u;
    ++g_vpi.fired_count;
}

static size_t watch_bucket(const void* key, size_t capacity) {
    uint64_t hash = (uint64_t)(uintptr_t)key;
    hash ^= hash >> 33;
    hash *= UINT64_C(0xff51afd7ed558ccd);
    hash ^= hash >> 33;
    return (size_t)hash & (capacity - 1u);
}

static int watch_insert(llg_vpi_watch_t* watch) {
    if (g_vpi.watch_count + 1u > g_vpi.watch_capacity - g_vpi.watch_capacity / 4u) {
        size_t capacity = g_vpi.watch_capacity ? g_vpi.watch_capacity * 2u : 64u;
        llg_vpi_watch_t** table = (llg_vpi_watch_t**)calloc(capacity, sizeof(*table));
        if (!table) return 0;
        for (size_t i = 0; i < g_vpi.watch_capacity; ++i) {
            for (llg_vpi_watch_t* cursor = g_vpi.watch_buckets[i]; cursor;) {
                llg_vpi_watch_t* next = cursor->next;
                size_t bucket = watch_bucket(cursor->key, capacity);
                cursor->next = table[bucket];
                table[bucket] = cursor;
                cursor = next;
            }
        }
        free(g_vpi.watch_buckets);
        g_vpi.watch_buckets = table;
        g_vpi.watch_capacity = capacity;
    }
    if (watch->event == LLG_VPI_WATCH_EVENT)
        llg_event_watch_add((llg_event_object_t*)(uintptr_t)watch->key);
    else if (!llg_value_watch_add(watch->key)) return 0;
    size_t bucket = watch_bucket(watch->key, g_vpi.watch_capacity);
    watch->next = g_vpi.watch_buckets[bucket];
    g_vpi.watch_buckets[bucket] = watch;
    ++g_vpi.watch_count;
    return 1;
}

static void watch_unlink(llg_vpi_watch_t* watch) {
    size_t bucket = watch_bucket(watch->key, g_vpi.watch_capacity);
    for (llg_vpi_watch_t** link = &g_vpi.watch_buckets[bucket]; *link; link = &(*link)->next) {
        if (*link != watch) continue;
        *link = watch->next;
        watch->next = NULL;
        --g_vpi.watch_count;
        if (watch->event == LLG_VPI_WATCH_EVENT)
            llg_event_watch_remove((llg_event_object_t*)(uintptr_t)watch->key);
        else llg_value_watch_remove(watch->key);
        return;
    }
}

/* Insert every watch of a record, or none. */
static int cb_watches_insert(llg_vpi_cb_t* cb) {
    for (uint64_t i = 0; i < cb->watch_total; ++i) {
        if (watch_insert(&cb->watches[i])) continue;
        while (i-- > 0) watch_unlink(&cb->watches[i]);
        return 0;
    }
    return 1;
}

static void cb_watches_unlink(llg_vpi_cb_t* cb) {
    for (uint64_t i = 0; i < cb->watch_total; ++i) watch_unlink(&cb->watches[i]);
}

/* Detach a record from every trigger, invalidate its handle (or, when
 * `done` is given, hand the handle to that fired-callback copy) and free the
 * record now or, inside a dispatch, when the outermost dispatch ends. */
static void vpi_cb_detach(llg_vpi_cb_t* cb, llg_vpi_cb_done_t* done) {
    if (cb->removed) return;
    cb->removed = 1;
    if (cb->scheduled) {
        llg_cancel_region_callback(cb->id);
        cb->scheduled = 0;
    }
    if (cb->kind == CB_VALUE) cb_watches_unlink(cb);
    if (cb->kind == CB_FORCE) --g_vpi.force_cbs;
    if (cb->kind == CB_DISABLE) --g_vpi.disable_cbs;
    if (cb->kind == CB_ASSERTION) assertion_cb_release(cb);
    if (cb->kind == CB_TCHK) --g_vpi.tchk_cbs;
    if (cb->kind == CB_OBJECT) dyn_object_cb_removed();
    if (cb->kind == CB_RUNTIME) rts_cb_removed(cb);
    if (cb->waiting) {
        cb->waiting = 0;
        --g_vpi.next_time_pending;
    }
    if (done) {
        fired_attach(cb->slot, done);
    } else {
        release_slot(cb->slot);
    }
    *cb->prev_link = cb->next;
    if (cb->next) cb->next->prev_link = cb->prev_link;
    else g_vpi.cbs_tail = cb->prev_link;
    if (g_vpi.dispatch_depth) {
        cb->next = g_vpi.retired;
        g_vpi.retired = cb;
    } else {
        vpi_cb_free(cb);
    }
}

static void vpi_cb_retire(llg_vpi_cb_t* cb) { vpi_cb_detach(cb, NULL); }

static void vpi_cb_handle_released(llg_vpi_cb_t* cb) { cb->handle_released = 1; }

/* A one-shot callback ran: keep its handle valid as a fired copy unless the
 * application released the handle (or removed the callback while it ran).
 * Without memory for the copy the handle goes stale, as before S43-D4. */
static void vpi_cb_fired(llg_vpi_cb_t* cb) {
    if (cb->removed) return;
    vpi_cb_detach(cb, cb->handle_released ? NULL : cb_done_new(&cb->data));
}

static void dispatch_begin(void) { ++g_vpi.dispatch_depth; }

static void dispatch_end(void) {
    if (--g_vpi.dispatch_depth) return;
    while (g_vpi.retired) {
        llg_vpi_cb_t* cb = g_vpi.retired;
        g_vpi.retired = cb->next;
        vpi_cb_free(cb);
    }
}

/* Simulation time in the requested representation; NULL for
 * vpiSuppressTime. `out` is the storage behind the returned pointer. */
static p_vpi_time callback_time(int type, const llg_vpi_model_object_t* scope, uint64_t now,
                                s_vpi_time* out) {
    memset(out, 0, sizeof(*out));
    if (type == vpiScaledRealTime) {
        uint64_t precision = llg_time_precision_fs();
        uint64_t unit = time_unit_of(scope);
        if (!precision) precision = 1;
        if (!unit) unit = precision;
        out->type = vpiScaledRealTime;
        out->real = (double)((long double)now * precision / unit);
        return out;
    }
    if (type != vpiSimTime) return NULL;
    out->type = vpiSimTime;
    out->high = (PLI_UINT32)(now >> 32);
    out->low = (PLI_UINT32)now;
    return out;
}

/* Run one callback routine. A nonzero return marks the VPI application
 * failed and ends the simulation, as for the start/end reasons. */
static void invoke_cb(llg_vpi_cb_t* cb, s_cb_data* data) {
    if (data->cb_rtn(data) != 0) {
        g_vpi.failed = 1;
        vpi_set_error(vpi_state(), vpiError, "LLG_VPI_CALLBACK", "VPI callback returned failure");
        llg_rt_request_finish();
    }
    (void)cb;
}

static void base_data(const llg_vpi_cb_t* cb, s_cb_data* out) {
    memset(out, 0, sizeof(*out));
    out->reason = cb->data.reason;
    out->cb_rtn = cb->data.cb_rtn;
    out->obj = cb->data.obj;
    out->user_data = cb->data.user_data;
}

/* Collect the live records matching `match`, in registration order, before
 * running any of them: records registered by a running callback wait for the
 * next occurrence, and removed ones are skipped. */
#define LLG_VPI_DISPATCH_INLINE 16

typedef int (*cb_match_fn)(const llg_vpi_cb_t* cb, const void* context);

static llg_vpi_cb_t** collect_cbs(llg_vpi_cb_t* first, cb_match_fn match,
                                  const void* context, llg_vpi_cb_t** inline_buffer,
                                  size_t* count) {
    llg_vpi_cb_t** items = inline_buffer;
    size_t capacity = LLG_VPI_DISPATCH_INLINE;
    *count = 0;
    for (llg_vpi_cb_t* cb = first; cb; cb = cb->next) {
        if (cb->removed || !match(cb, context)) continue;
        if (*count == capacity) {
            size_t grown_capacity = capacity * 2u;
            llg_vpi_cb_t** grown = (llg_vpi_cb_t**)malloc(grown_capacity * sizeof(*grown));
            if (!grown) {
                fputs("llg runtime fatal: VPI callback dispatch allocation failed\n", stderr);
                abort();
            }
            memcpy(grown, items, *count * sizeof(*grown));
            if (items != inline_buffer) free(items);
            items = grown;
            capacity = grown_capacity;
        }
        items[(*count)++] = cb;
    }
    /* Registration order. Lists are short: insertion sort. */
    for (size_t i = 1; i < *count; ++i) {
        llg_vpi_cb_t* cb = items[i];
        size_t j = i;
        for (; j > 0 && items[j - 1]->sequence > cb->sequence; --j) items[j] = items[j - 1];
        items[j] = cb;
    }
    return items;
}

static int matches_reason(const llg_vpi_cb_t* cb, const void* context) {
    return cb->data.reason == *(const int*)context;
}

/* cbEndOfCompile, cbStartOfSimulation and cbEndOfSimulation: each occurs
 * once, so its records retire after they ran. */
static void invoke_action(int reason) {
    if (!g_vpi.indexed) return;
    llg_vpi_cb_t* buffer[LLG_VPI_DISPATCH_INLINE];
    size_t count;
    dispatch_begin();
    llg_vpi_cb_t** items = collect_cbs(g_vpi.cbs, matches_reason, &reason, buffer, &count);
    for (size_t i = 0; i < count; ++i) {
        llg_vpi_cb_t* cb = items[i];
        if (cb->removed) continue;
        s_cb_data data;
        s_vpi_time time;
        base_data(cb, &data);
        data.time = callback_time(vpiSimTime, NULL,
                                  reason == cbEndOfSimulation ? llg_rt_end_time() : llg_time(),
                                  &time);
        invoke_cb(cb, &data);
        vpi_cb_fired(cb);
    }
    if (items != buffer) free(items);
    dispatch_end();
}

static void time_fire(void* context) {
    llg_vpi_cb_t* cb = (llg_vpi_cb_t*)context;
    cb->scheduled = 0;
    if (cb->removed) return;
    s_cb_data data;
    s_vpi_time time;
    base_data(cb, &data);
    data.time = callback_time(cb->time.type, cb->scope, llg_time(), &time);
    dispatch_begin();
    invoke_cb(cb, &data);
    /* Time callbacks are one-shot (38.36.2). */
    vpi_cb_fired(cb);
    dispatch_end();
}

static int matches_next_time(const llg_vpi_cb_t* cb, const void* context) {
    return cb->waiting && cb->registered_at < *(const uint64_t*)context;
}

/* Runtime time-slot hook, called at the start of every Pre-Active pass:
 * queue the cbNextSimTime records registered in an earlier slot. */
static void vpi_time_slot(void) {
    if (!g_vpi.next_time_pending) return;
    uint64_t now = llg_time();
    llg_vpi_cb_t* buffer[LLG_VPI_DISPATCH_INLINE];
    size_t count;
    llg_vpi_cb_t** items = collect_cbs(g_vpi.cbs, matches_next_time, &now, buffer, &count);
    for (size_t i = 0; i < count; ++i) {
        llg_vpi_cb_t* cb = items[i];
        if (llg_schedule_region_callback_id(LLG_REGION_PRE_ACTIVE_PLI, time_fire, cb, 0,
                                            &cb->id)) {
            cb->scheduled = 1;
            cb->waiting = 0;
            --g_vpi.next_time_pending;
        }
    }
    if (items != buffer) free(items);
}

/* Index reported for an array element: its index in the rightmost unpacked
 * range (38.36.1). */
static int32_t element_index(const llg_vpi_model_object_t* object, uint64_t element) {
    if (element == LLG_VPI_NO_ELEMENT || object->dim_count == 0) return 0;
    int64_t left = object->dims[2u * (object->dim_count - 1u)];
    int64_t right = object->dims[2u * (object->dim_count - 1u) + 1u];
    uint64_t size = (uint64_t)(left >= right ? left - right : right - left) + 1u;
    uint64_t offset = element % size;
    int64_t index = left >= right ? left - (int64_t)offset : left + (int64_t)offset;
    return (int32_t)index;
}

/* The live watches of `key`, in registration order of their records, like
 * collect_cbs. */
static llg_vpi_watch_t** collect_watches(const void* key, llg_vpi_watch_t** inline_buffer,
                                         size_t* count) {
    llg_vpi_watch_t** items = inline_buffer;
    size_t capacity = LLG_VPI_DISPATCH_INLINE;
    *count = 0;
    for (llg_vpi_watch_t* watch = g_vpi.watch_buckets[watch_bucket(key, g_vpi.watch_capacity)];
         watch; watch = watch->next) {
        if (watch->key != key || watch->cb->removed) continue;
        if (*count == capacity) {
            size_t grown_capacity = capacity * 2u;
            llg_vpi_watch_t** grown = (llg_vpi_watch_t**)malloc(grown_capacity * sizeof(*grown));
            if (!grown) {
                fputs("llg runtime fatal: VPI callback dispatch allocation failed\n", stderr);
                abort();
            }
            memcpy(grown, items, *count * sizeof(*grown));
            if (items != inline_buffer) free(items);
            items = grown;
            capacity = grown_capacity;
        }
        items[(*count)++] = watch;
    }
    for (size_t i = 1; i < *count; ++i) {
        llg_vpi_watch_t* watch = items[i];
        size_t j = i;
        for (; j > 0 && items[j - 1]->cb->sequence > watch->cb->sequence; --j)
            items[j] = items[j - 1];
        items[j] = watch;
    }
    return items;
}

/* Runtime value-watch hook: the storage at `key` published a new value. */
static void vpi_value_changed(const void* key) {
    if (!g_vpi.watch_count) return;
    llg_vpi_watch_t* buffer[LLG_VPI_DISPATCH_INLINE];
    size_t count;
    dispatch_begin();
    llg_vpi_watch_t** items = collect_watches(key, buffer, &count);
    /* The newest registration runs first (decision S43-D6). */
    for (size_t i = count; i-- > 0;) {
        llg_vpi_watch_t* watch = items[i];
        llg_vpi_cb_t* cb = watch->cb;
        if (cb->removed) continue;
        if (watch->event) {
            /* 38.36.1: an event has no value; the callback's value is NULL. */
            s_cb_data data;
            s_vpi_time time;
            base_data(cb, &data);
            data.time = callback_time(cb->time.type, cb->scope, llg_time(), &time);
            data.value = NULL;
            invoke_cb(cb, &data);
            continue;
        }
        llg_vpi_target_t target = cb->target;
        int32_t index = cb->index;
        if (watch->element != LLG_VPI_NO_ELEMENT) {
            /* A whole-array record reports the element that changed. */
            target.element = watch->element;
            index = element_index(row_object(target.object), watch->element);
        }
        llg_vpi_datum_t datum;
        if (!read_target(&target, &datum)) {
            datum_destroy(&datum);
            continue;
        }
        if (cb->target.span) {
            /* A select fires only when its own bits changed. */
            if (sv4_same(cb->last, datum.packed)) {
                datum_destroy(&datum);
                continue;
            }
            sv4_copy(&cb->last, &datum.packed);
        }
        s_cb_data data;
        s_vpi_time time;
        s_vpi_value value;
        base_data(cb, &data);
        data.index = index;
        data.time = callback_time(cb->time.type, cb->scope, llg_time(), &time);
        if (cb->value.format != vpiSuppressVal) {
            memset(&value, 0, sizeof(value));
            value.format = cb->value.format;
            fill_value(&datum, cb->value_signed, cb->value_type, &value);
            data.value = &value;
        }
        datum_destroy(&datum);
        invoke_cb(cb, &data);
    }
    if (items != buffer) free(items);
    dispatch_end();
}

/* Whether a force or release statement writes what a CB_FORCE record
 * watches: overlapping bits of the same object (or element), or an element
 * of a watched array. */
static int statement_writes(const llg_vpi_cb_t* cb, const llg_vpi_stmt_t* stmt) {
    if (!cb->force_filter) return 1;
    const llg_vpi_model_object_t* object = row_object(cb->target.object);
    uint32_t row;
    uint64_t element;
    if (stmt->real) {
        if (!storage_row(stmt->real, &row, &element) || row != cb->target.object) return 0;
        return cb->force_filter == 2 || element == cb->target.element;
    }
    int64_t low = cb->target.span ? (int64_t)cb->target.lsb : 0;
    int64_t high = low + (int64_t)(cb->target.span ? cb->target.span : object->width) - 1;
    for (int i = 0; i < stmt->n_parts; ++i) {
        const llg_force_part_t* part = &stmt->parts[i];
        if (!part_location(part, &row, &element) || row != cb->target.object) continue;
        if (cb->force_filter == 2) return 1;
        if (element == cb->target.element && part->right <= high && part->left >= low) return 1;
    }
    return 0;
}

static int matches_statement(const llg_vpi_cb_t* cb, const void* context) {
    const llg_vpi_stmt_t* stmt = (const llg_vpi_stmt_t*)context;
    return cb->kind == CB_FORCE && cb->data.reason == (stmt->release ? cbRelease : cbForce) &&
           statement_writes(cb, stmt);
}

/* The value of a statement's left-hand side after it took effect (38.36.1):
 * its parts' current bits placed at their offsets in the right-hand value. */
static void statement_value(const llg_vpi_stmt_t* stmt, llg_vpi_datum_t* out, int* type,
                            int* is_signed) {
    memset(out, 0, sizeof(*out));
    out->packed = VPI_EMPTY;
    *type = vpiReg;
    *is_signed = 0;
    uint32_t row;
    uint64_t element;
    if (stmt->real) {
        out->is_real = 1;
        out->real = *stmt->real;
        *type = vpiRealVar;
        return;
    }
    uint32_t width = 0;
    for (int i = 0; i < stmt->n_parts; ++i) {
        const llg_force_part_t* part = &stmt->parts[i];
        uint64_t top = (uint64_t)part->value_lsb + part->width;
        if (top > width) width = (uint32_t)top;
    }
    out->packed = sv4_zero(width ? width : 1u, 0);
    for (int i = 0; i < stmt->n_parts; ++i) {
        const llg_force_part_t* part = &stmt->parts[i];
        if (!part->width) continue;
        sv4_t bits = sv4_part_select(*part->target, part->left, part->right);
        sv4_part_select_set(&out->packed, (int64_t)part->value_lsb + part->width - 1,
                            (int64_t)part->value_lsb, bits);
        sv4_destroy(&bits);
    }
    /* A whole variable, net or element keeps its own type and sign. */
    uint32_t lsb, bits;
    if (statement_target(stmt, &row, &element, &lsb, &bits)) {
        const llg_vpi_model_object_t* object = row_object(row);
        if (lsb == 0 && bits == object->width && bits == width) {
            *type = value_type_of(object, element);
            *is_signed = object->is_signed;
            llg_sv4_set_signed(&out->packed, (int8_t)object->is_signed);
        }
    }
}

/* Runtime force-watch hook: a force or release statement took effect. The
 * matching records run in registration order with a vpiForce or vpiRelease
 * handle that is valid until the dispatch returns. */
static void vpi_force_event(int release, const llg_force_part_t* parts, int n_parts,
                            double* real) {
    if (!g_vpi.force_cbs || g_vpi.quiet_force || !g_vpi.indexed) return;
    llg_vpi_stmt_t stmt = {release, parts, n_parts, real};
    llg_vpi_cb_t* buffer[LLG_VPI_DISPATCH_INLINE];
    size_t count;
    llg_vpi_cb_t** items = collect_cbs(g_vpi.cbs, matches_statement, &stmt, buffer, &count);
    if (!count) {
        if (items != buffer) free(items);
        return;
    }
    dispatch_begin();
    llg_vpi_datum_t datum;
    int type, is_signed;
    statement_value(&stmt, &datum, &type, &is_signed);
    uint32_t index = alloc_slot(SLOT_STMT);
    uint32_t generation = 0;
    vpiHandle handle = NULL;
    if (index != LLG_VPI_NONE) {
        g_vpi.slots[index].aux = release ? vpiRelease : vpiForce;
        g_vpi.slots[index].pointer = &stmt;
        generation = g_vpi.slots[index].generation;
        handle = encode_handle(index);
    }
    for (size_t i = 0; i < count; ++i) {
        llg_vpi_cb_t* cb = items[i];
        if (cb->removed) continue;
        s_cb_data data;
        s_vpi_time time;
        s_vpi_value value;
        base_data(cb, &data);
        data.obj = handle;
        data.time = callback_time(cb->time.type, cb->scope, llg_time(), &time);
        if (cb->value.format != vpiSuppressVal) {
            memset(&value, 0, sizeof(value));
            value.format = cb->value.format;
            fill_value(&datum, is_signed, type, &value);
            data.value = &value;
        }
        invoke_cb(cb, &data);
    }
    /* The application may have released the handle (and the slot may have
     * been reused) while a callback ran. */
    if (index != LLG_VPI_NONE && g_vpi.slots[index].kind == SLOT_STMT &&
        g_vpi.slots[index].generation == generation)
        release_slot(index);
    datum_destroy(&datum);
    if (items != buffer) free(items);
    dispatch_end();
}

/* Bind a cbForce/cbRelease/cbAssign/cbDeassign record to its object: NULL
 * for every statement, a variable, net, element, part-select or member, or a
 * whole unpacked array (each element). 38.36.1: never a bit-select. */
static int bind_force_cb(llg_vpi_cb_t* cb, const s_cb_data* data) {
    if (data->value && data->value->format != vpiSuppressVal &&
        (data->value->format < vpiBinStrVal || data->value->format > vpiObjTypeVal)) {
        vpi_error("LLG_VPI_CALLBACK", "unsupported callback value format");
        return 0;
    }
    if (!storage_index()) return 0;
    if (!data->obj) return 1;
    llg_vpi_slot_t* slot = checked_handle(data->obj, NULL);
    if (!slot) return 0;
    int type = handle_type(slot);
    if (type == vpiNetBit || type == vpiRegBit) {
        vpi_error("LLG_VPI_CALLBACK",
                  "a force, release, assign or deassign callback cannot be placed on a bit-select");
        return 0;
    }
    llg_vpi_target_t target;
    if (handle_target(slot, &target)) {
        llg_vpi_model_object_t* object = row_object(target.object);
        if (is_parameter_type(object->type) || !has_storage(object)) {
            vpi_error("LLG_VPI_CALLBACK", "a parameter or constant is never forced");
            return 0;
        }
        cb->force_filter = 1;
        cb->target = target;
        cb->scope = object;
        return 1;
    }
    llg_vpi_model_object_t* array = slot->kind == SLOT_OBJECT ? row_object(slot->object) : NULL;
    if (!array || !is_array_type(array->type) || !has_storage(array)) {
        vpi_error("LLG_VPI_CALLBACK",
                  "a force, release, assign or deassign callback needs a variable, net, array, "
                  "element, part-select or member handle, or NULL");
        return 0;
    }
    cb->force_filter = 2;
    cb->target.object = slot->object;
    cb->target.element = LLG_VPI_NO_ELEMENT;
    cb->scope = array;
    return 1;
}

/* ── cbDisable (38.36.1) ────────────────────────────────────────────────── */

static int row_is_activation(const llg_vpi_model_object_t* object, const uint32_t* identity) {
    return (object->flags & LLG_VPI_OBJ_ACTIVATION) &&
           object->activation_declaration == identity[0] &&
           object->activation_instance == identity[1];
}

/* A record on a named block, task or function runs when `disable` names it;
 * one on a system task or function call runs when a named block that
 * encloses the call (its vpiScope or an outer scope) is disabled. */
static int matches_disable(const llg_vpi_cb_t* cb, const void* context) {
    const uint32_t* identity = (const uint32_t*)context;
    if (cb->kind != CB_DISABLE) return 0;
    if (!cb->site) return row_is_activation(row_object(cb->target.object), identity);
    for (const llg_vpi_model_object_t* scope = cb->site->scope; scope; scope = scope->parent)
        if (row_is_activation(scope, identity)) return 1;
    return 0;
}

/* Runtime disable-watch hook: `disable` ended the activations of one
 * declaration in one instance. */
static void vpi_disabled(uint32_t declaration, uint32_t instance) {
    if (!g_vpi.disable_cbs || !g_vpi.indexed) return;
    uint32_t identity[2] = {declaration, instance};
    llg_vpi_cb_t* buffer[LLG_VPI_DISPATCH_INLINE];
    size_t count;
    dispatch_begin();
    llg_vpi_cb_t** items = collect_cbs(g_vpi.cbs, matches_disable, identity, buffer, &count);
    for (size_t i = 0; i < count; ++i) {
        llg_vpi_cb_t* cb = items[i];
        if (cb->removed) continue;
        s_cb_data data;
        s_vpi_time time;
        base_data(cb, &data);
        data.time = callback_time(cb->time.type, cb->scope, llg_time(), &time);
        invoke_cb(cb, &data);
    }
    if (items != buffer) free(items);
    dispatch_end();
}

/* 38.36.1: obj is a system task or function call, named begin, named fork,
 * task or function. */
static int bind_disable_cb(llg_vpi_cb_t* cb, const s_cb_data* data) {
    llg_vpi_slot_t* slot = data->obj ? checked_handle(data->obj, NULL) : NULL;
    if (!data->obj) {
        vpi_error("LLG_VPI_CALLBACK", "cbDisable needs an object");
        return 0;
    }
    if (!slot) return 0;
    if (slot->kind == SLOT_CALL) {
        llg_vpi_callsite_t* site = (llg_vpi_callsite_t*)slot->pointer;
        /* A call without a generated site (legacy entry points) has no
         * scope and does not outlive the call. */
        if (site->id == UINT64_MAX) {
            vpi_error("LLG_VPI_CALLBACK", "this system task or function call has no source scope");
            return 0;
        }
        cb->site = site;
        cb->scope = site->scope;
        return 1;
    }
    llg_vpi_model_object_t* object = slot->kind == SLOT_OBJECT ? row_object(slot->object) : NULL;
    if (!object || !(object->type == vpiNamedBegin || object->type == vpiNamedFork ||
                     object->type == vpiTask || object->type == vpiFunction)) {
        vpi_error("LLG_VPI_CALLBACK",
                  "cbDisable needs a system task or function call, named begin, named fork, "
                  "task or function");
        return 0;
    }
    cb->target.object = slot->object;
    cb->target.element = LLG_VPI_NO_ELEMENT;
    cb->scope = object;
    return 1;
}

/* A cbTchkViolation record with a NULL obj runs for every timing check, one
 * on a timing check only for its own violations. */
static int matches_tchk(const llg_vpi_cb_t* cb, const void* context) {
    uint32_t row = *(const uint32_t*)context;
    return cb->kind == CB_TCHK && (cb->target.object == LLG_VPI_NONE || cb->target.object == row);
}

/* Runtime violation hook: row `check` of `set` reported a violation
 * (38.36.1: obj is the timing check). It runs during the timing step, after
 * the report and before the step toggles the notifiers. */
static void vpi_tchk_violation(llg_timing_check_set_t* set, const llg_timing_check_t* check) {
    (void)set;
    if (!g_vpi.tchk_cbs || !g_vpi.indexed) return;
    uint32_t row = tchk_row_of(check);
    if (row == LLG_VPI_NONE) return;
    llg_vpi_cb_t* buffer[LLG_VPI_DISPATCH_INLINE];
    size_t count;
    dispatch_begin();
    llg_vpi_cb_t** items = collect_cbs(g_vpi.cbs, matches_tchk, &row, buffer, &count);
    for (size_t i = 0; i < count; ++i) {
        llg_vpi_cb_t* cb = items[i];
        if (cb->removed) continue;
        s_cb_data data;
        s_vpi_time time;
        base_data(cb, &data);
        data.obj = object_handle(row);
        data.time = callback_time(cb->time.type, cb->scope ? cb->scope : row_object(row),
                                  llg_time(), &time);
        invoke_cb(cb, &data);
    }
    if (items != buffer) free(items);
    dispatch_end();
}

/* 38.36.1: obj is NULL (every timing check) or a timing check. */
static int bind_tchk_cb(llg_vpi_cb_t* cb, const s_cb_data* data) {
    cb->target.object = LLG_VPI_NONE;
    cb->target.element = LLG_VPI_NO_ELEMENT;
    if (!data->obj) return 1;
    llg_vpi_slot_t* slot = checked_handle(data->obj, NULL);
    if (!slot) return 0;
    llg_vpi_model_object_t* object = slot->kind == SLOT_OBJECT ? row_object(slot->object) : NULL;
    if (!object || object->type != vpiTchk) {
        vpi_error("LLG_VPI_CALLBACK", "cbTchkViolation needs a NULL obj or a timing check");
        return 0;
    }
    cb->target.object = slot->object;
    cb->scope = object;
    return 1;
}

/* ── assertion callbacks (39.4) ─────────────────────────────────────────── */

/* The attempt reasons a record watches its item for, and the control
 * reasons vpi_control raises (39.4.2). */
static int assertion_attempt_reason(int reason) {
    return reason == cbAssertionStart || reason == cbAssertionSuccess ||
           reason == cbAssertionVacuousSuccess || reason == cbAssertionFailure ||
           reason == cbAssertionDisabledEvaluation || reason == cbAssertionKill;
}

static int assertion_control_reason(int reason) {
    return reason == cbAssertionDisable || reason == cbAssertionEnable ||
           reason == cbAssertionReset || reason == cbAssertionDisablePassAction ||
           reason == cbAssertionEnablePassAction || reason == cbAssertionDisableFailAction ||
           reason == cbAssertionEnableFailAction || reason == cbAssertionDisableVacuousAction ||
           reason == cbAssertionEnableNonvacuousAction;
}

static int assertion_system_reason(int reason) {
    return reason == cbAssertionSysInitialized || reason == cbAssertionSysOn ||
           reason == cbAssertionSysOff || reason == cbAssertionSysKill ||
           reason == cbAssertionSysEnd || reason == cbAssertionSysReset ||
           reason == cbAssertionSysEnablePassAction || reason == cbAssertionSysEnableFailAction ||
           reason == cbAssertionSysDisablePassAction || reason == cbAssertionSysDisableFailAction ||
           reason == cbAssertionSysEnableNonvacuousAction ||
           reason == cbAssertionSysDisableVacuousAction;
}

/* The assertion handle a record passes to its routine. The record owns one
 * handle; when the application released it, a new one replaces it. */
static vpiHandle assertion_cb_handle(llg_vpi_cb_t* cb) {
    llg_vpi_slot_t* slot = decode_handle(cb->data.obj, NULL);
    if (cb->instance_code) {
        if (slot && slot->kind == SLOT_CODE && slot->aux == (int32_t)(cb->instance_code - 1u))
            return cb->data.obj;
        cb->data.obj = code_handle(cb->instance_code - 1u);
        return cb->data.obj;
    }
    if (slot && slot->kind == SLOT_ASSERTION && slot->left == (int64_t)cb->assertion_item)
        return cb->data.obj;
    cb->data.obj = assertion_handle(cb->assertion_item);
    return cb->data.obj;
}

/* The failExpr of the record's item (decision A17-D4). The record owns one
 * handle; when the application released it, a new one replaces it. An
 * expression that is a catalog object has that object's handle. */
static vpiHandle assertion_cb_fail_expr(llg_vpi_cb_t* cb) {
    uint32_t code = assertion_fail_code(llg_acov_item_at(cb->assertion_item));
    if (code == LLG_VPI_NONE) return NULL;
    llg_vpi_slot_t* slot = decode_handle(cb->fail_expr, NULL);
    if (slot && slot->kind == SLOT_CODE && slot->aux == (int32_t)code) return cb->fail_expr;
    cb->fail_expr = code_handle(code);
    return cb->fail_expr;
}

static void assertion_cb_release(llg_vpi_cb_t* cb) {
    --g_vpi.assertion_cbs;
    if (cb->assertion_attempt) llg_acov_watch(cb->assertion_item, -1);
    uint32_t index;
    llg_vpi_slot_t* slot = decode_handle(cb->data.obj, &index);
    if (slot && ((slot->kind == SLOT_ASSERTION && slot->left == (int64_t)cb->assertion_item) ||
                 (cb->instance_code && slot->kind == SLOT_CODE &&
                  slot->aux == (int32_t)(cb->instance_code - 1u))))
        release_slot(index);
    cb->data.obj = NULL;
    slot = decode_handle(cb->fail_expr, &index);
    if (slot && slot->kind == SLOT_CODE) release_slot(index);
    cb->fail_expr = NULL;
}

typedef struct {
    uint32_t item;
    int reason;
} llg_vpi_assertion_match_t;

/* A record on an instance sees a vacuous success of its assertion as a
 * success: the property instance reached a success state (decision
 * A17-D5). */
static int matches_assertion(const llg_vpi_cb_t* cb, const void* context) {
    const llg_vpi_assertion_match_t* match = (const llg_vpi_assertion_match_t*)context;
    if (cb->kind != CB_ASSERTION || cb->assertion_item != match->item) return 0;
    if (cb->data.reason == match->reason) {
        /* A cover statement reports no failure: a cover attempt without a
         * match runs no callback on the statement (decision A17-D1), while
         * the sequence instance of a cover property fails (A17-D5). */
        if (match->reason == cbAssertionFailure && !cb->instance_code) {
            const llg_acov_item_t* item = llg_acov_item_at(match->item);
            if (item && item->kind == LLG_ASSERTION_COVER) return 0;
        }
        return 1;
    }
    return cb->instance_code && cb->data.reason == cbAssertionSuccess &&
           match->reason == cbAssertionVacuousSuccess;
}

/* Run the records of `reason` on `item`. `start` is the attempt's start
 * time, or NULL for the reasons without attempt information (39.4.2: kill
 * and the control reasons pass a NULL info pointer). Each routine gets its
 * own copies of the time and attempt information. The routine's return
 * value is not specified by 39.4.2 and is ignored. */
static void invoke_assertion_cbs(uint32_t item, int reason, uint64_t cb_time,
                                 const uint64_t* start) {
    if (!g_vpi.assertion_cbs || !g_vpi.indexed) return;
    llg_vpi_assertion_match_t match = {item, reason};
    llg_vpi_cb_t* buffer[LLG_VPI_DISPATCH_INLINE];
    size_t count;
    dispatch_begin();
    llg_vpi_cb_t** items = collect_cbs(g_vpi.cbs, matches_assertion, &match, buffer, &count);
    for (size_t i = 0; i < count; ++i) {
        llg_vpi_cb_t* cb = items[i];
        if (cb->removed) continue;
        vpiHandle assertion = assertion_cb_handle(cb);
        if (!assertion) continue;
        s_vpi_time time;
        (void)callback_time(vpiSimTime, NULL, cb_time, &time);
        s_vpi_attempt_info info;
        memset(&info, 0, sizeof(info));
        if (start) (void)callback_time(vpiSimTime, NULL, *start, &info.attemptStartTime);
        if (reason == cbAssertionFailure) info.detail.failExpr = assertion_cb_fail_expr(cb);
        (void)cb->assertion_rtn(cb->data.reason, &time, assertion, start ? &info : NULL,
                                cb->data.user_data);
    }
    if (items != buffer) free(items);
    dispatch_end();
}

/* Runtime assertion hook: one attempt event of a watched item, or the
 * effect of a control task on it. */
static void vpi_assertion_event(const llg_acov_item_t* item, int event, uint64_t cb_time,
                                uint64_t start) {
    const llg_acov_item_t* first = llg_acov_item_at(0);
    if (!first || item < first || g_vpi.assertions_ended) return;
    static const int reasons[] = {cbAssertionStart,   cbAssertionSuccess,
                                  cbAssertionVacuousSuccess, cbAssertionFailure,
                                  cbAssertionDisabledEvaluation, cbAssertionKill,
                                  cbAssertionDisable, cbAssertionEnable};
    if (event < 0 || (size_t)event >= sizeof(reasons) / sizeof(reasons[0])) return;
    int reason = reasons[event];
    /* 39.4.2: kill and the control reasons pass a NULL info pointer. */
    int no_info = reason == cbAssertionKill || reason == cbAssertionDisable ||
                  reason == cbAssertionEnable;
    invoke_assertion_cbs((uint32_t)(item - first), reason, cb_time, no_info ? NULL : &start);
}

static int matches_assertion_system(const llg_vpi_cb_t* cb, const void* context) {
    return cb->kind == CB_ASSERT_SYS && cb->data.reason == *(const int*)context;
}

/* The cbAssertionSys* records of `reason`. cbAssertionSysInitialized and
 * cbAssertionSysEnd occur once, so their records retire after they ran. */
static void invoke_assertion_system(int reason) {
    if (!g_vpi.indexed) return;
    llg_vpi_cb_t* buffer[LLG_VPI_DISPATCH_INLINE];
    size_t count;
    dispatch_begin();
    llg_vpi_cb_t** items = collect_cbs(g_vpi.cbs, matches_assertion_system, &reason, buffer, &count);
    int one_shot = reason == cbAssertionSysInitialized || reason == cbAssertionSysEnd;
    for (size_t i = 0; i < count; ++i) {
        llg_vpi_cb_t* cb = items[i];
        if (cb->removed) continue;
        s_cb_data data;
        s_vpi_time time;
        base_data(cb, &data);
        data.time = callback_time(cb->time.type, NULL, llg_time(), &time);
        invoke_cb(cb, &data);
        if (one_shot) vpi_cb_fired(cb);
    }
    if (items != buffer) free(items);
    dispatch_end();
}

/* 39.5.1 vpiAssertionSysEnd and the end of the run: no assertion callback
 * occurs any more, so every record goes. A record that ran keeps its
 * handle as a fired copy (S43-D4); the others become stale. */
static void assertion_callbacks_end(void) {
    if (g_vpi.assertions_ended) return;
    g_vpi.assertions_ended = 1;
    invoke_assertion_system(cbAssertionSysEnd);
    dispatch_begin();
    for (llg_vpi_cb_t* cb = g_vpi.cbs; cb;) {
        llg_vpi_cb_t* next = cb->next;
        if (!cb->removed && (cb->kind == CB_ASSERTION || cb->kind == CB_ASSERT_SYS))
            vpi_cb_retire(cb);
        cb = next;
    }
    dispatch_end();
    llg_assertion_watch_hook(NULL);
}

/* Region, delay rules and recurrence of each time reason (Table 4-1,
 * 38.36.2). Returns 0 with the error recorded when it cannot be placed. */
static int schedule_time_cb(llg_vpi_cb_t* cb, uint64_t ticks) {
    llg_region_t region;
    switch (cb->data.reason) {
        case cbAtStartOfSimTime:
        case cbAfterDelay: region = LLG_REGION_PRE_ACTIVE_PLI; break;
        /* cbReadWriteSynch runs Pre-NBA, once the Active and Inactive work
         * is done; what it wakes runs before cbNBASynch, which runs last,
         * immediately before the NBA region (decision S43-D1). */
        case cbReadWriteSynch: region = LLG_REGION_PRE_NBA_PLI; break;
        case cbNBASynch: region = LLG_REGION_PRE_NBA; break;
        case cbAtEndOfSimTime: region = LLG_REGION_PRE_POSTPONED_PLI; break;
        default: region = LLG_REGION_POSTPONED_PLI; break; /* cbReadOnlySynch */
    }
    if (ticks == 0 && g_vpi.phase == LLG_VPI_PHASE_RUNNING && llg_rt_running()) {
        llg_region_t current = llg_current_region();
        if (llg_region_is_read_only()) {
            vpi_error("LLG_VPI_CALLBACK",
                      "a zero-delay time callback cannot be placed from a read-only region");
            return 0;
        }
        if (region == LLG_REGION_PRE_ACTIVE_PLI && current != LLG_REGION_PRE_ACTIVE_PLI) {
            if (cb->data.reason != cbAfterDelay) {
                vpi_error("LLG_VPI_CALLBACK",
                          "a zero-delay start-of-time callback needs the slot's Pre-Active region");
                return 0;
            }
            /* Past Pre-Active, a zero delay runs like a #0 wait: in the
             * Inactive region of this slot (decision S43-D2). */
            region = LLG_REGION_INACTIVE;
        }
        if (region == LLG_REGION_PRE_POSTPONED_PLI && current > region) {
            vpi_error("LLG_VPI_CALLBACK", "this time slot already left its Pre-Postponed region");
            return 0;
        }
    }
    if (!llg_schedule_region_callback_id(region, time_fire, cb, ticks, &cb->id)) {
        vpi_error("LLG_VPI_SCHEDULE", "the scheduler rejected the VPI time callback");
        return 0;
    }
    cb->scheduled = 1;
    return 1;
}

/* Storage address of a value-change target. */
static const void* watch_key(const llg_vpi_target_t* target) {
    llg_vpi_model_object_t* object = row_object(target->object);
    return object->is_real ? (const void*)real_cell(target) : (const void*)packed_cell(target);
}

/* Bind a cbValueChange record to its storage. A whole unpacked array
 * watches each element and reports the one that changed (decision S43-D7). */
static int bind_value_cb(llg_vpi_cb_t* cb, const s_cb_data* data) {
    llg_vpi_slot_t* slot = checked_handle(data->obj, NULL);
    if (!slot) return 0;
    int dynamic = dyn_bind_value_cb(cb, slot);
    if (dynamic >= 0) return dynamic;
    if (slot->kind == SLOT_OBJECT && g_vpi.objects[slot->object].type == vpiNamedEvent) {
        /* 38.36.1: the callback runs after each trigger of the event's
         * synchronization object (the one it names at registration). */
        llg_vpi_model_object_t* object = &g_vpi.objects[slot->object];
        if (!object->event->object) {
            vpi_error("LLG_VPI_CALLBACK", "this event names no synchronization object");
            return 0;
        }
        llg_vpi_watch_t* watch = (llg_vpi_watch_t*)calloc(1, sizeof(*watch));
        if (!watch) {
            vpi_error("LLG_VPI_NOMEM", "VPI value-change watch allocation failed");
            return 0;
        }
        watch->key = object->event->object;
        watch->cb = cb;
        watch->element = LLG_VPI_NO_ELEMENT;
        watch->event = LLG_VPI_WATCH_EVENT;
        cb->watches = watch;
        cb->watch_total = 1;
        cb->target.object = slot->object;
        cb->target.element = LLG_VPI_NO_ELEMENT;
        cb->scope = object;
        cb->value_type = vpiNamedEvent;
        return 1;
    }
    llg_vpi_target_t target;
    int whole_array = 0;
    if (!handle_target(slot, &target)) {
        llg_vpi_model_object_t* array =
            slot->kind == SLOT_OBJECT ? row_object(slot->object) : NULL;
        if (!array || !is_array_type(array->type) || !has_storage(array)) {
            vpi_error("LLG_VPI_CALLBACK",
                      "cbValueChange needs a variable, net, array, element or select handle");
            return 0;
        }
        whole_array = 1;
    }
    llg_vpi_model_object_t* object = row_object(target.object);
    if (is_parameter_type(object->type)) {
        vpi_error("LLG_VPI_CALLBACK", "a parameter never changes value");
        return 0;
    }
    if (data->value && data->value->format != vpiSuppressVal &&
        (data->value->format < vpiBinStrVal || data->value->format > vpiObjTypeVal)) {
        vpi_error("LLG_VPI_CALLBACK", "unsupported cbValueChange value format");
        return 0;
    }
    uint64_t total = whole_array ? g_vpi.element_counts[target.object] : 1u;
    if (total == 0 || total > LLG_VPI_MAX_ARRAY_WATCH_ELEMENTS) {
        vpi_error("LLG_VPI_CALLBACK",
                  "this array has too many elements for one cbValueChange; watch its elements");
        return 0;
    }
    llg_vpi_watch_t* watches = (llg_vpi_watch_t*)calloc((size_t)total, sizeof(*watches));
    if (!watches) {
        vpi_error("LLG_VPI_NOMEM", "VPI value-change watch allocation failed");
        return 0;
    }
    for (uint64_t i = 0; i < total; ++i) {
        llg_vpi_target_t cell = target;
        if (whole_array) cell.element = i;
        watches[i].key = watch_key(&cell);
        watches[i].cb = cb;
        watches[i].element = whole_array ? i : LLG_VPI_NO_ELEMENT;
        if (!watches[i].key) {
            free(watches);
            vpi_error("LLG_VPI_CALLBACK", "this VPI object has no watchable storage");
            return 0;
        }
    }
    cb->watches = watches;
    cb->watch_total = total;
    cb->target = target;
    cb->scope = object;
    cb->value_type = whole_array ? value_type_of(object, 0) : handle_type(slot);
    cb->value_signed = target.is_signed;
    cb->index = slot->kind == SLOT_ELEMENT ? element_index(object, target.element) : 0;
    if (target.span) {
        llg_vpi_datum_t datum;
        if (read_target(&target, &datum)) sv4_move(&cb->last, &datum.packed);
        datum_destroy(&datum);
    }
    return 1;
}

static int time_reason(int reason) {
    return reason == cbAtStartOfSimTime || reason == cbReadWriteSynch ||
           reason == cbReadOnlySynch || reason == cbAfterDelay || reason == cbNBASynch ||
           reason == cbAtEndOfSimTime;
}

vpiHandle vpi_register_cb(p_cb_data data) {
    vpi_enter();
    if (!data || !data->cb_rtn) {
        vpi_set_error(vpi_state(), vpiError, "LLG_VPI_CALLBACK",
                      "unsupported or invalid VPI callback registration");
        return NULL;
    }
    llg_vpi_cb_kind_t kind;
    int reason = data->reason;
    if (reason == cbEndOfCompile || reason == cbStartOfSimulation || reason == cbEndOfSimulation) {
        kind = CB_ACTION;
        int late = reason == cbEndOfCompile ? g_vpi.phase >= LLG_VPI_PHASE_COMPILED
                   : reason == cbStartOfSimulation ? g_vpi.phase >= LLG_VPI_PHASE_RUNNING
                                                    : g_vpi.phase >= LLG_VPI_PHASE_ENDED;
        if (late) {
            /* Accepted, but it never runs: the handle names a fired copy
             * (decision S43-D4). */
            if (!vpi_ready()) return NULL;
            llg_vpi_cb_done_t* done = cb_done_new(data);
            if (!done) {
                vpi_error("LLG_VPI_NOMEM", "VPI callback allocation failed");
                return NULL;
            }
            uint32_t index = alloc_slot(SLOT_FIRED_CB);
            if (index == LLG_VPI_NONE) {
                free(done);
                return NULL;
            }
            fired_attach(index, done);
            return encode_handle(index);
        }
    } else if (reason == cbValueChange) {
        kind = CB_VALUE;
    } else if (reason == cbForce || reason == cbRelease || reason == cbAssign ||
               reason == cbDeassign) {
        /* cbAssign/cbDeassign are accepted but never run: llg rejects
         * procedural assign and deassign statements (decision A20-D9). */
        kind = CB_FORCE;
    } else if (reason == cbDisable) {
        kind = CB_DISABLE;
    } else if (assertion_system_reason(reason)) {
        kind = CB_ASSERT_SYS;
        if (reason == cbAssertionSysInitialized && g_vpi.assertion_sys_started) {
            /* It already occurred at time zero: accepted, never runs (as
             * S43-D4). */
            if (!vpi_ready()) return NULL;
            llg_vpi_cb_done_t* done = cb_done_new(data);
            if (!done) {
                vpi_error("LLG_VPI_NOMEM", "VPI callback allocation failed");
                return NULL;
            }
            uint32_t index = alloc_slot(SLOT_FIRED_CB);
            if (index == LLG_VPI_NONE) {
                free(done);
                return NULL;
            }
            fired_attach(index, done);
            return encode_handle(index);
        }
        if (g_vpi.assertions_ended || g_vpi.phase >= LLG_VPI_PHASE_ENDED) {
            vpi_error("LLG_VPI_CALLBACK",
                      "the assertion system has ended; no assertion system callback can occur");
            return NULL;
        }
    } else if (reason == cbTchkViolation) {
        kind = CB_TCHK;
    } else if (reason == cbCreateObj || reason == cbReclaimObj || reason == cbEndOfObject) {
        kind = CB_OBJECT;
    } else if (rts_cb_reason(reason)) {
        kind = CB_RUNTIME;
    } else if (reason == cbNextSimTime || time_reason(reason)) {
        kind = reason == cbNextSimTime ? CB_NEXT_TIME : CB_TIME;
        if (g_vpi.phase >= LLG_VPI_PHASE_ENDED) {
            vpi_error("LLG_VPI_CALLBACK", "the simulation has ended; no time callback can occur");
            return NULL;
        }
    } else {
        vpi_set_error(vpi_state(), vpiError, "LLG_VPI_CALLBACK",
                      "unsupported or invalid VPI callback registration");
        return NULL;
    }
    if ((kind == CB_VALUE || kind == CB_FORCE || kind == CB_DISABLE || kind == CB_TCHK ||
         kind == CB_OBJECT || kind == CB_RUNTIME) &&
        g_vpi.phase >= LLG_VPI_PHASE_ENDED) {
        vpi_error("LLG_VPI_CALLBACK", "the simulation has ended; no value can change");
        return NULL;
    }
    uint64_t ticks = 0;
    const llg_vpi_model_object_t* scope = NULL;
    if (kind == CB_TIME) {
        if (!data->time ||
            (data->time->type != vpiSimTime && data->time->type != vpiScaledRealTime)) {
            vpi_error("LLG_VPI_TIME", "a time callback needs a vpiSimTime or vpiScaledRealTime time");
            return NULL;
        }
        if (data->obj) {
            llg_vpi_slot_t* slot = checked_handle(data->obj, NULL);
            if (!slot) return NULL;
            scope = row_object(slot->object);
        }
        int ok;
        ticks = ticks_from_time(data->time, scope, &ok);
        if (!ok || ticks > UINT64_MAX - llg_time()) {
            vpi_error("LLG_VPI_TIME", "unsupported or negative callback delay");
            return NULL;
        }
    }
    if (!vpi_ready()) return NULL;
    llg_vpi_cb_t* cb = (llg_vpi_cb_t*)calloc(1, sizeof(*cb));
    if (!cb) {
        vpi_error("LLG_VPI_NOMEM", "VPI callback allocation failed");
        return NULL;
    }
    cb->kind = kind;
    cb->data = *data;
    cb->last = VPI_EMPTY;
    cb->scope = scope;
    cb->time.type = vpiSuppressTime;
    if (data->time) cb->time = *data->time;
    cb->value.format = data->value ? data->value->format : vpiSuppressVal;
    if (kind == CB_VALUE && !bind_value_cb(cb, data)) {
        vpi_cb_free(cb);
        return NULL;
    }
    if (kind == CB_FORCE && !bind_force_cb(cb, data)) {
        vpi_cb_free(cb);
        return NULL;
    }
    if (kind == CB_DISABLE && !bind_disable_cb(cb, data)) {
        vpi_cb_free(cb);
        return NULL;
    }
    if (kind == CB_TCHK && !bind_tchk_cb(cb, data)) {
        vpi_cb_free(cb);
        return NULL;
    }
    if (kind == CB_OBJECT && !dyn_bind_object_cb(cb, data)) {
        vpi_cb_free(cb);
        return NULL;
    }
    if (kind == CB_RUNTIME && !rts_bind_cb(cb, data)) {
        vpi_cb_free(cb);
        return NULL;
    }
    cb->data.time = data->time ? &cb->time : NULL;
    cb->data.value = data->value ? &cb->value : NULL;
    uint32_t index = alloc_slot(SLOT_CALLBACK);
    if (index == LLG_VPI_NONE) {
        vpi_cb_free(cb);
        return NULL;
    }
    if (kind == CB_VALUE && !cb_watches_insert(cb)) {
        release_slot(index);
        vpi_cb_free(cb);
        vpi_error("LLG_VPI_NOMEM", "VPI value-change watch allocation failed");
        return NULL;
    }
    g_vpi.slots[index].pointer = cb;
    cb->slot = index;
    cb->sequence = g_vpi.cb_sequence++;
    cb->registered_at = llg_time();
    if (!g_vpi.cbs_tail) g_vpi.cbs_tail = &g_vpi.cbs;
    cb->prev_link = g_vpi.cbs_tail;
    *g_vpi.cbs_tail = cb;
    g_vpi.cbs_tail = &cb->next;
    if (kind == CB_VALUE) llg_value_watch_hook(vpi_value_changed);
    if (kind == CB_FORCE) {
        ++g_vpi.force_cbs;
        llg_force_watch_hook(vpi_force_event);
    }
    if (kind == CB_DISABLE) {
        ++g_vpi.disable_cbs;
        llg_disable_watch_hook(vpi_disabled);
    }
    if (kind == CB_TCHK) {
        ++g_vpi.tchk_cbs;
        llg_timing_violation_hook(vpi_tchk_violation);
    }
    if (kind == CB_OBJECT) dyn_object_cb_added();
    if (kind == CB_RUNTIME) rts_cb_added(cb);
    if (kind == CB_NEXT_TIME) {
        cb->waiting = 1;
        ++g_vpi.next_time_pending;
        llg_time_slot_hook(vpi_time_slot);
    }
    if (kind == CB_TIME && !schedule_time_cb(cb, ticks)) {
        vpi_cb_retire(cb);
        return NULL;
    }
    return encode_handle(index);
}

static llg_vpi_cb_t* callback_record(vpiHandle handle, const char* routine) {
    llg_vpi_slot_t* slot = checked_handle(handle, NULL);
    if (!slot) return NULL;
    if (slot->kind != SLOT_CALLBACK) {
        vpi_set_errorf(vpi_state(), vpiError, "LLG_VPI_HANDLE", "%s needs a callback handle",
                       routine);
        return NULL;
    }
    return (llg_vpi_cb_t*)slot->pointer;
}

PLI_INT32 vpi_remove_cb(vpiHandle handle) {
    vpi_enter();
    uint32_t index;
    llg_vpi_slot_t* slot = checked_handle(handle, &index);
    if (!slot) return 0;
    if (slot->kind == SLOT_FIRED_CB) {
        fired_free(index);
        return 1;
    }
    llg_vpi_cb_t* cb = callback_record(handle, "vpi_remove_cb");
    if (!cb) return 0;
    vpi_cb_retire(cb);
    return 1;
}

void vpi_get_cb_info(vpiHandle handle, p_cb_data output) {
    vpi_enter();
    llg_vpi_slot_t* slot = checked_handle(handle, NULL);
    if (!slot) return;
    const s_cb_data* data = NULL;
    const llg_vpi_cb_t* cb = NULL;
    if (slot->kind == SLOT_FIRED_CB) {
        data = &((const llg_vpi_cb_done_t*)slot->pointer)->data;
    } else {
        cb = callback_record(handle, "vpi_get_cb_info");
        if (!cb) return;
        data = &cb->data;
    }
    if (!output) {
        vpi_error("LLG_VPI_ARGUMENT", "null VPI callback info output");
        return;
    }
    *output = *data;
    /* An assertion callback reports the routine it was placed with; that
     * routine has the 39.4.2 signature, so an application casts it back
     * before calling it. The cast goes through void (*)(void), the generic
     * function pointer type compilers accept without a type warning. */
    if (cb && cb->kind == CB_ASSERTION)
        output->cb_rtn = (PLI_INT32(*)(struct t_cb_data*))(void (*)(void))cb->assertion_rtn;
}

void llg_vpi_start_simulation(void) {
    if (!g_vpi.initialized || g_vpi.failed || g_vpi.phase >= LLG_VPI_PHASE_RUNNING) return;
    g_vpi.running = 1;
    g_vpi.starting = 1;
    g_vpi.phase = LLG_VPI_PHASE_COMPILED;
    invoke_action(cbEndOfCompile);
    g_vpi.phase = LLG_VPI_PHASE_RUNNING;
    invoke_action(cbStartOfSimulation);
    g_vpi.starting = 0;
}

/* The assertion system initializes and starts at time zero once the
 * initial and always procedures have taken their first turn (decision
 * A17-D3): a cbAssertionSysInitialized or cbAssertionSysOn placed by
 * vlog_startup_routines, a start callback or one of those turns still
 * runs, one placed later (a #0 turn, an always_comb, a woken process) has
 * missed SysInitialized. */
static void assertion_system_start(void* data) {
    (void)data;
    if (g_vpi.assertion_sys_started || !g_vpi.initialized || g_vpi.failed ||
        g_vpi.phase != LLG_VPI_PHASE_RUNNING)
        return;
    g_vpi.assertion_sys_started = 1;
    invoke_assertion_system(cbAssertionSysInitialized);
    invoke_assertion_system(cbAssertionSysOn);
}

void llg_vpi_procedures_started(void) {
    if (!g_vpi.initialized || g_vpi.failed || g_vpi.phase != LLG_VPI_PHASE_RUNNING ||
        g_vpi.assertion_sys_started || g_vpi.assertion_sys_scheduled)
        return;
    /* Only an application can place a system callback: a startup routine
     * that registered nothing leaves the run's scheduler passes as they
     * are. */
    if (!g_vpi.cbs && g_vpi.registration_count == 0) return;
    /* An Active event of time zero after the procedures queued so far. */
    if (llg_schedule_delayed_call("assertion system start", assertion_system_start, NULL, 0,
                                  NULL))
        g_vpi.assertion_sys_scheduled = 1;
}

void llg_vpi_end_simulation(void) {
    /* Once per model run, whichever of advance and close gets here first. */
    if (!g_vpi.initialized || g_vpi.phase != LLG_VPI_PHASE_RUNNING) return;
    /* All assertions have completed (39.4.1 cbAssertionSysEnd "typically
     * occurs after the end of simulation"), before cbEndOfSimulation. */
    assertion_callbacks_end();
    g_vpi.phase = LLG_VPI_PHASE_ENDED;
    invoke_action(cbEndOfSimulation);
    g_vpi.running = 0;
}

/* ── system task/function calls ─────────────────────────────────────────── */

static int site_is_function(const llg_vpi_callsite_t* site) {
    return site->registration && site->registration->data.type == vpiSysFunc;
}

/* Return shape of a registration. sizetf runs at most once per registration
 * (36.8.1), after the first call site's compiletf, in a phase where no
 * other VPI access is permitted (36.10.2): no call is active, so it cannot
 * size the result from one site's arguments (decision S42-D4). */
static void site_result_shape(llg_vpi_callsite_t* site) {
    llg_vpi_registration_t* registration = site->registration;
    int type = registration->data.sysfunctype;
    site->result_real = type == vpiRealFunc;
    site->result_signed = type == vpiIntFunc || type == vpiSizedSignedFunc;
    site->result_width = type == vpiTimeFunc ? 64u : 32u;
    if (type != vpiSizedFunc && type != vpiSizedSignedFunc) return;
    if (!registration->size_known) {
        registration->size_known = 1;
        registration->size = 32u;
        if (registration->data.sizetf) {
            llg_vpi_call_t* previous = g_vpi.active_call;
            g_vpi.active_call = NULL;
            PLI_INT32 width = registration->data.sizetf(registration->data.user_data);
            g_vpi.active_call = previous;
            if (width <= 0 || (uint32_t)width >= LLG_SUPPORTED_WIDTH_LIMIT) {
                vpi_set_errorf(vpiCompile, vpiError, "LLG_VPI_SIZETF",
                               "sizetf for `%s` returned invalid width %d", site->name, width);
                fprintf(stderr, "llg: VPI error: %s\n", g_vpi.error_message);
                registration->size = 0;
            } else {
                registration->size = (uint32_t)width;
            }
        }
    }
    site->result_width = registration->size;
}

static void reset_return(llg_vpi_call_t* call, const llg_vpi_callsite_t* site) {
    /* 38.34: a function whose calltf puts no value returns 0. */
    call->has_real_return = site->result_real;
    call->real_return = 0.0;
    sv4_replace(&call->return_value,
                sv4_zero(site->result_width ? site->result_width : 1u, site->result_signed));
}

static int compile_site(llg_vpi_callsite_t* site) {
    site->registration = find_registration(site->name);
    if (!site->registration) {
        vpi_set_errorf(vpiCompile, vpiError, "LLG_VPI_UNRESOLVED",
                       "no VPI registration for `%s`", site->name);
        /* Name the call site: the model stops before time 0, so nothing
         * else points at the source line. */
        if (site->file)
            fprintf(stderr, "llg: VPI error: no VPI registration for `%s` (called at %s:%d)\n",
                    site->name, site->file, site->line);
        else
            fprintf(stderr, "llg: VPI error: no VPI registration for `%s`\n", site->name);
        return 0;
    }
    llg_vpi_call_t call;
    memset(&call, 0, sizeof(call));
    call.site = site;
    call.return_value = VPI_EMPTY;
    call.arg_count = site->arg_count;
    call.is_function = site_is_function(site);
    /* Compile-time arguments carry shapes only: X values. */
    if (site->arg_count) {
        call.args = (llg_vpi_arg_t*)calloc((size_t)site->arg_count, sizeof(*call.args));
        if (!call.args) {
            vpi_set_error(vpiCompile, vpiError, "LLG_VPI_NOMEM", "VPI compile-call allocation failed");
            return 0;
        }
        for (int i = 0; i < site->arg_count; ++i) {
            llg_vpi_arg_t* argument = &call.args[i];
            argument->width = site->args[i].width;
            argument->is_signed = site->args[i].is_signed;
            argument->is_real = site->args[i].is_real;
            argument->kind = site->args[i].is_real ? LLG_FMT_REAL : LLG_FMT_PACKED;
            argument->packed = site->args[i].is_real
                ? VPI_EMPTY
                : sv4_x(site->args[i].width ? site->args[i].width : 1u, site->args[i].is_signed);
        }
    }
    int ok = 1;
    if (site->registration->data.compiletf) {
        llg_vpi_call_t* previous = g_vpi.active_call;
        g_vpi.active_call = &call;
        site->active = &call;
        PLI_INT32 result = site->registration->data.compiletf(site->registration->data.user_data);
        site->active = NULL;
        release_call_slots(&call);
        g_vpi.active_call = previous;
        if (result != 0) {
            vpi_set_errorf(vpiCompile, vpiError, "LLG_VPI_COMPILETF", "compiletf rejected `%s`",
                           site->name);
            fprintf(stderr, "llg: VPI error: compiletf rejected `%s`\n", site->name);
            ok = 0;
        }
    }
    for (int i = 0; i < call.arg_count; ++i) sv4_destroy(&call.args[i].packed);
    free(call.args);
    sv4_destroy(&call.return_value);
    if (ok && call.is_function) {
        site_result_shape(site);
        if (site->result_width == 0 && !site->result_real) ok = 0;
    }
    return ok && !g_vpi.failed;
}

static llg_vpi_callsite_t* new_site(uint64_t id, const char* name, const llg_vpi_compile_arg_t* args,
                                    int count, uint64_t time_unit_fs) {
    llg_vpi_callsite_t* site = (llg_vpi_callsite_t*)calloc(1, sizeof(*site));
    if (!site) return NULL;
    if (count) {
        site->args = (llg_vpi_compile_arg_t*)malloc((size_t)count * sizeof(*args));
        if (!site->args) {
            free(site);
            return NULL;
        }
        memcpy(site->args, args, (size_t)count * sizeof(*args));
    }
    site->id = id;
    site->name = name;
    site->arg_count = count;
    site->time_unit_fs = time_unit_fs;
    return site;
}

int llg_vpi_compile_call_site_at(uint64_t id, const char* name,
    const llg_vpi_compile_arg_t* args, int count, uint64_t time_unit_fs,
    llg_vpi_model_object_t* scope, const char* file, int line) {
    if (!vpi_ready()) return 0;
    if (id >= (uint64_t)SIZE_MAX / 2u || !name || count < 0 || count > LLG_VPI_MAX_ARGS ||
        (count && !args) || (id < g_vpi.site_count && g_vpi.sites[id])) {
        vpi_set_error(vpiCompile, vpiError, "LLG_VPI_CALLSITE", "invalid or duplicate VPI call site");
        return 0;
    }
    if (id >= g_vpi.site_capacity) {
        size_t capacity = g_vpi.site_capacity ? g_vpi.site_capacity : 16u;
        while (capacity <= id) capacity *= 2u;
        llg_vpi_callsite_t** grown =
            (llg_vpi_callsite_t**)realloc(g_vpi.sites, capacity * sizeof(*grown));
        if (!grown) {
            vpi_set_error(vpiCompile, vpiError, "LLG_VPI_NOMEM", "VPI call-site table allocation failed");
            return 0;
        }
        memset(grown + g_vpi.site_capacity, 0, (capacity - g_vpi.site_capacity) * sizeof(*grown));
        g_vpi.sites = grown;
        g_vpi.site_capacity = capacity;
    }
    llg_vpi_callsite_t* site = new_site(id, name, args, count, time_unit_fs);
    if (!site) {
        vpi_set_error(vpiCompile, vpiError, "LLG_VPI_NOMEM", "VPI call-site allocation failed");
        return 0;
    }
    site->scope = scope;
    site->file = file;
    site->line = line;
    g_vpi.sites[id] = site;
    if (id >= g_vpi.site_count) g_vpi.site_count = (size_t)id + 1u;
    site->valid = compile_site(site);
    return site->valid;
}

int llg_vpi_compile_call_site(uint64_t id, const char* name,
    const llg_vpi_compile_arg_t* args, int count, uint64_t time_unit_fs) {
    return llg_vpi_compile_call_site_at(id, name, args, count, time_unit_fs, NULL, NULL, 0);
}

int llg_vpi_compile_call(const char* name, const llg_vpi_compile_arg_t* args, int count) {
    if (!vpi_ready()) return 0;
    if (!name || count < 0 || count > LLG_VPI_MAX_ARGS || (count && !args)) {
        vpi_set_error(vpiCompile, vpiError, "LLG_VPI_ARGUMENT", "invalid VPI compile-call descriptor");
        return 0;
    }
    llg_vpi_callsite_t* site = new_site(UINT64_MAX, name, args, count, 0);
    if (!site) return 0;
    int ok = compile_site(site);
    if (site->call_slot) release_slot(site->call_slot - 1u);
    free(site->args);
    free(site);
    return ok;
}

int llg_vpi_compile_calls(const char* const* names,
                          const llg_vpi_compile_arg_t* const* args,
                          const int* counts, int call_count) {
    if (call_count < 0 || call_count > LLG_VPI_MAX_ARGS || (call_count && (!names || !counts))) {
        vpi_set_error(vpiCompile, vpiError, "LLG_VPI_ARGUMENT", "invalid VPI compile-call list");
        return 0;
    }
    for (int i = 0; i < call_count; ++i)
        if (!llg_vpi_compile_call(names[i], args ? args[i] : NULL, counts[i])) return 0;
    return 1;
}

static int begin_call(llg_vpi_call_t* call, llg_vpi_callsite_t* site, llg_vpi_arg_t* args,
                      int count, int function) {
    memset(call, 0, sizeof(*call));
    call->return_value = VPI_EMPTY;
    if (!site || !site->valid || !site->registration ||
        site_is_function(site) != function || site->arg_count != count ||
        (count && !args)) {
        vpi_fail_runtime("uncompiled or inconsistent VPI call site");
        return 0;
    }
    for (int i = 0; i < count; i++) {
        if (site->args[i].width != args[i].width || site->args[i].is_signed != args[i].is_signed ||
            site->args[i].is_real != args[i].is_real) {
            vpi_fail_runtime("VPI argument shape changed after compilation");
            return 0;
        }
    }
    if (site->active || g_vpi.call_depth >= LLG_VPI_MAX_CALL_DEPTH) {
        vpi_fail_runtime("recursive or too deeply nested VPI system call");
        return 0;
    }
    call->site = site;
    call->args = args;
    call->arg_count = count;
    call->is_function = function;
    if (function) reset_return(call, site);
    return 1;
}

static int invoke_calltf(llg_vpi_call_t* call) {
    llg_vpi_callsite_t* site = call->site;
    PLI_INT32 (*calltf)(PLI_BYTE8*) = site->registration->data.calltf;
    if (!calltf) return 1;
    call->previous = g_vpi.active_call;
    g_vpi.active_call = call;
    site->active = call;
    ++g_vpi.call_depth;
    PLI_INT32 result = calltf(site->registration->data.user_data);
    --g_vpi.call_depth;
    site->active = NULL;
    release_call_slots(call);
    g_vpi.active_call = call->previous;
    if (result != 0) {
        vpi_fail_runtime("VPI calltf returned failure");
        return 0;
    }
    return 1;
}

static llg_vpi_callsite_t* site_by_id(uint64_t id) {
    return id < g_vpi.site_count ? g_vpi.sites[id] : NULL;
}

/* Legacy entry points without source identity compile a transient site. */
static llg_vpi_callsite_t* transient_site(const char* name, llg_vpi_arg_t* args, int count) {
    if (!vpi_ready() || count < 0 || count > LLG_VPI_MAX_ARGS || (count && !args)) return NULL;
    llg_vpi_compile_arg_t shapes[LLG_VPI_MAX_ARGS];
    memset(shapes, 0, sizeof(shapes));
    for (int i = 0; i < count; ++i) {
        llg_vpi_compile_arg_t* argument = &shapes[i];
        argument->width = args[i].width;
        argument->is_signed = args[i].is_signed;
        argument->is_real = args[i].is_real;
        argument->object = -1;
    }
    llg_vpi_callsite_t* site = new_site(UINT64_MAX, name, shapes, count, 0);
    if (site) site->valid = compile_site(site);
    return site;
}

static void free_transient_site(llg_vpi_callsite_t* site) {
    if (!site) return;
    if (site->call_slot) release_slot(site->call_slot - 1u);
    free(site->args);
    free(site);
}

int llg_vpi_call_task_site(uint64_t id, const char* name, llg_vpi_arg_t* args, int count) {
    llg_vpi_callsite_t* site = id == UINT64_MAX ? transient_site(name, args, count) : site_by_id(id);
    llg_vpi_call_t call;
    int result = begin_call(&call, site, args, count, 0);
    if (result) result = invoke_calltf(&call);
    sv4_destroy(&call.return_value);
    if (id == UINT64_MAX) free_transient_site(site);
    return result;
}

sv4_t llg_vpi_call_function_site(uint64_t id, const char* name, llg_vpi_arg_t* args, int count,
                                 uint32_t fallback_width, int8_t fallback_signed) {
    llg_vpi_callsite_t* site = id == UINT64_MAX ? transient_site(name, args, count) : site_by_id(id);
    llg_vpi_call_t call;
    sv4_t result = VPI_EMPTY;
    if (!begin_call(&call, site, args, count, 1) || call.has_real_return) {
        if (site && site->valid && call.has_real_return)
            vpi_fail_runtime("VPI real function used in a packed expression");
        sv4_destroy(&call.return_value);
        result = sv4_x(fallback_width ? fallback_width : 1, fallback_signed);
    } else {
        (void)invoke_calltf(&call);
        sv4_move(&result, &call.return_value);
    }
    if (id == UINT64_MAX) free_transient_site(site);
    return result;
}

double llg_vpi_call_real_function_site(uint64_t id, const char* name, llg_vpi_arg_t* args, int count) {
    llg_vpi_callsite_t* site = id == UINT64_MAX ? transient_site(name, args, count) : site_by_id(id);
    llg_vpi_call_t call;
    double result = 0.0;
    if (begin_call(&call, site, args, count, 1)) {
        if (!call.has_real_return) vpi_fail_runtime("VPI packed function used in a real expression");
        else if (invoke_calltf(&call)) result = call.real_return;
    }
    sv4_destroy(&call.return_value);
    if (id == UINT64_MAX) free_transient_site(site);
    return result;
}

int llg_vpi_call_task(const char* name, llg_vpi_arg_t* args, int count) {
    return llg_vpi_call_task_site(UINT64_MAX, name, args, count);
}
sv4_t llg_vpi_call_function(const char* name, llg_vpi_arg_t* args, int count,
                            uint32_t width, int8_t sign) {
    return llg_vpi_call_function_site(UINT64_MAX, name, args, count, width, sign);
}
double llg_vpi_call_real_function(const char* name, llg_vpi_arg_t* args, int count) {
    return llg_vpi_call_real_function_site(UINT64_MAX, name, args, count);
}

int llg_vpi_failed(void) { return g_vpi.failed; }

int llg_vpi_check_builtin_overrides(const llg_vpi_builtin_call_t* calls, uint32_t count) {
    if (!g_vpi.registration_count) return 1;
    for (uint32_t i = 0; i < count; ++i) {
        if (!find_registration(calls[i].name)) continue;
        vpi_set_errorf(vpiCompile, vpiError, "LLG_VPI_BUILTIN_OVERRIDE",
                       "VPI registration of built-in `%s` cannot override it", calls[i].name);
        /* The model stops before time 0; name the call that would have
         * reached the application. */
        if (calls[i].file)
            fprintf(stderr,
                    "llg: VPI error: a VPI application registers `%s`, which llg implements "
                    "as a built-in (called at %s:%d); llg does not support overriding built-in "
                    "system tasks and functions (IEEE 1800-2009 36.3.2)\n",
                    calls[i].name, calls[i].file, calls[i].line);
        else
            fprintf(stderr,
                    "llg: VPI error: a VPI application registers `%s`, which llg implements "
                    "as a built-in; llg does not support overriding built-in system tasks and "
                    "functions (IEEE 1800-2009 36.3.2)\n",
                    calls[i].name);
        return 0;
    }
    return 1;
}

/* Shutdown hook of the DPI-C bridge, which the runtime compiles after this
 * file: llg_dpi_model_init registers it, so a build of this file alone (the
 * storage probes) needs no DPI symbol. */
static void (*g_vpi_dpi_shutdown)(void) = NULL;

void llg_vpi_on_shutdown(void (*hook)(void)) { g_vpi_dpi_shutdown = hook; }

void llg_vpi_shutdown(void) {
    llg_vpi_end_simulation();
    if (g_vpi_dpi_shutdown) {
        void (*hook)(void) = g_vpi_dpi_shutdown;
        g_vpi_dpi_shutdown = NULL;
        hook();
    }
    while (g_vpi.events) {
        llg_vpi_event_t* event = g_vpi.events;
        event->slot = 0;
        vpi_event_cancel(event);
    }
    /* Callback records: cancel queued region callbacks and drop watches
     * (no-ops once the runtime lifetime ended) before freeing them. */
    llg_value_watch_hook(NULL);
    llg_time_slot_hook(NULL);
    llg_force_watch_hook(NULL);
    llg_disable_watch_hook(NULL);
    llg_assertion_watch_hook(NULL);
    llg_timing_violation_hook(NULL);
    tchk_rows_release();
    while (g_vpi.cbs) vpi_cb_retire(g_vpi.cbs);
    free(g_vpi.watch_buckets);
    for (size_t i = g_vpi.static_count; i < g_vpi.slot_count; ++i) {
        llg_vpi_slot_t* slot = &g_vpi.slots[i];
        if (slot->kind == SLOT_EVENT && slot->pointer) {
            llg_vpi_event_t* event = (llg_vpi_event_t*)slot->pointer;
            if (!event->pending) free(event);
        }
        if (slot->kind == SLOT_FIRED_CB) free(slot->pointer);
        free(slot->items);
    }
    free(g_vpi.slots);
    free(g_vpi.sorted);
    free(g_vpi.first_child);
    free(g_vpi.next_sibling);
    free(g_vpi.element_counts);
    for (int i = 0; i < g_vpi.registration_count; ++i) {
        free(g_vpi.registrations[i]->name);
        free(g_vpi.registrations[i]);
    }
    free(g_vpi.registrations);
    for (size_t i = 0; i < g_vpi.site_count; ++i) {
        if (!g_vpi.sites[i]) continue;
        free(g_vpi.sites[i]->args);
        free(g_vpi.sites[i]);
    }
    free(g_vpi.sites);
    free(g_vpi.value_vector);
    free(g_vpi.scratch);
    free(g_vpi.value_text);
    free(g_vpi.value_strength);
    free(g_vpi.mcd_name);
    free(g_vpi.array_values);
    free(g_vpi.decompiled);
    free(g_vpi.storage);
    free(g_vpi.links);
    free(g_vpi.port_links);
    for (int i = 0; i < g_vpi.plugin_count; ++i) llg_dl_close(g_vpi.plugin_handles[i]);
    dyn_shutdown();
    rts_shutdown();
    free(g_vpi.code_parents);
    memset(&g_vpi, 0, sizeof(g_vpi));
}

PLI_INT32 vpi_get_vlog_info(p_vpi_vlog_info info) {
    vpi_enter();
    if (!info) {
        vpi_error("LLG_VPI_ARGUMENT", "null vpi_get_vlog_info output");
        return 0;
    }
    char** argv = NULL;
    info->argc = llg_rt_command_line(&argv);
    info->argv = argv;
    info->product = (PLI_BYTE8*)"lapligence";
    info->version = (PLI_BYTE8*)"llg";
    return 1;
}

PLI_INT32 vpi_vprintf(PLI_BYTE8* format, va_list ap) {
    vpi_enter();
    if (!format) {
        vpi_error("LLG_VPI_ARGUMENT", "null vpi_vprintf format");
        return EOF;
    }
    return vfprintf(stdout, (const char*)format, ap);
}

PLI_INT32 vpi_printf(PLI_BYTE8* format, ...) {
    vpi_enter();
    if (!format) {
        vpi_error("LLG_VPI_ARGUMENT", "null vpi_printf format");
        return EOF;
    }
    va_list ap;
    va_start(ap, format);
    int result = vfprintf(stdout, (const char*)format, ap);
    va_end(ap);
    return result;
}

PLI_INT32 vpi_flush(void) {
    vpi_enter();
    /* 38.5: returns 0 on success and nonzero on failure. */
    return fflush(stdout) == 0 ? 0 : 1;
}

/* ── multichannel descriptors (38.24-38.29) ───────────────────────────────
 * The runtime's $fopen table serves both sides, so descriptors, file names
 * and output order are shared with $fopen/$fdisplay/$fclose. Selected
 * channels that are not open are skipped and reported, as $fdisplay does
 * (decision S25-D7); a $fopen file descriptor (bit 31) is not an mcd. */

static int vpi_mcd_is_fd(PLI_UINT32 mcd, const char* routine) {
    if (!(mcd & UINT32_C(0x80000000))) return 0;
    vpi_set_errorf(vpi_state(), vpiError, "LLG_VPI_MCD",
                   "%s: 0x%08x is a file descriptor, not a multichannel descriptor",
                   routine, (unsigned)mcd);
    return 1;
}

static void vpi_mcd_unopened(PLI_UINT32 channels, const char* routine) {
    vpi_set_errorf(vpi_state(), vpiError, "LLG_VPI_MCD",
                   "%s: multichannel descriptor 0x%08x selects no open channel",
                   routine, (unsigned)channels);
}

PLI_UINT32 vpi_mcd_open(PLI_BYTE8* file) {
    vpi_enter();
    if (!file || !*file) {
        vpi_error("LLG_VPI_ARGUMENT", "vpi_mcd_open needs a file name");
        return 0;
    }
    uint32_t open_fd = 0;
    PLI_UINT32 mcd = llg_file_mcd_open((const char*)file, &open_fd);
    if (open_fd)
        vpi_set_errorf(vpi_state(), vpiError, "LLG_VPI_MCD",
                       "vpi_mcd_open: `%s` is open as file descriptor 0x%08x, which is not a "
                       "multichannel descriptor",
                       (const char*)file, (unsigned)open_fd);
    else if (!mcd)
        vpi_set_errorf(vpi_state(), vpiError, "LLG_VPI_MCD", "vpi_mcd_open: cannot open `%s`",
                       (const char*)file);
    return mcd;
}

PLI_UINT32 vpi_mcd_close(PLI_UINT32 mcd) {
    vpi_enter();
    if (vpi_mcd_is_fd(mcd, "vpi_mcd_close")) return mcd;
    PLI_UINT32 unclosed = llg_file_mcd_close(mcd);
    if (!mcd) {
        vpi_mcd_unopened(mcd, "vpi_mcd_close");
    } else if (unclosed & 1u) {
        vpi_error("LLG_VPI_MCD", "vpi_mcd_close: channel 1 (the standard output) cannot be closed");
    } else if (unclosed) {
        vpi_mcd_unopened(unclosed, "vpi_mcd_close");
    }
    return unclosed;
}

PLI_BYTE8* vpi_mcd_name(PLI_UINT32 cd) {
    vpi_enter();
    const char* name = llg_file_name(cd);
    if (!name) {
        vpi_set_errorf(vpi_state(), vpiError, "LLG_VPI_MCD",
                       "vpi_mcd_name: 0x%08x is not one open channel or file descriptor",
                       (unsigned)cd);
        return NULL;
    }
    /* 38.26: the result is overwritten by the next call; the channel may
     * close before that, so the name is copied. */
    size_t length = strlen(name) + 1u;
    if (length > g_vpi.mcd_name_capacity) {
        char* grown = (char*)realloc(g_vpi.mcd_name, length);
        if (!grown) {
            vpi_error("LLG_VPI_NOMEM", "vpi_mcd_name allocation failed");
            return NULL;
        }
        g_vpi.mcd_name = grown;
        g_vpi.mcd_name_capacity = length;
    }
    memcpy(g_vpi.mcd_name, name, length);
    return (PLI_BYTE8*)g_vpi.mcd_name;
}

PLI_INT32 vpi_mcd_vprintf(PLI_UINT32 mcd, PLI_BYTE8* format, va_list ap) {
    vpi_enter();
    if (!format) {
        vpi_error("LLG_VPI_ARGUMENT", "null vpi_mcd_vprintf format");
        return EOF;
    }
    if (vpi_mcd_is_fd(mcd, "vpi_mcd_printf")) return EOF;
    /* Format once, then write the same bytes to every selected channel. */
    char local[512];
    va_list sized;
    va_copy(sized, ap);
    int length = vsnprintf(local, sizeof(local), (const char*)format, sized);
    va_end(sized);
    if (length < 0) {
        vpi_error("LLG_VPI_ARGUMENT", "vpi_mcd_printf: invalid format");
        return EOF;
    }
    char* text = local;
    if ((size_t)length >= sizeof(local)) {
        text = (char*)malloc((size_t)length + 1u);
        if (!text) {
            vpi_error("LLG_VPI_NOMEM", "vpi_mcd_printf allocation failed");
            return EOF;
        }
        vsnprintf(text, (size_t)length + 1u, (const char*)format, ap);
    }
    PLI_UINT32 written = llg_file_mcd_write(mcd, text, (size_t)length);
    if (text != local) free(text);
    if (!written || written != mcd) vpi_mcd_unopened(mcd & ~written, "vpi_mcd_printf");
    /* 38.28: the number of characters printed, or EOF when nothing was. */
    return written ? length : EOF;
}

PLI_INT32 vpi_mcd_printf(PLI_UINT32 mcd, PLI_BYTE8* format, ...) {
    va_list ap;
    va_start(ap, format);
    PLI_INT32 result = vpi_mcd_vprintf(mcd, format, ap);
    va_end(ap);
    return result;
}

PLI_INT32 vpi_mcd_flush(PLI_UINT32 mcd) {
    vpi_enter();
    if (vpi_mcd_is_fd(mcd, "vpi_mcd_flush")) return 1;
    /* Descriptor 0 names no channel: there is nothing to flush, which
     * succeeds (decision A20-D4). */
    if (!mcd) return 0;
    PLI_UINT32 flushed = llg_file_mcd_flush(mcd);
    if (flushed != mcd) {
        vpi_mcd_unopened(mcd & ~flushed, "vpi_mcd_flush");
        return 1;
    }
    /* 38.25: 0 on success, nonzero on failure. */
    return 0;
}

/* ── routines llg does not provide yet ─────────────────────────────────────
 * Defined so that applications written against the full Annex K/M headers
 * link; each fails through vpi_chk_error() without reading or writing its
 * arguments, never with a guessed success. */

static void vpi_unsupported_routine(const char* routine, const char* reason) {
    vpi_set_errorf(vpi_state(), vpiError, "LLG_VPI_UNSUPPORTED", "%s is not supported by llg (%s)",
                   routine, reason);
}

/* 38.22: the reference handles after the first two are not read (an
 * intermodule path has two ports). */
vpiHandle vpi_handle_multi(PLI_INT32 type, vpiHandle ref_handle1, vpiHandle ref_handle2, ...) {
    vpi_enter();
    if (!vpi_ready()) return NULL;
    return intermodule_path(type, ref_handle1, ref_handle2);
}

PLI_INT32 vpi_get_data(PLI_INT32 id, PLI_BYTE8* dataLoc, PLI_INT32 numOfBytes) {
    (void)id;
    (void)dataLoc;
    (void)numOfBytes;
    vpi_enter();
    vpi_unsupported_routine("vpi_get_data", "no save/restart");
    return 0;
}

PLI_INT32 vpi_put_data(PLI_INT32 id, PLI_BYTE8* dataLoc, PLI_INT32 numOfBytes) {
    (void)id;
    (void)dataLoc;
    (void)numOfBytes;
    vpi_enter();
    vpi_unsupported_routine("vpi_put_data", "no save/restart");
    return 0;
}

/* The assertion item whose whole property is instance entry `row`, or -1:
 * its vpiProperty, or the vpiPropertyExpr of its property specification. */
static int64_t instance_item(uint32_t row) {
    for (size_t i = 0; i < llg_acov_item_count(); ++i) {
        const llg_acov_item_t* item = llg_acov_item_at(i);
        const llg_vpi_code_t* root = item->code ? code_at(item->code - 1u) : NULL;
        uint32_t property = root ? code_child(root, vpiProperty) : LLG_VPI_NONE;
        if (property == LLG_VPI_NONE) continue;
        const llg_vpi_code_t* node = code_at(property);
        if (node->type == vpiPropertySpec) property = code_child(node, vpiPropertyExpr);
        if (property != LLG_VPI_NONE && code_visible(property) == row) return (int64_t)i;
    }
    return -1;
}

/* 39.4.2: a callback on one assertion statement, or on a sequence or
 * property instance. The attempt reasons run from the runtime's attempt
 * events, the control reasons from vpi_control (39.5.2). An instance takes
 * cbAssertionStart, cbAssertionSuccess and cbAssertionFailure, raised by the
 * attempts of the assertion whose whole property it is (decision A17-D5);
 * a nested instance would need a hook in every sequence engine and fails
 * with vpiError (decision A17-D6). cbAssertionDisable and cbAssertionEnable
 * also run from the control tasks. A step callback is accepted but never
 * runs: stepping cannot be enabled (decision A17-D2). */
vpiHandle vpi_register_assertion_cb(vpiHandle assertion, PLI_INT32 reason,
                                    vpi_assertion_callback_func* cb_rtn, PLI_BYTE8* user_data) {
    vpi_enter();
    if (!cb_rtn) {
        vpi_error("LLG_VPI_CALLBACK", "vpi_register_assertion_cb needs a callback routine");
        return NULL;
    }
    int step = reason == cbAssertionStepSuccess || reason == cbAssertionStepFailure;
    int attempt = assertion_attempt_reason(reason);
    if (!attempt && !step && !assertion_control_reason(reason)) {
        vpi_error("LLG_VPI_CALLBACK", "invalid vpi_register_assertion_cb reason");
        return NULL;
    }
    llg_vpi_slot_t* slot = checked_handle(assertion, NULL);
    if (!slot) return NULL;
    uint32_t instance = 0;
    int64_t item_index = -1;
    if (slot->kind == SLOT_CODE &&
        (code_type(code_at((uint32_t)slot->aux)) == vpiSequenceInst ||
         code_type(code_at((uint32_t)slot->aux)) == vpiPropertyInst)) {
        if (reason != cbAssertionStart && reason != cbAssertionSuccess &&
            reason != cbAssertionFailure) {
            vpi_error("LLG_VPI_CALLBACK", "a sequence or property instance takes only "
                                          "cbAssertionStart, cbAssertionSuccess and "
                                          "cbAssertionFailure (39.4.2)");
            return NULL;
        }
        item_index = instance_item((uint32_t)slot->aux);
        if (item_index < 0) {
            vpi_error("LLG_VPI_UNSUPPORTED",
                      "llg raises instance callbacks only for an instance that is the whole "
                      "property of an assertion");
            return NULL;
        }
        instance = (uint32_t)slot->aux + 1u;
    } else if (slot->kind == SLOT_ASSERTION && slot_item(slot)) {
        item_index = slot->left;
    } else {
        vpi_error("LLG_VPI_CALLBACK",
                  "vpi_register_assertion_cb needs an assertion or a sequence or property "
                  "instance handle");
        return NULL;
    }
    if (g_vpi.assertions_ended || g_vpi.phase >= LLG_VPI_PHASE_ENDED) {
        vpi_error("LLG_VPI_CALLBACK",
                  "the assertion system has ended; no assertion callback can occur");
        return NULL;
    }
    uint32_t item = (uint32_t)item_index;
    llg_vpi_cb_t* cb = (llg_vpi_cb_t*)calloc(1, sizeof(*cb));
    if (!cb) {
        vpi_error("LLG_VPI_NOMEM", "VPI callback allocation failed");
        return NULL;
    }
    cb->kind = CB_ASSERTION;
    cb->last = VPI_EMPTY;
    cb->time.type = vpiSimTime;
    cb->value.format = vpiSuppressVal;
    cb->data.reason = reason;
    cb->data.user_data = user_data;
    cb->assertion_rtn = cb_rtn;
    cb->assertion_item = item;
    /* The runtime raises attempt events and the control-task events only
     * for watched items. */
    cb->assertion_attempt = attempt || reason == cbAssertionDisable || reason == cbAssertionEnable;
    cb->instance_code = instance;
    uint32_t index = alloc_slot(SLOT_CALLBACK);
    if (index == LLG_VPI_NONE) {
        vpi_cb_free(cb);
        return NULL;
    }
    cb->data.obj = instance ? code_handle(instance - 1u) : assertion_handle(item);
    if (!cb->data.obj) {
        release_slot(index);
        vpi_cb_free(cb);
        return NULL;
    }
    g_vpi.slots[index].pointer = cb;
    cb->slot = index;
    cb->sequence = g_vpi.cb_sequence++;
    cb->registered_at = llg_time();
    if (!g_vpi.cbs_tail) g_vpi.cbs_tail = &g_vpi.cbs;
    cb->prev_link = g_vpi.cbs_tail;
    *g_vpi.cbs_tail = cb;
    g_vpi.cbs_tail = &cb->next;
    ++g_vpi.assertion_cbs;
    if (cb->assertion_attempt) {
        llg_acov_watch(item, 1);
        llg_assertion_watch_hook(vpi_assertion_event);
    }
    return encode_handle(index);
}

/* ── collections (39.5.1; IEEE P1800-2005 30.4.2) ─────────────────────── */

/* vpi_create(vpiCollection, collection or NULL, member or NULL): add a
 * module, interface or program instance or an assertion to a collection,
 * made when `collection` is NULL; returns the collection (A17-D6). */
vpiHandle vpi_create(PLI_INT32 type, vpiHandle collection, vpiHandle object) {
    vpi_enter();
    if (type != vpiCollection) {
        vpi_error("LLG_VPI_UNSUPPORTED", "llg creates only vpiCollection objects");
        return NULL;
    }
    uintptr_t member = 0;
    if (object) {
        llg_vpi_slot_t* slot = checked_handle(object, NULL);
        if (!slot) return NULL;
        if (slot->kind == SLOT_ASSERTION && slot_item(slot)) {
            member = ((uintptr_t)slot->left << 1) | 1u;
        } else if (slot_instance(slot) >= 0) {
            member = (uintptr_t)slot->object << 1;
        } else {
            vpi_error("LLG_VPI_ARGUMENT",
                      "a collection holds module, interface and program instances and "
                      "assertions");
            return NULL;
        }
    }
    uint32_t index;
    if (collection) {
        llg_vpi_slot_t* slot = checked_handle(collection, &index);
        if (!slot) return NULL;
        if (slot->kind != SLOT_COLLECTION) {
            vpi_error("LLG_VPI_ARGUMENT", "vpi_create adds to a vpiCollection handle");
            return NULL;
        }
    } else {
        index = alloc_slot(SLOT_COLLECTION);
        if (index == LLG_VPI_NONE) return NULL;
        collection = encode_handle(index);
    }
    if (!object) return collection;
    llg_vpi_slot_t* slot = &g_vpi.slots[index];
    uintptr_t* items = (uintptr_t*)realloc(slot->items, (slot->count + 1u) * sizeof(*items));
    if (!items) {
        vpi_error("LLG_VPI_NOMEM", "VPI collection allocation failed");
        return NULL;
    }
    items[slot->count++] = member;
    slot->items = items;
    return collection;
}

/* vpi_iterate(vpiMember, collection): its members in insertion order. */
static vpiHandle iterate_collection(const llg_vpi_slot_t* slot) {
    if (!slot->count) return NULL;
    uint32_t count = slot->count;
    uintptr_t* members = (uintptr_t*)malloc(count * sizeof(*members));
    uintptr_t* items = (uintptr_t*)calloc(count, sizeof(*items));
    if (!members || !items) {
        free(members);
        free(items);
        vpi_error("LLG_VPI_NOMEM", "VPI iterator allocation failed");
        return NULL;
    }
    /* Making handles may move the slot array: copy the members first. */
    memcpy(members, slot->items, count * sizeof(*members));
    uint32_t made = 0;
    for (uint32_t i = 0; i < count; ++i) {
        vpiHandle handle = (members[i] & 1u) ? assertion_handle((size_t)(members[i] >> 1))
                                             : object_handle((uint32_t)(members[i] >> 1));
        if (!handle) break;
        items[made++] = (uintptr_t)handle;
    }
    free(members);
    return make_list_iterator(vpiMember, items, made);
}

/* Whether item `index` is selected by collection member `member`. */
static int item_in_member(size_t index, uintptr_t member) {
    if (member & 1u) return (size_t)(member >> 1) == index;
    llg_vpi_model_object_t* object = row_object((uint32_t)(member >> 1));
    int32_t scope = object ? llg_acov_instance_named(object->full_name) : -1;
    return scope >= 0 && item_in_instance(index, scope);
}

/* ── assertion control (39.5) ───────────────────────────────────────────── */

typedef struct {
    int operation; /* vpi_control operation */
    int runtime;   /* LLG_ACOV_OP_* */
    int reason;    /* callback reason it raises, 0 for none */
} llg_vpi_assertion_op_t;

/* 39.5.2: the operations on one assertion and the callback each raises. */
static const llg_vpi_assertion_op_t assertion_ops[] = {
    {vpiAssertionDisable, LLG_ACOV_OP_DISABLE, cbAssertionDisable},
    {vpiAssertionEnable, LLG_ACOV_OP_ENABLE, cbAssertionEnable},
    {vpiAssertionReset, LLG_ACOV_OP_RESET, cbAssertionReset},
    {vpiAssertionKill, LLG_ACOV_OP_KILL, 0},
    {vpiAssertionDisablePassAction, LLG_ACOV_OP_PASS_OFF, cbAssertionDisablePassAction},
    {vpiAssertionEnablePassAction, LLG_ACOV_OP_PASS_ON, cbAssertionEnablePassAction},
    {vpiAssertionDisableFailAction, LLG_ACOV_OP_FAIL_OFF, cbAssertionDisableFailAction},
    {vpiAssertionEnableFailAction, LLG_ACOV_OP_FAIL_ON, cbAssertionEnableFailAction},
    {vpiAssertionDisableVacuousAction, LLG_ACOV_OP_VACUOUS_OFF, cbAssertionDisableVacuousAction},
    {vpiAssertionEnableNonvacuousAction, LLG_ACOV_OP_NONVACUOUS_ON,
     cbAssertionEnableNonvacuousAction},
};

/* 39.5.1: the system operations, applied to each selected assertion, and
 * the system callback each raises. */
static const llg_vpi_assertion_op_t assertion_system_ops[] = {
    {vpiAssertionSysOn, LLG_ACOV_OP_ENABLE, cbAssertionSysOn},
    {vpiAssertionSysOff, LLG_ACOV_OP_DISABLE, cbAssertionSysOff},
    {vpiAssertionSysKill, LLG_ACOV_OP_KILL_ALL, cbAssertionSysKill},
    {vpiAssertionSysEnd, LLG_ACOV_OP_KILL_ALL, cbAssertionSysEnd},
    {vpiAssertionSysReset, LLG_ACOV_OP_RESET, cbAssertionSysReset},
    {vpiAssertionSysEnablePassAction, LLG_ACOV_OP_PASS_ON, cbAssertionSysEnablePassAction},
    {vpiAssertionSysEnableFailAction, LLG_ACOV_OP_FAIL_ON, cbAssertionSysEnableFailAction},
    {vpiAssertionSysDisablePassAction, LLG_ACOV_OP_PASS_OFF, cbAssertionSysDisablePassAction},
    {vpiAssertionSysDisableFailAction, LLG_ACOV_OP_FAIL_OFF, cbAssertionSysDisableFailAction},
    {vpiAssertionSysEnableNonvacuousAction, LLG_ACOV_OP_NONVACUOUS_ON,
     cbAssertionSysEnableNonvacuousAction},
    {vpiAssertionSysDisableVacuousAction, LLG_ACOV_OP_VACUOUS_OFF,
     cbAssertionSysDisableVacuousAction},
};

static const llg_vpi_assertion_op_t* find_assertion_op(const llg_vpi_assertion_op_t* table,
                                                       size_t count, int operation) {
    for (size_t i = 0; i < count; ++i)
        if (table[i].operation == operation) return &table[i];
    return NULL;
}

static int is_assertion_control(int operation) {
    return operation >= vpiAssertionDisable && operation <= vpiAssertionSysDisableVacuousAction;
}

/* The assertion item of an assertion handle, or -1 with the error set. */
static int64_t control_item(vpiHandle handle) {
    llg_vpi_slot_t* slot = checked_handle(handle, NULL);
    if (!slot) return -1;
    if (slot->kind != SLOT_ASSERTION || !slot_item(slot)) {
        vpi_error("LLG_VPI_ARGUMENT", "assertion control needs an assertion handle");
        return -1;
    }
    return slot->left;
}

/* An attempt start time argument (39.5.2), in simulation time units. */
static int control_start_time(const s_vpi_time* time, uint64_t* start) {
    int ok = 0;
    if (time) *start = ticks_from_time(time, NULL, &ok);
    if (!ok) vpi_error("LLG_VPI_ARGUMENT",
                       "assertion attempt start time needs a vpiSimTime or vpiScaledRealTime time");
    return ok;
}

/* Whether item `index` lies in instance `scope` or below it. */
static int item_in_instance(size_t index, int32_t scope) {
    int32_t instance = llg_acov_item_at(index)->instance;
    while (instance >= 0) {
        if (instance == scope) return 1;
        const llg_acov_instance_t* node = llg_acov_instance_at((size_t)instance);
        instance = node ? node->parent : -1;
    }
    return 0;
}

static PLI_INT32 assertion_control(int operation, va_list ap) {
    if (g_vpi.assertions_ended || g_vpi.phase >= LLG_VPI_PHASE_ENDED) {
        vpi_error("LLG_VPI_CONTROL", "the assertion system has ended (vpiAssertionSysEnd)");
        return 0;
    }
    vpiHandle handle = va_arg(ap, vpiHandle);
    if (operation == vpiAssertionEnableStep || operation == vpiAssertionDisableStep) {
        int64_t item = control_item(handle);
        uint64_t start = 0;
        if (item < 0 || !control_start_time(va_arg(ap, p_vpi_time), &start)) return 0;
        /* Stepping is never enabled: enabling it fails, and so does
         * disabling it, which has nothing to act on (decision A17-D2). */
        vpi_error("LLG_VPI_UNSUPPORTED", operation == vpiAssertionEnableStep
                                             ? "llg has no assertion step callbacks"
                                             : "assertion stepping is not enabled");
        return 0;
    }
    const llg_vpi_assertion_op_t* op =
        find_assertion_op(assertion_ops, sizeof(assertion_ops) / sizeof(assertion_ops[0]), operation);
    if (op) {
        int64_t item = control_item(handle);
        uint64_t start = 0;
        if (item < 0) return 0;
        if (operation == vpiAssertionKill) {
            if (!control_start_time(va_arg(ap, p_vpi_time), &start)) return 0;
            if (!llg_acov_attempt_at((size_t)item, start)) {
                vpi_error("LLG_VPI_ARGUMENT",
                          "no attempt of this assertion that began at that time is in progress");
                return 0;
            }
        }
        (void)llg_acov_control((size_t)item, op->runtime, start);
        if (op->reason) invoke_assertion_cbs((uint32_t)item, op->reason, llg_time(), NULL);
        return 1;
    }
    op = find_assertion_op(assertion_system_ops,
                           sizeof(assertion_system_ops) / sizeof(assertion_system_ops[0]), operation);
    if (!op) {
        vpi_error("LLG_VPI_UNSUPPORTED", "unsupported vpi_control operation");
        return 0;
    }
    /* NULL selects every assertion; an instance its assertions and those
     * below it; an assertion handle that assertion (39.5.1). */
    int64_t only = -1;
    int32_t scope = -1;
    uintptr_t* members = NULL;
    uint32_t member_count = 0;
    if (handle) {
        llg_vpi_slot_t* slot = checked_handle(handle, NULL);
        if (!slot) return 0;
        if (operation == vpiAssertionSysEnd) {
            vpi_error("LLG_VPI_ARGUMENT", "vpiAssertionSysEnd ends the whole assertion system: "
                                          "pass a NULL handle");
            return 0;
        }
        if (slot->kind == SLOT_COLLECTION) {
            /* Each member selects as a single handle would (A17-D6). A
             * callback the control raises may add to the collection, so
             * the members are copied. */
            member_count = slot->count;
            members = (uintptr_t*)malloc((member_count ? member_count : 1u) * sizeof(*members));
            if (!members) {
                vpi_error("LLG_VPI_NOMEM", "VPI collection allocation failed");
                return 0;
            }
            if (member_count) memcpy(members, slot->items, member_count * sizeof(*members));
        } else if (slot->kind == SLOT_ASSERTION && slot_item(slot)) {
            only = slot->left;
        } else {
            scope = slot_instance(slot);
            if (scope < 0) {
                vpi_error("LLG_VPI_ARGUMENT",
                          "assertion system control needs NULL, an instance or an assertion "
                          "handle");
                return 0;
            }
        }
    }
    if (operation == vpiAssertionSysEnd) llg_assertions_end();
    for (size_t i = 0; i < llg_acov_item_count(); ++i) {
        if (members) {
            uint32_t m = 0;
            while (m < member_count && !item_in_member(i, members[m])) ++m;
            if (m == member_count) continue;
        } else if (only >= 0 ? (int64_t)i != only : handle && !item_in_instance(i, scope)) {
            continue;
        }
        (void)llg_acov_control(i, op->runtime, 0);
    }
    free(members);
    if (operation == vpiAssertionSysEnd) assertion_callbacks_end();
    else invoke_assertion_system(op->reason);
    return 1;
}

/* 38.4: vpiStop and vpiFinish take the diagnostic level of $stop/$finish
 * and act when the application routine returns. vpiReset executes $reset,
 * an optional Annex D task llg does not provide, and llg has no interactive
 * mode whose scope vpiSetInteractiveScope could change: both return 0 (38.4:
 * "0 (false) on a failure") with vpiError and leave the run as it was
 * (decision A20-D11). */
PLI_INT32 vpi_control(PLI_INT32 operation, ...) {
    vpi_enter();
    if (is_assertion_control(operation)) {
        va_list ap;
        va_start(ap, operation);
        PLI_INT32 status = assertion_control(operation, ap);
        va_end(ap);
        return status;
    }
    if (operation >= vpiCoverageStart && operation <= vpiCoverageSave) {
        va_list ap;
        va_start(ap, operation);
        int type = va_arg(ap, int);
        PLI_INT32 status;
        if (operation == vpiCoverageMerge || operation == vpiCoverageSave) {
            const char* name = va_arg(ap, const char*);
            status = coverage_file_control(operation, type, name);
        } else {
            vpiHandle object = va_arg(ap, vpiHandle);
            status = coverage_control(operation, type, object);
        }
        va_end(ap);
        return status;
    }
    if (operation == vpiReset) {
        vpi_error("LLG_VPI_UNSUPPORTED",
                  "vpiReset executes $reset, an optional Annex D task llg does not provide");
        return 0;
    }
    if (operation == vpiSetInteractiveScope) {
        vpi_error("LLG_VPI_UNSUPPORTED", "llg has no interactive mode or interactive scope");
        return 0;
    }
    if (operation != vpiStop && operation != vpiFinish) {
        vpi_error("LLG_VPI_UNSUPPORTED", "unsupported vpi_control operation");
        return 0;
    }
    va_list ap;
    va_start(ap, operation);
    int level = va_arg(ap, int);
    va_end(ap);
    /* A level outside the $stop/$finish levels 0-2 is accepted and reports
     * like the default level 1 (decision S43-D8; 20.2). */
    if (level < 0 || level > 2) level = 1;
    if (g_vpi.phase >= LLG_VPI_PHASE_ENDED) return 1; /* the simulation already ended */
    if (operation == vpiFinish) {
        llg_rt_control_finish(level);
        return 1;
    }
    /* Before the run, a stop is taken when the start callbacks return
     * (decision S43-D10). */
    if (g_vpi.starting && llg_rt_control_stop_before_run(level)) return 1;
    if (!llg_rt_control_stop(level)) {
        vpi_error("LLG_VPI_CONTROL", "vpiStop needs a running, unfinished simulation");
        return 0;
    }
    return 1;
}

/* Destination-passing `_to` forms, see value/destinations.h. */
void llg_vpi_call_function_to(sv4_t* dst, const char* name, llg_vpi_arg_t* args, int arg_count, uint32_t fallback_width, int8_t fallback_signed) {
    sv4_replace(dst, llg_vpi_call_function(name, args, arg_count, fallback_width, fallback_signed));
}
void llg_vpi_call_function_site_to(sv4_t* dst, uint64_t site, const char* name, llg_vpi_arg_t* args, int count, uint32_t fallback_width, int8_t fallback_signed) {
    sv4_replace(dst, llg_vpi_call_function_site(site, name, args, count, fallback_width, fallback_signed));
}
