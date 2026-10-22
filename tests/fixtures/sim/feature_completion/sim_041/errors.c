/* SIM-041 companion for the neg_*.sv runtime protocol errors. Portable C11.
 * Each function breaks one rule of SV 35.5.3, 35.8 or 35.9; the simulator
 * stops with a fatal error and every value the chain allocated is released. */
#include "svdpi.h"
#include <stdio.h>

int err_inc(int n);
const char *err_text(const char *s);
int err_tick(void);
void err_kill(void);

/* 35.5.3: a noncontext import may not call an export. */
DPI_DLLESPEC int err_noncontext(int n)
{
    return err_inc(n);
}

/* 35.5.3: the context scope `tb.uq` does not export err_inc. A legal call
 * in `tb` first leaves a string result alive across the failing call. */
DPI_DLLESPEC int err_scope(int n)
{
    svScope self = svSetScope(svGetScopeFromName("tb"));
    const char *kept = err_text("held");
    printf("c err_scope kept=%s\n", kept ? kept : "(null)");
    svSetScope(self);
    return err_inc(n);
}

/* 35.8: an imported function calls an exported task; it runs (S41-D8). */
DPI_DLLESPEC int err_task_from_function(void)
{
    return err_tick();
}

/* 35.9 c: a disabled import function must acknowledge the disable. */
DPI_DLLESPEC int err_no_ack(void)
{
    err_kill();
    return 1;
}

/* 35.9 d: no export calls after the import entered the disabled state. */
DPI_DLLESPEC int err_after_disable(void)
{
    err_kill();
    svAckDisabledState();
    return err_inc(1);
}
