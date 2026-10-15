//! SIM-030 acceptance for the solver runtime: problems with units (objects
//! reached through `rand` handles, arrays, `foreach`, reductions and random
//! array indices) expanded per call (`constraints/expand.c`).
//!
//! The probe builds template problems by hand and checks: cross-object
//! constraints and per-object constraints, object deduplication (an object
//! reached twice is one instance; a self-reference is the root), fixed-array
//! `foreach` with declared indices, sizes solved first (S30-D5: the size of a
//! dynamic array is uniform over the feasible sizes, not weighted by the
//! number of element solutions), sizes no constraint names kept, random
//! indices excluding out-of-range positions (S30-D11), null handles as
//! evaluation errors a guard eliminates, empty reductions as identities
//! (S30-D6), per-object `randc` cycles, function calls, non-random objects
//! as state and their promotion when also reached through a random handle,
//! and the expansion budgets.

use super::*;
use std::collections::BTreeSet;

const EXPAND_MAIN: &str = r#"
#include "llg_constraint.h"
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static uint32_t rng_next(void* state) {
    uint64_t* s = (uint64_t*)state;
    uint64_t x = *s;
    x ^= x << 13; x ^= x >> 7; x ^= x << 17;
    *s = x;
    return (uint32_t)(x >> 32);
}

typedef struct {
    llg_cr_node_t nodes[256]; uint32_t nn;
    uint64_t consts[256]; uint32_t nk;
    uint32_t args[256]; uint32_t na;
    llg_cr_var_t vars[16]; uint32_t nv;
    llg_cr_block_t blocks[8]; uint32_t nb;
    llg_cr_unit_t units[4]; uint32_t nu;
    uint32_t items[32]; uint32_t ni;
    llg_cr_array_t arrays[4]; uint32_t nar;
    llg_cr_slot_t slots[4]; uint32_t ns;
    llg_cr_call_t calls[2]; uint32_t nc;
    llg_cr_problem_t p;
    llg_cr_session_t* s;
} B;

static uint32_t nd(B* b, uint16_t op, uint32_t w, int sg, uint32_t a, uint32_t x, uint32_t c) {
    llg_cr_node_t* n = &b->nodes[b->nn];
    n->op = op; n->sign_flag = (uint8_t)sg; n->reserved = 0; n->bit_width = w;
    n->a = a; n->b = x; n->c = c;
    return b->nn++;
}
static uint32_t k(B* b, int64_t v, uint32_t w, int sg) {
    b->consts[b->nk] = (uint64_t)v & (w >= 64u ? ~UINT64_C(0) : ((UINT64_C(1) << w) - 1u));
    return nd(b, LLG_CR_CONST, w, sg, b->nk++, 0, 0);
}
static uint32_t newvar(B* b, const char* name, uint32_t w, int sg, int cyclic) {
    llg_cr_var_t* v = &b->vars[b->nv];
    v->name = name; v->bit_width = w; v->sign_flag = (uint8_t)sg;
    v->domain_node = LLG_CR_NONE; v->cyclic = (uint8_t)cyclic;
    return b->nv++;
}
static uint32_t vr(B* b, uint32_t v) {
    return nd(b, LLG_CR_VAR, b->vars[v].bit_width, b->vars[v].sign_flag, v, 0, 0);
}
static uint32_t blk(B* b, const char* name, uint32_t root) {
    b->blocks[b->nb].name = name; b->blocks[b->nb].root = root;
    return b->nb++;
}
static uint32_t arr(B* b, const char* name, uint32_t w, int sg, uint8_t kind, int32_t l, int32_t r) {
    llg_cr_array_t* a = &b->arrays[b->nar];
    a->name = name; a->elem_width = w; a->elem_signed = (uint8_t)sg; a->kind = kind;
    a->left = l; a->right = r;
    return b->nar++;
}
static uint32_t slot(B* b, const char* name, int is_array, uint8_t kind) {
    llg_cr_slot_t* s = &b->slots[b->ns];
    s->name = name; s->kind = kind; s->is_array = (uint8_t)is_array; s->left = 0; s->right = 0;
    return b->ns++;
}
/* Unit with listed vars, arrays, slots and blocks [bf, bf+bc). */
static void unit(B* b, const char* name, const uint32_t* v, uint32_t nv, const uint32_t* a,
                 uint32_t na, const uint32_t* s, uint32_t ns, uint32_t bf, uint32_t bc) {
    llg_cr_unit_t* u = &b->units[b->nu++];
    memset(u, 0, sizeof(*u));
    u->name = name; u->item_first = b->ni;
    u->var_count = nv; u->array_count = na; u->slot_count = ns;
    for (uint32_t i = 0; i < nv; ++i) b->items[b->ni++] = v[i];
    for (uint32_t i = 0; i < na; ++i) b->items[b->ni++] = a[i];
    for (uint32_t i = 0; i < ns; ++i) b->items[b->ni++] = s[i];
    u->block_first = bf; u->block_count = bc;
}
static uint32_t loopn(B* b, uint16_t op, uint32_t w, int sg, uint32_t array, uint32_t body,
                      uint32_t ref, uint32_t loop, uint32_t rop) {
    uint32_t first = b->na;
    b->args[b->na++] = array; b->args[b->na++] = body; b->args[b->na++] = ref;
    b->args[b->na++] = loop;
    if (op == LLG_CR_REDUCE) b->args[b->na++] = rop;
    return nd(b, op, w, sg, 0, first, 0);
}
static uint32_t all2(B* b, uint32_t x, uint32_t y) {
    uint32_t first = b->na;
    b->args[b->na++] = x; b->args[b->na++] = y;
    return nd(b, LLG_CR_ALL, 1, 0, 0, first, 2);
}
static void finish(B* b, const char* name) {
    llg_cr_problem_t* p = &b->p;
    memset(p, 0, sizeof(*p));
    p->name = name;
    p->nodes = b->nodes; p->node_count = b->nn;
    p->args = b->args; p->arg_count = b->na;
    p->consts = b->consts; p->const_count = b->nk;
    p->vars = b->vars; p->var_count = b->nv;
    p->blocks = b->blocks; p->block_count = b->nb;
    p->calls = b->calls; p->call_count = b->nc;
    p->units = b->units; p->unit_count = b->nu;
    p->unit_items = b->items; p->unit_item_count = b->ni;
    p->arrays = b->arrays; p->array_count = b->nar;
    p->slots = b->slots; p->slot_count = b->ns;
    b->s = llg_cr_session_get(p, NULL);
}

