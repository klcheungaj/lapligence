module tb;
    logic [64:0] value;
    logic [65536:0] wide;
    initial begin
        value = 0;
        wide = '0;
        wide[65536] = 1'b1;
        repeat (4096) value = (value + 65'd3) ^ 65'h5;
        $display("%0d %0d", value, $countones(wide));
    end
endmodule
