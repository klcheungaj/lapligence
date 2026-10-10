/* SIM-040 A02 companion: prints each canonical chunk it receives and writes
 * back values with X and Z in every chunk. Portable C11. */
#include "svdpi.h"
#include <stdio.h>
#include <string.h>

/* Bits above the formal's width in the top chunk are unspecified (H.7.7),
 * so `top` masks them out. */
static void show(const char *name, const svLogicVecVal *v, int chunks, svBitVecVal top)
{
    int k;
    printf("%s", name);
    for (k = chunks - 1; k >= 0; k--) {
        svBitVecVal mask = k == chunks - 1 ? top : 0xffffffffu;
        printf(" %08x/%08x", (unsigned)(v[k].aval & mask), (unsigned)(v[k].bval & mask));
    }
    printf("\n");
}

void rt_logic(const svLogicVecVal *i, svLogicVecVal *o, svLogicVecVal *io)
{
    show("c i", i, 3, 0x3fu);
    show("c o", o, 3, 0x3fu);
    show("c io", io, 3, 0x3fu);
    o[0] = io[0];
    o[1] = i[1];
    o[2].aval = 0x15u;
    o[2].bval = 0x3cu;
    io[0].aval = i[0].aval;
    io[0].bval = i[0].bval;
    io[1].aval = 0xffff0000u;
    io[1].bval = 0xff00ff00u;
    io[2].aval = 0xffffffffu;
    io[2].bval = 0xffffffffu;
}

void rt_init(svLogicVecVal *lo, svBitVecVal *bo, svLogicVecVal *la, svBitVecVal *ba)
{
    show("c init lo", lo, 2, 0x1ffu);
    printf("c init bo %08x %08x\n", (unsigned)(bo[1] & 0x1ffu), (unsigned)bo[0]);
    printf("c init la %x/%x %x/%x ba %x %x\n", (unsigned)(la[0].aval & 0xfu),
           (unsigned)(la[0].bval & 0xfu), (unsigned)(la[1].aval & 0xfu),
           (unsigned)(la[1].bval & 0xfu), (unsigned)(ba[0] & 0xfu), (unsigned)(ba[1] & 0xfu));
    lo[0].aval = 0x0000000fu;
    lo[0].bval = 0x00000005u;
    la[1].aval = 0x3u;
    la[1].bval = 0x6u;
    ba[0] = 0xau;
}

void rt_alias(const svBitVecVal *i, svBitVecVal *o, svBitVecVal *io)
{
    printf("c alias i=%02x%08x io=%02x%08x\n", (unsigned)(i[1] & 0xffu), (unsigned)i[0],
           (unsigned)(io[1] & 0xffu), (unsigned)io[0]);
    o[0] = i[0] + 1u;
    o[1] = i[1];
    io[0] = io[0] + 2u;
    io[1] = 0xffffff00u | io[1];
}

int rt_result(int *o)
{
    printf("c result o=%d\n", *o);
    *o = 11;
    return 22;
}

static char buffer[32];

const char *rt_string(const char *s, const char **o)
{
    size_t len = strlen(s);
    printf("c string s=%s\n", s);
    *o = "output";
    if (len > sizeof buffer - 5u) {
        len = sizeof buffer - 5u;
    }
    memcpy(buffer, s, len);
    memcpy(buffer + len, "-ret", 5u);
    return buffer;
}

int rt_task(int i, svLogicVecVal *o)
{
    o[0].aval = (svBitVecVal)i;
    o[0].bval = 0u;
    o[1].aval = 0u;
    o[1].bval = 0u;
    o[2].aval = 0x3fu;
    o[2].bval = 0x20u;
    return 0;
}

void rt_wide(svLogicVecVal *v, svBitVecVal *b)
{
    printf("c wide v0=%x/%x v78=%x/%x v156=%x/%x b0=%x\n", (unsigned)v[0].aval,
           (unsigned)v[0].bval, (unsigned)v[78].aval, (unsigned)v[78].bval,
           (unsigned)(v[156].aval & 0xffu), (unsigned)(v[156].bval & 0xffu), (unsigned)b[0]);
    v[0].aval = 0xfu;
    v[0].bval = 0x5u;
    v[78].aval = 0x20u;
    v[78].bval = 0x10u;
    b[0] = 0x12345678u;
    b[128] = 0x1fu;
}

void rt_scalar(svLogic a, svLogic *b, svLogic *c)
{
    printf("c scalar a=%d b=%d c=%d\n", (int)a, (int)*b, (int)*c);
    *b = sv_z;
    *c = a;
}
