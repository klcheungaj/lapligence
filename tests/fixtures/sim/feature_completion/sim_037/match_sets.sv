// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_037/match_sets.sv
// SIM-037 A02: exact match sets of unequal-length branches, empty
// repetitions, intersect endpoints, first_match ties, `and`/`within`/
// `throughout` endpoints and overlapping attempts (IEEE 1800-2009
// 16.9.2-16.9.10). Every line is one match end; see readme.md.
module tb;
  logic clk = 1'b0;
  logic go = 1'b0;
  logic go3 = 1'b0;
  logic a = 1'b0;
  logic b = 1'b0;
  logic c = 1'b0;
  int t = 0;
  // Tick:              1 2 3 4 5 6 7 8
  bit [1:8] wave_a = 8'b1_1_1_0_1_1_0_0;
  bit [1:8] wave_b = 8'b0_1_1_1_0_1_1_0;
  bit [1:8] wave_c = 8'b1_0_1_1_1_0_1_0;

  m01: cover property (@(posedge clk) go ##1 ((a ##1 b) or (a ##2 c) or b) |-> 1'b1)
    $display("M01 t=%0d", t);
  m02: cover property (@(posedge clk) go ##1 ((a ##1 b) or (a ##1 a)) |-> 1'b1)
    $display("M02 t=%0d", t);
  m03: cover property (@(posedge clk) go ##1 (a[*0:2] ##1 b) |-> 1'b1)
    $display("M03 t=%0d", t);
  m04: cover property (@(posedge clk) go ##1 (c ##1 a[*0:1]) |-> 1'b1)
    $display("M04 t=%0d", t);
  m05: cover property (@(posedge clk) go ##1 (a[*0] ##0 c) |-> 1'b1)
    $display("M05 t=%0d", t);
  m06: cover property (@(posedge clk) go ##1 (a[*1:3] intersect (c ##[0:2] c)) |-> 1'b1)
    $display("M06 t=%0d", t);
  m07: cover property (@(posedge clk)
      go ##1 first_match((a ##1 b) or (a ##1 a) or (c ##2 c)) |-> 1'b1)
    $display("M07 t=%0d", t);
  m08: cover property (@(posedge clk) go ##1 ((a ##1 a) and (c ##2 c)) |-> 1'b1)
    $display("M08 t=%0d", t);
  m09: cover property (@(posedge clk) go ##1 ((b ##1 b) within (a ##[1:3] c)) |-> 1'b1)
    $display("M09 t=%0d", t);
  m10: cover property (@(posedge clk) go ##1 (a throughout (c ##[1:3] b)) |-> 1'b1)
    $display("M10 t=%0d", t);
  m11: cover property (@(posedge clk) go ##1 (a[*0:1] and (c ##2 c)) |-> 1'b1)
    $display("M11 t=%0d", t);
  m12: cover property (@(posedge clk) go3 ##0 (a ##[1:2] b) |-> 1'b1)
    $display("M12 t=%0d", t);

  initial begin
    go = 1'b1;
    for (int k = 1; k <= 9; k++) begin
      #5 clk = 1'b1;
      #5 clk = 1'b0;
      t = k;
      go = 1'b0;
      go3 = k >= 1 && k <= 3;
      a = k <= 8 ? wave_a[k] : 1'b0;
      b = k <= 8 ? wave_b[k] : 1'b0;
      c = k <= 8 ? wave_c[k] : 1'b0;
    end
    $finish;
  end
endmodule
