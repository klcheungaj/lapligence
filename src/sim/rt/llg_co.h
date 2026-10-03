/* llg_co.h - stackless coroutines for generated llg simulation models.
 *
 * Design: plan/stackless-coroutine-merged/design.md. Production home:
 * src/sim/rt/llg_co.h, with the cold paths in src/sim/rt/llg_co.c.
 *
 * Model
 * -----
 * A coroutine is an ordinary C function over an explicit frame:
 *
 *     llg_co_status_t fn(llg_co_frame_t* co, llg_co_chain_t* ch);
 *
 * Every value that is live across a resume point is a field of the frame.
 * Suspension stores a resume-point number in `co->state` and returns.
 * Resumption re-enters `fn`, whose prologue dispatches through a dense
 * `switch` to a function-scoped `goto` label. There is no stack switching,
 * assembly, ucontext, setjmp, TLS or hidden mutable global state, so the same
 * C11 source builds with GCC, Clang, AppleClang and MSVC on x86-64 and ARM64
 * and runs unmodified under ASan/UBSan/TSan/Valgrind.
 *
 * Resumption polls from an anchor. Every frame has an 8-byte header holding
 * only its resume state. A caller resumes a suspended static callee by
 * dispatching to its own call site and calling the callee again at the
 * frame's fixed (computed) address; the callee dispatches on its own state.
 * Frame addresses are therefore never loaded from memory on the resume path,
 * so cache misses of the chain and its frames overlap instead of serializing.
 *
 * An anchor is a frame resumed directly rather than through its caller: the
 * chain's root frame (stored immediately after the chain) and anchored
 * callees (recursion, virtual/indirect calls, oversized frames, and static
 * paths deeper than the emitter's poll-depth limit). Whether a call is
 * anchored is decided per call site: the call site reserves a 16-byte
 * llg_co_anchor_t prefix (descriptor + parent anchor) immediately before the
 * callee's ordinary frame, so every frame type, and the code of every
 * function, is the same whether it is polled or anchored. `ch->resume` is the
 * one function a resume calls: the root's own function in the common case,
 * so a resume is one load and one indirect call, or the library's anchor loop
 * while an anchored callee is active (`ch->top` names the innermost one). A resume walks at most the static
 * frames below one anchor, and native stack use is bounded by that depth, so
 * unbounded logical recursion (through anchors) uses constant native stack.
 *
 * Frames hold plain data only (no destructors). Cancellation never runs
 * coroutine code: the owner drains its registered cleanup scopes and then
 * releases the arena and the root allocation.
 *
 * Threading: nothing here is shared. A chain may migrate between threads when
 * ownership is transferred with release/acquire ordering; it must never be
 * resumed by two threads at once. Descriptors are immutable and shareable.
 *
 * Invariant the code generator must keep: in a coroutine function no C local
 * is live across a resume point. The macros' own temporaries live inside
 * `do { } while (0)` blocks that no label is inside. GCC's
 * -Werror=jump-misses-init enforces the declaration half of this rule.
 */
#ifndef LLG_CO_H
#define LLG_CO_H

#include <stddef.h>
#include <stdint.h>
#ifdef LLG_CO_DEBUG
#include <string.h>
#endif
#include <stdlib.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Bump on any change to a public struct layout, enum value or macro protocol.
 * Out-of-line symbols carry the version in their link name, so a model built
 * against another header version fails to link instead of misbehaving.
 * Debug builds also suffix these names: identical layouts do not make the
 * debug and release liveness protocols interchangeable. */
#define LLG_CO_ABI_VERSION 1
#ifdef LLG_CO_DEBUG
#define LLG_CO_SYM_(name, v) name##_abi##v##_debug
#else
#define LLG_CO_SYM_(name, v) name##_abi##v
#endif
#define LLG_CO_SYM(name, v) LLG_CO_SYM_(name, v)

