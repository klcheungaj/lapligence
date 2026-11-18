// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_045/neg_type_goal.sv
// SIM-045 negative: a constant type_option.goal outside 0 to 100.
module tb;
  bit [1:0] x;
  covergroup g;
    type_option.goal = 101;
    cp: coverpoint x;
  endgroup
  g a = new;
  initial $finish;
endmodule
