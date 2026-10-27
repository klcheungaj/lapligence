// ADV-014 A01: the global clocking future sampled value functions (IEEE
// 1800-2009 16.9.4) return the sampled value at the next global clock tick,
// strictly after the calling time step, and the action block of an
// assertion calling them runs at that tick. Assertion clocks: the global
// clock itself, `$global_clock`, its opposite edge and a faster clock whose
// first tick precedes the first global clock tick. Ticks at or after the
// last global clock tick (55) are never reported. Expected lines: readme.md.
`timescale 1ns / 1ns
module tb;
  logic clk = 1'b0, fast = 1'b0;
  logic s = 1'b0;
  logic [3:0] v = 4'h0;
  global clocking gc @(posedge clk);
  endclocking
  always #5 clk = ~clk;
  always #2 fast = ~fast;
  initial begin
    s = 1'b1;
    #12 s = 1'b0;
    #20 s = 1'b1;
    #3 s = 1'b0;
    #12 s = 1'b1;
    #10 $finish;
  end
  initial begin
    #8 v = 4'h3;
    #10 v = 4'bx011;
    #20 v = 4'h2;
  end
  fut: assert property (@(posedge clk) $future_gclk(s))
    $display("%0t fut pass", $time); else $display("%0t fut fail", $time);
  ris: assert property (@(posedge clk) $rising_gclk(s))
    $display("%0t ris pass", $time); else $display("%0t ris fail", $time);
  fal: assert property (@(posedge clk) $falling_gclk(s))
    $display("%0t fal pass", $time); else $display("%0t fal fail", $time);
  std: assert property (@(posedge clk) $steady_gclk(s))
    $display("%0t std pass", $time); else $display("%0t std fail", $time);
  chg: assert property (@(posedge clk) $changing_gclk(s))
    $display("%0t chg pass", $time); else $display("%0t chg fail", $time);
  glb: assert property (@($global_clock) $changing_gclk(s))
    $display("%0t glb pass", $time); else $display("%0t glb fail", $time);
  neg: assert property (@(negedge clk) $future_gclk(s))
    $display("%0t neg pass", $time); else $display("%0t neg fail", $time);
  vec: assert property (@(posedge fast) $steady_gclk(v))
    $display("%0t vec pass", $time); else $display("%0t vec fail", $time);
  xfl: assert property (@(posedge clk) $falling_gclk(v[3]))
    $display("%0t xfl pass", $time); else $display("%0t xfl fail", $time);
  val: assert property (@(posedge clk) $future_gclk(v) == 4'h3 || $future_gclk(v) === 4'bx011)
    $display("%0t val pass", $time); else $display("%0t val fail", $time);
endmodule
