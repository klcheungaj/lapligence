// IEEE 1364-2001 7.8-7.10, 7.14, 8.6, 17.1.1.5; IEEE 1800-2009 28.11-28.16,
// 29.8, 21.2.1.5. UDP outputs are independent drivers: competing UDPs
// resolve on wire/wand/wor/tri1 nets, drive strengths reach %v, and a delay2
// specification uses the rise, fall and smaller (to-x) delays inertially.
`timescale 1ns/1ns
primitive buf1(y, a);
    output y;
    input a;
    table
        0 : 0;
        1 : 1;
    endtable
endprimitive

module tb;
    parameter D = 2;
    reg a, b;
    wire y, yp, w, s, ww;
    wand wa;
    wor wo;
    tri1 t1;

    buf1 #(3, 5) rise_fall(y, a);
    buf1 #D single(yp, b);
    buf1 (strong0, weak1) mixed(w, a);
    buf1 (pull0, pull1) weak_side(s, a);
    buf1 (strong0, strong1) strong_side(s, b);
    buf1 w_a(ww, a);
    buf1 w_b(ww, b);
    buf1 and_a(wa, a);
    buf1 and_b(wa, b);
    buf1 or_a(wo, a);
    buf1 or_b(wo, b);
    buf1 (weak0, weak1) pulled(t1, a);

    always @(y) $display("%0t y=%b", $time, y);
    always @(yp) $display("%0t yp=%b", $time, yp);

    task show;
        $display("%0t w=%v s=%v ww=%v wa=%b wo=%b t1=%v", $time, w, s, ww, wa, wo, t1);
    endtask

    initial begin
        a = 0; b = 1;
        #1 show;
        #9 a = 1;
        #1 show;
        #9 a = 1'bx;
        #1 show;
        #9 a = 0;
        #1 show;
        #9 a = 1;
        #1 a = 0;
        #10 a = 1'bz;
        #1 show;
        #9 a = 1; b = 0;
        #1 show;
        #9 b = 1'bz;
        #1 show;
        #10 $finish(0);
    end
endmodule
