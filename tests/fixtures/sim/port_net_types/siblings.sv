// llg-test-fixture: R05 port net-type collapse / siblings
// IEEE 1364-2001 12.3.10 and IEEE 1800-2009 23.3.3.7.
`timescale 1ns/1ns
module and_leaf(inout wand p); assign p = 0; endmodule
module or_leaf(inout wor p); assign p = 1; endmodule
module tb;
    wire p;
    and_leaf first(p);
    or_leaf second(p);
    initial begin
        #1 $display("siblings=%b%b%b", p, first.p, second.p);
        $finish;
    end
endmodule
