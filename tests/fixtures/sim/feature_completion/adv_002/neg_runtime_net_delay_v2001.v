// llg-test-fixture: tests/fixtures/sim/feature_completion/adv_002/neg_runtime_net_delay_v2001.v
// IEEE 1364-2001 7.14.1: a net delay is a constant expression; with
// --edition v2001 a runtime-valued net delay is rejected.
module tb;
    reg a;
    reg [3:0] d;
    wire #(d) y;
    assign y = a;
    initial $finish(0);
endmodule
