// llg-test-fixture: tests/fixtures/sim/semaphore/function_return.sv
// IEEE 1800-2009 §15.3: a function may return a constructed semaphore handle
// for subsequent key-count operations.
module tb;
    timeunit 1ns;
    timeprecision 1ns;

    function automatic semaphore make(input integer count);
        make = new(count);
    endfunction

    initial begin
        automatic semaphore local_sem;
        local_sem = make(1);
        $display("returned=%0d", local_sem.try_get());
        $finish(0);
    end
endmodule
