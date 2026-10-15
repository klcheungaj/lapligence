/* Companion of S41-D7_export_outside_scope.sv. */
#include "svdpi.h"

int d7_inc(int n);

DPI_DLLESPEC int d7_call(int n)
{
    return d7_inc(n);
}
