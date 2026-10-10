/* Companion of S40-D3_open_array_queries.sv. */
#include "svdpi.h"
#include <stdio.h>

static void show(const char *name, const svOpenArrayHandle h)
{
    int d;
    printf("%s: dims=%d", name, svDimensions(h));
    for (d = -1; d <= svDimensions(h); d++) {
        printf(" d%d=[%d:%d],%d,%d,%d,%d", d, svLeft(h, d), svRight(h, d), svLow(h, d),
               svHigh(h, d), svIncrement(h, d), svSize(h, d));
    }
    printf("\n");
}

void d3_query(const svOpenArrayHandle c, const svOpenArrayHandle v,
              const svOpenArrayHandle r, const svOpenArrayHandle p)
{
    show("c", c);
    show("v", v);
    show("r", r);
    show("p", p);
}
