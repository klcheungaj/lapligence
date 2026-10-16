// Decision S99-D1: a concurrent assertion in procedural code is a procedural
// concurrent assertion. Reaching it queues one pending instance on the
// executing process; the instance matures in the Observed region and begins
// one attempt at its leading clock. A process that resumes from an event
// control flushes its queue first. llg previously ran these assertions as
// static assertions at every clock, ignoring the procedure's control flow.
//
// IEEE 1800-2009 16.15.6 (SystemVerilog-1800-2009.txt L26748-26751): "a
// procedural concurrent assertion is not immediately evaluated when reached
// in procedural code. Instead, the assertion and the current values of all
// constant and automatic expressions appearing in its assertion arguments
// (see 16.15.6.1) are placed in a procedural assertion queue associated with
// the currently executing process." L26767-26769: "If a procedural
// assertion flush point (see 16.15.6.2) is reached in a process, its
// procedural assertion queue is cleared." 16.15.6.2 L27018-27019: a flush
// point is reached when "The process, having been suspended earlier due to
// reaching an event control or wait statement, resumes execution."
// p1 is queued only while en is 1 (time steps 15 and 25). At 42 b1 runs
// twice; the first instance of a2 is flushed, so a2 reports once at 45.
`timescale 1ns/1ns
module tb;
  logic clk = 0;
  logic a = 0, b = 0, en = 0;
  always #5 clk = ~clk;
  default clocking @(posedge clk); endclocking
  always @(posedge clk) begin
    if (en)
      p1: assert property (b) $display("%0t p1 pass", $time); else $display("%0t p1 fail", $time);
  end
  always @(a or b) begin : b1
    a2: assert property (a == b) $display("%0t a2 pass", $time); else $display("%0t a2 fail", $time);
  end
  initial begin
    #12 en = 1;
    #20 en = 0;
    #10 a = 1;
    #6 $finish;
  end
  initial #42 b <= 1;
endmodule
