`timescale 1ns/1ps
// IEEE 1364-2001 4.1.8-4.1.11/4.1.13: four-state bitwise,
// reduction and conditional truth tables. Rust supplies an independent oracle.
module tb;
    parameter WIDTH = 128;
    reg [WIDTH-1:0] a;
    reg [WIDTH-1:0] b;
    reg condition;
    reg z_condition;
    integer left_state;
    integer right_state;
    integer bit_index;
    reg [7:0] pattern_a;
    reg [7:0] pattern_b;

    function [0:0] state;
        input integer index;
        begin
            case (index)
                0: state = 1'b0;
                1: state = 1'b1;
                2: state = 1'bx;
                3: state = 1'bz;
            endcase
        end
    endfunction

    initial begin
        for (left_state = 0; left_state < 4; left_state = left_state + 1) begin
            for (right_state = 0; right_state < 4; right_state = right_state + 1) begin
                for (bit_index = 0; bit_index < WIDTH; bit_index = bit_index + 1) begin
                    a[bit_index] = state(left_state);
                    b[bit_index] = state(right_state);
                end
                condition = 1'bx;
                z_condition = 1'bz;
                #1;
                $display("pair=%0d%0d", left_state, right_state);
                $display("and=%b", a & b);
                $display("or=%b", a | b);
                $display("xor=%b", a ^ b);
                $display("xnor=%b", a ~^ b);
                $display("not=%b", ~a);
                $display("mux=%b", condition ? a : b);
                $display("muxz=%b", z_condition ? a : b);
                $display("eq=%b case=%b", a == b, a === b);
                $display("land=%b lor=%b lnot=%b nand=%b nor=%b rxor=%b rxnor=%b",
                         a && b, a || b, !a, ~&a, ~|a, ^a, ~^a);
            end
        end

        // One bit per byte cycles through all four states; the high limbs are
        // not merely zero padding around a low-word test value.
        a = 0;
        b = 0;
        pattern_a = 8'b10xz01zx;
        pattern_b = 8'b0110xz10;
        for (bit_index = 0; bit_index < (WIDTH / 8) * 8; bit_index = bit_index + 1) begin
            a[bit_index] = pattern_a[bit_index % 8];
            b[bit_index] = pattern_b[bit_index % 8];
        end
        #1;
        $display("mixed-and=%b", a & b);
        $display("mixed-or=%b", a | b);
        $display("mixed-xor=%b", a ^ b);
        $display("mixed-mux=%b", condition ? a : b);

        a = 0;
        b = 0;
        a[WIDTH-1] = 1'b1;
        #1;
        $display("high eq=%b case=%b gt=%b or=%b and=%b xor=%b",
                 a == b, a === b, a > b, |a, &a, ^a);
        a[WIDTH-1] = 1'bx;
        #1;
        $display("highx eq=%b case=%b or=%b and=%b xor=%b", a == b, a === b,
                 |a, &a, ^a);
        a[0] = 1'b1;
        #1;
        $display("known-dominance or=%b", |a);
        for (bit_index = 0; bit_index < WIDTH; bit_index = bit_index + 1)
            a[bit_index] = 1'b1;
        a[WIDTH-1] = 1'bz;
        #1;
        $display("highz or=%b and=%b xor=%b not=%b", |a, &a, ^a, ~a);
        $finish;
    end
endmodule
