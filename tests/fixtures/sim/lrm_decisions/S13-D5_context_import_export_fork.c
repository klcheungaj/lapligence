/* Companion of S13-D5_context_import_export_fork.sv. */
#include "svdpi.h"

int s13_spawn(int n);

DPI_DLLESPEC int s13_relay(int n)
{
    return s13_spawn(n);
}
