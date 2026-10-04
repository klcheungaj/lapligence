// llg-test-fixture: tests/fixtures/sim/feature_completion/rtl_019/neg_2001_queue.v
// IEEE 1364-2001 A.2.1.3/A.2.5: no queue dimension; nearest legal: reg [7:0] m [0:3].
module tb; integer q [$]; initial $finish; endmodule
