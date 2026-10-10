// llg_dpi.h — generated-model bridge for DPI-C imports (SV Annex H).
//
// Embedded after llg_vpi.h. Generated thunks convert each packed or unpacked
// argument between the model's packed payload and a foreign buffer that the
// thunk owns for the call, so foreign code never sees simulator storage
// (H.6.7). The svdpi.h routines themselves are defined with the VPI bridge.
#ifndef LLG_DPI_H
#define LLG_DPI_H

#include <stddef.h>
#include <stdint.h>

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

/* An imported task returned nonzero although nothing disabled it (SV 35.9
 * item b): report a fatal simulation error naming `c_name` and request
 * completion. The caller copies nothing out and returns. */
void llg_dpi_task_protocol_error(const char* c_name);

#ifdef __cplusplus
}
#endif

#endif /* LLG_DPI_H */
