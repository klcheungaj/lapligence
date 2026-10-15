// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_044/neg_strobe.sv
// SIM-044 negative: type_option.strobe sampling in the Postponed region is rejected at the option (IEEE 1800-2009 19.7.1).
module tb;
  logic clk = 0;
  bit [1:0] a;
  covergroup g @(posedge clk);
    type_option.strobe = 1;
    coverpoint a;
  endgroup
  g c = new;
  initial $finish;
endmodule
