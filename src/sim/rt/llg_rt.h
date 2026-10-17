// llg_rt.h — event scheduler for the llg Verilog simulator's generated C11
// models. Four-state values are provided by llg_value.h.

// Processes are stackless llg_co coroutines. Each generated process descriptor
// names an explicit POD root frame and an llg_co function; blocking runtime
// services arm a wait and return READY, SUSPEND, or EXIT to generated code.
// The scheduler (`llg_rt_run`) keeps a separate queue for every
// IEEE 1800-2017 §4 execution region. Region queues are drained to a fixed
// point in the Figure 4-1 order;
// reactive work may enqueue design work and starts another design iteration
// before the postponed output point. Legacy Verilog scheduling remains the
// Active/Inactive/NBA subset of this state machine.
//
// Zero-time progress is bounded by two runtime limits.  The scheduler guard
// counts region passes and coroutine resumes; the process guard counts
// generated loop back-edges, so a coroutine that never yields is still
// interruptible.  Both default to LLG_ZERO_LOOP_LIMIT and can be configured
// with the LLG_ZERO_LOOP_LIMIT environment variable.  The per-process limit
// can be overridden with LLG_PROCESS_STEP_LIMIT (or the legacy-compatible
// LLG_NONCONVERGENCE_LIMIT alias).  Limits must be positive decimal uint64
// values; invalid or overflowing values are diagnosed before simulation.
//
// Time is measured in integer ticks; 1 tick == the design precision (the
// finest `timescale` precision across the design).  The runtime itself is
// timescale-agnostic: the codegen scales every `#N` delay and `$time` read per
// the calling module's `timescale` unit before calling `llg_arm_time` /
// `llg_time`. Typed `%t` arguments retain that owning unit and are converted
// through the design-wide `$timeformat` state. `$finish` reports its validated level through
// `llg_rt_finish_with_level`, sets the current chain's exit kind, and returns;
// no coroutine is resumed after a finish. Generated `$stop` uses
// `llg_arm_stop`, which preserves every queue, activation frame, output
// stream, and simulation tick until the stop policy resumes it.

#ifndef LLG_RT_H
#define LLG_RT_H

// Generated model.c defines LLG_MODEL_PROCESS_ABI before including this file.
// Keep this check in every model translation unit so a stale generated model
// fails at compile time; llg_co's versioned symbols independently reject a
// stale runtime archive at link time.
#define LLG_PROCESS_ABI_VERSION 3
#if defined(LLG_MODEL_PROCESS_ABI) && \
    LLG_MODEL_PROCESS_ABI != LLG_PROCESS_ABI_VERSION
#error "generated model process ABI does not match llg_rt.h"
#endif

#include <stddef.h>
#include <stdint.h>

#include "llg_compiler.h"
#include "llg_value.h"
#include "llg_string.h"
#include "llg_rng.h"
#include "llg_constraint.h"
#include "llg_container.h"

typedef struct llg_fixed_cell llg_fixed_cell_t;
typedef struct llg_fixed_image llg_fixed_image_t;
typedef struct llg_fixed_range llg_fixed_range_t;
typedef struct llg_fixed_array {
    uint64_t total;
    size_t count, capacity;
    llg_fixed_cell_t **buckets, *cells;
    sv4_t initial;
    sv4_t* contents;
    struct llg_fixed_array* owner;
    uint64_t origin;
    llg_fixed_range_t* ranges;
} llg_fixed_array_t;


#ifndef LLG_ZERO_LOOP_LIMIT
#define LLG_ZERO_LOOP_LIMIT 10000000ULL
#endif

#ifndef LLG_PROCESS_STEP_LIMIT
#define LLG_PROCESS_STEP_LIMIT LLG_ZERO_LOOP_LIMIT
#endif

// Live concurrent-assertion sequence threads (pending tokens plus attempts)
// across the model. Exhaustion is a reported execution error, never a
// silently dropped attempt. The LLG_SEQUENCE_THREAD_LIMIT environment
// variable (positive decimal uint64) overrides the default at run time.
#ifndef LLG_SEQUENCE_THREAD_LIMIT
#define LLG_SEQUENCE_THREAD_LIMIT 1000000ULL
#endif

// Native host stack of a generated model: scheduler entry, one polled
// coroutine segment, a plain-function chain as deep as the generated 256-call
// guard allows, and the runtime helpers (formatting, wide arithmetic including
// GMP temporaries, strings, containers, waveform writers). SystemVerilog
// recursion runs on heap frames and does not count. The value is the measured
// x86-64 worst case over GCC 14/Clang 19 at -O0/-O3 and both value backends:
// 255 nested functions that each format 24 arguments. Frames still grow with
// a statement's format-argument count, so this is a measured, not proven, bound.
#ifndef LLG_HOST_STACK_MEASURED_BYTES
#define LLG_HOST_STACK_MEASURED_BYTES (367u * 1024u)
#endif

// User DPI/VPI C code and libc run on the same stack and cannot be bounded
// here. glibc limits each internal alloca to 64 KiB; four such frames leave
// room for nested libc calls (printf/qsort class) below typical user code.
#ifndef LLG_HOST_STACK_FOREIGN_HEADROOM_BYTES
#define LLG_HOST_STACK_FOREIGN_HEADROOM_BYTES (256u * 1024u)
#endif

// POSIX hosts warn when RLIMIT_STACK is below this estimate (measured worst
// case plus foreign headroom, rounded up to 64 KiB: 640 KiB by default);
// generated MSVC projects reserve at least it with /STACK. Override per target
// with -D when its frames differ.
#ifndef LLG_HOST_STACK_ESTIMATE_BYTES
#define LLG_HOST_STACK_ESTIMATE_BYTES                                          \
    ((LLG_HOST_STACK_MEASURED_BYTES + LLG_HOST_STACK_FOREIGN_HEADROOM_BYTES +  \
      0xffffu) & ~0xffffu)
#endif

