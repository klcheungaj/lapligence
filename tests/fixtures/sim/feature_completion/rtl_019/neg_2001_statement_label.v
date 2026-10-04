// llg-test-fixture: tests/fixtures/sim/feature_completion/rtl_019/neg_2001_statement_label.v
// IEEE 1364-2001 A.6.3: blocks are named after begin; nearest legal: begin : lbl.
module tb; initial lbl: begin $finish; end endmodule
