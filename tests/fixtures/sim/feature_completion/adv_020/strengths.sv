// llg-test-fixture: ADV-020 net strengths. strengths.c ($st_read,
// $st_watch) reads vpiStrengthVal values of nets with drive strengths and
// registers cbValueChange records with vpiStrengthVal, which run on
// strength-only changes too (IEEE 1800-2009 38.15, 38.36.1). Expected
// output: strengths.out.
`timescale 1ns/1ns
module tb;
    reg en = 1'bx, a = 0;
    reg en1 = 1, en2 = 1, d = 1;
    wire w1, w2, w3, w4, w5, w6, w7, w8, w10, w11, w12;
    supply1 w9;
    bufif1 (weak0, weak1) g1 (w1, 1'b1, 1'b1);
    assign (pull0, pull1) w2 = 1'b0;
    assign (strong0, strong1) w3 = 1'b1;
    assign (weak0, weak1) w3 = 1'b0;
    assign (strong0, strong1) w4 = 1'b1;
    assign (strong0, strong1) w4 = 1'b0;
    assign (pull0, pull1) w5 = 1'b1;
    assign (weak0, weak1) w5 = 1'bx;
    bufif1 g6 (w6, 1'b0, en);
    bufif1 g7 (w7, 1'b1, en);
    pullup (w10);
    assign (weak0, weak1) w11 = a;
    assign (pull0, weak1) w12 = 1'bx;
    // w: strong and weak drivers of the same value; v[1] the same, v[0]
    // one strong driver.
    wire w;
    bufif1 (strong0, strong1) s1 (w, d, en1);
    bufif1 (weak0, weak1) s2 (w, d, en2);
    wire [1:0] v;
    bufif1 (strong0, strong1) v1 (v[1], d, en1);
    bufif1 (weak0, weak1) v2 (v[1], d, en2);
    assign v[0] = d;
    initial begin
        #1 $st_read;
        $st_watch;
        #1 en1 = 0;
        #1 en1 = 1;
        #1 en2 = 0;
        #1 d = 0;
        #1 en1 = 0;
        #1 $display("end w=%v v=%v %v", w, v[1], v[0]);
        $finish;
    end
endmodule
