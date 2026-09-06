// llg-test-fixture: tests/fixtures/sim/data_types/wide_arithmetic.v
`timescale 1ns/1ps
module tb;
    parameter WIDTH = 128;

    reg [WIDTH-1:0] a;
    reg [WIDTH-1:0] b;
    reg [WIDTH-1:0] expected;
    reg [WIDTH-1:0] result;
    reg signed [WIDTH-1:0] signed_a;
    reg signed [WIDTH-1:0] signed_b;
    integer index;

    initial begin
        // Carry across the first 64-bit runtime limb.
        a = 0;
        b = 0;
        a[63:0] = 64'hffff_ffff_ffff_ffff;
        b[0] = 1'b1;
        #1;
        result = a + b;
        expected = 0;
        expected[64] = 1'b1;
        if (result !== expected) begin
            $display("FAIL add-carry");
            $finish;
        end

        // Borrow propagates independently through every limb.
        a = 0;
        b = 0;
        b[0] = 1'b1;
        #1;
        result = a - b;
        expected = 0;
        expected = ~expected;
        if (result !== expected) begin
            $display("FAIL subtract-borrow");
            $finish;
        end

        // Carry across the upper half and into the final result bit.
        a = 0;
        b = 0;
        for (index = WIDTH/2; index < WIDTH-1; index = index + 1)
            a[index] = 1'b1;
        b[WIDTH/2] = 1'b1;
        #1;
        result = a + b;
        expected = 0;
        expected[WIDTH-1] = 1'b1;
        if (result !== expected) begin
            $display("FAIL add-upper-carry");
            $finish;
        end

        // A carry through every bit is truncated at the declared width.
        a = 0;
        a = ~a;
        b = 0;
        b[0] = 1'b1;
        #1;
        result = a + b;
        expected = 0;
        if (result !== expected) begin
            $display("FAIL add-overflow-truncation");
            $finish;
        end

        // (2^(WIDTH-3) + 1) * 3 truncates to four independently placed bits.
        a = 0;
        b = 0;
        a[WIDTH-3] = 1'b1;
        a[0] = 1'b1;
        b[1:0] = 2'b11;
        #1;
        result = a * b;
        expected = 0;
        expected[WIDTH-2] = 1'b1;
        expected[WIDTH-3] = 1'b1;
        expected[1] = 1'b1;
        expected[0] = 1'b1;
        if (result !== expected) begin
            $display("FAIL multiply-cross-limb");
            $finish;
        end

        // Shift distances on both sides of the 64-bit limb boundary.
        a = 0;
        a[0] = 1'b1;
        #1;
        expected = 0;
        expected[63] = 1'b1;
        if ((a << 63) !== expected) begin
            $display("FAIL shift-left-63");
            $finish;
        end
        expected = 0;
        expected[64] = 1'b1;
        if ((a << 64) !== expected) begin
            $display("FAIL shift-left-64");
            $finish;
        end
        expected = 0;
        expected[65] = 1'b1;
        if ((a << 65) !== expected) begin
            $display("FAIL shift-left-65");
            $finish;
        end
        expected = 0;
        expected[127] = 1'b1;
        if ((a << 127) !== expected) begin
            $display("FAIL shift-left-127");
            $finish;
        end
        expected = 0;
        if ((a << WIDTH) !== expected) begin
            $display("FAIL shift-left-width");
            $finish;
        end
        a = 0;
        a[WIDTH-1] = 1'b1;
        #1;
        expected = 0;
        expected[0] = 1'b1;
        if ((a >> (WIDTH-1)) !== expected) begin
            $display("FAIL shift-right-boundary");
            $finish;
        end

        // Unsigned ordering sees the high bit as the largest magnitude;
        // signed ordering sees the same bit pattern as negative.
        a = 0;
        b = 0;
        a[WIDTH-1] = 1'b1;
        b[0] = 1'b1;
        signed_a = a;
        signed_b = b;
        #1;
        if ((a > b) !== 1'b1 || (a < b) !== 1'b0) begin
            $display("FAIL compare-unsigned");
            $finish;
        end
        if ((signed_a < signed_b) !== 1'b1 || (signed_a > signed_b) !== 1'b0) begin
            $display("FAIL compare-signed");
            $finish;
        end

        // Four-state dominance and propagation at every limb, including
        // a high X bit that cannot be lost through a low-limb shortcut.
        a = 0;
        for (index = 0; index < WIDTH; index = index + 1)
            a[index] = 1'bx;
        b = 0;
        expected = 0;
        #1;
        if ((a & b) !== expected) begin
            $display("FAIL four-state-and-dominance");
            $finish;
        end
        b = 0;
        b = ~b;
        expected = 0;
        expected = ~expected;
        #1;
        if ((a | b) !== expected) begin
            $display("FAIL four-state-or-dominance");
            $finish;
        end
        a = 0;
        a[WIDTH-1] = 1'bx;
        b = 0;
        #1;
        result = a ^ b;
        if ((result[WIDTH-1] !== 1'bx) || (result[0] !== 1'b0)) begin
            $display("FAIL four-state-x-propagation");
            $finish;
        end

        // Any unknown arithmetic operand makes the complete result unknown,
        // including bits below the unknown high limb.
        a = 0;
        a[WIDTH-2] = 1'bx;
        b = 0;
        b[1:0] = 2'b11;
        expected = 0;
        for (index = 0; index < WIDTH; index = index + 1)
            expected[index] = 1'bx;
        #1;
        result = a + b;
        if (result !== expected) begin
            $display("FAIL arithmetic-x-add");
            $finish;
        end
        result = a - b;
        if (result !== expected) begin
            $display("FAIL arithmetic-x-subtract");
            $finish;
        end
        result = a * b;
        if (result !== expected) begin
            $display("FAIL arithmetic-x-multiply");
            $finish;
        end

        // Reductions must inspect the top limb rather than only bits 63:0.
        a = 0;
        a[WIDTH-1] = 1'b1;
        #1;
        if ((|a) !== 1'b1 || (^a) !== 1'b1 || (&a) !== 1'b0) begin
            $display("FAIL reduction-high-bit");
            $finish;
        end
        a = 0;
        a = ~a;
        #1;
        if ((&a) !== 1'b1) begin
            $display("FAIL reduction-all-ones");
            $finish;
        end

        // Build a WIDTH-bit concatenation from disjoint slices.  Expected
        // positions are set directly, independently of concatenation.
        a = 0;
        a[WIDTH-65] = 1'b1;
        a[0] = 1'b1;
        #1;
        result = {a[WIDTH-65:0], 64'ha5a5_5a5a_0123_4567};
        expected = 0;
        expected[WIDTH-1] = 1'b1;
        expected[64] = 1'b1;
        expected[63:0] = 64'ha5a5_5a5a_0123_4567;
        if (result !== expected) begin
            $display("FAIL concatenate-wide");
            $finish;
        end
        if (result[WIDTH-1] !== 1'b1 || result[64] !== 1'b1 ||
            result[63:0] !== 64'ha5a5_5a5a_0123_4567) begin
            $display("FAIL select-wide");
            $finish;
        end

        $display("PASS arithmetic WIDTH=%0d", WIDTH);
        $finish;
    end
endmodule
