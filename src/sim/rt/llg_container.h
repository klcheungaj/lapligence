// llg_container.h -- scheduler-independent unpacked container storage.
#ifndef LLG_CONTAINER_H
#define LLG_CONTAINER_H

#include "llg_value.h"
#include "llg_string.h"

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

enum {
    LLG_CONTAINER_REDUCE_SUM = 0,
    LLG_CONTAINER_REDUCE_PRODUCT = 1,
    LLG_CONTAINER_REDUCE_AND = 2,
    LLG_CONTAINER_REDUCE_OR = 3,
    LLG_CONTAINER_REDUCE_XOR = 4,
};

typedef void (*llg_container_notify_fn)(sv4_t* contents, sv4_t* shape,
                                        int change);

/* Packed inputs/indices/keys are borrowed for the call. Packed return values
 * are independent owners; destroy or move them. Init/destroy delimit each
 * container lifetime; setters deep-copy and replacements destroy old elements.
 * Pinned references own detached cells after removal and release them on the
 * final reference drop. A borrowed interior pointer cannot survive mutation.
 * Recursive value descriptors use llg_value_copy/llg_value_drop, never memcpy. */

/* Evaluate one packed array element for an array-method `with` clause. The
 * callback borrows the source element/index and receives an initialized empty
 * `out`. It must write an independent owner; the caller destroys that owner.
 * `context` is borrowed for the callback. */
typedef void (*llg_container_eval_fn)(sv4_t* out, sv4_t item, sv4_t index,
                                      void* context);

enum {
    LLG_VALUE_PACKED = 0,
    LLG_VALUE_REAL = 1,
    LLG_VALUE_STRING = 2,
    LLG_VALUE_CHANDLE = 3,
    LLG_VALUE_EVENT = 4,
    LLG_VALUE_AGGREGATE = 5,
    LLG_VALUE_FIXED_ARRAY = 6,
    LLG_VALUE_CONTAINER = 7,
    LLG_VALUE_OPAQUE = 8,
    /* A `process` handle: an identity whose element owns one reference. */
    LLG_VALUE_PROCESS = 9,
};

typedef struct llg_value_desc_t llg_value_desc_t;
typedef struct llg_value_member_desc_t llg_value_member_desc_t;
typedef struct llg_dyn_value_array_t llg_dyn_value_array_t;
typedef struct llg_queue_value_array_t llg_queue_value_array_t;
typedef struct llg_assoc_value_t llg_assoc_value_t;
typedef struct llg_value_t llg_value_t;
struct llg_queue_source_t;

struct llg_value_member_desc_t {
    const llg_value_desc_t* value;
};

struct llg_value_desc_t {
    uint8_t kind;
    uint64_t type_id;
    uint32_t packed_width;
    int8_t packed_signed;
    uint8_t packed_two_state;
    uint8_t real_short;
    size_t item_count;
    const llg_value_desc_t* element;
    const llg_value_member_desc_t* members;
    size_t member_count;
};

struct llg_value_t {
    const llg_value_desc_t* desc;
    union {
        sv4_t packed;
        double real;
        llg_string_t string;
        void* handle;
        llg_dyn_value_array_t* container;
        llg_value_t* items;
    } value;
};

/* Descriptor contract (SIM-003). A value descriptor is immutable static data
 * describing one recursive value type. Its storage identity decides copying:
 * DEEP values (packed, real, string, aggregate, fixed array, container) copy
 * into independent owners; IDENTITY handles (event, class/semaphore/mailbox/
 * virtual-interface opaque handles and counted process handles) share the
 * referenced object and are reported by llg_value_trace; BORROWED chandles
 * copy the foreign pointer and are neither traced nor freed by the value
 * runtime. */
enum {
    LLG_VALUE_COPY_DEEP = 0,
    LLG_VALUE_COPY_IDENTITY = 1,
    LLG_VALUE_COPY_BORROWED = 2,
};

/* Identity-handle lifecycle hooks (SIM-006), installed by the scheduler at
 * runtime initialization. `event_new` returns a fresh
 * synchronization object owned by the scheduler until model close: SV Table
 * 6-7 makes a newly created event element (new[], default construction)
 * refer to a new event, while a missing or invalid element reads null
 * (Table 7-1). With no hooks installed new event elements are null and
 * process references are not counted. */
typedef struct llg_value_handle_hooks_t {
    void* (*event_new)(void);
    /* Reference counting for LLG_VALUE_PROCESS handles; every value holding
     * a non-null process handle owns one reference. */
    void (*retain)(void* handle);
    void (*release)(void* handle);
} llg_value_handle_hooks_t;
void llg_value_set_handle_hooks(const llg_value_handle_hooks_t* hooks);

/* Maximum descriptor nesting accepted by validation; deeper (or cyclic)
 * descriptors are rejected rather than recursed into. */
#define LLG_VALUE_DESC_MAX_DEPTH 64

/* Return the LLG_VALUE_COPY_* policy of `desc`, or -1 for an invalid kind. */
int llg_value_desc_copy_policy(const llg_value_desc_t* desc);
/* Structural validation: known kinds, nonzero nominal aggregate identities,
 * member/item counts that agree and fit host allocation, packed widths below
 * LLG_SUPPORTED_WIDTH_LIMIT, and no cycle or nesting beyond
 * LLG_VALUE_DESC_MAX_DEPTH. Returns 1 when valid; has no side effects. */
int llg_value_desc_valid(const llg_value_desc_t* desc);
/* Fatal diagnostic naming `label` when `desc` is invalid. Generated models
 * check each emitted root descriptor once at startup. */
void llg_value_desc_check(const llg_value_desc_t* desc, const char* label);

/* Visit every non-null identity-handle slot reachable from `value`
 * (aggregate members, fixed-array items and nested dynamic-array elements).
 * Borrowed chandles and value payloads are not visited. The slot pointer is
 * borrowed for the callback, which must not mutate the traversed value. */
typedef void (*llg_value_visit_fn)(void* const* slot,
                                   const llg_value_desc_t* desc,
                                   void* context);
void llg_value_trace(const llg_value_t* value, llg_value_visit_fn visit,
                     void* context);

