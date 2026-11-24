// Two 32-bit 8-way priority multiplexers written as conditional operator
// chains, re-evaluated for 100,000 clock cycles: `rotate` grants each request
// in turn, so every depth of the chain is reached equally often, and `mixed`
// steps through all 256 request patterns, where the first requests win most.
module tb;
    logic clk = 0;
    logic [7:0] rotate = 8'h01, mixed = 8'h00;
    logic [31:0] d0 = 32'd11, d1 = 32'd22, d2 = 32'd33, d3 = 32'd44;
    logic [31:0] d4 = 32'd55, d5 = 32'd66, d6 = 32'd77, d7 = 32'd88;
    logic [31:0] sum = 32'd0;
    int cycles = 0;
    wire [31:0] by_turn = rotate[0] ? d0 : rotate[1] ? d1 : rotate[2] ? d2 : rotate[3] ? d3 :
                          rotate[4] ? d4 : rotate[5] ? d5 : rotate[6] ? d6 : rotate[7] ? d7 : 32'd0;
    wire [31:0] by_rank = mixed[0] ? d0 : mixed[1] ? d1 : mixed[2] ? d2 : mixed[3] ? d3 :
                          mixed[4] ? d4 : mixed[5] ? d5 : mixed[6] ? d6 : mixed[7] ? d7 : 32'd0;

    always #1 clk = ~clk;

    always @(posedge clk) begin
        sum <= sum + by_turn + (by_rank ^ {by_turn[15:0], by_turn[31:16]});
        rotate <= {rotate[6:0], rotate[7]};
        mixed <= mixed * 8'd5 + 8'd1;
        d0 <= d0 + 32'd1;
        d5 <= d5 ^ sum;
        cycles <= cycles + 1;
        if (cycles == 100000) begin
            $display("cycles=%0d sum=%h by_turn=%h by_rank=%h", cycles, sum, by_turn, by_rank);
            $finish;
        end
    end
endmodule
