// llg-test-fixture: tests/fixtures/sim/feature_completion/rtl_019/neg_2001_packed_dims.v
// IEEE 1364-2001 A.2.5: one packed range per declaration; nearest legal: reg [7:0] m.
module tb; reg [1:0][3:0] m; initial $finish; endmodule
