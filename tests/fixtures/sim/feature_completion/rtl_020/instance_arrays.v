// IEEE 1364-2001 7.1.5, 8.6; IEEE 1800-2009 28.3.6, 29.8: an expression as
// wide as one terminal connects to every instance; a wider one gives each
// instance a part-select starting with the right-hand index. Connections
// here are whole vectors, part-selects of wider nets, concatenations,
// literals, operator expressions (numbered by their own type), a slice
// with a run-time base of a vector whose low bound is 1, and replicated
// scalars.
`timescale 1ns/1ns
primitive xor2(y, a, b);
    output y;
    input a, b;
    table
        0 0 : 0;
        0 1 : 1;
        1 0 : 1;
        1 1 : 0;
    endtable
endprimitive

module tb;
    reg [3:0] a, b;
    reg c;
    reg [8:1] e;
    integer k;
    wire [3:0] w_desc, w_asc;
    wire [7:0] wide;
    wire [1:0] w_cat, w_lit, w_expr, w_rep, w_var;

    xor2 d[3:0] (w_desc, a, b);
    xor2 u[0:3] (w_asc, a, {b[0], b[1], b[2], b[3]});
    xor2 p[1:0] (wide[5:4], a[3:2], b[1:0]);
    xor2 q[1:0] (w_cat, {a[0], c}, b[3:2]);
    xor2 r[1:0] (w_lit, a[1:0], 2'b10);
    xor2 s[1:0] (w_expr, a[1:0] & b[1:0], ~a[3:2]);
    xor2 t[1:0] (w_rep, a[1:0], c);
    xor2 v[1:0] (w_var, e[k +: 2], ~e[8:7]);

    task show(input [7:0] label);
        $display("%s d=%b u=%b wide=%b q=%b r=%b s=%b t=%b v=%b",
                 label, w_desc, w_asc, wide, w_cat, w_lit, w_expr, w_rep, w_var);
    endtask

    initial begin
        a = 4'b1100; b = 4'b1010; c = 1'b0; e = 8'b1001_0110; k = 2;
        #1 show("A");
        a = 4'b0011; b = 4'b0110; c = 1'b1; e = 8'b0110_1100; k = 5;
        #1 show("B");
        a = 4'bxz10; b = 4'b0000; c = 1'bz; k = 1;
        #1 show("C");
        $finish(0);
    end
endmodule
