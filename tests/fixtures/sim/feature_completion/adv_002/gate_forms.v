// llg-test-fixture: tests/fixtures/sim/feature_completion/adv_002/gate_forms.v
// IEEE 1364-2001 7.14 Table 39; IEEE 1800-2009 28.16 Table 28-9. One, two
// and three gate delays: with two delays a change to Z or X takes the lesser
// delay; with three, a change to Z takes the third and a change to X the
// smallest. Every gate output, including each instance of a gate array,
// starts X until its first delayed update.
`timescale 1ns/1ns
module tb;
    reg [1:0] a, b;
    reg en, d;
    wire [1:0] y;
    wire n1, b3, nf;
    and #(2, 4) ga [1:0] (y, a, b);
    not #3 g1 (n1, d);
    bufif0 #(5, 7, 3) g2 (b3, d, en);
    notif1 #(5, 7, 3) g3 (nf, d, en);

    initial begin
        $monitor("%0t y=%b n1=%b b3=%b nf=%b", $time, y, n1, b3, nf);
        a = 2'b11;
        b = 2'b00;
        d = 1'b0;
        en = 1'b0;
        #10 b = 2'b11;
        #10 b = 2'b01;
        #10 en = 1'b1;
        #10 en = 1'bx;
        #10 d = 1'b1;
        en = 1'b0;
        #10 $finish(0);
    end
endmodule
