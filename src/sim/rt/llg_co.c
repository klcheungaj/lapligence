/* llg_co.c - cold paths of the stackless coroutine library.
 *
 * Everything on the resume/suspend/call fast path is inline in llg_co.h.
 * This translation unit holds only arena chunk management and the
 * corrupted-state handler, so generated models do not duplicate them.
 * No function here touches global mutable state.
 */
#include "llg_co.h"

#include <stdlib.h>

#ifdef LLG_CO_HOST_ALLOC
#define LLG_CO_CHUNK_ALLOC(bytes) llg_co_host_chunk_alloc(bytes)
#define LLG_CO_CHUNK_FREE(c) \
    llg_co_host_chunk_free((c), sizeof(llg_co_chunk_t) + (c)->size)
#else
#define LLG_CO_CHUNK_ALLOC(bytes) malloc(bytes)
#define LLG_CO_CHUNK_FREE(c) free(c)
#endif

void* llg_co_arena_grow(llg_co_arena_t* arena, size_t need) {
    llg_co_chunk_t* c = arena->head;
    llg_co_chunk_t* s;
    /* Geometric growth bounds the number of chunks for deep chains. */
    size_t cap = c ? c->size : LLG_CO_ARENA_MIN_CHUNK;
    if (c && cap <= (SIZE_MAX - sizeof(llg_co_chunk_t)) / 2) cap *= 2;
    if (cap < need) cap = need;
    if (cap > SIZE_MAX - sizeof(llg_co_chunk_t)) return NULL;
    s = (llg_co_chunk_t*)LLG_CO_CHUNK_ALLOC(sizeof(llg_co_chunk_t) + cap);
    if (!s) return NULL;
    s->size = cap;
    s->used = need;
    s->prev = c;
    arena->head = s;
    return s->data;
}

void llg_co_arena_retire(llg_co_arena_t* arena) {
    llg_co_chunk_t* c = arena->head;
    arena->head = c->prev;
    LLG_CO_CHUNK_FREE(c);
}

void llg_co_arena_release(llg_co_arena_t* arena) {
    llg_co_chunk_t* c = arena->head;
    while (c) {
        llg_co_chunk_t* prev = c->prev;
        LLG_CO_CHUNK_FREE(c);
        c = prev;
    }
    arena->head = NULL;
}

void llg_co_bad_state(const llg_co_frame_t* co, const char* fn) {
    (void)co;
    (void)fn;
    abort();
}

/* Visit one anchor's static path: follow the site table of each frame's
 * current state to the callee frame embedded at the recorded offset. */
static size_t visit_static(const llg_co_desc_t* d, const llg_co_frame_t* f,
                           llg_co_visit_fn visit, void* user, size_t n) {
    while (d && f) {
        visit(user, d, f, (unsigned)n++);
        if (!d->sites || f->state >= d->n_sites) break;
        const llg_co_site_t* site = &d->sites[f->state];
        /* A leaf wait, or an anchored call: anchored callees are visited from
         * the anchor chain, so their sites record no static callee. */
        if (!site->callee) break;
        f = (const llg_co_frame_t*)((const char*)f + site->offset);
        d = site->callee;
    }
    return n;
}

llg_co_status_t llg_co_anchor_resume(llg_co_frame_t* root, llg_co_chain_t* ch) {
    (void)root;
    for (;;) {
        llg_co_anchor_t* a = ch->top;
        llg_co_status_t s;
        llg_co_frame_t* f = LLG_CO_ANCHOR_FRAME(a);
        LLG_CO_DBG_LIVE_(f);
        s = a->desc->fn(f, ch);
        if (s == LLG_CO_CALLED) continue; /* a deeper anchor was entered */
        if (s != LLG_CO_DONE) return s;   /* PENDING or EXIT */
        LLG_CO_DBG_DONE_(f);
        /* A dynamic callee finished: resume its parent anchor, which polls
         * down to the call site and pops the callee there. An anchor that
         * finished after termination was requested must not hand control
         * back to its caller's normal path. */
        ch->top = a->parent;
        if (LLG_CO_UNLIKELY(ch->exiting)) return LLG_CO_EXIT;
        if (!ch->top) {
            ch->resume = ch->root->fn; /* back to the root, same turn */
            return LLG_CO_CALLED;
        }
    }
}

size_t llg_co_backtrace(const llg_co_chain_t* ch, llg_co_visit_fn visit,
                        void* user) {
    size_t n = 0;
    if (!ch->resume) return 0; /* finished */
    for (const llg_co_anchor_t* a = ch->top; a; a = a->parent)
        n = visit_static(a->desc, LLG_CO_ANCHOR_FRAME(a), visit, user, n);
    n = visit_static(ch->root, LLG_CO_ROOT(ch), visit, user, n);
    return n;
}
