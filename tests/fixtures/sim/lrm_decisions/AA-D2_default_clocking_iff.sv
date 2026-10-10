// llg-test-fixture: tests/fixtures/sim/lrm_decisions/AA-D2_default_clocking_iff.sv
// Decision AA-D2: an assertion that inherits `default clocking
// @(posedge clk iff en)` is clocked only by the edges where `en` is true;
// edges with `en` false are no clock ticks at all, also for the later ticks
// of a multi-tick sequence.
//
// IEEE 1800-2009 16.17 a) (SystemVerilog-1800-2009.txt L27646-27648):
//   "a concurrent assertion statement that has no otherwise specified leading
//   clocking event is treated as though the default clocking event had been
//   written explicitly as the leading clocking event."
// 9.4.2.3 (L11987-11988): "The event expression only triggers if the
//   expression after the iff is true, in this case when enable is equal to 1.
//   This type of expression is evaluated when a changes and not when enable
//   changes."
// 16.5 (L21496-21497): "The current value of the variable is used in the
//   clock expression, while the sampled value of the variable is used within
//   the assertion."
//
// Ticks 1-6 of `clk`; `en` is 1 on ticks 1, 3, 4 and 6, so the clocking event
// occurs 4 times. `a` is 1 on ticks 1 and 4, `b` on ticks 3 and 5.
// - `a ##1 b` from tick 1: the next clocking event is tick 3 (b = 1): match.
// - From tick 4: the next clocking event is tick 6 (b = 0): no match. (Tick
//   5, where b = 1, has en = 0 and is no clocking event.)
module tb;
  logic clk = 1'b0;
  logic en = 1'b0;
  logic a = 1'b0, b = 1'b0;
  int t = 0;
  int ticks = 0;
  // Tick:                1 2 3 4 5 6
  bit [1:6] wave_en = 6'b1_0_1_1_0_1;
  bit [1:6] wave_a = 6'b1_0_0_1_0_0;
  bit [1:6] wave_b = 6'b0_0_1_0_1_0;

  default clocking dc @(posedge clk iff en);
  endclocking

  c_tick: cover property (1'b1) ticks++;
  c_seq: cover sequence (a ##1 b) $display("a ##1 b matched t=%0d", t);

  initial begin
    for (int k = 1; k <= 6; k++) begin
      t = k;
      en = wave_en[k];
      a = wave_a[k];
      b = wave_b[k];
      #5 clk = 1'b1;
      #5 clk = 1'b0;
    end
    $display("clocking events=%0d", ticks);
    $finish;
  end
endmodule
