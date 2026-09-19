// llg-test-fixture: G1-34 rtl_composition_gate.
// Parameterized hierarchy with dependent widths, generated banks of a
// multiply-accumulate cell reached through a nested module instance array, and
// a resettable always_ff accumulator fed by combinational always_comb cells.
module g1_mac #(
    parameter int W = 8,
    parameter int BIAS = 0
) (
    input  logic signed [W-1:0] a,
    input  logic signed [W-1:0] b,
    output logic signed [2*W:0] y
);
    localparam int P = 2 * W;
    logic signed [P-1:0] product;
    always_comb product = a * b;
    always_comb y = product + BIAS;
endmodule

module g1_bank #(
    parameter int W = 8
) (
    input  logic signed [4*W-1:0] av,
    input  logic signed [4*W-1:0] bv,
    output logic signed [4*(2*W+1)-1:0] yv
);
    g1_mac #(.W(W), .BIAS(1)) u_mac[3:0] (.a(av), .b(bv), .y(yv));
endmodule

module g1_choose #(
    parameter int W = 8,
    parameter int MODE = 0
) (
    input  logic [W-1:0] a,
    input  logic [W-1:0] b,
    output logic [W-1:0] y
);
    generate
        case (MODE)
            0: assign y = a + b;
            1: assign y = a ^ b;
            default: assign y = a & b;
        endcase
    endgenerate
endmodule

module tb;
    localparam int W = 8;
    localparam int DW = 2 * W + 1;
    localparam int N = 4;
    localparam int LW = $clog2(N);

    logic signed [W-1:0] av [0:N-1];
    logic signed [W-1:0] bv [0:N-1];
    logic signed [4*W-1:0] av_flat;
    logic signed [4*W-1:0] bv_flat;
    logic signed [4*DW-1:0] yv_flat;
    logic signed [DW-1:0] yv [0:N-1];

    assign av_flat = {av[3], av[2], av[1], av[0]};
    assign bv_flat = {bv[3], bv[2], bv[1], bv[0]};
    assign {yv[3], yv[2], yv[1], yv[0]} = yv_flat;

    g1_bank #(.W(W)) u_bank (.av(av_flat), .bv(bv_flat), .yv(yv_flat));

    logic clk;
    logic rst_n;
    logic signed [DW-1:0] sum_lat [0:N-1];
    logic signed [DW-1:0] accum;

    generate
        for (genvar gi = 0; gi < N; gi = gi + 1) begin : g_lane
            always_ff @(posedge clk) begin
                if (!rst_n) sum_lat[gi] <= '0;
                else sum_lat[gi] <= yv[gi];
            end
        end
    endgenerate

    always_comb accum = sum_lat[0] + sum_lat[1] + sum_lat[2] + sum_lat[3];

    logic [W-1:0] gen_if;
    generate
        if (W == 8) begin : g_wide
            assign gen_if = 8'h0f + 8'h33;
        end else begin : g_narrow
            assign gen_if = 8'h00;
        end
    endgenerate

    logic [W-1:0] add_y;
    logic [W-1:0] xor_y;
    logic [W-1:0] and_y;
    g1_choose #(.W(W), .MODE(0)) u_add (.a(8'h0f), .b(8'h33), .y(add_y));
    g1_choose #(.W(W), .MODE(1)) u_xor (.a(8'h0f), .b(8'h33), .y(xor_y));
    g1_choose #(.W(W), .MODE(2)) u_and (.a(8'h0f), .b(8'h33), .y(and_y));

    logic [7:0] u8;
    logic signed [7:0] s8;
    logic signed [15:0] mixed;

    initial begin
        clk = 0;
        rst_n = 0;
        av[0] = 8'sd1;
        av[1] = -8'sd2;
        av[2] = 8'sd3;
        av[3] = -8'sd4;
        bv[0] = 8'sd5;
        bv[1] = -8'sd6;
        bv[2] = 8'sd7;
        bv[3] = -8'sd8;
        u8 = 8'h80;
        s8 = -8'sd1;
        #1 clk = 1;
        #1 clk = 0;
        rst_n = 1;
        mixed = u8 + s8;
        #1 clk = 1;
        #1 clk = 0;
        $display("lw=%0d", LW);
        $display("yv=%0d %0d %0d %0d", yv[0], yv[1], yv[2], yv[3]);
        $display("sum=%0d %0d %0d %0d accum=%0d",
                 sum_lat[0], sum_lat[1], sum_lat[2], sum_lat[3], accum);
        $display("gen=%0d if=%0d add=%0d xor=%0d and=%0d",
                 W, gen_if, add_y, xor_y, and_y);
        $display("mixed=%0d", mixed);
        $finish(0);
    end
endmodule
