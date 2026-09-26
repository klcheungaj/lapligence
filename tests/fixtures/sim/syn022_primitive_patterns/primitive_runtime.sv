// llg-test-fixture: tests/fixtures/sim/syn022_primitive_patterns/primitive_runtime.sv
// IEEE 1800-2009 12.6, 12.6.2-12.6.3: matches is determined; &&& is ordered.
module tb;
    typedef enum logic [2:0] { IDLE = 3'd0, ACTIVE = 3'd5 } state_t;
    logic signed [6:0] value;
    logic signed [6:0] captured;
    logic [32:0] wide;
    logic [128:0] very_wide;
    bit one_bit;
    state_t state;
    int calls;
    int checks;

    function automatic logic signed [6:0] sample();
        calls++;
        return value;
    endfunction

    initial begin
        captured = 7'sd9;
        value = -7'sd5;
        calls = 0;
        checks = 0;
        if (sample() matches -7'sd5 &&& sample() matches .captured
            &&& captured == -7'sd5)
            checks++;
        else $fatal(1, "ordered constant and binding");
        if (calls != 2 || captured != 7'sd9)
            $fatal(1, "pattern binding shadowed outer declaration");

        if (sample() matches 7'sd5 &&& sample() matches .skipped)
            $fatal(1, "false prefix");
        if (calls != 3) $fatal(1, "false prefix evaluated suffix");

        value = 7'bx101010;
        if (sample() matches .*) checks++;
        else $fatal(1, "wildcard rejected X value");
        if (sample() matches .unknown_value &&& unknown_value === value)
            checks++;
        else $fatal(1, "binding lost X value");
        if (sample() matches 7'bx101010) checks++;
        else $fatal(1, "X constant did not match exactly");
        if (sample() matches 7'b0101010) $fatal(1, "X matched known constant");
        if (calls != 7) $fatal(1, "pattern source evaluation count");
        if (sample() matches .* &&& value === 7'bx101010) checks++;
        else $fatal(1, "wildcard filter lost X value");
        if ((sample() matches .* ? 7'd3 : 7'd0) !== 7'd3)
            $fatal(1, "wildcard conditional expression");
        if (calls != 9) $fatal(1, "wildcard source evaluation count");

        value = 7'bz101010;
        if (sample() matches 7'bz101010) checks++;
        else $fatal(1, "Z constant did not match exactly");
        if (sample() matches 7'bx101010) $fatal(1, "X and Z matched");

        value = 7'sd21;
        if (1'bx &&& sample() matches .suppressed)
            $fatal(1, "ambiguous prefix took true arm");
        if (calls != 11) $fatal(1, "ambiguous prefix evaluated suffix");
        if ((sample() matches .ternary_bound &&& ternary_bound == 7'sd21
            ? ternary_bound : 7'sd0) !== 7'sd21)
            $fatal(1, "conditional binding");
        if (calls != 12) $fatal(1, "conditional source evaluation count");

        wide = 33'h1_1234_5678;
        if (wide matches 33'h1_1234_5678) checks++;
        else $fatal(1, "wide constant pattern");
        very_wide = 129'h1_00000000_00000000_00000000_00000001;
        if (very_wide matches 129'h1_00000000_00000000_00000000_00000001)
            checks++;
        else $fatal(1, "multiple-word constant pattern");
        one_bit = 1'bx;
        if (one_bit matches 1'b0) checks++;
        else $fatal(1, "two-state pattern input conversion");
        state = ACTIVE;
        if (state matches ACTIVE) checks++;
        else $fatal(1, "enum constant pattern");
        $display("primitive_patterns=pass checks=%0d calls=%0d", checks, calls);
        $finish(0);
    end
endmodule