static uint64_t seed = UINT64_C(0x9E3779B97F4A7C15);
static llg_cr_rng_t rng = {rng_next, &seed};
static int obj_a, obj_b, obj_c, obj_d;
static const char* names[] = {"ok", "unsat", "resource", "error", "calls"};

static void set_blocks(B* b, uint32_t inst, int enabled) {
    const llg_cr_unit_t* u = &b->units[llg_cr_inst_unit(b->s, inst)];
    for (uint32_t i = 0; i < u->block_count; ++i) llg_cr_iset_block(b->s, inst, u->block_first + i, enabled);
}
static void set_vars(B* b, uint32_t inst, uint64_t value, int active) {
    const llg_cr_unit_t* u = &b->units[llg_cr_inst_unit(b->s, inst)];
    for (uint32_t i = 0; i < u->var_count; ++i)
        llg_cr_iset_var(b->s, inst, b->items[u->item_first + i], &value, active, LLG_CR_INPUT_KNOWN);
}

/* S1: parent.x, child.y == x + 1 (parent block, 8 bits: x = 255 gives
 * y = 0), y < 10 (child block): ten solutions. */
static void s1(unsigned n) {
    static B b;
    uint32_t x = newvar(&b, "x", 8, 0, 0), y = newvar(&b, "y", 8, 0, 0);
    uint32_t child = slot(&b, "child", 0, 0);
    uint32_t ref = nd(&b, LLG_CR_REF, 1, 0, child, LLG_CR_NONE, LLG_CR_NONE);
    uint32_t hy = nd(&b, LLG_CR_HVAR, 8, 0, ref, y, 0);
    uint32_t sum = nd(&b, LLG_CR_ADD, 8, 0, vr(&b, x), k(&b, 1, 8, 0), 0);
    blk(&b, "c0", nd(&b, LLG_CR_EQ, 1, 0, hy, sum, 0));
    blk(&b, "c1", nd(&b, LLG_CR_LT, 1, 0, vr(&b, y), k(&b, 10, 8, 0), 0));
    uint32_t v0[] = {x}, s0[] = {child}, v1[] = {y};
    unit(&b, "Parent", v0, 1, NULL, 0, s0, 1, 0, 1);
    unit(&b, "Child", v1, 1, NULL, 0, NULL, 0, 1, 1);
    finish(&b, "s1");
    unsigned hist[256] = {0}, bad = 0, feeds = 0;
    for (unsigned i = 0; i < n; ++i) {
        llg_cr_begin(b.s);
        llg_cr_inst_root(b.s, &obj_a);
        uint32_t inst;
        while ((inst = llg_cr_next_inst(b.s)) != LLG_CR_NONE) {
            ++feeds;
            set_vars(&b, inst, 200, 1);
            set_blocks(&b, inst, 1);
            if (llg_cr_inst_unit(b.s, inst) == 0) llg_cr_link(b.s, inst, child, 0, &obj_b, 1, 1);
        }
        llg_cr_status_t st = llg_cr_step(b.s, &rng, NULL);
        if (st != LLG_CR_OK) { printf("S1 status=%s\n", names[st]); return; }
        uint64_t vx = llg_cr_ivalue(b.s, 0, x)[0], vy = llg_cr_ivalue(b.s, 1, y)[0];
        if (vy != ((vx + 1u) & 255u) || vy >= 10u || llg_cr_inst_count(b.s) != 2u) ++bad;
        hist[vx & 255u]++;
    }
    printf("S1 bad=%u feeds=%u h", bad, feeds);
    for (unsigned v = 0; v < 9; ++v) printf(" %u", hist[v]);
    printf(" %u\n", hist[255]);
}

