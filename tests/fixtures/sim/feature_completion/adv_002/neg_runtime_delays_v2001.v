// llg-test-fixture: tests/fixtures/sim/feature_completion/adv_002/neg_runtime_delays_v2001.v
// IEEE 1364-2001 7.14.1: delays on gates, nets and continuous assignments are
// "constant expressions separated by colons". With --edition v2001 a
// runtime-valued continuous-assignment delay is rejected; SystemVerilog
// accepts it (S2-D9).
module tb;
    reg a;
    reg [3:0] d;
    wire y;
    assign #(d) y = a;
    initial $finish(0);
endmodule
