
module top(
    input clk,
    output dat
);

reg[31:0] counter;

always @(posedge clk) begin
    counter <= dat;
end

endmodule
