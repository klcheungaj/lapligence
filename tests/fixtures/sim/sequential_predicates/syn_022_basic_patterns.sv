// llg-test-fixture: tests/fixtures/sim/sequential_predicates/syn_022_basic_patterns.sv
// LRM: IEEE 1800-2009 12.6, 12.6.2-12.6.3.
// Primitive integral conditional patterns retain a defined match result and
// bind only after a definite match in source order.
module tb;
    logic [7:0] value, result, comb_result;
    int calls;

    function automatic logic [7:0] sample();
        calls = calls + 1;
        sample = 8'h5a;
    endfunction

    function automatic logic [7:0] choose(input logic [7:0] source);
        if (source matches .function_bound)
            choose = function_bound;
        else
            choose = 8'h00;
    endfunction

    always_comb begin
        if (value matches .combinational_bound)
            comb_result = combinational_bound;
        else
            comb_result = 8'h00;
    end

    initial begin
        value = 8'h5a;
        if (value matches 8'h5a)
            result = 8'h11;
        else
            result = 8'h22;
        if (result !== 8'h11) $fatal(1, "constant match");

        result = value matches 8'h5a ? 8'h33 : 8'h44;
        if (result !== 8'h33) $fatal(1, "constant conditional expression");

        if (8'hx matches 8'hx)
            result = 8'h01;
        else
            result = 8'h02;
        if (result !== 8'h01) $fatal(1, "X constant match must be exact");
        if (8'hz matches 8'hz)
            result = 8'h01;
        else
            result = 8'h02;
        if (result !== 8'h01) $fatal(1, "Z constant match must be exact");
        if (8'hx matches 8'h0)
            result = 8'h01;
        else
            result = 8'h02;
        if (result !== 8'h02) $fatal(1, "unknown constant mismatch");

        if (value matches .bound &&& bound == 8'h5a)
            result = bound;
        else
            result = 8'h00;
        if (result !== 8'h5a) $fatal(1, "later clause binding");

        result = value matches .conditional_bound ? conditional_bound : 8'h00;
        if (result !== 8'h5a) $fatal(1, "binding in conditional true arm");

        if (value matches .*)
            result = 8'h66;
        else
            result = 8'h77;
        if (result !== 8'h66) $fatal(1, "wildcard match");
        value = 8'hxx;
        if (value matches .wild_x)
            result = 8'h67;
        else
            result = 8'h78;
        if (result !== 8'h67) $fatal(1, "wildcard X match");

        calls = 0;
        if (1'b0 &&& sample() matches .suppressed)
            result = 8'h01;
        else
            result = 8'h02;
        if (result !== 8'h02 || calls != 0)
            $fatal(1, "false prefix evaluated pattern source");
        calls = 0;
        if (1'bx &&& sample() matches .ambiguous)
            result = 8'h01;
        else
            result = 8'h02;
        if (result !== 8'h02 || calls != 0)
            $fatal(1, "X prefix evaluated pattern source");
        calls = 0;
        if (1'b1 &&& sample() matches .ordered &&& ordered == 8'h5a)
            result = ordered;
        else
            result = 8'h00;
        if (result !== 8'h5a || calls != 1)
            $fatal(1, "ordered pattern source");

        result = choose(8'ha5);
        if (result !== 8'ha5) $fatal(1, "function pattern binding");
        #1;
        if (comb_result !== 8'hxx) $fatal(1, "always_comb X initialization");
        value = 8'h3c;
        #1;
        if (comb_result !== 8'h3c) $fatal(1, "always_comb pattern dependency");
        $display("patterns=pass result=%h calls=%0d", comb_result, calls);
        $finish(0);
    end
endmodule
