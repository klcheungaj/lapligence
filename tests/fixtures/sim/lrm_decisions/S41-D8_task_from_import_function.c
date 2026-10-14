/* Companion of S41-D8_task_from_import_function.sv. */
#include "svdpi.h"

int d8_task(void);

int d8_function(void)
{
    return d8_task();
}
