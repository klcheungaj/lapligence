module child(
    input logic [7:0] in_bus [1:0][0:1],
    output wire [7:0] out_bus [1:0][0:1]
);
    assign out_bus[1][0] = in_bus[1][0] + 8'd1;
    assign out_bus[1][1] = in_bus[1][1] + 8'd2;
    assign out_bus[0][0] = in_bus[0][0] + 8'd3;
    assign out_bus[0][1] = in_bus[0][1] + 8'd4;
endmodule

module tb;
    logic [7:0] source [0:1][1:0];
    logic [7:0] result [0:1][1:0];

    child dut(.in_bus(source), .out_bus(result));

    initial begin
        source[0][1] = 8'h10;
        source[0][0] = 8'h20;
        source[1][1] = 8'h30;
        source[1][0] = 8'h40;
        #1 $display("%h %h %h %h", result[0][1], result[0][0], result[1][1], result[1][0]);
        source[0][1] = 8'ha0;
        source[0][0] = 8'hb0;
        source[1][1] = 8'hc0;
        source[1][0] = 8'hd0;
        #1 $display("%h %h %h %h", result[0][1], result[0][0], result[1][1], result[1][0]);
        $finish(0);
    end
endmodule
