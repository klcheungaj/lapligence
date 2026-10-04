// llg-test-fixture: tests/fixtures/sim/feature_completion/rtl_019/owned_error_include.sv
// IEEE 1800-2009 9.2.2.4, 22.4: a simulator semantic error inside an
// included always_ff names the header's physical position.
module tb;
  logic clk, q;
`include "owned_error.svh"
endmodule
