// llg-test-fixture: tests/fixtures/sim/feature_completion/adv_003/instance_array.v
// IEEE 1364-2001 8.6, 7.1.5-7.1.6; IEEE 1800-2009 29.8, 28.3.5-28.3.6. Each
// element of a UDP instance array, and each separate instance of one
// definition, keeps its own state. A scalar actual (the clock) feeds every
// element; vector actuals are split one bit per element.
primitive tff (q, t);
    output q; reg q;
    input t;
    initial q = 0;
    table
    // t : q : q+
        r : 0 : 1 ;
        r : 1 : 0 ;
        f : ? : - ;
        (x?) : ? : - ;
        (?x) : ? : - ;
    endtable
endprimitive

primitive dff (q, clk, d);
    output q; reg q;
    input clk, d;
    table
        r 0 : ? : 0 ;
        r 1 : ? : 1 ;
        n ? : ? : - ;
        (0x) ? : 0 : - ;
        ? * : ? : - ;
    endtable
endprimitive

module tb;
    reg [3:0] t;
    reg clk;
    reg [3:0] d;
    wire [3:0] q, r;
    wire s0, s1;
    tff toggles[3:0] (q, t);
    tff single0 (s0, t[0]);
    tff single1 (s1, t[1]);
    dff regs[0:3] (r, clk, d);

    initial begin
        t = 4'b0000;
        clk = 1'b0;
        d = 4'b0000;
        #1 $display("start q=%b s=%b%b r=%b", q, s1, s0, r);
        t = 4'b0001;
        #1 $display("rise0 q=%b s=%b%b", q, s1, s0);
        t = 4'b0011;
        #1 $display("rise1 q=%b s=%b%b", q, s1, s0);
        t = 4'b0000;
        #1 $display("fall q=%b s=%b%b", q, s1, s0);
        t = 4'b1001;
        #1 $display("rise30 q=%b s=%b%b", q, s1, s0);
        d = 4'b1010;
        #1 clk = 1'b1;
        #1 $display("clk1 r=%b", r);
        clk = 1'b0;
        d = 4'b0110;
        #1 $display("hold r=%b", r);
        clk = 1'b1;
        #1 $display("clk2 r=%b", r);
        $finish(0);
    end
endmodule
