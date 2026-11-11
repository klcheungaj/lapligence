// SIM-040 negative: a library that calls an svdpi.h routine llg does not provide
// (svGetSelectBit, of the deprecated SV3.1a set of H.13) is an explicit link error
// that names the routine. Build neg_unprovided_routine.c into a shared library
// and pass it with --dpi-lib.
module tb;
    import "DPI-C" context function int neg_scope();
    initial $display("%0d", neg_scope());
endmodule
