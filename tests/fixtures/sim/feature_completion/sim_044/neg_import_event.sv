// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_044/neg_import_event.sv
// SIM-044 negative: a block event on an imported function, which has no body to sample in, is rejected at the function (IEEE 1800-2009 19.3).
module tb;
  int n;
  import "DPI-C" function int c_value();
  covergroup g @@(begin c_value);
    coverpoint n;
  endgroup
  g c = new;
  initial $finish;
endmodule
