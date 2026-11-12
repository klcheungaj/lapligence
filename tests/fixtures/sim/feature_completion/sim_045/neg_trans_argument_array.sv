// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_045/neg_trans_argument_array.sv
// SIM-045 negative: `[]` transition bins whose values read a covergroup
// argument are an llg limitation (their sequences are counted at compile time).
module tb;
  bit [3:0] a;
  covergroup g(bit [3:0] n);
    cp: coverpoint a { bins t[] = (1 => n); }
  endgroup
  g c = new(4'd2);
  initial $finish;
endmodule
