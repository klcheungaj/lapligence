// llg-test-fixture: tests/fixtures/sim/data_types/wide_division.v
`timescale 1ns/1ps
module tb;
    parameter WIDTH = 128;

    reg [WIDTH-1:0] dividend;
    reg [WIDTH-1:0] divisor;
    reg [WIDTH-1:0] quotient;
    reg [WIDTH-1:0] expected;

    initial begin
        // (2^(WIDTH-1) + 2^(WIDTH-65)) / 2^(WIDTH-65) = 2^64 + 1.
        dividend = 0;
        dividend[WIDTH-1] = 1'b1;
        dividend[WIDTH-65] = 1'b1;
        divisor = 0;
        divisor[WIDTH-65] = 1'b1;
        #1;
        quotient = dividend / divisor;
        expected = 0;
        expected[64] = 1'b1;
        expected[0] = 1'b1;
        if (quotient !== expected) begin
            $display("FAIL division-high-bits");
            $finish;
        end

        $display("PASS division WIDTH=%0d", WIDTH);
        $finish;
    end
endmodule
