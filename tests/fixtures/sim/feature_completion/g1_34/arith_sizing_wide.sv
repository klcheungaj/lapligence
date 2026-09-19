// llg-test-fixture: G1-34 rtl_composition_gate.
// Expression sizing across mixed signed/unsigned operands, 96-bit vectors,
// reductions, shifts, comparisons and set membership.
module tb;
    logic [7:0] u8;
    logic signed [7:0] s8;
    logic signed [15:0] add_mixed;
    logic signed [15:0] sub_mixed;
    logic lt_mixed;
    logic gt_signed;

    logic [95:0] wide_a;
    logic [95:0] wide_b;
    logic [95:0] wide_sum;
    logic [95:0] wide_shl;
    logic signed [95:0] wide_neg;
    logic signed [95:0] wide_sra;
    logic red_xor;
    logic red_and;
    logic red_or;
    logic wide_eq;
    logic wide_ne;

    logic [3:0] x;
    logic in1;
    logic in2;
    logic in3;

    initial begin
        u8 = 8'h80;
        s8 = -8'sd1;
        add_mixed = u8 + s8;
        sub_mixed = u8 - s8;
        lt_mixed = u8 < s8;
        gt_signed = s8 > 0;
        $display("mixed add=%0d sub=%0d lt=%b gt=%b",
                 add_mixed, sub_mixed, lt_mixed, gt_signed);

        wide_a = 96'h0123456789ABCDEF_FEDCBA98;
        wide_b = 96'hFEDCBA9876543210_01234567;
        wide_neg = 96'hF000000000000000_00000001;
        wide_sum = wide_a + wide_b;
        wide_shl = wide_a << 13;
        wide_sra = wide_neg >>> 8;
        red_xor = ^wide_a;
        red_and = &wide_a;
        red_or = |wide_a;
        wide_eq = (wide_a == wide_b);
        wide_ne = (wide_sum != 0);
        $display("wide sum=%h shl=%h sra=%h", wide_sum, wide_shl, wide_sra);
        $display("red xor=%b and=%b or=%b eq=%b ne=%b",
                 red_xor, red_and, red_or, wide_eq, wide_ne);

        x = 4'd5;
        in1 = x inside {4'd1, 4'd5, 4'd9};
        in2 = x inside {[4'd6:4'd8]};
        in3 = x inside {4'd5, [4'd7:4'd9]};
        $display("inside %b %b %b", in1, in2, in3);
        $finish(0);
    end
endmodule