/* S2: a[3:0] fixed, foreach (a[i]) a[i] == i; a.sum() == 6 (8-bit). */
static void s2(void) {
    static B b;
    uint32_t a = arr(&b, "a", 4, 0, LLG_CR_ARRAY_FIXED, 3, 0);
    uint32_t it = nd(&b, LLG_CR_ITER, 32, 1, 0, 0, 0);
    uint32_t pos = nd(&b, LLG_CR_SUB, 32, 1, k(&b, 3, 32, 1), it, 0);
    uint32_t el = nd(&b, LLG_CR_AELEM, 4, 0, a, pos, LLG_CR_NONE);
    uint32_t body = nd(&b, LLG_CR_EQ, 1, 0, el, nd(&b, LLG_CR_RESIZE, 4, 0, it, 0, 0), 0);
    uint32_t fe = loopn(&b, LLG_CR_FOREACH, 1, 0, a, body, LLG_CR_NONE, 0, 0);
    uint32_t it1 = nd(&b, LLG_CR_ITER, 32, 1, 1, 0, 0);
    uint32_t el1 = nd(&b, LLG_CR_AELEM, 4, 0, a, nd(&b, LLG_CR_SUB, 32, 1, k(&b, 3, 32, 1), it1, 0), LLG_CR_NONE);
    uint32_t sum = loopn(&b, LLG_CR_REDUCE, 8, 0, a, el1, LLG_CR_NONE, 1, LLG_CR_ADD);
    blk(&b, "c0", all2(&b, fe, nd(&b, LLG_CR_EQ, 1, 0, sum, k(&b, 6, 8, 0), 0)));
    uint32_t a0[] = {a};
    unit(&b, "Fixed", NULL, 0, a0, 1, NULL, 0, 0, 1);
    finish(&b, "s2");
    llg_cr_begin(b.s);
    uint32_t inst;
    while ((inst = llg_cr_next_inst(b.s)) != LLG_CR_NONE) {
        set_blocks(&b, inst, 1);
        llg_cr_set_array(b.s, inst, a, 4, 1);
    }
    llg_cr_status_t st = llg_cr_step(b.s, &rng, NULL);
    printf("S2 status=%s a=", names[st]);
    for (uint32_t i = 0; i < 4 && st == LLG_CR_OK; ++i)
        printf("%llu%s", (unsigned long long)llg_cr_elem_value(b.s, 0, a, i)[0], i < 3 ? "," : "");
    printf("\n");
}

/* S3: dynamic d: size in [1:4]; foreach d[i] < 3; d.sum() == 2; x == d.size(). */
static void s3(unsigned n) {
    static B b;
    uint32_t x = newvar(&b, "x", 32, 1, 0);
    uint32_t d = arr(&b, "d", 8, 0, LLG_CR_ARRAY_DYNAMIC, 0, 0);
    uint32_t size = nd(&b, LLG_CR_ASIZE, 32, 1, d, 0, LLG_CR_NONE);
    uint32_t range = nd(&b, LLG_CR_LAND, 1, 0, nd(&b, LLG_CR_GE, 1, 0, size, k(&b, 1, 32, 1), 0),
                        nd(&b, LLG_CR_LE, 1, 0, size, k(&b, 4, 32, 1), 0), 0);
    uint32_t it = nd(&b, LLG_CR_ITER, 32, 1, 0, 0, 0);
    uint32_t el = nd(&b, LLG_CR_AELEM, 8, 0, d, it, LLG_CR_NONE);
    uint32_t fe = loopn(&b, LLG_CR_FOREACH, 1, 0, d, nd(&b, LLG_CR_LT, 1, 0, el, k(&b, 3, 8, 0), 0),
                        LLG_CR_NONE, 0, 0);
    uint32_t it1 = nd(&b, LLG_CR_ITER, 32, 1, 1, 0, 0);
    uint32_t sum = loopn(&b, LLG_CR_REDUCE, 8, 0, d, nd(&b, LLG_CR_AELEM, 8, 0, d, it1, LLG_CR_NONE),
                         LLG_CR_NONE, 1, LLG_CR_ADD);
    blk(&b, "range", range);
    blk(&b, "elems", fe);
    blk(&b, "sum", nd(&b, LLG_CR_EQ, 1, 0, sum, k(&b, 2, 8, 0), 0));
    blk(&b, "x", nd(&b, LLG_CR_EQ, 1, 0, vr(&b, x), size, 0));
    uint32_t v0[] = {x}, a0[] = {d};
    unit(&b, "Dyn", v0, 1, a0, 1, NULL, 0, 0, 4);
    finish(&b, "s3");
    unsigned sizes[5] = {0}, bad = 0;
    for (unsigned i = 0; i < n; ++i) {
        llg_cr_begin(b.s);
        uint32_t inst;
        while ((inst = llg_cr_next_inst(b.s)) != LLG_CR_NONE) {
            set_blocks(&b, inst, 1);
            set_vars(&b, inst, 0, 1);
            llg_cr_set_array(b.s, inst, d, 0, 1);
        }
        llg_cr_status_t st = llg_cr_step(b.s, &rng, NULL);
        if (st != LLG_CR_OK) { printf("S3 status=%s\n", names[st]); return; }
        uint32_t sz = llg_cr_array_size(b.s, 0, d);
        unsigned total = 0, big = 0;
        for (uint32_t e = 0; e < sz; ++e) {
            uint64_t v = llg_cr_elem_value(b.s, 0, d, e)[0];
            total += (unsigned)v;
            big |= v >= 3u;
        }
        if (sz < 1 || sz > 4 || total != 2 || big || llg_cr_ivalue(b.s, 0, x)[0] != sz) ++bad;
        else sizes[sz]++;
    }
    printf("S3 bad=%u sizes=%u,%u,%u,%u\n", bad, sizes[1], sizes[2], sizes[3], sizes[4]);
}

