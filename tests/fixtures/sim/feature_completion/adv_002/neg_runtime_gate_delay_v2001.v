// llg-test-fixture: tests/fixtures/sim/feature_completion/adv_002/neg_runtime_gate_delay_v2001.v
// IEEE 1364-2001 7.14.1: a gate delay is a constant expression; with
// --edition v2001 a runtime-valued gate delay is rejected.
module tb;
    reg a;
    reg [3:0] d;
    wire y;
    buf #(d) g (y, a);
    initial $finish(0);
endmodule
