/* Companion of S40-D4_invalid_element_access.sv. */
#include "svdpi.h"
#include <stdio.h>

DPI_DLLESPEC void d4_access(const svOpenArrayHandle l, const svOpenArrayHandle b)
{
    svLogicVecVal lv;
    svBitVecVal bv;
    int low_null = svGetArrElemPtr1(l, 0) == NULL;
    int high_null = svGetArrElemPtr1(l, 3) == NULL;
    int bit_null = svGetArrElemPtr2(b, 2, 3) == NULL;
    printf("pointers %d%d%d\n", low_null, high_null, bit_null);
    svGetLogicArrElem1VecVal(&lv, l, 3);
    printf("logic out of range=%x/%x\n", (unsigned)(lv.aval & 0xfu), (unsigned)(lv.bval & 0xfu));
    bv = 7u;
    svGetBitArrElem2VecVal(&bv, b, 2, 3);
    printf("bit out of range=%x scalar=%d\n", (unsigned)(bv & 1u), (int)svGetBitArrElem2(b, 0, 1));
    svGetBitArrElem1VecVal(&bv, l, 1);
    printf("bit view of 1x0z=%x\n", (unsigned)(bv & 0xfu));
    svGetBitArrElem1VecVal(&bv, l, 2);
    printf("bit view of 0x1x=%x scalar=%d\n", (unsigned)(bv & 0xfu), (int)svGetBitArrElem1(l, 2));
    lv.aval = 0xfu;
    lv.bval = 0u;
    svPutLogicArrElem1VecVal(l, &lv, 0);
    svPutBitArrElem2(b, sv_0, 0, 4);
    svPutBitArrElem2(b, sv_0, 2, 3);
}
