// llg-test-fixture: tests/fixtures/sim/feature_completion/rtl_019/neg_2001_empty_parens.v
// IEEE 1364-2001 A.2.7: task_port_list is not empty; nearest legal: task t;.
module tb; task t(); $display("t"); endtask initial $finish; endmodule
