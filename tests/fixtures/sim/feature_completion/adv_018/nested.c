/* ADV-018 companion for nested.sv. Portable C11. */
#include "svdpi.h"
#include <stdio.h>
#include <string.h>

int sv_step(int depth, const char* tag, int* got);
int sv_inner_wait(int depth, int* got);
int sv_count(int depth);

static int intact(const char* pattern, size_t size, char fill)
{
    for (size_t i = 0; i < size; ++i)
        if (pattern[i] != fill) return 0;
    return 1;
}

DPI_DLLESPEC int c_outer(int depth, int* result)
{
    svScope scope = svGetScope();
    const char* name = svGetNameFromScope(scope);
    char pattern[512];
    char tag[32];
    char fill = name[strlen(name) - 1];
    int got = -1;
    memset(pattern, fill, sizeof(pattern));
    snprintf(tag, sizeof(tag), "from_%s", name);
    printf("c_outer %s enter depth=%d\n", name, depth);
    int status = sv_step(depth, tag, &got);
    printf("c_outer %s back status=%d got=%d same_scope=%d intact=%d\n", name, status, got,
           svGetScope() == scope, intact(pattern, sizeof(pattern), fill));
    *result = got * 2;
    return 0;
}

DPI_DLLESPEC int c_inner(int depth, int* result)
{
    svScope own = svGetScope();
    const char* name = svGetNameFromScope(own);
    const char* other_name = strcmp(name, "tb.u1") == 0 ? "tb.u2" : "tb.u1";
    svScope other = svGetScopeFromName(other_name);
    volatile int local = depth * 7;
    int got = -1;
    svScope previous = svSetScope(other);
    int count = sv_count(depth);
    printf("c_inner %s moves to %s (previous is own: %d) count=%d\n", name, other_name,
           previous == own, count);
    int status = sv_inner_wait(depth, &got);
    printf("c_inner %s back status=%d got=%d scope_is_other=%d local=%d\n", name, status, got,
           svGetScope() == other, local);
    svSetScope(previous);
    *result = got;
    return 0;
}
