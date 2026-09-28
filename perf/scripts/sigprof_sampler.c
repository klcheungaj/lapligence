#define _GNU_SOURCE
#define _POSIX_C_SOURCE 200809L

#include <execinfo.h>
#include <fcntl.h>
#include <limits.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/time.h>
#include <unistd.h>

#define LLG_PROF_MAX_DEPTH 64

typedef struct {
    char magic[8];
    uint32_t version;
    uint32_t address_size;
    uint32_t max_depth;
    uint32_t reserved;
} llg_prof_header_t;

typedef struct {
    uint16_t depth;
    uint16_t reserved;
    uint32_t sequence;
    uintptr_t pcs[LLG_PROF_MAX_DEPTH];
} llg_prof_record_t;

static int sample_fd = -1;
static volatile sig_atomic_t in_handler;
static volatile sig_atomic_t sequence;
static volatile sig_atomic_t dropped;
static unsigned char signal_stack[64 * 1024];

static void sample_signal(int signal_number) {
    llg_prof_record_t record;
    int depth;
    ssize_t written;
    (void)signal_number;

    if (sample_fd < 0 || in_handler) {
        dropped++;
        return;
    }
    in_handler = 1;
    memset(&record, 0, sizeof(record));
    depth = backtrace((void **)record.pcs, LLG_PROF_MAX_DEPTH);
    if (depth < 0)
        depth = 0;
    record.depth = (uint16_t)depth;
    record.sequence = (uint32_t)sequence++;
    written = write(sample_fd, &record, sizeof(record));
    if (written != (ssize_t)sizeof(record))
        dropped++;
    in_handler = 0;
}

static void copy_proc_maps(const char *raw_path) {
    char maps_path[PATH_MAX];
    char buffer[16384];
    int input_fd;
    int output_fd;
    ssize_t size;

    if (snprintf(maps_path, sizeof(maps_path), "%s.maps", raw_path) >=
        (int)sizeof(maps_path))
        return;
    input_fd = open("/proc/self/maps", O_RDONLY | O_CLOEXEC);
    if (input_fd < 0)
        return;
    output_fd = open(maps_path, O_WRONLY | O_CREAT | O_TRUNC | O_CLOEXEC, 0644);
    if (output_fd < 0) {
        close(input_fd);
        return;
    }
    while ((size = read(input_fd, buffer, sizeof(buffer))) > 0) {
        char *cursor = buffer;
        while (size > 0) {
            ssize_t result = write(output_fd, cursor, (size_t)size);
            if (result <= 0) {
                size = -1;
                break;
            }
            cursor += result;
            size -= result;
        }
    }
    close(output_fd);
    close(input_fd);
}

__attribute__((constructor)) static void start_sampling(void) {
    const char *raw_path = getenv("LLG_PROF_OUT");
    const char *rate_text;
    char *end = NULL;
    long rate = 99;
    struct sigaction action;
    struct itimerval timer;
    stack_t alternate_stack;
    llg_prof_header_t header = {{'L', 'L', 'G', 'P', 'R', 'O', 'F', '\0'},
                                1,
                                sizeof(uintptr_t),
                                LLG_PROF_MAX_DEPTH,
                                0};
    void *warmup[1];

    if (raw_path == NULL || raw_path[0] == '\0')
        return;
    rate_text = getenv("LLG_PROF_HZ");
    if (rate_text != NULL && rate_text[0] != '\0') {
        rate = strtol(rate_text, &end, 10);
        if (end == rate_text || *end != '\0' || rate < 1 || rate > 10000)
            rate = 99;
    }

    sample_fd = open(raw_path, O_WRONLY | O_CREAT | O_TRUNC | O_CLOEXEC, 0644);
    if (sample_fd < 0)
        return;
    if (write(sample_fd, &header, sizeof(header)) != (ssize_t)sizeof(header)) {
        close(sample_fd);
        sample_fd = -1;
        return;
    }
    copy_proc_maps(raw_path);

    // Force the unwinder's lazy loader work to happen before the first signal.
    (void)backtrace(warmup, 1);
    memset(&alternate_stack, 0, sizeof(alternate_stack));
    alternate_stack.ss_sp = signal_stack;
    alternate_stack.ss_size = sizeof(signal_stack);
    if (sigaltstack(&alternate_stack, NULL) != 0) {
        close(sample_fd);
        sample_fd = -1;
        return;
    }
    memset(&action, 0, sizeof(action));
    action.sa_handler = sample_signal;
    sigemptyset(&action.sa_mask);
    action.sa_flags = SA_ONSTACK | SA_RESTART;
    if (sigaction(SIGPROF, &action, NULL) != 0) {
        close(sample_fd);
        sample_fd = -1;
        return;
    }

    memset(&timer, 0, sizeof(timer));
    timer.it_interval.tv_sec = 0;
    timer.it_interval.tv_usec = 1000000 / rate;
    if (timer.it_interval.tv_usec == 0)
        timer.it_interval.tv_usec = 1;
    timer.it_value = timer.it_interval;
    if (setitimer(ITIMER_PROF, &timer, NULL) != 0) {
        close(sample_fd);
        sample_fd = -1;
    }
}

__attribute__((destructor)) static void stop_sampling(void) {
    struct itimerval timer;
    if (sample_fd < 0)
        return;
    memset(&timer, 0, sizeof(timer));
    (void)setitimer(ITIMER_PROF, &timer, NULL);
    close(sample_fd);
    sample_fd = -1;
    if (dropped != 0) {
        char message[128];
        int length = snprintf(message, sizeof(message),
                              "llg_sigprof: dropped %d samples\n", (int)dropped);
        if (length > 0)
            (void)write(STDERR_FILENO, message, (size_t)length);
    }
}
