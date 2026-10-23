// Decision A12-D11: a DEVICE entry on a cell without specify paths replaces
// the delays of the primitives that drive the cell's outputs (an and gate
// declared with delay 1 gets rise 3 and fall 4), as an ABSOLUTE annotation
// replaces any other timing value.
//
// IEEE 1800-2009 Table 32-1 (SystemVerilog-1800-2009.txt L52686-52687):
// "(DEVICE... All specify paths to module outputs. If no specify paths, all
// primitives driving module outputs."; 32.4 (L52644-52646): "Backannotation
// into SystemVerilog is done by matching SDF constructs to the
// corresponding SystemVerilog declarations and then replacing the existing
// SystemVerilog timing values with those from the SDF file."
`timescale 1ns/1ps
module AND2 (input a, input b, output y);
  and #1 g (y, a, b);
endmodule
module tb;
  reg a = 1'b0, b = 1'b1;
  wire y;
  AND2 u (a, b, y);
  initial begin
    #10 $sdf_annotate("A12-D11_device_replaces_primitive.sdf");
    #10 a = 1'b1;
    #20 a = 1'b0;
    #20 $finish;
  end
  always @(y) if ($realtime > 0) $display("%0t y=%b", $realtime, y);
endmodule
