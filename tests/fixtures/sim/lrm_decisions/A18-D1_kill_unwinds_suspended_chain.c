/* Companion of A18-D1_kill_unwinds_suspended_chain.sv. */
#include "svdpi.h"

int a1_wait(void);

static int seen = -1;

DPI_DLLESPEC int a1_task(int *o)
{
    int status = a1_wait();
    int disabled = svIsDisabledState();
    seen = status * 10 + disabled;
    *o = 99;
    return disabled ? 1 : 0;
}

DPI_DLLESPEC int a1_seen(void)
{
    return seen;
}
