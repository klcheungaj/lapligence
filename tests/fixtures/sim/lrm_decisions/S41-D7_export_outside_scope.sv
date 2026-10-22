// Decision S41-D7: a context import that calls an exported subroutine its
// current scope does not export, without svSetScope to a scope that does,
// is a fatal simulation error at that call.
//
// IEEE 1800-2009 35.5.3 (SystemVerilog-1800-2009.txt L55841-55844):
//   "This scope defines which exported SystemVerilog subroutines can be
//   called directly from the imported subroutine; only subroutines defined
//   and exported from the same scope as the import can be called directly. To
//   call any other exported SystemVerilog subroutines, the imported
//   subroutine shall first have to modify its current scope"
// The text does not say what happens otherwise; llg reports a fatal error
// naming the scope.
//
// The import is called from its own scope, so the caller's scope and the
// import's declaration scope agree.
//
// Expected: the first line only, then a fatal error (exit status nonzero).
// Build S41-D7_export_outside_scope.c into a shared library and load it with
// the simulator's DPI library option.
module side_m;
    import "DPI-C" context function int d7_call(input int n);

    int r = 0;

    initial begin
        $display("before");
        r = d7_call(1);
        $display("after r=%0d", r);
        $finish;
    end
endmodule

module tb;
    export "DPI-C" function d7_inc;

    function int d7_inc(input int n);
        return n + 1;
    endfunction

    side_m u();
endmodule