/* S4: dynamic d of current size 3, foreach d[i] < 5, no size constraint. */
static void s4(void) {
    static B b;
    uint32_t d = arr(&b, "d", 8, 0, LLG_CR_ARRAY_QUEUE, 0, 0);
    uint32_t it = nd(&b, LLG_CR_ITER, 32, 1, 0, 0, 0);
    uint32_t el = nd(&b, LLG_CR_AELEM, 8, 0, d, it, LLG_CR_NONE);
    blk(&b, "c0", loopn(&b, LLG_CR_FOREACH, 1, 0, d, nd(&b, LLG_CR_LT, 1, 0, el, k(&b, 5, 8, 0), 0),
                        LLG_CR_NONE, 0, 0));
    uint32_t a0[] = {d};
    unit(&b, "Q", NULL, 0, a0, 1, NULL, 0, 0, 1);
    finish(&b, "s4");
    unsigned bad = 0;
    for (unsigned i = 0; i < 50; ++i) {
        llg_cr_begin(b.s);
        uint32_t inst;
        while ((inst = llg_cr_next_inst(b.s)) != LLG_CR_NONE) {
            set_blocks(&b, inst, 1);
            llg_cr_set_array(b.s, inst, d, 3, 1);
            for (uint32_t e = 0; e < 3; ++e) {
                uint64_t v = 200;
                llg_cr_set_elem(b.s, inst, d, e, &v, LLG_CR_INPUT_KNOWN);
            }
        }
        if (llg_cr_step(b.s, &rng, NULL) != LLG_CR_OK || llg_cr_array_size(b.s, 0, d) != 3u) {
            ++bad;
            continue;
        }
        for (uint32_t e = 0; e < 3; ++e) bad += llg_cr_elem_value(b.s, 0, d, e)[0] >= 5u;
    }
    printf("S4 bad=%u\n", bad);
}

/* S5: idx (3 bits) and a[0:3]: a[idx] == 7; positions 4..7 do not exist. */
static void s5(unsigned n) {
    static B b;
    uint32_t idx = newvar(&b, "idx", 3, 0, 0);
    uint32_t a = arr(&b, "a", 8, 0, LLG_CR_ARRAY_FIXED, 0, 3);
    uint32_t el = nd(&b, LLG_CR_AELEM, 8, 0, a, vr(&b, idx), LLG_CR_NONE);
    blk(&b, "c0", nd(&b, LLG_CR_EQ, 1, 0, el, k(&b, 7, 8, 0), 0));
    uint32_t v0[] = {idx}, a0[] = {a};
    unit(&b, "R", v0, 1, a0, 1, NULL, 0, 0, 1);
    finish(&b, "s5");
    unsigned hist[8] = {0}, bad = 0;
    for (unsigned i = 0; i < n; ++i) {
        llg_cr_begin(b.s);
        uint32_t inst;
        while ((inst = llg_cr_next_inst(b.s)) != LLG_CR_NONE) {
            set_blocks(&b, inst, 1);
            set_vars(&b, inst, 0, 1);
            llg_cr_set_array(b.s, inst, a, 4, 1);
        }
        if (llg_cr_step(b.s, &rng, NULL) != LLG_CR_OK) { ++bad; continue; }
        uint64_t v = llg_cr_ivalue(b.s, 0, idx)[0];
        if (v > 3u || llg_cr_elem_value(b.s, 0, a, (uint32_t)v)[0] != 7u) ++bad;
        hist[v & 7u]++;
    }
    printf("S5 bad=%u h %u %u %u %u %u\n", bad, hist[0], hist[1], hist[2], hist[3],
           hist[4] + hist[5] + hist[6] + hist[7]);
}

/* S6: child.y > 0 with a null child: unguarded is a null error, guarded
 * (child != null -> ...) holds. */
static void s6(void) {
    static B b;
    uint32_t y = newvar(&b, "y", 8, 0, 0), z = newvar(&b, "z", 8, 0, 0);
    uint32_t child = slot(&b, "child", 0, 0);
    uint32_t ref = nd(&b, LLG_CR_REF, 1, 0, child, LLG_CR_NONE, LLG_CR_NONE);
    uint32_t gt = nd(&b, LLG_CR_GT, 1, 0, nd(&b, LLG_CR_HVAR, 8, 0, ref, y, 0), k(&b, 0, 8, 0), 0);
    uint32_t live = nd(&b, LLG_CR_LNOT, 1, 0, nd(&b, LLG_CR_ISNULL, 1, 0, ref, 0, 0), 0, 0);
    blk(&b, "plain", gt);
    blk(&b, "guarded", nd(&b, LLG_CR_IMPL, 1, 0, live, gt, 0));
    uint32_t v0[] = {z}, s0[] = {child}, v1[] = {y};
    unit(&b, "P", v0, 1, NULL, 0, s0, 1, 0, 2);
    unit(&b, "C", v1, 1, NULL, 0, NULL, 0, 2, 0);
    finish(&b, "s6");
    for (int plain = 1; plain >= 0; --plain) {
        llg_cr_begin(b.s);
        uint32_t inst;
        while ((inst = llg_cr_next_inst(b.s)) != LLG_CR_NONE) {
            set_vars(&b, inst, 0, 1);
            llg_cr_iset_block(b.s, inst, 0, plain);
            llg_cr_iset_block(b.s, inst, 1, !plain);
            llg_cr_link(b.s, inst, child, 0, NULL, 1, 1);
        }
        llg_cr_report_t report;
        llg_cr_status_t st = llg_cr_step(b.s, &rng, &report);
        printf("S6 %s status=%s reason=%d\n", plain ? "plain" : "guarded", names[st],
               st == LLG_CR_ERROR ? report.reason : 0);
    }
}

