// Decision S22-D10: nothing of a generate block that elaboration does not
// instantiate exists, so an `$sdf_annotate` call there never runs: the
// buffer keeps its specify path delay of 1 and the IOPATH delays of the SDF
// file (3, 4) are not applied. A call that leads an initial procedure of a
// selected block would apply before simulation starts (decision A12-D1).
//
// IEEE 1800-2009 27.3 (SystemVerilog-1800-2009.txt L45986-45988): "The
// elaboration of a generate construct results in zero or more instances of
// a generate block. An instance of a generate block is similar in some ways
// to an instance of a module. It creates a new level of hierarchy. It brings
// the objects, behavioral constructs, and module instances within the block
// into existence." 27.5 L46364-46366: "The conditional generate constructs,
// if-generate and case-generate, select at most one generate block from a
// set of alternative generate blocks based on constant expressions evaluated
// during elaboration. The selected generate block, if any, is instantiated
// into the model."
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
  if (0) begin : g
    initial $sdf_annotate("S22-D10_untaken_sdf_annotate.sdf");
  end
  initial begin
    $monitor("%0t y=%b", $realtime, y);
    #20 $finish;
  end
endmodule
