// Decision S41-D2: when C calls an exported subroutine, outputs and inouts
// are written back to the C locations one at a time in declaration order,
// after the subroutine body; the function result is returned last. When C
// passes one address for several formals, the last in declaration order wins.
// Inouts are all copied in before the body runs.
//
// IEEE 1800-2009 35.6.1 (SystemVerilog-1800-2009.txt L56128-56131):
//   "For the SystemVerilog side of the interface, the semantics of arguments
//   passing is as if input arguments are passed by copy-in, output arguments
//   are passed by copy-out, and inout arguments were passed by copy- in,
//   copy-out."
// The text does not order the copy-outs; llg uses declaration order, as for
// imports (S40-D2) and native subroutines.
//
// Build S41-D2_export_copy_back.c into a shared library and load it with the
// simulator's DPI library option.
module tb;
    import "DPI-C" context function void d2_run();
    export "DPI-C" function d2_pair;
    export "DPI-C" function d2_both;

    function int d2_pair(input int a, output int x, output int y);
        x = a;
        y = a + 1;
        return a + 2;
    endfunction

    function void d2_both(inout int p, inout int q);
        p = p + 10;
        q = q + 100;
    endfunction

    initial begin
        d2_run();
        $finish;
    end
endmodule
