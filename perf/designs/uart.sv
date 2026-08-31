// Copied verbatim from tests/sim_stress.rs `stress_uart_shift_baud_ps`
// (the 1-bit part-select variant `shreg[0:0]`).
module uart_tx #(parameter DIV = 4) (
    input  logic clk,
    input  logic rst_n,
    input  logic tx_start,
    input  logic [7:0] tx_data,
    output logic tx,
    output logic tx_busy
);
    reg [3:0] baud_cnt;
    reg [2:0] bit_cnt;
    reg [7:0] shreg;
    reg tx_line;
    reg busy;

    always @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            baud_cnt <= 4'd0;
            bit_cnt  <= 3'd0;
            shreg    <= 8'd0;
            tx_line  <= 1'b1;
            busy     <= 1'b0;
        end else if (tx_start && !busy) begin
            busy     <= 1'b1;
            bit_cnt  <= 3'd0;
            baud_cnt <= 4'd0;
            shreg    <= tx_data;
            tx_line  <= 1'b0;              // start bit
        end else if (busy) begin
            if (baud_cnt == DIV - 1) begin // baud tick
                baud_cnt <= 4'd0;
                tx_line  <= shreg[0:0];    // 1-bit part-select (works)
                shreg    <= {1'b1, shreg[7:1]};
                if (bit_cnt == 3'd7) begin
                    busy    <= 1'b0;
                    bit_cnt <= 3'd0;
                end else begin
                    bit_cnt <= bit_cnt + 3'd1;
                end
            end else begin
                baud_cnt <= baud_cnt + 4'd1;
            end
        end else begin
            baud_cnt <= 4'd0;
        end
    end

    assign tx = tx_line;
    assign tx_busy = busy;
endmodule

module tb;
    reg clk, rst_n, tx_start;
    reg [7:0] tx_data;
    wire tx, tx_busy;
    reg [7:0] rx_buf;

    uart_tx #(.DIV(4)) u_tx(.clk(clk), .rst_n(rst_n), .tx_start(tx_start),
                            .tx_data(tx_data), .tx(tx), .tx_busy(tx_busy));

    always #5 clk = ~clk;

    initial begin
        clk = 0; rst_n = 1; tx_start = 0; tx_data = 8'hA5; rx_buf = 8'h00;
        #1 rst_n = 0;
        #3 rst_n = 1;
        #8 $display("t=12 idle: tx=%b busy=%b", tx, tx_busy);
        #2 tx_start = 1;
        #2 tx_start = 0;
        #1 $display("t=17 start: tx=%b busy=%b", tx, tx_busy);
        #40 rx_buf = {rx_buf[6:0], tx}; $display("t=57 bit0: tx=%b busy=%b", tx, tx_busy);
        #40 rx_buf = {rx_buf[6:0], tx}; $display("t=97 bit1: tx=%b busy=%b", tx, tx_busy);
        #40 rx_buf = {rx_buf[6:0], tx}; $display("t=137 bit2: tx=%b busy=%b", tx, tx_busy);
        #40 rx_buf = {rx_buf[6:0], tx}; $display("t=177 bit3: tx=%b busy=%b", tx, tx_busy);
        #40 rx_buf = {rx_buf[6:0], tx}; $display("t=217 bit4: tx=%b busy=%b", tx, tx_busy);
        #40 rx_buf = {rx_buf[6:0], tx}; $display("t=257 bit5: tx=%b busy=%b", tx, tx_busy);
        #40 rx_buf = {rx_buf[6:0], tx}; $display("t=297 bit6: tx=%b busy=%b", tx, tx_busy);
        #40 rx_buf = {rx_buf[6:0], tx}; $display("t=337 bit7: tx=%b busy=%b", tx, tx_busy);
        #10 $display("t=347 received=%h busy=%b", rx_buf, tx_busy);
        #7 tx_start = 1;
        #10 tx_start = 0;
        #3 $display("t=367 restart: tx=%b busy=%b", tx, tx_busy);
        $finish;
    end
endmodule