/* S7: kids[] handle array: foreach (kids[i]) kids[i].y == i; then the same
 * object as kids[0] and kids[2] (one instance: unsatisfiable). */
static void s7(void) {
    static B b;
    uint32_t y = newvar(&b, "y", 8, 0, 0);
    uint32_t kids = slot(&b, "kids", 1, LLG_CR_ARRAY_DYNAMIC);
    uint32_t it = nd(&b, LLG_CR_ITER, 32, 1, 0, 0, 0);
    uint32_t ref = nd(&b, LLG_CR_REF, 1, 0, kids, LLG_CR_NONE, it);
    uint32_t body = nd(&b, LLG_CR_EQ, 1, 0, nd(&b, LLG_CR_HVAR, 8, 0, ref, y, 0),
                       nd(&b, LLG_CR_RESIZE, 8, 0, it, 0, 0), 0);
    blk(&b, "c0", loopn(&b, LLG_CR_FOREACH, 1, 0, LLG_CR_SLOT_ARRAY | kids, body, LLG_CR_NONE, 0, 0));
    uint32_t s0[] = {kids}, v1[] = {y};
    unit(&b, "P", NULL, 0, NULL, 0, s0, 1, 0, 1);
    unit(&b, "K", v1, 1, NULL, 0, NULL, 0, 1, 0);
    finish(&b, "s7");
    for (int same = 0; same < 2; ++same) {
        llg_cr_begin(b.s);
        llg_cr_inst_root(b.s, &obj_a);
        uint32_t inst;
        while ((inst = llg_cr_next_inst(b.s)) != LLG_CR_NONE) {
            set_blocks(&b, inst, 1);
            set_vars(&b, inst, 99, 1);
            if (llg_cr_inst_unit(b.s, inst) != 0) continue;
            llg_cr_set_slot_size(b.s, inst, kids, 3);
            llg_cr_link(b.s, inst, kids, 0, &obj_b, 1, 1);
            llg_cr_link(b.s, inst, kids, 1, &obj_c, 1, 1);
            llg_cr_link(b.s, inst, kids, 2, same ? (void*)&obj_b : (void*)&obj_d, 1, 1);
        }
        llg_cr_status_t st = llg_cr_step(b.s, &rng, NULL);
        printf("S7 same=%d status=%s instances=%u", same, names[st], llg_cr_inst_count(b.s));
        for (uint32_t i = 1; i < llg_cr_inst_count(b.s) && st == LLG_CR_OK; ++i)
            printf(" %llu", (unsigned long long)llg_cr_ivalue(b.s, i, y)[0]);
        printf("\n");
    }
}

/* S8: two kids with a randc r (3 bits) each: 16 calls, two cycles per kid. */
static void s8(void) {
    static B b;
    uint32_t r = newvar(&b, "r", 3, 0, 1);
    uint32_t k0 = slot(&b, "k0", 0, 0), k1 = slot(&b, "k1", 0, 0);
    uint32_t s0[] = {k0, k1}, v1[] = {r};
    unit(&b, "P", NULL, 0, NULL, 0, s0, 2, 0, 0);
    unit(&b, "K", v1, 1, NULL, 0, NULL, 0, 0, 0);
    finish(&b, "s8");
    uint64_t state[2][2] = {{0, 0}, {0, 0}};
    int* handles[2] = {&obj_b, &obj_c};
    printf("S8");
    for (unsigned call = 0; call < 16; ++call) {
        llg_cr_begin(b.s);
        llg_cr_inst_root(b.s, &obj_a);
        uint32_t inst;
        while ((inst = llg_cr_next_inst(b.s)) != LLG_CR_NONE) {
            if (llg_cr_inst_unit(b.s, inst) == 0) {
                llg_cr_link(b.s, inst, k0, 0, handles[0], 1, 1);
                llg_cr_link(b.s, inst, k1, 0, handles[1], 1, 1);
                continue;
            }
            set_vars(&b, inst, 0, 1);
            int which = llg_cr_inst_handle(b.s, inst) == (void*)handles[1];
            llg_cr_iset_cyclic(b.s, inst, r, state[which]);
        }
        if (llg_cr_step(b.s, &rng, NULL) != LLG_CR_OK) { printf(" fail\n"); return; }
        for (uint32_t i = 1; i < 3; ++i) {
            int which = llg_cr_inst_handle(b.s, i) == (void*)handles[1];
            memcpy(state[which], llg_cr_icyclic(b.s, i, r), sizeof(state[which]));
        }
        printf(" %llu:%llu", (unsigned long long)llg_cr_ivalue(b.s, 1, r)[0],
               (unsigned long long)llg_cr_ivalue(b.s, 2, r)[0]);
    }
    printf("\n");
}

/* S9: self.next == self: next.x == 5 constrains x itself. */
static void s9(void) {
    static B b;
    uint32_t x = newvar(&b, "x", 8, 0, 0);
    uint32_t next = slot(&b, "next", 0, 0);
    uint32_t ref = nd(&b, LLG_CR_REF, 1, 0, next, LLG_CR_NONE, LLG_CR_NONE);
    blk(&b, "c0", nd(&b, LLG_CR_EQ, 1, 0, nd(&b, LLG_CR_HVAR, 8, 0, ref, x, 0), k(&b, 5, 8, 0), 0));
    uint32_t v0[] = {x}, s0[] = {next};
    unit(&b, "Node", v0, 1, NULL, 0, s0, 1, 0, 1);
    finish(&b, "s9");
    llg_cr_begin(b.s);
    llg_cr_inst_root(b.s, &obj_a);
    uint32_t inst;
    while ((inst = llg_cr_next_inst(b.s)) != LLG_CR_NONE) {
        set_blocks(&b, inst, 1);
        set_vars(&b, inst, 0, 1);
        llg_cr_link(b.s, inst, next, 0, &obj_a, 0, 1);
    }
    llg_cr_status_t st = llg_cr_step(b.s, &rng, NULL);
    printf("S9 status=%s instances=%u x=%llu\n", names[st], llg_cr_inst_count(b.s),
           (unsigned long long)llg_cr_value(b.s, x)[0]);
}

