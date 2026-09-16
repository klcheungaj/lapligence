// Instrument the production storage fragment without changing its allocator API.
#include "llg_value.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define CHECK(condition) do { \
    if (!(condition)) { \
        fprintf(stderr, "storage check failed at line %d: %s\n", __LINE__, #condition); \
        exit(2); \
    } \
} while (0)

typedef struct {
    void* pointer;
    size_t bytes;
} allocation_t;

static allocation_t allocations[32];
static size_t allocation_calls;
static size_t free_calls;
static size_t live_allocations;
static size_t live_bytes;
static size_t last_request;
static int fail_next;
static const sv4_storage_t* failure_destination;
static uint64_t* failure_destination_bits;

static void* tracked_malloc(size_t bytes) {
    CHECK(bytes > 0);
    if (fail_next) {
        fail_next = 0;
        if (failure_destination) {
            CHECK(live_allocations == 2);
            CHECK(failure_destination->bits == failure_destination_bits);
            CHECK(failure_destination->width == 17);
            CHECK(failure_destination->bits[0] == 123);
        }
        return NULL;
    }
    void* pointer = malloc(bytes);
    CHECK(pointer != NULL);
    memset(pointer, 0xa5, bytes);
    size_t slot = 0;
    while (slot < 32 && allocations[slot].pointer) slot++;
    CHECK(slot < 32);
    allocations[slot].pointer = pointer;
    allocations[slot].bytes = bytes;
    allocation_calls++;
    live_allocations++;
    live_bytes += bytes;
    last_request = bytes;
    return pointer;
}

static void tracked_free(void* pointer) {
    if (!pointer) return;
    size_t slot = 0;
    while (slot < 32 && allocations[slot].pointer != pointer) slot++;
    CHECK(slot < 32);
    live_bytes -= allocations[slot].bytes;
    allocations[slot] = (allocation_t){0};
    live_allocations--;
    free_calls++;
    free(pointer);
}

#define malloc tracked_malloc
#define free tracked_free
#include "value/storage.c"
#undef free
#undef malloc

static void check_empty(const sv4_storage_t* value) {
    CHECK(value->bits == NULL && value->x == NULL && value->z == NULL);
    CHECK(value->width == 0 && value->is_signed == 0);
    CHECK(sv4_storage_bytes(value) == 0);
}

static void check_widths(void) {
    const uint32_t widths[] = {
        0, 1, 31, 32, 63, 64, 65, 127, 128, 129, 1024, 65536,
        LLG_SUPPORTED_WIDTH_LIMIT - 1u
    };
    for (size_t i = 0; i < sizeof(widths) / sizeof(widths[0]); i++) {
        uint32_t width = widths[i];
        size_t limbs = (size_t)(width / 64u) + (width % 64u != 0);
        size_t before = allocation_calls;
        sv4_storage_t value = sv4_storage_zero(width, 0);
        CHECK(value.width == width);
        CHECK(sv4_storage_bytes(&value) == 3u * limbs * sizeof(uint64_t));
        if (!width) {
            CHECK(allocation_calls == before);
            check_empty(&value);
        } else {
            CHECK(allocation_calls == before + 1u);
            CHECK(last_request == 3u * limbs * sizeof(uint64_t));
            CHECK(value.x == value.bits + limbs);
            CHECK(value.z == value.x + limbs);
            for (size_t j = 0; j < limbs; j++) {
                CHECK(value.bits[j] == 0);
                CHECK(value.x[j] == 0 && value.z[j] == 0);
            }
            value.bits[limbs - 1u] = UINT64_C(1) << ((width - 1u) % 64u);
            sv4_storage_t copy = sv4_storage_clone(&value);
            CHECK(copy.bits != value.bits);
            CHECK(copy.bits[limbs - 1u] == value.bits[limbs - 1u]);
            value.bits[limbs - 1u] = 0;
            CHECK(copy.bits[limbs - 1u] != 0);
            sv4_storage_destroy(&copy);
        }
        sv4_storage_destroy(&value);
        sv4_storage_destroy(&value);
        check_empty(&value);
        CHECK(live_allocations == 0 && live_bytes == 0);
    }
}

