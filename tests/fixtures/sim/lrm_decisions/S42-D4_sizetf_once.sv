// Decision S42-D4: sizetf runs at most once per registered system
// function, after the compiletf of its first call site, with no system
// function call available (vpi_handle(vpiSysTfCall, NULL) is NULL); every
// call site of that function returns the width it gave.
//
// IEEE 1800-2009 36.8.1 (SystemVerilog-1800-2009.txt L56517-56518):
//   "Each sizetf routine shall be called at most once. It shall be called if
//   its associated system function appears in the design."
// 36.10.2 (L56682-56683): "The next earliest phase is when the sizetf
//   routines are called for the user-defined system functions. At this phase,
//   no additional access is permitted."
// The order against compiletf is not specified; running it after the first
// compiletf is an llg choice.
//
// Build S42-D4_sizetf_once.c as a VPI application (llg: static archive given
// to --dpi-lib; `function logic [5:0] $d4(input logic [7:0] x)` and
// `task $d4_report()`).
module tb;
    logic [7:0] a, b;
    initial begin
        a = $d4(8'd1);
        b = $d4(8'd200);
        $display("a=%0d b=%0d", a, b);
        $d4_report;
        $finish;
    end
endmodule
