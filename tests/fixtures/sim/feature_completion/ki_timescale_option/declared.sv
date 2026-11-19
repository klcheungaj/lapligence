// Every design element declares its time scale: a `timescale directive
// before the package and `sub`, and timeunit/timeprecision in `tb`.
`timescale 1ns/1ps
package pkg;
  localparam int W = 4;
endpackage

module sub;
  initial #1 $display("sub t=%0t", $realtime);
endmodule

module tb;
  timeunit 1us;
  timeprecision 1ns;
  sub u();
  initial begin
    #2 $display("tb t=%0t W=%0d", $realtime, pkg::W);
    $finish;
  end
endmodule
