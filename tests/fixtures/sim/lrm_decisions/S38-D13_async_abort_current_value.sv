// Decision S38-D13: accept_on/reject_on act in the time step in which their
// condition holds, reading its current value there; an evaluation that
// starts or ends on a clock tick reads the sampled value at that tick.
// - r1: c rises at 23 between the ticks: the attempt from 10 fails at 23;
//   the attempts from 30 and 50 start with c sampled 1 and fail at once.
// - r2: g rises and falls at 13 within one statement: never seen.
// - r3: e is 1 during [26, 27): the attempt from 10 is accepted at 26 (a
//   vacuous success: the assert passes); the one from 30 fails at 50.
// - r4: n rises by a nonblocking assignment in the step of the tick at 30:
//   the attempt from 10 ends on that tick with n sampled 0 and passes; the
//   attempt that starts there is rejected in the same step.
// - r5 (negedge clock, ticks 20, 40): h is 1 during [39, 40): the attempt
//   from 20 fails at 39; the one from 40 starts with h sampled 1 and fails.
// llg formerly read the sampled value in the first time step with a signal
// update after the condition rose (here 26, 27, 31 and 40 instead of 23,
// 26, 30 and 39).
//
// IEEE 1800-2009 16.13.14 (SystemVerilog-1800-2009.txt L25265-25267): "The
// operators accept_on and reject_on are evaluated at the granularity of the
// simulation time step like disable iff but their abort condition is
// evaluated using sampled value as a regular Boolean expression in
// assertions." The industry-standard behaviour reads the current value
// instead; llg follows it. 16.15.8 ab) (L27439-27441): accept_on is
// nonvacuous only if "expression_or_dist does not hold in any time step of
// that evaluation attempt."
`timescale 1ns / 1ns
module tb;
  bit clk = 1'b0;
  always #10 clk = ~clk;
  bit a = 1'b0, b0 = 1'b0, b1 = 1'b1, c = 1'b0, g = 1'b0, e = 1'b0, n = 1'b0,
      h = 1'b0;
  r1: assert property (@(posedge clk) reject_on (c) a |-> ##2 b0)
    $display("%0t r1 pass", $time); else $display("%0t r1 fail", $time);
  r2: assert property (@(posedge clk) reject_on (g) a |-> ##1 b1)
    $display("%0t r2 pass", $time); else $display("%0t r2 fail", $time);
  r3: assert property (@(posedge clk) accept_on (e) a |-> ##1 b0)
    $display("%0t r3 pass", $time); else $display("%0t r3 fail", $time);
  r4: assert property (@(posedge clk) reject_on (n) a |-> ##1 b1)
    $display("%0t r4 pass", $time); else $display("%0t r4 fail", $time);
  r5: assert property (@(negedge clk) reject_on (h) a |-> ##1 b1)
    $display("%0t r5 pass", $time); else $display("%0t r5 fail", $time);
  initial begin
    @(posedge clk);
    @(posedge clk);
    n <= 1'b1;
    #1 n <= 1'b0;
  end
  initial begin
    #9 a = 1'b1;
    #4 begin
      g = 1'b1;
      g = 1'b0;
    end
    #10 c = 1'b1;
    #3 e = 1'b1;
    #1 e = 1'b0;
    #12 h = 1'b1;
    #1 h = 1'b0;
    #15 $display("%0t end", $time);
    $finish;
  end
endmodule
