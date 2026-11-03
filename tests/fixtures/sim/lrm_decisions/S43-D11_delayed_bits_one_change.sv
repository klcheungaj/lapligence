// Decision S43-D11: the update events of delayed drivers that mature at the
// same time and drive different bits of one vector net are all applied
// before the net changes, so the vector changes once for them: a
// cbValueChange callback on the vector, on one of its bits or on a port
// formal merged with a bit runs once per time slot and reads the vector with
// every new bit (industry-standard behaviour). Each bit still takes every
// value it would take with one update at a time, and `@(q)` and `$monitor`
// see the same changes either way. Callbacks of different nets updated in the
// same slot may run in any order, so S43-D11_delayed_bits_one_change.c prints
// them sorted.
//
// IEEE 1800-2009 4.7 (SystemVerilog-1800-2009.txt L3465-3466):
//   "One source of nondeterminism is the fact that active events can be taken
//   off the Active or Reactive event region and processed in any order."
// IEEE 1800-2009 38.36.1 (L64642-64643):
//   "cbValueChange After value change on some variables, any expression, or
//   terminal"
// The text does not say how many changes the updates of one slot make; with
// the events taken in any order, applying the update events of one vector
// together is one of the allowed orders.
//
// Build S43-D11_delayed_bits_one_change.c as a VPI application (llg: static
// archive given to --dpi-lib).
`timescale 1ns/1ns
module stage(input d, output q, output r);
  buf #2 b(q, d);
  not #2 n(r, d);
endmodule

module tb;
  reg d = 1'b0;
  wire [7:0] q, r;
  integer wq = 0, wr = 0;
  genvar i;
  generate
    for (i = 0; i < 8; i = i + 1) begin : g
      stage s(.d(d), .q(q[i]), .r(r[i]));
    end
  endgenerate
  always @(q) wq = wq + 1;
  always @(r) wr = wr + 1;
  initial $monitor("%0t monitor q=%b r=%b", $time, q, r);
  initial begin
    #5 d = 1'b1;
    #5 d = 1'b0;
    #5 $display("wakes q=%0d r=%0d", wq, wr);
    $finish;
  end
endmodule
