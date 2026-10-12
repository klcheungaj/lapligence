/* Companion of packed_widths.sv. Portable C11: unused canonical bits are
 * masked before printing because their contents are undetermined (H.7.7). */
#include "svdpi.h"
#include <stdio.h>

static unsigned top(svBitVecVal value, int bits) {
    return (unsigned)(value & (((svBitVecVal)1u << bits) - 1u));
}

void pw_in(const svBitVecVal *w2, const svBitVecVal *w31, const svBitVecVal *w32,
           const svBitVecVal *w33, const svBitVecVal *w64, const svBitVecVal *w65,
           const svLogicVecVal *l96, const svLogicVecVal *l129) {
    printf("w2=%u w31=%08x w32=%08x\n", top(w2[0], 2), top(w31[0], 31), (unsigned)w32[0]);
    printf("w33=%u:%08x w64=%08x:%08x w65=%u:%08x:%08x\n", top(w33[1], 1),
           (unsigned)w33[0], (unsigned)w64[1], (unsigned)w64[0], top(w65[2], 1),
           (unsigned)w65[1], (unsigned)w65[0]);
    printf("l96=%08x/%08x %08x/%08x %08x/%08x\n", (unsigned)l96[2].aval,
           (unsigned)l96[2].bval, (unsigned)l96[1].aval, (unsigned)l96[1].bval,
           (unsigned)l96[0].aval, (unsigned)l96[0].bval);
    printf("l129=%u/%u %08x/%08x\n", top(l129[4].aval, 1), top(l129[4].bval, 1),
           (unsigned)l129[0].aval, (unsigned)l129[0].bval);
}

void pw_out(svBitVecVal *o33, svLogicVecVal *o64, svBitVecVal *o65, svLogicVecVal *o128) {
    int k;
    o33[0] = 0x89abcdefu;
    o33[1] = 0xfffffffeu; /* only bit 0 is part of the value */
    o64[0].aval = 0x3u;
    o64[0].bval = 0x6u;
    o64[1].aval = 0xcafef00du;
    o64[1].bval = 0u;
    o65[0] = 0xffffffffu;
    o65[1] = 0xffffffffu;
    o65[2] = 0xfffffffdu;
    for (k = 0; k < 3; k++) {
        o128[k].aval = 0x11111111u * (svBitVecVal)(k + 1);
        o128[k].bval = 0u;
    }
    o128[3].aval = 0x4abcdef0u;
    o128[3].bval = 0xf0000000u;
}

void pw_inout(svBitVecVal *x33, svLogicVecVal *x97) {
    printf("x33=%u:%08x x97=%u/%u %08x/%08x %08x/%08x %08x/%08x\n", top(x33[1], 1),
           (unsigned)x33[0], top(x97[3].aval, 1), top(x97[3].bval, 1),
           (unsigned)x97[2].aval, (unsigned)x97[2].bval, (unsigned)x97[1].aval,
           (unsigned)x97[1].bval, (unsigned)x97[0].aval, (unsigned)x97[0].bval);
    x33[0] += 1u;
    x97[0].aval += 1u;
    if (x97[0].aval == 0u) x97[1].aval += 1u;
    x97[2].aval = 0x0000aaaau;
    x97[2].bval = 0u;
    x97[3].aval = 1u;
    x97[3].bval = 0u;
}

void pw_kinds(const svLogicVecVal *i, const svLogicVecVal *t, const svLogicVecVal *ps,
              const svLogicVecVal *pu, const svBitVecVal *e, svLogicVecVal *oi,
              svLogicVecVal *ot) {
    printf("integer=%08x/%x time=%08x,%u ps=%02x/%x pu=%02x/%x e=%u\n",
           (unsigned)i->aval, (unsigned)i->bval, (unsigned)t[0].aval, (unsigned)t[1].aval,
           top(ps->aval, 8), top(ps->bval, 8), top(pu->aval, 8), top(pu->bval, 8),
           top(e[0], 3));
    oi->aval = 0x0000ffffu;
    oi->bval = 0x000000f0u;
    ot[0].aval = 7u;
    ot[0].bval = 0u;
    ot[1].aval = 0x80000000u;
    ot[1].bval = 0u;
}
