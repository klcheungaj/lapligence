// Positional oracles exercise arithmetic across low, middle, and final limbs
// without printing vectors whose output size scales with WIDTH.
module tb #(parameter WIDTH = 2048);
    logic [WIDTH-1:0] left;
    logic [WIDTH-1:0] right;
    logic [WIDTH-1:0] result;
    logic [WIDTH-1:0] expected;
    integer failed;

    initial begin
        failed = 0;
        left = '0;
        right = '0;
        left[63:0] = 64'hffff_ffff_ffff_ffff;
        right[0] = 1'b1;
        result = left + right;
        expected = '0;
        expected[64] = 1'b1;
        if (result !== expected) begin
            $display("FAIL low-carry WIDTH=%0d", WIDTH);
            failed = 1;
        end

        left = '0;
        right = '0;
        left[WIDTH-2:WIDTH-65] = {64{1'b1}};
        right[WIDTH-65] = 1'b1;
        result = left + right;
        expected = '0;
        expected[WIDTH-1] = 1'b1;
        if (!failed && result !== expected) begin
            $display("FAIL high-carry WIDTH=%0d", WIDTH);
            failed = 1;
        end

        left = '0;
        right = '0;
        right[0] = 1'b1;
        result = left - right;
        if (!failed && result !== {WIDTH{1'b1}}) begin
            $display("FAIL full-borrow WIDTH=%0d", WIDTH);
            failed = 1;
        end

        left = '0;
        right = '0;
        left[WIDTH/2] = 1'b1;
        left[0] = 1'b1;
        right[WIDTH/2] = 1'b1;
        right[0] = 1'b1;
        result = left * right;
        expected = '0;
        expected[WIDTH/2+1] = 1'b1;
        expected[0] = 1'b1;
        if (!failed && result !== expected) begin
            $display("FAIL wide-multiply-truncation WIDTH=%0d", WIDTH);
            failed = 1;
        end

        if (!failed) $display("PASS scalable_arithmetic WIDTH=%0d", WIDTH);
        $finish;
    end
endmodule