/* Descriptor-backed native aggregate storage (subroutine formals, results and
 * locals). `init` default-constructs an empty (zeroed) value: packed leaves
 * take their state-domain default, strings are empty and handles null.
 * `destroy` releases a value constructed by init/copy and leaves it empty; its
 * `void*` signature matches registered value-scope object destructors.
 * `copy` replaces `dst` with a converted deep copy of `src` (which may alias
 * `dst`); `try_copy` returns 0 on an item allocation failure and then leaves
 * `dst` unchanged and releases the partial copy, while `copy` reports it as a
 * fatal error. Leaf payload allocators keep their own fatal OOM policy. */
void llg_native_value_init(llg_value_t* value, const llg_value_desc_t* desc);
void llg_native_value_destroy(void* value);
int llg_native_value_try_copy(llg_value_t* dst, const llg_value_t* src);
void llg_native_value_copy(llg_value_t* dst, const llg_value_t* src);

/* Explicit native roots. Every live descriptor-backed value owned by model
 * storage, an activation scope or a call temporary is one registered root, so
 * a collector can enumerate the identity handles they keep reachable without
 * scanning C stacks. `value` is the first member: a root is addressed by its
 * value. init default-constructs and links the root; destroy releases the
 * value and unlinks it (registered value-scope objects use it as their
 * destructor, so lexical exit, cancellation and model close all unregister).
 * Destroying an unlinked root is a no-op. The registry is per process and
 * single-threaded, like the scheduler. */
typedef struct llg_native_root_t {
    llg_value_t value;
    struct llg_native_root_t* prev;
    struct llg_native_root_t* next;
} llg_native_root_t;

void llg_native_root_init(llg_native_root_t* root, const llg_value_desc_t* desc);
void llg_native_root_destroy(void* root);
/* Number of live roots; zero after a model closes without native leaks. */
size_t llg_native_roots_count(void);
/* Trace the identity handles of every live root (see llg_value_trace). */
void llg_native_roots_trace(llg_value_visit_fn visit, void* context);

struct llg_dyn_value_array_t {
    llg_value_t* data;
    size_t size;
    const llg_value_desc_t* element;
    sv4_t* contents_dependency;
    sv4_t* shape_dependency;
    llg_container_notify_fn notify;
};

void llg_dyn_value_init(llg_dyn_value_array_t* array,
                        const llg_value_desc_t* element);
void llg_dyn_value_destroy(llg_dyn_value_array_t* array);
void llg_dyn_value_delete(llg_dyn_value_array_t* array);
void llg_dyn_value_copy(llg_dyn_value_array_t* dst,
                        const llg_dyn_value_array_t* src);
void llg_dyn_value_new(llg_dyn_value_array_t* dst, sv4_t size,
                       const llg_dyn_value_array_t* initializer);
void llg_dyn_value_assign_reals(llg_dyn_value_array_t* dst,
                                const double* values, size_t count);
void llg_dyn_value_assign_strings(llg_dyn_value_array_t* dst,
                                  llg_string_t* values, size_t count);
void llg_dyn_value_assign_chandles(llg_dyn_value_array_t* dst,
                                   void* const* values, size_t count);
size_t llg_dyn_value_size(const llg_dyn_value_array_t* array);
double llg_dyn_value_get_real(const llg_dyn_value_array_t* array, sv4_t index);
llg_string_t llg_dyn_value_get_string(const llg_dyn_value_array_t* array,
                                      sv4_t index);
void* llg_dyn_value_get_chandle(const llg_dyn_value_array_t* array,
                                sv4_t index);
sv4_t llg_dyn_value_get_nested(const llg_dyn_value_array_t* array,
                               const sv4_t* indices, size_t count);
double llg_dyn_value_get_nested_real(const llg_dyn_value_array_t* array,
                                     const sv4_t* indices, size_t count);
llg_string_t llg_dyn_value_get_nested_string(
    const llg_dyn_value_array_t* array, const sv4_t* indices, size_t count);
void* llg_dyn_value_get_nested_chandle(
    const llg_dyn_value_array_t* array, const sv4_t* indices, size_t count);
int llg_dyn_value_set_real(llg_dyn_value_array_t* array, sv4_t index,
                           double value);
int llg_dyn_value_set_string(llg_dyn_value_array_t* array, sv4_t index,
                             llg_string_t value);
int llg_dyn_value_set_chandle(llg_dyn_value_array_t* array, sv4_t index,
                              void* value);
int llg_dyn_value_set_nested(llg_dyn_value_array_t* array,
                             const sv4_t* indices, size_t count, sv4_t value);
int llg_dyn_value_set_nested_real(llg_dyn_value_array_t* array,
                                  const sv4_t* indices, size_t count,
                                  double value);
int llg_dyn_value_set_nested_string(llg_dyn_value_array_t* array,
                                    const sv4_t* indices, size_t count,
                                    llg_string_t value);
int llg_dyn_value_set_nested_chandle(llg_dyn_value_array_t* array,
                                     const sv4_t* indices, size_t count,
                                     void* value);
int llg_dyn_value_set_nested_container(
    llg_dyn_value_array_t* array, const sv4_t* indices, size_t count,
    const llg_dyn_value_array_t* source);

enum {
    LLG_CONTAINER_CHANGED_CONTENTS = 1,
    LLG_CONTAINER_CHANGED_SHAPE = 2,
};

/* Runtime selectors used by streaming concatenations.  NONE streams the
 * whole container; INDEX, RANGE, and INDEXED_PLUS/MINUS use the first and
 * second packed arguments as their source-language bounds. */
enum {
    LLG_STREAM_SELECTOR_NONE = 0,
    LLG_STREAM_SELECTOR_INDEX = 1,
    LLG_STREAM_SELECTOR_RANGE = 2,
    LLG_STREAM_SELECTOR_INDEXED_PLUS = 3,
    LLG_STREAM_SELECTOR_INDEXED_MINUS = 4,
};

uint32_t llg_stream_selector_width(int selector_kind, sv4_t first,
                                   sv4_t second, uint32_t element_width);

