#define _GNU_SOURCE
#define _POSIX_C_SOURCE 200809L

#include <errno.h>
#include <limits.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/resource.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

static long long monotonic_ns(void) {
    struct timespec now;
    if (clock_gettime(CLOCK_MONOTONIC, &now) != 0) {
        perror("clock_gettime");
        exit(2);
    }
    return (long long)now.tv_sec * 1000000000LL + now.tv_nsec;
}

static void usage(void) {
    fprintf(stderr, "usage: perf_measure --label LABEL --record PATH -- COMMAND [ARGS...]\n");
}

int main(int argc, char **argv) {
    const char *label = NULL;
    const char *record = NULL;
    int command_index = -1;

    for (int i = 1; i < argc; i++) {
        if (strcmp(argv[i], "--label") == 0 && i + 1 < argc) {
            label = argv[++i];
        } else if (strcmp(argv[i], "--record") == 0 && i + 1 < argc) {
            record = argv[++i];
        } else if (strcmp(argv[i], "--") == 0) {
            command_index = i + 1;
            break;
        } else {
            usage();
            return 2;
        }
    }

    if (label == NULL || record == NULL || command_index < 0 ||
        command_index >= argc) {
        usage();
        return 2;
    }

    long long start_ns = monotonic_ns();
    pid_t child = fork();
    if (child < 0) {
        perror("fork");
        return 2;
    }
    if (child == 0) {
        execvp(argv[command_index], &argv[command_index]);
        _exit(127);
    }

    int wait_status = 0;
    struct rusage usage_info;
    pid_t waited;
    do {
        waited = wait4(child, &wait_status, 0, &usage_info);
    } while (waited < 0 && errno == EINTR);
    if (waited < 0) {
        perror("wait4");
        return 2;
    }
    long long end_ns = monotonic_ns();

    char temporary[PATH_MAX];
    int written = snprintf(
        temporary, sizeof(temporary), "%s.tmp.%ld", record, (long)getpid());
    if (written < 0 || (size_t)written >= sizeof(temporary)) {
        fprintf(stderr, "perf_measure: record path is too long\n");
        return 2;
    }

    FILE *output = fopen(temporary, "w");
    if (output == NULL) {
        perror("fopen");
        return 2;
    }
    fprintf(output, "%s\t", label);
    if (WIFEXITED(wait_status)) {
        fprintf(output, "%d\t", WEXITSTATUS(wait_status));
    } else if (WIFSIGNALED(wait_status)) {
        fprintf(output, "%d\t", 128 + WTERMSIG(wait_status));
    } else {
        fprintf(output, "1\t");
    }
    fprintf(output, "%lld\t%ld\n", end_ns - start_ns, usage_info.ru_maxrss);
    if (fclose(output) != 0 || rename(temporary, record) != 0) {
        perror("perf_measure: write record");
        unlink(temporary);
        return 2;
    }

    if (WIFEXITED(wait_status)) {
        return WEXITSTATUS(wait_status);
    }
    if (WIFSIGNALED(wait_status)) {
        return 128 + WTERMSIG(wait_status);
    }
    return 1;
}
