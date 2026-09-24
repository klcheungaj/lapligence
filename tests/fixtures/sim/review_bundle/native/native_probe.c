/* Review-only probe of the supplied native helpers, not generated HDL. */
#include "llg_rt.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

int main(int argc, char **argv) {
    if (argc != 2) return 2;
    sv4_t selector = sv4_x(1,0), z = sv4_zero(1,0); z.z[0] = 1;
    sv4_t mux = sv4_mux(selector,z,z);
    printf("mux_x_z_z: x=%llu z=%llu\n", (unsigned long long)mux.x[0], (unsigned long long)mux.z[0]);
    sv4_destroy(&mux); sv4_destroy(&selector); sv4_destroy(&z);
    sv4_t memory[2] = {sv4_from_u64(1,2,0), sv4_from_u64(0,2,0)};
    sv4_t legal[2] = {sv4_from_u64(0,2,0), sv4_from_u64(1,2,0)};
    sv4_t absent = sv4_zero(32,1);
    int32_t dims[2] = {0,1};
    llg_memory_read(llg_string_bytes(argv[1],strlen(argv[1])),memory,2,2,0,0,dims,1,
                    absent,absent,0,0,LLG_MEMORY_ADDRESSING_SYSTEMVERILOG_2009,legal,2,16);
    printf("enum_after_read: %llu %llu\n", (unsigned long long)sv4_to_u64(memory[0]), (unsigned long long)sv4_to_u64(memory[1]));
    for(int i=0;i<2;++i) {sv4_destroy(&memory[i]);sv4_destroy(&legal[i]);}
    sv4_destroy(&absent);
    return 0;
}
