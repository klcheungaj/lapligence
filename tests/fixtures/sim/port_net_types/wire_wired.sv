// llg-test-fixture: R05 port net-type collapse / wire_wired
// IEEE 1364-2001 12.3.10 and IEEE 1800-2009 23.3.3.7.
`timescale 1ns/1ns
module plain(inout wire p, input wire d);
    assign p = d;
endmodule
module and_leaf(inout wand p, input wire d);
    assign p = d;
endmodule
module or_leaf(inout wor p, input wire d);
    assign p = d;
endmodule
module tb;
    reg a, b;
    wand external_and;
    wor external_or;
    wire internal_and, internal_or;
    plain u0(external_and, a), u1(external_or, a);
    and_leaf u2(internal_and, a);
    or_leaf u3(internal_or, a);
    assign external_and = b;
    assign external_or = b;
    assign internal_and = b;
    assign internal_or = b;
    initial begin
        a = 1; b = 0;
        #1 $display("known=%b%b/%b%b", external_and, external_or, internal_and, internal_or);
        a = 1'bx;
        #1 $display("unknown=%b%b/%b%b", external_and, external_or, internal_and, internal_or);
        a = 1'bz; b = 1;
        #1 $display("one=%b%b/%b%b", external_and, external_or, internal_and, internal_or);
        b = 1'bz;
        #1 $display("released=%b%b/%b%b", external_and, external_or, internal_and, internal_or);
        $finish(0);
    end
endmodule