/* Fixed-unpacked-array streaming targets resolve their runtime `with`
 * selector in the generated translation unit, which cannot see the static
 * selector helpers above. `llg_fixed_stream_bounds` returns the requested
 * logical index range and element count; `llg_fixed_stream_index_at` maps a
 * stream offset to the logical index at that offset. Preflight source size
 * before publishing staged writes. A false `target_in_bounds` result requires
 * an error AND writes to the in-range elements, not a silent clipped success.
 * Operands are borrowed; these helpers neither allocate nor publish writes. */
void llg_fixed_stream_bounds(int selector_kind, sv4_t first, sv4_t second,
                             int64_t declaration_left, int64_t declaration_right,
                             int64_t* left, int64_t* right, size_t* count);
uint32_t llg_fixed_stream_width(int selector_kind, sv4_t first, sv4_t second,
                                uint32_t element_width);
void llg_stream_require_bits(int64_t available, uint32_t required);
int llg_fixed_stream_target_in_bounds(int64_t declaration_left,
                                      int64_t declaration_right,
                                      int64_t left, int64_t right, size_t count);
int64_t llg_fixed_stream_index_at(int64_t left, int64_t right, size_t offset);
/* Pack the selected fixed-array elements into one value in stream order. The
 * declared bounds map a logical index to storage order; out-of-range elements
 * contribute `fallback`, the element's default-uninitialized value, and the
 * selector bounds are independent of the array size. */
sv4_t llg_fixed_stream_source(const sv4_t* values, int64_t declaration_left,
                              int64_t declaration_right, uint32_t element_width,
                              sv4_t fallback, int selector_kind,
                              sv4_t first, sv4_t second);
/* Fixed-array `with` helpers for arrays without model storage, represented
 * by their whole declaration-order image (left declared element in the MSBs).
 * `storage_offset` returns -1 outside the bounds. `image_stream_source`
 * packs the selection like `llg_fixed_stream_source`; `image_stream_scatter`
 * writes the leading selected elements of `segment` into the in-bounds
 * positions of `image`; `image_element_lsb` locates one in-bounds element.
 * Packed operands are borrowed; only `image` is modified. */
/* Consume `bits` from the left of an unpack source, then reorder them by the
 * stream operator. A wider source keeps its leftmost bits (SV 11.4.14.3);
 * a narrower one is a fatal error. Borrows `value`. */
sv4_t llg_stream_unpack_source(sv4_t value, uint64_t bits, uint32_t slice,
                               int right_to_left);
int64_t llg_fixed_stream_storage_offset(int64_t declaration_left,
                                        int64_t declaration_right,
                                        int64_t logical);
sv4_t llg_fixed_image_stream_source(sv4_t image, int64_t declaration_left,
                                    int64_t declaration_right,
                                    uint32_t element_width, sv4_t fallback,
                                    int selector_kind, sv4_t first,
                                    sv4_t second);
void llg_fixed_image_stream_scatter(sv4_t* image, sv4_t segment,
                                    int64_t declaration_left,
                                    int64_t declaration_right,
                                    uint32_t element_width, int64_t left,
                                    int64_t right, size_t count);
int64_t llg_fixed_image_element_lsb(int64_t declaration_left,
                                    int64_t declaration_right, int64_t logical,
                                    uint32_t element_width);
/* Assign a runtime-sized streaming concatenation to a fixed-size bit-stream
 * target: the stream is left-aligned and zero-filled on the right, and a
 * stream larger than the target is an error (IEEE 1800-2009 11.4.14). The
 * source is borrowed; the returned value is owned by the caller. */
sv4_t llg_stream_to_fixed(sv4_t value, uint32_t width, int is_signed);

enum {
    LLG_CONTAINER_METHOD_FIND = 0,
    LLG_CONTAINER_METHOD_FIND_INDEX = 1,
    LLG_CONTAINER_METHOD_FIND_FIRST = 2,
    LLG_CONTAINER_METHOD_FIND_FIRST_INDEX = 3,
    LLG_CONTAINER_METHOD_FIND_LAST = 4,
    LLG_CONTAINER_METHOD_FIND_LAST_INDEX = 5,
    LLG_CONTAINER_METHOD_MIN = 6,
    LLG_CONTAINER_METHOD_MAX = 7,
    LLG_CONTAINER_METHOD_UNIQUE = 8,
    LLG_CONTAINER_METHOD_UNIQUE_INDEX = 9,
    LLG_CONTAINER_METHOD_SORT = 10,
    LLG_CONTAINER_METHOD_RSORT = 11,
    LLG_CONTAINER_METHOD_REVERSE = 12,
    LLG_CONTAINER_METHOD_SHUFFLE = 13,
};

typedef struct {
    sv4_t* data;
    size_t size;
    uint32_t element_width;
    int8_t element_signed;
    uint8_t element_two_state;
    sv4_t* contents_dependency;
    sv4_t* shape_dependency;
    llg_container_notify_fn notify;
} llg_dyn_array_t;

int llg_dyn_value_set_nested_container_from_packed(
    llg_dyn_value_array_t* array, const sv4_t* indices, size_t count,
    const llg_dyn_array_t* source);

/* Queue storage for recursive/non-packed elements.  Packed queues use the
 * inline llg_queue_t representation below. */
struct llg_queue_value_array_t {
    llg_value_t* data;
    size_t size;
    size_t capacity;
    size_t limit;
    const llg_value_desc_t* element;
    sv4_t* contents_dependency;
    sv4_t* shape_dependency;
    llg_container_notify_fn notify;
    uint64_t mutation_epoch;
};

void llg_queue_value_init(llg_queue_value_array_t* queue,
                          const llg_value_desc_t* element,
                          uint64_t maximum_elements);
void llg_queue_value_destroy(llg_queue_value_array_t* queue);
void llg_queue_value_delete(llg_queue_value_array_t* queue);
void llg_queue_value_copy(llg_queue_value_array_t* dst,
                          const llg_queue_value_array_t* src);
void llg_queue_value_assign_reals(llg_queue_value_array_t* dst,
                                  const double* values, size_t count);
void llg_queue_value_assign_strings(llg_queue_value_array_t* dst,
                                    llg_string_t* values, size_t count);
