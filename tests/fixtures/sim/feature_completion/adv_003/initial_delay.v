// llg-test-fixture: tests/fixtures/sim/feature_completion/adv_003/initial_delay.v
// IEEE 1364-2001 8.5 (Examples 1 and 2, Figure 30), 8.6, 7.14; IEEE
// 1800-2009 29.7, 29.8, 28.16. The initial value is the output when
// simulation begins and is not delayed by the instance delay; later changes
// take the rise, fall or (to x) smaller delay. `output reg q = v` is the
// ANSI form of the same initial value.
`timescale 1ns/1ns
primitive srff (q, s, r);
    output q; reg q;
    input s, r;
    initial q = 1'b1;
    table
    // s r : q : q+
        1 0 : ? : 1 ;
        f 0 : 1 : - ;
        0 r : ? : 0 ;
        0 f : 0 : - ;
        1 1 : ? : 0 ;
    endtable
endprimitive

primitive dff1 (q, clk, d);
    input clk, d;
    output q; reg q;
    initial q = 1'b1;
    table
    // clk d : q : q+
        r 0 : ? : 0 ;
        r 1 : ? : 1 ;
        f ? : ? : - ;
        ? * : ? : - ;
    endtable
endprimitive

primitive ansi_init (output reg q = 1'b0, input a);
    table
        r : ? : 1 ;
        f : ? : - ;
    endtable
endprimitive

module dff (q, qb, clk, d);
    input clk, d;
    output q, qb;
    dff1 g1 (qi, clk, d);
    buf #3 g2 (q, qi);
    not #5 g3 (qb, qi);
endmodule

module tb;
    reg s, r, clk, d, a, clk2;
    wire qs, q, qb, qd, qa;
    srff u_sr (qs, s, r);
    dff u_dff (q, qb, clk, d);
    dff1 #(4, 6) u_delayed (qd, clk2, d);
    ansi_init u_ansi (qa, a);

    initial begin
        #1 $display("t1 qs=%b qi=%b q=%b qb=%b qd=%b qa=%b", qs, u_dff.qi, q, qb, qd, qa);
        #1 $display("t2 q=%b qb=%b qd=%b", q, qb, qd);
        #2 $display("t4 q=%b qb=%b", q, qb);
        #2 $display("t6 q=%b qb=%b qd=%b", q, qb, qd);
        r = 1'b0;  #1 $display("sr r0 qs=%b", qs);
        s = 1'b1;  #1 $display("sr set qs=%b", qs);
        s = 1'b0;  #1 $display("sr f0 qs=%b", qs);
        r = 1'b1;  #1 $display("sr reset qs=%b", qs);
        r = 1'b0;  #1 $display("sr hold qs=%b", qs);
        a = 1'b0;  #1 $display("ansi x0 qa=%b", qa);
        a = 1'b1;  #1 $display("ansi r qa=%b", qa);
    end

    initial begin
        // x -> 1 on clk2 matches no row: X follows after min(4, 6).
        clk2 = 1'b1;
        d = 1'b0;
        #20 clk2 = 1'b0;
        #10 clk2 = 1'b1;        // t30: rise with d=0, fall delay 6
        #5 $display("t35 qd=%b", qd);
        #2 $display("t37 qd=%b", qd);
        d = 1'b1;
        #3 clk2 = 1'b0;         // t40
        #10 clk2 = 1'b1;        // t50: rise with d=1, rise delay 4
        #3 $display("t53 qd=%b", qd);
        #2 $display("t55 qd=%b", qd);
        #5 clk2 = 1'bx;         // t60: 1 -> x is unspecified: x after min(4, 6)
        #3 $display("t63 qd=%b", qd);
        #2 $display("t65 qd=%b", qd);
        $finish(0);
    end
endmodule
