/* SIM-040 negative companion: calls a time routine llg does not provide. */
#include "svdpi.h"
#include <stddef.h>

DPI_DLLESPEC int neg_scope(void)
{
    svTimeVal now;
    return svGetTime(NULL, &now);
}
