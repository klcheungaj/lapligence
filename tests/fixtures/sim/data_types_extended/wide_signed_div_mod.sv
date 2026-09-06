// IEEE 1800-2009 11.4.3 and 11.8: integral division truncates toward zero,
// the remainder has the dividend's sign, and a zero divisor produces X.
module tb #(parameter WIDTH = 2048);
    logic signed [WIDTH-1:0] dividend;
    logic signed [WIDTH-1:0] divisor;
    logic signed [WIDTH-1:0] quotient;
    logic signed [WIDTH-1:0] remainder;
    logic signed [WIDTH-1:0] expected;
    integer failed;

    initial begin
        failed = 0;

        // (2^(WIDTH-2) + 2^(WIDTH-67)) / 2^(WIDTH-67) = 2^65 + 1.
        dividend = '0;
        divisor = '0;
        dividend[WIDTH-2] = 1'b1;
        dividend[WIDTH-67] = 1'b1;
        divisor[WIDTH-67] = 1'b1;
        #1;
        quotient = dividend / divisor;
        remainder = dividend % divisor;
        expected = '0;
        expected[65] = 1'b1;
        expected[0] = 1'b1;
        if (quotient !== expected || remainder !== '0) begin
            $display("FAIL positional-positive WIDTH=%0d", WIDTH);
            failed = 1;
        end

        dividend = -100;
        divisor = 7;
        #1;
        quotient = dividend / divisor;
        remainder = dividend % divisor;
        if (!failed && (quotient !== -14 || remainder !== -2)) begin
            $display("FAIL negative-dividend WIDTH=%0d", WIDTH);
            failed = 1;
        end

        dividend = 100;
        divisor = -7;
        #1;
        quotient = dividend / divisor;
        remainder = dividend % divisor;
        if (!failed && (quotient !== -14 || remainder !== 2)) begin
            $display("FAIL negative-divisor WIDTH=%0d", WIDTH);
            failed = 1;
        end

        dividend = -100;
        divisor = -7;
        #1;
        quotient = dividend / divisor;
        remainder = dividend % divisor;
        if (!failed && (quotient !== 14 || remainder !== -2)) begin
            $display("FAIL both-negative WIDTH=%0d", WIDTH);
            failed = 1;
        end

        // The unrepresentable mathematical quotient wraps to the declared
        // two's-complement width; the division is otherwise exact.
        dividend = '0;
        dividend[WIDTH-1] = 1'b1;
        divisor = -1;
        #1;
        quotient = dividend / divisor;
        remainder = dividend % divisor;
        if (!failed && (quotient !== dividend || remainder !== '0)) begin
            $display("FAIL minimum-overflow WIDTH=%0d", WIDTH);
            failed = 1;
        end

        dividend = 123;
        divisor = '0;
        #1;
        quotient = dividend / divisor;
        remainder = dividend % divisor;
        if (!failed &&
            (quotient !== {WIDTH{1'bx}} || remainder !== {WIDTH{1'bx}})) begin
            $display("FAIL divide-by-zero WIDTH=%0d", WIDTH);
            failed = 1;
        end

        if (!failed) $display("PASS wide_signed_div_mod WIDTH=%0d", WIDTH);
        $finish;
    end
endmodule
