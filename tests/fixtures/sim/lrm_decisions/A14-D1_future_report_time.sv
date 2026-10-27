// Decision A14-D1: a global clocking future sampled value function returns
// the sampled value at the first global clock tick strictly after the time
// step of the call, also when the assertion clock does not tick at a global
// clock tick, and the action block of an assertion calling one runs at the
// global clock tick after the last tick of the attempt, a vacuous success
// included. Global clock ticks: 5, 15, 25, 35, 45. f1 ticks at 13 and reads
// s at 15 (1). i1 (one attempt, begun at 21) ends at its tick 33, where s is
// 0 and changes to 1 by the global clock tick at 35. v1 succeeds vacuously
// at 42.
//
// IEEE 1800-2009 16.9.4 (SystemVerilog-1800-2009.txt L22876-22877): "the
// sampled value at the global clock tick that immediately precedes or follows
// the time step at which the function is called"; L22916: "$future_gclk(v)
// is the sampled value of v at the next global clocking tick."; L22961-22962:
// "Execution of the action block of an assertion containing global clocking
// future sampled value functions shall be delayed until the global clocking
// tick that follows the last tick of the assertion clock for the attempt."
`timescale 1ns / 1ns
module tb;
  logic clk = 1'b0, t1 = 1'b0, t2 = 1'b0, t3 = 1'b0, s = 1'b0;
  global clocking gc @(posedge clk);
  endclocking
  always #5 clk = ~clk;
  f1: assert property (@(posedge t1) $future_gclk(s))
    $display("%0t f1 pass", $time); else $display("%0t f1 fail", $time);
  initial begin
    i1: assert property (@(posedge t2) s |=> $changing_gclk(s))
      $display("%0t i1 pass", $time); else $display("%0t i1 fail", $time);
  end
  v1: assert property (@(posedge t3) !s |-> $future_gclk(s))
    $display("%0t v1 pass", $time); else $display("%0t v1 fail", $time);
  initial begin
    #12 s = 1'b1;
    #1 t1 = 1'b1;
    #8 t2 = 1'b1;
    #3 s = 1'b0;
    #3 t2 = 1'b0;
    #6 t2 = 1'b1;
    #1 s = 1'b1;
    #8 t3 = 1'b1;
    #10 $finish;
  end
endmodule
