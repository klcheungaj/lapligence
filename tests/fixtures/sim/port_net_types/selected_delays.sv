// llg-test-fixture: R05 port net-type collapse / selected_delays
// IEEE 1364-2001 12.3.10 and IEEE 1800-2009 23.3.3.7.
`timescale 1ns/1ns
module no_delay(inout wand p, input wire d);
    assign p = d;
endmodule
module delayed(p, d);
    inout p;
    wor #3 p;
    input d;
    assign p = d;
endmodule
module tb;
    wire [1:0] #9 bus;
    reg d;
    no_delay u0(bus[0], d);
    delayed u1(bus[1], d);
    initial begin
        d = 1'bz;
        #1 d = 1;
        #1 $display("t2=%b", bus);
        #3 $display("t5=%b", bus);
        $finish(0);
    end
endmodule
