/* Companion of record_columns.sv: C declarations equivalent to the
 * SystemVerilog structures (H.7.2), with packed members in canonical form. */
#include "svdpi.h"
#include <stdio.h>
#include <string.h>

typedef struct {
    int id;
    svLogicVecVal tag[SV_PACKED_DATA_NELEMS(4)];
    char data[16];
    svBitVecVal wide[SV_PACKED_DATA_NELEMS(40)];
} col_t;

typedef struct {
    short n;
    svLogicVecVal m[4][8][SV_PACKED_DATA_NELEMS(4)];
} grid_t;

extern void ex_auto(const col_t *in, col_t *out, grid_t *io);
extern void ex_static(const col_t *in, col_t *out);
extern int ex_wait(col_t *v);

static int sv_byte(char c) {
    return (int)((unsigned char)c ^ 0x80u) - 0x80;
}

static void show_col(const char *name, const col_t *c) {
    printf("%s id=%d tag=%x/%x wide=%02x%08x data=%d,%d,%d,%d\n", name, c->id,
           (unsigned)(c->tag[0].aval & 0xfu), (unsigned)(c->tag[0].bval & 0xfu),
           (unsigned)(c->wide[1] & 0xffu), (unsigned)c->wide[0], sv_byte(c->data[0]),
           sv_byte(c->data[1]), sv_byte(c->data[7]), sv_byte(c->data[15]));
}

static void show_grid(const char *name, const grid_t *g) {
    printf("%s n=%d %x/%x %x/%x %x/%x %x/%x\n", name, g->n,
           (unsigned)g->m[0][0][0].aval & 0xfu, (unsigned)g->m[0][0][0].bval & 0xfu,
           (unsigned)g->m[0][7][0].aval & 0xfu, (unsigned)g->m[0][7][0].bval & 0xfu,
           (unsigned)g->m[3][0][0].aval & 0xfu, (unsigned)g->m[3][0][0].bval & 0xfu,
           (unsigned)g->m[3][7][0].aval & 0xfu, (unsigned)g->m[3][7][0].bval & 0xfu);
}

DPI_DLLESPEC void rc_in(const col_t *c, const grid_t *g) {
    show_col("rc_in c", c);
    show_grid("rc_in g", g);
    fflush(stdout);
}

DPI_DLLESPEC void rc_out(col_t *c) {
    memset(c, 0, sizeof(*c));
    c->id = 77;
    c->tag[0].aval = 0xau;
    c->tag[0].bval = 0x3u;
    c->wide[0] = 0xdeadbeefu;
    c->wide[1] = 0x42u;
    for (int i = 0; i < 16; ++i) c->data[i] = (char)(i * 3 - 20);
}

DPI_DLLESPEC void rc_inout(col_t *c, grid_t *g) {
    c->id += 1000;
    c->data[7] = (char)-100;
    g->n = (short)(g->n * 2);
    g->m[0][7][0].aval = 0x5u;
    g->m[0][7][0].bval = 0x2u;
}

DPI_DLLESPEC int rc_drive(void) {
    col_t in, out;
    grid_t io;
    memset(&in, 0, sizeof(in));
    memset(&io, 0, sizeof(io));
    in.id = 9;
    in.tag[0].aval = 0x6u;
    in.wide[1] = 0x1u;
    for (int i = 0; i < 16; ++i) in.data[i] = (char)(i + 1);
    io.n = 11;
    io.m[3][0][0].aval = 0x9u;
    fflush(stdout);
    ex_auto(&in, &out, &io);
    fflush(stdout);
    show_col("rc_drive out", &out);
    show_grid("rc_drive io", &io);
    ex_static(&in, &out);
    show_col("rc_drive static", &out);
    if (ex_wait(&in) != 0) return 1;
    show_col("rc_drive wait", &in);
    fflush(stdout);
    return 0;
}
