// IEEE 1800-2009 11.4.10-11.4.12 and 11.8: shift counts are self-determined,
// concatenation is unsigned, and an X/Z condition merges bits.
module tb #(parameter WIDTH = 4096, parameter HALF = WIDTH / 2);
    logic [WIDTH-1:0] value;
    logic [WIDTH-1:0] shifted;
    logic [WIDTH-1:0] expected;
    logic [WIDTH-1:0] left;
    logic [WIDTH-1:0] right;
    logic [WIDTH-1:0] merged;
    logic [WIDTH-1:0] shift_count;
    logic condition;
    logic reduce_or;
    logic reduce_and;
    logic reduce_xor;
    integer failed;

    initial begin
        failed = 0;
        value = '0;
        value[WIDTH-1] = 1'b1;
        value[64] = 1'b1;
        value[0] = 1'b1;

        shifted = value << 0;
        if (shifted !== value) begin
            $display("FAIL shift-zero WIDTH=%0d", WIDTH);
            failed = 1;
        end
        shifted = value >> (WIDTH-1);
        expected = '0;
        expected[0] = 1'b1;
        if (!failed && shifted !== expected) begin
            $display("FAIL shift-width-minus-one WIDTH=%0d", WIDTH);
            failed = 1;
        end
        shifted = value >> WIDTH;
        if (!failed && shifted !== '0) begin
            $display("FAIL shift-by-width WIDTH=%0d", WIDTH);
            failed = 1;
        end
        shifted = value << (WIDTH+17);
        if (!failed && shifted !== '0) begin
            $display("FAIL shift-beyond-width WIDTH=%0d", WIDTH);
            failed = 1;
        end
        shift_count = '0;
        shift_count[WIDTH-1] = 1'b1;
        shifted = value >> shift_count;
        if (!failed && shifted !== '0) begin
            $display("FAIL high-bit-shift-count WIDTH=%0d", WIDTH);
            failed = 1;
        end

        shifted = $signed(value) >>> WIDTH;
        if (!failed && shifted !== {WIDTH{1'b1}}) begin
            $display("FAIL arithmetic-shift-by-width WIDTH=%0d", WIDTH);
            failed = 1;
        end

        left = '0;
        right = '0;
        left[HALF-1] = 1'b1;
        left[0] = 1'b1;
        right[HALF-2] = 1'b1;
        right[0] = 1'b1;
        shifted = {left[HALF-1:0], right[HALF-1:0]};
        expected = '0;
        expected[WIDTH-1] = 1'b1;
        expected[HALF] = 1'b1;
        expected[HALF-2] = 1'b1;
        expected[0] = 1'b1;
        if (!failed && shifted !== expected) begin
            $display("FAIL concatenation-layout WIDTH=%0d", WIDTH);
            failed = 1;
        end

        left = '0;
        right = '0;
        left[WIDTH-1] = 1'b1;
        right[WIDTH-1] = 1'b1;
        left[64] = 1'bx;
        right[64] = 1'bx;
        left[1] = 1'bz;
        right[1] = 1'bz;
        left[0] = 1'b1;
        condition = 1'bx;
        merged = condition ? left : right;
        expected = '0;
        expected[WIDTH-1] = 1'b1;
        expected[64] = 1'bx;
        expected[1] = 1'bz;
        expected[0] = 1'bx;
        if (!failed && merged !== expected) begin
            $display("FAIL conditional-merge WIDTH=%0d", WIDTH);
            failed = 1;
        end

        left = '0;
        left[WIDTH-1] = 1'bx;
        left[0] = 1'b1;
        reduce_or = |left;
        left = '1;
        left[WIDTH-1] = 1'bz;
        left[0] = 1'b0;
        reduce_and = &left;
        left = '0;
        left[64] = 1'bx;
        reduce_xor = ^left;
        if (!failed &&
            (reduce_or !== 1'b1 || reduce_and !== 1'b0 || reduce_xor !== 1'bx)) begin
            $display("FAIL reductions WIDTH=%0d", WIDTH);
            failed = 1;
        end

        if (!failed) $display("PASS shifts_concat_conditional WIDTH=%0d", WIDTH);
        $finish;
    end
endmodule
