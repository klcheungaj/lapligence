// llg-test-fixture: tests/fixtures/sim/process_control/control.sv
// IEEE 1800-2009 §9.7: process handles retain identity and expose observable
// status transitions. resume() resensitizes the outstanding event wait, so the
// posedge while the worker is suspended is not delivered and a second posedge
// after resume() wakes it. The printed ordinals follow the declaration order
// of process::state.
module tb;
    timeunit 1ns;
    timeprecision 1ns;

    process worker;
    logic trigger;
    integer waiting_status;
    integer suspended_status;
    integer triggered_status;
    integer resumed_status;
    integer done_status;
    integer repeated_status;
    integer identity;
    integer stage;

    initial begin
        trigger = 1'b0;
        waiting_status = -1;
        suspended_status = -1;
        triggered_status = -1;
        resumed_status = -1;
        done_status = -1;
        repeated_status = -1;
        identity = 0;
        stage = 0;
        fork
            begin
                automatic process local_worker;
                local_worker = process::self();
                worker = process::self();
                @(posedge trigger);
                identity = (local_worker == process::self());
                stage = stage + 1;
                $display("identity=%0d child_status=%0d", identity, local_worker.status());
            end
        join_none
        #0;
        waiting_status = worker.status();
        worker.suspend();
        suspended_status = worker.status();
        trigger = 1'b1;
        triggered_status = worker.status();
        worker.resume();
        resumed_status = worker.status();
        #1 trigger = 1'b0;
        #1 trigger = 1'b1;
        worker.await();
        done_status = worker.status();
        worker.await();
        repeated_status = worker.status();
        $display(
            "control waiting=%0d suspended=%0d triggered=%0d resumed=%0d done=%0d repeated=%0d stage=%0d",
            waiting_status,
            suspended_status,
            triggered_status,
            resumed_status,
            done_status,
            repeated_status,
            stage
        );
        $finish(0);
    end
endmodule
