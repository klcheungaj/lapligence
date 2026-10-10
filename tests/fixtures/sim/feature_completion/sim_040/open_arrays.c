/* Companion of open_arrays.sv: walks open arrays through the svdpi.h query
 * and access routines using the actuals' original ranges (H.12.1). */
#include "svdpi.h"
#include <stdio.h>

static int step(const svOpenArrayHandle h, int d) {
    return svLeft(h, d) > svRight(h, d) ? -1 : 1;
}

int oa_walk(const svOpenArrayHandle h) {
    int d, i, j, total = 0;
    printf("dims=%d bytes=%d\n", svDimensions(h), svSizeOfArray(h));
    for (d = 0; d <= 2; d++)
        printf("d%d [%d:%d] low=%d high=%d inc=%d size=%d\n", d, svLeft(h, d), svRight(h, d),
               svLow(h, d), svHigh(h, d), svIncrement(h, d), svSize(h, d));
    for (i = svLeft(h, 1);; i += step(h, 1)) {
        for (j = svLeft(h, 2);; j += step(h, 2)) {
            int value = *(const int *)svGetArrElemPtr2(h, i, j);
            printf("[%d][%d]=%d ", i, j, value);
            total += value;
            if (j == svRight(h, 2)) break;
        }
        printf("\n");
        if (i == svRight(h, 1)) break;
    }
    printf("c-order=%d,%d\n", ((const int *)svGetArrayPtr(h))[0],
           ((const int *)svGetArrayPtr(h))[5]);
    return total;
}

void oa_fill(const svOpenArrayHandle h) {
    int i, j;
    for (i = svLow(h, 1); i <= svHigh(h, 1); i++)
        for (j = svLow(h, 2); j <= svHigh(h, 2); j++)
            *(int *)svGetArrElemPtr2(h, i, j) = i * 100 + j;
}

void oa_cube(const svOpenArrayHandle h) {
    int i, j;
    printf("cube dims=%d d3=[%d:%d]\n", svDimensions(h), svLeft(h, 3), svRight(h, 3));
    for (i = svLow(h, 1); i <= svHigh(h, 1); i++)
        for (j = svLow(h, 2); j <= svHigh(h, 2); j++) {
            short *a = (short *)svGetArrElemPtr3(h, i, j, -1);
            short *b = (short *)svGetArrElemPtr(h, i, j, -1);
            *a = (short)(*a + (a == b ? 1 : 1000));
        }
}

int oa_packed(const svOpenArrayHandle h) {
    svBitVecVal bits = *(const svBitVecVal *)svGetArrayPtr(h);
    return svSize(h, 0) * 10000 + (int)(bits & 0x1fffu) + 100000 * svDimensions(h);
}

void oa_vectors(const svOpenArrayHandle v, const svOpenArrayHandle p) {
    int i;
    printf("v dims=%d d0=[%d:%d] d1=[%d:%d] p d0=[%d:%d] d1=[%d:%d]\n", svDimensions(v),
           svLeft(v, 0), svRight(v, 0), svLeft(v, 1), svRight(v, 1), svLeft(p, 0),
           svRight(p, 0), svLeft(p, 1), svRight(p, 1));
    for (i = svLow(v, 1); i <= svHigh(v, 1); i++) {
        svLogicVecVal w;
        svBitVecVal mask;
        svGetLogicArrElem1VecVal(&w, v, i);
        svGetBitArrElem1VecVal(&mask, p, i % 2);
        w.aval = (w.aval ^ mask) & 0xfffu;
        if (i == 3) w.bval = 0x00fu;
        svPutLogicArrElem1VecVal(v, &w, i);
    }
}

void oa_sized(const svOpenArrayHandle s) {
    int i;
    printf("sized d1=[%d:%d]", svLeft(s, 1), svRight(s, 1));
    for (i = 3; i >= 1; i--) {
        svLogicVecVal w;
        svGetLogicArrElem1VecVal(&w, s, i);
        printf(" s[%d]=%02x", i, (unsigned)w.aval);
    }
    printf("\n");
}

typedef struct { char b; int i; } pair_t;

int oa_records(const svOpenArrayHandle s, const svOpenArrayHandle d) {
    int i, total = 0;
    printf("records [%d:%d] dims=%d bytes=%d\n", svLeft(s, 1), svRight(s, 1),
           svDimensions(s), svSizeOfArray(s));
    for (i = svLow(s, 1); i <= svHigh(s, 1); i++) {
        const pair_t *in = (const pair_t *)svGetArrElemPtr1(s, i);
        pair_t *out = (pair_t *)svGetArrElemPtr1(d, i);
        total += in->i + in->b;
        out->b = (char)(in->b * 2);
        out->i = -in->i;
    }
    return total;
}
