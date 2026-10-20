/* SIM-040 A03 companion: element accesses with a wrong number of indices.
 * The first one ends the run (S40-D6); the later ones are quiet and do
 * nothing. Portable C11. */
#include "svdpi.h"
#include <stdio.h>

DPI_DLLESPEC void ic_access(const svOpenArrayHandle l, const svOpenArrayHandle b)
{
    printf("c: before\n");
    fflush(stdout);
    int p1 = svGetArrElemPtr2(l, 0, 0) == NULL;
    int p2 = svGetArrElemPtr1(b, 0) == NULL;
    int p3 = svGetArrElemPtr3(b, 0, 0, 0) == NULL;
    int g1 = (int)svGetLogicArrElem2(l, 0, 0);
    int g2 = (int)svGetBitArrElem1(b, 1);
    svPutLogicArrElem2(l, sv_0, 0, 0);
    svPutBitArrElem3(b, sv_0, 0, 0, 0);
    printf("c: null=%d%d%d get=%d%d\n", p1, p2, p3, g1, g2);
}
