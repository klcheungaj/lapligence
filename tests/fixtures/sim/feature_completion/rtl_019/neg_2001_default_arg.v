// llg-test-fixture: tests/fixtures/sim/feature_completion/rtl_019/neg_2001_default_arg.v
// IEEE 1364-2001 A.2.7: task ports have no default value; nearest legal: task t(input [3:0] x).
module tb; task t(input [3:0] x = 4'd3); $display("%0d", x); endtask initial $finish; endmodule
