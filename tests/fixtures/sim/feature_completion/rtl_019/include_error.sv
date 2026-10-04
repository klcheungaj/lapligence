// llg-test-fixture: tests/fixtures/sim/feature_completion/rtl_019/include_error.sv
// IEEE 1800-2009 22.4: an error in an included file names that file and its
// physical line, not the including file, in both editions.
module tb;
`include "include_error.svh"
  initial $finish;
endmodule
