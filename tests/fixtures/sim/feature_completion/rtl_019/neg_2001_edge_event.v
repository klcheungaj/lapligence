// llg-test-fixture: tests/fixtures/sim/feature_completion/rtl_019/neg_2001_edge_event.v
// IEEE 1364-2001 A.6.5: event expressions use posedge/negedge.
module tb; reg clk; always @(edge clk) $display("x"); initial $finish; endmodule
