// llg_vpi.h — generated-model bridge for the public VPI surface.
#ifndef LLG_VPI_H
#define LLG_VPI_H

#include "llg_rt.h"
#include "sv_vpi_user.h" /* vpi_user.h and the typespec types the catalog names */

#ifdef __cplusplus
extern "C" {
#endif

/* Flags of llg_vpi_model_object_t. */
#define LLG_VPI_OBJ_TWO_STATE 0x01u   /* bit/int/byte/... storage */
#define LLG_VPI_OBJ_TOP 0x02u         /* top-level module instance */
#define LLG_VPI_OBJ_LOCAL_PARAM 0x04u /* localparam / genvar value */
#define LLG_VPI_OBJ_SHORTREAL 0x08u   /* writes round through float */
#define LLG_VPI_OBJ_RANGE 0x10u       /* left/right hold the packed range */
#define LLG_VPI_OBJ_TEXT 0x20u        /* `constant` is string-parameter text */
#define LLG_VPI_OBJ_MEMBER 0x40u      /* member of the struct/union row `parent` */
#define LLG_VPI_OBJ_SLICE 0x80u       /* value: low `width` bits of `packed` */
#define LLG_VPI_OBJ_RECORD 0x100u     /* unpacked struct/union: members hold the value */
#define LLG_VPI_OBJ_UNNAMED 0x200u    /* no vpiName/vpiFullName; never found by name */
#define LLG_VPI_OBJ_QUALIFIER 0x400u  /* vpiNetDeclAssign / vpiAutomatic */
#define LLG_VPI_OBJ_ACTIVATION 0x800u /* activation_* name what `disable` targets */
#define LLG_VPI_OBJ_DYNAMIC 0x1000u   /* string, handle or container: `storage`, `shape` */

/* Typespecs (IEEE 1800-2009 37.23). The generated model emits interned
 * tables; fields naming an optional entry hold its index + 1 (0: none),
 * other indices are plain table positions. */
#define LLG_VPI_TS_SIGNED 0x01u
#define LLG_VPI_TS_TWO_STATE 0x02u
#define LLG_VPI_TS_PACKED 0x04u       /* vpiPacked struct/union */
#define LLG_VPI_TS_UNPACKED 0x08u     /* struct/union known not to be packed */
#define LLG_VPI_TS_TAGGED 0x10u

typedef struct {
    int type;                 /* vpiXxxTypespec */
    const char* name;         /* typedef name, or NULL */
    uint32_t alias;           /* vpiTypedefAlias: index + 1 */
    uint32_t element;         /* vpiElemTypespec: index + 1 */
    uint32_t base;            /* vpiBaseTypespec of an enum: index + 1 */
    uint32_t width;           /* bits of a fixed packed value or image, else 0 */
    uint32_t flags;           /* LLG_VPI_TS_* */
    uint32_t first_range;     /* (left, right) pairs, leftmost first */
    uint32_t range_count;
    uint32_t first_member;    /* vpiTypespecMember entries */
    uint32_t member_count;
    uint32_t first_const;     /* vpiEnumConst entries */
    uint32_t const_count;
    uint32_t first_node;      /* top-level member nodes of a value of this type */
    uint32_t node_count;
} llg_vpi_typespec_t;

typedef struct {
    const char* name;
    uint32_t typespec;        /* index */
} llg_vpi_typespec_member_t;

typedef struct {
    const char* name;
    const char* value;        /* 0/1/x/z digits, most significant first */
} llg_vpi_enum_const_t;

/* A member of a packed struct/union value (or a fixed unpacked image) at an
 * absolute bit offset in the value; the children of a node are contiguous. */
typedef struct {
    const char* name;
    int type;                 /* vpiType of the member variable */
    uint32_t typespec;        /* index */
    uint32_t lsb;
    uint32_t width;
    uint32_t flags;           /* LLG_VPI_TS_SIGNED, LLG_VPI_TS_TWO_STATE */
    uint32_t parent;          /* node index + 1; 0: a top-level member */
    uint32_t first_child;
    uint32_t child_count;
} llg_vpi_member_node_t;

typedef struct {
    const llg_vpi_typespec_t* typespecs;
    uint32_t typespec_count;
    const int64_t* ranges;    /* range_count (left, right) pairs */
    uint32_t range_count;
    const llg_vpi_typespec_member_t* members;
    uint32_t member_count;
    const llg_vpi_enum_const_t* consts;
    uint32_t const_count;
    const llg_vpi_member_node_t* nodes;
    uint32_t node_count;
} llg_vpi_type_tables_t;

/* Expression and statement objects (IEEE 1800-2009 37.54-37.74) of a
 * model generated for VPI applications: one static row per object. The
 * children of a row are contiguous rows; each names the relation that
 * reaches it from its parent (`role`: vpiOperand, vpiLhs, vpiStmt, ...).
 * Fields naming an optional entry hold its index + 1 (0: none). */
#define LLG_VPI_CODE_SIGNED 0x01u
#define LLG_VPI_CODE_REAL 0x02u       /* real-valued expression */
#define LLG_VPI_CODE_HIDDEN 0x04u     /* implicit conversion: evaluated, never an object */
#define LLG_VPI_CODE_OPAQUE 0x08u     /* expression llg does not decompose: no operands or value */
#define LLG_VPI_CODE_ALIAS 0x10u      /* stands for catalog row `object` (a reference, a named block) */
#define LLG_VPI_CODE_BLOCKING 0x20u   /* vpiBlocking */
#define LLG_VPI_CODE_CONSTANT 0x40u   /* vpiConstantSelect */
#define LLG_VPI_CODE_USER 0x80u       /* vpiUserDefn of a system task or function call */
#define LLG_VPI_CODE_LOCALS 0x100u    /* vpiLocalVarDecls of a for statement */
#define LLG_VPI_CODE_TWO_STATE 0x200u /* a conversion to a two-state type */
#define LLG_VPI_CODE_STRING 0x400u    /* a constant whose text is the string's characters */
#define LLG_VPI_CODE_INFERRED 0x800u  /* an assertion whose clocking event is inferred */
#define LLG_VPI_CODE_LINK 0x1000u     /* stands for root row `first` (no children) */

typedef struct {
    int16_t type;             /* vpiType */
    int16_t subtype;          /* vpiOpType, vpiConstType, vpiCaseType, vpiIndexedPartSelectType */
    int16_t role;             /* relation from the parent; 0 for a root */
    int16_t qualifier;        /* vpiQualifier of if, if-else and case */
    uint32_t flags;           /* LLG_VPI_CODE_* */
    uint32_t width;           /* vpiSize of an expression */
    uint32_t file;            /* file table index + 1 */
    uint32_t line;
    int32_t object;           /* catalog row it stands for or names, or -1 */
    uint32_t scope;           /* enclosing scope: catalog row + 1 */
    uint32_t first;           /* first child row */
    uint32_t count;           /* children */
    uint32_t data;            /* text index + 1 (constant digits or text, a call, object or
                               * label name); range index + 1 of a select */
    uint32_t typespec;        /* vpiTypespec of a cast: typespec index + 1 */
} llg_vpi_code_t;

typedef struct {
    const llg_vpi_code_t* nodes;
    uint32_t node_count;
    const char* const* texts;
    uint32_t text_count;
    const char* const* files;
    uint32_t file_count;
    const int64_t* ranges;    /* (left, right) of the dimension a select indexes */
    uint32_t range_count;
} llg_vpi_code_tables_t;

/* How a call argument is visible to applications (llg_vpi_compile_arg_t). */
#define LLG_VPI_ARG_EXPRESSION 0 /* read-only value snapshot (vpiOperation) */
#define LLG_VPI_ARG_CONSTANT 1   /* literal (vpiConstant, const_type) */
#define LLG_VPI_ARG_OBJECT 2     /* catalog object `object` */
#define LLG_VPI_ARG_SELECT 3     /* bits [offset, offset+select_width) of `object` */
#define LLG_VPI_ARG_ELEMENT 4    /* element `offset` (row-major) of array `object` */

typedef struct {
    uint32_t width;
    int8_t is_signed;
    int8_t is_real;
    int8_t ref_kind;          /* LLG_VPI_ARG_* */
    int const_type;           /* vpiConstType of a constant */
    int64_t object;           /* catalog index, or -1 */
    uint64_t offset;
    uint32_t select_width;
    int64_t left;             /* declared bounds of a select */
    int64_t right;
    uint32_t code;            /* the expression object (llg_vpi_code_t row + 1), or 0 */
} llg_vpi_compile_arg_t;

/* One expression connection of a structure object (37.14, 37.31, 37.43):
 * the high connection of a port, the lhs and rhs of a continuous
 * assignment, a primitive terminal. `ref` says how the application sees it;
 * `value` holds the digits of a constant (0/1/x/z, most significant first). */
typedef struct {
    llg_vpi_compile_arg_t ref;
    int direction;            /* vpiInput/vpiOutput/vpiInout of a terminal, else 0 */
    const char* value;
    int role;                 /* LLG_VPI_ROLE_* of a module path or timing check entry */
    int edge;                 /* vpiEdge of a path or timing check terminal */
    uint32_t condition;       /* a terminal's condition: conn index + 1, 0: none */
} llg_vpi_conn_t;

/* Roles of the connections of a module path (37.35) or timing check
 * (37.36). A timing check lists its arguments in source order (vpiExpr),
 * then the conditions its terminals name. */
#define LLG_VPI_ROLE_CONNECTION 0 /* any other structure connection */
#define LLG_VPI_ROLE_PATH_IN 1    /* vpiModPathIn path term */
#define LLG_VPI_ROLE_PATH_OUT 2   /* vpiModPathOut path term */
#define LLG_VPI_ROLE_PATH_DATA 3  /* vpiModDataPathIn path term */
#define LLG_VPI_ROLE_CONDITION 4  /* vpiCondition of a path */
#define LLG_VPI_ROLE_TCHK_REF 5   /* vpiTchkRefTerm tchk term */
#define LLG_VPI_ROLE_TCHK_DATA 6  /* vpiTchkDataTerm tchk term */
#define LLG_VPI_ROLE_ARGUMENT 7   /* another timing check argument (vpiExpr) */
#define LLG_VPI_ROLE_NOTIFIER 8   /* vpiTchkNotifier (also a vpiExpr argument) */
#define LLG_VPI_ROLE_TERM_COND 9  /* a terminal condition, not an argument */

/* Delays of a module path, timing check, primitive or continuous assignment
 * (vpi_get_delays/vpi_put_delays, 38.10, 38.32). `ops` are the runtime
 * values back-annotation writes, in the form llg_sdf_apply takes; an op's
 * `first` is the index of the timing check limit it holds. `declared` holds
 * the values as elaborated, in design-precision ticks (twelve transition
 * delays of a path, the limits of a check, rise/fall/turn-off of a
 * primitive or continuous assignment), read when no op holds them. `count`
 * is the number of delays or limits the source gives (0: none). */
typedef struct {
    const llg_sdf_op_t* ops;
    uint32_t op_count;
    const int64_t* declared;
    uint32_t declared_count;
    uint32_t count;
    int polarity;             /* vpiPolarity of a path */
    int data_polarity;        /* vpiDataPolarity of a path */
    int has_ifnone;           /* vpiModPathHasIfNone */
} llg_vpi_timing_t;

/* One catalog row. Generated models emit a static array of these; the
 * runtime only borrows it and indexes it on the first VPI use. */
typedef struct llg_vpi_model_object {
    int type;                 /* vpiType */
    const char* name;
    const char* full_name;    /* SV 27.4 hierarchical name */
    const char* definition_name;
    const char* file;
    int line;
    uint32_t width;           /* packed width; element width of an array */
    int8_t is_signed;
    int8_t is_real;
    int8_t is_net;
    sv4_t* packed;            /* value, or element 0 of an array */
    double* real;             /* real value, or element 0 of a real array */
    struct llg_vpi_model_object* parent;
    uint64_t time_unit_fs;    /* 0 inherits the parent or simulation time unit. */
    int subtype;              /* array element vpiType, vpiNetType, port vpiDirection */
    uint32_t flags;           /* LLG_VPI_OBJ_* */
    int64_t left;             /* outermost packed range (LLG_VPI_OBJ_RANGE) */
    int64_t right;
    const int64_t* dims;      /* unpacked (left, right) pairs, outermost first */
    uint32_t dim_count;
    uint32_t port_index;
    struct llg_vpi_model_object* target; /* a port's low connection; the interface or
                                          * modport of an interface port */
    llg_net_t* net;           /* resolved net behind `packed`, for force/release */
    const char* constant;     /* parameter: 0/1/x/z digits MSB first, or text */
    double real_constant;     /* real parameter value */
    uint32_t typespec;        /* vpiTypespec: typespec index + 1, 0: none */
    int detail;               /* vpiAlwaysType, vpiPrimType, vpiFuncType, vpiPortType,
                               * or the element vpiType of an instance array */
    llg_event_t* event;       /* named event */
    const llg_vpi_conn_t* conns;
    uint32_t conn_count;
    /* Named block, task or function: the llg_disable_target identity. */
    uint32_t activation_declaration;
    uint32_t activation_instance;
    const llg_vpi_timing_t* timing; /* delays of a path, check, primitive, cont assign */
    /* A dynamic object (LLG_VPI_OBJ_DYNAMIC): its storage (llg_string_t,
     * void* handle or container), the change marker a handle store
     * publishes (NULL for strings and containers, which publish their own),
     * and its shape index + 1 in the dynamic tables. */
    void* storage;
    sv4_t* dependency;
    uint32_t shape;
    uint32_t code;            /* vpiStmt of a process, task or function, the
                               * statement of a named block: code row + 1 */
} llg_vpi_model_object_t;

/* Dynamic objects (IEEE 1800-2009 37.17, 37.27-37.29, 37.3.7): the value
 * shapes of strings, handles, containers and class properties, and the
 * class layouts a class object exposes. Shapes refer to earlier shapes only
 * (container elements). */
enum {
    LLG_VPI_SHAPE_PACKED = 0,    /* sv4_t */
    LLG_VPI_SHAPE_REAL = 1,      /* double */
    LLG_VPI_SHAPE_STRING = 2,    /* llg_string_t */
    LLG_VPI_SHAPE_CLASS = 3,     /* void*: a collected class object */
    LLG_VPI_SHAPE_CHANDLE = 4,   /* void*: a foreign pointer */
    LLG_VPI_SHAPE_CONTAINER = 5, /* dynamic array, queue or associative array */
    LLG_VPI_SHAPE_OTHER = 6,     /* listed without a value */
};
/* Associative array index kinds. */
enum {
    LLG_VPI_KEY_NONE = 0,
    LLG_VPI_KEY_INTEGRAL = 1,
    LLG_VPI_KEY_STRING = 2,
    LLG_VPI_KEY_OTHER = 3,       /* wildcard: elements are not reachable */
};

typedef struct {
    int type;                 /* vpiType of a variable of this shape */
    uint8_t shape;            /* LLG_VPI_SHAPE_* */
    uint8_t array_type;       /* vpiDynamicArray, vpiQueueArray, vpiAssocArray */
    uint8_t values;           /* container elements are llg_value_t, not sv4_t */
    uint8_t key;              /* LLG_VPI_KEY_* */
    uint32_t width;           /* packed width */
    int8_t is_signed;
    int8_t two_state;
    int8_t shortreal;
    uint32_t class_index;     /* class layout + 1 of a class handle; 0: built-in */
    uint32_t element;         /* container element shape */
    uint32_t typespec;        /* typespec index + 1 of a packed value; 0: none */
} llg_vpi_shape_t;

typedef struct {
    const char* name;         /* NULL: a hidden runtime field, not a property */
    uint32_t shape;
} llg_vpi_class_field_t;

typedef struct {
    const char* name;
    uint32_t base;            /* base class layout + 1; 0: none */
    uint32_t first_field;     /* field_count fields, base fields first */
    uint32_t field_count;
} llg_vpi_class_t;

/* Field kinds the generated object accessor reports, matching the model's
 * class field slots: the shape of a property must agree before VPI reads it. */
enum {
    LLG_VPI_FIELD_PACKED = 0,
    LLG_VPI_FIELD_REAL = 1,
    LLG_VPI_FIELD_STRING = 2,
    LLG_VPI_FIELD_HANDLE = 3,
    LLG_VPI_FIELD_PAYLOAD = 4,  /* owned container or record */
};

typedef struct {
    const llg_vpi_shape_t* shapes;
    uint32_t shape_count;
    const llg_vpi_class_t* classes;
    uint32_t class_count;
    const llg_vpi_class_field_t* fields;
    uint32_t field_count;
    /* The class layout of a live object (0 when it was reclaimed). */
    int (*object)(void* handle, uint32_t* class_id);
    /* The storage of field `index` of a live object and its kind. */
    void* (*field)(void* handle, uint32_t index, unsigned* kind);
} llg_vpi_dynamic_tables_t;

/* The dynamic tables of the catalog, borrowed like the catalog and
 * validated on the first VPI use; NULL for a model without them. */
int llg_vpi_model_dynamic(const llg_vpi_dynamic_tables_t* tables);
/* cbCreateObj (38.36.1): models generated with the catalog report every
 * class object after its constructor (or shallow copy) completed. */
void llg_vpi_object_created(void* handle);

/* A built-in system task or function the model runs natively and its first
 * call site (file may be NULL). */
typedef struct {
    const char* name;
    const char* file;
    int line;
} llg_vpi_builtin_call_t;

typedef struct {
    int kind;
    uint32_t width;
    int8_t is_signed;
    int8_t is_real;
    sv4_t packed;
    double real;
} llg_vpi_arg_t;

/* Model metadata is static generated storage; the runtime borrows it until
 * llg_vpi_shutdown, while all public handles remain generation-checked.
 * Initialization only records the table: it is validated and indexed on the
 * first VPI use, so models that never load an application pay nothing. */
int llg_vpi_model_init(const char* design_name,
                       llg_vpi_model_object_t* objects, size_t object_count);
/* The typespec tables the catalog rows name, borrowed like the catalog and
 * validated on the first VPI use; NULL for a model without a catalog. */
int llg_vpi_model_types(const llg_vpi_type_tables_t* tables);
/* The expression and statement tables, borrowed like the catalog and
 * validated on the first VPI use; NULL for a model generated without them. */
int llg_vpi_model_code(const llg_vpi_code_tables_t* tables);
/* How the model was generated for vpi_put_delays: `writable` is 1 when every
 * delay the catalog names is runtime storage (generated for VPI
 * applications), `delay_select` the min:typ:max operand the model uses (0
 * min, 1 typ, 2 max), which a put with mtm values takes. */
void llg_vpi_model_delays(int writable, int delay_select);
int llg_vpi_startup(void);
/* Run a statically linked application's startup table (the model links the
 * archive that defines it) before LLG_VPI_PLUGIN libraries are loaded. */
int llg_vpi_startup_static(void (*const* routines)(void));
int llg_vpi_compile_call(const char* name, const llg_vpi_compile_arg_t* args,
                         int arg_count);
int llg_vpi_compile_calls(const char* const* names,
                          const llg_vpi_compile_arg_t* const* args,
                          const int* arg_counts, int call_count);
void llg_vpi_start_simulation(void);
/* The model has queued its time-zero procedures except always_comb and
 * always_latch (9.2.2.2): the assertion system start callbacks run as the
 * next Active event of time zero (39.4.1, decision A17-D3). */
void llg_vpi_procedures_started(void);
void llg_vpi_end_simulation(void);
void llg_vpi_shutdown(void);
/* Run `hook` once at the next llg_vpi_shutdown (DPI-C scope user data). */
void llg_vpi_on_shutdown(void (*hook)(void));

int llg_vpi_call_task(const char* name, llg_vpi_arg_t* args, int arg_count);
sv4_t llg_vpi_call_function(const char* name, llg_vpi_arg_t* args,
                            int arg_count, uint32_t fallback_width,
                            int8_t fallback_signed);
double llg_vpi_call_real_function(const char* name, llg_vpi_arg_t* args,
                                  int arg_count);
/* Generated calls have stable per-model identities; return shape and scope
 * survive independently of the name-level registration. */
int llg_vpi_compile_call_site(uint64_t site, const char* name,
    const llg_vpi_compile_arg_t* args, int count, uint64_t time_unit_fs);
/* llg_vpi_compile_call_site with the call's scope row (vpiScope), source
 * file and line. Site identities are dense from 0 in generated models. */
int llg_vpi_compile_call_site_at(uint64_t site, const char* name,
    const llg_vpi_compile_arg_t* args, int count, uint64_t time_unit_fs,
    llg_vpi_model_object_t* scope, const char* file, int line);
int llg_vpi_call_task_site(uint64_t site, const char* name, llg_vpi_arg_t* args, int count);
sv4_t llg_vpi_call_function_site(uint64_t site, const char* name, llg_vpi_arg_t* args,
    int count, uint32_t fallback_width, int8_t fallback_signed);
double llg_vpi_call_real_function_site(uint64_t site, const char* name, llg_vpi_arg_t* args, int count);
int llg_vpi_failed(void);
/* After startup: fail, naming the call site, when a VPI application
 * registered one of the `count` built-ins in `calls`. llg runs built-ins
 * natively and cannot let a registration override them (IEEE 1800-2009
 * 36.3.2). Returns 1 when none is registered. */
int llg_vpi_check_builtin_overrides(const llg_vpi_builtin_call_t* calls, uint32_t count);

#ifdef __cplusplus
}
#endif

/* Destination-passing forms of the packed-returning calls above, used by
 * generated code (see value/destinations.h). `X_to(dst, ...)` replaces the
 * initialized owner at dst with exactly the result `X(...)` would return. Packed
 * arguments are borrowed by address; string arguments are consumed through
 * their address and left empty, like the returning forms' by-value strings. */
void llg_vpi_call_function_to(sv4_t* dst, const char* name, llg_vpi_arg_t* args, int arg_count, uint32_t fallback_width, int8_t fallback_signed);
void llg_vpi_call_function_site_to(sv4_t* dst, uint64_t site, const char* name, llg_vpi_arg_t* args, int count, uint32_t fallback_width, int8_t fallback_signed);

/* The DPI-C bridge. Generated models receive it in the same header, ahead of
 * this text (so the guard skips the include); in-tree users find the file. */
#ifndef LLG_DPI_H
#include "llg_dpi.h"
#endif

#endif /* LLG_VPI_H */
