/* SIM-039 companion for endpoints_dpi.sv. Portable C11. */
#include "svdpi.h"
#include <stdio.h>

int ep_status(void);
void ep_probe(int t);

/* Two reads in one native frame see the same end-point status: the status
 * belongs to the time step, not to the call. */
void ep_probe(int t)
{
    int first = ep_status();
    int second = ep_status();
    printf("%d c probe %d %d\n", t, first, second);
}
