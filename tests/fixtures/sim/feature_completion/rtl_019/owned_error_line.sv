// llg-test-fixture: tests/fixtures/sim/feature_completion/rtl_019/owned_error_line.sv
// IEEE 1800-2009 9.2.2.4, 22.12: a simulator semantic error keeps the
// physical position and appends the `line position.
module tb;
  logic clk, q;
`line 40 "orig_rtl.sv" 0
  always_ff @(posedge clk) begin fork q <= 1'b1; join end
endmodule
