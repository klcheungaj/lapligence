// llg-test-fixture: tests/fixtures/sim/semaphore/task_arg.sv
// IEEE 1800-2009 §§9.3.2 and 15.3: a semaphore argument remains valid while
// an automatic task is suspended in get and is resumed after a later put.
module tb;
    timeunit 1ns;
    timeprecision 1ns;

    semaphore sem = new(0);
    bit acquired;

    task automatic waiter(input semaphore handle);
        handle.get(1);
        acquired = 1;
    endtask

    initial begin
        fork
            waiter(sem);
        join_none
        #0;
        sem.put(1);
        #0;
        $display("task_arg=%0d", acquired);
        wait fork;
        $finish;
    end
endmodule
