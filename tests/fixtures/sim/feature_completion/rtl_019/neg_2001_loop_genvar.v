// llg-test-fixture: tests/fixtures/sim/feature_completion/rtl_019/neg_2001_loop_genvar.v
// IEEE 1364-2001 A.4.2: genvar_assignment names a declared genvar.
module tb; generate for (genvar i = 0; i < 2; i = i + 1) begin : g end endgenerate initial $finish; endmodule
