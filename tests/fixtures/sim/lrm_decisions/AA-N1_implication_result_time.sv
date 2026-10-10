// llg-test-fixture: tests/fixtures/sim/lrm_decisions/AA-N1_implication_result_time.sv
// Decision AA-N1: an implication attempt reports exactly one result. A pass
// is reported once every consequent started by a match of the antecedent
// has succeeded and the antecedent can match no more; a failure is reported
// as soon as one consequent fails (see AA-D1).
//
// IEEE 1800-2009 16.13.6 (SystemVerilog-1800-2009.txt L24617-24619):
//   "From a given start point, evaluation of the implication succeeds and
//   returns true if, and only if, for every match of the antecedent
//   sequence_expr beginning at the start point, the evaluation of the
//   consequent property_expr beginning at the end point of the match succeeds
//   and returns true."
// 16.15.3 (L26601-26606): "Number of times succeeded (maximum of one per
//   attempt) [...] The pass statement specified in statement_or_null shall be
//   executed once for each successful evaluation attempt of the underlying
//   property_spec. The pass statement shall be executed in the Reactive region
//   of the time step in which the corresponding evaluation attempt succeeds."
// The text does not say when an attempt whose consequents have all passed
// "succeeds" while its antecedent could still match; this case fixes the
// choice: at the tick the antecedent is exhausted.
//
// Ticks are numbered 1-6; `t` holds the current tick number. Each expect
// starts its single attempt on tick 1.
// - C: `a ##[1:2] b` matches at ticks 2 and 3 and `c` holds at both: one pass,
//   at tick 3.
// - D: `d ##[1:3] e` matches at tick 2 only (`e` is 0 on ticks 3-4) and `f`
//   holds at 2. The `##3` alternative is ruled out on tick 4: one pass, at
//   tick 4.
module tb;
  logic clk = 1'b0;
  logic a = 1'b0, b = 1'b0, c = 1'b0;
  logic d = 1'b0, e = 1'b0, f = 1'b0;
  int t = 0;
  // Tick:               1 2 3 4 5 6
  bit [1:6] wave_a = 6'b1_0_0_0_0_0;
  bit [1:6] wave_b = 6'b0_1_1_0_0_0;
  bit [1:6] wave_c = 6'b0_1_1_0_0_0;
  bit [1:6] wave_d = 6'b1_0_0_0_0_0;
  bit [1:6] wave_e = 6'b0_1_0_0_0_0;
  bit [1:6] wave_f = 6'b0_1_0_0_0_0;

  initial begin
    expect (@(posedge clk) (a ##[1:2] b) |-> c)
      $display("C pass t=%0d", t);
    else
      $display("C fail t=%0d", t);
  end

  initial begin
    expect (@(posedge clk) (d ##[1:3] e) |-> f)
      $display("D pass t=%0d", t);
    else
      $display("D fail t=%0d", t);
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
    $finish;
  end
endmodule
