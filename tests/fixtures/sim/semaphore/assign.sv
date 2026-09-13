// llg-test-fixture: tests/fixtures/sim/semaphore/assign.sv
// IEEE 1800-2009 §15.3: a semaphore variable can receive a newly constructed
// object before its key-count operations are used.
module tb;
    timeunit 1ns;
    timeprecision 1ns;

    initial begin
        automatic semaphore local_sem;
        local_sem = new(1);
        $display("assigned=%0d", local_sem.try_get());
        $finish(0);
    end
endmodule
