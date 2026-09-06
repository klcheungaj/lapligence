// llg-test-fixture: tests/fixtures/sim/data_types/wide_power.v
`timescale 1ns/1ps
module tb;
    parameter WIDTH = 128;

    reg [WIDTH-1:0] base;
    reg [WIDTH-1:0] exponent;
    reg [WIDTH-1:0] power;
    reg [WIDTH-1:0] expected;

    initial begin
        // (2^(WIDTH-2) + 1)^2 truncates to 2^(WIDTH-1) + 1.
        base = 0;
        base[WIDTH-2] = 1'b1;
        base[0] = 1'b1;
        exponent = 0;
        exponent[1] = 1'b1;
        #1;
        power = base ** exponent;
        expected = 0;
        expected[WIDTH-1] = 1'b1;
        expected[0] = 1'b1;
        if (power !== expected) begin
            $display("FAIL power-high-bits");
            $finish;
        end

        $display("PASS power WIDTH=%0d", WIDTH);
        $finish;
    end
endmodule
