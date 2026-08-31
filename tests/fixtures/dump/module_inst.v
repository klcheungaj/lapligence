
module adder (
    input clk,
    input din,
    output dout
);

reg dreg;
assign dout = dreg;

always @(posedge clk) begin
    dreg <= din + 1;
end

endmodule


module top (
    input clk,
    input din,
    output dout
);


adder u_adder(
    .clk(clk),
    .din(din),
    .dout(dout)
);

endmodule
