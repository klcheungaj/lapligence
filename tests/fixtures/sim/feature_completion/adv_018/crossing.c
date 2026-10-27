/* ADV-018 companion for crossing.sv. Portable C11. */
#include "svdpi.h"
#include <stdio.h>
#include <string.h>

int sv_wait(int id, int delay);

DPI_DLLESPEC int c_chain(int id, int delay)
{
    char pattern[256];
    volatile int local = id * 1000 + delay;
    memset(pattern, 'a' + id, sizeof(pattern));
    printf("c_chain %d enter\n", id);
    int status = sv_wait(id, delay);
    int intact = local == id * 1000 + delay;
    for (size_t i = 0; i < sizeof(pattern); ++i)
        if (pattern[i] != (char)('a' + id)) intact = 0;
    printf("c_chain %d back status=%d intact=%d\n", id, status, intact);
    return 0;
}
