/* Companion of CS-D7_disable_fork_in_export.sv. */
#include "svdpi.h"
#include <stdio.h>

int cs7_export(int mode);

DPI_DLLESPEC int cs7_call(int mode)
{
    int disabled = cs7_export(mode);

    printf("c: export returned %d\n", disabled);
    fflush(stdout);
    return 0;
}
