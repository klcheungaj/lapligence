// llg-test-fixture: tests/fixtures/sim/semaphore/local.sv
// IEEE 1800-2009 §§9.3.2 and 15.3: an automatic semaphore local captured by a
// fork remains available to a blocked child until a later put resumes it.
module tb;
    timeunit 1ns;
    timeprecision 1ns;

    bit acquired;

    initial begin
        automatic semaphore local_sem = new(0);
        fork
            begin
                local_sem.get();
                acquired = 1;
            end
        join_none
        #0;
        local_sem.put();
        #0;
        $display("local=%0d", acquired);
        wait fork;
        $finish;
    end
endmodule
