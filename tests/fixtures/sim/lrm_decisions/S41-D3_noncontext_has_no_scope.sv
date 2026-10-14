// Decision S41-D3: a noncontext import has no context scope and no caller
// information, also when a context call chain reaches it through an export:
// svGetScope returns NULL and svGetCallerInfo returns 0. A context import
// called from SystemVerilog gets its call site.
//
// IEEE 1800-2009 35.5.3 (SystemVerilog-1800-2009.txt L55783-55785):
//   "This implies that the context property at the root of the DPI import
//   call chain or any intermediate import call in the call chain is not
//   transitively promoted to subsequent import calls in the DPI import call
//   chain."
// H.9.3 (L72673-72674): svGetScope: "The return value is undefined if this
//   function is invoked from a noncontext imported function."
// H.9.3 (L72730-72736): svGetCallerInfo: "Whether this information is
//   available or not is implementation-specific."
// The values in noncontext imports are undefined by the text; llg returns
// NULL and 0. The caller line is an llg choice of the availability H.9.3
// leaves open.
//
// Build S41-D3_noncontext_has_no_scope.c into a shared library and load it
// with the simulator's DPI library option.
module tb;
    import "DPI-C" function int d3_plain();
    import "DPI-C" context function int d3_ctx();
    export "DPI-C" function d3_back;

    function int d3_back();
        return d3_plain();
    endfunction

    int r = 0;

    initial begin
        r = d3_plain();
        $display("plain=%0d", r);
        r = d3_ctx();
        $display("ctx=%0d", r);
        $finish;
    end
endmodule
