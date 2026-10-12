/* SIM-040 negative companion: calls a scope routine llg does not provide. */
#include "svdpi.h"
#include <stddef.h>

int neg_scope(void)
{
    return svGetScope() != NULL;
}
