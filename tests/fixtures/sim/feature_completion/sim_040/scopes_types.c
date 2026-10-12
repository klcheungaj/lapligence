/* SIM-040 companion for scopes_types.sv. Portable C11. */
#include "svdpi.h"
#include <stdio.h>

int st_sum(const svOpenArrayHandle h)
{
    int sum = 0;
    int i;
    for (i = svLow(h, 1); i <= svHigh(h, 1); i++) {
        sum += *(const int *)svGetArrElemPtr1(h, i);
    }
    printf("c sum range [%d:%d]\n", svLeft(h, 1), svRight(h, 1));
    return sum;
}

int st_pure(const svOpenArrayHandle h)
{
    return st_sum(h);
}

int st_twice(int a)
{
    return 2 * a;
}

int st_enum(int c, const svLogicVecVal *l, int *o)
{
    printf("c l=%x/%x\n", (unsigned)(l->aval & 3u), (unsigned)(l->bval & 3u));
    *o = c + 1;
    return c + 10;
}

void st_packed(svLogicVecVal *p, const svLogicVecVal *i)
{
    printf("c p=%x/%x i=%x/%x\n", (unsigned)(p->aval & 0xffu), (unsigned)(p->bval & 0xffu),
           (unsigned)i->aval, (unsigned)i->bval);
    p->aval = 0x5cu;
    p->bval = 0x03u;
}

void st_views(const svBitVecVal *a, const svLogicVecVal *b)
{
    printf("c bit=%x logic=%x/%x\n", (unsigned)(*a & 0xffu), (unsigned)(b->aval & 0xffu),
           (unsigned)(b->bval & 0xffu));
}