#ifdef __cplusplus
extern "C" {
#endif

/* init/reset consume a fresh default; peek borrows until the next mutation.
 * cell remains stable until array destruction. Destroy globals after runtime cleanup.
 * copy snapshots its entire source before publishing any destination. */
void llg_fixed_array_init(llg_fixed_array_t*, uint64_t, sv4_t, sv4_t*);
void llg_fixed_array_reset(llg_fixed_array_t*, sv4_t);
void llg_fixed_array_view_init(llg_fixed_array_t*, llg_fixed_array_t*, uint64_t, uint64_t, int);
void llg_fixed_array_stream_segments(llg_fixed_array_t*, const llg_fixed_array_t* const*, size_t, int, int, uint32_t);
void llg_fixed_array_merge(llg_fixed_array_t*, const llg_fixed_array_t*, const llg_fixed_array_t*, uint64_t, int);
const sv4_t* llg_fixed_array_peek(const llg_fixed_array_t*, uint64_t);
sv4_t* llg_fixed_array_cell(llg_fixed_array_t*, uint64_t);
void llg_fixed_array_destroy(void*);
void llg_fixed_array_copy(llg_fixed_array_t*, const llg_fixed_array_t*, int, int);
void llg_fixed_array_fill(llg_fixed_array_t*, sv4_t, int, int);
void llg_fixed_array_stream_copy(llg_fixed_array_t*, const llg_fixed_array_t*, int, int, uint32_t);
sv4_t llg_fixed_array_compare(const llg_fixed_array_t*, const llg_fixed_array_t*, int, int);
/* Pack a runtime `with` selection of a one-dimensional descriptor array in
 * stream order; indices outside the bounds stream `fallback` (borrowed). */
sv4_t llg_fixed_array_stream_source(const llg_fixed_array_t*, int64_t, int64_t, uint32_t, sv4_t, int, sv4_t, sv4_t);
/* Descriptor stream operands (SV 11.4.14). Each initializes a zeroed
 * scratch array and returns 0, leaving it zeroed, when the operand has no
 * bits. packed_source splits a borrowed packed value into `cell_width`-bit
 * cells, left cell first. with_source presents a runtime `with` selection of
 * a one-dimensional descriptor array in storage order; indices outside the
 * bounds read the borrowed `fallback`, and no owner cell is materialized.
 * stream_value lazily images a nested stream of `count` sources. */
int llg_fixed_array_packed_source(llg_fixed_array_t*, sv4_t, uint32_t);
int llg_fixed_array_with_source(llg_fixed_array_t*, const llg_fixed_array_t*, int64_t, int64_t, sv4_t, int, sv4_t, sv4_t);
int llg_fixed_array_stream_value(llg_fixed_array_t*, const llg_fixed_array_t* const*, size_t, uint32_t);
/* Copy `total` dense cells from `origin` into a zeroed scratch array; an
 * invalid view (`origin` UINT64_MAX) reads the borrowed `fallback`. */
void llg_fixed_array_dense_source(llg_fixed_array_t*, const sv4_t*, uint64_t, uint64_t, sv4_t);
/* A descriptor stream operand holding the `cell_width`-bit cells of a
 * runtime-sized bit stream, left cell first; returns 0 for an empty stream. */
int llg_fixed_array_bitstream_source(llg_fixed_array_t*, const llg_bitstream_t*, uint32_t);
/* Append every cell in storage order to a runtime-sized bit stream. */
void llg_bitstream_append_fixed_array(llg_bitstream_t*, const llg_fixed_array_t*);

// Keep the library and every model translation unit on the host-allocation
// declaration contract. CMake supplies the same definition when compiling
// llg_co.c, which includes llg_co.h directly.
#ifndef LLG_CO_HOST_ALLOC
#define LLG_CO_HOST_ALLOC 1
#endif
struct llg_co_frame;
_Noreturn void llg_rt_co_oom(size_t bytes);
// Reports `co` and `fn`, then walks the current process's descriptor chain
// with llg_co_backtrace when a process resume is active. Never returns.
_Noreturn void llg_rt_co_bad_state(const struct llg_co_frame* co,
                                   const char* fn);
#define LLG_CO_OOM(bytes) llg_rt_co_oom(bytes)
#define LLG_CO_BAD_STATE(co, fn) llg_rt_co_bad_state((co), (fn))
// The handler above is _Noreturn: lets the coroutine dispatch drop its
// unreachable fallback return on MSVC (C4702 at /W4).
#define LLG_CO_BAD_STATE_NORETURN 1
#include "llg_co.h"

// Startup is mandatory even for models without arena calls. Separate debug
// link names reject mismatched liveness protocols despite identical layouts.
#ifdef LLG_CO_DEBUG
#define llg_rt_init llg_rt_init_debug
#define llg_rt_init_with_args llg_rt_init_with_args_debug
#define llg_rt_init_with_precision llg_rt_init_with_precision_debug
#define llg_rt_init_with_args_and_precision llg_rt_init_with_args_and_precision_debug
#endif

// Public generated-process entry type. The descriptor's `fn` is the sole
// entry pointer passed to spawn/fork services.
typedef llg_co_status_t (*llg_process_fn_t)(llg_co_frame_t* co,
                                            llg_co_chain_t* ch);

enum {
    LLG_EXIT_NONE = 0,
    LLG_EXIT_COMPLETE = 1, // run normal process-completion bookkeeping
    LLG_EXIT_ABANDON = 2,  // cancellation bookkeeping has already run
};
// Services set `chain.exiting` only for nonlocal process termination:
// finish/fatal, blocking-mailbox type failure, process/assertion-control kill,
// disable reaching self, program exit, budget abort, and terminating VPI
// calls. COMPLETE is used for finish/fatal/mailbox failures and for kill paths
// that request program completion; cancellation, disable, program exit and
// budget exhaustion use ABANDON. Services return normally after bookkeeping.

// Total retained coroutine-arena chunk bytes for the simulation thread.
// MT-1 will give each worker an independent cache with this cap.
#ifndef LLG_CO_CHUNK_CACHE_MAX_BYTES
#define LLG_CO_CHUNK_CACHE_MAX_BYTES (1024u * 1024u)
#endif

typedef struct {
    size_t system_allocations;
    size_t cache_hits;
    size_t system_frees;
    size_t cached_bytes;
    size_t peak_cached_bytes;
} llg_rt_co_cache_stats_t;

// Snapshot cumulative chunk-cache counters. The cache is owned by the
// simulation thread; callers must not race this accessor with simulation.
void llg_rt_co_cache_get_stats(llg_rt_co_cache_stats_t* stats);

// ── Collected objects (SIM-018, IEEE 1800-2009 §8.27) ───────────────────────
// Class objects live on a precise, non-moving mark-sweep heap. Collection runs
// only at scheduler safe points (after a process turn or region callback has
// returned, never inside a process turn or callback window) once enough
// objects were allocated. Roots are enumerated exactly: registered root
// producers, frame maps of suspended coroutines, handle cells, captured
// frames, native roots, mailboxes, queued NBAs, wait subscriptions and pins.
// A candidate handle is accepted only when it is the exact address of a live
// object, so opaque slots holding other identities (mailboxes, events,
// virtual interfaces, chandles) are ignored without being dereferenced.
// Objects never move: handle equality and random streams are unaffected.

// Objects allocated since the last collection before another is requested
// (run-time override LLG_GC_THRESHOLD).
#ifndef LLG_GC_DEFAULT_THRESHOLD
#define LLG_GC_DEFAULT_THRESHOLD 4096u
#endif
// After a collection the threshold becomes max(threshold, live * this / 100)
// (run-time override LLG_GC_GROWTH_PERCENT).
#ifndef LLG_GC_DEFAULT_GROWTH_PERCENT
#define LLG_GC_DEFAULT_GROWTH_PERCENT 100u
#endif

typedef struct llg_gc_tracer llg_gc_tracer_t;
typedef struct llg_gc_type llg_gc_type_t;
// First member of every collected object; owned by the collector.
typedef struct llg_gc_header {
    const llg_gc_type_t* type;
    uint32_t mark;
    uint32_t pins;
} llg_gc_header_t;
struct llg_gc_type {
    const char* name;
    // Visit every outgoing handle (llg_gc_visit and friends).
    void (*trace)(void* object, llg_gc_tracer_t* tracer);
    // Optional: nonzero when the object's own storage contains an address
    // reported through llg_gc_visit_interior (see llg_gc_interior_hit).
    int (*interior)(const void* object, const llg_gc_tracer_t* tracer);
    // Release the object's fields; never follows handles or frees `object`.
    void (*finalize)(void* object);
    // Optional (LLG_GC_VERIFY): poison an unreachable object kept allocated so
    // a later access through a missed root is reported, not undefined.
    void (*condemn)(void* object);
};
typedef void (*llg_gc_root_fn)(llg_gc_tracer_t* tracer, void* context);
typedef void (*llg_gc_payload_trace_fn)(const void* payload,
                                        llg_gc_tracer_t* tracer);
// One traced coroutine frame field: a handle (`void*`) or a pointer that may
// point into an object's storage.
enum { LLG_GC_FRAME_HANDLE = 0, LLG_GC_FRAME_INTERIOR = 1 };
typedef struct {
    uint32_t offset;
    uint32_t kind;
} llg_gc_frame_slot_t;
// A frame suspended at resume state `s` (< n_states) holds live traced
// fields slots[first[s]] .. slots[first[s] + count[s] - 1].
typedef struct {
    const llg_gc_frame_slot_t* slots;
    const uint32_t* first;
    const uint32_t* count;
    uint32_t n_states;
} llg_gc_frame_map_t;
typedef struct {
    uint64_t collections;
    uint64_t failed_collections;
    uint64_t allocated;
    uint64_t freed;
    uint64_t condemned;
    uint64_t live;
    uint64_t peak_live;
    uint64_t pinned;
    uint64_t last_marked;
} llg_gc_stats_t;

// Allocate a zeroed object of `size` bytes (>= sizeof(llg_gc_header_t)) whose
// header is initialized with `type`. Allocation failure is fatal.
void* llg_gc_alloc(size_t size, const llg_gc_type_t* type);
// Whether `handle` is the exact address of a live collected object.
int llg_gc_is_object(const void* handle);
// Tracer callbacks, valid only inside trace/root/interior callbacks.
void llg_gc_visit(llg_gc_tracer_t* tracer, const void* handle);
void llg_gc_visit_interior(llg_gc_tracer_t* tracer, const void* address);
void llg_gc_visit_value(llg_gc_tracer_t* tracer, const llg_value_t* value);
// Trace a payload owned through destructor `drop` (registered payload tracers;
// payloads without one hold no handles).
void llg_gc_visit_payload(llg_gc_tracer_t* tracer, void (*drop)(void*),
                          const void* payload);
int llg_gc_interior_hit(const llg_gc_tracer_t* tracer, const void* address);
// Root producers and tracers register after runtime initialization and stay
// registered until the next initialization. Return 0 on allocation failure.
int llg_gc_register_roots(llg_gc_root_fn fn, void* context);
int llg_gc_register_payload_tracer(void (*drop)(void*),
                                   llg_gc_payload_trace_fn trace);
int llg_gc_register_frame_map(const llg_co_desc_t* desc,
                              const llg_gc_frame_map_t* map);
// Destructor tag of registered automatic handle cells (`void*` payloads).
void llg_gc_handle_cell_drop(void* cell);
// Payload tracers for descriptor-backed containers (dynamic arrays, queues,
// associative arrays), for generated container destructors.
void llg_gc_trace_dyn_values(const void* payload, llg_gc_tracer_t* tracer);
void llg_gc_trace_queue_values(const void* payload, llg_gc_tracer_t* tracer);
void llg_gc_trace_assoc_values(const void* payload, llg_gc_tracer_t* tracer);
// Foreign retention: a pinned object is a root. Pin/unpin of a non-object
// returns 0 and changes nothing; an unbalanced unpin is fatal.
int llg_gc_pin(void* handle);
int llg_gc_unpin(void* handle);
// Collect now. Only the scheduler's safe points and embeddings without a
// running process may call it.
void llg_gc_collect(void);
void llg_gc_get_stats(llg_gc_stats_t* stats);
// Finalize and free every object (model close, after llg_rt_cleanup), print
// statistics when requested and reset the collector.
void llg_gc_teardown(void);

// Typed display values. The runtime owns string members after a display call
// or while a deferred monitor/strobe snapshot is live.
enum {
    LLG_FMT_PACKED = 0,
    LLG_FMT_REAL = 1,
    LLG_FMT_STRING = 2,
    // A net's strength view (see llg_net_t.strength): one byte per bit,
    // formatted only by `%v`.
    LLG_FMT_STRENGTH = 3,
    // Conversion text already rendered by the model (a `%p` pattern),
    // printed verbatim; owned like LLG_FMT_STRING.
    LLG_FMT_TEXT = 4,
};

// Assignment-pattern text (`%p`/`%0p`, SV 21.2.1.7) of one value, described
// by immutable generated tables. `storage` names the root's representation:
// FROM_PACKED `const sv4_t*` (a packed scalar or a flattened aggregate whose
// first member/element is most significant), FROM_REAL `const double*`,
// FROM_STRING `const llg_string_t*`, FROM_HANDLE `void* const*`, FROM_VALUE
// `const llg_value_t*`, FROM_PACKED_CONTAINER a packed dynamic array, queue or
// associative array and FROM_VALUE_CONTAINER its value-element form (selected
// by the type kind), FROM_NESTED_CONTAINER a container nested in a value.
// The source is only read; the returned string is owned by the caller.
enum {
    LLG_PATTERN_PACKED = 0,
    LLG_PATTERN_PACKED_STRUCT = 1,
    LLG_PATTERN_REAL = 2,
    LLG_PATTERN_STRING = 3,
    LLG_PATTERN_FIXED_ARRAY = 4,
    LLG_PATTERN_STRUCT = 5,
    LLG_PATTERN_UNION = 6,
    LLG_PATTERN_QUEUE = 7,
    LLG_PATTERN_DYNAMIC = 8,
    LLG_PATTERN_ASSOC = 9,
    LLG_PATTERN_CLASS = 10,
    LLG_PATTERN_CHANDLE = 11,
    LLG_PATTERN_EVENT = 12,
    LLG_PATTERN_VIRTUAL_INTERFACE = 13,
    LLG_PATTERN_PROCESS = 14,
};

enum {
    LLG_PATTERN_FROM_PACKED = 0,
    LLG_PATTERN_FROM_REAL = 1,
    LLG_PATTERN_FROM_STRING = 2,
    LLG_PATTERN_FROM_HANDLE = 3,
    LLG_PATTERN_FROM_VALUE = 4,
    LLG_PATTERN_FROM_PACKED_CONTAINER = 5,
    LLG_PATTERN_FROM_VALUE_CONTAINER = 6,
    LLG_PATTERN_FROM_NESTED_CONTAINER = 7,
};

// Nested class objects printed by one `%p` before `(...)` replaces deeper
// objects; it bounds the walk's chain and native recursion.
#define LLG_PATTERN_MAX_DEPTH 64u
// Longest `%p` text; longer output is truncated with a warning (the LRM
// requires at least 1024 characters).
#define LLG_PATTERN_OUTPUT_LIMIT ((size_t)1u << 20)

// Types refer to each other by index into one model table;
// LLG_PATTERN_NO_TYPE selects a storage's generic form.
#define LLG_PATTERN_NO_TYPE UINT32_MAX
typedef struct llg_pattern_type llg_pattern_type_t;
typedef struct {
    const char* name;
    uint32_t type;
} llg_pattern_member_t;
// `words` holds the member value's bits, x and z limbs (3 * limbs entries).
typedef struct {
    const char* name;
    const uint64_t* words;
} llg_pattern_enum_t;
// `flat_width` is the packed or flattened width; `count` the members, enum
// members or fixed dimensions (`bounds` holds left/right per dimension).
struct llg_pattern_type {
    uint8_t kind;
    uint8_t signed_flag;
    uint8_t shortreal;
    uint32_t flat_width;
    size_t count;
    const llg_pattern_member_t* members;
    const llg_pattern_enum_t* enums;
    const int32_t* bounds;
    uint32_t element;
};
// One property of a class layout, base class properties first. A NULL name
// marks a hidden member (a randomization mode bit) that is not displayed.
typedef struct {
    const char* name;
    uint32_t type;
    uint8_t storage;
} llg_pattern_field_t;
typedef struct {
    size_t count;
    const llg_pattern_field_t* fields;
} llg_pattern_class_t;
// The model's view of a live object: its class id and property count, or 0
// for a reclaimed object; and the storage of property `index`.
typedef int (*llg_pattern_object_fn)(void* handle, uint32_t* class_id,
                                     size_t* count);
typedef const void* (*llg_pattern_field_fn)(void* handle, size_t index);
void llg_pattern_set_classes(const llg_pattern_class_t* classes, uint32_t count,
                             llg_pattern_object_fn object,
                             llg_pattern_field_fn field);
// Class field types index the table passed to llg_pattern_format.
llg_string_t llg_pattern_format(const llg_pattern_type_t* types,
                                uint32_t type_count, uint32_t type,
                                uint8_t storage, const void* source,
                                int abbreviated);

enum {
    LLG_SEVERITY_INFO = 0,
    LLG_SEVERITY_WARNING = 1,
    LLG_SEVERITY_ERROR = 2,
    LLG_SEVERITY_FATAL = 3,
};

enum {
    LLG_ASSERTION_ASSERT = 0,
    LLG_ASSERTION_ASSUME = 1,
    LLG_ASSERTION_COVER = 2,
    LLG_ASSERTION_EXPECT = 3,
    /* Registration-only kind accepted by the sequence registrations: a
     * `cover sequence` is a cover that reports every match of an attempt
     * (IEEE 1800-2009 16.15.3). Runtime records keep LLG_ASSERTION_COVER. */
    LLG_ASSERTION_COVER_SEQUENCE = 4,
};

enum {
    LLG_ASSERTION_CONTROL_ON = 0,
    LLG_ASSERTION_CONTROL_OFF = 1,
    LLG_ASSERTION_CONTROL_KILL = 2,
    LLG_ASSERTION_CONTROL_FULL = 3,
    /* Action controls (IEEE 1800-2009 20.12) take the same level/selector
     * arguments as ON/OFF/KILL and change only which actions execute. */
    LLG_ASSERTION_CONTROL_PASS_ON = 4,
    LLG_ASSERTION_CONTROL_PASS_OFF = 5,
    LLG_ASSERTION_CONTROL_FAIL_ON = 6,
    LLG_ASSERTION_CONTROL_FAIL_OFF = 7,
    LLG_ASSERTION_CONTROL_NONVACUOUS_ON = 8,
    LLG_ASSERTION_CONTROL_VACUOUS_OFF = 9,
};

/* Result of llg_immediate_assertion_controls: the assertion is checked, and
 * whether its pass and fail actions currently execute. */
#define LLG_IMMEDIATE_CHECKED 1
#define LLG_IMMEDIATE_PASS_ACTION 2
#define LLG_IMMEDIATE_FAIL_ACTION 4

typedef struct {
    int kind;
    // Physical unit of a packed/real `%t` argument in femtoseconds.  Zero
    // keeps the legacy ABI's design-precision fallback for hand-written
    // runtime callers; generated display evaluators always set it.
    uint64_t time_unit_fs;
    union {
        sv4_t packed;
        double real;
        llg_string_t string;
    } value;
} llg_fmt_arg_t;

typedef struct {
    int kind;
    void* ptr;
} llg_display_read_t;

// `kind` of a trigger entry that names no storage: the monitor re-evaluates
// at every settled time slot because an argument reads through a class or
// virtual-interface handle. Other kinds are llg_fmt_kind values.
#define LLG_MONITOR_READ_POLL 100

// ── Collapsed inout nets ──────────────────────────────────────────────────────
//
// An inout port collapses its parent and child nets into ONE simulated net
// (IEEE 1800-2017 §23.3.3.7): every side writes a per-driver slot and readers
// see the wire/tri resolution of all slots. Standalone continuous-assignment
// groups carry per-slot drive-strength endpoints; collapsed inout and wired
// groups carry their per-slot strength endpoints through the same resolver.
// TRI0/TRI1 and SUPPLY0/SUPPLY1 retain their implicit pull/supply source.
// All-Z is produced only when no explicit or implicit source is active;
// equal-strength conflicts resolve according to the net's wired rule.
//
// The struct is a valid file-scope static initializer: driver cells are
// separate `sv4_t` globals whose addresses the codegen wires into `drivers`.
// `drivers`, `strength0` and `strength1` point at exact elaborated-size
// read-only tables emitted with the model. `driver_index` and
// `overlap_scratch` point at exact-size mutable arrays, so range queries do not
// allocate and no fixed driver ceiling exists. The alias list is grown on
// demand; `alias_capacity` tracks its allocation.

typedef struct llg_inertial llg_inertial_t;
llg_inertial_t** llg_fixed_array_inertial(sv4_t*, const void*);
typedef struct llg_net llg_net_t;
typedef struct llg_net_driver_index llg_net_driver_index_t;
typedef struct llg_net_alias_part llg_net_alias_part_t;
typedef struct llg_net_alias llg_net_alias_t;

// One intrusive interval-tree node per driver. Generated models provide an
// exact-size mutable array; no node is allocated while a contribution moves.
// Inactive drivers are electrically all-Z and are absent from the tree.
struct llg_net_driver_index {
    uint32_t low;
    uint32_t high;
    uint32_t max_high;
    uint32_t priority;
    int left;
    int right;
    uint8_t active;
};

struct llg_net {
    sv4_t resolved;                       /* what readers/waiters see */
    uint32_t width;
    int8_t is_signed;
    int8_t resolution;
    int n_drivers;
    sv4_t* const* drivers;                /* per-driver contribution cells */
    const uint8_t* strength0;             /* per-driver source drive levels */
    const uint8_t* strength1;
    llg_net_driver_index_t* driver_index; /* exact-size interval-tree nodes */
    int* overlap_scratch;                 /* exact-size query output */
    int index_root;
    int8_t propagation_enabled;
    llg_inertial_t* propagation;
    uint64_t propagation_rise;
    uint64_t propagation_fall;
    uint64_t propagation_turn_off;
    /* A vector net delay applies to each bit on its own (IEEE 1364-2001
     * 6.1.3): one inertial handle per bit, emitted only for delayed nets
     * wider than one bit. NULL keeps the whole-net handle above. */
    llg_inertial_t** propagation_bits;
    int n_aliases;
    int alias_capacity;
    llg_net_alias_t** aliases;
    /* Optional strength view, emitted only for nets with a strength consumer
     * (`%v`). Each net bit owns one two-state byte: the high nibble is
     * hi + 7 and the low nibble lo + 7, where [lo, hi] is the resolved
     * strength range on the signed scale (-7 = Su0 .. 0 = HiZ .. 7 = Su1).
     * It is published after every resolution, so strength-only changes wake
     * its readers while ordinary value readers stay asleep. */
    sv4_t* strength;
    /* Module paths into this net (IEEE 1800-2009 30.4), bound by
     * llg_module_paths_start. NULL for every net without a specify path, so
     * generated initializers leave it out. A bound net also has
     * propagation_enabled and a per-bit propagation_bits array. */
    struct llg_module_path_dest* path_dest;
};

/* One contiguous run: view bits [signal_bit, signal_bit + bit_count) map to
 * group bits [group_bit, group_bit + bit_count) of one driver slot. Runs keep
 * declared-view metadata proportional to connected ranges, not bits. */
struct llg_net_alias_part {
    llg_net_t* net;
    int slot;
    uint32_t signal_bit;
    uint32_t group_bit;
    uint32_t bit_count;
};

struct llg_net_alias {
    sv4_t* storage;
    sv4_t visible;
    uint32_t width;
    int8_t is_signed;
    const llg_net_alias_part_t* parts;
    uint32_t n_parts;
    /* Optional array-cell publication target; borrowed from model storage. */
    sv4_t* publication_target;
};

void llg_net_resolve(llg_net_t* net); /* strength-aware resolution, per limb */
void llg_net_write(llg_net_t* net, int idx, sv4_t value);
// Selected writes replace the driver's previous range, so a moving selector
// releases its old contribution to Z. `value` has plan.width bits; reverse is
// true for ascending ordinary part-selects.
void llg_net_write_selected(llg_net_t* net, int idx, sv4_t value,
                            sv4_select_plan_t plan, int reverse);
void llg_net_index_reset(llg_net_t* net);
void llg_net_strength_reset(llg_net_t* net); /* startup view, no wakeups */
void llg_net_alias_bind(llg_net_alias_t* alias);
void llg_net_alias_clear(llg_net_t* net); /* release a model net's alias list */
sv4_t llg_net_alias_read(llg_net_alias_t* alias);
void llg_net_alias_write(llg_net_alias_t* alias, sv4_t value);

// The runtime owns each inertial driver and its pending event. The caller's
// initially NULL handle, target and net must persist until cleanup, which
// resets the handle to NULL. Repeated evaluation never suspends the caller.
void llg_inertial_assign(llg_inertial_t** handle, sv4_t* target,
                         sv4_t value, uint64_t rise, uint64_t fall,
                         uint64_t turn_off);
void llg_inertial_net(llg_inertial_t** handle, llg_net_t* net, int slot,
                      sv4_t value, uint64_t rise, uint64_t fall,
                      uint64_t turn_off);
void llg_inertial_selected_assign(llg_inertial_t** handle, sv4_t* target,
                                  sv4_t value, sv4_t mask, uint64_t rise,
                                  uint64_t fall, uint64_t turn_off);
void llg_inertial_selected_net(llg_inertial_t** handle, llg_net_t* net,
                               int slot, sv4_t value, sv4_t mask,
                               uint64_t rise, uint64_t fall,
                               uint64_t turn_off);

// ── Module path delays ────────────────────────────────────────────────────────

/* Simple module paths (IEEE 1364-2001 14.2-14.4, IEEE 1800-2009 30.4-30.6).
 * A generated model emits one set per module instance that declares paths.
 * Terminals are storage-bit ranges (bit 0 is the least significant storage
 * bit). `delays` holds scheduler ticks per transition in the order
 * 0->1, 1->0, 0->z, z->1, 1->z, z->0, 0->x, x->1, 1->x, x->0, x->z, z->x and
 * stays writable so back-annotation can replace it; a new value applies to
 * the next scheduled destination transition. */
#define LLG_PATH_TRANSITIONS 12

typedef struct llg_module_path {
    uint32_t source;       /* index into the set's sources */
    uint32_t source_lsb;
    uint32_t source_width;
    uint32_t dest_lsb;
    uint32_t dest_width;
    uint8_t full;          /* 1: `*>` every bit to every bit, 0: `=>` */
    uint8_t polarity;      /* 0 unknown, 1 positive, 2 negative (metadata) */
    uint64_t delays[LLG_PATH_TRANSITIONS];
    const char* origin;    /* "file:line:col" of the declaration */
} llg_module_path_t;

typedef struct llg_module_path_set llg_module_path_set_t;

typedef struct llg_module_path_dest {
    llg_net_t* net;
    llg_module_path_t* paths;
    uint32_t n_paths;
    /* Runtime state, owned between llg_module_paths_start and cleanup. */
    llg_module_path_set_t* set;
    uint64_t* path_stamps; /* per path: latest change of a full source range */
} llg_module_path_dest_t;

struct llg_module_path_set {
    const char* scope;     /* hierarchical instance name */
    sv4_t* const* sources; /* borrowed model storage of each source signal */
    uint32_t n_sources;
    llg_module_path_dest_t* dests;
    uint32_t n_dests;
    /* Runtime state, owned between llg_module_paths_start and cleanup. */
    sv4_t* last;           /* per source: value at the last refresh */
    uint64_t** stamps;     /* per source bit: time of its last change */
    llg_module_path_set_t* next;
};

/* Bind the set's destination nets and take the current source values as the
 * reference for change detection. Called once per set during model
 * initialization, after net and signal storage exist; the runtime releases
 * the state in llg_rt_cleanup. A source that has not changed since counts as
 * changed at time 0. */
void llg_module_paths_start(llg_module_path_set_t* set);
/* Record the current time for every source bit that changed since the last
 * record. Generated hidden processes call it whenever a source changes. */
void llg_module_path_stamp(llg_module_path_set_t* set);
/* The bound sets, most recently started first, linked through `next`. */
llg_module_path_set_t* llg_module_path_sets(void);
/* 1 when bit `bit` of a path destination has a scheduled update; then its
 * absolute time and four-state value (0, 1, 2 = x, 3 = z) are stored. */
int llg_net_path_pending(const llg_net_t* net, uint32_t bit, uint64_t* time,
                         int* state);

// ── Scheduler ─────────────────────────────────────────────────────────────────

// Registered owners scoped to a process (or the root runtime). Heap cells
// can escape lexical exit; caller-owned cells must not. Completion/cancellation
// drains scopes before coroutine frame/arena storage is reclaimed. End scopes normally
// as soon as their values die, not only during runtime teardown.
typedef struct llg_proc llg_proc_t;
// Runtime-owned links and payload ownership. Caller-owned nodes are initialized
// by registration, never copied while active, and remain live until scope end.
typedef struct llg_value_scope {
    struct llg_value_scope* next;
    struct llg_value_scope* all_next;
    struct llg_value_scope* all_prev;
    size_t references;
    int active;
    int intrusive;
    llg_proc_t* owner;
    size_t count;
    sv4_t* values;
    void* object;
    void (*destroy_object)(void*);
} llg_value_scope_t;
llg_value_scope_t* llg_value_scope_begin(size_t count);
// Register caller-owned empty cells/node without allocating either. Their
// addresses must outlive registration and must not escape lexical scope exit.
// End destroys payloads and removes exact descriptor keys, but never frees the
// node/cells. The same storage may be registered again after end/unwind.
llg_value_scope_t* llg_value_scope_register(llg_value_scope_t* node,
                                          sv4_t* values, size_t count);
// As above for zero-initialized native payloads. Destructor borrows the payload.
llg_value_scope_t* llg_value_scope_register_object(llg_value_scope_t* node,
    void* object, void (*destroy)(void*));
// The allocated or caller-owned array neither moves nor is replaced while
// registered; generated coroutines rely on this to keep the pointer in a C
// local reloaded from the frame after each resume.
sv4_t* llg_value_scope_values(llg_value_scope_t* scope);
void llg_value_scope_end(llg_value_scope_t* scope);
// Zeroed native storage shares lexical/nonlocal cleanup with packed scopes.
// The destructor borrows the storage; it must not free it, yield or notify HDL.
llg_value_scope_t* llg_value_scope_begin_object(size_t size, void (*destroy)(void*));
void* llg_value_scope_object(llg_value_scope_t* scope);
// A mark borrows the current active-scope head. It is valid until its enclosing
// scope ends. End-since performs lexical return/goto cleanup, excluding mark.
llg_value_scope_t* llg_value_scope_mark(void);
void llg_value_scopes_end_since(llg_value_scope_t* mark);

typedef struct llg_process_handle llg_process_handle_t;
typedef struct llg_semaphore llg_semaphore_t;
typedef struct llg_frame llg_frame_t;
typedef struct llg_activation llg_activation_t;
typedef struct llg_mailbox llg_mailbox_t;
typedef struct { sv4_t* sig; int kind; } llg_event_spec_t;
// One typed storage dependency. Exactly one pointer is non-null; real
// dependencies point directly at the generated double companion. Real
// equality follows the write path's bitwise comparison and never converts
// through packed storage.
typedef struct {
    sv4_t* sig;
    double* real;
    sv4_t* value; /* Optional value behind a change-marker signal. */
    uint32_t lsb;
    uint32_t width; /* Zero selects the whole storage. */
} llg_wait_dependency_t;

// ── Mailboxes (IEEE 1800-2009 §15.4) ────────────────────────────────────────
// Message and destination descriptors are borrowed for one operation. A
// string in a message descriptor is owned by the descriptor until the
// runtime either queues/delivers it or destroys it on a failed try operation;
// packed and handle values are copied by value/identity respectively.
// LLG_MAILBOX_VALUE messages (SIM-017) own a recursive descriptor-backed
// value: records, unpacked arrays and whole queues/dynamic arrays. Their
// copies are deep except for identity handles, exactly as llg_value_t.
enum {
    LLG_MAILBOX_PACKED = 0,
    LLG_MAILBOX_REAL = 1,
    LLG_MAILBOX_STRING = 2,
    LLG_MAILBOX_HANDLE = 3,
    LLG_MAILBOX_UNTYPED = 4,
    LLG_MAILBOX_VALUE = 5,
};

typedef struct {
    int kind;
    // Zero: structural scalar. Otherwise the canonical nominal type (enums,
    // classes) or, for LLG_MAILBOX_VALUE, the model's equivalence class of
    // the aggregate type (SV 6.22.2), decided when the model was generated.
    uint64_t type_id;
    uint32_t width;
    int8_t is_signed;
    int8_t two_state;
    int8_t shortreal;
    union {
        sv4_t packed;
        double real;
        llg_string_t string;
        void* handle;
        llg_value_t native;
    } value;
} llg_mailbox_value_t;

typedef struct {
    int kind;
    uint64_t type_id; // as llg_mailbox_value_t.type_id
    uint32_t width;
    int8_t is_signed;
    int8_t two_state;
    int8_t shortreal;
    llg_ref_t* reference; // optional packed destination descriptor
    union {
        sv4_t* packed;
        double* real;
        llg_string_t* string;
        void** handle;
        llg_value_t* native; // an initialized value of an equivalent type
    } target;
} llg_mailbox_target_t;

llg_mailbox_value_t llg_mailbox_typed_value(llg_mailbox_value_t value, uint64_t type_id);
llg_mailbox_target_t llg_mailbox_typed_target(llg_mailbox_target_t target, uint64_t type_id);
llg_mailbox_target_t llg_mailbox_target_ref(llg_ref_t* target);
llg_mailbox_value_t llg_mailbox_value_packed(sv4_t value, uint32_t width,
                                              int is_signed, int two_state);
llg_mailbox_value_t llg_mailbox_value_real(double value, int shortreal);
llg_mailbox_value_t llg_mailbox_value_string(llg_string_t value);
llg_mailbox_value_t llg_mailbox_value_handle(void* value);
llg_mailbox_target_t llg_mailbox_target_packed(sv4_t* target, uint32_t width,
                                                int is_signed, int two_state);
llg_mailbox_target_t llg_mailbox_target_real(double* target, int shortreal);
llg_mailbox_target_t llg_mailbox_target_string(llg_string_t* target);
llg_mailbox_target_t llg_mailbox_target_handle(void** target);
// Deep copy of `value` (borrowed) as a message of aggregate type `type_id`.
llg_mailbox_value_t llg_mailbox_value_native(const llg_value_t* value,
                                             uint64_t type_id);
llg_mailbox_target_t llg_mailbox_target_native(llg_value_t* target,
                                               uint64_t type_id);
llg_mailbox_t* llg_mailbox_new(sv4_t bound, int kind, uint32_t width,
                               int is_signed, int two_state, int shortreal);
// A typed mailbox of an aggregate message type (LLG_MAILBOX_VALUE).
llg_mailbox_t* llg_mailbox_new_value(sv4_t bound, uint64_t type_id);
uint64_t llg_mailbox_num(const llg_mailbox_t* mailbox);
// Blocking put/get are one-shot arms. READY means the transfer completed
// synchronously; SUSPEND means the runtime owns the queued value until
// wake/cancellation; EXIT means a blocking type/null error requested
// LLG_EXIT_COMPLETE.
//
// A blocked get completes in one defined order (SIM-017): when a message
// arrives, the runtime checks its type against the waiter's destination,
// removes it from the queue (a peek copies it) into the waiter's pending
// delivery, and wakes the waiter. The waiter writes its destination only
// when it resumes, through llg_mailbox_delivery_take with the same
// destination descriptor, so the write always lands in live storage. A
// waiter killed, disabled or torn down before it resumes hands a consumed
// message back to the head of its mailbox (a peek copy is dropped), so no
// message is lost, duplicated or written into dead storage.
llg_co_arm_t llg_arm_mailbox_put_value(llg_proc_t* self,
                                       llg_mailbox_t* mailbox,
                                       llg_mailbox_value_t value);
int llg_mailbox_try_put_value(llg_mailbox_t* mailbox,
                              llg_mailbox_value_t value);
llg_co_arm_t llg_arm_mailbox_get_value(llg_proc_t* self,
                                       llg_mailbox_t* mailbox,
                                       llg_mailbox_target_t target, int peek);
int llg_mailbox_try_get_value(llg_mailbox_t* mailbox,
                              llg_mailbox_target_t target, int peek);
// Called after a blocking get/peek arm: writes a pending delivery (if the
// arm suspended) into `target`. A no-op when the arm completed synchronously.
void llg_mailbox_delivery_take(llg_proc_t* self, llg_mailbox_target_t target);
// Tracing roots for a collector (SIM-018): visit every identity-handle slot
// held by queued messages, blocked putters' messages and pending deliveries.
// Handle-kind messages pass a NULL descriptor (an opaque identity whose
// class the mailbox does not record); value messages are traced through
// llg_value_trace. The callback must not mutate mailbox state.
void llg_mailbox_trace(llg_value_visit_fn visit, void* context);

// Execution regions, in reference-algorithm order. PLI control points are
// explicit even when no public VPI registration has been lowered yet. The
// semantic aliases keep generated Verilog-facing code readable.
typedef enum {
    LLG_REGION_PREPONED = 0,
    LLG_REGION_PREPONED_PLI,
    LLG_REGION_PRE_ACTIVE_PLI,
    LLG_REGION_ACTIVE,
    LLG_REGION_INACTIVE,
    LLG_REGION_PRE_NBA_PLI,
    LLG_REGION_PRE_NBA,
    LLG_REGION_NBA,
    LLG_REGION_POST_NBA,
    LLG_REGION_POST_NBA_PLI,
    LLG_REGION_PRE_OBSERVED_PLI,
    LLG_REGION_PRE_OBSERVED,
    LLG_REGION_OBSERVED,
    LLG_REGION_POST_OBSERVED,
    LLG_REGION_POST_OBSERVED_PLI,
    LLG_REGION_REACTIVE,
    LLG_REGION_RE_INACTIVE,
    LLG_REGION_PRE_RE_NBA_PLI,
    LLG_REGION_PRE_RE_NBA,
    LLG_REGION_RE_NBA,
    LLG_REGION_POST_RE_NBA,
    LLG_REGION_POST_RE_NBA_PLI,
    LLG_REGION_PRE_POSTPONED_PLI,
    LLG_REGION_PRE_POSTPONED,
    LLG_REGION_POSTPONED,
    LLG_REGION_POSTPONED_PLI,
    LLG_REGION_COUNT
} llg_region_t;

#define LLG_REGION_NONBLOCKING_ASSIGN LLG_REGION_NBA
#define LLG_REGION_RE_NONBLOCKING_ASSIGN LLG_REGION_RE_NBA

// The live process registry grows on demand; this constant survives only as
// the standalone runtime self-test's sequential-fork iteration base and must
// not be treated as a concurrency ceiling.
#define LLG_MAX_PROCS 4096

// Stable SystemVerilog process states. The numeric order is the declaration
// order of the LRM `process::state` enum and is part of the generated-model
// ABI: FINISHED=0, RUNNING=1, WAITING=2, SUSPENDED=3, KILLED=4.
enum {
    LLG_PROCESS_FINISHED = 0,
    LLG_PROCESS_RUNNING = 1,
    LLG_PROCESS_WAITING = 2,
    LLG_PROCESS_SUSPENDED = 3,
    LLG_PROCESS_KILLED = 4,
};

// Event kinds used by llg_event_spec_t.
enum {
    LLG_EV_ANY = 0,     // any change
    LLG_EV_POSEDGE = 1, // 0->1, 0->X, X->1
    LLG_EV_NEGEDGE = 2, // 1->0, 1->X, X->0
};

void llg_rt_init(void);
// Initialize the runtime and retain the generated model's argv view for
// `$test$plusargs`/`$value$plusargs`. The runtime never takes ownership of
// `argv`; callers keep it valid for the duration of the simulation.
void llg_rt_init_with_args(int argc, char** argv);
// Initialize a runtime whose scheduler ticks represent `precision_fs`
// femtoseconds. Generated models use this entry point so the default
// `$timeformat` unit is the design precision, while standalone callers retain
// the one-femtosecond default through llg_rt_init().
void llg_rt_init_with_precision(uint64_t precision_fs);
// Combine the generated model's command-line view with its scheduler tick
// precision. The runtime borrows `argv` for the duration of the simulation.
void llg_rt_init_with_args_and_precision(int argc, char** argv,
                                         uint64_t precision_fs);
// Request a scheduler stop after the current process turn returns. Generated
// functions and VPI vpiStop use this deferred form because they cannot suspend
// a stackless coroutine at their own call depth.
void llg_rt_request_stop(int verbosity, const char* location);
// Release all runtime-owned scheduler, coroutine, fork-group, monitor and
// strobe allocations. Call only when no runtime coroutine is executing; init
// and run invoke it automatically. Repeated calls are safe.
void llg_rt_cleanup(void);
// Run until $finish, a deadlock, all processes ending, or a `$stop` whose
// policy is `exit`. The default stop policy is `resume`, which automatically
// resumes the stopped process at the same simulation time so noninteractive
// command-line runs cannot hang waiting for input. An embedding may select
// `exit`, inspect `llg_rt_is_suspended`, call `llg_rt_resume`, and invoke
// `llg_rt_run` again.
void llg_rt_run(void);
// True when the runtime stopped because of a configuration, nonconvergence,
// or other controlled simulation failure.  The result survives cleanup.
int llg_rt_failed(void);
// Region currently being drained. During initialization this is PREPONED.
llg_region_t llg_current_region(void);
// Return a stable printable name for diagnostics and callback traces.
const char* llg_region_name(llg_region_t region);
// True for sampling/observation/output regions while the scheduler is running.
int llg_region_is_read_only(void);
// Request scheduler termination from a native callback and return through the
// foreign frame. When a process is current (for example during VPI calltf),
// also set its chain to LLG_EXIT_COMPLETE; the generated native-call boundary
// performs the Terminate check after the callback returns.
void llg_rt_request_finish(void);
// Mark a simulator-generated failure before terminating through a severity task.
void llg_rt_mark_failed(void);
// Request normal completion of the current process and scheduler. These
// services set chain.exiting=LLG_EXIT_COMPLETE and return. Coroutine callers
// must immediately use LLG_CO_EXIT_CHECK; plain functions must immediately
// test llg_rt_exiting() and branch to their unwind return label.
void llg_rt_finish(void);
void llg_rt_finish_with_level(int verbosity, const char* location);
// True only while the current process or transient final record has a pending
// COMPLETE/ABANDON exit. This out-of-line query is for plain generated
// functions; coroutine code reads its chain with LLG_CO_EXIT_CHECK.
int llg_rt_exiting(void);
// Deferred stop for generated functions and vpi_control(vpiStop). Reports the
// stop now and, when called during a scheduler process turn, records a request
// that the scheduler observes immediately after llg_co_run returns and before
// any other process, region, or callback work. No continuation is requeued.
// Outside a scheduler process turn (including finals and idle embeddings), the
// reported request is ignored with a warning. Coroutine-body `$stop` uses
// llg_arm_stop so it can preserve its exact continuation.
void llg_rt_request_stop(int verbosity, const char* location);
// `$stop` as a deferred assertion action (SV 16.4), which runs as a Reactive
// callback rather than a process turn. Reports the stop and suspends the
// scheduler after the action returns; the remaining matured reports run when
// it resumes. During $finish/final draining it only reports.
void llg_rt_action_stop(int verbosity, const char* location);
// vpi_control(vpiStop/vpiFinish) from any VPI context (SV 38.4: the task runs
// "upon return of the application routine"). A stop inside a process turn is
// the deferred llg_rt_request_stop; inside a region callback it is
// llg_rt_action_stop. Both return 0, reporting nothing, when no running and
// unfinished scheduler can take the request. `verbosity` must be 0..2.
int llg_rt_control_stop(int verbosity);
// Report the `$finish` message for `verbosity` (0..2) once and request
// termination like llg_rt_request_finish. Returns 0 when a finish is already
// pending.
int llg_rt_control_finish(int verbosity);
// The time at which the last run ended (the time final procedures see), or
// the current time while the scheduler state is live (running or suspended).
uint64_t llg_rt_end_time(void);

// Value-change watches (VPI cbValueChange). A watch on a published packed or
// real storage address makes every changing publication of that address call
// the installed hook after all runtime consumers ran. Storage without a watch
// pays nothing: publication already finds the address's wait row. Watches
// are counted per address and end with the runtime lifetime (cleanup);
// removing a watch from an earlier lifetime is a no-op.
typedef void (*llg_value_watch_fn)(const void* key);
// True while llg_rt_run drains the scheduler (not during startup, after the
// run ended, while suspended or in final procedures).
int llg_rt_running(void);
// Hook called at the start of every Pre-Active PLI pass (each scheduler
// iteration of a time slot, SV 4.10), with the current region set to
// LLG_REGION_PRE_ACTIVE_PLI so it may queue zero-delay callbacks there.
void llg_time_slot_hook(void (*hook)(void));
void llg_value_watch_hook(llg_value_watch_fn hook);
int llg_value_watch_add(const void* key);
void llg_value_watch_remove(const void* key);

enum {
    LLG_STOP_POLICY_RESUME = 0,
    LLG_STOP_POLICY_EXIT = 1,
};

// Select how `$stop` behaves when the scheduler reaches the stop point. This
// may be called before `llg_rt_init` or while the runtime is suspended. It
// returns zero for an invalid policy or a running scheduler and one on
// success. `LLG_STOP_POLICY_RESUME` is the default.
int llg_rt_set_stop_policy(int policy);
int llg_rt_stop_policy(void);
// True after a stop with the EXIT policy suspended scheduling. An arm stop
// also retains a process continuation; a deferred stop has no continuation.
// Scheduler state remains live until `llg_rt_resume` or cleanup.
int llg_rt_is_suspended(void);
// Continue after a stopped scheduler. An arm stop queues its continuation at
// the same simulation time; a deferred stop simply resumes scheduler work.
// Returns one when a stop was resumed and zero when none is pending.
int llg_rt_resume(void);
uint64_t llg_time(void);              // current tick count
// Current time rounded to the nearest local unit; exact half units round up.
// The caller applies any result-width conversion (for example, $stime's
// low-32-bit result) after this operation.
/* Physical femtoseconds represented by one scheduler tick. */
uint64_t llg_time_precision_fs(void);
uint64_t llg_time_scaled(uint64_t precision_fs, uint64_t unit_fs);
// Set the design-wide `$timeformat` state. The suffix is consumed by the
// runtime on both success and controlled failure. Arguments are evaluated by
// generated code before this call, in source order.
void llg_timeformat(sv4_t units, sv4_t precision, llg_string_t suffix,
                    sv4_t minimum_field_width);
// Diagnostic count of allocated process objects, including completed fork
// parents retained while detached descendants are still live.
int llg_rt_process_count(void);

// ── IEEE stochastic analysis queues ─────────────────────────────────────────
//
// These queues implement the Verilog stochastic analysis system tasks
// (IEEE 1364-2001 §17.6 / IEEE 1800-2009 §20.16). They are deliberately
// separate from SystemVerilog queue containers: each queue stores a job ID,
// an information ID, and the simulation tick at which the job arrived.
// Integer arguments are checked four-state values; unknown or out-of-range
// values are reported as a controlled runtime failure instead of being
// silently truncated. Output values are written through the ordinary
// procedural-write path so force rules remain consistent with HDL.
enum {
    LLG_Q_OK = 0,
    LLG_Q_FULL = 1,
    LLG_Q_UNKNOWN_ID = 2,
    LLG_Q_EMPTY = 3,
    LLG_Q_BAD_TYPE = 4,
    LLG_Q_BAD_LENGTH = 5,
    LLG_Q_DUPLICATE_ID = 6,
    LLG_Q_NO_MEMORY = 7,
};

void llg_q_initialize(sv4_t q_id, sv4_t q_type, sv4_t max_length,
                      sv4_t* status);
void llg_q_add(sv4_t q_id, sv4_t job_id, sv4_t inform_id, sv4_t* status);
void llg_q_remove(sv4_t q_id, sv4_t* job_id, sv4_t* inform_id,
                  sv4_t* status);
sv4_t llg_q_full(sv4_t q_id, sv4_t* status);
void llg_q_exam(sv4_t q_id, sv4_t stat_code, sv4_t* stat_value,
                sv4_t* status);

// ── Process/object random streams ───────────────────────────────────────────
//
// Every generated process owns one stream. A static process derives from the
// initialization stream of the instance that declares it, in that instance's
// creation order; forked children derive from their parent in branch-creation
// order. A draw mutates only its owning stream. Seeds and range bounds read
// X/Z bits as 0 (2-state `int` formals). Shuffle draws from the calling
// thread's stream.
sv4_t llg_urandom(void);
sv4_t llg_urandom_seed(sv4_t seed);
sv4_t llg_urandom_range(sv4_t max, sv4_t min, int has_min);
// `randcase` (IEEE 1800-2009 18.16): picks the index of the item whose share
// of the exact weight sum contains one draw from the calling process stream,
// or -1 (after a warning naming `location`) when every weight is zero. The
// weights are borrowed for the call; `sum_width` is the bit width that holds
// their exact sum and `count` is at least 1.
int32_t llg_randcase_select(const sv4_t* const* weights, size_t count,
                            uint32_t sum_width, const char* location);
// A randsequence production's rule (IEEE 1800-2009 18.17.1): like
// `llg_randcase_select`, but a negative or unknown weight is an error that
// marks the run failed and selects no rule (-1), and the all-zero warning
// names the production.
int32_t llg_randsequence_select(const sv4_t* const* weights, size_t count,
                                uint32_t sum_width, const char* location);
// `rand join` (18.17.5): the index of the next sequence to advance given the
// remaining lengths (unsigned, borrowed) and a ratio in [0, 1], or -1 when
// every length is zero. Draws from the calling process stream.
int32_t llg_randsequence_join_pick(const sv4_t* const* remaining, size_t count,
                                   double ratio);
void llg_process_srandom(sv4_t seed);
llg_string_t llg_process_get_randstate(void);
int llg_process_set_randstate(llg_string_t state);
// Class-object streams. A generated class layout owns the state when the
// model inspects object streams; `llg_object_rng_create` seeds it from the
// creating thread (or only consumes that draw when `object` is NULL). The
// srandom/get/set operations take the object's state; NULL is a no-op for a
// receiver already reported as null. set_randstate consumes `state`.
// A model that observes random streams calls this before spawning: static
// processes then seed from their instance's initialization stream (18.14.1)
// instead of the root stream in model spawn order. Reset by llg_rt_cleanup.
void llg_rt_use_instance_random_streams(void);
// Brackets one static handle initializer of the instance `label` names (its
// last component is dropped): objects created inside seed from that
// instance's initialization stream. `label` must outlive the model.
void llg_rng_initializer_scope_begin(const char* label);
void llg_rng_initializer_scope_end(void);
void llg_object_rng_create(llg_rng_state_t* object);
void llg_object_srandom(llg_rng_state_t* object, sv4_t seed);
llg_string_t llg_object_get_randstate(const llg_rng_state_t* object);
void llg_object_set_randstate(llg_rng_state_t* object, llg_string_t state);

// randomize() glue (IEEE 1800-2009 18.6, 18.12; scheduler/randomize.c).
// Values are resized to `width` (zero extension) and passed as 2-state words;
// an X/Z bit marks the input LLG_CR_INPUT_XZ (S29-D1). A leaf whose
// evaluation met a null handle passes `null_dereference` (18.5.12 E).
void llg_cr_set_var_sv4(llg_cr_session_t* s, uint32_t var, sv4_t value, uint32_t width,
                        int active);
void llg_cr_set_leaf_sv4(llg_cr_session_t* s, uint32_t leaf, sv4_t value, uint32_t width,
                         int null_dereference);
// The solved value of an active variable as an independent owner.
sv4_t llg_cr_value_sv4(const llg_cr_session_t* s, uint32_t var, uint32_t width,
                       int8_t is_signed);
// A randc variable's cycle state (rand_cyclic_state_width bits, 2-state)
// in, and its updated state out after a successful solve (S30-D3).
void llg_cr_set_cyclic_sv4(llg_cr_session_t* s, uint32_t var, sv4_t state, uint32_t width);
sv4_t llg_cr_cyclic_sv4(const llg_cr_session_t* s, uint32_t var, uint32_t width);
/* Problems with units (SIM-030): the instance forms of the above. */
void llg_cr_iset_var_sv4(llg_cr_session_t* s, uint32_t inst, uint32_t var, sv4_t value,
                         uint32_t width, int active);
void llg_cr_iset_leaf_sv4(llg_cr_session_t* s, uint32_t inst, uint32_t leaf, sv4_t value,
                          uint32_t width, int null_dereference);
void llg_cr_iset_cyclic_sv4(llg_cr_session_t* s, uint32_t inst, uint32_t var, sv4_t state,
                            uint32_t width);
void llg_cr_set_elem_sv4(llg_cr_session_t* s, uint32_t inst, uint32_t array, uint32_t index,
                         sv4_t value, uint32_t width);
sv4_t llg_cr_ivalue_sv4(const llg_cr_session_t* s, uint32_t inst, uint32_t var, uint32_t width,
                        int8_t is_signed);
sv4_t llg_cr_icyclic_sv4(const llg_cr_session_t* s, uint32_t inst, uint32_t var, uint32_t width);
sv4_t llg_cr_elem_value_sv4(const llg_cr_session_t* s, uint32_t inst, uint32_t array,
                            uint32_t index, uint32_t width, int8_t is_signed);
// A constraint function's result for the solver (X/Z marks the input).
void llg_cr_set_call_result_sv4(llg_cr_session_t* s, uint32_t call, sv4_t value,
                                uint32_t width);
// One solve step (llg_cr_step) drawing from `stream`, or from the calling
// thread's stream when NULL (S29-D3). Returns 4 while constraint functions
// are ready (LLG_CR_NEED_CALLS); otherwise reports the outcome and returns
// 1 when the caller may commit, 0 after a failure. UNSAT warns; RESOURCE and
// ERROR are run-time errors (exit status 1, S29-D5).
int32_t llg_randomize_step(const llg_cr_problem_t* problem, llg_cr_session_t* s,
                           llg_rng_state_t* stream, const char* site);

// Stable dependency markers used by generated fixed-array and container
// readers. A marker's address remains valid when a resizable container moves
// its backing storage. Bindings are cleared by llg_rt_cleanup.
void llg_dependency_bind(sv4_t* target, sv4_t* dependency);
void llg_dependency_bind_real(double* target, sv4_t* dependency);
// Destroy hook (llg_frame_capture_object) of a change marker that a frame
// slot owns: a fork-shared string that a wait observes. It does not free the
// marker's own storage.
void llg_dependency_marker_destroy(void* marker);
void llg_dependency_changed(sv4_t* dependency);
// `dependency`, or a marker that is never written when it is NULL (an
// activation container no other process can change).
sv4_t* llg_dependency_or_never(sv4_t* dependency);
void llg_dependency_notify(sv4_t* contents, sv4_t* shape, int change);

void llg_display(const char* fmt, ...);  // formatted output followed by a newline
void llg_write(const char* fmt, ...);    // formatted output without a newline
// Typed formatting calls consume packed/string members, but not the argument
// array allocation. Initialize each member as an independent owner.
void llg_display_typed(const char* fmt, llg_fmt_arg_t* args, int n,
                       const char* scope);
void llg_write_typed(const char* fmt, llg_fmt_arg_t* args, int n,
                     const char* scope);
// Format into a newly-owned string. The format value and argument array are
// consumed exactly once, including destruction of every owned packed/string member.
llg_string_t llg_string_format_typed(llg_string_t format, llg_fmt_arg_t* args,
                                     int n, const char* scope);
// Runtime severity tasks use the same typed formatter as display tasks and
// write one source-context diagnostic to stderr. The argument array is
// consumed exactly once, including destruction of owned packed values and strings.
void llg_rt_severity_typed(int severity, const char* fmt, llg_fmt_arg_t* args,
                           int n, const char* scope, const char* location);
// Formats and consumes `args`, requests LLG_EXIT_COMPLETE, and returns. The
// caller must perform the same immediate exit check as after llg_rt_finish.
void llg_rt_fatal_typed(int finish_number, const char* fmt,
                        llg_fmt_arg_t* args, int n,
                        const char* scope, const char* location);
// Counts reset at llg_rt_init and remain available through final-block
// execution. Invalid levels return zero.
uint64_t llg_rt_severity_count(int severity);
// Immediate assertion default-failure and successful-cover callbacks. The
// identity is retained at the call boundary for future per-assertion APIs.
void llg_assertion_failure(int kind, uint64_t identity, const char* label,
                           const char* location);
void llg_assertion_cover(uint64_t identity, const char* label, const char* location);
uint64_t llg_assertion_count(int kind);
uint64_t llg_assertion_vacuous_count(void);
// Records a failure of an immediate assertion whose fail action is
// suppressed by $assertfailoff: the counter changes, nothing is reported.
void llg_assertion_failure_quiet(int kind);
// The control state (LLG_IMMEDIATE_* bits) of a simple immediate assertion
// in `scope`. Emitted only in designs that call an assertion control task.
int llg_immediate_assertion_controls(int kind, const char* label, const char* scope);
// KILL may cancel the current assertion-action process. In that case it sets
// ABANDON, or COMPLETE when program-completion bookkeeping requests finish,
// before returning; generated callers perform an immediate Terminate check.
int llg_assertion_control(int kind, const sv4_t* args, int n_args,
                          const char* const* scopes, int n_scopes);
// Arm a procedural expect (16.18). The caller runs the action block itself
// after it resumes: `has_fail_action` is whether an else arm exists, which
// replaces the default failure report. llg_assertion_expect_outcome then
// returns 0 (ended without a result), 1 (success) or 2 (failure).
int llg_assertion_expect_start(uint64_t identity, int has_fail_action);
int llg_assertion_expect_outcome(uint64_t identity);

// Concurrent assertion callbacks are generated as side-effect-free sampled
// predicates and Reactive-region action processes. The runtime owns the
// attempt queues and never evaluates a property against live NBA state.
typedef int (*llg_concurrent_assertion_predicate_fn)(void* data);
/* Each present action is spawned as a Reactive process through its descriptor.
 * A descriptor is NULL when that action arm is absent. */

// A sequence graph is an owned, finite NFA whose transition delays are
// measured in sampled clock edges.  The graph itself is emitted as static C
// data; the runtime owns only the active token sets.  UINT64_MAX denotes an
// unbounded upper endpoint and UINT32_MAX denotes an epsilon transition.
#define LLG_SEQUENCE_UNBOUNDED UINT64_MAX
#define LLG_SEQUENCE_EPSILON UINT32_MAX
typedef int (*llg_sequence_atom_fn)(uint32_t atom, void* data);
typedef void (*llg_sequence_init_fn)(void* data);
typedef void (*llg_sequence_match_fn)(uint32_t item, void* data);
typedef struct {
    uint32_t from;
    uint32_t to;
    uint64_t min_delay;
    uint64_t max_delay;
    /* Generated model storage owns the pointer. NULL inherits the registered
     * assertion clock; a present pointer identifies this segment's direct
     * sampled clock and `edge`. */
    sv4_t* clock;
    int edge;
    uint32_t atom;
    uint32_t match_start;
    uint32_t match_count;
    uint32_t enter_scope;
    uint32_t exit_scope;
    /* One-based index into the graph's join table; zero means no action. */
    uint32_t enter_join;
    uint32_t exit_join;
} llg_sequence_transition_t;
/* A join forks one thread into each operand at its enter edge and pairs the
 * operand endpoints at the exit edges: AND ends at the later operand
 * endpoint, INTERSECT needs both operands to end on the same clock tick.
 * An operand admitting the empty word counts as matched at the fork for AND. */
#define LLG_SEQUENCE_JOIN_AND 1u
#define LLG_SEQUENCE_JOIN_INTERSECT 2u
typedef struct {
    uint32_t kind;
    uint32_t left_start;
    uint32_t right_start;
    uint8_t left_empty;
    uint8_t right_empty;
} llg_sequence_join_t;
typedef struct {
    uint32_t states;
    uint32_t start;
    uint32_t accept;
    uint32_t transition_count;
    const llg_sequence_transition_t* transitions;
    uint32_t first_match_state_count;
    const uint32_t* first_match_states;
    llg_sequence_atom_fn atom;
    void* data;
    llg_sequence_init_fn init;
    int first_match;
    uint32_t local_count;
    const struct llg_sequence_local* locals;
    uint32_t match_item_count;
    llg_sequence_match_fn match;
    int admits_empty;
    sv4_t* leading_clock;
    int leading_edge;
    uint32_t join_count;
    const llg_sequence_join_t* joins;
} llg_sequence_graph_t;
typedef struct llg_sequence_local {
    uint32_t width;
    int8_t is_signed;
    uint8_t two_state;
    uint64_t declaration;
} llg_sequence_local_t;
/* Match-item callbacks use these helpers to address storage owned by their
 * current sequence attempt. The runtime validates the opaque attempt/slot
 * pair before returning a pointer, so generated callbacks cannot escape the
 * attempt's lifetime. */
int llg_sequence_local_inherited(void* attempt, uint32_t slot);
sv4_t* llg_sequence_local_addr(void* attempt, uint32_t slot);
sv4_t llg_sequence_local_read(void* attempt, uint32_t slot);
void llg_sequence_local_write(sv4_t* target, sv4_t value);
int llg_assertion_register(
    sv4_t* clock, int edge, sv4_t* disable,
    llg_concurrent_assertion_predicate_fn antecedent,
    llg_concurrent_assertion_predicate_fn consequent,
    const llg_co_desc_t* pass_desc,
    const llg_co_desc_t* fail_desc, void* data, int kind,
    int overlapped, uint64_t identity, const char* label, const char* location,
    const char* scope);
/* Extended concurrent-assertion registration with bounded accept_on /
 * reject_on controls. `abort_condition` is evaluated from the live value
 * domain for asynchronous controls and from the immutable sampled domain for
 * synchronous controls. The legacy registration entry point above remains a
 * source-compatible wrapper with no abort control. */
int llg_assertion_register_control(
    sv4_t* clock, int edge, sv4_t* disable,
    llg_concurrent_assertion_predicate_fn antecedent,
    llg_concurrent_assertion_predicate_fn consequent,
    llg_concurrent_assertion_predicate_fn abort_condition,
    const llg_co_desc_t* pass_desc,
    const llg_co_desc_t* fail_desc, void* data, int kind,
    int overlapped, int abort_reject, int abort_sync, uint64_t identity,
    const char* label, const char* location, const char* scope);
// Queue one deferred immediate-assertion result. The condition result and
// selected action are fixed at statement execution; the runtime matures the
// report in Reactive and owns `frame` until the callback (or teardown).
typedef void (*llg_deferred_assertion_fn)(llg_frame_t* frame);
int llg_deferred_assertion_enabled(int kind, const char* label, const char* scope);
void llg_deferred_assertion_scoped(int kind, int passed, uint64_t identity,
    const char* label, const char* location, const char* scope,
    llg_deferred_assertion_fn action, llg_frame_t* frame);
// `disable` of a deferred assertion cancels its pending reports; `disable`
// of a procedure's outermost scope flushes the executing processes' report
// queues (SV 16.4.4). Emitted before the matching llg_disable_target.
void llg_deferred_assertion_cancel(uint64_t identity);
// Queue a subroutine call attached to a sequence match (IEEE 1800-2009
// 16.11). Its by-value arguments were copied into `frame` when the match
// was evaluated; `call` runs with it in the Reactive region, in match order.
// Takes ownership of `frame`.
void llg_sequence_match_call(llg_deferred_assertion_fn call, llg_frame_t* frame);
void llg_deferred_assertion_flush_scope(uint32_t declaration, uint32_t instance);
void llg_deferred_assertion(int kind, int passed, uint64_t identity,
                            const char* label, const char* location,
                            llg_deferred_assertion_fn action,
                            llg_frame_t* frame);
int llg_assertion_register_sequence(
    sv4_t* clock, int edge, sv4_t* disable,
    const llg_sequence_graph_t* antecedent,
    const llg_sequence_graph_t* consequent,
    const llg_co_desc_t* pass_desc,
    const llg_co_desc_t* fail_desc, void* data, int kind,
    int overlapped, uint64_t identity, const char* label, const char* location,
    const char* scope);
int llg_assertion_register_sequence_control(
    sv4_t* clock, int edge, sv4_t* disable,
    const llg_sequence_graph_t* antecedent,
    const llg_sequence_graph_t* consequent,
    llg_concurrent_assertion_predicate_fn abort_condition,
    const llg_co_desc_t* pass_desc,
    const llg_co_desc_t* fail_desc, void* data, int kind,
    int overlapped, int abort_reject, int abort_sync, uint64_t identity,
    const char* label, const char* location, const char* scope);

/* A property program (IEEE 1800-2009 16.12-16.13) is a static node table
 * whose operands index earlier nodes; `root` is evaluated once per attempt.
 * Node operand fields by kind:
 *   BOOLEAN       first = atom
 *   SEQUENCE      first = sequence graph; STRONG flag
 *   NOT           first = operand
 *   AND/OR/IMPLIES/IFF  first, second = operands
 *   IMPLICATION / FOLLOWED_BY  first = antecedent graph, second = consequent;
 *                 OVERLAPPED for |-> and #-#
 *   IF            first = condition atom, second = then, third = else or NONE
 *   NEXTTIME      first = operand, min = tick count; STRONG flag
 *   ALWAYS / EVENTUALLY  first = operand, [min, max] ticks; STRONG flag
 *   UNTIL         first = left, second = right; STRONG, OVERLAPPED (until_with)
 *   ABORT         first = condition atom, second = operand; ACCEPT, SYNC
 * Atoms read Preponed sampled values. Ticks are counted on the assertion's
 * leading clock from the tick at which the node starts. */
#define LLG_PROPERTY_BOOLEAN 1u
#define LLG_PROPERTY_SEQUENCE 2u
#define LLG_PROPERTY_NOT 3u
#define LLG_PROPERTY_AND 4u
#define LLG_PROPERTY_OR 5u
#define LLG_PROPERTY_IMPLIES 6u
#define LLG_PROPERTY_IFF 7u
#define LLG_PROPERTY_IMPLICATION 8u
#define LLG_PROPERTY_FOLLOWED_BY 9u
#define LLG_PROPERTY_IF 10u
#define LLG_PROPERTY_NEXTTIME 11u
#define LLG_PROPERTY_ALWAYS 12u
#define LLG_PROPERTY_EVENTUALLY 13u
#define LLG_PROPERTY_UNTIL 14u
#define LLG_PROPERTY_ABORT 15u
#define LLG_PROPERTY_STRONG 1u
#define LLG_PROPERTY_OVERLAPPED 2u
#define LLG_PROPERTY_ACCEPT 4u
#define LLG_PROPERTY_SYNC 8u
#define LLG_PROPERTY_NONE UINT32_MAX
typedef int (*llg_property_atom_fn)(uint32_t atom, void* data);
typedef struct {
    uint32_t kind;
    uint32_t flags;
    uint32_t first;
    uint32_t second;
    uint32_t third;
    uint64_t min;
    uint64_t max;
} llg_property_node_t;
typedef struct {
    uint32_t node_count;
    const llg_property_node_t* nodes;
    uint32_t root;
    uint32_t sequence_count;
    const llg_sequence_graph_t* const* sequences;
    uint32_t atom_count;
    llg_property_atom_fn atom;
} llg_property_program_t;
int llg_assertion_register_property(
    sv4_t* clock, int edge, sv4_t* disable, const llg_property_program_t* program,
    const llg_co_desc_t* pass_desc, const llg_co_desc_t* fail_desc, void* data,
    int kind, uint64_t identity, const char* label, const char* location,
    const char* scope);

// ── Command-line plusargs ───────────────────────────────────────────────────
//
// Plusargs are the argv entries beginning with '+'. Test queries use literal
// prefix matching after the leading '+'. Value queries accept the standard
// %d/%o/%h/%x/%b/%e/%f/%g/%s conversions (including uppercase and leading-0
// forms); an unmatched query returns zero and leaves its destination unchanged.
int llg_test_plusargs(const char* pattern);
int llg_value_plusargs_packed(const char* format, sv4_t* out, uint32_t width,
                              int is_signed, int two_state);
int llg_value_plusargs_real(const char* format, double* out);
int llg_value_plusargs_string(const char* format, llg_string_t* out);
// Execute one `$system` command in the generated simulator process. This
// host boundary is disabled unless LLG_ALLOW_SYSTEM is set to 1, true, yes,
// or on. When disabled, the runtime diagnoses the attempted command, marks
// the simulation failed, and returns an all-known -1 status without invoking
// a shell. `has_command == 0` preserves the standard's omitted-argument
// `system(NULL)` query; `has_command == 1` passes the owned command, including
// an explicit empty string, to the host C `system()` function. The returned
// 32-bit signed value is the host C `system()` status; its nonzero encoding is
// platform-specific and is not normalized here. `command` is consumed
// regardless of whether execution is permitted.
sv4_t llg_system(llg_string_t command, int has_command);

// ── Fixed-array streaming selectors ─────────────────────────────────────────
// A runtime `with` selector on a fixed unpacked destination array is resolved
// in the generated translation unit. These helpers expose the container
// selector math without requiring the generated model to include the
// container header. `first`/`second` carry the selector's source-language
// bounds; the returned bit width is the actual selected extent, never the
// index expression's storage width.
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
/* Pack the runtime-selected elements of a fixed unpacked array into one
 * packed value in stream order, using `fallback` (the element's
 * default-uninitialized value) for logical indices outside the bounds. */
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
/* Left-align a runtime-sized stream in a fixed-size bit-stream target; an
 * oversize stream is an error (IEEE 1800-2009 11.4.14). Borrows `value`. */
sv4_t llg_stream_to_fixed(sv4_t value, uint32_t width, int is_signed);
/* Bit-stream cast of a dynamically sized source to a fixed-size type: the
 * sizes must match (IEEE 1800-2009 6.24.3). Borrows `value`. */
sv4_t llg_stream_cast_fixed(sv4_t value, uint32_t width, int is_signed);

// ── File descriptors and output ─────────────────────────────────────────────
// A mode-string fopen returns a bit-31-tagged FD. Preopened FDs 0x80000000,
// 0x80000001 and 0x80000002 name stdin, stdout and stderr. A one-argument
// fopen returns an MCD with bit 31 clear; MCD bit 0 is stdout and other bits
// are independently allocated output channels. Only MCDs may be combined.
// Runtime-owned FILE objects close at cleanup; host standard streams are borrowed.
uint32_t llg_file_descriptor(sv4_t value);
// Consumes the owned path/mode strings; an omitted mode selects write mode.
uint32_t llg_file_open(llg_string_t path, llg_string_t mode, int has_mode);
void llg_file_close(uint32_t descriptor);
int llg_file_flush(uint32_t descriptor, int all);
void llg_file_rewind(uint32_t descriptor);
int64_t llg_file_tell(uint32_t descriptor);
int llg_file_seek(uint32_t descriptor, sv4_t offset, sv4_t operation);
int llg_file_error(uint32_t descriptor, llg_string_t* message);
int llg_file_eof(uint32_t descriptor);
void llg_file_display_typed(uint32_t descriptor, const char* fmt,
                            llg_fmt_arg_t* args, int n, const char* scope,
                            int newline);

// ── Memory file tasks ────────────────────────────────────────────────────────
// Consume an owned path and read/write a fixed packed memory view. `dims` and
// `strides` describe the remaining unpacked dimensions in declaration order;
// the runtime addresses the first dimension from the file and walks all
// suffix dimensions in row-major order. `origin` and `view_total` select the
// view in the generated array's flat storage.
// `dims` carries the declaration's left/right bounds; start/finish are used
// only when the corresponding flag is non-zero. Radix is 2 for binary and 16
// for hexadecimal files. File syntax accepts whitespace, comments, radix
// digits, and `@` address jumps while preserving four-state X/Z digits.
// Addressing policy is 0 for the Verilog-2001 declaration-order defaults and
// 1 for the SystemVerilog-2009 low-to-high defaults. Explicit start/finish
// bounds are honored in source order for either policy.
// `enum_values` is a borrowed table used by reads to reject a non-member word;
// writes ignore that table. Both the table and its elements remain owned by
// the generated call scope.
// The unsuffixed entry points retain the one-dimensional ABI; generated
// multidimensional calls use the `_view` entry points below.
enum {
    LLG_MEMORY_ADDRESSING_VERILOG_2001 = 0,
    LLG_MEMORY_ADDRESSING_SYSTEMVERILOG_2009 = 1,
};
// `$sreadmemb/$sreadmemh` (IEEE 1800-2009 D.14): the next memory load reads
// `data` (not copied, valid until `llg_memory_text_end`) instead of opening
// the file its `path` names, which then only labels its diagnostics. Bracket
// exactly one load call.
void llg_memory_text_begin(const char* data, size_t length);
void llg_memory_text_end(void);
void llg_memory_read(llg_string_t path, sv4_t* memory, uint64_t total,
                     uint32_t elem_width, int8_t elem_signed, int8_t two_state,
                     const int32_t* dims, int n_dims, sv4_t start, sv4_t finish,
                     int has_start, int has_finish, int addressing_policy,
                     const sv4_t* enum_values, uint32_t enum_count, int radix);
void llg_memory_write(llg_string_t path, sv4_t* memory, uint64_t total,
                      uint32_t elem_width, int8_t elem_signed, int8_t two_state,
                      const int32_t* dims, int n_dims, sv4_t start, sv4_t finish,
                      int has_start, int has_finish, int addressing_policy,
                      const sv4_t* enum_values, uint32_t enum_count, int radix);
void llg_memory_read_view(llg_string_t path, sv4_t* memory, uint64_t total,
                          uint32_t elem_width, int8_t elem_signed, int8_t two_state,
                          const int32_t* dims, int n_dims,
                          const uint64_t* strides, uint64_t origin,
                          uint64_t view_total, sv4_t start, sv4_t finish,
                          int has_start, int has_finish, int addressing_policy,
                          const sv4_t* enum_values, uint32_t enum_count, int radix);
void llg_fixed_memory_read_view(llg_string_t path, llg_fixed_array_t* memory, uint64_t total,
                          uint32_t elem_width, int8_t elem_signed, int8_t two_state,
                          const int32_t* dims, int n_dims,
                          const uint64_t* strides, uint64_t origin,
                          uint64_t view_total, sv4_t start, sv4_t finish,
                          int has_start, int has_finish, int addressing_policy,
                          const sv4_t* enum_values, uint32_t enum_count, int radix);
void llg_memory_write_view(llg_string_t path, sv4_t* memory, uint64_t total,
                           uint32_t elem_width, int8_t elem_signed, int8_t two_state,
                           const int32_t* dims, int n_dims,
                           const uint64_t* strides, uint64_t origin,
                           uint64_t view_total, sv4_t start, sv4_t finish,
                           int has_start, int has_finish, int addressing_policy,
                           const sv4_t* enum_values, uint32_t enum_count, int radix);
void llg_fixed_memory_write_view(llg_string_t path, llg_fixed_array_t* memory, uint64_t total,
                           uint32_t elem_width, int8_t elem_signed, int8_t two_state,
                           const int32_t* dims, int n_dims,
                           const uint64_t* strides, uint64_t origin,
                           uint64_t view_total, sv4_t start, sv4_t finish,
                           int has_start, int has_finish, int addressing_policy,
                           const sv4_t* enum_values, uint32_t enum_count, int radix);

// Image entry points address the packed image of a fixed array without model
// storage: `width` = `total` * `elem_width` bits of `*storage` starting at bit
// `offset`, cell 0 in the most significant bits. A published image is a
// variable's own storage (a whole variable or local, or a constant member of
// one): a load stores each changed word in place, skips every store when
// `llg_ba` would (forced target, read-only region) and publishes the target
// once after the last word. An unpublished image is a lexical copy that the
// generated code stores back after a load; writing it publishes nothing.
typedef struct {
    sv4_t* storage;
    uint32_t offset;
    uint8_t publish;
} llg_memory_image_t;
void llg_image_memory_read_view(llg_string_t path, const llg_memory_image_t* memory,
                          uint64_t total,
                          uint32_t elem_width, int8_t elem_signed, int8_t two_state,
                          const int32_t* dims, int n_dims,
                          const uint64_t* strides, uint64_t origin,
                          uint64_t view_total, sv4_t start, sv4_t finish,
                          int has_start, int has_finish, int addressing_policy,
                          const sv4_t* enum_values, uint32_t enum_count, int radix);
void llg_image_memory_write_view(llg_string_t path, const llg_memory_image_t* memory,
                           uint64_t total,
                           uint32_t elem_width, int8_t elem_signed, int8_t two_state,
                           const int32_t* dims, int n_dims,
                           const uint64_t* strides, uint64_t origin,
                           uint64_t view_total, sv4_t start, sv4_t finish,
                           int has_start, int has_finish, int addressing_policy,
                           const sv4_t* enum_values, uint32_t enum_count, int radix);

// Container entry points address a dynamic array, queue or fixed-size class
// property in place. With `n_dims` 0 the view is `[0:size-1]` of the current
// size and `total`, `origin` and `view_total` are ignored. With `total` 0 and
// one ascending dimension the view is that slice of the current size, which
// must contain it.
#define LLG_CONTAINER_MEMORY_DECL(name, type)                                        \
    void name(llg_string_t path, type* memory, uint64_t total, uint32_t elem_width,  \
              int8_t elem_signed, int8_t two_state, const int32_t* dims, int n_dims, \
              const uint64_t* strides, uint64_t origin, uint64_t view_total,         \
              sv4_t start, sv4_t finish, int has_start, int has_finish,              \
              int addressing_policy, const sv4_t* enum_values, uint32_t enum_count,  \
              int radix);
LLG_CONTAINER_MEMORY_DECL(llg_dyn_memory_read_view, struct llg_dyn_array_t)
LLG_CONTAINER_MEMORY_DECL(llg_dyn_memory_write_view, struct llg_dyn_array_t)
LLG_CONTAINER_MEMORY_DECL(llg_queue_memory_read_view, struct llg_queue_t)
LLG_CONTAINER_MEMORY_DECL(llg_queue_memory_write_view, struct llg_queue_t)
// Associative entry points take an integral-index array; only the path,
// element, start/finish, enum and radix arguments apply (IEEE 1800-2009
// 21.4.1, 21.5.3; decisions S27-D15..D17).
LLG_CONTAINER_MEMORY_DECL(llg_assoc_memory_read_view, struct llg_assoc_t)
LLG_CONTAINER_MEMORY_DECL(llg_assoc_memory_write_view, struct llg_assoc_t)
#undef LLG_CONTAINER_MEMORY_DECL

// Row entry points address a dynamic array or queue whose elements are fixed
// arrays of `row_cells` packed words, in place. `LLG_MEMORY_ROWS_ONE` loads or
// dumps element `row` (an unknown or missing element is an error) through the
// view `dims`/`strides`/`origin`/`view_total` inside it. `LLG_MEMORY_ROWS_ALL`
// addresses every element of the current size, `LLG_MEMORY_ROWS_SLICE` the
// ascending slice `dims[0]:dims[1]` of it; the outer bounds, `origin` and
// `view_total` then come from the size, `strides[0]` is `row_cells` and
// `dims[2..]` are the element dimensions.
enum {
    LLG_MEMORY_ROWS_ONE = 0,
    LLG_MEMORY_ROWS_ALL = 1,
    LLG_MEMORY_ROWS_SLICE = 2,
};
#define LLG_ROWS_MEMORY_DECL(name, type)                                                 \
    void name(llg_string_t path, type* memory, int mode, sv4_t row, uint64_t row_cells,  \
              uint32_t elem_width, int8_t elem_signed, int8_t two_state,                 \
              const int32_t* dims, int n_dims, const uint64_t* strides, uint64_t origin, \
              uint64_t view_total, sv4_t start, sv4_t finish, int has_start,             \
              int has_finish, int addressing_policy, const sv4_t* enum_values,           \
              uint32_t enum_count, int radix);
LLG_ROWS_MEMORY_DECL(llg_dyn_rows_memory_read_view, struct llg_dyn_value_array_t)
LLG_ROWS_MEMORY_DECL(llg_dyn_rows_memory_write_view, struct llg_dyn_value_array_t)
LLG_ROWS_MEMORY_DECL(llg_queue_rows_memory_read_view, struct llg_queue_value_array_t)
LLG_ROWS_MEMORY_DECL(llg_queue_rows_memory_write_view, struct llg_queue_value_array_t)
// An associative array with an integral index takes `ROWS_ONE` (`row` is the
// key) or `ROWS_ALL` only; the latter ignores `origin` and `view_total`, and
// its `dims[0]` is the placeholder (0, 0) (decisions S27-D22..D24).
LLG_ROWS_MEMORY_DECL(llg_assoc_rows_memory_read_view, struct llg_assoc_value_t)
LLG_ROWS_MEMORY_DECL(llg_assoc_rows_memory_write_view, struct llg_assoc_value_t)
#undef LLG_ROWS_MEMORY_DECL

// ── File input ─────────────────────────────────────────────────────────────
// Formatted input uses an HDL-aware scanner rather than the host scanf family:
// packed destinations preserve X/Z and arbitrary model widths, while return
// values count successful assignments only.  Target descriptors are borrowed
// for the duration of one call and are never retained by the runtime.
enum {
    LLG_FILE_INPUT_PACKED = 0,
    LLG_FILE_INPUT_REAL = 1,
    LLG_FILE_INPUT_STRING = 2,
};

typedef struct {
    int kind;
    llg_ref_t* packed;
    double* real;
    llg_string_t* string;
    int shortreal;
} llg_file_input_target_t;

int llg_file_getc(uint32_t descriptor);
int llg_file_ungetc(uint32_t descriptor, sv4_t character);
int llg_file_gets(uint32_t descriptor, llg_string_t* target);
int llg_file_gets_packed(uint32_t descriptor, llg_ref_t* target);
int llg_file_scanf(uint32_t descriptor, const char* format,
                   const llg_file_input_target_t* targets, int target_count);
int llg_string_scanf(const char* source, size_t source_length,
                     const char* format,
                     const llg_file_input_target_t* targets, int target_count);
// Scoped forms used by generated models: `scope` is the `%m` text and
// `time_unit_fs` the calling scope's unit for `%t` (21.3.4.3).
int llg_file_scanf_scoped(uint32_t descriptor, const char* format,
                          const llg_file_input_target_t* targets, int target_count,
                          const char* scope, uint64_t time_unit_fs);
int llg_string_scanf_scoped(const char* source, size_t source_length,
                            const char* format,
                            const llg_file_input_target_t* targets, int target_count,
                            const char* scope, uint64_t time_unit_fs);
// The bytes of a borrowed packed scan source or format, leading zero bytes
// dropped; `*unknown` reports X/Z bits, which make the scan return EOF.
llg_string_t llg_scan_text_from_packed(sv4_t value, int* unknown);
int llg_file_read_packed(uint32_t descriptor, llg_ref_t* target);
int llg_file_read_array(uint32_t descriptor, sv4_t* values, uint32_t elem_width,
                        int elem_signed, int elem_two_state, uint64_t total,
                        const int32_t* dimensions, int dimension_count,
                        int has_start, sv4_t start, int has_count, sv4_t count);
int llg_fixed_file_read_array(uint32_t descriptor, llg_fixed_array_t* values, uint32_t elem_width,
                        int elem_signed, int elem_two_state, uint64_t total,
                        const int32_t* dimensions, int dimension_count,
                        int has_start, sv4_t start, int has_count, sv4_t count);
// `$fread` into a whole packed dynamic array or queue: addresses 0..size-1.
struct llg_dyn_array_t;
struct llg_queue_t;
int llg_dyn_file_read(uint32_t descriptor, struct llg_dyn_array_t* array,
                      int has_start, sv4_t start, int has_count, sv4_t count);
int llg_queue_file_read(uint32_t descriptor, struct llg_queue_t* queue,
                        int has_start, sv4_t start, int has_count, sv4_t count);

// ── $monitor / $strobe ────────────────────────────────────────────────────────
//
// A monitor's or strobe's arguments are re-evaluated by generated code through
// `eval`, which writes one owned `llg_fmt_arg_t` per argument into `out`, so
// the runtime reads CURRENT values each time it prints (after the NBA region
// commits, for $strobe). Format strings use the same typed formatter as
// immediate display/write, including `%m`, real, and string conversions.

typedef void (*llg_mon_eval_fn)(sv4_t* out, void* context);
typedef void (*llg_real_eval_fn)(double* out, void* context);
typedef void (*llg_display_eval_fn)(llg_fmt_arg_t* out, void* context);

// Register (or replace) the active $monitor.  `reads` contains the signal
// pointers that trigger re-evaluation; display-only time queries are not
// triggers.  Registration queues a report for the scheduler's settled
// observation point rather than printing immediately.  Only the most recent
// $monitor is active; a new call replaces it.  $fmonitor lists
// (llg_file_monitor_with_typed_reads) are separate entries: any number are
// active at once, none replaces or is replaced by $monitor, and $fclose
// removes a list when its last channel closes.  An invalid or closed
// descriptor registers nothing and sets the $ferror status.  Reports of one
// slot print in registration order, after that slot's strobes.
void llg_monitor_with_reads(const char* fmt, int n, llg_mon_eval_fn eval,
                            sv4_t* const* reads, int n_reads);
// Compatibility entry point for callers without an explicit trigger set.
void llg_monitor(const char* fmt, int n, llg_mon_eval_fn eval);
// Queue a $strobe: prints `fmt` once with the argument values read after the
// NBA region of the current time step commits (unlike $display, which reads
// them when the statement executes). Printing waits for active/inactive/NBA
// iteration to settle, including driver updates triggered by NBAs.
void llg_strobe(const char* fmt, int n, llg_mon_eval_fn eval);
void llg_monitor_with_typed_reads(const char* fmt, int n,
                                  llg_display_eval_fn eval, const char* scope,
                                  const llg_display_read_t* reads, int n_reads);
void llg_strobe_typed(const char* fmt, int n, llg_display_eval_fn eval,
                      const char* scope);
void llg_file_monitor_with_typed_reads(
    uint32_t descriptor, const char* fmt, int n, llg_display_eval_fn eval,
    const char* scope, const llg_display_read_t* reads, int n_reads);
void llg_file_strobe_typed(uint32_t descriptor, const char* fmt, int n,
                           llg_display_eval_fn eval, const char* scope);
// $monitoron / $monitoroff: set / clear the monitor flag, which only the
// $monitor list observes.  The flag outlives the list (a $monitor issued
// while it is clear stays silent), the last-printed snapshot is kept, and
// enabling queues one report at the next settled observation point even when
// values are unchanged.
void llg_monitor_set(int on);

// Spawn one process. `desc` and its `fn` remain immutable for the process
// lifetime. The runtime co-allocates desc->frame_size bytes immediately after
// the process's chain, initializes the chain, and owns the process/root frame
// until safe reclamation. Generated code initializes frame payload fields at
// their declaration sites; frames themselves are POD and own no cleanup. A
// process function obtains `self` with LLG_CO_OWNER(ch, llg_proc_t); the
// runtime addresses the root as LLG_CO_ROOT(&p->chain). Arena/recursive/large
// callees use `ch->arena` through LLG_CO_ARENA_ENTER/LLG_CO_CALL_ARENA.
// LLG_CO_DEBUG fills fresh root/callee payloads with LLG_CO_POISON_BYTE before
// initialization. Generated sibling overlays are poisoned after lexical owner
// cleanup; retained heap cells and live parent fields are preserved. Poison is
// observable stale-byte data, not a read trap, and adds no release operations.
llg_proc_t* llg_spawn(const llg_co_desc_t* desc, const char* name);
// Spawn a non-program process directly into an explicit execution region.
// Ordinary initial/always, continuous and link processes use this or
// llg_spawn (ACTIVE). Concurrent-assertion actions also enter through this
// service in Reactive; there is no separate function-pointer spawn ABI.
llg_proc_t* llg_spawn_in_region(const llg_co_desc_t* desc,
                                const char* name, llg_region_t region);
// Spawn in Reactive with a stable elaborated program-instance identity.
// Only initial procedures count toward natural completion; fork descendants
// inherit the origin but never extend the lifetime of their program.
llg_proc_t* llg_spawn_program_in_region(const llg_co_desc_t* desc,
                                         const char* name, llg_region_t region,
                                         uint64_t instance, int is_initial);
// Shared bodies borrow an immutable model-lifetime instance record. Initialize
// its pointer in the root payload before the queued process can run. The offset
// is generated with offsetof and must describe one complete pointer field.
llg_proc_t* llg_spawn_instance_in_region(const llg_co_desc_t* desc,
    const char* name, llg_region_t region, const void* record, size_t record_offset);
llg_proc_t* llg_spawn_program_instance_in_region(const llg_co_desc_t* desc,
    const char* name, llg_region_t region, uint64_t instance, int is_initial,
    const void* record, size_t record_offset);
// Current process while the scheduler is inside one llg_co_run call or a
// plain final call; NULL during initialization, scheduler bookkeeping, and
// callbacks without a process. The scheduler sets and clears this around
// every resume and installs a transient process record around each final.
llg_proc_t* llg_current(void);
// Return the activation frame retained by a process, or NULL for ordinary
// static-storage processes. The returned pointer is borrowed from `self`.
llg_frame_t* llg_proc_frame(llg_proc_t* self);
// Terminate the originating program's initials and descendants. Calls from
// a non-program origin are ignored. When `self` belongs to a program this
// performs cancellation bookkeeping, sets self->chain.exiting to
// LLG_EXIT_ABANDON, and returns for immediate generated-code propagation.
void llg_program_exit(llg_proc_t* self);

// ── Fine-grain process handles (IEEE 1800-2009 §9.7) ─────────────────────────
//
// A handle is a stable identity independent of the coroutine allocation. It
// remains queryable after the process has completed or been killed, until all
// HDL references and outstanding await registrations release it. `self` is a
// borrowed handle; assignment/capture operations retain the value explicitly.
llg_process_handle_t* llg_process_self(llg_proc_t* self);
int llg_process_status(const llg_process_handle_t* handle);
void llg_process_retain(llg_process_handle_t* handle);
void llg_process_release(llg_process_handle_t* handle);
void llg_process_assign(llg_process_handle_t** target,
                        llg_process_handle_t* source);
// Same for a generated evaluation temporary, which may hold a handle in a
// read-only region.
void llg_process_assign_temp(llg_process_handle_t** target,
                             llg_process_handle_t* source);
// Register an automatic process-handle slot before its first assignment. The
// runtime retains the slot's value until the owning process is completed or
// killed, even when the C block that declared the slot has already unwound.
void llg_process_local_register(llg_process_handle_t** slot);
// Pin a handle stored into uncounted handle storage (class properties,
// non-input formals, function results, fork-shared slots): the identity then
// stays valid until runtime cleanup. Idempotent; returns `handle`.
llg_process_handle_t* llg_process_pin(llg_process_handle_t* handle);
// Kill/resume are nonblocking process-control services. Kill sets
// self->chain.exiting to ABANDON when cancellation reaches `self`, or COMPLETE
// when program completion requests finish. Generated code checks immediately.
// Random-stream methods of a process handle (SV 18.14); the target's own
// stream, never the caller's. A null handle is a null object access; a
// terminated target reports an error and the call has no effect.
void llg_process_handle_srandom(llg_process_handle_t* handle, sv4_t seed);
llg_string_t llg_process_handle_get_randstate(llg_process_handle_t* handle);
int llg_process_handle_set_randstate(llg_process_handle_t* handle,
                                     llg_string_t state);
void llg_process_kill(llg_proc_t* self, llg_process_handle_t* handle);
void llg_process_resume(llg_proc_t* self, llg_process_handle_t* handle);

// ── Semaphores (IEEE 1800-2009 §15.3) ─────────────────────────────────────────
//
// A semaphore owns its key count and FIFO waiter queue in the runtime.  The
// generated model only holds an opaque pointer; each key-count argument is a
// four-state value so invalid/unknown counts are diagnosed at the boundary.
llg_semaphore_t* llg_semaphore_new(sv4_t key_count);
void llg_semaphore_put(llg_semaphore_t* semaphore, sv4_t key_count);
int llg_semaphore_try_get(llg_semaphore_t* semaphore, sv4_t key_count);

// Cooperative generated-loop interruption point. Zero means the loop may
// continue. Nonzero means the process budget was exhausted: the diagnostic
// and failure bookkeeping are complete and chain.exiting is ABANDON. A
// coroutine returns EXIT; a plain function branches to its unwind label.
int llg_budget_point(const char* location);
// Report a SystemVerilog unique/unique0/priority branch check. `check` is
// 1=unique, 2=unique0, 3=priority; `matched` counts matching case groups (or
// is zero/one for a conditional); `has_default` suppresses no-match reports.
// Diagnostics are warnings and do not stop simulation.
void llg_unique_priority_check(int check, int matched, int has_default,
                               const char* location);

// ── final blocks ──────────────────────────────────────────────────────────────
//
// `final begin … end` processes (SV 1800-2005 §10.7) run ONCE at the end of
// simulation — after `llg_rt_run` returns on $finish, deadlock, or running
// out of future events.  Generated `main()` registers them via
// `llg_spawn_final` (before or after `llg_rt_run`; registration is plain
// bookkeeping) and calls `llg_rt_run_finals()` afterwards.
//
// Finals run SEQUENTIALLY to completion in registration order.  Timing
// controls inside a final are rejected by codegen (LRM §10.7) — fork/join
// included, so a final never suspends and never leaves children behind; the
// ready-queue drain in `llg_rt_run_finals` is dead-defensive only.
// `$finish` inside a final terminates that final immediately and skips all
// remaining final procedures, as required by LRM §10.7.
// `$time` inside finals reports the time of the last scheduler event.

// Register one final-block function (no coroutine or frame is created here).
// Finals are plain `void fn(void)` calls run with a transient current record.
// Timing, fork and suspendable calls are forbidden in finals.
void llg_spawn_final(void (*fn)(void), const char* name);
// Run every registered final process sequentially and then release the
// runtime (the finals phase owns teardown).  A no-op when nothing was
// registered.  Repeated init/run cycles reset the registration list.
void llg_rt_run_finals(void);

// ── fork/join ─────────────────────────────────────────────────────────────────
//
// Processes spawn children with `llg_fork`; each child owns a chain and a POD
// root frame co-allocated with its process record. Children are resumed only
// by the scheduler, never inline. A
// `llg_fork_group_t` tracks the children of one `fork` statement:
//
//     llg_fork_group_t* g = llg_fork_group_new(LLG_JOIN);
//     llg_fork(&child_a_desc, "a", g);
//     llg_fork(&child_b_desc, "b", g);
//     LLG_CO_AWAIT(co, ch, n, llg_arm_join(self, g));
//
// `join_none` children are created in source order but become eligible only
// when their parent first suspends or terminates. `llg_arm_wait_fork` suspends
// until every live group of the current process is done (useful after join_none /
// join_any, whose groups outlive the parent's wait). `llg_disable_fork` kills
// all descendants of the current process, including children still pending
// their first execution; killed children's immediate NBA lists are discarded.
// Future updates already in the global timed NBA queue retain their persistent
// targets.

typedef struct llg_fork_group llg_fork_group_t;

// Join kinds shared with the executable IR's C emission.
enum {
    LLG_JOIN = 0,      // wait for every child
    LLG_JOIN_NONE = 1, // return immediately
    LLG_JOIN_ANY = 2,  // wait for the first child to finish
};

// Create a fork group owned by the current process (registers it on the
// process's live-group list).
llg_fork_group_t* llg_fork_group_new(int join_kind);
// Create a named fork group whose resolved target can be disabled from any
// process in the same elaborated instance. Anonymous groups use the function
// above and carry no disable target.
llg_fork_group_t* llg_fork_group_new_target(int join_kind,
                                            uint32_t declaration,
                                            uint32_t instance);
// Spawn desc->fn as a child of `grp`; the runtime borrows the immutable
// descriptor and name and owns the process/root-frame allocation.
llg_proc_t* llg_fork(const llg_co_desc_t* desc, const char* name,
                     llg_fork_group_t* grp);
// Spawn a child with one retained reference to `frame`. The child releases
// that reference on completion or cancellation; the caller retains ownership
// of its own reference and may release it after this call.
llg_proc_t* llg_fork_with_frame(const llg_co_desc_t* desc,
                                const char* name, llg_fork_group_t* grp,
                                llg_frame_t* frame);
// Spawn an independent process that is not a child of the caller: no fork
// group tracks it, so `wait fork`, `disable fork` and the caller's completion
// or cancellation never observe it. It inherits the caller's program origin
// (without extending that program's lifetime), random state lineage and
// Active/Reactive set, starts in the current time slot, and retains one
// reference to `frame` exactly like llg_fork_with_frame. Used for
// process-evaluated nonblocking event assignments, whose pending update must
// outlive the issuing process like a runtime-owned NBA.
llg_proc_t* llg_spawn_detached_with_frame(const llg_co_desc_t* desc,
                                          const char* name,
                                          llg_frame_t* frame);
// Spawn a runtime-owned Active process that belongs to no process: no
// parent, fork group or program origin, so `wait fork`, `disable fork` and
// the end of the calling process or program never observe it. Callable
// from a process or during model initialization. Its random stream is
// derived from the root without drawing from any stream. Retains one
// reference to `frame` like llg_fork_with_frame. Used for the per-instance
// sampling-event waiter of a covergroup, which lives as long as its instance.
llg_proc_t* llg_spawn_owned_with_frame(const llg_co_desc_t* desc,
                                       const char* name,
                                       llg_frame_t* frame);
// Create and manage typed activation storage. Slots hold copied values by
// default. Frame-to-frame aliases retain their source frame. Joined fork
// aliases borrow a registered numeric cell in the suspended parent activation;
// model-storage aliases borrow generated static cells. No activation slot
// retains a host stack pointer.
typedef enum {
    LLG_FRAME_PACKED = 0,
    LLG_FRAME_REAL = 1,
    LLG_FRAME_OPAQUE = 2,
    // An owned string value, destroyed when the frame is released.
    LLG_FRAME_STRING = 3,
    // An owned heap object (a container), destroyed and freed when the frame
    // is released.
    LLG_FRAME_OBJECT = 4,
} llg_frame_slot_kind_t;
llg_frame_t* llg_frame_new(size_t slots);
void llg_frame_retain(llg_frame_t* frame);
void llg_frame_release(llg_frame_t* frame);
void llg_frame_capture_value(llg_frame_t* frame, size_t slot, sv4_t value);
void llg_frame_capture_real(llg_frame_t* frame, size_t slot, double value);
// Opaque object identities (including event objects) remain valid until model
// teardown. A joined child may instead borrow a parent coroutine's event handle
// address; the parent must outlive the child, including cancellation cleanup.
void llg_frame_capture_opaque(llg_frame_t* frame, size_t slot, void* value);
// Copy `value` into a string slot owned by the frame.
void llg_frame_capture_string(llg_frame_t* frame, size_t slot,
                              const llg_string_t* value);
// The string cell of a slot (following frame-to-frame aliases); stable for
// the life of the frame.
llg_string_t* llg_frame_string_address(llg_frame_t* frame, size_t slot);
// Allocate a zeroed object of `size` bytes owned by the slot; `destroy` (may
// be NULL) runs on it before it is freed with the frame.
void* llg_frame_capture_object(llg_frame_t* frame, size_t slot, size_t size,
                               void (*destroy)(void*));
// The object of a slot (following frame-to-frame aliases).
void* llg_frame_object_address(llg_frame_t* frame, size_t slot);
// The handle cell of an opaque slot (following frame-to-frame aliases).
void** llg_frame_opaque_address(llg_frame_t* frame, size_t slot);
// The change marker of an opaque handle slot (following frame-to-frame
// aliases): allocated by the first call with `allocate` set (a fork-shared
// handle that a wait can observe) and released with the frame; NULL while
// none was allocated, so stores through other aliases publish nothing.
sv4_t* llg_frame_opaque_marker(llg_frame_t* frame, size_t slot, int allocate);
// The frame that owns a slot's storage (following frame-to-frame aliases), so
// a branch can reach the other slots of a shared environment frame.
llg_frame_t* llg_frame_slot_frame(llg_frame_t* frame, size_t slot);
void llg_frame_alias_value(llg_frame_t* frame, size_t slot, sv4_t* target);
void llg_frame_alias_real(llg_frame_t* frame, size_t slot, double* target);
void llg_frame_alias_slot(llg_frame_t* frame, size_t slot,
                          llg_frame_t* target, size_t target_slot);
llg_frame_slot_kind_t llg_frame_slot_kind(const llg_frame_t* frame,
                                          size_t slot);
// Return the stable numeric cell for a joined child borrowing its enclosing
// activation. The parent must remain suspended until that child completes.
sv4_t* llg_frame_value_address(llg_frame_t* frame, size_t slot);
double* llg_frame_real_address(llg_frame_t* frame, size_t slot);
sv4_t llg_frame_read_value(const llg_frame_t* frame, size_t slot);
void llg_frame_write_value(llg_frame_t* frame, size_t slot, sv4_t value);
double llg_frame_read_real(const llg_frame_t* frame, size_t slot);
void* llg_frame_read_opaque(const llg_frame_t* frame, size_t slot);
void llg_frame_write_real(llg_frame_t* frame, size_t slot, double value);
// Kill every descendant of the current process (immediate NBA lists are discarded).
void llg_disable_fork(llg_proc_t* self);

// Named block/task activation registry. Declaration and instance identities
// come from the owned semantic database; textual names never reach this ABI.
llg_activation_t* llg_activation_enter(uint32_t declaration,
                                       uint32_t instance);
void llg_activation_exit(llg_activation_t* activation);
// Exit of the named block that is a fork branch's whole body: when a disable
// ended it, the branch process terminates KILLED rather than FINISHED (SV 9.7).
void llg_activation_exit_terminal(llg_activation_t* activation);
int llg_activation_cancelled(void);
// Disabling an activation that reaches `self` completes cancellation
// bookkeeping, sets LLG_EXIT_ABANDON, and returns for immediate propagation.
// This is the only setter of an activation's `disabled` flag: generated code
// checks llg_activation_cancelled() only after resume points and after
// operations that may reach this call.
void llg_disable_target(llg_proc_t* self, uint32_t declaration,
                        uint32_t instance);

// ── Coroutine arms ───────────────────────────────────────────────────────────
//
// Arms validate and either satisfy or register exactly one blocking request;
// they never resume or visit the scheduler. READY continues in the same turn.
// SUSPEND means the waiter is fully registered and join_none children have
// been released. EXIT means `self->chain.exiting` is already set. Every arm in
// this ABI is one-shot and is emitted with LLG_CO_AWAIT, not AWAIT_RETRY.

// #delay; zero schedules Inactive/Re-Inactive in the same time step.
llg_co_arm_t llg_arm_time(llg_proc_t* self, uint64_t ticks);
// Set the explicit region used by the next signal/dependency wait. Generated
// sensitivity terminators use this to migrate a coroutine across region sets.
void llg_wait_resume_in_region(llg_region_t region);
// Signal/dependency arms copy their descriptor arrays before SUSPEND. Copy
// contract: llg_arm_any copies `sigs`, llg_arm_any_dependencies copies `deps`
// and llg_arm_any_events copies `specs`; none keeps a pointer into them, so
// generated coroutines pass each as a compound literal in the arm call and the
// array need not outlive it. Keep that true when changing these arms.
llg_co_arm_t llg_arm_edge(llg_proc_t* self, sv4_t* sig, int posedge);
// Signal/real dependency wakes preserve reverse wait-registration order within
// each resume region, whose process queue is FIFO. Duplicate addresses retain
// all descriptor checks but create one scheduler membership per waiter/source.
llg_co_arm_t llg_arm_any(llg_proc_t* self, sv4_t** sigs, int n);
llg_co_arm_t llg_arm_any_dependencies(llg_proc_t* self,
                                      const llg_wait_dependency_t* deps,
                                      int n);
llg_co_arm_t llg_arm_any_events(llg_proc_t* self,
                                const llg_event_spec_t* specs, int n);
// An implicit or process-evaluated event control's dependency wait: unlike
// llg_arm_any_dependencies (a wait condition's), an occurrence while the
// process is suspended is withheld and resume() resensitizes it (SV 9.7).
llg_co_arm_t llg_arm_event_dependencies(llg_proc_t* self,
                                        const llg_wait_dependency_t* deps,
                                        int n);
// llg_arm_any_dependencies (llg_arm_event_dependencies when `event`) over
// `deps` plus every storage the `ref` descriptors in `refs` name (several for
// an unpacked aggregate or a view into one). Both arrays are copied as for
// llg_arm_any_dependencies.
llg_co_arm_t llg_arm_ref_dependencies(llg_proc_t* self,
                                      const llg_wait_dependency_t* deps, int n,
                                      const llg_ref_t* const* refs, int n_refs,
                                      int event);
// Mark the next event-control wait of `self` as evaluated by the process: if
// an occurrence was withheld while it was suspended, resume() wakes it with
// llg_wait_refreshed() true so it takes its armed values again instead of
// comparing against values from before the suspension.
void llg_wait_refresh_on_resume(llg_proc_t* self);
int llg_wait_refreshed(const llg_proc_t* self);
llg_co_arm_t llg_arm_level(llg_proc_t* self, sv4_t* sig, sv4_t value);

// Synchronization and process-control arms deliver before wake and therefore
// are one-shot. Suspending process/mailbox destinations must be frame fields
// or registered stable storage. Suspending self releases pending join_none
// children; suspending another process returns READY in the caller.
llg_co_arm_t llg_arm_join(llg_proc_t* self, llg_fork_group_t* grp);
llg_co_arm_t llg_arm_wait_fork(llg_proc_t* self);
llg_co_arm_t llg_arm_process_suspend(llg_proc_t* self,
                                     llg_process_handle_t* handle);
llg_co_arm_t llg_arm_process_await(llg_proc_t* self,
                                   llg_process_handle_t* handle);
// Called when a granted get resumes: the request's keys are taken. A process
// killed between the grant and this call returns its keys (SV 15.3).
void llg_semaphore_grant_taken(llg_proc_t* self);
llg_co_arm_t llg_arm_semaphore_get(llg_proc_t* self,
                                   llg_semaphore_t* semaphore,
                                   sv4_t key_count);
// Coroutine-body `$stop` only. SUSPEND preserves the exact chain continuation
// and queues. Functions and VPI use the non-suspending deferred request.
llg_co_arm_t llg_arm_stop(llg_proc_t* self, int verbosity,
                          const char* location);

// ── Named events ──────────────────────────────────────────────────────────────
//
// A named event (`event ev; -> ev; @(ev);`, LRM 1364-1995 §9.7.3) carries no
// value: it is purely a wakeup channel.  Triggering wakes EVERY process
// currently suspended on the event, in deterministic waiter-table order —
// registration order until earlier partial unlinks (swap-with-last) reorder
// the table; a trigger with no waiters is lost — events are edge-triggered,
// not stateful, so trigger-before-wait never latches.  Each event holds a
// waiter table that grows on demand with checked allocation, so the number of
// concurrent waiters on one event is bounded only by available memory.
//
// The struct is a valid zero initializer: generated models define one global
// per declared event. Ordinary waiters and persistent-trigger waiters are
// separate so a trigger never latches an ordinary `@(event)` control. The
// generation distinguishes a completed runtime from its next initialization,
// so a static generated event cannot retain `.triggered` across runs.
// `llg_event_object_reset` releases the grown tables; generated model
// init/teardown calls it so a static event object cannot leak or retain a
// dangling table across runs.

typedef struct {
    llg_proc_t** waiters;
    int n_waiters;
    size_t waiters_capacity;
    llg_proc_t** triggered_waiters;
    int n_triggered_waiters;
    size_t triggered_waiters_capacity;
    uint64_t triggered_time;
    uint64_t triggered_generation;
    // Monotonic count of triggers in this run. A process that evaluates its
    // own event control compares counts to learn that the event fired while
    // it was parked on a list that also names value sources.
    uint64_t trigger_count;
    int triggered;
} llg_event_object_t;

// Release any grown waiter tables and clear the event state. Safe on a
// zero-initialized object and idempotent.
void llg_event_object_reset(llg_event_object_t* ev);
// A fresh synchronization object for an automatic event declaration. Like
// new event elements of resizable containers it is owned by the scheduler
// until model close, so handles copied out of the activation stay valid.
llg_event_object_t* llg_event_object_new(void);

typedef struct {
    llg_event_object_t* object;
} llg_event_t;

// Resolve a fixed unpacked event-array select using declaration-order
// flattening. Unknown/out-of-range indices resolve to a null handle, which
// preserves the ordinary null-event trigger/wait behavior.
llg_event_t* llg_event_array_select(llg_event_t* const* elements,
                                     uint64_t total,
                                     const int32_t* left,
                                     const int32_t* right,
                                     const sv4_t* indices,
                                     int n);

// Assigning a handle changes only the future object resolved by the target;
// waiter registrations already attached to the previous object are retained.
void llg_event_assign(llg_event_t* target, const llg_event_t* source);
void llg_event_assign_null(llg_event_t* target);

// Wake every current waiter of `ev` and clear its waiter list.
/* Internal clocking-block publication, not an ordinary HDL trigger. */
int llg_clocking_event_observed(llg_event_t* event);
void llg_event_trigger(llg_event_t* ev);
// Return whether the synchronization object was triggered in the current
// simulation time slot. A null handle is never triggered.
int llg_event_triggered(const llg_event_t* ev);
// Number of triggers of the object `ev` currently names since the runtime was
// initialized. A null handle reports zero. Read-only and allocation-free.
uint64_t llg_event_trigger_count(const llg_event_t* ev);
/* Make the concurrent assertion just registered (a cover of every match of
 * a sequence instance) the sequence endpoint `index`, numbered from zero in
 * registration order (IEEE 1800-2009 16.9.11, 16.14.5-16.14.6, 9.4.2.4). Each
 * match triggers `event` in the Observed region and latches the `.matched`
 * status of each destination clock in `clocks`/`edges` (copied). Endpoints
 * have no counters or actions and ignore assertion control tasks. */
int llg_assertion_endpoint(uint64_t identity, uint32_t index, llg_event_t* event,
                           uint32_t slot_count, sv4_t* const* clocks, const int* edges);
/* `.matched` of endpoint `endpoint` at a tick of its destination clock
 * `slot`: a match since the previous tick of that clock. */
int llg_sequence_matched(uint32_t endpoint, uint32_t slot);
/* `.triggered` of endpoint `endpoint` inside an assertion: it matched in the
 * current time step. */
int llg_sequence_triggered(uint32_t endpoint);
// Queue a nonblocking event trigger for the NBA region. The event pointer is
// copied into runtime-owned queue state, so the issuing process may finish
// before the trigger commits.
void llg_nba_event(llg_event_t* ev);
// Queue a nonblocking event trigger after `ticks`; zero stays in the current
// time slot's NBA region, while a positive delay enters the timed NBA queue.
void llg_nba_event_after(llg_event_t* ev, uint64_t ticks);
// A single named event uses inline copied storage; larger lists are heap-owned.
// Named-event arms copy the event list before SUSPEND. Triggered returns READY
// if the event is already set in this slot. `result` for wait_order must point
// at a frame field; the runtime writes 1 (success) or -1 (out of order) before
// waking, and leaves zero while pending. Copy contract: llg_arm_events copies
// `evs` and llg_arm_order copies `evs` (it keeps only the resolved event
// objects, never the array or the handles), so generated coroutines pass the
// list as a compound literal; `result` is kept and must stay a frame field.
llg_co_arm_t llg_arm_event(llg_proc_t* self, llg_event_t* ev);
llg_co_arm_t llg_arm_events(llg_proc_t* self,
                            const llg_event_t* const* evs, int n);
llg_co_arm_t llg_arm_event_triggered(llg_proc_t* self,
                                     const llg_event_t* ev);
llg_co_arm_t llg_arm_assertion(llg_proc_t* self, uint64_t identity);
llg_co_arm_t llg_arm_order(llg_proc_t* self,
                           const llg_event_t* const* evs, int n,
                           int* result);

// One source of a mixed signal/event or-list (`@(posedge a or ev)`): exactly
// one of `sig`/`ev` is set.  Exactly one such wait covers ALL entries, so a
// trigger arriving while the process is parked on the signal half is not lost.
typedef struct {
    sv4_t* sig;               // signal entry (NULL for an event entry)
    int kind;                 // LLG_EV_* edge kind for signal entries
    const llg_event_t* ev;   // event entry (NULL for a signal entry)
} llg_wait_src_t;

// Atomic mixed wait until any signal entry matches its edge kind or any event
// entry is triggered. The runtime copies `srcs` before SUSPEND and does not
// keep it or any pointer into it (the signals and event objects it names are
// model storage), so `srcs` need not outlive the call.
llg_co_arm_t llg_arm_mixed(llg_proc_t* self,
                           const llg_wait_src_t* srcs, int n);

// One clocking-cycle arm. When `accept_current` is nonzero it returns READY if
// a source already fired in this slot (the ##0 rule); otherwise it registers
// the same one-shot wait as llg_arm_mixed. Generated code keeps a uint64_t
// remaining-cycle field, emits one LLG_CO_AWAIT per loop iteration, and
// decrements after each wake. No runtime retry arm retains the count. `srcs`
// is copied (via llg_arm_mixed) and need not outlive the call, so generated
// code passes it as a compound literal at each arm site.
llg_co_arm_t llg_arm_clocking_cycle(llg_proc_t* self,
                                    const llg_wait_src_t* srcs, int n,
                                    int accept_current);

// Evaluators and dependencies refer to model storage. The runtime copies
// every descriptor and dependency array before suspending the caller, so
// llg_arm_expressions copies `specs` and each `dependencies` array and keeps
// no pointer into them; generated coroutines pass both as compound literals.
// `event` and `eval_context` name model/frame storage, not the arrays. The
// nonblocking registrations below copy the arrays too, but generated code
// still declares those arrays (they are not arms).
// Callbacks must not suspend or mutate scheduler-observed storage. When a
// generated descriptor supplies eval_context or condition_context, it passes
// ownership of one initial llg_frame_t reference to the expression wait; the
// wait releases that reference on wake, cancellation, or runtime teardown.
typedef struct {
    sv4_t* sig;
    llg_mon_eval_fn eval;
    llg_mon_eval_fn condition;
    const llg_event_t* event;
    // Resolved at wait registration so a later handle assignment does not
    // move or suppress this expression waiter.
    llg_event_object_t* event_object;
    sv4_t** reads;
    int n_reads;
    int kind;
    double* real_sig;
    llg_real_eval_fn real_eval;
    llg_wait_dependency_t* dependencies;
    int n_dependencies;
    int real;
    void* eval_context;
    void* condition_context;
} llg_expr_event_spec_t;
llg_co_arm_t llg_arm_expressions(llg_proc_t* self,
                                 const llg_expr_event_spec_t* specs, int n);
// An llg_arm_expressions source for a plain or edge control (`kind`) on the
// value `ref` names. A whole descriptor waits on its variable itself; any
// other descriptor waits on every storage it names and compares the value it
// reads, through a new context reference the wait releases (see
// llg_expr_event_spec_t).
llg_expr_event_spec_t llg_ref_wait_spec(const llg_ref_t* ref, int kind);
// Register a nonblocking trigger whose source control is evaluated at issue
// time. The copied descriptors remain live until one source matches; the
// target event is then submitted to the ordinary NBA queue. `repeat` is zero
// for a no-op request and otherwise the number of matches required.
void llg_nba_event_when(const llg_expr_event_spec_t* specs, int n,
                        llg_event_t* target, uint64_t repeat);
// Register a nonblocking assignment whose source control is evaluated at
// issue time. The runtime owns `frame` until the source matches or teardown;
// `action` submits the detached NBA using the captured frame values. A zero
// repeat count invokes `action` immediately, without registering a waiter.
typedef void (*llg_event_assignment_fn)(llg_frame_t* frame);
void llg_nba_event_assign_when(const llg_expr_event_spec_t* specs, int n,
                               uint64_t repeat,
                               llg_event_assignment_fn action,
                               llg_frame_t* frame);
// Normalize a packed repeat count without truncating values wider than 64 bits.
uint64_t llg_repeat_count(sv4_t value);

// One-shot region callback hooks. `data` remains caller-owned and is passed
// unchanged. A zero delay schedules the callback in the current time slot;
// positive delays enter the timed callback queue. Writable iterative regions
// may re-enter design/reactive work; read-only phases reject current-slot
// scheduling except for the Observed-to-Reactive assertion handoff.
typedef void (*llg_region_callback_fn)(void* data);
int llg_schedule_region_callback(llg_region_t region,
                                 llg_region_callback_fn callback, void* data);
int llg_schedule_region_callback_after(llg_region_t region,
                                       llg_region_callback_fn callback,
                                       void* data, uint64_t ticks);
// Explicit name for PLI users; currently one-shot and otherwise identical to
// llg_schedule_region_callback.
int llg_register_pli_callback(llg_region_t region,
                              llg_region_callback_fn callback, void* data);
// Identity of one queued callback: the runtime lifetime (bumped by every
// llg_rt_cleanup) and the issue sequence within it. Callers may keep it
// after the callback runs; a later cancel is then rejected as stale.
typedef struct {
    uint64_t generation;
    uint64_t sequence;
} llg_region_callback_id_t;
// llg_schedule_region_callback_after that also reports the callback's
// identity in `*id` (which may be NULL) for llg_cancel_region_callback.
int llg_schedule_region_callback_id(llg_region_t region,
                                    llg_region_callback_fn callback,
                                    void* data, uint64_t ticks,
                                    llg_region_callback_id_t* id);
// Remove a still-queued callback without calling it. Returns 1 when removed;
// 0 when it already ran or was cancelled, or when `id` belongs to an earlier
// runtime lifetime. `data` stays caller-owned either way.
int llg_cancel_region_callback(llg_region_callback_id_t id);

// Register a signal for a copied, immutable value sampled at the beginning of
// each time slot. The returned pointer is runtime-owned and valid until the
// next llg_rt_cleanup. Unregistered signals produce a controlled diagnostic.
// The signal also serves Observed clocking copies; history is not retained.
void llg_sampled_register(sv4_t* signal);
// Like llg_sampled_register, and keep the per-slot history that clocking
// input skews of at most `ticks` read. Repeated registrations keep the
// largest depth; older slots are released as time advances.
void llg_sampled_register_history(sv4_t* signal, uint64_t ticks);
// Register a signal for its Preponed value only (procedural `$sampled`,
// IEEE 1800-2009 16.9.3). It keeps no per-slot history, so its memory stays
// constant. A later history registration promotes it.
void llg_sampled_register_value(sv4_t* signal);
const sv4_t* llg_sampled_value(const sv4_t* signal);
int llg_sampled_copy(const sv4_t* signal, sv4_t* out);
// Real variables keep their own numeric Preponed snapshot. Reading an
// unregistered real reports the same controlled diagnostic and yields 0.0.
void llg_sampled_register_real(double* signal);
double llg_sampled_real(const double* signal);
/// Sampled-value clocks (IEEE 1800-2009 16.9.3). A clock owns the ticks
/// shared by every history domain sampled on it; identities are dense small
/// integers assigned by the generated model. An edge clock ticks when
/// `signal` makes the `edge` transition (LLG_EV_POSEDGE or LLG_EV_NEGEDGE)
/// and the optional `gate` (`iff` and the `$past` gating expression) is true
/// in the current values at that moment, like an event control's `iff`. An
/// event clock ticks only through llg_sampled_clock_tick, which a generated
/// process calls after waiting on any other legal clocking event (event
/// lists, `edge`, value changes, named and clocking-block events, expression
/// edges); its `gate` is evaluated when the tick is reported. Repeated ticks
/// in one time step count once.
typedef int (*llg_sampled_gate_fn)(void* data);
int llg_sampled_clock_register_edge(uint64_t clock, sv4_t* signal, int edge,
                                    llg_sampled_gate_fn gate, void* data);
/* Gate the leading clock of the concurrent assertion just registered with
 * `identity`: its clocking event `@(edge clk iff cond)` occurs only when
 * `gate` (reading current values, called with NULL) is true at the clock
 * write (IEEE 1800-2009 9.4.2.3). Call after the registration, before run. */
// Restrict a registered assertion to one evaluation attempt, begun at its
// first leading clock event (an assertion that is the whole body of an
// initial procedure, IEEE 1800-2009 16.15.6).
int llg_assertion_single_attempt(uint64_t identity);
// Make the last registered assertion `identity` procedural concurrent
// assertion `index` (dense from 0 in registration order, IEEE 1800-2009
// 16.15.6): a leading clock event begins one attempt per matured instance of
// it, never one on its own. Call after the registration, before run.
int llg_assertion_procedural(uint64_t identity, uint32_t index);
// Queue one pending instance of procedural assertion `index` on the current
// process. Flush points and `disable` clear pending instances like pending
// deferred assertion reports (16.15.6.2, 16.15.6.4); the Observed region
// matures the rest.
void llg_assertion_queue(uint32_t index);
int llg_assertion_gate_clock(uint64_t identity, llg_sampled_gate_fn gate);
int llg_sampled_clock_register_event(uint64_t clock, llg_sampled_gate_fn gate,
                                     void* data);
void llg_sampled_clock_tick(uint64_t clock);
/// One sampled history domain: an expression sampled on one clock. The
/// callback stores the expression's Preponed value into `out`, an
/// initialized owner. `history_ticks` (at least 1) is the deepest `$past`
/// tick count read from the domain; status functions need 1. The domain
/// keeps at most that many earlier time steps plus the current one in a ring
/// that grows only up to that bound, so recording a tick never allocates
/// once the ring is full and memory never grows with simulated time.
typedef void (*llg_sampled_domain_eval_fn)(void* data, sv4_t* out);
int llg_sampled_domain_register(uint64_t identity, uint64_t clock,
                                llg_sampled_domain_eval_fn value, void* data,
                                uint64_t history_ticks);
sv4_t llg_sampled_domain_past(uint64_t identity, uint64_t ticks);
/// Compare the Preponed value of the calling time step with the value of the
/// most recent strictly earlier tick (or the initial value). `kind`: 0 rose,
/// 1 fell, 2 stable, 3 changed; 4 stable and 5 changed compare 64-bit real
/// images numerically (`==` on the decoded reals).
int llg_sampled_domain_status(uint64_t identity, int kind);
// Clocking input copies. Each publishes a changed sample to its waiters.
// llg_clocking_sample copies the Preponed (#1step) value; observed copies are
// queued into the current time slot's observed region; history copies read the
// preponed sample at or before `ticks` simulation ticks in the past, where
// `ticks` must not exceed the source's llg_sampled_register_history depth.
int llg_clocking_sample(const sv4_t* source, sv4_t* sample);
int llg_clocking_sample_observed(sv4_t* source, sv4_t* sample);
int llg_clocking_sample_history(sv4_t* source, sv4_t* sample,
                                uint64_t ticks);
// Real clockvar samples: `source` is the packed 64-bit `$realtobits` image
// of the clocking expression, registered like a packed source; the selected
// image is decoded into the real sample.
int llg_clocking_sample_real(const sv4_t* source, double* sample);
int llg_clocking_sample_observed_real(sv4_t* source, double* sample);
int llg_clocking_sample_history_real(sv4_t* source, double* sample,
                                     uint64_t ticks);

// Forget clocking history before freeing/reusing an externally owned packed
// descriptor that received runtime writes. Registered packed value scopes do
// this at final release; model storage survives until llg_rt_cleanup. This does
// not unregister waits, sampling or assertions: their borrowed sources must
// still outlive their consumers. NULL and already-forgotten signals are harmless.
void llg_clocking_forget_signal(sv4_t* signal);

// Assignments.  llg_nba records on the current process's list and commits in
// the NBA region; llg_ba writes immediately and notifies waiters.
// Packed write/enqueue inputs are borrowed. Retained values/masks are cloned;
// targets must be initialized stable cells that outlive pending writes.
void llg_nba(sv4_t* target, sv4_t value);
// Capture values now, retaining target storage through the future NBA commit.
// A zero tick delay stays in the current time slot's NBA region.
void llg_nba_after(sv4_t* target, sv4_t value, uint64_t ticks);
/* Model-owned net/driver descriptors outlive all queued writes. */
void llg_nba_net_after(llg_net_t* net, int slot, sv4_t value, uint64_t ticks);
void llg_nba_net_masked_after(llg_net_t* net, int slot, sv4_t value,
                              sv4_t mask, uint64_t ticks);
void llg_nba_net_selected_after(llg_net_t* net, int slot, sv4_t value,
                                sv4_select_plan_t plan, int reverse,
                                uint64_t ticks);
// Synchronous drives use the target clocking event. If the event has not
// occurred in the current time slot, the runtime retains the captured value
// until the next matching event before applying the output skew. `cycles`
// is the drive's `##n` (SV 14.16): an on-event drive matures `n` events
// later, an off-event drive at the `max(n, 1)`-th following event.
void llg_clocking_nba_sync_after(sv4_t* target, sv4_t value, uint64_t ticks,
                                 uint64_t cycles,
                                 const llg_wait_src_t* specs, int n_specs);
void llg_clocking_nba_net_sync_after(llg_net_t* net, int slot, sv4_t value,
                                     uint64_t ticks, uint64_t cycles,
                                     const llg_wait_src_t* specs, int n_specs);
void llg_nba_d_after(double* target, double value, uint64_t ticks);
void llg_clocking_nba_d_sync_after(double* target, double value, uint64_t ticks,
                                   uint64_t cycles,
                                   const llg_wait_src_t* specs, int n_specs);
// Native NBAs to persistent storage. The string value is consumed; the
// chandle or class handle value is stored as-is at commit, which toggles
// `dependency` (the handle's change marker, or NULL) when the value changes.
void llg_string_nba_after(llg_string_t* target, llg_string_t value,
                          uint64_t ticks);
void llg_chandle_nba_after(void** target, void* value, uint64_t ticks,
                           sv4_t* dependency);
// Nonblocking write to persistent descriptor-backed array storage (a
// fixed-array view of string, record or handle elements, SIM-007). The
// source elements (`count` from storage position `src_start`, or the whole
// source when `whole`) are copied at issue; `dst_start` is captured at issue.
// The commit replaces the whole target, or writes the in-range positions
// from `dst_start`, and notifies changed contents. The target must outlive
// the pending write. Packed positions and `src` are borrowed.
struct llg_dyn_value_array_t;
void llg_dyn_value_nba(struct llg_dyn_value_array_t* target, sv4_t dst_start,
                       const struct llg_dyn_value_array_t* src, sv4_t src_start,
                       uint64_t count, int whole);
// Selected writes retain only the plan's valid contiguous slice. Ascending
// declared part-selects set reverse so queued bits remain in storage order.
void llg_nba_selected_after(sv4_t* target, sv4_t value,
                            sv4_select_plan_t plan, int reverse,
                            uint64_t ticks);
// A tagged-member NBA (SV 11.9): the target and value are fixed at issue,
// and the commit repeats each borrowed active-tag check against the target's
// storage at that time. A failed check reports a source-addressed runtime
// error and drops the write, so a retag between issue and commit can never
// leave a payload of another member's type. `checks` and `location` are
// copied/borrowed static literals; `checks` itself need not outlive the call.
void llg_nba_tagged_selected_after(sv4_t* target, sv4_t value,
                                   sv4_select_plan_t plan, int reverse,
                                   uint64_t ticks,
                                   const llg_ref_tag_check_t* checks,
                                   size_t check_count, const char* location);
// Merge only known-one mask positions into the target at commit time. This is
// the general fallback; contiguous masks are narrowed internally.
void llg_nba_masked(sv4_t* target, sv4_t value, sv4_t mask, uint64_t ticks);
void llg_clocking_nba_sync_masked_after(
    sv4_t* target, sv4_t value, sv4_t mask, uint64_t ticks, uint64_t cycles,
    const llg_wait_src_t* specs, int n_specs);
void llg_clocking_nba_net_sync_masked_after(
    llg_net_t* net, int slot, sv4_t value, sv4_t mask, uint64_t ticks,
    uint64_t cycles, const llg_wait_src_t* specs, int n_specs);
void llg_clocking_nba_sync_selected_after(
    sv4_t* target, sv4_t value, sv4_select_plan_t plan, int reverse,
    uint64_t ticks, uint64_t cycles, const llg_wait_src_t* specs, int n_specs);
void llg_clocking_nba_net_sync_selected_after(
    llg_net_t* net, int slot, sv4_t value, sv4_select_plan_t plan, int reverse,
    uint64_t ticks, uint64_t cycles, const llg_wait_src_t* specs, int n_specs);
void llg_ba(sv4_t* target, sv4_t value);
// llg_ba borrowing `value` by address. Generated stores use it so that
// unoptimized AArch64 (and other ABIs passing large structs through a
// caller-owned copy) do not reserve a separate stack copy per call site.
void llg_ba_from(sv4_t* target, const sv4_t* value);
// Commit a write through a canonical `ref` descriptor immediately. Selected
// aliases update the original storage once, preserving normal wakeups and
// force/continuous-assignment checks.
// Every generated call with packed refs owns one pin scope, entered before
// actual evaluation. Normal return releases it; process teardown unwinds it.
typedef struct llg_ref_scope llg_ref_scope_t;
llg_ref_scope_t* llg_ref_scope_begin(void);
void llg_ref_scope_end(llg_ref_scope_t* scope);
// Call-local queue cells released by normal and nonlocal value-scope cleanup.
void llg_ref_scope_begin_owned(void);
llg_ref_t* llg_ref_queue(llg_queue_t* queue, uint64_t index);
// Retained dynamic-array element and associative entry references (13.5.2).
struct llg_dyn_array_t;
struct llg_assoc_t;
llg_ref_t* llg_ref_dyn(struct llg_dyn_array_t* array, sv4_t index);
llg_ref_t* llg_ref_assoc_integral(struct llg_assoc_t* array, sv4_t key);
llg_ref_t* llg_ref_assoc_string(struct llg_assoc_t* array, const void* key,
                                size_t key_length);
void llg_ref_write(llg_ref_t* ref, sv4_t value);
// Runtime-facing read reports checked tagged-union reference-view failures.
sv4_t llg_rt_ref_read(const llg_ref_t* ref);
/* Borrow value and mask; update only the selected logical reference bits. */
void llg_ref_write_masked(llg_ref_t* ref, sv4_t value, sv4_t mask);
/* Capture persistent fixed leaves now; the descriptor graph may expire before commit. */
void llg_ref_nba_masked(llg_ref_t* ref, sv4_t value, sv4_t mask, uint64_t ticks);
// Modify one packed bit through the original descriptor, preserving alias
// notification and retained queue-element identity. Invalid indices no-op.
void llg_ref_write_bit(llg_ref_t* ref, uint64_t index, sv4_t value);
void llg_nba_d(double* target, double value);
void llg_ba_d(double* target, double value);
// Reorder `count` elements of `element_cells` contiguous real cells in place
// (LLG_CONTAINER_METHOD_REVERSE/SORT/RSORT; sorting needs one-cell elements).
// Keys compare numerically and NaN keys keep their positions. The new order is
// snapshotted first, then every cell publishes through llg_ba_d.
void llg_real_cells_order(double* cells, uint64_t count, uint64_t element_cells,
                          int method);

// ── force / release ───────────────────────────────────────────────────────────
//
// A force entry is an overriding live driver. Its callback is evaluated when
// the force is installed and whenever one of its explicitly registered source
// values changes. Packed targets are described as one or more canonical
// storage parts; a net pointer on a part keeps the underlying driver
// resolution available for release and re-application. No pre-force value is
// saved: variables retain the currently forced value on release, while nets
// are recomputed from their current driver slots. The live-entry table grows
// on demand with checked allocation.

typedef struct {
    sv4_t* target;
    llg_net_t* net;
    int64_t left;
    int64_t right;
    uint32_t width;
    uint32_t value_lsb;
    int two_state;
} llg_force_part_t;

typedef struct {
    sv4_t* sig;
    double* real;
    int is_real;
} llg_force_read_t;

typedef void (*llg_force_eval_fn)(sv4_t* out);
typedef void (*llg_force_real_eval_fn)(double* out);

void llg_force_expr_parts(const llg_force_part_t* parts, int n_parts,
                          uint32_t stream_slice, int stream_right_to_left,
                          llg_force_eval_fn eval,
                          const llg_force_read_t* reads, int n_reads);
void llg_release_parts(const llg_force_part_t* parts, int n_parts,
                       uint32_t stream_slice, int stream_right_to_left);
void llg_force_real(double* target, llg_force_real_eval_fn eval,
                    const llg_force_read_t* reads, int n_reads);
// Whether a live force binding still reads `sig` (packed) or `real`. Exactly
// one pointer is non-null. An effectful force site evaluates its RHS into a
// hidden source in its guard process only while this holds, so the helper's
// effects never run after release or replacement. Read-only.
int llg_force_source_active(const sv4_t* sig, const double* real);
void llg_release_real(double* target);

// Legacy constant-value entry points retained for runtime self-tests and
// hand-written generated models. They use the same live-entry table but have
// no source dependencies.
void llg_force(sv4_t* sig, sv4_t value);
void llg_release(sv4_t* sig);
// Constant-value forces used by vpi_put_value(vpiForceFlag). `value` is
// borrowed and copied into the live entry.
void llg_force_value_parts(const llg_force_part_t* parts, int n_parts, sv4_t value);
void llg_force_real_value(double* target, double value);

// Borrowed command line of the run (argc; argv through `argv`).
int llg_rt_command_line(char*** argv);

/* Destination-passing forms of the packed-returning calls above, used by
 * generated code (see value/destinations.h). `X_to(dst, ...)` replaces the
 * initialized owner at dst with exactly the result `X(...)` would return. Packed
 * arguments are borrowed by address; string arguments are consumed through
 * their address and left empty, like the returning forms' by-value strings. */
void llg_fixed_array_compare_to(sv4_t* dst, const llg_fixed_array_t* p0, const llg_fixed_array_t* p1, int p2, int p3);
void llg_fixed_array_stream_source_to(sv4_t* dst, const llg_fixed_array_t* array, int64_t declaration_left, int64_t declaration_right, uint32_t element_width, const sv4_t* fallback, int selector_kind, const sv4_t* first, const sv4_t* second);
void llg_net_alias_read_to(sv4_t* dst, llg_net_alias_t* alias);
void llg_q_full_to(sv4_t* dst, const sv4_t* q_id, sv4_t* status);
void llg_urandom_to(sv4_t* dst);
void llg_urandom_seed_to(sv4_t* dst, const sv4_t* seed);
void llg_urandom_range_to(sv4_t* dst, const sv4_t* max, const sv4_t* min, int has_min);
void llg_sequence_local_read_to(sv4_t* dst, void* attempt, uint32_t slot);
void llg_system_to(sv4_t* dst, llg_string_t* command, int has_command);
void llg_frame_read_value_to(sv4_t* dst, const llg_frame_t* frame, size_t slot);
void llg_sampled_domain_past_to(sv4_t* dst, uint64_t identity, uint64_t ticks);
void llg_rt_ref_read_to(sv4_t* dst, const llg_ref_t* ref);
/* String destination forms: `X_to(dst, ...)` replaces the expression owner at
 * dst (destroyed first; it must carry no change callback) with the string
 * `X(...)` would return. Argument conventions match the packed forms above. */
void llg_process_get_randstate_to(llg_string_t* dst);
void llg_process_handle_get_randstate_to(llg_string_t* dst, llg_process_handle_t* handle);
void llg_string_format_typed_to(llg_string_t* dst, llg_string_t* format, llg_fmt_arg_t* args, int n, const char* scope);
/* Implemented with the container runtime; declared in both headers. */
void llg_fixed_stream_source_to(sv4_t* dst, const sv4_t* values, int64_t declaration_left, int64_t declaration_right, uint32_t element_width, const sv4_t* fallback, int selector_kind, const sv4_t* first, const sv4_t* second);
void llg_stream_unpack_source_to(sv4_t* dst, const sv4_t* value, uint64_t bits, uint32_t slice, int right_to_left);
void llg_fixed_image_stream_source_to(sv4_t* dst, const sv4_t* image, int64_t declaration_left, int64_t declaration_right, uint32_t element_width, const sv4_t* fallback, int selector_kind, const sv4_t* first, const sv4_t* second);
void llg_stream_to_fixed_to(sv4_t* dst, const sv4_t* value, uint32_t width, int is_signed);
void llg_stream_cast_fixed_to(sv4_t* dst, const sv4_t* value, uint32_t width, int is_signed);

/* ── Functional coverage (IEEE 1800-2009 19, coverage/coverage.c) ─────────────
 * A coverpoint with more bins than this keeps its counters in a sparse table
 * sized by the bins actually hit, so a large auto_bin_max costs memory only
 * for sampled values. */
#ifndef LLG_COV_DENSE_BINS
#define LLG_COV_DENSE_BINS 4096u
#endif
/* The most classes of select-equivalent cross products a cross's bin
 * counts may enumerate (19.11.2); more is reported as an llg limitation. */
#ifndef LLG_COV_CROSS_CLASSES
#define LLG_COV_CROSS_CLASSES (UINT64_C(1) << 20)
#endif
/* The automatic bins of a coverpoint without `bins` declarations (19.5.2);
 * EXPLICIT points have none. */
enum { LLG_COV_POINT_AUTO = 0, LLG_COV_POINT_ENUM = 1, LLG_COV_POINT_EXPLICIT = 2 };
enum {
    LLG_COV_OPTION_WEIGHT = 0,
    LLG_COV_OPTION_GOAL = 1,
    LLG_COV_OPTION_AT_LEAST = 2,
    LLG_COV_OPTION_AUTO_BIN_MAX = 3,
    LLG_COV_OPTION_DETECT_OVERLAP = 4,
    LLG_COV_OPTION_GET_INST_COVERAGE = 5,
    LLG_COV_OPTION_PER_INSTANCE = 6,
    LLG_COV_OPTION_CROSS_NUM_PRINT_MISSING = 7
};
/* Covergroup type options (Table 19-3), at the group or an item. */
enum {
    LLG_COV_TYPE_OPTION_WEIGHT = 0,
    LLG_COV_TYPE_OPTION_GOAL = 1,
    LLG_COV_TYPE_OPTION_STROBE = 2,
    LLG_COV_TYPE_OPTION_MERGE_INSTANCES = 3
};
/* String options: option.name (group only) and option.comment or
 * type_option.comment (19.10). */
enum { LLG_COV_TEXT_NAME = 0, LLG_COV_TEXT_COMMENT = 1 };
/* The keyword of a declared bin (19.5); DEFAULT is `bins b = default`. */
enum {
    LLG_COV_BIN_BINS = 0,
    LLG_COV_BIN_IGNORE = 1,
    LLG_COV_BIN_ILLEGAL = 2,
    LLG_COV_BIN_DEFAULT = 3,
    LLG_COV_BIN_ILLEGAL_DEFAULT = 4
};
/* `bins b`, `bins b[]` (one bin per value) and `bins b[N]` (19.5). */
enum { LLG_COV_SHAPE_SCALAR = 0, LLG_COV_SHAPE_PER_VALUE = 1, LLG_COV_SHAPE_COUNT = 2 };
/* One bound of a value item: a key resolved at compile time, a slot value
 * (a covergroup argument, 19.5) resolved by 19.5.6 when the instance is
 * ready, or `$`. A static item has two KEY bounds. */
enum {
    LLG_COV_BOUND_KEY = 0,
    LLG_COV_BOUND_SLOT = 1,
    LLG_COV_BOUND_MIN = 2,
    LLG_COV_BOUND_MAX = 3
};
/* Keys order a coverpoint's values numerically: the value masked to the
 * coverpoint width, with the sign bit flipped for a signed coverpoint. */
typedef struct {
    uint64_t lo, hi; /* keys, or slot indices */
    uint8_t lo_kind, hi_kind;
    const char* site; /* a slot item's location, for 19.5.6 warnings */
} llg_cov_item_t;
typedef struct {
    const char* name;
    const char* site;      /* declaration location, for run-time messages */
    uint8_t kind;          /* LLG_COV_BIN_* */
    uint8_t shape;         /* LLG_COV_SHAPE_* */
    uint8_t count_is_slot; /* COUNT: `count` names a slot */
    int32_t guard;         /* index of the bin's `iff` guard, or -1 */
    uint64_t count;        /* COUNT: N, or its slot */
    uint32_t first_item, n_items;
} llg_cov_bin_t;
/* Transition bins (19.5.1). Keys fall into classes, maximal key ranges that
 * every transition item holds entirely or not at all: class i holds the keys
 * from bounds[i] up to the next bound. A step accepts a set of classes (a
 * bitset of `words` words at `accept + step * words`); each set is an
 * automaton over (step, repetition count) states numbered from the steps'
 * `state`. A step without repetition is CONSECUTIVE [1:1]; a nonconsecutive
 * repetition that ends its set is GOTO. */
enum {
    LLG_COV_REPEAT_CONSECUTIVE = 0,
    LLG_COV_REPEAT_GOTO = 1,
    LLG_COV_REPEAT_NONCONSECUTIVE = 2
};
typedef struct {
    uint8_t repeat; /* LLG_COV_REPEAT_* */
    uint8_t last;   /* the final step of its set */
    uint32_t lo, hi;
    uint32_t state; /* the state of repetition count 0 */
    uint32_t set;
} llg_cov_step_t;
typedef struct {
    uint32_t bin, first_step, n_steps;
} llg_cov_tset_t;
typedef struct {
    const char* name;
    const char* site;
    uint8_t kind;  /* LLG_COV_BIN_*; DEFAULT and ILLEGAL_DEFAULT: `default sequence` */
    uint8_t array; /* `b[]`: one bin per sequence */
    int32_t guard;
    /* Bins counted by coverage (19.11.1): 0 or 1 for a scalar bin, the
     * number of sequences of an array bin. */
    uint64_t coverable;
} llg_cov_tbin_t;
typedef struct {
    uint32_t n_classes;
    const uint64_t* bounds;
    uint32_t words;
    const uint64_t* accept;
    uint32_t n_steps;
    const llg_cov_step_t* steps;
    uint32_t n_sets;
    const llg_cov_tset_t* sets;
    const uint32_t* start_index; /* n_classes + 1 offsets into start_sets */
    const uint32_t* start_sets;  /* per class: sets whose first step accepts it */
    uint32_t n_states;
    uint32_t window; /* the longest array-bin sequence */
    uint32_t n_bins;
    const llg_cov_tbin_t* bins;
    int32_t default_bin; /* the `default sequence` bin, or -1 */
    /* Pairs of `bins` declarations that share a sequence, for
     * option.detect_overlap. */
    uint32_t n_overlaps;
    const uint32_t* overlaps;
} llg_cov_trans_t;
typedef struct {
    const char* name;
    uint32_t width;  /* 1..64 */
    uint8_t is_signed;
    uint8_t kind;    /* LLG_COV_POINT_* */
    uint8_t dynamic; /* a bin reads a slot: the layout is built per instance */
    uint32_t n_keys;
    const uint64_t* keys; /* ENUM: ascending member keys, one bin each */
    uint32_t n_bins;
    const llg_cov_bin_t* bins; /* declared value bins, in source order */
    const llg_cov_item_t* items;
    const llg_cov_trans_t* trans; /* transition bins, or NULL */
    uint8_t crossed;              /* a cross reads the bins each sample counts */
    int64_t type_weight;          /* type_option.weight, for merged type coverage */
} llg_cov_point_t;
/* Crosses (19.6). Along each dimension (a crossed coverpoint), the
 * coordinates are the coverpoint's coverable bins in counter order. A select
 * condition `binsof(point[.decl]) [intersect {items}]`, possibly negated, is
 * a set of coordinates of its dimension; a box is a conjunction of
 * conditions; a bin's select expression is a union of boxes. */
typedef struct {
    uint32_t dim;
    int32_t decl; /* the coverpoint's value bins declaration, or -1 */
    uint8_t intersect;
    uint8_t negate;
    uint32_t first_item, n_items; /* into the cross's items */
} llg_cov_select_t;
typedef struct {
    uint32_t first_select, n_selects; /* into box_selects */
} llg_cov_box_t;
typedef struct {
    const char* name;
    const char* site;
    uint8_t kind; /* LLG_COV_BIN_BINS, _IGNORE or _ILLEGAL */
    int32_t guard;
    uint32_t first_box, n_boxes;
} llg_cov_xbin_t;
typedef struct {
    const char* name;
    uint32_t n_dims;
    const uint32_t* dims; /* coverpoint indices */
    uint32_t n_selects;
    const llg_cov_select_t* selects;
    const llg_cov_item_t* items;
    uint32_t n_boxes;
    const llg_cov_box_t* boxes;
    const uint32_t* box_selects;
    uint32_t n_bins;
    const llg_cov_xbin_t* bins;
    uint8_t dynamic; /* an intersect value reads a slot */
    int64_t type_weight;
} llg_cov_cross_t;
typedef struct llg_cov_inst llg_cov_inst_t;
typedef struct llg_cov_layout llg_cov_layout_t;
/* The declared type options of one coverpoint or cross (Table 19-3, 19-4). */
typedef struct {
    int64_t weight;
    int64_t goal;
    const char* comment; /* NULL is "" */
} llg_cov_type_decl_t;
/* The same options once a procedural write changed one of them (19.7.1). */
typedef struct {
    int64_t weight;
    int64_t goal;
    char* comment; /* runtime-owned; NULL is "" */
} llg_cov_type_item_t;
/* One covergroup declaration identity: every instance ever created, for
 * type coverage (19.11.3). Mutable model storage owned by the runtime;
 * generated code initializes every current value to its declared value. */
typedef struct llg_cov_type {
    const char* name;
    /* type_option.weight, goal and merge_instances: procedural writes change
     * them (19.7.1); llg_cov_release_all restores the declared values. */
    int64_t weight;
    llg_cov_inst_t* first;
    llg_cov_inst_t* last;
    uint64_t n_instances;
    struct llg_cov_type* next_used;
    uint8_t used;
    uint8_t merge;       /* type_option.merge_instances (19.11.3) */
    uint8_t merge_error; /* a merged cross limitation was reported */
    uint8_t strobe;      /* type_option.strobe */
    int64_t declared_weight;
    uint8_t declared_merge;
    int64_t goal;
    int64_t declared_goal;
    const char* declared_comment;
    char* comment;       /* a procedurally written comment */
    uint8_t comment_set; /* `comment` replaces declared_comment */
    /* Item type options: the declaration's, then a runtime copy on the
     * first procedural write. */
    uint32_t n_items;
    const llg_cov_type_decl_t* declared_items;
    llg_cov_type_item_t* items;
    /* Records loaded by $load_coverage_db (19.9), in the instance list. */
    uint64_t n_loaded;
} llg_cov_type_t;
/* One elaborated covergroup declaration: its coverpoints and the objects its
 * coverage event samples. */
typedef struct llg_cov_group {
    const char* name;
    /* Hierarchical declaration path, the base of default instance names. */
    const char* path;
    uint64_t created; /* instances created in this run */
    uint32_t n_points;
    const llg_cov_point_t* points;
    /* Coverage items are numbered points first, then crosses, wherever an
     * API takes a `point` index. */
    uint32_t n_crosses;
    const llg_cov_cross_t* crosses;
    uint32_t n_slots; /* argument values bins read (llg_cov_slot) */
    llg_cov_type_t* type;
    void** registry;
    size_t n_registered;
    size_t registry_capacity;
    struct llg_cov_group* next_registered;
    uint8_t registered;
    /* type_option.strobe: calls the formal-free sample method on one
     * object; NULL when the group samples at the event (19.7.1). */
    void (*strobe_sample)(void* object);
    uint8_t strobe_pending; /* a registry pass is queued for this slot */
    /* Layouts shared by instances whose bins depend on nothing per
     * instance, one per point and then one per cross; built by the first
     * llg_cov_ready. */
    llg_cov_layout_t** layouts;
    struct llg_cov_xlayout** xlayouts;
} llg_cov_group_t;
/* Create the runtime record of one instance with Table 19-1 defaults; apply
 * llg_cov_option and llg_cov_slot, then llg_cov_ready before the first
 * sample or query. */
void* llg_cov_new(llg_cov_group_t* group);
/* `point` < 0 selects the group scope. Invalid values report at `site`. */
void llg_cov_option(void* inst, int32_t point, uint32_t option, sv4_t value, const char* site);
/* The value of argument slot `slot` that bins of this instance read. */
void llg_cov_slot(void* inst, uint32_t slot, sv4_t value);
void llg_cov_ready(void* inst);
/* Count one sampled value; X/Z values and stopped scopes count nothing.
 * `value` is only read during the call, so generated code passes the address
 * of the sampled variable itself when the coverpoint is a plain read.
 * `guards` holds the group's bin `iff` results, or is NULL without any. */
void llg_cov_hit(void* inst, uint32_t point, const sv4_t* value, const uint8_t* guards);
/* The coverpoint's `iff` was false at a sample: the transition sequences in
 * progress end (decision S45-D9). */
void llg_cov_skip(void* inst, uint32_t point);
/* Begin one sample of a group with crosses, before its llg_cov_hit calls. */
void llg_cov_sample_begin(void* inst);
/* Count the products of the bins the crossed coverpoints counted in this
 * sample (19.6); call after every llg_cov_hit of the sample. */
void llg_cov_cross_hit(void* inst, uint32_t cross, const uint8_t* guards);
void llg_cov_control(void* inst, int32_t point, int start);
/* Objects sampled by the group's coverage event, pinned for the run. */
void llg_cov_register(llg_cov_group_t* group, void* object);
size_t llg_cov_registered(const llg_cov_group_t* group);
/* Whether an event-driven sample of the instance can count anything: false
 * while the whole instance is stopped, so its waiter skips the sample. */
int llg_cov_sampling(const void* inst);
/* type_option.strobe (19.7.1): queue one sample of `object` (whose record
 * is `inst`), or one pass over the group's registered objects, for the
 * Postponed region of the current time slot. Further requests in the same
 * slot are merged. */
void llg_cov_strobe(llg_cov_group_t* group, void* inst, void* object);
void llg_cov_strobe_registered(llg_cov_group_t* group);
void* llg_cov_registered_at(const llg_cov_group_t* group, size_t index);
/* Coverage percentages (19.11); `point` < 0 selects the whole group.
 * With non-NULL `covered` and `total`, also store the bin numbers of the
 * two-argument forms of 19.8 (saturated to int, decision S46-D7). */
double llg_cov_inst_coverage(const void* inst, int32_t point);
double llg_cov_inst_coverage_counts(const void* inst, int32_t point, int32_t* covered,
                                    int32_t* total);
/* Type coverage of one declaration identity: its live instances and the
 * records $load_coverage_db read. */
double llg_cov_type_coverage(llg_cov_type_t* type, int32_t point);
double llg_cov_type_coverage_counts(llg_cov_type_t* type, int32_t point, int32_t* covered,
                                    int32_t* total);
double llg_cov_total_coverage(void);
/* Procedural option access (19.7, 19.7.1). Integer options read as int,
 * flags as bit (19.10). */
int64_t llg_cov_option_get(const void* inst, int32_t point, uint32_t option);
void llg_cov_type_option(llg_cov_type_t* type, int32_t point, uint32_t option, sv4_t value,
                         const char* site);
int64_t llg_cov_type_option_get(const llg_cov_type_t* type, int32_t point, uint32_t option);
/* String options (option.name, option.comment, type_option.comment):
 * writes consume `value`; reads return an owned string. */
void llg_cov_text(void* inst, int32_t point, uint32_t which, llg_string_t value);
llg_string_t llg_cov_text_get(const void* inst, int32_t point, uint32_t which);
void llg_cov_type_text(llg_cov_type_t* type, int32_t point, llg_string_t value);
llg_string_t llg_cov_type_text_get(const llg_cov_type_t* type, int32_t point);
/* The coverage database (19.9): every covergroup declaration and type of the
 * model, registered at startup. The declarations identify the database's
 * schema; the types are restored to their declared options at cleanup. */
void llg_cov_model(llg_cov_group_t* const* groups, size_t n_groups, llg_cov_type_t* const* types,
                   size_t n_types);
void llg_cov_set_db_name(llg_string_t name);
void llg_cov_load_db(llg_string_t name, const char* site);
/* Save the database named by $set_coverage_db_name, once per run, after
 * final procedures (19.9); called by llg_rt_cleanup. */
void llg_cov_end_of_run(void);
void llg_cov_release_all(void);

/* ── Assertion coverage and coverage access (IEEE 1800-2009 40.2-40.3) ──
 *
 * One item per assertion statement instance, with the counters of 40.5.3.
 * The generated model owns the item array (static storage); llg_acov_model
 * zeroes the counters and binds the table before assertions register. A
 * model without the table has no assertion coverage: every assertion runs
 * with a NULL item and the 40.3.2 functions report SV_COV_NOCOV for it. */
enum {
    LLG_ACOV_CONCURRENT = 0,
    LLG_ACOV_COVER_SEQUENCE = 1,
    LLG_ACOV_IMMEDIATE = 2,
    LLG_ACOV_DEFERRED = 3,
};

typedef struct llg_acov_item {
    uint64_t identity;    /* the statement's owned semantic identity */
    const char* name;     /* label; "" when unnamed */
    const char* full_name; /* hierarchical name; "" when unnamed */
    int32_t instance;     /* owning instance index, -1 for $root */
    uint8_t kind;         /* LLG_ASSERTION_ASSERT/ASSUME/COVER */
    uint8_t form;         /* LLG_ACOV_* */
    /* Run-time state: collection stopped by $coverage_control (40.3.2.1). */
    uint8_t stopped;
    /* 40.5.3 counters. successes are nonvacuous (matches of a cover
     * sequence); failures count assert/assume only. */
    uint64_t attempts;
    uint64_t successes;
    uint64_t vacuous;
    uint64_t failures;
    uint64_t disabled;
    uint64_t killed;
} llg_acov_item_t;

typedef struct llg_acov_instance {
    const char* path;
    const char* definition;
    int32_t parent; /* -1 for a top-level instance */
} llg_acov_instance_t;

enum {
    LLG_COVERAGE_CONTROL = 0,
    LLG_COVERAGE_GET_MAX = 1,
    LLG_COVERAGE_GET = 2,
    LLG_COVERAGE_MERGE = 3,
    LLG_COVERAGE_SAVE = 4,
};

void llg_acov_model(llg_acov_item_t* items, size_t n_items, const llg_acov_instance_t* instances,
                    size_t n_instances);
/* One enabled check of an immediate assertion: `passed` is its outcome. */
void llg_acov_check(llg_acov_item_t* item, int passed);
/* $coverage_control/_get_max/_get/_merge/_save. `args` holds the integral
 * arguments; `instance` names the scope (an instance index, -1 $root, -2 the
 * definition named by `text`); merge and save name the database in `text`.
 * Returns the SV_COV_* status or the coverage count. */
int32_t llg_coverage_call(int function, const sv4_t* args, int n_args, int32_t instance,
                          llg_string_t text, int has_text, const char* site);

/* The assertion coverage tables for the VPI bridge (llg_vpi.c, 39.3,
 * 40.5): NULL beyond the bound tables. */
size_t llg_acov_item_count(void);
const llg_acov_item_t* llg_acov_item_at(size_t index);
size_t llg_acov_instance_count(void);
const llg_acov_instance_t* llg_acov_instance_at(size_t index);
/* The instance index of the instance named `path`, or -1. */
int32_t llg_acov_instance_named(const char* path);
/* The items owned directly by `instance` with a nonvacuous success
 * (S46-D4, MODULE semantics). */
size_t llg_acov_covered_in(int32_t instance);
/* vpi_control coverage operations (40.5.4, S46-D10): `control` and `type`
 * are the SV_COV_* values of 40.3.1. `item` >= 0 selects one item;
 * otherwise `instance` (MODULE semantics). Returns the SV_COV_* status. */
int32_t llg_coverage_control_vpi(int control, int type, int32_t instance, int64_t item);
/* vpiCoverageSave (`merge` 0) and vpiCoverageMerge (`merge` 1). */
int32_t llg_coverage_file_vpi(int merge, int type, const char* name);

#ifdef __cplusplus
}
#endif

#endif // LLG_RT_H
