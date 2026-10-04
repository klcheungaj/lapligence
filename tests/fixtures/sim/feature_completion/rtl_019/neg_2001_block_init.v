// llg-test-fixture: tests/fixtures/sim/feature_completion/rtl_019/neg_2001_block_init.v
// IEEE 1364-2001 A.2.1.3: block variables have no initializer.
module tb; initial begin : b reg [3:0] x = 4'd1; $finish; end endmodule
