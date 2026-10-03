// RTL-011: a uwire collapsed through inout ports keeps one driver per bit
// (IEEE 1800-2009 6.6.2, 23.3.3.6-23.3.3.7, Table 23-1 uwire column).
`timescale 1ns/1ns
module leaf(inout wire x, input d); assign x = d; endmodule
module mid(inout wire m, input d); leaf l(m, d); endmodule
module reader(inout wire x); wire seen = x; endmodule
module bitdrv(inout wire x, input d); buf b(x, d); endmodule
module wand_child(inout wand w); endmodule
module tb;
    reg d0, d1, d2, d3;
    // Three levels: the only driver is the leaf two levels down.
    uwire deep;
    mid u_deep(deep, d0);
    // A reader-only inout child is not a driver of the parent's driver.
    uwire read_only;
    reader u_read(read_only);
    assign read_only = d1;
    // Disjoint bits: one through a port, one by a parent assignment.
    uwire [1:0] split;
    bitdrv u_bit(split[0], d2);
    assign split[1] = d3;
    // uwire dominates an internal wand with a warning and resolves as wire.
    uwire dom;
    wand_child u_wand(dom);
    assign dom = d0;
    initial begin
        d0 = 1; d1 = 0; d2 = 1; d3 = 0;
        #1 $display("%b%b %b%b %b%b %b%b", deep, u_deep.l.x, read_only, u_read.seen,
                    split, u_bit.x, dom, u_wand.w);
        d0 = 0; d1 = 1; d2 = 0; d3 = 1;
        #1 $display("%b%b %b%b %b%b %b%b", deep, u_deep.l.x, read_only, u_read.seen,
                    split, u_bit.x, dom, u_wand.w);
        $finish(0);
    end
endmodule
