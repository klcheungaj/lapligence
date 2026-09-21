// llg-test-fixture: SYN-019 begin_keywords cannot admit whole-array values
// IEEE 1800-2009 §22.14 changes lexical interpretation only; it cannot relax
// the selected Verilog-2001 semantic policy.
`begin_keywords "1800-2009"
module tb;
    reg [7:0] a [0:1];
    reg [7:0] b [0:1];
    initial b = a;
endmodule
`end_keywords
