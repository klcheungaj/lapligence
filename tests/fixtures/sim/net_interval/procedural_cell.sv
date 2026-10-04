// llg-test-fixture: KI-NET-INTERVAL nearest illegal form
// IEEE 1800-2009 10.4: an undriven net-array cell is still a net and cannot
// be the target of a procedural assignment.
module tb;
    wire [3:0] free [0:3];
    initial free[1] = 4'h3;
endmodule
