// llg-test-fixture: aliasing nets with incompatible net types must be reported
// precisely before code generation. LRM: IEEE 1800-2009 10.11.
module tb;
    wire a;
    wand b;

    alias a = b;
endmodule