#if defined(__GNUC__) || defined(__clang__)
#define LLG_CO_LIKELY(x) __builtin_expect(!!(x), 1)
#define LLG_CO_UNLIKELY(x) __builtin_expect(!!(x), 0)
#define LLG_CO_INLINE static inline __attribute__((always_inline))
#define LLG_CO_NORETURN __attribute__((noreturn))
#elif defined(_MSC_VER)
#define LLG_CO_LIKELY(x) (x)
#define LLG_CO_UNLIKELY(x) (x)
#define LLG_CO_INLINE static __forceinline
#define LLG_CO_NORETURN __declspec(noreturn)
#else
#define LLG_CO_LIKELY(x) (x)
#define LLG_CO_UNLIKELY(x) (x)
#define LLG_CO_INLINE static inline
#define LLG_CO_NORETURN
#endif

/* Result of running a frame. */
typedef enum llg_co_status {
    LLG_CO_DONE = 0,    /* frame finished; outputs written through its pointers */
    LLG_CO_PENDING = 1, /* suspended; every frame on the path recorded its state */
    LLG_CO_EXIT = 2,    /* terminate the whole chain without resuming it */
    LLG_CO_CALLED = 3   /* a new anchor was entered (ch->top): run it in this
                         * same turn. Consumed by llg_co_run; the scheduler
                         * never sees it. */
} llg_co_status_t;

/* Result of arming a runtime wait (two-phase suspension protocol). An arm
 * function validates, then either satisfies the request immediately or
 * registers the waiter; it never suspends by itself. */
typedef enum llg_co_arm {
    LLG_CO_ARM_READY = 0,   /* already satisfied (or a no-op); continue */
    LLG_CO_ARM_SUSPEND = 1, /* waiter registered; caller must suspend */
    LLG_CO_ARM_EXIT = 2     /* the chain must terminate */
} llg_co_arm_t;

typedef struct llg_co_frame llg_co_frame_t;
typedef struct llg_co_anchor llg_co_anchor_t;
typedef struct llg_co_chain llg_co_chain_t;
typedef struct llg_co_desc llg_co_desc_t;
typedef struct llg_co_site llg_co_site_t;
typedef llg_co_status_t (*llg_co_fn)(llg_co_frame_t* co, llg_co_chain_t* ch);

/* Common header; must be the first member of every generated frame. It keeps
 * its natural 4-byte alignment: an explicit 8-byte alignment was measured to
 * produce byte-identical code for real (pointer-holding, hence 8-aligned)
 * frames and only padding for 32-bit-only ones (design.md §5.3). */
struct llg_co_frame {
    uint32_t state; /* resume point; 0 = function entry */
    uint32_t flags; /* 0, or LLG_CO_FRAME_LIVE under LLG_CO_DEBUG */
};

/* Anchor prefix, stored immediately before an anchored callee's frame. Only
 * the anchoring call site (or arena slot) pays for it; the callee's frame and
 * code are unchanged. The root needs none: its descriptor is in the chain. */
struct llg_co_anchor {
    const llg_co_desc_t* desc; /* function to resume */
    llg_co_anchor_t* parent;   /* enclosing anchor; NULL = the root */
};

/* The anchored frame that follows a prefix. */
#define LLG_CO_ANCHOR_FRAME(a) \
    ((llg_co_frame_t*)((char*)(a) + sizeof(llg_co_anchor_t)))

/* Storage for an anchored call site: prefix plus the callee's frame. */
#define LLG_CO_ANCHORED(frame_type) \
    struct {                        \
        llg_co_anchor_t an;         \
        frame_type f;               \
    }
/* Hosts assert this for every frame type they place after a prefix (the
 * frame must start exactly at LLG_CO_ANCHOR_FRAME). */
