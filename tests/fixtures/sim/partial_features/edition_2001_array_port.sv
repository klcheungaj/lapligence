// llg-test-fixture: SYN-019 Verilog-2001 rejection of unpacked array ports
// IEEE 1800-2009 §23.2.2.2; unpacked port dimensions are SystemVerilog syntax.
module child(
    input [7:0] a [0:1],
    output [7:0] b [0:1]
);
    assign b[0] = a[0];
    assign b[1] = a[1];
endmodule

module tb;
    reg [7:0] a [0:1];
    wire [7:0] b [0:1];
    child u(a, b);
    initial $finish;
endmodule
