// RTL-011: true aliases across nested fixed projections keep physical bit
// pairing (IEEE 1800-2009 10.11, A.8.3/A.8.5 constant_select): ascending
// declarations with +: and -:, packed arrays, net-array cells, packed members
// of net-array cells and members of an unpacked-structure net array.
`timescale 1ns/1ns
typedef struct packed { logic [1:0] hi; logic [1:0] lo; } pair_t;
typedef struct { logic [7:0] lane; logic flag; } record_t;
module tb;
    wire [0:3] g [0:1][1:0];
    wire [1:0][0:3] pa;
    wire pair_t na [0:1];
    wire [7:0] t;
    wire record_t rec [1:0];
    wire [3:0] mirror;
    reg [7:0] src;
    reg [3:0] msrc;
    alias g[1][0][1 +: 2] = t[7:6];
    alias pa[0][3 -: 2] = t[5:4];
    alias na[1].hi = t[3:2];
    alias pa[1][0:1] = na[0].lo;
    alias rec[0].lane[3:0] = mirror;
    alias rec[1].lane[7:6] = na[1].lo;
    assign t = src;
    assign na[0] = 4'b0110;
    assign mirror = msrc;
    assign rec[1].lane[1:0] = 2'b10;
    assign na[1].lo = src[1:0];
    initial begin
        src = 8'b10_01_11_00; msrc = 4'ha;
        #1 $display("%b %b %b %b %b %h %b", g[1][0], pa, na[1], na[0], t,
                    rec[0].lane, rec[1].lane);
        src = 8'b01_10_00_11; msrc = 4'h5;
        #1 $display("%b %b %b %b %b %h %b", g[1][0], pa, na[1], na[0], t,
                    rec[0].lane, rec[1].lane);
        $finish(0);
    end
endmodule
