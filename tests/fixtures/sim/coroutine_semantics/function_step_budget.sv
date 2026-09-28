// llg-test-fixture: tests/fixtures/sim/coroutine_semantics/function_step_budget.sv
// A non-yielding loop in a plain function must hit the calling process's
// cooperative step budget rather than monopolizing the simulator.
module tb;
    function automatic integer spin;
        integer i;
        begin
            i = 0;
            forever i++;
            spin = i;
        end
    endfunction

    initial begin
        integer unreachable;
        unreachable = spin();
        $display("FAIL function_step_budget %0d", unreachable);
    end
endmodule
