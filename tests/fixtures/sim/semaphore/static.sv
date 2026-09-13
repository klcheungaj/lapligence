// llg-test-fixture: tests/fixtures/sim/semaphore/static.sv
// IEEE 1800-2009 §§6.21 and 15.3: a static procedural semaphore initializer
// executes once, even when its enclosing always process re-enters.
module tb;
    timeunit 1ns;
    timeprecision 1ns;

    bit entered;

    always begin
        semaphore local_sem = new(1);
        if (!entered) begin
            entered = 1;
            local_sem.get();
            #1;
        end else begin
            $display("static_try=%0d", local_sem.try_get());
            $finish;
        end
    end
endmodule
