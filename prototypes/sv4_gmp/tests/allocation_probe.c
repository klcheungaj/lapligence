#include "gmp4.h"
#include "check.h"
#include <stdint.h>
#include <stdlib.h>

/* Test-only instrumentation; no such counters or globals exist in gmp4. */
static size_t attempts, fail_at=SIZE_MAX, live;
void *sv4_test_alloc(size_t bytes) {
    if (attempts++ == fail_at) return NULL;
    void *p=malloc(bytes); if(p) ++live; return p;
}
void sv4_test_free(void *p) { if(p) { CHECK(live>0); --live; free(p); } }
int main(int argc,char **argv) {
    if(argc>1) fail_at=(size_t)strtoull(argv[1],NULL,10);
    gmp4_t a=gmp4_from_u64(7,64,0),b=gmp4_fill(3,64,0),r=gmp4_and(a,b);
    gmp4_destroy(&r);r=gmp4_add(a,a);gmp4_destroy(&r);
    gmp4_destroy(&a);gmp4_destroy(&b);CHECK(attempts==0);
    a=gmp4_from_u64(7,65,0); b=gmp4_from_u64(9,65,0);
    r=gmp4_clone(&a); size_t before=attempts;
    for(unsigned i=0;i<100;++i) {gmp4_copy(&r,&a);gmp4_add_into(&r,r,b);}
    CHECK(attempts==before);
    gmp4_set_bit(&a,64,2);gmp4_set_bit(&a,64,0);gmp4_compact(&a);
    gmp4_t wide=gmp4_fill(3,1024,1);
    gmp4_copy(&r,&wide);gmp4_copy(&r,&a);gmp4_destroy(&wide);
    wide=gmp4_add(a,r);gmp4_destroy(&wide);
    wide=gmp4_from_u64(11,257,0);
    gmp4_replace(&r,gmp4_add(a,wide));
    gmp4_workspace_t ws=GMP4_WORKSPACE_EMPTY;
    gmp4_mul_into(&r,a,wide,&ws);
    before=attempts;
    for(unsigned i=0;i<100;++i)gmp4_mul_into(&r,a,wide,&ws);
    CHECK(attempts==before);
    gmp4_replace(&b,gmp4_fill(2,1024,0));
    gmp4_mul_into(&r,wide,wide,&ws);
    gmp4_destroy(&a);gmp4_destroy(&b);gmp4_destroy(&r);gmp4_destroy(&wide);
    gmp4_workspace_destroy(&ws);CHECK(live==0);
    printf("%zu\n",attempts);
    return 0;
}
