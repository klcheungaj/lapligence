// RTL-011: generated, parameterized 65-bit wand children collapse onto wire
// net-array cells (IEEE 1800-2009 23.3.3.5, 23.3.3.7), and a packed member
// of a wider structure net aliases one collapsed cell (10.11).
`timescale 1ns/1ns
typedef struct packed { logic [64:0] hi; logic [2:0] lo; } wide_t;
module drv #(parameter int W = 4) (inout wand [W-1:0] p, input [W-1:0] d);
    assign p = d;
endmodule
module tb;
    localparam int W = 65;
    wire [W-1:0] bus [0:2];
    reg [W-1:0] src [0:2];
    wire wide_t ws;
    alias ws.hi = bus[1];
    assign ws.lo = 3'b101;
    for (genvar i = 0; i < 3; i++) begin : g
        drv #(W) u(bus[i], src[i]);
        assign bus[i] = {W{1'b1}} >> i;
    end
    initial begin
        src[0] = 65'h1_0000_0000_0000_0001;
        src[1] = 65'h1_8000_0000_0000_0003;
        src[2] = 65'h1_c000_0000_0000_0007;
        #1 $display("%h %h %h %h %h", bus[0], bus[1], bus[2], ws, g[1].u.p);
        src[1] = 65'h0_0000_0000_0000_0000;
        #1 $display("%h %h", bus[1], ws);
        $finish(0);
    end
endmodule
