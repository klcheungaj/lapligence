// ADV-014 A03: each clock event of an assertion with global clocking future
// sampled value functions waits, with its captured operands, for the next
// global clock tick (IEEE 1800-2009 16.9.4). Ten assertion ticks fall in each
// global clock period, so about ten evaluations are pending at once and
// their buffers are reused; while the gated global clock does not tick
// (2000-4000) they accumulate, and the queue counts against
// LLG_SEQUENCE_THREAD_LIMIT. Every tick before the last global clock tick
// (4090) reports: the odd times 1 to 4089.
`timescale 1ns / 1ns
module tb;
  logic clk = 1'b0, fast = 1'b0, en = 1'b1, a = 1'b0;
  int passes = 0;
  global clocking gc @(posedge clk iff en);
  endclocking
  always #10 clk = ~clk;
  always #1 fast = ~fast;
  initial begin
    #2000 en = 1'b0;
    #2000 en = 1'b1;
    #100 $display("done passes=%0d", passes);
    $finish;
  end
  bud: assert property (@(posedge fast) $steady_gclk(a)) passes++;
endmodule
