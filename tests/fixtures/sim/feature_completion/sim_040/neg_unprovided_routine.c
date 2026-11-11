/* SIM-040 negative companion: calls a deprecated SV3.1a routine (H.13) llg
 * does not provide. */
#include "svdpi.h"
#include <stddef.h>

DPI_DLLESPEC int neg_scope(void)
{
    return (int)svGetSelectBit(NULL, 0);
}
