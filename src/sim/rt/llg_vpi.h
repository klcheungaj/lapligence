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
    struct llg_vpi_model_object* target; /* a port's low connection */
    llg_net_t* net;           /* resolved net behind `packed`, for force/release */
    const char* constant;     /* parameter: 0/1/x/z digits MSB first, or text */
    double real_constant;     /* real parameter value */
    uint32_t typespec;        /* vpiTypespec: typespec index + 1, 0: none */
} llg_vpi_model_object_t;

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
} llg_vpi_compile_arg_t;

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
