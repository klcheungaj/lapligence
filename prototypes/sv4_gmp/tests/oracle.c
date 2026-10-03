#include "gmp4.h"
#include "../golden/llg_value.h"
#include "check.h"
#include <inttypes.h>

typedef struct {
    uint32_t wa, wb;
    int8_t sa, sb;
    const uint64_t *a, *b, *add, *sub, *mul;
} oracle_case_t;
#include "oracle_vectors.inc"

static void expected(size_t index, const char *name, gmp4_t value, sv4_t old,
                     const uint64_t *bits, uint32_t width, int8_t sign) {
    CHECK(value.width == width && old.width == width);
    CHECK(value.is_signed == sign && old.is_signed == sign);
    CHECK(!gmp4_is_unknown(value) && !sv4_is_unknown(old));
    for (size_t i=0; i<((size_t)width+63u)/64u; ++i) {
        if (gmp4_word(value,i,0) != bits[i] || old.bits[i] != bits[i]) {
            fprintf(stderr,"math oracle case=%zu op=%s limb=%zu expected=%016" PRIx64 "\n",
                    index,name,i,bits[i]); exit(1);
        }
    }
    gmp4_destroy(&value); sv4_destroy(&old);
}
int main(void) {
    for (size_t i=0; i<sizeof(oracle_cases)/sizeof(oracle_cases[0]); ++i) {
        const oracle_case_t *v=&oracle_cases[i];
        gmp4_t a=gmp4_from_limbs(v->a,NULL,NULL,v->wa,v->sa);
        gmp4_t b=gmp4_from_limbs(v->b,NULL,NULL,v->wb,v->sb);
        sv4_t oa=sv4_from_limbs(v->a,NULL,NULL,v->wa,v->sa);
        sv4_t ob=sv4_from_limbs(v->b,NULL,NULL,v->wb,v->sb);
        uint32_t w=v->wa>v->wb?v->wa:v->wb;
        int8_t sign=v->sa && v->sb;
        expected(i,"add",gmp4_add(a,b),sv4_add(oa,ob),v->add,w,sign);
        expected(i,"sub",gmp4_sub(a,b),sv4_sub(oa,ob),v->sub,w,sign);
        expected(i,"mul",gmp4_mul(a,b),sv4_mul(oa,ob),v->mul,w,sign);
        gmp4_destroy(&a); gmp4_destroy(&b); sv4_destroy(&oa); sv4_destroy(&ob);
    }
    puts("400 independent Python-integer vectors x 3 arithmetic operations x 2 backends: PASS");
    return 0;
}
