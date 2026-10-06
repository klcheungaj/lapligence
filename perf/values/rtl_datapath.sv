// Value-backend workload: clocked packed datapaths with register, net and
// operator traffic. `rtl_narrow` mixes 8/16/32/64-bit lanes; `rtl_wide`
// mixes 128/256/1024-bit lanes. Registers start X until reset releases, so
// the first cycles carry X/Z planes before the design settles to known values.

`ifndef LLG_VB_LANES
`define LLG_VB_LANES 64
`endif

`ifndef LLG_VB_CYCLES
`define LLG_VB_CYCLES 4000
`endif

module vb_lane #(
    parameter int W = 32,
    parameter int SEED = 1
) (
    input logic clk,
    input logic rst,
    output logic [63:0] digest
);
    localparam int H = W / 2;
    logic [W-1:0] state;
    logic [W-1:0] acc;
    logic [W-1:0] prev;
    logic [2:0] step;
    wire [W-1:0] rotated = {acc[H-1:0], acc[W-1:H]};
    wire [W-1:0] mixed = (state ^ rotated) + {{(W - 1){1'b0}}, 1'b1};
    wire carry_like = mixed < prev;

    always_ff @(posedge clk) begin
        if (rst) begin
            state <= W'(SEED) | W'(1);
            acc <= '0;
            prev <= '0;
            step <= '0;
        end else begin
            state <= state ^ (state << 7) ^ (state >> 3);
            prev <= mixed;
            step <= step + 3'd1;
            case (step)
                3'd0: acc <= acc + mixed;
                3'd1: acc <= acc - (mixed & ~prev);
                3'd2: acc <= acc ^ {mixed[H-1:0], prev[W-1:H]};
                3'd3: acc <= carry_like ? acc | mixed : acc & ~mixed;
                3'd4: acc <= (acc == prev) ? mixed : acc + prev;
                3'd5: acc <= acc * W'(SEED * 2 + 1);
                3'd6: acc <= {acc[W-2:0], ^acc};
                default: acc <= acc >> (mixed[3:0] + 1);
            endcase
        end
    end

    always_comb digest = 64'(acc ^ state) ^ 64'(acc[W-1 -: 8]);
endmodule

module vb_rtl #(
    parameter int LANES = `LLG_VB_LANES,
    parameter int CYCLES = `LLG_VB_CYCLES,
    parameter int W0 = 8,
    parameter int W1 = 16,
    parameter int W2 = 32,
    parameter int W3 = 64,
    parameter string NAME = "rtl"
);
    logic clk = 1'b0;
    logic rst = 1'b1;
    logic [63:0] digests [LANES];

    genvar i;
    for (i = 0; i < LANES; i = i + 1) begin : lanes
        localparam int W = (i % 4 == 0) ? W0 : (i % 4 == 1) ? W1 : (i % 4 == 2) ? W2 : W3;
        vb_lane #(.W(W), .SEED(i * 7919 + 13)) lane(.clk(clk), .rst(rst), .digest(digests[i]));
    end

    always #1 clk = ~clk;

    initial begin
        logic [63:0] checksum;
        int unknown;
        repeat (3) @(posedge clk);
        rst = 1'b0;
        repeat (CYCLES) @(posedge clk);
        #1;
        checksum = '0;
        unknown = 0;
        for (int lane = 0; lane < LANES; lane++) begin
            if ($isunknown(digests[lane])) unknown++;
            checksum = {checksum[62:0], checksum[63]} ^ digests[lane];
        end
        $display("%s lanes=%0d cycles=%0d unknown=%0d checksum=%h", NAME, LANES, CYCLES,
                 unknown, checksum);
        $finish(0);
    end
endmodule

module rtl_narrow;
    vb_rtl #(.W0(8), .W1(16), .W2(32), .W3(64), .NAME("rtl_narrow")) top();
endmodule

module rtl_wide;
    vb_rtl #(.W0(128), .W1(256), .W2(256), .W3(1024), .NAME("rtl_wide")) top();
endmodule