void llg_queue_value_assign_chandles(llg_queue_value_array_t* dst,
                                     void* const* values, size_t count);
void llg_queue_value_assign_sources(llg_queue_value_array_t* dst,
                                    const struct llg_queue_source_t* sources,
                                    size_t source_count);
size_t llg_queue_value_size(const llg_queue_value_array_t* queue);
sv4_t llg_queue_value_get(const llg_queue_value_array_t* queue, sv4_t index);
double llg_queue_value_get_real(const llg_queue_value_array_t* queue,
                                sv4_t index);
llg_string_t llg_queue_value_get_string(const llg_queue_value_array_t* queue,
                                        sv4_t index);
void* llg_queue_value_get_chandle(const llg_queue_value_array_t* queue,
                                  sv4_t index);
sv4_t llg_queue_value_get_nested(const llg_queue_value_array_t* queue,
                                 const sv4_t* indices, size_t count);
double llg_queue_value_get_nested_real(const llg_queue_value_array_t* queue,
                                       const sv4_t* indices, size_t count);
llg_string_t llg_queue_value_get_nested_string(
    const llg_queue_value_array_t* queue, const sv4_t* indices, size_t count);
void* llg_queue_value_get_nested_chandle(
    const llg_queue_value_array_t* queue, const sv4_t* indices, size_t count);
int llg_queue_value_set(llg_queue_value_array_t* queue, sv4_t index,
                        sv4_t value);
int llg_queue_value_set_real(llg_queue_value_array_t* queue, sv4_t index,
                             double value);
int llg_queue_value_set_string(llg_queue_value_array_t* queue, sv4_t index,
                               llg_string_t value);
int llg_queue_value_set_chandle(llg_queue_value_array_t* queue, sv4_t index,
                                void* value);
int llg_queue_value_set_nested(llg_queue_value_array_t* queue,
                               const sv4_t* indices, size_t count, sv4_t value);
int llg_queue_value_set_nested_real(llg_queue_value_array_t* queue,
                                    const sv4_t* indices, size_t count,
                                    double value);
int llg_queue_value_set_nested_string(
    llg_queue_value_array_t* queue, const sv4_t* indices, size_t count,
    llg_string_t value);
int llg_queue_value_set_nested_chandle(
    llg_queue_value_array_t* queue, const sv4_t* indices, size_t count,
    void* value);
int llg_queue_value_set_nested_container(
    llg_queue_value_array_t* queue, const sv4_t* indices, size_t count,
    const llg_dyn_value_array_t* source);
int llg_queue_value_set_nested_container_from_packed(
    llg_queue_value_array_t* queue, const sv4_t* indices, size_t count,
    const llg_dyn_array_t* source);
void llg_queue_value_push_front(llg_queue_value_array_t* queue, sv4_t value);
void llg_queue_value_push_back(llg_queue_value_array_t* queue, sv4_t value);
void llg_queue_value_push_front_real(llg_queue_value_array_t* queue,
                                     double value);
void llg_queue_value_push_back_real(llg_queue_value_array_t* queue,
                                    double value);
void llg_queue_value_push_front_string(llg_queue_value_array_t* queue,
                                       llg_string_t value);
void llg_queue_value_push_back_string(llg_queue_value_array_t* queue,
                                      llg_string_t value);
void llg_queue_value_push_front_chandle(llg_queue_value_array_t* queue,
                                        void* value);
void llg_queue_value_push_back_chandle(llg_queue_value_array_t* queue,
                                       void* value);
void llg_queue_value_push_front_container(
    llg_queue_value_array_t* queue, const llg_dyn_value_array_t* source);
void llg_queue_value_push_back_container(
    llg_queue_value_array_t* queue, const llg_dyn_value_array_t* source);
void llg_queue_value_push_front_container_from_packed(
    llg_queue_value_array_t* queue, const llg_dyn_array_t* source);
void llg_queue_value_push_back_container_from_packed(
    llg_queue_value_array_t* queue, const llg_dyn_array_t* source);
int llg_queue_value_insert(llg_queue_value_array_t* queue, sv4_t index,
                           sv4_t value);
int llg_queue_value_insert_real(llg_queue_value_array_t* queue, sv4_t index,
                                double value);
int llg_queue_value_insert_string(llg_queue_value_array_t* queue, sv4_t index,
                                  llg_string_t value);
int llg_queue_value_insert_chandle(llg_queue_value_array_t* queue,
                                   sv4_t index, void* value);
int llg_queue_value_insert_container(llg_queue_value_array_t* queue,
                                     sv4_t index,
                                     const llg_dyn_value_array_t* source);
int llg_queue_value_insert_container_from_packed(
    llg_queue_value_array_t* queue, sv4_t index, const llg_dyn_array_t* source);
int llg_queue_value_delete_index(llg_queue_value_array_t* queue, sv4_t index);
/* Remove the front (`back` == 0) or back element and return it as an
 * independent result: a real, the moved string (into the empty or
 * expression-owned `dst`) or the handle identity. An empty queue yields the
 * Table 7-1 value (0.0, "", null) and does not notify. */
double llg_queue_value_pop_real(llg_queue_value_array_t* queue, int back);
void llg_queue_value_pop_string_to(llg_string_t* dst,
                                   llg_queue_value_array_t* queue, int back);
void* llg_queue_value_pop_chandle(llg_queue_value_array_t* queue, int back);
/* Process elements transfer their reference into `*dst`, releasing the
 * handle it previously held. */
void llg_queue_value_pop_process_to(void** dst, llg_queue_value_array_t* queue,
                                    int back);

void llg_dyn_init(llg_dyn_array_t* array, uint32_t element_width,
                  int8_t element_signed, int element_two_state);
void llg_dyn_destroy(llg_dyn_array_t* array);
void llg_dyn_delete(llg_dyn_array_t* array);
void llg_dyn_copy(llg_dyn_array_t* dst, const llg_dyn_array_t* src);
void llg_dyn_assign_values(llg_dyn_array_t* dst, const sv4_t* values,
                           size_t count);
void llg_dyn_new(llg_dyn_array_t* dst, sv4_t size,
                 const llg_dyn_array_t* initializer);
