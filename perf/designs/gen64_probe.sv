// Generated large-scale design: N instances of an 8-bit counter + comb tree.
module perf_blk #(parameter W = 8) (
    input  logic clk,
    input  logic rst_n,
    output logic [W-1:0] cnt,
    output logic [W-1:0] sum,
    output logic parity
);
    wire [W-1:0] cnt_next;

    assign cnt_next = cnt + 8'd1;
    assign sum = {cnt_next[W-2:0], cnt_next[W-1]} ^ 8'hA5;
    assign parity = ^cnt_next;

    always @(posedge clk or negedge rst_n) begin
        if (!rst_n) cnt <= 8'd0;
        else cnt <= cnt_next;
    end
endmodule

module tb_gen64;
    reg clk, rst_n;
    wire [7:0] cnt [0:15];
    wire [7:0] sum [0:15];
    wire parity [0:15];
    genvar i;
    for (i = 0; i < 64; i = i + 1) begin : g
        perf_blk #(.W(8)) u(.clk(clk), .rst_n(rst_n), .cnt(cnt[i]), .sum(sum[i]), .parity(parity[i]));
    end

    always #5 clk = ~clk;

    initial begin
        clk = 0; rst_n = 1;
        #1 rst_n = 0;
        #3 rst_n = 1;
        #12 $display("t=64 cnt0=%0d sum0=%0d parity0=%b", cnt[0], sum[0], parity[0]);
        #10 $display("cnt59=%0d cnt62=%0d cnt63=%0d", cnt[63], sum[63], parity[63]);
        $finish;
    end
endmodule