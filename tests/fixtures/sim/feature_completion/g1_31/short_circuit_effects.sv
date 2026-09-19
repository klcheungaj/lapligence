// llg-test-fixture: tests/fixtures/sim/expression_mutations/short_circuit_effects.sv
// IEEE 1800-2009 11.4.7: && and || evaluate the right operand only when the
// left operand does not decide the result.
module tb;
    integer calls;
    integer result;
    integer x;

    function automatic integer zero;
        begin
            calls = calls + 1;
            zero = 0;
        end
    endfunction

    function automatic integer one;
        begin
            calls = calls + 1;
            one = 1;
        end
    endfunction

    initial begin
        calls = 0;
        result = zero() && one();
        $display("and %0d %0d", result, calls);

        calls = 0;
        result = one() || zero();
        $display("or %0d %0d", result, calls);

        x = 0;
        calls = 0;
        result = zero() && (x++ > 0);
        $display("and_effect %0d %0d %0d", result, calls, x);

        x = 1;
        calls = 0;
        result = one() || (x++ > 0);
        $display("or_effect %0d %0d %0d", result, calls, x);

        $finish(0);
    end
endmodule
