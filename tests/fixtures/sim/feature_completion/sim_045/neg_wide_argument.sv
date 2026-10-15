// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_045/neg_wide_argument.sv
// SIM-045 negative: bins values read from covergroup arguments are limited to 64 bits (llg limitation).
module tb;
  bit [3:0] a;
  covergroup g(bit [79:0] lo);
    cp: coverpoint a { bins b = {[lo:$]}; }
  endgroup
  g c = new(1);
  initial $finish;
endmodule
