// IEEE 1800-2009 7.12.3 makes this width-changing with clause legal and gives
// the sum the 32-bit type of int'(item). The bounded simulator subset must
// reject the unsupported with clause explicitly instead of ignoring it.
module tb;
    logic [7:0] values[];
    int total;

    initial begin
        values = new[2];
        values[0] = 8'd255;
        values[1] = 8'd1;
        total = values.sum() with (int'(item));
        $display("UNEXPECTED reduction_with total=%0d", total);
        $finish;
    end
endmodule
