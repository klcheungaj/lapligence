// llg-test-fixture: R05 port net-type collapse / delays
// IEEE 1364-2001 12.3.10 and IEEE 1800-2009 23.3.3.7.
`timescale 1ns/1ns
module slow_plain(p, d);
    inout p;
    wire #9 p;
    input d;
    assign p = d;
endmodule
module delayed_and(p, d);
    inout p;
    wand #(3,5,2) p;
    input d;
    assign p = d;
endmodule
module plain_and(inout wand p, input wire d);
    assign p = d;
endmodule
module tb;
    reg d;
    wand #(3,5,2) external_delay;
    wire #9 internal_delay;
    wand external_zero;
    wire #9 internal_zero;
    slow_plain u0(external_delay, d), u2(external_zero, d);
    delayed_and u1(internal_delay, d);
    plain_and u3(internal_zero, d);
    initial begin
        d = 1'bz;
        #1 d = 1;
        #2 $display("t3=%b%b/%b%b", external_delay, internal_delay, external_zero, internal_zero);
        #2 $display("t5=%b%b/%b%b", external_delay, internal_delay, external_zero, internal_zero);
        #1 d = 0;
        #4 $display("t10=%b%b/%b%b", external_delay, internal_delay, external_zero, internal_zero);
        #2 $display("t12=%b%b/%b%b", external_delay, internal_delay, external_zero, internal_zero);
        #1 d = 1'bz;
        #1 $display("t14=%b%b/%b%b", external_delay, internal_delay, external_zero, internal_zero);
        #2 $display("t16=%b%b/%b%b", external_delay, internal_delay, external_zero, internal_zero);
        $finish(0);
    end
endmodule
