// llg-test-fixture: R05 port net-type collapse / ascending
// IEEE 1364-2001 12.3.10 and IEEE 1800-2009 23.3.3.7.
`timescale 1ns/1ns
module plain(inout wire p, input wire d);
    assign p = d;
endmodule
module single(inout wand p, input wire d);
    assign p = d;
endmodule
module pair(inout wand [1:0] p);
    assign p = 2'b01;
endmodule
module tb;
    wand external_wired;
    wire internal_wired;
    wire [0:3] split;
    wire [1:0] cells [0:1];
    logic unrelated;
    reg d;
    plain u0(external_wired, d);
    single u1(internal_wired, d);
    pair u2(split[2:3]);
    single u3(cells[0][0], d);
    assign split[0:1] = 2'b10;
    // Unconnected electrical bits and unrelated variable storage keep their
    // existing independent update paths.
    assign cells[0][1] = d;
    assign unrelated = d;
    initial begin
        d = 1;
        #1 $display("wired=%b%b split=%b cell=%b/%b", external_wired, internal_wired, split, cells[0], unrelated);
        d = 0;
        #1 $display("zero=%b%b cell=%b/%b", external_wired, internal_wired, cells[0], unrelated);
        $finish(0);
    end
endmodule
