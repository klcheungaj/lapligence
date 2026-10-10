/* Companion of aggregates.sv: C declarations equivalent to the SystemVerilog
 * structures and arrays (H.7.2), with packed members in canonical form. */
#include "svdpi.h"
#include <stdio.h>

typedef struct {
    int id;
    svLogicVecVal tag[SV_PACKED_DATA_NELEMS(4)];
    svBitVecVal wide[SV_PACKED_DATA_NELEMS(40)];
    char pair[2];
} rec_t;

typedef struct {
    rec_t r[2];
    short n;
} nest_t;

static void show(const char *name, const rec_t *r) {
    printf("%s id=%d tag=%x/%x wide=%02x%08x pair=%d,%d\n", name, r->id,
           (unsigned)(r->tag[0].aval & 0xfu), (unsigned)(r->tag[0].bval & 0xfu),
           (unsigned)(r->wide[1] & 0xffu), (unsigned)r->wide[0], r->pair[0], r->pair[1]);
}

void ag_rec(const rec_t *i, rec_t *o, nest_t *n) {
    show("i", i);
    show("n.r[c0]", &n->r[0]);
    show("n.r[c1]", &n->r[1]);
    printf("n.n=%d\n", n->n);
    o->id = i->id * 2;
    o->tag[0].aval = 0xau;
    o->tag[0].bval = 0x3u;
    o->wide[0] = 0xdeadbeefu;
    o->wide[1] = 0x42u;
    o->pair[0] = (char)(i->pair[1] - 1);
    o->pair[1] = (char)(i->pair[0] + 1);
    n->r[0].id += 1;
    n->r[0].tag[0].aval = 0xfu;
    n->r[0].tag[0].bval = 0x0u;
    n->r[0].wide[0] = 0x00000100u;
    n->r[1].tag[0].bval = 0xfu;
    n->n = (short)(n->n - 1);
}

void ag_arrays(const char *b, svLogicVecVal *o, const svBit *f, svLogic *l) {
    int k;
    printf("b=%d,%d,%d,%d f=%u%u%u%u l=%u%u%u\n", b[0], b[1], b[2], b[3], (unsigned)f[0],
           (unsigned)f[1], (unsigned)f[2], (unsigned)f[3], (unsigned)l[0], (unsigned)l[1],
           (unsigned)l[2]);
    for (k = 0; k < 6; k++) {
        o[k].aval = (svBitVecVal)k;
        o[k].bval = k == 5 ? 0xfu : 0u;
    }
    l[0] = sv_1;
    l[1] = sv_z;
    l[2] = sv_x;
}

static char result[16] = "ret:";

const char *ag_strings(const char *i, const char **o, const char **io) {
    int k;
    printf("i=%s io=%s\n", i, *io);
    for (k = 0; i[k] != '\0' && k < 8; k++) result[4 + k] = i[k];
    result[4 + k] = '\0';
    *o = "output";
    *io = "changed";
    return result;
}
