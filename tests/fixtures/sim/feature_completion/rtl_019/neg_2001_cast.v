// llg-test-fixture: tests/fixtures/sim/feature_completion/rtl_019/neg_2001_cast.v
// IEEE 1364-2001 A.8.4: no cast operator; nearest legal: $signed(a).
module tb; reg [3:0] a; initial begin a = 4'd9; $display("%0d", 8'(a)); $finish; end endmodule