/* S10: reductions of an empty (non-random) array are identities. */
static void s10(void) {
    static B b;
    uint32_t z = newvar(&b, "z", 1, 0, 0);
    uint32_t e = arr(&b, "e", 8, 0, LLG_CR_ARRAY_DYNAMIC, 0, 0);
    uint32_t ops[] = {LLG_CR_ADD, LLG_CR_MUL, LLG_CR_AND, LLG_CR_OR, LLG_CR_XOR, LLG_CR_LOR};
    int64_t want[] = {0, 1, 255, 0, 0, 0};
    uint32_t root = LLG_CR_NONE;
    for (uint32_t i = 0; i < 6; ++i) {
        uint32_t it = nd(&b, LLG_CR_ITER, 32, 1, i, 0, 0);
        uint32_t red = loopn(&b, LLG_CR_REDUCE, 8, 0, e, nd(&b, LLG_CR_AELEM, 8, 0, e, it, LLG_CR_NONE),
                             LLG_CR_NONE, i, ops[i]);
        uint32_t eq = nd(&b, LLG_CR_EQ, 1, 0, red, k(&b, want[i], 8, 0), 0);
        root = root == LLG_CR_NONE ? eq : all2(&b, root, eq);
    }
    uint32_t it = nd(&b, LLG_CR_ITER, 32, 1, 6, 0, 0);
    uint32_t land = loopn(&b, LLG_CR_REDUCE, 1, 0, e, nd(&b, LLG_CR_AELEM, 8, 0, e, it, LLG_CR_NONE),
                          LLG_CR_NONE, 6, LLG_CR_LAND);
    blk(&b, "c0", all2(&b, root, nd(&b, LLG_CR_EQ, 1, 0, vr(&b, z), land, 0)));
    uint32_t v0[] = {z}, a0[] = {e};
    unit(&b, "E", v0, 1, a0, 1, NULL, 0, 0, 1);
    finish(&b, "s10");
    llg_cr_begin(b.s);
    uint32_t inst;
    while ((inst = llg_cr_next_inst(b.s)) != LLG_CR_NONE) {
        set_blocks(&b, inst, 1);
        set_vars(&b, inst, 0, 1);
        llg_cr_set_array(b.s, inst, e, 0, 0);
    }
    llg_cr_status_t st = llg_cr_step(b.s, &rng, NULL);
    printf("S10 status=%s z=%llu\n", names[st], (unsigned long long)llg_cr_value(b.s, z)[0]);
}

/* S11: y == f(x) through the call thunk (unit 0). */
static int thunk(void* context, uint32_t call, const uint64_t* const* args, uint64_t* result) {
    (void)context; (void)call;
    result[0] = (args[0][0] * 3u) & 255u;
    return LLG_CR_INPUT_KNOWN;
}
static void s11(void) {
    static B b;
    uint32_t x = newvar(&b, "x", 4, 0, 0), y = newvar(&b, "y", 8, 0, 0);
    uint32_t vx = vr(&b, x);
    b.args[b.na] = vx;
    b.calls[0].name = "f"; b.calls[0].bit_width = 8; b.calls[0].sign_flag = 0;
    b.calls[0].arg_first = b.na++; b.calls[0].arg_count = 1;
    b.nc = 1;
    uint32_t call = nd(&b, LLG_CR_CALL, 8, 0, 0, 0, 0);
    blk(&b, "c0", nd(&b, LLG_CR_EQ, 1, 0, vr(&b, y), call, 0));
    uint32_t v0[] = {x, y};
    unit(&b, "F", v0, 2, NULL, 0, NULL, 0, 0, 1);
    finish(&b, "s11");
    unsigned bad = 0;
    for (unsigned i = 0; i < 40; ++i) {
        llg_cr_begin(b.s);
        uint32_t inst;
        while ((inst = llg_cr_next_inst(b.s)) != LLG_CR_NONE) {
            set_blocks(&b, inst, 1);
            set_vars(&b, inst, 0, 1);
        }
        llg_cr_status_t st;
        while ((st = llg_cr_step(b.s, &rng, NULL)) == LLG_CR_NEED_CALLS) {
            const uint64_t* args[1] = {llg_cr_call_arg(b.s, 0, 0)};
            uint64_t result[1];
            if (!llg_cr_call_ready(b.s, 0)) { ++bad; break; }
            thunk(NULL, 0, args, result);
            llg_cr_set_call_result(b.s, 0, result, LLG_CR_INPUT_KNOWN);
        }
        if (st != LLG_CR_OK || llg_cr_value(b.s, y)[0] != (llg_cr_value(b.s, x)[0] * 3u & 255u)) ++bad;
    }
    printf("S11 bad=%u\n", bad);
}

/* S12: a child reached through a non-random link is state; reached later
 * through a random link (after it was fed) it is fed again and randomized. */
