#include "gmp4.h"
#include "check.h"
#include <string.h>
int main(int argc,char **argv) {
    CHECK(argc==2);
    gmp4_t a=GMP4_EMPTY;
    if(!strcmp(argv[1],"limit"))a=gmp4_zero(GMP4_WIDTH_LIMIT,0);
    else if(!strcmp(argv[1],"overflow"))a=gmp4_zero(UINT32_MAX,0);
    else if(!strcmp(argv[1],"mask_width"))a=gmp4_from_masks(1,0,0,65,0);
    else if(!strcmp(argv[1],"overlap"))a=gmp4_from_masks(0,1,1,1,0);
    else if(!strcmp(argv[1],"state"))a=gmp4_fill(4,1,0);
    else if(!strcmp(argv[1],"plane")) {a=gmp4_zero(1,0);(void)gmp4_word(a,0,3);}
    else if(!strcmp(argv[1],"drivers"))a=gmp4_resolve_wire(NULL,1,1,0);
    else if(!strcmp(argv[1],"driver_width")) {
        gmp4_t b=gmp4_zero(2,0);const gmp4_t *drivers[1]={&b};
        a=gmp4_resolve_wire(drivers,1,1,0);gmp4_destroy(&b);
    } else return 2;
    gmp4_destroy(&a);
    return 0;
}
