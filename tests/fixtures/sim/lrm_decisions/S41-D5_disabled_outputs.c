/* Companion of S41-D5_disabled_outputs.sv. */
#include "svdpi.h"
#include <stdio.h>

int d5_kill_task(void);
void d5_kill_func(void);

int d5_task(int *o)
{
    int status;
    *o = 50;
    status = d5_kill_task();
    printf("export task returned %d disabled %d\n", status, svIsDisabledState());
    *o = 51;
    return 1;
}

int d5_func(int *o)
{
    *o = 60;
    d5_kill_func();
    printf("function disabled %d\n", svIsDisabledState());
    svAckDisabledState();
    return 61;
}
