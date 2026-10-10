// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_037/multiplicity_budget.sv
// SIM-037 multiplicity limits: `(a or a)[*12]` has 4096 matches on tick 12
// (16.9.7), counted by one thread. The attached match item must run once per
// match (16.11), so those paths are enumerated under LLG_SEQUENCE_THREAD_LIMIT:
// the default budget runs all 4096 calls; a budget of 50 stops the run with a
// reported error instead of dropping calls.
module tb;
  logic clk = 1'b0;
  logic go = 1'b0;
  logic a = 1'b1;
  int hits = 0;

  function automatic void note();
  endfunction

  p: cover sequence (@(posedge clk) go ##0 ((a or a)[*12], note())) hits++;

  initial begin
    for (int k = 1; k <= 14; k++) begin
      go = k == 1;
      #5 clk = 1'b1;
      #5 clk = 1'b0;
    end
    $display("done hits=%0d", hits);
    $finish;
  end
endmodule
