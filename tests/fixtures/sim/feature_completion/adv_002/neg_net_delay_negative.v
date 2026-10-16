// llg-test-fixture: tests/fixtures/sim/feature_completion/adv_002/neg_net_delay_negative.v
// IEEE 1364-2001 7.14; IEEE 1800-2009 28.16. A negative real net delay has no
// tick value; the net delay (not a driver delay, S2-D5) is rejected.
module tb;
    reg source;
    wire #(-1.0) bad;
    assign bad = source;
    initial $finish(0);
endmodule
