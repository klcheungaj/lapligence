// llg_dpi.h — generated-model bridge for DPI-C imports (SV Annex H).
//
// Embedded ahead of llg_vpi.h in the generated bridge header. Generated
// thunks convert each packed or unpacked argument between the model's packed
// payload and a foreign buffer that the thunk owns for the call, so foreign
// code never sees simulator storage (H.6.7). The svdpi.h routines themselves are defined with the VPI bridge.
#ifndef LLG_DPI_H
#define LLG_DPI_H

#include <stddef.h>
#include <stdint.h>

#include "llg_rt.h"

#ifdef __cplusplus
extern "C" {
#endif

/* Foreign representation of one DPI value. */
enum {
    LLG_DPI_BIT = 1,      /* svBit code */
    LLG_DPI_LOGIC = 2,    /* svLogic code */
    LLG_DPI_INT = 3,      /* C integer of `size` bytes */
    LLG_DPI_BITVEC = 4,   /* svBitVecVal chunks (H.7.7) */
    LLG_DPI_LOGICVEC = 5, /* svLogicVecVal chunks */
    LLG_DPI_ARRAY = 6,    /* one unpacked dimension, natural order (H.7.6) */
    LLG_DPI_STRUCT = 7    /* unpacked structure, C compiler layout (H.7.8) */
};

typedef struct llg_dpi_member llg_dpi_member_t;

/* Immutable layout table emitted beside each thunk. `width` is the bit count
 * of one value in the packed payload; `size` its C size, taken from `sizeof`
 * of a matching generated C type so the C compiler's layout decides. */
typedef struct llg_dpi_type {
    uint8_t kind;
    uint32_t width;
    size_t size;
    int32_t left, right;            /* LLG_DPI_ARRAY: declared range */
    size_t count;                   /* ARRAY elements or STRUCT members */
    const struct llg_dpi_type* element; /* LLG_DPI_ARRAY */
    const llg_dpi_member_t* members;    /* LLG_DPI_STRUCT, declaration order */
} llg_dpi_type_t;

struct llg_dpi_member {
    size_t offset;
    const llg_dpi_type_t* type;
};

/* The object behind an svOpenArrayHandle (H.12): `dims` unpacked
 * dimensions as LLG_DPI_ARRAY levels of `type`, the element below them, and
 * the thunk-owned C-layout buffer. Valid only during the import call. */
typedef struct llg_dpi_open {
    const llg_dpi_type_t* type;
    int dims;
    void* data;
} llg_dpi_open_t;

/* Canonical packed vectors: `count` 32-bit chunks, least significant first.
 * Exports zero unused bits; imports into an owner already shaped to the
 * formal's width and ignore bits beyond it (H.7.7). */
void llg_dpi_export_bits(const sv4_t* value, uint32_t* out, size_t count);
void llg_dpi_export_logic(const sv4_t* value, void* out, size_t count);
void llg_dpi_import_bits(sv4_t* value, const uint32_t* in, size_t count);
void llg_dpi_import_logic(sv4_t* value, const void* in, size_t count);
/* Fill `count` svLogicVecVal chunks with X: the initial value of a 4-state
 * output vector. */
void llg_dpi_fill_x(void* out, size_t count);

/* Aggregates: payload <-> C layout of `type`. `llg_dpi_init_c` gives an
 * output buffer the type's default (X for 4-state leaves, 0 otherwise). */
void llg_dpi_to_c(const llg_dpi_type_t* type, const sv4_t* payload, void* out);
void llg_dpi_from_c(const llg_dpi_type_t* type, const void* in, sv4_t* payload);
void llg_dpi_init_c(const llg_dpi_type_t* type, void* out);

/* ---- Context call chains (SV 35.5.3, Annex H.9) ----
 *
 * An svScope points at one llg_dpi_scope_t of the table the model passes to
 * llg_dpi_model_init: an instantiated scope holding DPI-C import or export
 * declarations. Generated thunks save, set and restore the call-chain state
 * on every crossing between SystemVerilog and foreign code, so a nested
 * SV -> C -> SV -> C chain sees the scope of its innermost context import
 * and gets the outer state back when each call returns. Nothing suspends
 * inside a chain: exported tasks are zero-time. */

/* Entry of one exported subroutine in a scope: `id` numbers the exported C
 * identifier model-wide, `fn` is that scope's export thunk (cast to its
 * Annex H prototype by the C entry point of the identifier). */
typedef void (*llg_dpi_fn_t)(void);
typedef struct llg_dpi_export_slot {
    uint32_t id;
    llg_dpi_fn_t fn;
} llg_dpi_export_slot_t;

typedef struct llg_dpi_user llg_dpi_user_t;

typedef struct llg_dpi_scope {
    const char* name;                     /* fully qualified scope name */
    const llg_dpi_export_slot_t* exports; /* ascending `id` */
    uint32_t export_count;
    llg_dpi_user_t* user;                 /* svPutUserData entries */
} llg_dpi_scope_t;

/* Source position of an import call (svGetCallerInfo, H.9.3). */
typedef struct llg_dpi_site {
    const char* file;
    int line;
} llg_dpi_site_t;

/* Call-chain state saved by one crossing, on the C stack of its thunk. */
typedef struct llg_dpi_call {
    llg_dpi_scope_t* scope;
    const llg_dpi_site_t* site;
    int depth;
    uint8_t chain;
    uint8_t acked;
} llg_dpi_call_t;

/* Set by an import's call site just before the call and consumed by the
 * import thunk (cleared after the call when the model keeps no call-chain
 * state). */
extern const llg_dpi_site_t* llg_dpi_next_site;

/* Register the model's scope table, sorted by strcmp of `name`. */
void llg_dpi_model_init(llg_dpi_scope_t* scopes, size_t count);
/* Release svPutUserData entries and forget the table. */
void llg_dpi_shutdown(void);

/* SV -> C: an import thunk saves the chain state in `saved` and enters a
 * context call (`scope` its declaring scope) or, with `scope` NULL, a
 * noncontext call of an imported task (`task` 1) or function. `depth` is
 * the thunk's recursion depth. */
void llg_dpi_import_enter(llg_dpi_call_t* saved, llg_dpi_scope_t* scope, int task, int depth);
/* The import returned: restore the chain state. For a function (`task`
 * 0), a disable that the foreign code did not acknowledge is a fatal
 * error (35.9 item c). */
void llg_dpi_import_leave(const llg_dpi_call_t* saved, const char* c_name, int task);
/* Check an imported task's disable-protocol result (35.9 item b) and
 * return nonzero when its outputs must not be copied out (it was disabled
 * or broke the protocol, a fatal error). */
int llg_dpi_task_status(int status, const char* c_name);

/* C -> SV: the entry point of exported C identifier `id` (a task when
 * `task` is 1) finds the export thunk of the current context scope. Calls
 * from outside a context import chain, after the calling import was
 * disabled (35.9 item d) or into a scope that does not export `c_name` are
 * fatal errors that return NULL. A task called from an imported function
 * runs (decision S41-D8). */
llg_dpi_fn_t llg_dpi_export_target(uint32_t id, const char* c_name, int task);
/* An export thunk saves the chain state and returns the recursion depth of
 * the SystemVerilog call it makes. */
int llg_dpi_export_enter(llg_dpi_call_t* saved);
/* Restore the chain state; for an exported task return the disable status
 * of 35.9 item a) (1 when a disable now cancels the calling import). */
int llg_dpi_export_leave(const llg_dpi_call_t* saved, int task);

#ifdef __cplusplus
}
#endif

#endif /* LLG_DPI_H */
