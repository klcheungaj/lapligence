// llg-test-fixture: tests/fixtures/sim/net_resolution/invalid_alias_width.sv
// LRM: IEEE 1800-2009 10.11.
module tb;
    wire [3:0] a;
    wire [1:0] b;
    alias a = b;
endmodule
