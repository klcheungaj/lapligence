// IEEE 1800-2009 11.4.3: exponentiation is an integral arithmetic operator;
// the result is sized to its expression context and unknown inputs propagate.
module tb #(parameter WIDTH = 2048);
    logic signed [WIDTH-1:0] base;
    logic signed [WIDTH-1:0] exponent;
    logic signed [WIDTH-1:0] result;
    logic signed [WIDTH-1:0] expected;
    integer failed;

    initial begin
        failed = 0;

        // (2^(WIDTH-2) + 1)^2 truncates to 2^(WIDTH-1) + 1.
        base = '0;
        base[WIDTH-2] = 1'b1;
        base[0] = 1'b1;
        exponent = 2;
        #1;
        result = base ** exponent;
        expected = '0;
        expected[WIDTH-1] = 1'b1;
        expected[0] = 1'b1;
        if (result !== expected) begin
            $display("FAIL positional-square WIDTH=%0d", WIDTH);
            failed = 1;
        end

        base = -3;
        exponent = 3;
        #1;
        result = base ** exponent;
        if (!failed && result !== -27) begin
            $display("FAIL negative-odd WIDTH=%0d", WIDTH);
            failed = 1;
        end

        exponent = 4;
        #1;
        result = base ** exponent;
        if (!failed && result !== 81) begin
            $display("FAIL negative-even WIDTH=%0d", WIDTH);
            failed = 1;
        end

        base = '0;
        exponent = '0;
        #1;
        result = base ** exponent;
        if (!failed && result !== 1) begin
            $display("FAIL zero-exponent WIDTH=%0d", WIDTH);
            failed = 1;
        end

        base = 2;
        exponent = WIDTH;
        #1;
        result = base ** exponent;
        if (!failed && result !== '0) begin
            $display("FAIL overflow-truncation WIDTH=%0d", WIDTH);
            failed = 1;
        end

        base = 3;
        exponent = '0;
        exponent[0] = 1'bx;
        #1;
        result = base ** exponent;
        if (!failed && result !== {WIDTH{1'bx}}) begin
            $display("FAIL unknown-exponent WIDTH=%0d", WIDTH);
            failed = 1;
        end

        if (!failed) $display("PASS wide_power WIDTH=%0d", WIDTH);
        $finish;
    end
endmodule