sv4_t llg_dyn_stream(const llg_dyn_array_t* array, uint32_t slice,
                     int right_to_left, int selector_kind, sv4_t first,
                     sv4_t second);
void llg_dyn_unstream_assign(llg_dyn_array_t* dst, sv4_t source,
                             uint32_t slice, int right_to_left,
                             int selector_kind, sv4_t first, sv4_t second);
void llg_dyn_resize(llg_dyn_array_t* array, sv4_t size);
size_t llg_dyn_size(const llg_dyn_array_t* array);
sv4_t llg_dyn_get(const llg_dyn_array_t* array, sv4_t index);
int llg_dyn_set(llg_dyn_array_t* array, sv4_t index, sv4_t value);
sv4_t llg_dyn_reduce(const llg_dyn_array_t* array, int operation);
sv4_t llg_dyn_reduce_with(const llg_dyn_array_t* array, int operation,
                          uint32_t result_width, int8_t result_signed,
                          int result_two_state, llg_container_eval_fn eval,
                          void* context);
void llg_dyn_method_assign(llg_queue_t* dst, const llg_dyn_array_t* src,
                           int method, llg_container_eval_fn eval,
                           void* context);
void llg_dyn_method(llg_dyn_array_t* array, int method,
                    llg_container_eval_fn eval, void* context);
void llg_container_seed(uint64_t seed);

/* Fixed-array sort workspace for generated models. The model evaluates each
 * element's key once, in declaration order, into `keys`; llg_fixed_order_sort
 * then stores in order[i] the original position of the element that belongs
 * at position i (stable; unknown keys stay in place, as for container sort).
 * `row` saves one element of `row_cells` cells while the model applies a
 * permutation cycle through its own storage writes. Allocation failure is
 * fatal; destroy releases every owner and accepts a partially used workspace. */
typedef struct llg_fixed_order_t {
    sv4_t* keys;
    size_t* order;
    sv4_t* row;
    size_t count;
    size_t row_cells;
} llg_fixed_order_t;
void llg_fixed_order_init(llg_fixed_order_t* order, uint64_t count,
                          uint64_t row_cells);
int llg_fixed_order_sort(llg_fixed_order_t* order, int descending);
void llg_fixed_order_destroy(void* order);

struct llg_queue_t {
    sv4_t* data;
    uint64_t* element_ids;
    size_t size;
    size_t capacity;
    // Maximum element count; SIZE_MAX denotes an unbounded queue.
    size_t limit;
    uint32_t element_width;
    int8_t element_signed;
    uint8_t element_two_state;
    sv4_t* contents_dependency;
    sv4_t* shape_dependency;
    llg_container_notify_fn notify;
    uint64_t next_element_id;
    struct llg_queue_cell* references; // borrowed list; reference holders own the cells
};

typedef struct llg_queue_source_t {
    const llg_queue_t* queue;
    sv4_t left;
    sv4_t right;
    uint8_t left_unbounded;
    uint8_t right_unbounded;
    const llg_queue_value_array_t* value_queue;
    uint8_t value_kind;
} llg_queue_source_t;

void llg_queue_init(llg_queue_t* queue, uint32_t element_width,
                    int8_t element_signed, int element_two_state,
                    uint64_t maximum_elements);
void llg_queue_destroy(llg_queue_t* queue);
void llg_queue_delete(llg_queue_t* queue);
void llg_queue_copy(llg_queue_t* dst, const llg_queue_t* src);
void llg_queue_assign_values(llg_queue_t* dst, const sv4_t* values,
                             size_t count);
sv4_t llg_queue_stream(const llg_queue_t* queue, uint32_t slice,
                       int right_to_left, int selector_kind, sv4_t first,
                       sv4_t second);
void llg_queue_unstream_assign(llg_queue_t* dst, sv4_t source,
                               uint32_t slice, int right_to_left,
                               int selector_kind, sv4_t first, sv4_t second);
void llg_queue_assign_sources(llg_queue_t* dst,
                              const llg_queue_source_t* sources,
                              size_t source_count);
size_t llg_queue_size(const llg_queue_t* queue);
sv4_t llg_queue_get(const llg_queue_t* queue, sv4_t index);
int llg_queue_set(llg_queue_t* queue, sv4_t index, sv4_t value);
int llg_queue_insert(llg_queue_t* queue, sv4_t index, sv4_t value);
int llg_queue_delete_index(llg_queue_t* queue, sv4_t index);
void llg_queue_push_front(llg_queue_t* queue, sv4_t value);
void llg_queue_push_back(llg_queue_t* queue, sv4_t value);
/* Output forms publish into caller-owned storage before notifying. Use a
 * registered output when notifications may terminate a coroutine. The returned
 * value convenience forms require notifications to return normally. */
void llg_queue_pop_front_into(llg_queue_t* queue, sv4_t* out);
void llg_queue_pop_back_into(llg_queue_t* queue, sv4_t* out);
sv4_t llg_queue_pop_front(llg_queue_t* queue);
sv4_t llg_queue_pop_back(llg_queue_t* queue);
sv4_t llg_queue_front(const llg_queue_t* queue);
sv4_t llg_queue_back(const llg_queue_t* queue);
sv4_t llg_queue_reduce(const llg_queue_t* queue, int operation);
sv4_t llg_queue_reduce_with(const llg_queue_t* queue, int operation,
                            uint32_t result_width, int8_t result_signed,
                            int result_two_state, llg_container_eval_fn eval,
                            void* context);
void llg_queue_method_assign(llg_queue_t* dst, const llg_queue_t* src,
                             int method, llg_container_eval_fn eval,
                             void* context);
/* Stable order of real keys, ascending or (when `descending`) descending:
 * on return order[i] names the original position whose key belongs at i.
 * `order` holds 2 * count entries (the upper half is merge scratch). NaN keys
 * are unordered and keep their positions; the keys between them sort
 * independently, like unknown packed keys. Returns whether any key moves. */
int llg_real_sort_order(const double* keys, size_t count, int descending,
                        size_t* order);
