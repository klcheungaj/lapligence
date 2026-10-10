// llg-test-fixture: tests/fixtures/sim/lrm_decisions/AA-D4_leading_range_empty.sv
// Decision AA-D4: a leading `##[0:n] r` keeps the empty match of `r`, exactly
// like `##0 r`.
//
// IEEE 1800-2009 F.3.4.2.2 (SystemVerilog-1800-2009.txt L70580-70581):
//   "Let m < n.
//      ( ##[m:n] R ) (1[*m:n] ##1 R )."
//   (the equivalence sign between the two forms is lost in the text
//   extraction).
// 16.7 (L21795): "##0 a // means a".
// 16.9.2 (L22497-22498): "(empty ##n seq), where n is greater than 0, is
//   equivalent to (##(n-1) seq). (seq ##n empty), where n is greater than 0,
//   is equivalent to (seq ##(n-1) `true)."
//
// `a` is always 0, so `a[*0:1]` matches only empty. `go` is 1 on tick 1.
// - `go ##1 (##0 a[*0:1])` = `go ##1 empty` = `go ##0 1`: one match, tick 1.
// - `go ##1 (##[0:1] a[*0:1])` = `go ##1 ((1[*0] ##1 a[*0:1]) or
//   (1[*1] ##1 a[*0:1]))`: the first term is `go ##1 empty` (tick 1); the
//   second is `go ##1 (1 ##1 empty)` = `go ##1 1` (tick 2). Two matches.
module tb;
  logic clk = 1'b0;
  logic go = 1'b0;
  logic a = 1'b0;
  int t = 0;
  // Matches per tick, printed at the end so the output does not depend on
  // the order of pass statements of different covers in one time step.
  int zero_hits[1:4] = '{0, 0, 0, 0};
  int range_hits[1:4] = '{0, 0, 0, 0};

  c_zero: cover sequence (@(posedge clk) go ##1 (##0 a[*0:1])) zero_hits[t]++;
  c_range: cover sequence (@(posedge clk) go ##1 (##[0:1] a[*0:1])) range_hits[t]++;

  initial begin
    for (int k = 1; k <= 4; k++) begin
      t = k;
      go = k == 1;
      #5 clk = 1'b1;
      #5 clk = 1'b0;
    end
    $display("##0 matches per tick: %0d %0d %0d %0d", zero_hits[1], zero_hits[2],
             zero_hits[3], zero_hits[4]);
    $display("##[0:1] matches per tick: %0d %0d %0d %0d", range_hits[1], range_hits[2],
             range_hits[3], range_hits[4]);
    $finish;
  end
endmodule
