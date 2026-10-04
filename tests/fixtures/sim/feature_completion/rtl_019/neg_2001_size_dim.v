// llg-test-fixture: tests/fixtures/sim/feature_completion/rtl_019/neg_2001_size_dim.v
// IEEE 1364-2001 A.2.5: dimension ::= [ expr : expr ]; nearest legal: [0:3].
module tb; reg [7:0] m [4]; initial $finish; endmodule