/* In-place sort/rsort/reverse/shuffle of a resizable container whose
 * elements are real/shortreal values (LLG_CONTAINER_METHOD_*). Values are
 * compared numerically, never as bit patterns. */
void llg_dyn_value_method(llg_dyn_value_array_t* array, int method);
/* `with` evaluator over a real iterator item; `out` receives the packed
 * truth of the clause. `context` is borrowed for the callback. */
typedef void (*llg_container_real_eval_fn)(sv4_t* out, double item,
                                           sv4_t index, void* context);
/* Queue-valued locator methods over `count` real elements (find*, min, max,
 * unique). Value results replace the real queue `dst`; the *_index forms
 * replace the integral queue `dst_indices`. find* select items whose clause
 * is known true; min/max/unique compare values numerically without a clause
 * (NaN is unordered: it is skipped by min/max unless every element is NaN,
 * and every NaN is unique). The source elements are borrowed. */
void llg_real_method_assign_values(llg_queue_value_array_t* dst,
                                   const llg_value_t* data, size_t count,
                                   int method, llg_container_real_eval_fn eval,
                                   void* context);
void llg_real_method_assign_indices(llg_queue_t* dst_indices,
                                    const llg_value_t* data, size_t count,
                                    int method, llg_container_real_eval_fn eval,
                                    void* context);
void llg_queue_value_method(llg_queue_value_array_t* queue, int method);
void llg_queue_method(llg_queue_t* queue, int method,
                      llg_container_eval_fn eval, void* context);
/* Retain an element independently of queue membership. Acquisitions of the
 * same live element share one cell. Release once per acquisition, including
 * cancellation; deleting/replacing/destroying the queue only disconnects it. */
void* llg_queue_ref_acquire(llg_queue_t* queue, uint64_t index);
void llg_queue_ref_release(void* cell);
sv4_t llg_queue_cell_read(const void* cell);
int llg_queue_cell_write(void* cell, sv4_t value);
uint64_t llg_queue_ref_identity(const llg_queue_t* queue, uint64_t index);
sv4_t llg_queue_ref_read(const llg_queue_t* queue, uint64_t identity);
int llg_queue_ref_write(llg_queue_t* queue, uint64_t identity, sv4_t value);

/* Associative storage for recursive/non-packed elements. */
typedef struct {
    sv4_t integral_key;
    unsigned char* string_key;
    size_t string_length;
    llg_value_t value;
} llg_assoc_value_entry_t;

struct llg_assoc_value_t {
    llg_assoc_value_entry_t* entries;
    size_t size;
    size_t capacity;
    const llg_value_desc_t* element;
    uint8_t key_kind;
    uint32_t key_width;
    int8_t key_signed;
    uint8_t key_two_state;
    llg_value_t default_value;
    uint8_t has_default_value;
    sv4_t* contents_dependency;
    sv4_t* shape_dependency;
    llg_container_notify_fn notify;
    uint64_t mutation_epoch;
};

void llg_assoc_value_init_integral(llg_assoc_value_t* array,
                                   const llg_value_desc_t* element,
                                   uint32_t key_width, int8_t key_signed,
                                   int key_two_state);
void llg_assoc_value_init_string(llg_assoc_value_t* array,
                                 const llg_value_desc_t* element);
void llg_assoc_value_destroy(llg_assoc_value_t* array);
void llg_assoc_value_delete(llg_assoc_value_t* array);
void llg_assoc_value_copy(llg_assoc_value_t* dst,
                          const llg_assoc_value_t* src);
size_t llg_assoc_value_count(const llg_assoc_value_t* array);
sv4_t llg_assoc_value_get_integral(const llg_assoc_value_t* array, sv4_t key);
double llg_assoc_value_get_integral_real(const llg_assoc_value_t* array,
                                         sv4_t key);
llg_string_t llg_assoc_value_get_integral_string(
    const llg_assoc_value_t* array, sv4_t key);
void* llg_assoc_value_get_integral_chandle(
    const llg_assoc_value_t* array, sv4_t key);
sv4_t llg_assoc_value_get_nested_integral(
    const llg_assoc_value_t* array, const sv4_t* indices, size_t count);
double llg_assoc_value_get_nested_integral_real(
    const llg_assoc_value_t* array, const sv4_t* indices, size_t count);
llg_string_t llg_assoc_value_get_nested_integral_string(
    const llg_assoc_value_t* array, const sv4_t* indices, size_t count);
void* llg_assoc_value_get_nested_integral_chandle(
    const llg_assoc_value_t* array, const sv4_t* indices, size_t count);
int llg_assoc_value_set_integral(llg_assoc_value_t* array, sv4_t key,
                                 sv4_t value);
int llg_assoc_value_set_integral_real(llg_assoc_value_t* array, sv4_t key,
                                      double value);
int llg_assoc_value_set_integral_string(llg_assoc_value_t* array, sv4_t key,
                                        llg_string_t value);
int llg_assoc_value_set_integral_chandle(llg_assoc_value_t* array, sv4_t key,
                                         void* value);
int llg_assoc_value_set_nested_integral_container(
    llg_assoc_value_t* array, const sv4_t* indices, size_t count,
    const llg_dyn_value_array_t* source);
int llg_assoc_value_set_nested_integral_container_from_packed(
    llg_assoc_value_t* array, const sv4_t* indices, size_t count,
    const llg_dyn_array_t* source);
int llg_assoc_value_set_nested_integral(
    llg_assoc_value_t* array, const sv4_t* indices, size_t count,
    sv4_t value);
int llg_assoc_value_set_nested_integral_real(
    llg_assoc_value_t* array, const sv4_t* indices, size_t count,
    double value);
int llg_assoc_value_set_nested_integral_string(
    llg_assoc_value_t* array, const sv4_t* indices, size_t count,
    llg_string_t value);
int llg_assoc_value_set_nested_integral_chandle(
    llg_assoc_value_t* array, const sv4_t* indices, size_t count,
    void* value);
int llg_assoc_value_exists_integral(const llg_assoc_value_t* array, sv4_t key);
int llg_assoc_value_delete_integral(llg_assoc_value_t* array, sv4_t key);
void llg_assoc_value_set_default(llg_assoc_value_t* array, sv4_t value);
void llg_assoc_value_set_default_real(llg_assoc_value_t* array, double value);
void llg_assoc_value_set_default_string(llg_assoc_value_t* array,
                                        llg_string_t value);
