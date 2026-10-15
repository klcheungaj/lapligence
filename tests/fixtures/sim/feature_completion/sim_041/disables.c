/* SIM-041 companion for disables.sv. Portable C11. */
#include "svdpi.h"
#include <stdio.h>

int dt_kill(int n);
int dt_self(void);
int df_kill(void);

DPI_DLLESPEC int dt_outer(int n, int *o)
{
    int status = dt_kill(n);
    printf("c dt_kill returned %d disabled=%d\n", status, svIsDisabledState());
    *o = 99;
    return status ? 1 : 0;
}

DPI_DLLESPEC int dt_self_outer(int *o)
{
    int status = dt_self();
    printf("c dt_self returned %d disabled=%d\n", status, svIsDisabledState());
    *o = 42;
    return 0;
}

DPI_DLLESPEC int df_outer(int n)
{
    int value = df_kill();
    printf("c df_kill returned disabled=%d\n", svIsDisabledState());
    if (svIsDisabledState()) {
        svAckDisabledState();
        return 0;
    }
    return n + value;
}
