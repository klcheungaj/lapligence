// llg-test-fixture: tests/fixtures/sim/logical_ops/implication_equivalence.sv
// IEEE 1800-2009 §11.4.7: ordinary expression implication and equivalence.
// Property implication (|-> / |=>) is intentionally not exercised here.
module tb;
    localparam real REAL_ZERO_PARAM = 0.0;
    localparam logic DECL_IMP = REAL_ZERO_PARAM -> 1'bx;
    localparam logic DECL_EQ = REAL_ZERO_PARAM <-> 1'b0;
    logic [3:0] zero;
    logic [3:0] one;
    logic [3:0] x_value;
    logic [3:0] z_value;
    logic [3:0] mixed;
    logic [0:0] result;
    real real_zero;
    real real_one;
    integer calls;

    function automatic logic mark(input logic value);
        begin
            calls = calls + 1;
            mark = value;
        end
    endfunction

    initial begin
        zero = 4'b0000;
        one = 4'b0001;
        x_value = 4'bxxxx;
        z_value = 4'bzzzz;
        mixed = 4'b0x01;
        real_zero = 0.0;
        real_one = 1.0;

        // Every 0/1/X/Z pair, with the result reduced to one bit.
        $display("imp0=%b%b%b%b", (zero -> zero), (zero -> one),
                 (zero -> x_value), (zero -> z_value));
        $display("imp1=%b%b%b%b", (one -> zero), (one -> one),
                 (one -> x_value), (one -> z_value));
        $display("impx=%b%b%b%b", (x_value -> zero), (x_value -> one),
                 (x_value -> x_value), (x_value -> z_value));
        $display("impz=%b%b%b%b", (z_value -> zero), (z_value -> one),
                 (z_value -> x_value), (z_value -> z_value));

        $display("eq0=%b%b%b%b", (zero <-> zero), (zero <-> one),
                 (zero <-> x_value), (zero <-> z_value));
        $display("eq1=%b%b%b%b", (one <-> zero), (one <-> one),
                 (one <-> x_value), (one <-> z_value));
        $display("eqx=%b%b%b%b", (x_value <-> zero), (x_value <-> one),
                 (x_value <-> x_value), (x_value <-> z_value));
        $display("eqz=%b%b%b%b", (z_value <-> zero), (z_value <-> one),
                 (z_value <-> x_value), (z_value <-> z_value));

        // A known one in a vector makes its logical value true.
        $display("mixed=%b%b", (mixed -> zero), (mixed <-> one));

        // Implication short-circuits only a known-false antecedent. Equivalence
        // evaluates both operands, including function side effects.
        calls = 0;
        result = 1'b0 -> mark(1'b1);
        $display("short_false=%b calls=%0d", result, calls);
        calls = 0;
        result = 1'b1 -> mark(1'b0);
        $display("short_true=%b calls=%0d", result, calls);
        calls = 0;
        result = 1'bx -> mark(1'b0);
        $display("short_unknown=%b calls=%0d", result, calls);
        calls = 0;
        result = mark(1'b0) <-> mark(1'b1);
        $display("equiv_calls=%b calls=%0d", result, calls);

        // -> and <-> have lower precedence than || and are right associative.
        $display("precedence=%b chain=%b", (zero -> zero || one),
                 (zero -> zero -> zero));

        // Logical operations accept real operands, but still produce a packed
        // one-bit result and preserve unknowns on the four-state side.
        $display("real=%b%b decl=%b%b", (real_zero -> x_value),
                 (real_one <-> x_value), DECL_IMP, DECL_EQ);
        $finish;
    end
endmodule
