module child(input logic [7:0] in_value, output wire [7:0] out_value);
    assign out_value = in_value;
endmodule

module tb;
    logic [7:0] source [0:1];
    logic [7:0] result;
    integer index;

    child dut(.in_value(source[index]), .out_value(result));

    initial begin
        index = 0;
        source[0] = 8'h11;
        source[1] = 8'h22;
        #1 $display("%h", result);
        index = 1;
        #1 $display("%h", result);
        source[1] = 8'h33;
        #1 $display("%h", result);
        $finish(0);
    end
endmodule
