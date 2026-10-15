// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_044/null_sample.sv
// SIM-044 negative: sampling or querying a covergroup handle that was never constructed is a null handle access at run time (IEEE 1800-2009 19.4, 8.4).
module tb;
  bit [1:0] a;
  covergroup g;
    coverpoint a;
  endgroup
  g c;
  initial begin
    $display("before");
    c.sample();
    $display("after");
    $finish;
  end
endmodule
