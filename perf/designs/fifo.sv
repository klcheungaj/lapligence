// Copied verbatim from tests/sim_stress.rs `stress_fifo_push_pop`.
module fifo #(parameter DEPTH = 4, parameter DW = 8) (
    input  logic        clk,
    input  logic        rst_n,
    input  logic        push,
    input  logic        pop,
    input  logic [DW-1:0] wdata,
    output logic [DW-1:0] rdata,
    output logic        full,
    output logic        empty,
    output logic [1:0]  head,
    output logic [1:0]  tail
);
    reg [DW-1:0] mem [0:DEPTH-1];

    always @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            head  <= 2'd0;
            tail  <= 2'd0;
            full  <= 1'b0;
            empty <= 1'b1;
            rdata <= 8'h00;
        end else begin
            if (push && !full) begin
                mem[head] <= wdata;
                head <= head + 2'd1;
            end
            if (pop && !empty) begin
                rdata <= mem[tail];
                tail <= tail + 2'd1;
            end
            if (push && !full) begin
                full  <= (head + 2'd1 == tail);
                empty <= 1'b0;
            end else if (pop && !empty) begin
                empty <= (tail + 2'd1 == head);
                full  <= 1'b0;
            end
        end
    end
endmodule

module tb;
    reg clk, rst_n, push, pop;
    reg [7:0] wdata;
    wire [7:0] rdata;
    wire full, empty;
    wire [1:0] head, tail;

    fifo #(.DEPTH(4), .DW(8)) u_fifo(
        .clk(clk), .rst_n(rst_n), .push(push), .pop(pop), .wdata(wdata),
        .rdata(rdata), .full(full), .empty(empty), .head(head), .tail(tail)
    );

    always #5 clk = ~clk;

    initial begin
        clk = 0; rst_n = 1; push = 0; pop = 0; wdata = 8'h00;
        #1 rst_n = 0;
        #3 rst_n = 1; push = 1; wdata = 8'h11;
        #10 push = 1; wdata = 8'h22;
        #10 push = 1; wdata = 8'h33;
        #10 push = 1; wdata = 8'h44;
        #10 push = 1; wdata = 8'hEE;
        #2 $display("push-when-full: full=%b empty=%b head=%0d tail=%0d rdata=%h", full, empty, head, tail, rdata);
        #8 push = 0; pop = 1;
        #10 pop = 1;
        #10 pop = 0; push = 1; wdata = 8'h55;
        #10 push = 0;
        #3 $display("full=%b empty=%b head=%0d tail=%0d rdata=%h", full, empty, head, tail, rdata);
        pop = 1;
        #7 pop = 1;
        #10 pop = 1;
        #10 pop = 1;
        #10 pop = 1;
        #2 $display("full=%b empty=%b head=%0d tail=%0d rdata=%h", full, empty, head, tail, rdata);
        #10 $display("pop-when-empty: empty=%b head=%0d tail=%0d rdata=%h", empty, head, tail, rdata);
        $finish;
    end
endmodule