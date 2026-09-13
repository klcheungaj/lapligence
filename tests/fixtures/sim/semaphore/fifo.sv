// llg-test-fixture: tests/fixtures/sim/semaphore/fifo.sv
// IEEE 1800-2009 §15.3: blocked semaphore gets are serviced in specified
// FIFO order; a later smaller request cannot bypass an earlier larger one.
module tb;
    timeunit 1ns;
    timeprecision 1ns;

    semaphore sem = new(0);

    initial begin
        fork
            begin
                sem.get(2);
                $display("wide");
            end
            begin
                sem.get(1);
                $display("narrow");
            end
        join_none
        #0;
        $display("zero_try=%0d", sem.try_get(0));
        sem.put(1);
        #0;
        sem.put(1);
        #0;
        sem.put(1);
        wait fork;
        $finish;
    end
endmodule