void llg_assoc_value_set_default_chandle(llg_assoc_value_t* array, void* value);
void llg_assoc_value_reset_default(llg_assoc_value_t* array);
int llg_assoc_value_first_integral(const llg_assoc_value_t* array, sv4_t* key);
int llg_assoc_value_last_integral(const llg_assoc_value_t* array, sv4_t* key);
int llg_assoc_value_next_integral(const llg_assoc_value_t* array, sv4_t* key);
int llg_assoc_value_prev_integral(const llg_assoc_value_t* array, sv4_t* key);
llg_string_t llg_assoc_value_get_string(const llg_assoc_value_t* array,
                                        const void* key, size_t key_length);
double llg_assoc_value_get_string_real(const llg_assoc_value_t* array,
                                       const void* key, size_t key_length);
llg_string_t llg_assoc_value_get_string_string(const llg_assoc_value_t* array,
                                               const void* key,
                                               size_t key_length);
void* llg_assoc_value_get_string_chandle(const llg_assoc_value_t* array,
                                         const void* key, size_t key_length);
int llg_assoc_value_set_string(llg_assoc_value_t* array, const void* key,
                               size_t key_length, sv4_t value);
int llg_assoc_value_set_string_real(llg_assoc_value_t* array, const void* key,
                                    size_t key_length, double value);
int llg_assoc_value_set_string_string(llg_assoc_value_t* array,
                                      const void* key, size_t key_length,
                                      llg_string_t value);
int llg_assoc_value_set_string_chandle(llg_assoc_value_t* array,
                                       const void* key, size_t key_length,
                                       void* value);
int llg_assoc_value_exists_string(const llg_assoc_value_t* array,
                                  const void* key, size_t key_length);
int llg_assoc_value_delete_string(llg_assoc_value_t* array, const void* key,
                                  size_t key_length);
int llg_assoc_value_first_string(const llg_assoc_value_t* array,
                                 const unsigned char** key, size_t* key_length);
int llg_assoc_value_last_string(const llg_assoc_value_t* array,
                                const unsigned char** key, size_t* key_length);
int llg_assoc_value_next_string(const llg_assoc_value_t* array,
                                const void* current, size_t current_length,
                                const unsigned char** key, size_t* key_length);
int llg_assoc_value_prev_string(const llg_assoc_value_t* array,
                                const void* current, size_t current_length,
                                const unsigned char** key, size_t* key_length);

enum {
    LLG_ASSOC_INTEGRAL = 0,
    LLG_ASSOC_STRING = 1,
};

typedef struct {
    sv4_t integral_key;
    unsigned char* string_key;
    size_t string_length;
    sv4_t value;
} llg_assoc_entry_t;

typedef struct {
    llg_assoc_entry_t* entries;
    size_t size;
    size_t capacity;
    uint32_t element_width;
    int8_t element_signed;
    uint8_t element_two_state;
    uint8_t key_kind;
    // Zero denotes the wildcard integral index type. Wildcard keys are
    // canonicalized to minimal significant width; traversal is rejected because IEEE
    // 1800-2009 7.8.1 forbids it.
    uint32_t key_width;
    int8_t key_signed;
    uint8_t key_two_state;
    // Missing associative entries read this value without allocating an
    // entry.  `has_default_value` distinguishes an explicit assignment
    // pattern default from the element-type default for copy semantics.
    sv4_t default_value;
    uint8_t has_default_value;
    sv4_t* contents_dependency;
    sv4_t* shape_dependency;
    llg_container_notify_fn notify;
} llg_assoc_t;

void llg_assoc_init_integral(llg_assoc_t* array, uint32_t element_width,
                             int8_t element_signed, int element_two_state,
                             uint32_t key_width, int8_t key_signed,
                             int key_two_state);
void llg_assoc_init_string(llg_assoc_t* array, uint32_t element_width,
                           int8_t element_signed, int element_two_state);
void llg_assoc_destroy(llg_assoc_t* array);
void llg_assoc_delete(llg_assoc_t* array);
void llg_assoc_copy(llg_assoc_t* dst, const llg_assoc_t* src);
size_t llg_assoc_count(const llg_assoc_t* array);
sv4_t llg_assoc_value_at(const llg_assoc_t* array, size_t index);
sv4_t llg_assoc_reduce(const llg_assoc_t* array, int operation);
sv4_t llg_assoc_reduce_with(const llg_assoc_t* array, int operation,
                            uint32_t result_width, int8_t result_signed,
                            int result_two_state, llg_container_eval_fn eval,
                            void* context);
void llg_assoc_method_assign(llg_queue_t* dst, const llg_assoc_t* src,
                             int method, llg_container_eval_fn eval,
                             void* context);

sv4_t llg_assoc_get_integral(const llg_assoc_t* array, sv4_t key);
int llg_assoc_set_integral(llg_assoc_t* array, sv4_t key, sv4_t value);
int llg_assoc_exists_integral(const llg_assoc_t* array, sv4_t key);
int llg_assoc_delete_integral(llg_assoc_t* array, sv4_t key);
void llg_assoc_set_default(llg_assoc_t* array, sv4_t value);
void llg_assoc_reset_default(llg_assoc_t* array);
int llg_assoc_first_integral(const llg_assoc_t* array, sv4_t* key);
int llg_assoc_last_integral(const llg_assoc_t* array, sv4_t* key);
int llg_assoc_next_integral(const llg_assoc_t* array, sv4_t* key);
int llg_assoc_prev_integral(const llg_assoc_t* array, sv4_t* key);

sv4_t llg_assoc_get_string(const llg_assoc_t* array, const void* key,
                           size_t key_length);
int llg_assoc_set_string(llg_assoc_t* array, const void* key, size_t key_length,
                         sv4_t value);
int llg_assoc_exists_string(const llg_assoc_t* array, const void* key,
                            size_t key_length);
int llg_assoc_delete_string(llg_assoc_t* array, const void* key,
                            size_t key_length);
