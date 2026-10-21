/* SIM-039 companion for match_native.sv. Portable C11. */
#include "svdpi.h"
#include <stdio.h>

int mn_now(void);
void mn_note(int t, int x);

/* `x` was copied when the sequence matched; `mn_now` reads the current
 * value of `d` when the call runs. */
DPI_DLLESPEC void mn_note(int t, int x)
{
    printf("%d c note x=%d d=%d\n", t, x, mn_now());
}
