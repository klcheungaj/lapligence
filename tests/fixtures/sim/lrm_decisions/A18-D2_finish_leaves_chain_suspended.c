/* Companion of A18-D2_finish_leaves_chain_suspended.sv. */
#include "svdpi.h"

int a2_wait(void);

static int resumed = 0;

DPI_DLLESPEC int a2_task(void)
{
    int status = a2_wait();
    resumed = 1;
    return status;
}

DPI_DLLESPEC int a2_resumed(void)
{
    return resumed;
}
