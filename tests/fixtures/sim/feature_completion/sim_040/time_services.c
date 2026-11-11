/* SIM-040 companion for time_services.sv. Portable C11. */
#include "svdpi.h"
#include <stdio.h>

static void report(const char *tag, const svScope scope)
{
    svTimeVal now;
    int32_t unit = 77;
    int32_t precision = 77;
    const char *name = scope ? svGetNameFromScope(scope) : "<simulation>";
    int status;
    /* The routine leaves `type` alone and fills both forms of the time. */
    now.type = 55;
    now.high = 0xaaaaaaaau;
    now.low = 0xbbbbbbbbu;
    now.real = -1.0;
    status = svGetTime(scope, &now);
    printf("c %s: %s time=%d type=%d high=%u low=%u real=%.6f\n", tag, name, status,
           (int)now.type, (unsigned)now.high, (unsigned)now.low, now.real);
    status = svGetTimeUnit(scope, &unit);
    printf("c %s: %s unit=%d %d", tag, name, status, (int)unit);
    status = svGetTimePrecision(scope, &precision);
    printf(" precision=%d %d\n", status, (int)precision);
}

DPI_DLLESPEC void ts_report(const char *tag)
{
    report(tag, svGetScope());
    report(tag, NULL);
}

/* A noncontext import has no scope of its own (svGetScope is NULL), but it
 * may ask about a scope it finds by name. */
DPI_DLLESPEC void ts_plain(const char *tag, const char *scope_name)
{
    report(tag, svGetScope());
    report(tag, svGetScopeFromName(scope_name));
}

DPI_DLLESPEC void ts_invalid(void)
{
    svTimeVal now;
    int32_t value = 77;
    int not_a_scope = 0;
    const svScope bad = (svScope)&not_a_scope;
    int status;
    now.type = 55;
    now.high = 1u;
    now.low = 2u;
    now.real = -1.0;
    status = svGetTime(bad, &now);
    printf("c invalid scope: time=%d type=%d high=%u low=%u real=%.1f\n", status, (int)now.type,
           (unsigned)now.high, (unsigned)now.low, now.real);
    status = svGetTimeUnit(bad, &value);
    printf("c invalid scope: unit=%d %d", status, (int)value);
    status = svGetTimePrecision(bad, &value);
    printf(" precision=%d %d\n", status, (int)value);
    printf("c null result: time=%d unit=%d precision=%d\n", svGetTime(svGetScope(), NULL),
           svGetTimeUnit(svGetScope(), NULL), svGetTimePrecision(NULL, NULL));
}
