// llg-test-fixture: R05 port net-type collapse / bad_uwire
// IEEE 1364-2001 12.3.10 and IEEE 1800-2009 6.6.2, 23.3.3.7: the uwire
// formal collapses with `p`, which then has two drivers.
`timescale 1ns/1ns
module single(inout uwire p);
    assign p = 0;
endmodule
module tb;
    wire p;
    single u0(p);
    assign p = 1;
    initial #1 $finish;
endmodule
