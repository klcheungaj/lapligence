// V 2.5.1 / SV 5.7.1: based integral literals at scalar and vector widths.
module tb;
    reg bit_value;
    reg [6:0] octal_value;
    reg [7:0] hex_value;
    reg [31:0] unknown_value;

    initial begin
        bit_value = 1'b1;
        octal_value = 7'o77;
        hex_value = 8'ha5;
        unknown_value = 32'h0000_x0z5;
        if (bit_value !== 1'b1 || octal_value !== 7'd63 || hex_value !== 8'ha5 ||
            unknown_value[15:12] !== 4'bxxxx ||
            unknown_value[7:4] !== 4'bzzzz) begin
            $display("FAIL radix literals");
            $finish(1);
        end
        $display("literals=1,3f,a5,xz");
        $finish(0);
    end
endmodule
