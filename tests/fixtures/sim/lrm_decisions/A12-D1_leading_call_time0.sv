// Decision A12-D1: a $sdf_annotate call that leads an initial procedure
// (no timing control, task call or $finish before it) annotates before any
// time-0 transition: the first transitions of the annotated path and of the
// annotated port of an instance fed directly by a variable use the SDF
// delays. Process order at time 0 is otherwise not under the user's control.
//
// IEEE 1800-2009 4.7 (SystemVerilog-1800-2009.txt L3465-3466): "active
// events can be taken off the Active or Reactive event region and processed
// in any order."
// IEEE 1800-2009 32.4.4 (L52906-52907): "When annotating a PORT construct,
// the SDF annotator shall search for the port and, if it exists, shall
// annotate an interconnect delay to that port that shall represent the
// delay from all sources on the net to that port."
`timescale 1ns/1ps
module BUF (input a, output y);
  buf b (y, a);
  specify
    (a => y) = (1, 1);
  endspecify
endmodule
module tb;
  reg a = 1'b0;
  wire y, z;
  BUF u (a, y);
  BUF v (a, z);
  initial begin
    $sdf_annotate("A12-D1_leading_call_time0.sdf");
    $monitor("%0t y=%b z=%b", $realtime, y, z);
    #20 $finish;
  end
endmodule
