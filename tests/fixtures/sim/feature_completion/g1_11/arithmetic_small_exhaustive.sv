// IEEE 1800-2009 11.4: every 2-bit four-state operand pair for the bitwise,
// arithmetic, equality, relational, logical and shift operators. The exact
// trace is compared against an independent Rust truth-table oracle in
// tests/sim_g1_closure.rs; no value is captured from the simulator.
module tb;
    reg [1:0] pats [0:15];
    reg [1:0] a, b;
    integer ai, bi;

    initial begin
        pats[0]  = 2'b00; pats[1]  = 2'b01; pats[2]  = 2'b0x; pats[3]  = 2'b0z;
        pats[4]  = 2'b10; pats[5]  = 2'b11; pats[6]  = 2'b1x; pats[7]  = 2'b1z;
        pats[8]  = 2'bx0; pats[9]  = 2'bx1; pats[10] = 2'bxx; pats[11] = 2'bxz;
        pats[12] = 2'bz0; pats[13] = 2'bz1; pats[14] = 2'bzx; pats[15] = 2'bzz;

        for (ai = 0; ai < 16; ai = ai + 1) begin
            for (bi = 0; bi < 16; bi = bi + 1) begin
                a = pats[ai];
                b = pats[bi];
                $display("%0d %0d and %b", ai, bi, a & b);
                $display("%0d %0d or %b", ai, bi, a | b);
                $display("%0d %0d xor %b", ai, bi, a ^ b);
                $display("%0d %0d xnor %b", ai, bi, a ~^ b);
                $display("%0d %0d add %b", ai, bi, a + b);
                $display("%0d %0d sub %b", ai, bi, a - b);
                $display("%0d %0d mul %b", ai, bi, a * b);
                $display("%0d %0d div %b", ai, bi, a / b);
                $display("%0d %0d mod %b", ai, bi, a % b);
                $display("%0d %0d eq %b", ai, bi, a == b);
                $display("%0d %0d ne %b", ai, bi, a != b);
                $display("%0d %0d ceq %b", ai, bi, a === b);
                $display("%0d %0d cne %b", ai, bi, a !== b);
                $display("%0d %0d lt %b", ai, bi, a < b);
                $display("%0d %0d le %b", ai, bi, a <= b);
                $display("%0d %0d gt %b", ai, bi, a > b);
                $display("%0d %0d ge %b", ai, bi, a >= b);
                $display("%0d %0d land %b", ai, bi, a && b);
                $display("%0d %0d lor %b", ai, bi, a || b);
                $display("%0d %0d shl %b", ai, bi, a << b);
                $display("%0d %0d shr %b", ai, bi, a >> b);
            end
        end
        $display("PASS arithmetic_small_exhaustive");
        $finish(0);
    end
endmodule