#define LLG_CO_ANCHORED_OK(frame_type) \
    _Static_assert(_Alignof(frame_type) <= 16, #frame_type " must be at most 16-aligned")

/* Static metadata for one resume point, indexed by state (1..n_sites-1).
 * Used only for diagnostics and backtraces: static frames do not point to
 * their descriptor, so a backtrace follows these tables from an anchor. */
struct llg_co_site {
    const llg_co_desc_t* callee; /* static callee at this point, or NULL */
    uint32_t offset;             /* callee frame offset within this frame */
    uint32_t reserved;
    const char* loc;             /* HDL source location, or NULL */
};

/* One immutable descriptor per coroutine function, emitted `static const`. */
struct llg_co_desc {
    llg_co_fn fn;
    const char* name;           /* HDL path */
    size_t frame_size;          /* sizeof the typed frame (arena calls) */
    const llg_co_site_t* sites; /* sites[state]; may be NULL */
    uint32_t n_sites;           /* number of entries in sites */
    uint32_t reserved;
};

/* Debug liveness tag. A frame is LIVE from entry until it returns DONE. The
 * checks catch entering a frame that is still live (union or overlay slot
 * reused too early) and resuming a frame whose header was overwritten. An
 * exact magic value, not a bit, so poisoned or reused storage does not look
 * live by accident. Release builds only store 0. */
#define LLG_CO_FRAME_LIVE 0x4C495645u

/* Corrupted resume state or debug check failure. The runtime may define this
 * to a diagnostic naming the C function `fn` (a string); it must not return. */
#ifndef LLG_CO_BAD_STATE
#define LLG_CO_BAD_STATE(co, fn) llg_co_bad_state((co), (fn))
#endif

/* Poison only dead storage, after registered owners have been drained. Fresh
 * callee payloads are poisoned before arguments and declaration initialization;
 * CALL macros accept populated frames and never overwrite their payloads.
 * This exposes stale bytes in diagnostics; it does not trap arbitrary reads. */
#define LLG_CO_POISON_BYTE 0xA5
#ifdef LLG_CO_DEBUG
#define LLG_CO_DEBUG_POISON(storage, bytes) \
    memset((storage), LLG_CO_POISON_BYTE, (bytes))
/* Used as a statement inside a generated loop, without a do/while wrapper:
 * the break must leave that loop before it can reread poisoned condition data. */
#define LLG_CO_DEBUG_POISON_LOOP_EXIT(condition, storage, bytes) \
    if (condition) { LLG_CO_DEBUG_POISON(storage, bytes); break; }
#define LLG_CO_DEBUG_POISON_FRAME(frame, bytes) \
    LLG_CO_DEBUG_POISON((char*)(frame) + sizeof(llg_co_frame_t), \
                        (bytes) - sizeof(llg_co_frame_t))
#define LLG_CO_DBG_ENTER_(f)                                                \
    do {                                                                    \
        if ((f)->flags == LLG_CO_FRAME_LIVE) LLG_CO_BAD_STATE(f, __func__); \
        (f)->flags = LLG_CO_FRAME_LIVE;                                     \
    } while (0)
#define LLG_CO_DBG_LIVE_(f)                                                 \
    do {                                                                    \
        if ((f)->flags != LLG_CO_FRAME_LIVE) LLG_CO_BAD_STATE(f, __func__); \
    } while (0)
#define LLG_CO_DBG_DONE_(f) ((f)->flags = 0)
#define LLG_CO_DBG_FRESH_(f) ((f)->flags = 0)
#else
#define LLG_CO_DEBUG_POISON_LOOP_EXIT(condition, storage, bytes)
#define LLG_CO_DEBUG_POISON(storage, bytes)
#define LLG_CO_DEBUG_POISON_FRAME(frame, bytes)
#define LLG_CO_DBG_ENTER_(f) ((f)->flags = 0)
#define LLG_CO_DBG_LIVE_(f) ((void)0)
#define LLG_CO_DBG_DONE_(f) ((void)0)
#define LLG_CO_DBG_FRESH_(f) ((void)0)
#endif

/* ── LIFO frame arena ────────────────────────────────────────────────────
 * Storage for dynamic callee frames. Chunks never move, so frame addresses
 * stay stable while suspended. Allocation is strictly LIFO, so a pop needs no
 * per-allocation header: the popped pointer is the chunk's previous fill
 * level. A chunk is released as soon as it empties, so an idle chain retains
 * no arena memory. The first chunk is allocated on the first push only;
 * chains that use only embedded frames never allocate one.
 *
 * Chunk memory comes from malloc/free, or from host functions when llg_co.c
 * is compiled with LLG_CO_HOST_ALLOC. The runtime uses that to serve all
 * chains from one bounded per-thread chunk cache, which makes a release and
 * re-acquire at a call boundary a free-list operation and keeps the
 * allocation counters.
 */
/* MSVC declares max_align_t only in its C++ library, not in C11 mode. The
 * union spans the fundamental types and matches MSVC's C++ definition
 * (double, 8-byte aligned) on every MSVC target. */
#if defined(_MSC_VER) && !defined(__clang__)
typedef union llg_co_max_align {
    long long ll;
    long double ld;
    void* p;
    void (*fn)(void);
} llg_co_max_align_t;
#else
typedef max_align_t llg_co_max_align_t;
#endif

typedef struct llg_co_chunk llg_co_chunk_t;
struct llg_co_chunk {
    llg_co_chunk_t* prev;
    size_t size; /* usable bytes in data[] */
    size_t used;
    llg_co_max_align_t data[];
};

typedef struct llg_co_arena {
    llg_co_chunk_t* head;
} llg_co_arena_t;

#ifdef LLG_CO_HOST_ALLOC
/* Provided by the host when llg_co.c is built with LLG_CO_HOST_ALLOC.
 * `bytes` includes the chunk header; alloc returns NULL on failure. */
void* llg_co_host_chunk_alloc(size_t bytes);
void llg_co_host_chunk_free(void* chunk, size_t bytes);
#endif

#ifndef LLG_CO_ARENA_MIN_CHUNK
#define LLG_CO_ARENA_MIN_CHUNK 1024u
#endif

/* Allocation failure policy for arena calls. The runtime defines this before
 * including the header and maps it to its fatal-allocation path. */
#ifndef LLG_CO_OOM
#define LLG_CO_OOM(bytes) abort()
#endif

#define LLG_CO_ALIGN_UP(n) \
    (((n) + (_Alignof(llg_co_max_align_t) - 1)) & ~(size_t)(_Alignof(llg_co_max_align_t) - 1))

#define llg_co_arena_grow LLG_CO_SYM(llg_co_arena_grow, LLG_CO_ABI_VERSION)
#define llg_co_arena_retire LLG_CO_SYM(llg_co_arena_retire, LLG_CO_ABI_VERSION)
#define llg_co_arena_release LLG_CO_SYM(llg_co_arena_release, LLG_CO_ABI_VERSION)
#define llg_co_bad_state LLG_CO_SYM(llg_co_bad_state, LLG_CO_ABI_VERSION)
#define llg_co_backtrace LLG_CO_SYM(llg_co_backtrace, LLG_CO_ABI_VERSION)
#define llg_co_anchor_resume LLG_CO_SYM(llg_co_anchor_resume, LLG_CO_ABI_VERSION)

/* Cold paths, implemented in llg_co.c. */
void* llg_co_arena_grow(llg_co_arena_t* arena, size_t need); /* NULL on OOM */
void llg_co_arena_retire(llg_co_arena_t* arena);             /* head chunk emptied */
void llg_co_arena_release(llg_co_arena_t* arena);            /* teardown/cancel */
LLG_CO_NORETURN void llg_co_bad_state(const llg_co_frame_t* co, const char* fn);

/* Push `size` bytes aligned for any object. Returns NULL on OOM. */
LLG_CO_INLINE void* llg_co_arena_push(llg_co_arena_t* arena, size_t size) {
    llg_co_chunk_t* c = arena->head;
    if (LLG_CO_LIKELY(size <= SIZE_MAX / 2)) {
        size_t need = LLG_CO_ALIGN_UP(size);
        if (LLG_CO_LIKELY(c && c->size - c->used >= need)) {
            void* p = (char*)c->data + c->used;
            c->used += need;
            return p;
        }
        return llg_co_arena_grow(arena, need);
    }
    return NULL;
}

/* Pop the most recent push. `p` must be exactly that allocation. */
LLG_CO_INLINE void llg_co_arena_pop(llg_co_arena_t* arena, void* p) {
    llg_co_chunk_t* c = arena->head;
    c->used = (size_t)((char*)p - (char*)c->data);
    if (LLG_CO_UNLIKELY(c->used == 0)) llg_co_arena_retire(arena);
}

/* ── Chain ────────────────────────────────────────────────────────────── */
#define LLG_CO_CHAIN_RUNNING 1u /* debug: a resume is in progress */

struct llg_co_chain {
    void* owner;               /* scheduler record (llg_proc_t*) */
    llg_co_arena_t arena;
    uint32_t exiting;          /* set by the runtime: unwind without resuming */
    uint32_t flags;            /* LLG_CO_CHAIN_*; spare bits for the runtime */
    const llg_co_desc_t* root; /* the root frame's descriptor */
    llg_co_anchor_t* top;      /* innermost non-root anchor; NULL = root */
    llg_co_fn resume;          /* root->fn, llg_co_anchor_resume, or NULL
                                * once the root finished. Last, so it shares
                                * a line with the root frame header. */
};

/* Anchor loop installed in ch->resume while a dynamic callee is active. */
llg_co_status_t llg_co_anchor_resume(llg_co_frame_t* root, llg_co_chain_t* ch);

/* The root frame is stored immediately after the chain (sizeof is a multiple
 * of 8 and frames are at most 8-aligned), so its address is computed, never
 * loaded. Hosts allocate `{ llg_co_chain_t; root frame }` contiguously. */
#define LLG_CO_ROOT(ch) ((llg_co_frame_t*)((char*)(ch) + sizeof(llg_co_chain_t)))
/* Hosts assert this for every root frame type they place after a chain. */
#define LLG_CO_ROOT_FRAME_OK(frame_type) \
    _Static_assert(_Alignof(frame_type) <= 8, #frame_type " must be at most 8-aligned")
#define LLG_CO_OWNER(ch, type) ((type*)(ch)->owner)

LLG_CO_INLINE void llg_co_start(llg_co_chain_t* ch, const llg_co_desc_t* root,
                                void* owner) {
    llg_co_frame_t* f = LLG_CO_ROOT(ch);
    f->state = 0;
    f->flags = 0;
    LLG_CO_DBG_ENTER_(f);
    ch->root = root;
    ch->top = NULL;
    ch->resume = root->fn;
    ch->exiting = 0;
    ch->flags = 0;
    ch->arena.head = NULL;
    ch->owner = owner;
}

/* Resume the chain: one load and one indirect call in the common case. The
 * root (or, while a dynamic callee is active, the anchor loop) is re-entered
 * and polls down to the suspended frame. Returns DONE when the root ends,
 * PENDING when suspended, EXIT when the chain must terminate. */
LLG_CO_INLINE llg_co_status_t llg_co_run(llg_co_chain_t* ch) {
    llg_co_status_t s;
#ifdef LLG_CO_DEBUG
    if (!ch->resume)
        LLG_CO_BAD_STATE(LLG_CO_ROOT(ch), "llg_co_run: finished chain");
    if (ch->flags & LLG_CO_CHAIN_RUNNING)
        LLG_CO_BAD_STATE(LLG_CO_ROOT(ch), "llg_co_run: reentrant chain");
    ch->flags |= LLG_CO_CHAIN_RUNNING;
#endif
    do {
        LLG_CO_DBG_LIVE_(LLG_CO_ROOT(ch));
        s = ch->resume(LLG_CO_ROOT(ch), ch);
    }
    while (LLG_CO_UNLIKELY(s == LLG_CO_CALLED));
    if (LLG_CO_UNLIKELY(s == LLG_CO_DONE)) { /* only the root returns DONE */
        LLG_CO_DBG_DONE_(LLG_CO_ROOT(ch));
        ch->resume = NULL; /* resuming a finished chain faults at once */
    }
#ifdef LLG_CO_DEBUG
    ch->flags &= ~LLG_CO_CHAIN_RUNNING;
#endif
    return s;
}

/* Diagnostics: visit every live frame. Anchors are visited innermost first;
 * within an anchor, frames are visited from the anchor down through its
 * static callees (following the descriptors' site tables). Returns the
 * number of frames visited. */
typedef void (*llg_co_visit_fn)(void* user, const llg_co_desc_t* desc,
                                const llg_co_frame_t* frame, unsigned depth);
size_t llg_co_backtrace(const llg_co_chain_t* ch, llg_co_visit_fn visit,
                        void* user);

/* ── Code-generation macros ──────────────────────────────────────────────
 * The emitter numbers resume points 1..N per function and emits one
 * LLG_CO_RESUME_CASE per point. Labels are function-scoped, so resume points
 * may sit inside loops, conditionals or nested C `switch` statements.
 * `n` must be a decimal literal (it is pasted into a label name).
 */
#define LLG_CO_DISPATCH_BEGIN(co) \
    switch ((co)->state) {        \
    case 0:                       \
        break;
#define LLG_CO_RESUME_CASE(n) \
    case (n):                 \
        goto llg_co_resume_##n;
#define LLG_CO_DISPATCH_END(co)         \
    default:                            \
        LLG_CO_BAD_STATE(co, __func__); \
        return LLG_CO_EXIT;             \
        }

/* Unconditional suspension (the caller already registered its waiter). */
#define LLG_CO_SUSPEND(co, ch, n) \
    do {                          \
        (void)(ch);               \
        (co)->state = (n);        \
        return LLG_CO_PENDING;    \
    } while (0);                  \
    llg_co_resume_##n:            \
    (void)0

/* Arm once; suspend if needed; continue after the wake. The runtime delivers
 * the result (key, message, order flag) before waking, so no re-arm. */
#define LLG_CO_AWAIT(co, ch, n, arm_expr)                  \
    do {                                                   \
        llg_co_arm_t llg_co_r_ = (arm_expr);               \
        (void)(ch);                                        \
        if (llg_co_r_ == LLG_CO_ARM_SUSPEND) {             \
            (co)->state = (n);                             \
            return LLG_CO_PENDING;                         \
        }                                                  \
        if (LLG_CO_UNLIKELY(llg_co_r_ == LLG_CO_ARM_EXIT)) \
            return LLG_CO_EXIT;                            \
    } while (0);                                           \
    llg_co_resume_##n:                                     \
    (void)0

/* Re-arm after every wake until READY, for a future operation that cannot
 * deliver on wake. The arm expression may read only frame fields/globals. */
#define LLG_CO_AWAIT_RETRY(co, ch, n, arm_expr)            \
    llg_co_resume_##n:                                     \
    do {                                                   \
        llg_co_arm_t llg_co_r_ = (arm_expr);               \
        (void)(ch);                                        \
        if (llg_co_r_ == LLG_CO_ARM_SUSPEND) {             \
            (co)->state = (n);                             \
            return LLG_CO_PENDING;                         \
        }                                                  \
        if (LLG_CO_UNLIKELY(llg_co_r_ == LLG_CO_ARM_EXIT)) \
            return LLG_CO_EXIT;                            \
    } while (0)

/* Call a static suspendable callee `fn` whose frame `child` is embedded in
 * this frame (a direct, inlinable call). Arguments must already be stored in
 * the child frame. The resume label sits *before* the call: on resume the
 * dispatch lands here and calls the callee again, which continues from its
 * own recorded state. A callee that finished after termination was requested
 * does not return to this caller's normal path. */
#define LLG_CO_CALL(co, ch, n, fn, child)                       \
    (child)->state = 0;                                         \
    LLG_CO_DBG_ENTER_(child);                                   \
    llg_co_resume_##n:                                          \
    do {                                                        \
        llg_co_status_t llg_co_s_;                              \
        LLG_CO_DBG_LIVE_(child);                                \
        llg_co_s_ = (fn)((child), (ch));                        \
        if (llg_co_s_ != LLG_CO_DONE) {                         \
            /* PENDING or CALLED: resume at this call later. */ \
            if (LLG_CO_LIKELY(llg_co_s_ != LLG_CO_EXIT))        \
                (co)->state = (n);                              \
            return llg_co_s_;                                   \
        }                                                       \
        LLG_CO_DBG_DONE_(child);                                \
        if (LLG_CO_UNLIKELY((ch)->exiting)) return LLG_CO_EXIT; \
    } while (0)

/* Enter a callee as an anchor through the trampoline: `slot` is the
 * llg_co_anchor_t* prefix of caller-owned storage (an embedded
 * LLG_CO_ANCHORED member, or pushed with LLG_CO_ARENA_ENTER) whose frame,
 * at LLG_CO_ANCHOR_FRAME(slot), already holds the arguments. Every static frame
 * up to the current anchor records its state and returns CALLED; llg_co_run
 * then runs the callee. When it finishes, the current anchor is resumed,
 * polls back to this call site and continues after the label. Constant
 * native stack for any recursion depth, for about one extra dispatch per
 * enclosing static level. */
#define LLG_CO_CALL_ANCHOR(co, ch, n, dsc, slot)                     \
    do {                                                             \
        llg_co_anchor_t* llg_co_a_ = (slot);                         \
        llg_co_frame_t* llg_co_af_ = LLG_CO_ANCHOR_FRAME(llg_co_a_); \
        llg_co_af_->state = 0;                                       \
        LLG_CO_DBG_ENTER_(llg_co_af_);                               \
        llg_co_a_->desc = (dsc);                                     \
        llg_co_a_->parent = (ch)->top;                               \
        (ch)->top = llg_co_a_;                                       \
        (ch)->resume = llg_co_anchor_resume;                         \
        (co)->state = (n);                                           \
        return LLG_CO_CALLED;                                        \
    } while (0);                                                     \
    llg_co_resume_##n:                                               \
    (void)0

/* Dynamic callee: push prefix + frame into `slot` (a frame field of type
 * llg_co_anchor_t*), store the arguments in LLG_CO_ANCHOR_FRAME(slot), then
 * LLG_CO_CALL_ARENA. */
#define LLG_CO_ARENA_ENTER(ch, dsc, slot)                                      \
    do {                                                                       \
        size_t llg_co_z_ = sizeof(llg_co_anchor_t) + (dsc)->frame_size;        \
        (slot) = (llg_co_anchor_t*)llg_co_arena_push(&(ch)->arena, llg_co_z_); \
        if (LLG_CO_UNLIKELY(!(slot))) LLG_CO_OOM(llg_co_z_);                   \
        LLG_CO_DEBUG_POISON_FRAME(LLG_CO_ANCHOR_FRAME(slot), (dsc)->frame_size); \
        /* A cancelled activation can leave a LIVE tag in a cached chunk. */ \
        LLG_CO_DBG_FRESH_(LLG_CO_ANCHOR_FRAME(slot));                         \
    } while (0)

/* Dynamic call through the arena: anchor call, then pop once it finished. */
#define LLG_CO_CALL_ARENA(co, ch, n, dsc, slot) \
    LLG_CO_CALL_ANCHOR(co, ch, n, dsc, (slot)); \
    llg_co_arena_pop(&(ch)->arena, (slot));     \
    (slot) = NULL

/* Leave after a runtime operation that may have requested termination. */
#define LLG_CO_EXIT_CHECK(ch)                                   \
    do {                                                        \
        if (LLG_CO_UNLIKELY((ch)->exiting)) return LLG_CO_EXIT; \
    } while (0)

#ifdef __cplusplus
}
#endif

#endif /* LLG_CO_H */
