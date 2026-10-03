// SV 23.3.3.3, 23.3.3.5, 23.3.3.7: net-array input, output and inout ports,
// slices and rows; inout cells resolve with every connected driver.
module drv(output wire [3:0] o[4], input logic [3:0] v);
    for (genvar i = 0; i < 4; i++) begin : g
        assign o[i] = v + 4'(i);
    end
endmodule

module rd(input wire [3:0] i[4], output wire [5:0] s);
    assign s = i[0] + i[1] + i[2] + i[3];
endmodule

module src(output wire [3:0] o[0:1], input logic [3:0] v);
    assign o[0] = v;
    assign o[1] = ~v;
endmodule

module snk(input wire [3:0] i[1:0], output logic [3:0] x);
    assign x = i[1] ^ i[0];
endmodule

module side(inout wire [3:0] io[2], input logic en, input logic [3:0] v);
    assign io[0] = en ? v : 4'bz;
    assign io[1] = en ? ~v : 4'bz;
endmodule

module tb;
    wire [3:0] n[4];
    wire [5:0] s;
    logic [3:0] v = 4'd1;
    wire [3:0] sl[3:0];
    logic [3:0] x0, x1;
    wire [3:0] bus[2];
    wire [3:0] bus2[1:0][2];
    logic e0 = 1, e1 = 0;
    drv d(n, v);
    rd r(n, s);
    src s0(.o(sl[3:2]), .v(4'h6));
    src s1(.o(sl[1:0]), .v(4'h1));
    snk k0(.i(sl[3:2]), .x(x0));
    snk k1(.i(sl[1:0]), .x(x1));
    side a(bus, e0, 4'h3);
    side b(bus, e1, 4'h5);
    side c(bus2[0], 1'b1, 4'h2);
    initial begin
        #1 $display("%0d %h %h | %h %h %h %h %h %h | %h %h | %h %h %b", s, n[0], n[3],
                    sl[3], sl[2], sl[1], sl[0], x0, x1, bus[0], bus[1],
                    bus2[0][0], bus2[0][1], bus2[1][1]);
        v = 4'bx;
        e1 = 1;
        #1 $display("%b | %b %b", s, bus[0], bus[1]);
        e0 = 0;
        #1 $display("%h %h", bus[0], bus[1]);
        $finish(0);
    end
endmodule
