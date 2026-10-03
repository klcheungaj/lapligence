// RTL-011: three-level and multiway inout collapse (IEEE 1800-2009 23.3.3.7,
// Table 23-1). Parent links precede descendants; same-depth connections form
// one batch whose result does not depend on instance or port order. This twin
// permutes the module and instance declaration order of inout_chains.sv.
`timescale 1ns/1ns
module drv_or(inout wor p, input d); assign p = d; endmodule
module drv_and(inout wand p, input d); assign p = d; endmodule
module hold0(inout supply0 p); endmodule
module mid_wire(inout wire m, input d); drv_and leaf(m, d); endmodule
module mid_tri0(inout tri0 m, input d); drv_or leaf(m, d); endmodule
module tb;
    // wire / wire / wand: the leaf dominates the whole chain (wired AND).
    wire a;
    reg da, la;
    assign a = da;
    mid_wire ma(a, la);
    // wor / tri0 / wor: tri0 is dominated by its external wor (warning).
    wor b;
    reg db, lb;
    assign b = db;
    mid_tri0 mb(b, lb);
    // wire with wand and wor siblings: a warning-only tie selects wand.
    wire c;
    reg dc1, dc2;
    drv_or c_or(c, dc2);
    drv_and c_and(c, dc1);
    // depth 1 makes the net wor; the depth-2 wand leaf is then dominated.
    wire n;
    reg ln, dn;
    drv_or n_or(n, dn);
    mid_wire mn(n, ln);
    // supply0 dominates a wand sibling.
    wire e;
    reg de;
    hold0 e_hold(e);
    drv_and e_and(e, de);
    initial begin
        da = 1; la = 1; db = 0; lb = 0; dc1 = 1; dc2 = 0; ln = 0; dn = 0; de = 1;
        #1 $display("a=%b%b%b b=%b%b%b c=%b%b%b n=%b%b%b e=%b%b",
                    a, ma.m, ma.leaf.p, b, mb.m, mb.leaf.p, c, c_and.p, c_or.p,
                    n, mn.leaf.p, n_or.p, e, e_and.p);
        la = 0; db = 1; dc2 = 1; ln = 1;
        #1 $display("a=%b%b%b b=%b%b%b c=%b%b%b n=%b%b%b e=%b%b",
                    a, ma.m, ma.leaf.p, b, mb.m, mb.leaf.p, c, c_and.p, c_or.p,
                    n, mn.leaf.p, n_or.p, e, e_and.p);
        da = 0; db = 0; lb = 1; dc1 = 0; dc2 = 0; ln = 0; dn = 1; de = 0;
        #1 $display("a=%b%b%b b=%b%b%b c=%b%b%b n=%b%b%b e=%b%b",
                    a, ma.m, ma.leaf.p, b, mb.m, mb.leaf.p, c, c_and.p, c_or.p,
                    n, mn.leaf.p, n_or.p, e, e_and.p);
        $finish(0);
    end
endmodule
