/* Companion of S41-D9_run_ends_in_export.sv. */
#include "svdpi.h"
#include <stdio.h>

int d9_middle(void);
int d9_inner(void);

static void say(const char *text)
{
    printf("%s\n", text);
    fflush(stdout);
}

DPI_DLLESPEC void d9_inner_call(void)
{
    say("c: inner call");
    (void)d9_inner();
    say("c: after inner");
}

DPI_DLLESPEC int d9_outer(void)
{
    int r;

    say("c: outer call");
    r = d9_middle();
    say("c: after middle");
    return r;
}
