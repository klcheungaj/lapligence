// llg-test-fixture: tests/fixtures/sim/coroutine_semantics/copyback_once_cancel.sv
// Output and inout formals publish once when a timed task returns. Cancelling
// a suspended task suppresses its output copy-back completely.
module tb;
    logic [7:0] value = 8'd9;
    logic [7:0] cancelled_value = 8'd33;
    integer changes = 0;
    integer cancelled_changes = 0;
    bit armed = 0;

    always @(value) if (armed) changes++;
    always @(cancelled_value) if (armed) cancelled_changes++;

    task automatic update(inout logic [7:0] target);
        target = target + 1;
        #1;
        target = target + 1;
        #1;
        target = target + 1;
    endtask

    task automatic cancelled(output logic [7:0] target);
        target = 8'd44;
        #20;
        target = 8'd55;
    endtask

    initial begin
        #0 armed = 1;
        update(value);
        #0;
        if (value != 12 || changes != 1)
            $fatal(1, "inout copy-back count mismatch");
        value = 8'd12;
        #0;
        if (changes != 1)
            $fatal(1, "unchanged publication notified");

        fork
            cancelled(cancelled_value);
        join_none
        #1 disable cancelled;
        #1;
        if (cancelled_value != 33 || cancelled_changes != 0)
            $fatal(1, "cancelled output copied back");
        $display("PASS copyback_once_cancel value=%0d changes=%0d cancelled=%0d",
            value, changes, cancelled_changes);
        $finish(0);
    end
endmodule
