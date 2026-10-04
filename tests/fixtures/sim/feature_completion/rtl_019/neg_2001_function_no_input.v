// llg-test-fixture: tests/fixtures/sim/feature_completion/rtl_019/neg_2001_function_no_input.v
// IEEE 1364-2001 10.3.1(c): a function has at least one input.
module tb; function [3:0] f; reg [3:0] r; f = 4'd1; endfunction initial $finish; endmodule
