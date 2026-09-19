// IEEE 1800-2009 7.4 and 7.6: fixed unpacked arrays keep their declared
// (possibly reversed or negative) bounds, and whole-array copy follows the
// logical declaration order rather than storage order.
module tb;
    logic [7:0] src [-1:-3][2:0];
    logic [7:0] dst [-1:-3][2:0];
    integer i;
    integer j;
    integer k;

    initial begin
        k = 0;
        for (i = -1; i >= -3; i = i - 1) begin
            for (j = 2; j >= 0; j = j - 1) begin
                src[i][j] = 8'h10 + k[7:0];
                k = k + 1;
            end
        end
        dst = src;
        k = 0;
        for (i = -1; i >= -3; i = i - 1) begin
            for (j = 2; j >= 0; j = j - 1) begin
                if (dst[i][j] !== 8'h10 + k[7:0]) begin
                    $display("FAIL reverse_bounds_copy %0d %0d", i, j);
                    $finish;
                end
                k = k + 1;
            end
        end
        dst[-2][1] = 8'hee;
        if (src[-2][1] !== 8'h14 || dst[-2][1] !== 8'hee) begin
            $display("FAIL reverse_bounds_independence");
            $finish;
        end
        $display("PASS fixed_array_reversed_bounds");
        $finish;
    end
endmodule
