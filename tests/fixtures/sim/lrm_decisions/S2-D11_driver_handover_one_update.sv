// Decision S2-D11: the zero-delay drivers of one net that evaluate in the
// same pass of a region all update their contributions before the net is
// resolved, so the net changes at most once for them. One tri-state driver
// turning off and another turning on in one blocking sequence is therefore
// no glitch: a waiting `@(m)` or `@(posedge m)` does not wake when the net
// ends at its old value. Drivers that evaluate in different passes (here
// separated by `#0`) still show the intermediate value.
//
// IEEE 1800-2009 4.9.1 (SystemVerilog-1800-2009.txt L3507-3509):
//   "A continuous assignment statement (see 10.3) corresponds to a process,
//   sensitive to the source elements in the expression. When the value of
//   the expression changes, it causes an active update event to be added to
//   the event region, using current values to determine the target."
// IEEE 1800-2009 4.5 (L3407-3408):
//   "E = any event from region;
//    remove E from the region;"
// IEEE 1800-2009 4.7 (L3465-3466):
//   "One source of nondeterminism is the fact that active events can be taken
//   off the Active or Reactive event region and processed in any order."
//
// The text lets the net's update events follow all the drivers' evaluation
// events of the pass; llg takes that order.
`timescale 1ns/1ns
module tb;
  bit a;
  bit en1, en2, e3, e4, c1, c2;
  wire m, n, k;
  int ev_m, ev_n, ev_k, pos_m;
  bufif1 b1 (m, a, en1);
  bufif1 b2 (m, a, en2);
  assign n = e3 ? a : 1'bz;
  assign n = e4 ? a : 1'bz;
  bufif1 b5 (k, a, c1);
  bufif1 b6 (k, a, c2);
  always @(m) ev_m = ev_m + 1;
  always @(posedge m) pos_m = pos_m + 1;
  always @(n) ev_n = ev_n + 1;
  always @(k) ev_k = ev_k + 1;
  initial begin
    #1 en1 = 1'b1; e3 = 1'b1; c1 = 1'b1;
    #1 $display("t2 m=%b n=%b k=%b events m=%0d n=%0d k=%0d pos_m=%0d",
                m, n, k, ev_m, ev_n, ev_k, pos_m);
    en1 = 1'b0; en2 = 1'b1;
    e3 = 1'b0; e4 = 1'b1;
    c1 = 1'b0; #0 c2 = 1'b1;
    #1 $display("t3 m=%b n=%b k=%b events m=%0d n=%0d k=%0d pos_m=%0d",
                m, n, k, ev_m, ev_n, ev_k, pos_m);
    a = 1'b1;
    #1 en2 = 1'b0; en1 = 1'b1;
    #1 $display("t5 m=%b events m=%0d pos_m=%0d", m, ev_m, pos_m);
    $finish;
  end
endmodule
