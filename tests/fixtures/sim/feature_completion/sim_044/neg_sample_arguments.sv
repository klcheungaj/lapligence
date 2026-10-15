// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_044/neg_sample_arguments.sv
// SIM-044 negative: sample() of a with-function-sample covergroup takes exactly its declared arguments (IEEE 1800-2009 19.8.1).
module tb;
  covergroup g with function sample(bit [3:0] x);
    coverpoint x;
  endgroup
  g c = new;
  initial begin
    c.sample();
    $finish;
  end
endmodule
