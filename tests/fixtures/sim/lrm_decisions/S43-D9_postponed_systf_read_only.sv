// Decision S43-D9: a user VPI system function may be an argument of
// $strobe and $monitor. It runs in the read-only Postponed region: it reads
// values and returns its result, and its vpi_put_value to a design variable
// fails with a vpiError status and changes nothing.
//
// IEEE 1800-2009 4.4.2.9 (SystemVerilog-1800-2009.txt L3212-3214): "Within
// this region, it is illegal to write values to any net or variable or to
// schedule an event in any previous region within the current time slot."
// 38.34 (vpi_put_value) does not say how such a write is reported; llg
// reports it as an error and keeps the value. Writing the function's own
// result (the vpiSysFuncCall handle) is not a design write.
//
// Build S43-D9_postponed_systf_read_only.c as a VPI application (llg: static
// archive given to --dpi-lib; `function int $bump(input int v)` declared).
`timescale 1ns/1ns
module tb;
    int x = 1;
    int y = 0;
    initial begin
        $strobe("strobe bump=%0d x=%0d y=%0d", $bump(x), x, y);
        #1 $display("active x=%0d y=%0d", x, y);
        $finish;
    end
endmodule
