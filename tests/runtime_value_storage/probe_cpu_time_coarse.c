// The process CPU-time service quantized to a coarse tick, so every host
// exercises the scaling probe's handling of a clock like Windows
// GetProcessTimes (15.625 ms; fork_group_scale_coarse_clock). The tick is
// twice that: one reverse removal of 400000 groups takes about 16 ms on
// x86_64 Linux, so a single run is a fraction of a tick, as it was on the
// Windows arm64 runner.
#include "llg_platform_native.h"

#define PROBE_COARSE_TICK_SECONDS 0.03125

double probe_cpu_seconds(void);
double probe_cpu_seconds(void) {
    double seconds = llg_process_cpu_seconds();
    if (seconds < 0) return seconds;
    return (double)(long long)(seconds / PROBE_COARSE_TICK_SECONDS) * PROBE_COARSE_TICK_SECONDS;
}
