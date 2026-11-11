/* SIM-040 companion for native_context_task.sv. Portable C11. */
#include "svdpi.h"
#include <stdio.h>

extern int ct_tick(int delay);

typedef struct {
    double r;
    const char *s;
} rec_t;

DPI_DLLESPEC int ct_wait(const svOpenArrayHandle a, rec_t *r, const char **names,
                         const svOpenArrayHandle w)
{
    const double *weights = (const double *)svGetArrayPtr(w);
    printf("c before size=%d r=%g %s names=%s %s w=%g\n", svSize(a, 1), r->r, r->s, names[0],
           names[1], weights[2]);
    *(int *)svGetArrElemPtr1(a, 0) = 10;
    if (ct_tick(5)) {
        return 1;
    }
    printf("c after size=%d r=%g %s names=%s %s w=%g\n", svSize(a, 1), r->r, r->s, names[0],
           names[1], weights[2]);
    *(int *)svGetArrElemPtr1(a, 1) = 20;
    r->r += 1.0;
    names[0] = names[1];
    return 0;
}
