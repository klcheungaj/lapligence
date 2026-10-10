/* Review-only native controls. No generated HDL pipeline is exercised. */
#include "llg_rt.h"
#include <stdio.h>
#include <string.h>
static int check_order(const char *path, int policy, unsigned first, unsigned second) {
    sv4_t mem[2]={sv4_zero(8,0),sv4_zero(8,0)};
    sv4_t absent=sv4_zero(32,1); int32_t dims[2]={1,0};
    llg_memory_read(llg_string_bytes(path,strlen(path)),mem,2,8,0,0,dims,1,absent,absent,0,0,policy,NULL,0,16);
    int ok=sv4_to_u64(mem[0])==first && sv4_to_u64(mem[1])==second;
    printf("memory_policy_%d=%s physical[%llx,%llx]\n",policy,ok?"PASS":"FAIL",(unsigned long long)sv4_to_u64(mem[0]),(unsigned long long)sv4_to_u64(mem[1]));
    sv4_destroy(&mem[0]);sv4_destroy(&mem[1]);sv4_destroy(&absent);return !ok;
}
static int check_2d(const char *path) {
    sv4_t mem[4]; for(int i=0;i<4;i++)mem[i]=sv4_zero(8,0);
    sv4_t absent=sv4_zero(32,1);int32_t dims[4]={1,0,1,0};uint64_t strides[2]={2,1};
    llg_memory_read_view(llg_string_bytes(path,strlen(path)),mem,4,8,0,0,dims,2,strides,0,4,absent,absent,0,0,1,NULL,0,16);
    unsigned expect[4]={0x44,0x33,0x22,0x11};int ok=1;
    for(int i=0;i<4;i++)if(sv4_to_u64(mem[i])!=expect[i])ok=0;
    printf("memory_reversed_2d=%s\n",ok?"PASS":"FAIL");
    for(int i=0;i<4;i++)sv4_destroy(&mem[i]);sv4_destroy(&absent);return !ok;
}
int main(int argc,char **argv){
    if(argc!=3)return 2;
    /* Separate statements: the operands of `|` are unsequenced (C11 6.5p3),
     * and MSVC x64 evaluated the second check first, printing its line first. */
    int failed=check_order(argv[1],0,0x11,0x22);
    failed|=check_order(argv[1],1,0x22,0x11);
    failed|=check_2d(argv[2]);
    return failed;
}
