/* SIM-099 DPI-C reference model of testbench.sv's ALU. Portable C11. */
#include "svdpi.h"

DPI_DLLESPEC int ref_alu(int op, int a, int b)
{
    switch (op) {
    case 0:
        return a + b;
    case 1:
        return (a - b) & 0xff;
    case 2:
        return a & b;
    default:
        return a ^ b;
    }
}
