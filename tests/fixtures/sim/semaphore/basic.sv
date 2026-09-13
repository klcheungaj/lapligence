// llg-test-fixture: tests/fixtures/sim/semaphore/basic.sv
// IEEE 1800-2009 §15.3: semaphore construction, zero-key requests, and
// try_get return success/failure values without blocking.
module tb;
    timeunit 1ns;
    timeprecision 1ns;

    semaphore zero = new;
    semaphore sem = new(2);

    initial begin
        zero.get(0);
        zero.put(0);
        $display("zero=%0d ztry=%0d", zero.try_get(), zero.try_get(0));
        sem.get(1);
        $display("first=%0d", sem.try_get(1));
        $display("empty=%0d", sem.try_get(1));
        sem.put(1);
        $display("after_put=%0d", sem.try_get(1));
        sem.put(0);
        $display("zero_put=%0d", zero.try_get(0));
        $finish;
    end
endmodule
