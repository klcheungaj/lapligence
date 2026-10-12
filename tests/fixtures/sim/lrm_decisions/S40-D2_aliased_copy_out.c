/* Companion of S40-D2_aliased_copy_out.sv. */
#include "svdpi.h"
#include <string.h>

void d2_three(int i, int *o, int *io)
{
    *o = i + 1;
    *io = *io + 2;
}

void d2_reverse(int *io, int *o)
{
    *o = 1;
    *io = *io + 2;
}

int d2_result(int *o)
{
    *o = 11;
    return 22;
}

static char buffer[16];

const char *d2_string(const char *s, const char **o)
{
    size_t len = strlen(s);
    *o = "output";
    if (len > sizeof buffer - 5u) {
        len = sizeof buffer - 5u;
    }
    memcpy(buffer, s, len);
    memcpy(buffer + len, "-ret", 5u);
    return buffer;
}
