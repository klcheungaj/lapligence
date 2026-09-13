// llg-test-fixture: tests/fixtures/sim/semaphore/null.sv
// IEEE 1800-2009 §15.3: an unconstructed semaphore handle is null and an
// explicit null initializer preserves that state without allocating keys.
module tb;
    timeunit 1ns;
    timeprecision 1ns;

    semaphore sem = null;

    initial begin
        $display("null=%0d", sem == null);
        $finish(0);
    end
endmodule
