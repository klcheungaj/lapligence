/* ADV-018 companion for disables.sv. Portable C11. */
#include "svdpi.h"
#include <stdio.h>

int sv_hold(int id);

DPI_DLLESPEC int c_hold_task(int id, int* o)
{
    printf("c %d enter\n", id);
    int status = sv_hold(id);
    int disabled = svIsDisabledState();
    printf("c %d sv_hold returned %d disabled=%d\n", id, status, disabled);
    *o = 40 + id;
    return disabled ? 1 : 0;
}
