/* SIM-040 companion for native_disabled.sv. Portable C11. */
#include "svdpi.h"
#include <stdio.h>

int nd_kill_task(void);
void nd_kill_func(void);

typedef struct {
    double r;
    const char *s;
} rec_t;

static void fill(rec_t *o, const svOpenArrayHandle a, const char **s, double *q)
{
    o->r = 99.0;
    o->s = "changed";
    *(int *)svGetArrElemPtr1(a, 0) = 99;
    s[0] = "changed";
    q[1] = 99.0;
}

DPI_DLLESPEC int nd_task(rec_t *o, const svOpenArrayHandle a, const char **s, double *q)
{
    int status;
    fill(o, a, s, q);
    status = nd_kill_task();
    printf("c task: export returned %d disabled %d\n", status, svIsDisabledState());
    return 1;
}

DPI_DLLESPEC void nd_func(rec_t *o, const svOpenArrayHandle a, const char **s, double *q)
{
    fill(o, a, s, q);
    nd_kill_func();
    printf("c function: disabled %d\n", svIsDisabledState());
    svAckDisabledState();
}
