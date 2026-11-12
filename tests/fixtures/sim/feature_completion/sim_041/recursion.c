/* SIM-041 companion for recursion.sv. Portable C11. */
#include "svdpi.h"

int rc_up(int n);

DPI_DLLESPEC int rc_down(int n)
{
    return n <= 0 ? 0 : 1 + rc_up(n - 1);
}
