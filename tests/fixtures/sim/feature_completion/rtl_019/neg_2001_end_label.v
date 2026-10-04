// llg-test-fixture: tests/fixtures/sim/feature_completion/rtl_019/neg_2001_end_label.v
// IEEE 1364-2001 A.6.3: no label after end; nearest legal: begin : b ... end.
module tb; initial begin : b $finish; end : b endmodule
