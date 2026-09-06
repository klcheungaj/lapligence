// IEEE 1800-2009 7.4 and 11.5.1: select indices are self-determined;
// writes through out-of-range packed or unpacked indices have no effect.
module tb #(parameter WIDTH = 96);
    logic [31:0] index_high;
    logic [31:0] index_middle;
    logic [31:0] index_low;
    logic [15:0] packed_value;
    logic [7:0] unpacked_value [0:3];

    initial begin
        packed_value = 16'h0000;
        unpacked_value[0] = 8'h10;
        unpacked_value[1] = 8'h21;
        unpacked_value[2] = 8'h32;
        unpacked_value[3] = 8'h43;

        index_high = 32'd0;
        index_middle = 32'd0;
        index_low = 32'd3;
        packed_value[{index_high, index_middle, index_low}] = 1'b1;
        unpacked_value[{index_high, index_middle, index_low}] = 8'ha5;
        if (packed_value !== 16'h0008 || unpacked_value[3] !== 8'ha5) begin
            $display("FAIL wide_lhs_indices low_index WIDTH=%0d", WIDTH);
            $finish;
        end

        index_high = 32'd1;
        packed_value[{index_high, index_middle, index_low}] = 1'b0;
        unpacked_value[{index_high, index_middle, index_low}] = 8'h5a;
        if (packed_value !== 16'h0008 ||
            unpacked_value[0] !== 8'h10 || unpacked_value[1] !== 8'h21 ||
            unpacked_value[2] !== 8'h32 || unpacked_value[3] !== 8'ha5) begin
            $display("FAIL wide_lhs_indices high_index WIDTH=%0d", WIDTH);
            $finish;
        end

        $display("PASS wide_lhs_indices WIDTH=%0d", WIDTH);
        $finish;
    end
endmodule
