// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_045/overlap_budget.sv
// SIM-045: option.detect_overlap (IEEE 1800-2009 19.7) on transition bins
// whose sequences share no complete sequence but whose lockstep search
// explores more than 2^20 steps: llg warns at compile time that it stopped
// checking (llg limitation) and the run itself warns of no overlap.
module tb;
  bit [1:0] x;
  covergroup g;
    option.detect_overlap = 1;
    option.per_instance = 1;
    cp: coverpoint x {
      bins a = (0 => 1 [-> 1000] => 2);
      bins b = (0 => 3 [-> 1000] => 2);
    }
  endgroup
  g c = new;
  initial begin
    x = 2'd0; c.sample();
    x = 2'd2; c.sample();
    $display("cp %0.2f", c.cp.get_inst_coverage());
    $finish;
  end
endmodule
