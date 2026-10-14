/* Companion of S41-D2_export_copy_back.sv. */
#include "svdpi.h"
#include <stdio.h>

int d2_pair(int a, int *x, int *y);
void d2_both(int *p, int *q);

void d2_run(void)
{
    int v = 0;
    int r = d2_pair(3, &v, &v);
    printf("pair v=%d r=%d\n", v, r);
    v = 1;
    d2_both(&v, &v);
    printf("both v=%d\n", v);
}
