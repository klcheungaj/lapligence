// Include the production writer so the probe can observe the private ring
// indexes and waiting flag; publication still goes through queue_push and the
// real writer thread.
//
// Regression for a lost not_empty wakeup: the writer parks by storing its
// waiting flag and re-reading head, while the producer stores head and then
// reads the flag. Without a store-load fence both reads could see stale values,
// leaving one published event unconsumed. Before a close join or a flush
// acknowledgement wait that hangs the model. The producer publishes one no-op
// event at a time and waits (without blocking) until it is consumed, then
// delays a pseudo-random number of cycles so the next publication lands at a
// varying point of the writer's park sequence.
#include "llg_wave.c"

#include <time.h>

#define CHECK(condition) do { \
    if (!(condition)) { \
        fprintf(stderr, "wakeup check failed at line %d: %s\n", __LINE__, #condition); \
        exit(2); \
    } \
} while (0)

// Total probing time. The defect it guards against was observed roughly once
// per ten thousand publications on x86_64; this budget covers well over a
// hundred thousand publications in both Debug and optimized builds.
#define WAKEUP_PROBE_SECONDS 2.0
// A parked writer with an unconsumed publication for this long has lost its
// wakeup. Far above any scheduling delay, so a loaded host cannot fail it.
#define WAKEUP_STALL_SECONDS 10.0
#define WAKEUP_MAX_PUBLICATIONS 2000000ul

static double seconds_now(void) {
    struct timespec now;
    CHECK(timespec_get(&now, TIME_UTC) == TIME_UTC);
    return (double)now.tv_sec + (double)now.tv_nsec * 1e-9;
}

int main(void) {
    static const unsigned delay_ranges[] = {64u, 512u, 2048u, 8192u};
    const char* path = "wakeup.vcd";
    CHECK(llg_wave_model_init(1) == 0);
    llg_wave_file(path, 0);
    CHECK(g_wave.worker_started);

    unsigned seed = 0x2545f491u;
    unsigned long published = 0, parked = 0;
    const double start = seconds_now();
    while (published < WAKEUP_MAX_PUBLICATIONS && seconds_now() - start < WAKEUP_PROBE_SECONDS) {
        uint64_t head = atomic_u64_load(&g_wave.head);
        double waited = seconds_now();
        while (atomic_u64_load(&g_wave.tail) != head) {
            if (seconds_now() - waited > WAKEUP_STALL_SECONDS) {
                fprintf(stderr,
                        "lost writer wakeup after %lu publications: head=%llu tail=%llu "
                        "writer_waiting=%d\n",
                        published, (unsigned long long)head,
                        (unsigned long long)atomic_u64_load(&g_wave.tail),
                        atomic_int_load(&g_wave.consumer_waiting));
                exit(2);
            }
        }
        if (atomic_int_load(&g_wave.consumer_waiting)) parked++;
        seed = seed * 1103515245u + 12345u;
        unsigned range = delay_ranges[(published >> 10) % 4u];
        for (volatile unsigned delay = (seed >> 8) % range; delay; delay--) {
        }
        enqueue_simple(EV_SNAPSHOT_END, published, 0);
        published++;
    }
    CHECK(llg_wave_close(published) == 0);
    CHECK(remove(path) == 0);
    // The probe only exercises the race while the writer actually parks.
    CHECK(parked > 0);
    printf("waveform writer wakeups: OK (%lu publications, %lu observed parked)\n", published,
           parked);
    return 0;
}
