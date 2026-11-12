// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_045/neg_wide_enum.sv
// SIM-045 negative: an enum coverpoint wider than 64 bits is an llg
// limitation.
module tb;
  typedef enum bit [69:0] {A, B} e_t;
  e_t e;
  covergroup g;
    cp: coverpoint e;
  endgroup
  g c = new;
  initial $finish;
endmodule
