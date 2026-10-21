// llg-test-fixture: tests/fixtures/sim/semaphore/suspend.sv
// IEEE 1800-2009 §§9.7 and 15.3: a process suspended while blocked in a
// semaphore get stays a candidate (decision AB-N1): a put grants it the key
// while it stays suspended, and resume lets it continue.
module tb;
    timeunit 1ns;
    timeprecision 1ns;

    semaphore sem = new(0);
    process worker;
    bit acquired;
    integer waiting;
    integer suspended;
    integer woken;
    integer left;
    integer done;

    initial begin
        acquired = 0;
        fork
            begin
                worker = process::self();
                sem.get();
                acquired = 1;
            end
        join_none
        #0;
        waiting = worker.status();
        worker.suspend();
        suspended = worker.status();
        sem.put();
        woken = worker.status();
        left = sem.try_get();
        worker.resume();
        worker.await();
        done = worker.status();
        $display(
            "waiting=%0d suspended=%0d woken=%0d left=%0d acquired=%0d done=%0d",
            waiting,
            suspended,
            woken,
            left,
            acquired,
            done
        );
        $finish(0);
    end
endmodule
