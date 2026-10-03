#include "gmp4.h"
#include "check.h"
#include <string.h>

static void boundary(uint32_t width) {
    gmp4_t a=gmp4_from_u64(1,width,0), b=gmp4_clone(&a), c=GMP4_EMPTY;
    size_t known_bytes=gmp4_bytes(&a);
    gmp4_copy(&a,&a); gmp4_move(&a,&a); gmp4_replace(&a,a);
    CHECK(gmp4_same(a,b));
    gmp4_set_bit(&a,width ? width-1u : 0,2);
    CHECK(width == 0 || !gmp4_same(a,b));
    CHECK(!gmp4_is_unknown(b));
    if (width > 64) CHECK(gmp4_bytes(&a) == 2u * known_bytes);
    gmp4_set_bit(&a,width ? width-1u : 0,3);
    CHECK(gmp4_get_bit(a,UINT64_MAX) == 2);
    gmp4_set_bit(&a,UINT64_MAX,1);
    if (width) CHECK(gmp4_get_bit(a,width-1u) == 3);
    gmp4_assign(&a,b);
    CHECK(gmp4_same(a,b));
    gmp4_compact(&a); CHECK(gmp4_bytes(&a) == known_bytes);
    gmp4_move(&c,&a); CHECK(a.width == 0 && gmp4_bytes(&a) == 0);
    CHECK(gmp4_same(c,b));
    /* Explicit ownership transfer by raw relocation, clearing the original.
     * No self-relative pointer can be left behind in inline storage. */
    gmp4_t moved; memcpy(&moved,&c,sizeof(c)); c=(gmp4_t)GMP4_EMPTY;
    CHECK(gmp4_same(moved,b));
    gmp4_destroy(&moved); gmp4_destroy(&moved);
    gmp4_destroy(&a); gmp4_destroy(&b); gmp4_destroy(&c);
}
int main(void) {
    static const uint32_t widths[]={0,1,31,32,63,64,65,127,128,129,257,4096,GMP4_WIDTH_LIMIT-1u};
    for (size_t i=0;i<sizeof(widths)/sizeof(widths[0]);++i) boundary(widths[i]);
    gmp4_t a=gmp4_from_masks(UINT64_MAX,1,2,3,0);
    CHECK(gmp4_word(a,0,0)==4 && gmp4_word(a,0,1)==1 && gmp4_word(a,0,2)==2);
    CHECK(gmp4_word(a,SIZE_MAX,0)==0);
    gmp4_destroy(&a);
    /* A borrowed inline argument remains valid while destination shape changes. */
    a=gmp4_from_u64(7,64,0);
    gmp4_t b=gmp4_from_u64(9,65,0), expected=gmp4_mul(a,b);
    gmp4_workspace_t ws=GMP4_WORKSPACE_EMPTY;
    gmp4_mul_into(&a,a,b,&ws); CHECK(gmp4_same(a,expected));
    gmp4_destroy(&expected);
    expected=gmp4_mul(a,a);
    gmp4_mul_into(&a,a,a,&ws); CHECK(gmp4_same(a,expected));
    gmp4_workspace_destroy(&ws); gmp4_workspace_destroy(&ws);
    gmp4_destroy(&a);gmp4_destroy(&b);gmp4_destroy(&expected);
    puts("ownership, promotion/compaction, maximum width and relocation: PASS");
    return 0;
}
