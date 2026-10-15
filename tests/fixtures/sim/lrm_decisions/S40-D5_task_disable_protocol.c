/* Companion of S40-D5_task_disable_protocol.sv: returns its first argument as
 * the disable-protocol result. */
#include "svdpi.h"

DPI_DLLESPEC int d5_task(int status, int *o)
{
    *o = 40 + status;
    return status;
}
