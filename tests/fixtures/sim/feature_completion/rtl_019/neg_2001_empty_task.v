// llg-test-fixture: tests/fixtures/sim/feature_completion/rtl_019/neg_2001_empty_task.v
// IEEE 1364-2001 A.2.7: a task body is a statement, not empty.
module tb; task t; input x; endtask initial $finish; endmodule
