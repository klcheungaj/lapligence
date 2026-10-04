// llg-test-fixture: KI-NET-INTERVAL UDP driving one bit of a 200,000-cell array
// IEEE 1800-2009 29: the other 1,599,999 bits are undriven wire bits.
`timescale 1ns/1ns
primitive inv(output y, input a);
    table
        0 : 1;
        1 : 0;
    endtable
endprimitive
module tb;
    wire [7:0] n [0:199999];
    logic a;
    integer i;
    inv g(n[5][3], a);
    initial begin
        a = 0; i = 199999;
        #1 $display("%b %b %b", n[5], n[0], n[i]);
        a = 1;
        #1 $display("%b %b", n[5], n[4]);
        a = 1'bx;
        #1 $display("%b", n[5]);
        $finish(0);
    end
endmodule
