/* FIN-002 DPI-C companion of tb_dpi_disable.sv. The imports are declared in
 * a package, the exports in module tb, so every export call selects that
 * scope first. */
#include "svdpi.h"

extern int sv_cancel(int id);
extern int sv_kill(int id);
extern int sv_delay(int ticks);

static int acknowledged;

DPI_DLLESPEC int c_request(int action, int id)
{
    svSetScope(svGetScopeFromName("tb"));
    return action == 0 ? sv_cancel(id) : sv_kill(id);
}

DPI_DLLESPEC int c_sleep(int ticks, int* slept)
{
    svSetScope(svGetScopeFromName("tb"));
    if (sv_delay(ticks)) {
        /* 35.9: the exported task returned because of a disable. */
        acknowledged++;
        svAckDisabledState();
        return 1;
    }
    *slept = ticks;
    return 0;
}

DPI_DLLESPEC int c_acknowledged(void)
{
    return acknowledged;
}
