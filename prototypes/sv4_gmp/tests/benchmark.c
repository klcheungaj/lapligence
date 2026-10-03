#include "gmp4.h"
#include "../golden/llg_value.h"
#include "check.h"
#include <inttypes.h>
#include <string.h>
#include <time.h>

static volatile uint64_t sink;
static uint64_t seed=UINT64_C(0x439fe63762de2301);
static uint64_t next_word(void) {seed^=seed<<13;seed^=seed>>7;seed^=seed<<17;return seed;}
enum { ADD, MUL, AND, CLONE, COPY, ADD_REUSE, MUL_REUSE };
static const char *names[]={"add_known","mul_known","and","clone_known","copy_known","add_reuse","mul_reuse"};

static double measure(int backend, int operation, size_t count,
                      sv4_t a,sv4_t b,gmp4_t ga,gmp4_t gb) {
    sv4_t old=SV4_EMPTY;
    gmp4_t result=GMP4_EMPTY;
    gmp4_workspace_t scratch=GMP4_WORKSPACE_EMPTY;
    if(backend) {
        result=gmp4_zero(ga.width,ga.is_signed);
        if(operation==MUL_REUSE)gmp4_mul_into(&result,ga,gb,&scratch);
    } else old=sv4_zero(a.width,a.is_signed);
    clock_t start=clock();CHECK(start!=(clock_t)-1);
    if(!backend) {
        for(size_t i=0;i<count;++i) {
            switch(operation) {
            case ADD:case ADD_REUSE:sv4_replace(&old,sv4_add(a,b));break;
            case MUL:case MUL_REUSE:sv4_replace(&old,sv4_mul(a,b));break;
            case AND:sv4_replace(&old,sv4_and(a,b));break;
            case CLONE:sv4_replace(&old,sv4_clone(&a));break;
            default:sv4_copy(&old,&a);break;
            }
        }
    } else {
        for(size_t i=0;i<count;++i) {
            switch(operation) {
            case ADD:gmp4_replace(&result,gmp4_add(ga,gb));break;
            case MUL:gmp4_replace(&result,gmp4_mul(ga,gb));break;
            case AND:gmp4_replace(&result,gmp4_and(ga,gb));break;
            case CLONE:gmp4_replace(&result,gmp4_clone(&ga));break;
            case COPY:gmp4_copy(&result,&ga);break;
            case ADD_REUSE:gmp4_add_into(&result,ga,gb);break;
            default:gmp4_mul_into(&result,ga,gb,&scratch);break;
            }
        }
    }
    clock_t end=clock();CHECK(end!=(clock_t)-1);
    sink^=backend?gmp4_to_u64(result):sv4_to_u64(old);
    gmp4_destroy(&result);sv4_destroy(&old);gmp4_workspace_destroy(&scratch);
    return (double)(end-start)/CLOCKS_PER_SEC;
}
static size_t calibrate(int backend,int operation,sv4_t a,sv4_t b,gmp4_t ga,gmp4_t gb) {
    size_t n=16;
    while(measure(backend,operation,n,a,b,ga,gb)<0.005 && n<(1u<<24))n*=2;
    return n;
}
static void run(uint32_t width,int unknown,int operation) {
    size_t n=((size_t)width+63u)/64u;
    uint64_t *v=calloc(n*6u,sizeof(uint64_t));CHECK(v);
    uint64_t *x=v+n,*z=v+2u*n,*v2=v+3u*n,*x2=v+4u*n,*z2=v+5u*n;
    for(size_t i=0;i<n;++i) {
        if(unknown) {x[i]=next_word();z[i]=next_word()&~x[i];x2[i]=next_word();z2[i]=next_word()&~x2[i];}
        v[i]=next_word()&~(x[i]|z[i]);v2[i]=next_word()&~(x2[i]|z2[i]);
    }
    sv4_t a=sv4_from_limbs(v,x,z,width,0),b=sv4_from_limbs(v2,x2,z2,width,0);
    gmp4_t ga=gmp4_from_limbs(v,x,z,width,0),gb=gmp4_from_limbs(v2,x2,z2,width,0);
    size_t old_n=calibrate(0,operation,a,b,ga,gb),new_n=calibrate(1,operation,a,b,ga,gb);
    double old_times[5],new_times[5];
    for(size_t r=0;r<5;++r) {
        /* Alternate order to reduce ordering bias; do not report sanitizer timings. */
        if(r%2) {
            new_times[r]=measure(1,operation,new_n,a,b,ga,gb)*1e9/new_n;
            old_times[r]=measure(0,operation,old_n,a,b,ga,gb)*1e9/old_n;
        } else {
            old_times[r]=measure(0,operation,old_n,a,b,ga,gb)*1e9/old_n;
            new_times[r]=measure(1,operation,new_n,a,b,ga,gb)*1e9/new_n;
        }
    }
    printf("{\"kind\":\"timing\",\"width\":%u,\"operation\":\"%s%s\",\"old_iterations\":%zu,\"new_iterations\":%zu,\"old_ns\":[",
           width,names[operation],operation==AND?(unknown?"_xz":"_known"):"",old_n,new_n);
    for(size_t i=0;i<5;++i)printf("%s%.6f",i?",":"",old_times[i]);
    printf("],\"new_ns\":[");
    for(size_t i=0;i<5;++i)printf("%s%.6f",i?",":"",new_times[i]);
    puts("]}");
    sv4_destroy(&a);sv4_destroy(&b);gmp4_destroy(&ga);gmp4_destroy(&gb);free(v);
}
int main(void) {
    CHECK(!strcmp(gmp_version,"6.3.0") && mp_bits_per_limb==GMP_NUMB_BITS);
    printf("{\"kind\":\"layout\",\"gmp_version\":\"%s\",\"limb_bits\":%u,\"old_descriptor\":%zu,\"new_descriptor\":%zu,\"clock_ticks_per_second\":%ld}\n",
           gmp_version,(unsigned)GMP_NUMB_BITS,sizeof(sv4_t),sizeof(gmp4_t),(long)CLOCKS_PER_SEC);
    static const uint32_t widths[]={1,8,32,64,65,256,1024,4096,16384};
    for(size_t i=0;i<sizeof(widths)/sizeof(widths[0]);++i) {
        uint32_t w=widths[i];
        sv4_t old=sv4_zero(w,0);gmp4_t known=gmp4_zero(w,0),xz=gmp4_x(w,0);
        printf("{\"kind\":\"memory\",\"width\":%u,\"old_payload\":%zu,\"new_known_payload\":%zu,\"new_xz_payload\":%zu}\n",
               w,sv4_bytes(&old),gmp4_bytes(&known),gmp4_bytes(&xz));
        sv4_destroy(&old);gmp4_destroy(&known);gmp4_destroy(&xz);
        for(int op=ADD;op<=MUL_REUSE;++op)run(w,0,op);
        run(w,1,AND);
    }
    fprintf(stderr,"benchmark checksum: %" PRIu64 "\n",sink);
    return 0;
}
