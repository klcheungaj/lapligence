// Decision A12-D12: an INCREMENT that would make a delay negative leaves it
// at 0, as 32.7 requires for pulse limits; the rise delay 1 - 5 becomes 0
// and the fall delay 1 + 2 becomes 3.
//
// IEEE 1800-2009 32.7 (SystemVerilog-1800-2009.txt L53050-53051):
// "Annotations in INCREMENT mode can result in pulse limits less than 0, in
// which case they shall be adjusted to 0." The text says nothing about
// negative delays.
`timescale 1ns/1ps
module BUF (input a, output y);
  buf b (y, a);
  specify
    (a => y) = (1, 1);
  endspecify
endmodule
module tb;
  reg a = 1'b0;
  wire y;
  BUF u (a, y);
  initial begin
    #10 $sdf_annotate("A12-D12_increment_floor.sdf");
    #10 a = 1'b1;
    #20 a = 1'b0;
    #20 $finish;
  end
  always @(y) if ($realtime > 0) $display("%0t y=%b", $realtime, y);
endmodule
