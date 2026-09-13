// llg-test-fixture: tests/fixtures/sim/process_control/static_handle.sv
// IEEE 1800-2009 §§6.21 and 9.7: a block-scoped process handle retains its
// static storage identity until the referenced process reaches a terminal
// state.
module tb;
    timeunit 1ns;
    timeprecision 1ns;

    process owner;
    integer identity;
    integer final_status;

    initial begin
        identity = 0;
        final_status = -1;
        fork
            begin
                process local_owner;
                local_owner = process::self();
                owner = local_owner;
                #1;
                identity = (local_owner == process::self());
            end
        join_none
        #0;
        owner.await();
        final_status = owner.status();
        $display("static identity=%0d status=%0d", identity, final_status);
        $finish(0);
    end
endmodule
