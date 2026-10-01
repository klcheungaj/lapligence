// llg-test-fixture: tests/fixtures/sim/coroutine_semantics/scheduler_wait_cancel.sv
// Kill and named disable remove every dependency membership before publication;
// wait fork accounts for killed children and surviving waiters exactly once.
module tb;
    bit left = 0;
    bit right = 0;
    process victim;
    integer escaped = 0;
    integer disabled_after = 0;
    integer survived = 0;
    integer joined = 0;
    initial begin
        fork
            begin
                victim = process::self();
                @(left or right or left);
                escaped++;
            end
            begin
                @(left or right);
                survived++;
            end
        join_none
        wait fork;
        joined = 1;
    end
    initial begin
        begin : blocked
            @(left or right);
            escaped++;
        end
        disabled_after = 1;
    end
    initial begin
        #1;
        victim.kill();
        disable tb.blocked;
        #0;
        left = 1;
        right = 1;
        #1;
        $display("cancel killed=%0d escaped=%0d disabled=%0d survived=%0d joined=%0d",
            victim.status(), escaped, disabled_after, survived, joined);
        $finish(0);
    end
endmodule
