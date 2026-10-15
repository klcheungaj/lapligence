// SIM-043 A01/A03: callbacks that register and remove callbacks while they
// run, callback handle lifetime and unsupported reasons (reentry.c).
`timescale 1ns / 1ns
module tb;
  parameter int P = 3;
  logic [3:0] v = 0;
  logic [7:0] m[0:1];
  initial begin
    #1 v = 4'd1;
    #1 v = 4'd2;
    #1 v = 4'd3;
    m[0] = 8'd0;
    #10 $display("[%0t] hdl: v=%0d m0=%0d P=%0d", $time, v, m[0], P);
    $finish;
  end
endmodule
