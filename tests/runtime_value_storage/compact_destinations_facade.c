#define LLG_SV4_GMP_PUBLIC_NAMES
#include "backend.h"
#include <stdlib.h>

int main(void) {
    void (*ops[])(sv4_t*, sv4_t, sv4_t) = {sv4_add_into, sv4_sub_into, sv4_mul_into};
    for (unsigned op = 0; op < 3; ++op) {
        sv4_t a = SV4_S(7, 65), b = SV4_C(3, 65), d = SV4_EMPTY;
        ops[op](&d, a, b);
        uint64_t expected = op == 0 ? 10 : op == 1 ? 4 : 21;
        if (sv4_to_u64(d) != expected || llg_sv4_signed(d))
            abort();
        sv4_destroy(&a);
        sv4_destroy(&b);
        sv4_destroy(&d);
    }
    return 0;
}
