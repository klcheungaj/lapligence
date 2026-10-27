// llg_dpi.h — generated-model bridge for DPI-C imports (SV Annex H).
//
// Embedded ahead of llg_vpi.h in the generated bridge header. Generated
// thunks convert each packed or unpacked argument between the model's packed
// payload and a foreign buffer that the thunk owns for the call, so foreign
// code never sees simulator storage (H.6.7). The svdpi.h routines themselves are defined with the VPI bridge.
#ifndef LLG_DPI_H
#define LLG_DPI_H

#include <setjmp.h>
#include <stddef.h>
#include <stdint.h>

#include "llg_rt.h"

#if defined(_MSC_VER)
/* Import thunks with open-array formals call setjmp (llg_dpi_guard_t);
 * generated models contain no C++ objects whose destruction it could skip. */
#pragma warning(disable : 4611)
#endif

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
 * and gets the outer state back when each call returns. A chain whose
 * exported task consumes time runs its foreign code on a foreign call
 * context (below). */

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

/* The foreign call of an import with open-array formals runs under a guard
 * on the thunk's C stack. A fatal svdpi.h access in that foreign code
 * (S40-D6) abandons it: the runtime pops the innermost guard and jumps back
 * to its thunk, which restores the chain state, releases its value scopes
 * from `mark` and returns without copying anything out. The guards of one
 * native stack nest like its calls; each foreign call context keeps its own
 * guard stack (llg_dpi_context_switch). */
typedef struct llg_dpi_guard {
    jmp_buf jump;
    struct llg_dpi_guard* outer;
    llg_value_scope_t* mark;
} llg_dpi_guard_t;
void llg_dpi_guard_push(llg_dpi_guard_t* guard);
void llg_dpi_guard_pop(llg_dpi_guard_t* guard);
/* Restore the chain state saved by llg_dpi_import_enter after the foreign
 * code was abandoned; no protocol check applies. */
void llg_dpi_import_abort(const llg_dpi_call_t* saved);

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
/* The import's process was killed while its foreign code waited in an
 * export (ADV-018): restore the chain state and check that the task
 * returned 1 (35.9 item b). Nothing is copied out and the value scopes are
 * left to the process's cleanup. */
void llg_dpi_import_abandoned(const llg_dpi_call_t* saved, int status, const char* c_name);
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
/* Restore the chain state of an export whose calling process was killed
 * while the exported task waited, and return 1 (35.9 item a). */
int llg_dpi_export_abandoned(const llg_dpi_call_t* saved);

/* ---- Foreign call contexts (ADV-018; foreign_contexts/readme.md) ----
 *
 * A context imported task of a design with an exported task that can
 * consume time is a coroutine shim: its thunk body runs on a context, an
 * OS thread with its own native stack, while a run token keeps exactly one
 * thread running. When the foreign code calls such an exported task, the
 * export thunk pushes the task's coroutine frame on the calling chain's
 * arena and hands the token back; the shim runs the frame as an arena call
 * on the scheduler thread, so the task's waits suspend the process like any
 * task call, and then resumes the context. These entry points are defined
 * in llg_dpi_context.c, which only models with such shims link. */
typedef struct llg_dpi_context llg_dpi_context_t;
/* The thunk body a context runs, given the shim's frame. */
typedef void (*llg_dpi_body_fn)(void* frame);
/* Take a context from the pool (or start one) for one call of the shim
 * running in `chain`. NULL when no thread can be started: the shim then
 * runs `body` directly and a waiting export is refused. */
llg_dpi_context_t* llg_dpi_context_begin(llg_co_chain_t* chain, llg_dpi_body_fn body, void* frame);
/* Hand the token to `context` until it returns it. Returns the anchor of an
 * exported task frame to run, or NULL once the body returned (or the
 * context was parked for good after a fatal protocol violation). */
llg_co_anchor_t* llg_dpi_context_run(llg_dpi_context_t* context);
/* The body returned: give the context back to the pool. */
void llg_dpi_context_end(llg_dpi_context_t* context);
/* An export thunk of an exported task that can consume time: push the
 * task's frame (`desc`) on the calling chain's arena. NULL, after a fatal
 * error, when the caller is not a context (an imported function, or a
 * context import that cannot suspend; SV 35.8). */
llg_co_anchor_t* llg_dpi_export_begin(const llg_co_desc_t* desc, const char* c_name);
/* The frame holds the arguments: hand the token back until the task
 * returned. Nonzero when the calling process was killed meanwhile (the
 * frame is gone; the thunk returns llg_dpi_export_abandoned). */
int llg_dpi_export_wait(llg_co_anchor_t* slot);
/* Nonzero while the token holder unwinds a killed process's chain. */
int llg_dpi_context_abandoned(void);
/* Hooks between llg_dpi.c and llg_dpi_context.c (contexts only). */
void llg_dpi_context_hooks(void (*park)(void), void (*shutdown)(void));
llg_dpi_guard_t* llg_dpi_context_switch(llg_dpi_guard_t* guards, int unwinding);
void llg_dpi_chain_snapshot(llg_dpi_call_t* out);

#ifdef __cplusplus
}
#endif

#endif /* LLG_DPI_H */
