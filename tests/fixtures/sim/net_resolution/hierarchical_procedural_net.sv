// llg-test-fixture: tests/fixtures/sim/net_resolution/hierarchical_procedural_net.sv
// IEEE 1800-2009 10.4.1, 23.6: a procedural assignment to a hierarchical net
// remains illegal even when the same net admits structural continuous drivers.
module child;
    wand w;
endmodule

module tb;
    child u();

    initial u.w = 1'b1;
endmodule