static void s12(void) {
    static B b;
    uint32_t x = newvar(&b, "x", 8, 0, 0), y = newvar(&b, "y", 8, 0, 0);
    uint32_t s0 = slot(&b, "fixed", 0, 0), s1 = slot(&b, "free", 0, 0);
    uint32_t r0 = nd(&b, LLG_CR_REF, 1, 0, s0, LLG_CR_NONE, LLG_CR_NONE);
    blk(&b, "c0", nd(&b, LLG_CR_EQ, 1, 0, vr(&b, x),
                     nd(&b, LLG_CR_ADD, 8, 0, nd(&b, LLG_CR_HVAR, 8, 0, r0, y, 0), k(&b, 1, 8, 0), 0), 0));
    blk(&b, "c1", nd(&b, LLG_CR_LT, 1, 0, vr(&b, y), k(&b, 3, 8, 0), 0));
    uint32_t v0[] = {x}, sl[] = {s0, s1}, v1[] = {y};
    unit(&b, "P", v0, 1, NULL, 0, sl, 2, 0, 1);
    unit(&b, "C", v1, 1, NULL, 0, NULL, 0, 1, 1);
    finish(&b, "s12");
    for (int promote = 0; promote < 2; ++promote) {
        unsigned feeds = 0;
        llg_cr_begin(b.s);
        llg_cr_inst_root(b.s, &obj_a);
        uint32_t inst;
        while ((inst = llg_cr_next_inst(b.s)) != LLG_CR_NONE) {
            ++feeds;
            set_blocks(&b, inst, 1);
            set_vars(&b, inst, 40, 1);
            if (llg_cr_inst_unit(b.s, inst) != 0) {
                llg_cr_link(b.s, 0, s1, 0, promote ? (void*)&obj_b : NULL, 1, 1);
                continue;
            }
            llg_cr_link(b.s, inst, s0, 0, &obj_b, 1, 0);
            llg_cr_link(b.s, inst, s1, 0, NULL, 1, 1);
        }
        llg_cr_status_t st = llg_cr_step(b.s, &rng, NULL);
        printf("S12 promote=%d status=%s feeds=%u random=%d x=%llu y=%llu\n", promote, names[st],
               feeds, llg_cr_inst_random(b.s, 1), (unsigned long long)llg_cr_value(b.s, x)[0],
               (unsigned long long)llg_cr_ivalue(b.s, 1, y)[0]);
    }
}

/* Budgets: d.size() == 10 against LLG_RAND_ARRAY_ELEMENTS, a 64-element
 * foreach against LLG_RAND_EXPAND_NODES. */
static void budget(void) {
    static B b;
    uint32_t d = arr(&b, "d", 8, 0, LLG_CR_ARRAY_DYNAMIC, 0, 0);
    uint32_t f = arr(&b, "f", 8, 0, LLG_CR_ARRAY_FIXED, 0, 63);
    uint32_t size = nd(&b, LLG_CR_ASIZE, 32, 1, d, 0, LLG_CR_NONE);
    blk(&b, "size", nd(&b, LLG_CR_EQ, 1, 0, size, k(&b, 10, 32, 1), 0));
    uint32_t it = nd(&b, LLG_CR_ITER, 32, 1, 0, 0, 0);
    uint32_t el = nd(&b, LLG_CR_AELEM, 8, 0, f, it, LLG_CR_NONE);
    blk(&b, "elems", loopn(&b, LLG_CR_FOREACH, 1, 0, f,
                           nd(&b, LLG_CR_NE, 1, 0, el, nd(&b, LLG_CR_RESIZE, 8, 0, it, 0, 0), 0),
                           LLG_CR_NONE, 0, 0));
    uint32_t a0[] = {d, f};
    unit(&b, "B", NULL, 0, a0, 2, NULL, 0, 0, 2);
    finish(&b, "budget");
    for (int which = 0; which < 2; ++which) {
        llg_cr_begin(b.s);
        uint32_t inst;
        while ((inst = llg_cr_next_inst(b.s)) != LLG_CR_NONE) {
            llg_cr_iset_block(b.s, inst, 0, which == 0);
            llg_cr_iset_block(b.s, inst, 1, which == 1);
            llg_cr_set_array(b.s, inst, d, 0, which == 0);
            llg_cr_set_array(b.s, inst, f, 64, 1);
        }
        llg_cr_report_t report;
        llg_cr_status_t st = llg_cr_step(b.s, &rng, &report);
        printf("BUDGET %d status=%s budget=%s\n", which, names[st],
               st == LLG_CR_RESOURCE ? report.budget : "-");
    }
}

int main(int argc, char** argv) {
    const char* which = argc > 1 ? argv[1] : "all";
    unsigned n = argc > 2 ? (unsigned)strtoul(argv[2], NULL, 10) : 1000u;
    if (!strcmp(which, "budget")) {
        budget();
    } else {
        s1(n * 10u);
        s2();
        s3(n * 8u);
        s4();
        s5(n * 4u);
        s6();
        s7();
        s8();
        s9();
        s10();
        s11();
        s12();
    }
    llg_cr_session_release_all();
    return 0;
}
"#;

fn field<'a>(output: &'a str, tag: &str) -> &'a str {
    output
        .lines()
        .find(|line| line.starts_with(tag))
        .unwrap_or_else(|| panic!("no `{tag}` line in:\n{output}"))
}

fn counts(line: &str, after: &str) -> Vec<f64> {
    line.split(after)
        .nth(1)
        .unwrap()
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|t| !t.is_empty())
        .map(|t| t.parse::<f64>().unwrap())
        .collect()
}

