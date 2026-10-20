// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_045/transition_illegal.sv
// SIM-045: an illegal default sequence reports each sample the default
// sequence would count and the run continues (decision S45-D10); an
// explicit illegal transition is a run-time error that ends the simulation
// and removes every covered sequence that contains it (IEEE 1800-2009
// 19.5.1, 19.5.5, decision S45-D3). The final procedures run and the run
// exits with a failure status.
module tb;
  bit [3:0] v;
  covergroup g;
    cp: coverpoint v {
      bins up = (1 => 2 => 3);
      bins pair = (5 => 6);
      illegal_bins bad = (2 => 3);
      illegal_bins rest = default sequence;
    }
  endgroup
  g c = new;
  bit [3:0] values[7] = '{5, 6, 9, 9, 1, 2, 3};
  final $display("final coverage %0.2f", c.get_inst_coverage());
  initial begin
    foreach (values[i]) begin
      v = values[i];
      $display("sample %0d: %0d", i + 1, v);
      c.sample();
    end
    $display("coverage %0.2f", c.get_inst_coverage());
    $finish;
  end
endmodule
