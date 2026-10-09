// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_037/multiplicity_overflow.sv
// SIM-037 multiplicity limits: `(a or a)[*70]` would have 2^70 matches
// (16.9.7). The count does not fit in 64 bits, so the run stops with a
// reported overflow error instead of a wrong count.
module tb;
  logic clk = 1'b0;
  logic go = 1'b0;
  logic a = 1'b1;
  int hits = 0;

  p: cover sequence (@(posedge clk) go ##0 (a or a)[*70]) hits++;

  initial begin
    for (int k = 1; k <= 72; k++) begin
      go = k == 1;
      #5 clk = 1'b1;
      #5 clk = 1'b0;
    end
    $display("done hits=%0d", hits);
    $finish;
  end
endmodule
