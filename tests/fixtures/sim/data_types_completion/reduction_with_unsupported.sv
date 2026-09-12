// IEEE 1800-2009 7.12.3 makes this width-changing with clause legal and gives
// the sum the 32-bit type of int'(item). The callback must be evaluated for
// every source element rather than silently reducing the original 8-bit data.
module tb;
    logic [7:0] values[];
    int total;

    initial begin
        values = new[2];
        values[0] = 8'd255;
        values[1] = 8'd1;
        total = values.sum() with (int'(item));
        if (total !== 32'd256) begin
            $display("FAIL reduction_with total=%0d", total);
            $finish;
        end
        $display("PASS reduction_with");
        $finish;
    end
endmodule
