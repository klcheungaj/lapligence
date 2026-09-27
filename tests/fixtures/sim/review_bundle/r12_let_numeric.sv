// llg-test-fixture: tests/fixtures/sim/review_bundle/r12_let_numeric.sv
// IEEE 1800-2009 §11.13: a let expression substitutes a runtime argument
// while resolving a free parameter name in its declaration scope, even when
// the caller shadows that name with a different local value.
module tb;
    parameter integer BIAS = 3;
    let add_bias(x) = x + BIAS;
    let typed_add(int x) = x + BIAS;
    localparam integer CONSTANT_RESULT = add_bias(4);
    integer seed;
    integer result;
    integer typed_result;

    initial begin
        if (!$value$plusargs("seed=%d", seed) || seed != 4)
            $fatal(1, "seed must be 4");
        begin : caller
            integer BIAS;
            BIAS = 100;
            result = add_bias(seed);
            typed_result = typed_add(seed);
        end
        if (result !== 7 || typed_result !== 7 || CONSTANT_RESULT !== 7)
            $fatal(1, "let expansion lost declaration scope or runtime argument");
        $display("let=%0d,%0d", result, typed_result);
        $finish(0);
    end
endmodule
