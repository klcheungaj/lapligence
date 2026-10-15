// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_044/neg_strobe.sv
// SIM-044 negative: type_option.strobe is a covergroup option; setting it in a coverpoint is rejected at the option (IEEE 1800-2009 Table 19-3).
module tb;
  logic clk = 0;
  bit [1:0] a;
  covergroup g @(posedge clk);
    cp: coverpoint a {
      type_option.strobe = 1;
    }
  endgroup
  g c = new;
  initial $finish;
endmodule
