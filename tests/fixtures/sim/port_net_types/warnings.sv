// llg-test-fixture: R05 port net-type collapse / warnings
// IEEE 1364-2001 12.3.10 and IEEE 1800-2009 23.3.3.7.
`timescale 1ns/1ns
module and_leaf(inout wand p);
    assign p = 0;
endmodule
module or_leaf(inout wor p);
    assign p = 1;
endmodule
module down_leaf(inout tri0 p); endmodule
module up_leaf(inout tri1 p); endmodule
module ground_leaf(inout supply0 p); endmodule
module power_leaf(inout supply1 p); endmodule
module tb;
    wand wa;
    wor wo;
    tri0 down;
    tri1 up;
    supply0 ground;
    supply1 power;
    or_leaf u0(wa);
    and_leaf u1(wo);
    up_leaf u2(down);
    down_leaf u3(up);
    power_leaf u4(ground);
    ground_leaf u5(power);
    assign wa = 0;
    assign wo = 1;
    initial begin
        #1 $display("conflicts=%b%b%b%b%b%b", wa, wo, down, up, ground, power);
        $finish;
    end
endmodule
