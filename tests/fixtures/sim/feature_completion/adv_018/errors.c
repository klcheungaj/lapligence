/* ADV-018 companion for the neg_*.sv protocol violations. Portable C11. */
#include "svdpi.h"
#include <stdio.h>

int sv_wait_e(int delay);

DPI_DLLESPEC int c_no_ack(void)
{
    printf("c_no_ack enter\n");
    int status = sv_wait_e(100);
    printf("c_no_ack status=%d returns 0\n", status);
    return 0;
}

DPI_DLLESPEC int c_func(int x)
{
    printf("c_func enter\n");
    int status = sv_wait_e(1);
    printf("c_func status=%d\n", status);
    return x;
}

DPI_DLLESPEC int c_again(void)
{
    printf("c_again enter\n");
    int status = sv_wait_e(100);
    printf("c_again status=%d disabled=%d\n", status, svIsDisabledState());
    sv_wait_e(1);
    printf("c_again not reached\n");
    return 1;
}
