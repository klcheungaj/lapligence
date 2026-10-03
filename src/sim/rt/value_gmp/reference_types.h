#ifndef LLG_SV4_COMPACT_REFERENCE_TYPES_H
#define LLG_SV4_COMPACT_REFERENCE_TYPES_H
/* Owner-free descriptors mirror llg_value.h. Their containing call scopes,
 * retained cells and publication behavior belong to the runtime facade. */
#ifndef LLG_GMP_SV4_SELECT_PLAN_DEFINED
#define LLG_GMP_SV4_SELECT_PLAN_DEFINED
typedef struct {
    uint32_t storage_width;
    uint32_t width;
    uint32_t storage_lsb;
    uint32_t value_lsb;
    uint32_t count;
} llg_gmp_sv4_select_plan_t;

#endif

typedef struct llg_queue_t llg_queue_t;
typedef g4_t (*llg_gmp_queue_ref_read_fn)(const llg_queue_t* queue,
                                       uint64_t identity);
typedef int (*llg_gmp_queue_ref_write_fn)(llg_queue_t* queue, uint64_t identity,
                                      g4_t value);

// Canonical lvalue descriptor used by subroutine `ref` arguments.  The
// descriptor always names the original packed storage (`base`); selected
// aliases retain their source bounds so reads and writes remain immediate and
// do not require copy-in/copy-out temporaries.
typedef enum {
    LLG_GMP_REF_WHOLE = 0,
    LLG_GMP_REF_BIT = 1,
    LLG_GMP_REF_PART = 2,
    LLG_GMP_REF_INDEXED = 3,
    LLG_GMP_REF_ARRAY = 4,
    LLG_GMP_REF_QUEUE = 5,
    // Synchronous file-input target; retained borrows a llg_gmp_sv4_select_plan_t.
    // Neither the descriptor nor its plan may escape the input call.
    LLG_GMP_REF_PACKED_PLAN = 6,
    LLG_GMP_REF_COMPOSITE = 7,
    LLG_GMP_REF_VIEW = 8,
    // A selected tagged-union member with captured receiver plans for every
    // active-tag check. Call scopes own the view and check array.
    LLG_GMP_REF_TAGGED_VIEW = 9,
} llg_gmp_ref_kind_t;

typedef struct {
    g4_t* base;
    llg_queue_t* queue;
    uint32_t width;
    int8_t is_signed;
    uint8_t two_state;
    uint8_t kind;
    int64_t left;
    int64_t right;
    uint64_t index;
    uint32_t indexed_width;
    uint8_t indexed_negative;
    uint64_t array_size;
    uint64_t queue_identity;
    llg_gmp_queue_ref_read_fn queue_read;
    llg_gmp_queue_ref_write_fn queue_write;
    void* retained;
    g4_t (*retained_read)(const void*);
    int (*retained_write)(void*, g4_t);
} llg_gmp_ref_t;

/* Borrowed descriptor graphs. Generated call scopes own the graph storage;
 * leaves retain their original variable identity across calls and suspension. */
typedef struct {
    size_t count;
    llg_gmp_ref_t** parts;
} llg_gmp_ref_composite_t;

typedef struct {
    llg_gmp_sv4_select_plan_t receiver_plan;
    uint32_t tag_width;
    uint32_t member_index;
    const char* member_name;
} llg_gmp_ref_tag_check_t;

typedef struct {
    llg_gmp_ref_t* parent;
    llg_gmp_sv4_select_plan_t plan;
    size_t tag_check_count;
    const llg_gmp_ref_tag_check_t* tag_checks;
    const char* location;
} llg_gmp_ref_view_t;


#endif
