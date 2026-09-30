// llg-test-fixture: tests/fixtures/sim/coroutine_semantics/escaped_function_step_budget.sv
module tb;
    function automatic integer \spin+loop ();
        integer i = 0;
        forever i++;
        return i;
    endfunction
    initial begin
        integer unreachable;
        unreachable = \spin+loop ();
        $display("FAIL escaped_function_step_budget %0d", unreachable);
    end
endmodule
