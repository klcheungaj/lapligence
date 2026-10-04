// RTL-105: `inout uwire` formals (IEEE 1800-2009 6.6.2, 23.3.3.6-23.3.3.7).
// Neither clause forbids them; the collapsed net keeps one driver per bit,
// wherever that driver is.
`timescale 1ns/1ns
module drv(inout uwire p, input d);
    assign p = d;
endmodule
// Non-ANSI uwire formal that only reads the network.
module rd(p, q);
    inout uwire p;
    output q;
    assign q = p;
endmodule
module mid(inout uwire [1:0] m, input d);
    drv lo(m[0], d);
endmodule
module tb;
    reg d0, d1;
    // Driven only inside the child, through its uwire formal.
    wire a;
    drv u_a(a, d0);
    // Driven only by the parent; the uwire formal is a reader.
    wire b, qb;
    rd u_b(b, qb);
    assign b = d1;
    // uwire on both sides of the port.
    uwire c;
    drv u_c(c, d1);
    // Two levels: bit 0 driven two levels down, bit 1 by the parent.
    wire [1:0] e;
    mid u_e(e, d0);
    assign e[1] = d1;
    initial begin
        d0 = 0; d1 = 1;
        #1 $display("%b%b %b%b %b%b %b %b%b", a, u_a.p, b, qb, c, u_c.p, e, u_e.m, u_e.lo.p);
        d0 = 1; d1 = 0;
        #1 $display("%b%b %b%b %b%b %b %b%b", a, u_a.p, b, qb, c, u_c.p, e, u_e.m, u_e.lo.p);
        $finish(0);
    end
endmodule
