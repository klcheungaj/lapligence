// llg-test-fixture: R05 port net-type collapse / bad_alias
// IEEE 1364-2001 12.3.10 and IEEE 1800-2009 23.3.3.7.
`timescale 1ns/1ns
module plain(inout wire p); endmodule
module tb;
    wand a;
    wor b;
    alias a = b;
    plain u0(a);
    initial #1 $finish;
endmodule
