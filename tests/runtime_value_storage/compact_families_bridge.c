#if LLG_ORACLE_COMPACT
#define LLG_SV4_GMP_PUBLIC_NAMES
#include "backend.h"
#else
#include "llg_value.h"
#endif
#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>

#include "compact_families_dispatch.h"

int main(void) {
    unsigned op;
    while (scanf("%u", &op) == 1) {
        sv4_t values[3] = {SV4_EMPTY, SV4_EMPTY, SV4_EMPTY};
        for (unsigned j = 0; j < 3; ++j) {
            unsigned width;
            int sign;
            if (scanf("%u %d", &width, &sign) != 2)
                return 1;
            size_t n = ((size_t)width + 63u) / 64u;
            uint64_t* p = (uint64_t*)calloc(3u * (n ? n : 1), sizeof(uint64_t));
            if (!p)
                return 1;
            for (size_t i = 0; i < 3u * n; ++i)
                if (scanf("%" SCNx64, &p[i]) != 1)
                    return 1;
            values[j] = sv4_from_limbs(p, p + n, p + 2u * n, width, (int8_t)sign);
            free(p);
        }
        sv4_t result = evaluate(op, values[0], values[1], values[2]);
        printf("%u %d %zu", llg_sv4_width(result), llg_sv4_signed(result), sv4_bytes(&result));
        for (unsigned plane = 0; plane < 3; ++plane)
            for (size_t i = 0; i < llg_sv4_words(result); ++i)
                printf(" %" PRIx64, llg_sv4_word(result, i, plane));
        putchar('\n');
        fflush(stdout);
        sv4_destroy(&result);
        sv4_destroy_array(values, 3);
    }
    return ferror(stdin) ? 1 : 0;
}
