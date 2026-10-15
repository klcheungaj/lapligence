// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_045/neg_trans_array_goto.sv
// SIM-045 negative: `[]` transition bins cannot use sequences of varying length (IEEE 1800-2009 19.5.1).
module tb;
  bit [3:0] a;
  covergroup g;
    cp: coverpoint a { bins t[] = (1 => 2 [-> 2]); }
  endgroup
  g c = new;
  initial $finish;
endmodule