static void check_copies_and_moves(void) {
    uint64_t bits[2] = {UINT64_C(0x12345678), UINT64_MAX};
    uint64_t x[2] = {2, UINT64_MAX};
    uint64_t z[2] = {4, 0};
    sv4_storage_t source = sv4_storage_from_limbs(bits, x, z, 65, -1);
    CHECK(source.width == 65 && source.is_signed == 1);
    CHECK(source.bits[1] == 1 && source.x[1] == 1 && source.z[1] == 0);
    bits[0] = 0;
    x[0] = 0;
    z[0] = 0;
    CHECK(source.bits[0] == UINT64_C(0x12345678));
    CHECK(source.x[0] == 2 && source.z[0] == 4);

    sv4_storage_t target = sv4_storage_zero(1024, 0);
    sv4_storage_copy(&target, &source);
    CHECK(live_allocations == 2);
    CHECK(live_bytes == 2u * 3u * 2u * sizeof(uint64_t));
    CHECK(target.width == 65 && target.is_signed == 1);
    CHECK(target.bits != source.bits);
    source.bits[0] = 9;
    CHECK(target.bits[0] == UINT64_C(0x12345678));
    size_t before = allocation_calls;
    uint64_t* pointer = target.bits;
    sv4_storage_copy(&target, &target);
    sv4_storage_move(&target, &target);
    CHECK(target.bits == pointer && allocation_calls == before);

    pointer = source.bits;
    sv4_storage_move(&target, &source);
    CHECK(target.bits == pointer && target.bits[0] == 9);
    CHECK(live_allocations == 1);
    check_empty(&source);
    sv4_storage_destroy(&source);
    sv4_storage_t empty = SV4_STORAGE_EMPTY;
    sv4_storage_move(&target, &empty);
    check_empty(&target);
    check_empty(&empty);
    CHECK(live_allocations == 0 && live_bytes == 0);

    target = sv4_storage_from_limbs(NULL, NULL, NULL, 129, 0);
    for (size_t i = 0; i < 3; i++)
        CHECK(target.bits[i] == 0 && target.x[i] == 0 && target.z[i] == 0);
    sv4_storage_copy(&target, &empty);
    check_empty(&target);
    CHECK(live_allocations == 0);
    sv4_storage_destroy(NULL);

    uint64_t ignored = UINT64_MAX;
    target = sv4_storage_from_limbs(&ignored, &ignored, &ignored, 0, 1);
    CHECK(target.width == 0 && target.is_signed == 1 && target.bits == NULL);
    sv4_storage_destroy(&target);
    check_empty(&target);
}

static void check_replacement_cycles(void) {
    sv4_storage_t retained = SV4_STORAGE_EMPTY;
    for (uint32_t i = 0; i < 10000; i++) {
        sv4_storage_t next = sv4_storage_zero(1u + i % 129u, 0);
        next.bits[0] = 1;
        sv4_storage_copy(&retained, &next);
        CHECK(retained.bits != next.bits);
        sv4_storage_move(&retained, &next);
        check_empty(&next);
        CHECK(live_allocations == 1);
        CHECK(live_bytes == sv4_storage_bytes(&retained));
    }
    sv4_storage_destroy(&retained);
    CHECK(live_allocations == 0 && live_bytes == 0);
    CHECK(allocation_calls == free_calls);
}

int main(int argc, char** argv) {
    if (argc > 1) {
        if (strcmp(argv[1], "limit") == 0) {
            (void)sv4_storage_zero(LLG_SUPPORTED_WIDTH_LIMIT, 0);
        } else if (strcmp(argv[1], "uint32-max") == 0) {
            (void)sv4_storage_zero(UINT32_MAX, 0);
        } else if (strcmp(argv[1], "oom") == 0) {
            fail_next = 1;
            (void)sv4_storage_zero(65, 0);
        } else if (strcmp(argv[1], "oom-copy") == 0) {
            sv4_storage_t destination = sv4_storage_zero(17, 0);
            sv4_storage_t source = sv4_storage_zero(65, 0);
            destination.bits[0] = 123;
            failure_destination = &destination;
            failure_destination_bits = destination.bits;
            fail_next = 1;
            sv4_storage_copy(&destination, &source);
        }
        CHECK(0);
    }
    check_widths();
    check_copies_and_moves();
    check_replacement_cycles();
    puts("dynamic packed storage: OK");
    return 0;
}
