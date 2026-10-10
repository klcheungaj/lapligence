// llg-test-fixture: tests/fixtures/sim/lrm_decisions/AA-D1_implication_one_result.sv
// Decision AA-D1: an implication whose antecedent matches several times is
// one evaluation attempt with one result. It fails once, as soon as one
// consequent fails, and its pass statement does not run for that attempt.
//
// IEEE 1800-2009 16.13.6 (SystemVerilog-1800-2009.txt L24614-24619):
//   "For each successful match of the antecedent sequence_expr, the consequent
//   property_expr is separately evaluated. [...] From a given start point,
//   evaluation of the implication succeeds and returns true if, and only if,
//   for every match of the antecedent sequence_expr beginning at the start
//   point, the evaluation of the consequent property_expr beginning at the end
//   point of the match succeeds and returns true."
// 16.18 (L27950-27951): "An expect statement causes the executing process to
//   block until the given property succeeds or fails."
//
// Ticks are numbered 1-6; `t` holds the current tick number. Both expect
// statements start their single attempt on tick 1.
// - A: `a ##[1:2] b` matches at ticks 2 and 3; `c` holds at 2, not at 3. The
//   attempt fails once, at tick 3.
// - B: `d ##[1:3] e` matches at ticks 2, 3 and 4; `f` is false at 2. The
//   attempt fails at tick 2 without waiting for the later matches.
// - The concurrent assert sees the same attempt as A on tick 1 (fails once);
//   its other attempts start where `a` is 0 and never fail.
module tb;
  logic clk = 1'b0;
  logic a = 1'b0, b = 1'b0, c = 1'b0;
  logic d = 1'b0, e = 1'b0, f = 1'b0;
  int t = 0;
  int failures = 0;
  // Tick:               1 2 3 4 5 6
  bit [1:6] wave_a = 6'b1_0_0_0_0_0;
  bit [1:6] wave_b = 6'b0_1_1_0_0_0;
  bit [1:6] wave_c = 6'b0_1_0_0_0_0;
  bit [1:6] wave_d = 6'b1_0_0_0_0_0;
  bit [1:6] wave_e = 6'b0_1_1_1_0_0;
  bit [1:6] wave_f = 6'b0_0_0_0_0_0;

  p_a: assert property (@(posedge clk) (a ##[1:2] b) |-> c) else failures++;

  initial begin
    expect (@(posedge clk) (a ##[1:2] b) |-> c)
      $display("A pass t=%0d", t);
    else
      $display("A fail t=%0d", t);
  end

  initial begin
    expect (@(posedge clk) (d ##[1:3] e) |-> f)
      $display("B pass t=%0d", t);
    else
      $display("B fail t=%0d", t);
  end

  initial begin
    for (int k = 1; k <= 6; k++) begin
      t = k;
      a = wave_a[k];
      b = wave_b[k];
      c = wave_c[k];
      d = wave_d[k];
      e = wave_e[k];
      f = wave_f[k];
      #5 clk = 1'b1;
      #5 clk = 1'b0;
    end
    $display("assert failures=%0d", failures);
    $finish;
  end
endmodule
