module tb;
    logic [6:0] index;
    logic source;
    wire [127:0] bus;

    assign bus[index] = source;
endmodule
