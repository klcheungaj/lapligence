// IEEE 1800-2009 6.5, 10.3, 10.9, 10.11 and 23.3: parameterized instances,
// including generated ones, combine aliases, net-array cells and pattern
// drivers that read other pattern-driven nets, across a limb boundary.
module lane #(parameter int W = 4) (
    input logic [W-1:0] x0, x1,
    output wire [W-1:0] o0, o1
);
    typedef logic [1:0][W-1:0] pair_t;
    wire [W-1:0] a, b;
    alias a = b;
    wire [W-1:0] cells[0:1];
    pair_t p;
    assign p = {x0, x1};
    assign '{a, cells[1]} = p;
    assign '{cells[0], o1} = pair_t'({b, cells[1]});
    assign o0 = cells[0];
endmodule
module tb;
    logic [3:0] x0 = 4'h1, x1 = 4'h2;
    logic [64:0] y0 = {1'b1, 64'h3}, y1 = {65{1'bz}};
    wire [3:0] o0, o1;
    wire [64:0] q0, q1;
    wire [3:0] g0[0:1], g1[0:1];
    lane #(4) l4(x0, x1, o0, o1);
    lane #(65) l65(y0, y1, q0, q1);
    for (genvar k = 0; k < 2; k++) begin : rep
        lane #(4) l(x0 + 4'(k), x1, g0[k], g1[k]);
    end
    initial begin
        #1 $display("%h %h %h %h | %h %h %h %h", o0, o1, q0, q1, g0[0], g1[0], g0[1], g1[1]);
        x0 = 4'hx;
        y1 = 65'h5;
        #1 $display("%h %h %h %h | %h %h %h %h", o0, o1, q0, q1, g0[0], g1[0], g0[1], g1[1]);
        x1 = 4'h9;
        #1 $display("%h %h %h %h | %h %h %h %h", o0, o1, q0, q1, g0[0], g1[0], g0[1], g1[1]);
        $finish(0);
    end
endmodule
