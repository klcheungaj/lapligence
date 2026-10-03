// llg-test-fixture: R05/RTL-011 a uwire actual on an inout port collapses
// (IEEE 1800-2009 6.6.2, 23.3.3.6-23.3.3.7); with no driver it floats.
`timescale 1ns/1ns
module plain(inout wire p); endmodule
module tb;
    uwire p;
    plain u0(p);
    initial begin
        #1 $display("uwire=%b/%b", p, u0.p);
        $finish(0);
    end
endmodule
