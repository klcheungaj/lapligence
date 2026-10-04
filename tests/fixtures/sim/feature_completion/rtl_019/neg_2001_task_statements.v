// llg-test-fixture: tests/fixtures/sim/feature_completion/rtl_019/neg_2001_task_statements.v
// IEEE 1364-2001 A.2.7: a task body is one statement; nearest legal: begin ... end.
module tb; reg [3:0] r; task t; input [3:0] x; r = x; r = r + 1; endtask initial $finish; endmodule
