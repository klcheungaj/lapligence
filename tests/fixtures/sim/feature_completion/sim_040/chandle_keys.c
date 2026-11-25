/* SIM-040 companion for chandle_keys.sv. Portable C11: the pointers rise with
 * the argument. */
#include "svdpi.h"

static int cells[4];

DPI_DLLESPEC void *mk(int i) { return &cells[i]; }
