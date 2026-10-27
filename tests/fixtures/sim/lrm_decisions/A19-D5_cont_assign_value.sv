// Decision A19-D5: vpi_get_value on a continuous assignment returns the value
// it drives: its right-hand side converted to the size of its left-hand side
// (zero- or sign-extended by the right-hand side's signedness, or truncated),
// which is also its vpiSize. vpiLhs and vpiRhs return the objects or selects
// the assignment names. A continuous assignment has no name: vpiName is NULL
// with a vpiError status.
//
// IEEE 1800-2009 37.43 (SystemVerilog-1800-2009.txt L60275-L60304) lists
// "-> value vpi_get_value()" (L60301-L60302) and vpiLhs/vpiRhs for a cont
// assign but defines neither the value nor the size; detail 1 (L60306) "The
// size of a cont assign bit is always scalar." and the vpiBit relation to
// cont assign bits suggest the size of the left-hand side. The text is
// silent; this is an llg choice. The diagram gives no name property.
//
// Build A19-D5_cont_assign_value.c as a VPI application (llg: static archive
// given to --dpi-lib; `task $d5()` declared as a system task).
`timescale 1ns/1ns
module tb;
    logic [3:0] narrow = 4'b1010;
    logic signed [3:0] neg = -4'sd3;
    logic [11:0] wide = 12'habc;
    wire [7:0] w_zero, w_sign, w_trunc;
    assign w_zero = narrow;
    assign w_sign = neg;
    assign w_trunc = wide;
    initial begin
        #1 $d5;
        $display("hdl %b %b %b", w_zero, w_sign, w_trunc);
        $finish;
    end
endmodule