/// Every count within 5 standard deviations of `n * p` (predeclared).
fn uniform_ok(values: &[f64], n: f64) -> bool {
    let p = 1.0 / values.len() as f64;
    let sigma = (n * p * (1.0 - p)).sqrt();
    values.iter().all(|v| (v - n * p).abs() <= 5.0 * sigma)
}

#[test]
fn template_problems_expand_objects_arrays_and_loops() {
    let Some((_dir, exe)) = build_c_probe("constraint_expand", EXPAND_MAIN) else {
        return;
    };
    let mut command = Command::new(&exe);
    command.args(["all", "1000"]);
    let output = run_command_output(&mut command, &[]);

    let s1 = field(&output, "S1 ");
    assert!(s1.starts_with("S1 bad=0 feeds=20000 h"), "{s1}");
    let h = counts(s1, " h");
    assert!(
        uniform_ok(&h, 10000.0),
        "S1 x is uniform over 0..8 and 255: {s1}"
    );

    assert_eq!(field(&output, "S2 "), "S2 status=ok a=3,2,1,0");

    let s3 = field(&output, "S3 ");
    assert!(s3.starts_with("S3 bad=0 "), "{s3}");
    let sizes = counts(s3, "sizes=");
    assert!(
        uniform_ok(&sizes, 8000.0),
        "sizes are solved first, uniformly over the feasible sizes (S30-D5): {s3}"
    );

    assert_eq!(field(&output, "S4 "), "S4 bad=0");

    let s5 = field(&output, "S5 ");
    assert!(s5.starts_with("S5 bad=0 h"), "{s5}");
    let h = counts(s5, " h");
    assert_eq!(h[4], 0.0, "no index outside the array: {s5}");
    assert!(uniform_ok(&h[..4], 4000.0), "{s5}");

    assert_eq!(field(&output, "S6 plain"), "S6 plain status=error reason=1");
    assert_eq!(
        field(&output, "S6 guarded"),
        "S6 guarded status=ok reason=0"
    );

    assert_eq!(
        field(&output, "S7 same=0"),
        "S7 same=0 status=ok instances=4 0 1 2"
    );
    assert_eq!(
        field(&output, "S7 same=1"),
        "S7 same=1 status=unsat instances=3"
    );

    let s8 = field(&output, "S8");
    let pairs: Vec<(u32, u32)> = s8
        .split_whitespace()
        .skip(1)
        .map(|p| {
            let (a, b) = p.split_once(':').unwrap();
            (a.parse().unwrap(), b.parse().unwrap())
        })
        .collect();
    assert_eq!(pairs.len(), 16, "{s8}");
    for kid in 0..2 {
        for cycle in 0..2 {
            let values: BTreeSet<u32> = pairs[cycle * 8..cycle * 8 + 8]
                .iter()
                .map(|p| if kid == 0 { p.0 } else { p.1 })
                .collect();
            assert_eq!(
                values.len(),
                8,
                "kid {kid} cycle {cycle} is a permutation: {s8}"
            );
        }
    }

    assert_eq!(field(&output, "S9 "), "S9 status=ok instances=1 x=5");
    assert_eq!(field(&output, "S10 "), "S10 status=ok z=1");
    assert_eq!(field(&output, "S11 "), "S11 bad=0");
    assert_eq!(
        field(&output, "S12 promote=0"),
        "S12 promote=0 status=ok feeds=2 random=0 x=41 y=40"
    );
    let s12 = field(&output, "S12 promote=1");
    assert!(
        s12.starts_with("S12 promote=1 status=ok feeds=3 random=1 "),
        "{s12}"
    );
    let x: u32 = s12
        .split("x=")
        .nth(1)
        .unwrap()
        .split(' ')
        .next()
        .unwrap()
        .parse()
        .unwrap();
    let y: u32 = s12.split("y=").nth(1).unwrap().parse().unwrap();
    assert!(y < 3 && x == y + 1, "{s12}");
}

#[test]
fn template_expansion_budgets_are_resource_errors() {
    let Some((_dir, exe)) = build_c_probe("constraint_expand_budget", EXPAND_MAIN) else {
        return;
    };
    let run = |env: &[(&str, &str)]| {
        let mut command = Command::new(&exe);
        command.arg("budget");
        let output = run_command_output(&mut command, env);
        (
            field(&output, "BUDGET 0").to_owned(),
            field(&output, "BUDGET 1").to_owned(),
        )
    };
    // 10 + 64 random elements against 70; 64 elements alone fit.
    let (sized, fixed) = run(&[("LLG_RAND_ARRAY_ELEMENTS", "70")]);
    assert_eq!(
        sized,
        "BUDGET 0 status=resource budget=LLG_RAND_ARRAY_ELEMENTS"
    );
    assert_eq!(fixed, "BUDGET 1 status=ok budget=-");
    let (sized, fixed) = run(&[("LLG_RAND_EXPAND_NODES", "40")]);
    assert_eq!(
        sized,
        "BUDGET 0 status=resource budget=LLG_RAND_EXPAND_NODES"
    );
    assert_eq!(
        fixed,
        "BUDGET 1 status=resource budget=LLG_RAND_EXPAND_NODES"
    );
    let (sized, fixed) = run(&[]);
    assert_eq!(sized, "BUDGET 0 status=ok budget=-");
    assert_eq!(fixed, "BUDGET 1 status=ok budget=-");
}
