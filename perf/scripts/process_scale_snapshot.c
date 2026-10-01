#define _POSIX_C_SOURCE 200809L
#include <stdio.h>
#include <stdlib.h>

/* Called synchronously while every spawned child is parked. */
void llg_scale_snapshot(void) {
    const char* output = getenv("LLG_SCALE_SNAPSHOT");
    if (!output) return;
    FILE* source = fopen("/proc/self/smaps", "r");
    FILE* destination = fopen(output, "w");
    if (!source || !destination) {
        perror("process scale snapshot");
        abort();
    }
    char buffer[4096];
    size_t count;
    while ((count = fread(buffer, 1, sizeof(buffer), source)) != 0) {
        if (fwrite(buffer, 1, count, destination) != count) abort();
    }
    if (ferror(source) || fclose(destination)) abort();
    fclose(source);
}
