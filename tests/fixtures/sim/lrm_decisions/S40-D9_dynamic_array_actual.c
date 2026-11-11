/* Companion for S40-D9_dynamic_array_actual.sv. Portable C11. */
#include "svdpi.h"
#include <stdio.h>

extern void d9_grow(int size);

static void shape(const char *tag, const svOpenArrayHandle h)
{
    printf("%s: [%d:%d] low=%d high=%d inc=%d size=%d\n", tag, svLeft(h, 1), svRight(h, 1),
           svLow(h, 1), svHigh(h, 1), svIncrement(h, 1), svSize(h, 1));
}

DPI_DLLESPEC int d9_sum(const svOpenArrayHandle a)
{
    int sum = 0;
    int i;
    shape("sum", a);
    for (i = svLow(a, 1); i <= svHigh(a, 1); i++) {
        sum += *(const int *)svGetArrElemPtr1(a, i);
    }
    printf("sum: above=%d\n", svGetArrElemPtr1(a, svHigh(a, 1) + 1) != NULL);
    return sum;
}

DPI_DLLESPEC void d9_resize(const svOpenArrayHandle a)
{
    shape("before", a);
    *(int *)svGetArrElemPtr1(a, 0) = 555;
    d9_grow(5);
    shape("after", a);
    printf("after: first=%d fifth=%d\n", *(const int *)svGetArrElemPtr1(a, 0),
           svGetArrElemPtr1(a, 4) != NULL);
    *(int *)svGetArrElemPtr1(a, 1) = 666;
}

DPI_DLLESPEC void d9_scale(const svOpenArrayHandle a)
{
    double *values = (double *)svGetArrayPtr(a);
    shape("scale", a);
    values[0] *= 2.0;
    values[1] *= 4.0;
}
