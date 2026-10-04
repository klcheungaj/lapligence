// llg-test-fixture: tests/fixtures/sim/feature_completion/rtl_019/neg_2001_unnamed_decl.v
// IEEE 1364-2001 A.6.3: declarations follow begin : name only.
module tb; initial begin reg [3:0] x; x = 4'd1; $finish; end endmodule
