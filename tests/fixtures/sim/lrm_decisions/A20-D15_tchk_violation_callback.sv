// Decision A20-D15: cbTchkViolation. A record runs once for every violation
// a timing check reports, at the time of the report, with obj the timing
// check (vpiTchk) and a NULL value. It runs before the notifier of the check
// changes: the callback still reads the old notifier value. A record with a
// NULL obj runs for every timing check, one with a timing check only for
// that check; registering it on any other object is a vpiError.
//
// IEEE 1800-2009 38.36.1 (SystemVerilog-1800-2009.txt L65023): "cbTchkViolation
// Timing check error occurred"; L65044-L65045: "For cbTchkViolation
// callbacks, the obj field shall be a handle to the timing check." Whether
// obj filters the record, and its order against the notifier, are llg
// choices.
//
// Build A20-D15_tchk_violation_callback.c as a VPI application (llg: static
// archive given to --dpi-lib; `task $d15()` declared as a system task).
`timescale 1ns/1ns
module dut (input clk, input d);
  reg notifier;
  specify
    $setup(d, posedge clk, 3, notifier);
    $hold(posedge clk, d, 2);
  endspecify
endmodule

module tb;
  reg clk = 1'b0, d = 1'b0;
  dut u (.clk(clk), .d(d));
  initial begin
    #1 $d15;
    // $setup: d changes 1 ns before the clock rises.
    #9 d = 1'b1;
    #1 clk = 1'b1;
    // $hold: d changes 1 ns after the clock rose.
    #1 d = 1'b0;
    #1 $display("%0d notifier=%b", $time, u.notifier);
    #5 $finish;
  end
endmodule