int llg_assoc_first_string(const llg_assoc_t* array, const unsigned char** key,
                           size_t* key_length);
int llg_assoc_last_string(const llg_assoc_t* array, const unsigned char** key,
                          size_t* key_length);
int llg_assoc_next_string(const llg_assoc_t* array, const void* current,
                          size_t current_length, const unsigned char** key,
                          size_t* key_length);
int llg_assoc_prev_string(const llg_assoc_t* array, const void* current,
                          size_t current_length, const unsigned char** key,
                          size_t* key_length);

/* Destination-passing forms of the packed-returning calls above, used by
 * generated code (see value/destinations.h). `X_to(dst, ...)` replaces the
 * initialized owner at dst with exactly the result `X(...)` would return. Packed
 * arguments are borrowed by address; string arguments are consumed through
 * their address and left empty, like the returning forms' by-value strings. */
void llg_dyn_value_get_nested_to(sv4_t* dst, const llg_dyn_value_array_t* array, const sv4_t* indices, size_t count);
void llg_fixed_stream_source_to(sv4_t* dst, const sv4_t* values, int64_t declaration_left, int64_t declaration_right, uint32_t element_width, const sv4_t* fallback, int selector_kind, const sv4_t* first, const sv4_t* second);
void llg_stream_unpack_source_to(sv4_t* dst, const sv4_t* value, uint64_t bits, uint32_t slice, int right_to_left);
void llg_fixed_image_stream_source_to(sv4_t* dst, const sv4_t* image, int64_t declaration_left, int64_t declaration_right, uint32_t element_width, const sv4_t* fallback, int selector_kind, const sv4_t* first, const sv4_t* second);
void llg_stream_to_fixed_to(sv4_t* dst, const sv4_t* value, uint32_t width, int is_signed);
void llg_queue_value_get_to(sv4_t* dst, const llg_queue_value_array_t* queue, const sv4_t* index);
void llg_queue_value_get_nested_to(sv4_t* dst, const llg_queue_value_array_t* queue, const sv4_t* indices, size_t count);
void llg_dyn_stream_to(sv4_t* dst, const llg_dyn_array_t* array, uint32_t slice, int right_to_left, int selector_kind, const sv4_t* first, const sv4_t* second);
void llg_dyn_get_to(sv4_t* dst, const llg_dyn_array_t* array, const sv4_t* index);
void llg_dyn_reduce_to(sv4_t* dst, const llg_dyn_array_t* array, int operation);
void llg_dyn_reduce_with_to(sv4_t* dst, const llg_dyn_array_t* array, int operation, uint32_t result_width, int8_t result_signed, int result_two_state, llg_container_eval_fn eval, void* context);
void llg_queue_stream_to(sv4_t* dst, const llg_queue_t* queue, uint32_t slice, int right_to_left, int selector_kind, const sv4_t* first, const sv4_t* second);
void llg_queue_get_to(sv4_t* dst, const llg_queue_t* queue, const sv4_t* index);
void llg_queue_pop_front_to(sv4_t* dst, llg_queue_t* queue);
void llg_queue_pop_back_to(sv4_t* dst, llg_queue_t* queue);
void llg_queue_front_to(sv4_t* dst, const llg_queue_t* queue);
void llg_queue_back_to(sv4_t* dst, const llg_queue_t* queue);
void llg_queue_reduce_to(sv4_t* dst, const llg_queue_t* queue, int operation);
void llg_queue_reduce_with_to(sv4_t* dst, const llg_queue_t* queue, int operation, uint32_t result_width, int8_t result_signed, int result_two_state, llg_container_eval_fn eval, void* context);
void llg_queue_cell_read_to(sv4_t* dst, const void* cell);
void llg_queue_ref_read_to(sv4_t* dst, const llg_queue_t* queue, uint64_t identity);
void llg_assoc_value_get_integral_to(sv4_t* dst, const llg_assoc_value_t* array, const sv4_t* key);
void llg_assoc_value_get_nested_integral_to(sv4_t* dst, const llg_assoc_value_t* array, const sv4_t* indices, size_t count);
void llg_assoc_value_at_to(sv4_t* dst, const llg_assoc_t* array, size_t index);
void llg_assoc_reduce_to(sv4_t* dst, const llg_assoc_t* array, int operation);
void llg_assoc_reduce_with_to(sv4_t* dst, const llg_assoc_t* array, int operation, uint32_t result_width, int8_t result_signed, int result_two_state, llg_container_eval_fn eval, void* context);
void llg_assoc_get_integral_to(sv4_t* dst, const llg_assoc_t* array, const sv4_t* key);
void llg_assoc_get_string_to(sv4_t* dst, const llg_assoc_t* array, const void* key, size_t key_length);

/* String destination forms: `X_to(dst, ...)` replaces the expression owner at
 * dst (destroyed first; it must carry no change callback) with the string
 * `X(...)` would return. Argument conventions match the packed forms above. */
void llg_dyn_value_get_string_to(llg_string_t* dst, const llg_dyn_value_array_t* array, const sv4_t* index);
void llg_dyn_value_get_nested_string_to(llg_string_t* dst, const llg_dyn_value_array_t* array, const sv4_t* indices, size_t count);
void llg_queue_value_get_string_to(llg_string_t* dst, const llg_queue_value_array_t* queue, const sv4_t* index);
void llg_queue_value_get_nested_string_to(llg_string_t* dst, const llg_queue_value_array_t* queue, const sv4_t* indices, size_t count);
void llg_assoc_value_get_integral_string_to(llg_string_t* dst, const llg_assoc_value_t* array, const sv4_t* key);
void llg_assoc_value_get_nested_integral_string_to(llg_string_t* dst, const llg_assoc_value_t* array, const sv4_t* indices, size_t count);
void llg_assoc_value_get_string_to(llg_string_t* dst, const llg_assoc_value_t* array, const void* key, size_t key_length);
void llg_assoc_value_get_string_string_to(llg_string_t* dst, const llg_assoc_value_t* array, const void* key, size_t key_length);

#ifdef __cplusplus
}
#endif

#endif // LLG_CONTAINER_H
