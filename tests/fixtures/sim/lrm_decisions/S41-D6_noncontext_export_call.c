/* Companion of S41-D6_noncontext_export_call.sv. */
#include "svdpi.h"

int d6_inc(int n);

DPI_DLLESPEC int d6_plain(int n)
{
    return d6_inc(n);
}
