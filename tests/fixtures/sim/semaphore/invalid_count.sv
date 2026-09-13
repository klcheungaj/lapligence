// llg-test-fixture: tests/fixtures/sim/semaphore/invalid_count.sv
// IEEE 1800-2009 §15.3: a signed-negative key-count conversion is rejected at
// the runtime boundary instead of being treated as a large unsigned count.
module tb;
    timeunit 1ns;
    timeprecision 1ns;

    semaphore sem = new(0);
    integer result;

    initial begin
        result = sem.try_get(32'hffff_ffff);
        $finish(0);
    end
endmodule
