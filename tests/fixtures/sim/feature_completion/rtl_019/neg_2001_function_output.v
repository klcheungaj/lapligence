// llg-test-fixture: tests/fixtures/sim/feature_completion/rtl_019/neg_2001_function_output.v
// IEEE 1364-2001 A.2.6, 10.3.1: function ports are inputs only.
module tb; function [3:0] f; input [3:0] a; output [3:0] b; f = a; endfunction initial $finish; endmodule
