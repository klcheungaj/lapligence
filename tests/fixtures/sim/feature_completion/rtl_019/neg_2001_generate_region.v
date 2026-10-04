// llg-test-fixture: tests/fixtures/sim/feature_completion/rtl_019/neg_2001_generate_region.v
// IEEE 1364-2001 12.1.3: generate constructs appear in generate ... endgenerate.
module tb; genvar i; for (i = 0; i < 2; i = i + 1) begin : g wire w; end initial $finish; endmodule
