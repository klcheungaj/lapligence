/* Companion of S40-D4_invalid_element_access.sv. */
#include "svdpi.h"
#include <stdio.h>

void d4_access(const svOpenArrayHandle l, const svOpenArrayHandle b)
{
    svLogicVecVal lv;
    svBitVecVal bv;
    printf("pointers %d%d%d%d\n", svGetArrElemPtr1(l, 0) == NULL, svGetArrElemPtr1(l, 3) == NULL,
           svGetArrElemPtr2(l, 1, 1) == NULL, svGetArrElemPtr1(b, 0) == NULL);
    svGetLogicArrElem1VecVal(&lv, l, 3);
    printf("logic out of range=%x/%x\n", (unsigned)(lv.aval & 0xfu), (unsigned)(lv.bval & 0xfu));
    bv = 7u;
    svGetBitArrElem2VecVal(&bv, b, 2, 3);
    printf("bit out of range=%x scalar=%d\n", (unsigned)(bv & 1u), (int)svGetBitArrElem2(b, 0, 1));
    svGetBitArrElem1VecVal(&bv, l, 1);
    printf("bit view of 1x0z=%x\n", (unsigned)(bv & 0xfu));
    lv.aval = 0xfu;
    lv.bval = 0u;
    svPutLogicArrElem1VecVal(l, &lv, 0);
    svPutLogicArrElem2VecVal(l, &lv, 1, 2);
    svPutBitArrElem2(b, sv_0, 0, 4);
    svPutBitArrElem1(b, sv_0, 0);
}
