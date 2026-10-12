/* Companion of S40-D1_output_initial_values.sv: prints the output formals it
 * receives (masked to their widths), then writes each one. */
#include "svdpi.h"
#include <stdio.h>

typedef struct {
    int i;
    svLogicVecVal l[1];
} rec_t;

void d1_outputs(svLogicVecVal *lv, svBitVecVal *bv, int *i, svLogic *s, rec_t *r,
                svBitVecVal *ba)
{
    printf("llg: lv=%03x%08x/%03x%08x bv=%03x%08x\n", (unsigned)(lv[1].aval & 0x1ffu),
           (unsigned)lv[0].aval, (unsigned)(lv[1].bval & 0x1ffu), (unsigned)lv[0].bval,
           (unsigned)(bv[1] & 0x1ffu), (unsigned)bv[0]);
    printf("llg: i=%d s=%d r=%d,%x/%x ba=%x,%x\n", *i, (int)*s, r->i,
           (unsigned)(r->l[0].aval & 0xfu), (unsigned)(r->l[0].bval & 0xfu),
           (unsigned)(ba[0] & 0xfu), (unsigned)(ba[1] & 0xfu));
    lv[0].aval = 1u;
    lv[0].bval = 0u;
    lv[1].aval = 0u;
    lv[1].bval = 0u;
    bv[0] = 2u;
    bv[1] = 0u;
    *i = 3;
    *s = sv_0;
    r->i = 4;
    r->l[0].aval = 0x5u;
    r->l[0].bval = 0x0u;
    ba[0] = 0x6u;
    ba[1] = 0x7u;
}
