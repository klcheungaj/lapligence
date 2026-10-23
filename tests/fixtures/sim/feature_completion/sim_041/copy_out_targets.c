/* SIM-041 companion for copy_out_targets.sv: each import changes the
 * selector `i` through the exported `set_i` before it writes its formals.
 * Portable C11. */
#include "svdpi.h"

void set_i(int ni);

DPI_DLLESPEC void co_output(int *x, int ni)
{
    set_i(ni);
    *x = 5;
}

DPI_DLLESPEC void co_inout(int *x, int ni)
{
    set_i(ni);
    *x += 100;
}

DPI_DLLESPEC int co_result(svBit *x, int ni)
{
    set_i(ni);
    *x = 1;
    return 7;
}
