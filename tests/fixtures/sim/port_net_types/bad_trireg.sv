// llg-test-fixture: R05 port net-type collapse / bad_trireg
// IEEE 1364-2001 12.3.10 and IEEE 1800-2009 23.3.3.7.
`timescale 1ns/1ns
module plain(inout wire p); endmodule
module tb;
    trireg p;
    plain u0(p);
    initial #1 $finish;
endmodule
