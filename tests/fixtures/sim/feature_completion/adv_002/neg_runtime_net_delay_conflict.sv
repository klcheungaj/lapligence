// llg-test-fixture: tests/fixtures/sim/feature_completion/adv_002/neg_runtime_net_delay_conflict.sv
// IEEE 1800-2009 10.11, 28.16.1. A runtime-valued net delay must be the only
// declared delay of its physical net; aliasing it to a net with another
// declared delay is rejected (S2-D8, S2-D9).
module tb;
    reg a;
    integer d;
    wire #(d) w1;
    wire #2 w2;
    alias w1 = w2;
    assign w1 = a;
    initial $finish(0);
endmodule
