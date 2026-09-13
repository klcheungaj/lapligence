// llg-test-fixture: tests/fixtures/sim/semaphore/task_local.sv
// IEEE 1800-2009 §§9.3.2 and 15.3: an automatic task may construct and use a
// semaphore local without leaking the runtime-owned object handle.
module tb;
    timeunit 1ns;
    timeprecision 1ns;

    task automatic worker(output integer result);
        semaphore local_sem = new(1);
        local_sem.get();
        result = 1;
    endtask

    initial begin
        integer result;
        worker(result);
        $display("task_local=%0d", result);
        $finish(0);
    end
endmodule
