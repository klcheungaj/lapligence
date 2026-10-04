// llg-test-fixture: tests/fixtures/sim/feature_completion/rtl_019/neg_2001_time_literal.v
// IEEE 1364-2001 A.8.7: delays are numbers without units; nearest legal: #1.
module tb; initial begin #1ns $finish; end endmodule
