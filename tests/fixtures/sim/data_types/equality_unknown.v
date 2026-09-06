`timescale 1ns/1ps
// IEEE 1800-2009 11.4.5: only an ambiguous logical comparison yields X.
// A known differing bit makes equality false even if another bit is X/Z.
module tb;
    parameter WIDTH = 128;
    reg [WIDTH-1:0] a;
    reg [WIDTH-1:0] b;
    initial begin
        a = 0;
        b = 0;
        a[WIDTH-1] = 1'bx;
        a[0] = 1'b1;
        #1;
        $display("high-x eq=%b ne=%b case=%b ncase=%b", a == b, a != b,
                 a === b, a !== b);
        a = 0;
        a[0] = 1'bx;
        a[WIDTH-1] = 1'b1;
        #1;
        $display("low-x eq=%b ne=%b case=%b ncase=%b", a == b, a != b,
                 a === b, a !== b);
        a[0] = 1'bz;
        #1;
        $display("low-z eq=%b ne=%b case=%b ncase=%b", a == b, a != b,
                 a === b, a !== b);
        $finish;
    end
endmodule
