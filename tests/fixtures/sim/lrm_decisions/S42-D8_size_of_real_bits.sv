// Decision S42-D8: vpiSize of a variable array is its element count and of
// an integral variable its width in bits; vpiSize of a real variable, which
// the text leaves undefined, is the size of its IEEE 754 storage: 64 for
// real and realtime, 32 for shortreal, with no error status
// (industry-standard behaviour).
//
// IEEE 1800-2009 37.17 detail 9 (SystemVerilog-1800-2009.txt L58760-58764):
//   "vpiSize for a variable array shall return the number of variables in the
//   array. For variables belonging to an integer data type (see 6.11), for
//   enum vars, and for packed struct and union variables, vpiSize shall
//   return the size of the variable in bits. ... For all other variables, the
//   behavior of the vpiSize property is not defined."
//
// Build S42-D8_size_of_real_bits.c as a VPI application (llg: static
// archive given to --dpi-lib; `task $d8()` declared as a system task).
module tb;
    logic [7:0] mem [0:2];
    int i;
    real r;
    initial begin
        $d8;
        $finish;
    end
endmodule
