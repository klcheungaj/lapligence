// Value-backend workload: concurrent assertions and covers on narrow and wide
// handshakes, with implication, bounded delays, $past/$rose/$stable sampled
// functions and pass actions that update module counters.

`ifndef LLG_VB_LANES
`define LLG_VB_LANES 32
`endif

`ifndef LLG_VB_CYCLES
`define LLG_VB_CYCLES 4000
`endif

module vb_handshake #(
    parameter int SEED = 1
) (
    input logic clk,
    input logic rst,
    output int passes,
    output int covers
);
    logic req;
    logic ack;
    logic [1:0] wait_left;
    logic [31:0] data;
    logic [127:0] wide;
    logic [31:0] lfsr;

    initial begin
        passes = 0;
        covers = 0;
    end

    always_ff @(posedge clk) begin
        if (rst) begin
            req <= 1'b0;
            ack <= 1'b0;
            wait_left <= '0;
            data <= 32'(SEED);
            wide <= '0;
            lfsr <= 32'(SEED) | 32'd1;
        end else begin
            lfsr <= {lfsr[30:0], lfsr[31] ^ lfsr[21] ^ lfsr[1] ^ lfsr[0]};
            ack <= 1'b0;
            if (!req && lfsr[0]) begin
                req <= 1'b1;
                wait_left <= lfsr[2:1] % 3;
                data <= data + lfsr;
                wide <= {wide[95:0], data};
            end else if (req) begin
                if (wait_left == 0) begin
                    req <= 1'b0;
                    ack <= 1'b1;
                end else begin
                    wait_left <= wait_left - 2'd1;
                end
            end
        end
    end

    handshake: assert property (@(posedge clk) disable iff (rst) $rose(req) |-> ##[1:3] ack)
        passes++;
    stable_data: assert property (@(posedge clk) disable iff (rst) req && $past(req) |-> $stable(data))
        passes++;
    stable_wide: assert property (@(posedge clk) disable iff (rst) req |-> wide == $past(wide) || $rose(req))
        passes++;
    ack_after_req: assert property (@(posedge clk) disable iff (rst) ack |-> $past(req))
        passes++;
    quick: cover property (@(posedge clk) disable iff (rst) $rose(req) ##1 ack)
        covers++;
endmodule

module assertions #(
    parameter int LANES = `LLG_VB_LANES,
    parameter int CYCLES = `LLG_VB_CYCLES
);
    logic clk = 1'b0;
    logic rst = 1'b1;
    int passes [LANES];
    int covers [LANES];

    genvar i;
    for (i = 0; i < LANES; i = i + 1) begin : lanes
        vb_handshake #(.SEED(i * 2654435 + 7)) lane(.clk(clk), .rst(rst), .passes(passes[i]),
                                                    .covers(covers[i]));
    end

    always #1 clk = ~clk;

    initial begin
        longint pass_total;
        longint cover_total;
        repeat (3) @(posedge clk);
        rst = 1'b0;
        repeat (CYCLES) @(posedge clk);
        #1;
        pass_total = 0;
        cover_total = 0;
        for (int lane = 0; lane < LANES; lane++) begin
            pass_total += passes[lane];
            cover_total += covers[lane];
        end
        $display("assertions lanes=%0d cycles=%0d passes=%0d covers=%0d", LANES, CYCLES,
                 pass_total, cover_total);
        $finish(0);
    end
endmodule
