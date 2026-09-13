// llg-test-fixture: tests/fixtures/sim/process_control/kill_join.sv
// IEEE 1800-2009 §§9.3.2 and 9.7: killing a child process accounts for its
// parent fork group, so wait fork observes the terminal transition.
module tb;
    timeunit 1ns;
    timeprecision 1ns;

    process victim;
    integer waited;

    initial begin
        waited = 0;
        fork
            begin
                victim = process::self();
                #20;
            end
        join_none
        #0;
        victim.kill();
        wait fork;
        waited = 1;
        $display("kill join status=%0d waited=%0d", victim.status(), waited);
        $finish(0);
    end
endmodule
