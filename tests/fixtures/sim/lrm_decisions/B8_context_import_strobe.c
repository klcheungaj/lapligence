/* Companion of B8_context_import_strobe.sv: a context import that reads only
 * its argument. */
#include "svdpi.h"

DPI_DLLESPEC int b8_twice(int value) {
    return value * 2;
}
