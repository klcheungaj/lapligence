// `pkg` and `sub` declare no time scale; `tb` follows a `timescale.
package pkg;
  localparam int W = 4;
endpackage

module sub;
  initial #1 $display("sub t=%0t", $realtime);
endmodule

`timescale 1us/1ns
module tb;
  sub u();
  initial begin
    #2 $display("tb t=%0t W=%0d", $realtime, pkg::W);
    $finish;
  end
endmodule
