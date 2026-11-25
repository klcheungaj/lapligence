// Decision S41-D9: when the run ends in an exported subroutine ($finish here;
// $fatal and run-ending errors likewise), the C code that called it does not
// continue, nor does the C code of any outer import of the call chain. Nothing
// is copied out of the abandoned imports, and the final procedures run next.
//
// IEEE 1800-2009 20.2 (SystemVerilog-1800-2009.txt L34119):
//   "The $finish system task causes the simulator to exit and pass control back to the host operating system."
// The text does not say when foreign code on the call chain stops; llg follows
// the industry-standard behaviour.
//
// Build S41-D9_run_ends_in_export.c into a shared library and load it with the
// simulator's DPI library option.
module tb;
    import "DPI-C" context function int d9_outer();
    import "DPI-C" context function void d9_inner_call();
    export "DPI-C" function d9_middle;
    export "DPI-C" function d9_inner;

    function int d9_middle();
        $display("middle");
        d9_inner_call();
        $display("middle after");
        return 1;
    endfunction

    function int d9_inner();
        $display("inner");
        $finish(0);
        $display("inner after");
        return 0;
    endfunction

    int r = 7;

    initial begin
        $display("before");
        r = d9_outer();
        $display("after");
    end
    final $display("final r=%0d", r);
endmodule
