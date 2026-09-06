// llg-test-fixture: tests/fixtures/sim/data_types/wide_modulo.v
`timescale 1ns/1ps
module tb;
    parameter WIDTH = 128;

    reg [WIDTH-1:0] dividend;
    reg [WIDTH-1:0] divisor;
    reg [WIDTH-1:0] remainder;
    reg [WIDTH-1:0] expected;

    initial begin
        // The top bit is divisible by 2^(WIDTH-64); lower set bits remain.
        dividend = 0;
        dividend[WIDTH-1] = 1'b1;
        dividend[WIDTH-65] = 1'b1;
        dividend[0] = 1'b1;
        divisor = 0;
        divisor[WIDTH-64] = 1'b1;
        #1;
        remainder = dividend % divisor;
        expected = 0;
        expected[WIDTH-65] = 1'b1;
        expected[0] = 1'b1;
        if (remainder !== expected) begin
            $display("FAIL modulo-high-bits");
            $finish;
        end

        $display("PASS modulo WIDTH=%0d", WIDTH);
        $finish;
    end
endmodule
