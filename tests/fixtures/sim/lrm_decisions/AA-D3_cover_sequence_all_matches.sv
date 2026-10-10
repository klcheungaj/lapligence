// llg-test-fixture: tests/fixtures/sim/lrm_decisions/AA-D3_cover_sequence_all_matches.sv
// Decision AA-D3: `cover sequence` runs its pass statement for every match of
// an attempt; `cover property` succeeds at most once per attempt.
//
// IEEE 1800-2009 16.15.3 (SystemVerilog-1800-2009.txt L26590-26592):
//   "The difference between the two categories is that for sequence coverage,
//   all matches per evaluation attempt are reported, whereas for property
//   coverage the coverage count is incremented at most once per evaluation
//   attempt."
// L26625-26627:
//   "all matches of the sequence_expr that complete without the occurrence of
//   the disable iff condition shall be counted, with multiplicity, toward the
//   total number of times matched for the attempt. [...] The pass statement
//   specified in statement_or_null shall be executed, with multiplicity, for
//   each match that is counted toward the total for the attempt."
//
// One attempt (go on tick 0): a on ticks 1-2, b on ticks 2-3, so
// `a ##[1:2] b` from tick 1 matches at tick 2 and at tick 3. The sequence
// cover counts both; the property cover counts one success.
module tb;
  logic clk = 1'b0;
  logic go = 1'b0;
  logic a = 1'b0;
  logic b = 1'b0;
  int t = 0;
  int seq_hits = 0;
  int prop_hits = 0;

  c_seq: cover sequence (@(posedge clk) go ##1 (a ##[1:2] b)) begin
    seq_hits++;
    $display("sequence match t=%0d", t);
  end
  c_prop: cover property (@(posedge clk) go ##1 (a ##[1:2] b)) prop_hits++;

  initial begin
    go = 1'b1;
    for (int k = 1; k <= 5; k++) begin
      #5 clk = 1'b1;
      #5 clk = 1'b0;
      t = k;
      go = 1'b0;
      a = k == 1 || k == 2;
      b = k == 2 || k == 3;
    end
    $display("sequence matches=%0d property successes=%0d", seq_hits, prop_hits);
    $finish;
  end
endmodule
