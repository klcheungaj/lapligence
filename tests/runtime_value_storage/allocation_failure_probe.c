/* Fatal allocation failure inside runtime-reachable packed operations, with
 * either selected backend: constructors, wide copies, X/Z storage growth and
 * a container element capture and a whole record element copy. The tracked
 * allocator injects the failure. */
#include "llg_container.h"
#include "probe.h"
#include <string.h>

int main(int argc, char** argv) {
    CHECK(argc == 2);
    sv4_t wide = sv4_from_u64(5, 129, 0);
    if (strcmp(argv[1], "construct") == 0) {
        value_test_fail_allocation_after(1);
        sv4_t failed = sv4_zero(4097, 0);
        sv4_destroy(&failed);
    } else if (strcmp(argv[1], "clone") == 0) {
        value_test_fail_allocation_after(1);
        sv4_t failed = sv4_clone(&wide);
        sv4_destroy(&failed);
    } else if (strcmp(argv[1], "unknown") == 0) {
        /* Legacy keeps X/Z planes already; compact grows its B plane here. */
        sv4_t unknown = sv4_x(129, 0);
        value_test_fail_allocation_after(1);
        sv4_t failed = sv4_add(wide, unknown);
        sv4_destroy(&failed);
        sv4_destroy(&unknown);
    } else if (strcmp(argv[1], "queue") == 0) {
        llg_queue_t queue;
        llg_queue_init(&queue, 129, 0, 0, UINT64_MAX);
        value_test_fail_allocation_after(1);
        llg_queue_push_back(&queue, wide);
        llg_queue_destroy(&queue);
    } else if (strcmp(argv[1], "record_queue") == 0) {
        /* A whole record element copied into a queue (SIM-006). */
        static const llg_value_desc_t wide_desc = {LLG_VALUE_PACKED, 0, 129, 0, 0, 0, 0, NULL, NULL, 0};
        static const llg_value_member_desc_t members[] = {{&wide_desc}, {&wide_desc}};
        static const llg_value_desc_t record_desc = {
            LLG_VALUE_AGGREGATE, 1, 0, 0, 0, 0, 2, NULL, members, 2};
        llg_queue_value_array_t queue;
        llg_value_t record = {0};
        llg_queue_value_init(&queue, &record_desc, UINT64_MAX);
        llg_native_value_init(&record, &record_desc);
        value_test_fail_allocation_after(1);
        llg_queue_value_push_value(&queue, 1, &record);
        llg_native_value_destroy(&record);
        llg_queue_value_destroy(&queue);
    } else {
        CHECK(0);
    }
    sv4_destroy(&wide);
    return 0;
}
