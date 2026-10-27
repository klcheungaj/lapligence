/* Companion of A18-D3_waiting_task_from_import_function.sv. */
#include "svdpi.h"

int a3_wait(int delay);

DPI_DLLESPEC int a3_function(int n)
{
    return a3_wait(n) + n;
}
