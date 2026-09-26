// llg-test-fixture: tests/fixtures/sim/partial_features/edition_for_function_step.sv
// A function call step is legal in 2009, not in the 2001 for grammar.
// No SV keyword, type, compound operator, or multi-step list confounds it.
module tb;
    integer i, total;
    function integer advance(input integer amount);
        begin
            i = i + amount;
            advance = i;
        end
    endfunction
    initial begin
        total = 0;
        for (i = 0; i < 4; advance(1)) total = total + i;
        $display("for_call=%0d i=%0d", total, i);
        $finish(0);
    end
endmodule
