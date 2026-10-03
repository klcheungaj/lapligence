// llg-test-fixture: tests/fixtures/sim/syn036_capacity/cell_limit.sv
// IEEE 1364-2001 §3.10 and IEEE 1800-2009 §7.4.2: the declaration is legal;
// RTL-002 executes this former generated-model storage boundary.
module tb;
    reg [0:0] cells [0:65536];

    initial $finish(0);
endmodule
