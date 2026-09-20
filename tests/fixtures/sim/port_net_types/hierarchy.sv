// llg-test-fixture: R05 port net-type collapse / hierarchy
// IEEE 1364-2001 12.3.10 and IEEE 1800-2009 23.3.3.7.
`timescale 1ns/1ns
module leaf(inout wand p, input wire d);
    assign p = d;
endmodule
module middle(inout wire p, input wire d, input wire inner_d);
    assign p = d;
    leaf inner(p, inner_d);
endmodule
module tb;
    wire bus;
    reg a, b, c;
    middle outer(bus, b, c);
    assign bus = a;
    initial begin
        a = 1; b = 1; c = 0;
        #1 $display("chain=%b%b%b", bus, outer.p, outer.inner.p);
        c = 1;
        #1 $display("one=%b%b%b", bus, outer.p, outer.inner.p);
        force outer.p = 0;
        #1 $display("force=%b%b%b", bus, outer.p, outer.inner.p);
        release outer.p;
        #1 $display("release=%b%b%b", bus, outer.p, outer.inner.p);
        a = 1'bz; b = 1'bz; c = 1'bz;
        #1 $display("float=%b%b%b", bus, outer.p, outer.inner.p);
        $finish(0);
    end
endmodule
