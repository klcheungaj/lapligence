// The process CPU-time service in its own translation unit: probes that
// include the flat scheduler (llg_rt.c) must not see <windows.h> macros.
#include "llg_platform_native.h"

double probe_cpu_seconds(void);
double probe_cpu_seconds(void) { return llg_process_cpu_seconds(); }
