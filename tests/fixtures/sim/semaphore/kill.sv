// llg-test-fixture: tests/fixtures/sim/semaphore/kill.sv
// IEEE 1800-2009 §§9.7 and 15.3: cancelling a blocked process removes its
// semaphore waiter, so a later put remains available to another request.
module tb;
    timeunit 1ns;
    timeprecision 1ns;

    semaphore sem = new(0);
    process victim;

    initial begin
        fork
            begin
                victim = process::self();
                sem.get(1);
                $display("BAD");
            end
        join_none
        #0;
        victim.kill();
        sem.put(1);
        $display("recovered=%0d", sem.try_get(1));
        wait fork;
        $finish;
    end
endmodule
