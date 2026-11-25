#include "svdpi.h"
#include <stdio.h>
extern int sv_task(const char* act, int* o);
extern int sv_func(const char* act);
extern int sv_note(int n);
int c_task(const char* act, int* o)
{
    int status = sv_task(act, o);
    printf("c_task status=%d disabled=%d\n", status, svIsDisabledState()); fflush(stdout);
    if (svIsDisabledState()) { svAckDisabledState(); return 1; }
    printf("c_task note -> %d\n", sv_note(100)); fflush(stdout);
    return 0;
}
int c_func(const char* act)
{
    int r = sv_func(act);
    printf("c_func r=%d disabled=%d\n", r, svIsDisabledState()); fflush(stdout);
    if (svIsDisabledState()) { svAckDisabledState(); return 0; }
    printf("c_func note -> %d\n", sv_note(10)); fflush(stdout);
    return r;
}
