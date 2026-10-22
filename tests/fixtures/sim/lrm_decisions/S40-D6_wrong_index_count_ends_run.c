/* Companion of S40-D6_wrong_index_count_ends_run.sv. */
#include "svdpi.h"
#include <stdio.h>

DPI_DLLESPEC void d6_access(const svOpenArrayHandle b)
{
    printf("c: before access\n");
    fflush(stdout);
    (void)svGetArrElemPtr1(b, 0);
    printf("c: after access\n");
    svPutBitArrElem2(b, sv_1, 0, 3);
}
