/* Companion of S41-D1_context_scope_chain.sv. */
#include "svdpi.h"
#include <stdio.h>

int d1_get(void);
int d1_nested(void);

static const char *name_of(svScope scope)
{
    const char *name = svGetNameFromScope(scope);
    return name ? name : "(null)";
}

DPI_DLLESPEC int d1_where(void)
{
    printf("where %s\n", name_of(svGetScope()));
    return 0;
}

DPI_DLLESPEC int d1_root(void)
{
    svScope self = svGetScope();
    svScope previous;
    printf("root %s\n", name_of(self));
    previous = svSetScope(svGetScopeFromName("tb.u1"));
    printf("set returned %s\n", previous == self ? "the caller scope" : "another scope");
    printf("u1 get %d\n", d1_get());
    svSetScope(svGetScopeFromName("tb.g[0].c"));
    printf("g get %d in %s\n", d1_get(), name_of(svGetScope()));
    svSetScope(self);
    d1_nested();
    printf("after nested %s\n", name_of(svGetScope()));
    printf("unknown %s\n", svGetScopeFromName("tb.nope") ? "found" : "NULL");
    return 7;
}
