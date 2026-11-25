#include "svdpi.h"
#include <stdio.h>
extern int sv_task(const char* act, int depth, int* o);
extern int sv_func(const char* act, int depth);
static void say(const char* s, int a, int b) { printf("%s %d disabled=%d\n", s, a, b); fflush(stdout); }
DPI_DLLESPEC int c_task(const char* act, int depth, int* o)
{
    int status;
    *o = 77;
    status = sv_task(act, depth, o);
    say("c_task: sv_task returned", status, svIsDisabledState());
    if (svIsDisabledState()) { svAckDisabledState(); return 1; }
    return 0;
}
DPI_DLLESPEC int c_func(const char* act, int depth)
{
    int r = sv_func(act, depth);
    say("c_func: sv_func returned", r, svIsDisabledState());
    if (svIsDisabledState()) { svAckDisabledState(); return 0; }
    return r;
}
