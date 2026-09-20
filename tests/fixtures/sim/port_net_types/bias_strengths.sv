// llg-test-fixture: R05 port net-type collapse / bias_strengths
// IEEE 1364-2001 12.3.10 and IEEE 1800-2009 23.3.3.7.
`timescale 1ns/1ns
module weak_plain(inout wire p, input wire d);
    assign (weak1, weak0) p = d;
endmodule
module strong_plain(inout wire p, input wire d);
    assign p = d;
endmodule
module up_leaf(inout tri1 p); endmodule
module down_leaf(inout tri0 p); endmodule
module power_leaf(inout supply1 p); endmodule
module tb;
    reg d;
    tri0 weak_down;
    tri1 weak_up, strong_up;
    supply0 ground;
    supply1 power;
    wire internal_up, internal_down, internal_supply;
    weak_plain u0(weak_down, d), u1(weak_up, d);
    strong_plain u2(strong_up, d), u3(ground, d), u4(power, d);
    up_leaf u5(internal_up);
    down_leaf u6(internal_down);
    power_leaf u7(internal_supply);
    initial begin
        d = 0;
        #1 $display("zero=%b%b%b%b%b/%b%b%b", weak_down, weak_up, strong_up, ground, power,
                    internal_down, internal_up, internal_supply);
        d = 1;
        #1 $display("one=%b%b%b%b%b", weak_down, weak_up, strong_up, ground, power);
        d = 1'bx;
        #1 $display("unknown=%b%b%b%b%b", weak_down, weak_up, strong_up, ground, power);
        d = 1'bz;
        #1 $display("float=%b%b%b%b%b", weak_down, weak_up, strong_up, ground, power);
        $finish(0);
    end
endmodule
