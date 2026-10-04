// llg-test-fixture: tests/fixtures/sim/feature_completion/rtl_019/neg_2001_empty_call.v
// IEEE 1364-2001 A.6.9: a task enable has no empty ( ); nearest legal: t;.
module tb; task t; $display("t"); endtask initial begin t(); $finish; end endmodule
