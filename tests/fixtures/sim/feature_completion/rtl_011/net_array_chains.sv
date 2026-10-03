// RTL-011: fixed net-array rows and cells collapse through three levels of
// inout ports with dissimilar types (IEEE 1800-2009 23.3.3.5, 23.3.3.7), and
// a true alias of one cell crosses the port projection (10.11).
`timescale 1ns/1ns
module leaf(inout wor [1:0] q, input [1:0] d); assign q = d; endmodule
module mid(inout wand [1:0] r [0:1], input [1:0] d0, input [1:0] d1);
    leaf l0(r[0], d0);
    leaf l1(r[1], d1);
    assign r[1] = 2'b10;
endmodule
module row(inout wire [3:0] c [0:1]); assign c[1] = 4'h5; endmodule
module rows(inout wire [3:0] m [0:1]); row inner(m); endmodule
module tb;
    wire [1:0] n [0:1];
    wire [3:0] grid [0:2][0:1];
    wire [1:0] tap;
    reg [1:0] d, l0, l1;
    reg [3:0] g0;
    reg [1:0] tsrc;
    assign n[0] = d;
    mid u(n, l0, l1);
    rows r(grid[1]);
    assign grid[1][0] = g0;
    alias tap = grid[1][1][2:1];
    assign tap = tsrc;
    initial begin
        d = 2'b11; l0 = 2'b01; l1 = 2'b11; g0 = 4'h9; tsrc = 2'b10;
        #1 $display("%b %b %b %b %h %b %b %h %b", n[0], n[1], u.l0.q, u.l1.q,
                    grid[1][0], grid[1][1], r.inner.c[1], grid[0][1], tap);
        d = 2'b10; l0 = 2'b11; l1 = 2'b01; g0 = 4'h6; tsrc = 2'b01;
        #1 $display("%b %b %b %b %h %b %b %h %b", n[0], n[1], u.l0.q, u.l1.q,
                    grid[1][0], grid[1][1], r.inner.c[1], grid[0][1], tap);
        $finish(0);
    end
endmodule
