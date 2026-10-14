/* Companion of S41-D3_noncontext_has_no_scope.sv. */
#include "svdpi.h"
#include <stdio.h>

int d3_back(void);

int d3_plain(void)
{
    const char *file = NULL;
    int line = 0;
    int known = svGetCallerInfo(&file, &line);
    printf("plain scope %s caller %d\n", svGetScope() ? "set" : "NULL", known);
    return 1;
}

int d3_ctx(void)
{
    const char *file = NULL;
    int line = 0;
    int r;
    if (svGetCallerInfo(&file, &line)) {
        printf("ctx caller line %d\n", line);
    } else {
        printf("ctx caller unknown\n");
    }
    printf("ctx scope %s\n", svGetNameFromScope(svGetScope()));
    r = d3_back();
    printf("ctx scope after %s\n", svGetNameFromScope(svGetScope()));
    return 10 + r;
}
