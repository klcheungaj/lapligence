// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_045/neg_wildcard_wide.sv
// SIM-045 negative: a wildcard bins value whose open bits above its lowest
// fixed bit need more than 4096 value ranges is an llg limitation.
module tb;
  bit [15:0] a;
  covergroup g;
    cp: coverpoint a { wildcard bins odd = {16'b????_????_????_???1}; }
  endgroup
  g c = new;
  initial $finish;
endmodule
