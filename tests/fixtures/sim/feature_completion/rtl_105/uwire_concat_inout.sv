// RTL-105: uwire nets concatenated on an inout port (IEEE 1800-2009 6.6.2,
// 23.3.3.6-23.3.3.7). The port collapses each uwire with its formal bit; it
// is not a second driver. One driver per bit, on either side of the port.
`timescale 1ns/1ns
module none(inout wire [1:0] p);
endmodule
module hi(inout wire [1:0] p, input d);
    assign p[1] = d;
endmodule
module both(inout uwire [2:0] p, input [2:0] d);
    assign p = d;
endmodule
module tb;
    reg d0, d1;
    reg [2:0] d3;
    // Probe p06: the child drives nothing; the parent drives both nets.
    uwire a, b;
    assign a = 1'b1;
    assign b = 1'b0;
    none u_none({a, b});
    // The child drives the upper bit, the parent the lower one.
    uwire c, f;
    hi u_hi({c, f}, d0);
    assign f = d1;
    // A uwire and a plain wire; the wire may keep a second driver.
    uwire g;
    wire w;
    hi u_mix({g, w}, d1);
    assign w = d0;
    assign w = 1'bz;
    // Concatenation of a selected uwire bit, a uwire and a wire into a uwire formal.
    uwire [1:0] h;
    uwire k;
    wire m;
    both u_both({h[0], k, m}, d3);
    assign h[1] = d0;
    initial begin
        d0 = 0; d1 = 1; d3 = 3'b101;
        #1 $display("%b%b %b%b%b %b%b %b%b%b %b", a, b, c, f, u_hi.p[1], g, w, h, k, m, u_both.p);
        d0 = 1; d1 = 0; d3 = 3'b010;
        #1 $display("%b%b %b%b%b %b%b %b%b%b %b", a, b, c, f, u_hi.p[1], g, w, h, k, m, u_both.p);
        $finish(0);
    end
endmodule
