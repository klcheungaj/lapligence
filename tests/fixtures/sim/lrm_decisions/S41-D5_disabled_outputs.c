/* Companion of S41-D5_disabled_outputs.sv. */
#include "svdpi.h"
#include <stdio.h>

int d5_kill_task(void);
void d5_kill_func(void);

DPI_DLLESPEC int d5_task(svLogicVecVal *o)
{
    int status;
    o->aval = 50;
    o->bval = 0;
    status = d5_kill_task();
    printf("export task returned %d disabled %d\n", status, svIsDisabledState());
    o->aval = 51;
    return 1;
}

DPI_DLLESPEC int d5_func(svLogicVecVal *o)
{
    o->aval = 60;
    o->bval = 0;
    d5_kill_func();
    printf("function disabled %d\n", svIsDisabledState());
    svAckDisabledState();
    return 61;
}
