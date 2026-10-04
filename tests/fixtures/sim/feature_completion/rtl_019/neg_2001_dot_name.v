// llg-test-fixture: tests/fixtures/sim/feature_completion/rtl_019/neg_2001_dot_name.v
// IEEE 1364-2001 A.4.1.1: named_port_connection needs ( ); nearest legal: .a(a).
module leaf (a); input a; endmodule
module tb; wire a; leaf u (.a); initial $finish; endmodule
