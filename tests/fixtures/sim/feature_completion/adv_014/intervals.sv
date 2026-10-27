// ADV-014 A01: the action block of an assertion with global clocking future
// sampled value functions runs at the global clock tick after the last tick
// of its attempt (IEEE 1800-2009 16.9.4), also for a vacuous success and a
// cover; a consequent on another clock; an assertion clock unrelated to a
// gated global clock, whose gated-off ticks (65, 75) are no global ticks.
// Expected lines: readme.md.
`timescale 1ns / 1ns
module tb;
  logic clk = 1'b0, slow = 1'b0, en = 1'b1;
  logic a = 1'b0, b = 1'b0, c = 1'b0;
  global clocking gc @(posedge clk iff en);
  endclocking
  always #5 clk = ~clk;
  always #20 slow = ~slow;
  initial begin
    a = 1'b1;
    #22 b = 1'b1;
    #40 en = 1'b0;
    #20 en = 1'b1;
    #68 c = 1'b1;
    #22 $finish;
  end
  imp: assert property (@(posedge slow) a |=> $steady_gclk(b))
    $display("%0t imp pass", $time); else $display("%0t imp fail", $time);
  vac: assert property (@(posedge slow) !a |-> $future_gclk(b))
    $display("%0t vac pass", $time); else $display("%0t vac fail", $time);
  cov: cover property (@(posedge slow) $rising_gclk(b))
    $display("%0t cov hit", $time);
  rng: assert property (@(posedge clk) $rose(b) |-> ##[1:2] $falling_gclk(b))
    else $display("%0t rng fail", $time);
  mcl: assert property (@(posedge slow) a |=> @(posedge clk) $rising_gclk(c))
    $display("%0t mcl pass", $time); else $display("%0t mcl fail", $time);
endmodule
