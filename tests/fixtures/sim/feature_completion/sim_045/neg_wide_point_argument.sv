// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_045/neg_wide_point_argument.sv
// SIM-045 negative: bins values of a coverpoint wider than 64 bits read from
// a covergroup argument are an llg limitation.
module tb;
  bit [70:0] a;
  covergroup g(int lo);
    cp: coverpoint a { bins b = {[lo:$]}; }
  endgroup
  g c = new(1);
  initial $finish;
endmodule
