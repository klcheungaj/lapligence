// llg-test-fixture: tests/fixtures/sim/coroutine_semantics/deep_cancellation.sv
// Killing a process below three timing-task calls and disabling a named block
// within a called task abandon their suspended continuations without stopping
// an independent process.
module tb;
    process victim;
    integer kill_after = 0;
    integer disable_after = 0;
    integer escaped = 0;
    integer survivor = 0;

    task automatic kill_l3;
        #20;
        kill_after = 1;
    endtask
    task automatic kill_l2;
        kill_l3();
        kill_after = 2;
    endtask
    task automatic kill_l1;
        kill_l2();
        kill_after = 3;
    endtask

    task automatic disabled_task;
        begin : suspended_block
            #20;
            escaped = 1;
        end
        disable_after = 1;
    endtask

    initial begin
        fork
            begin
                victim = process::self();
                kill_l1();
                kill_after = 4;
            end
        join_none
        #1;
        victim.kill();
    end

    initial begin
        disabled_task();
        disable_after = 1;
    end

    initial begin
        #2 disable tb.disabled_task.suspended_block;
        #1 survivor = 1;
        #1;
        if (victim.status() != 4 || kill_after != 0 ||
            disable_after != 1 || escaped != 0 || survivor != 1)
            $fatal(1, "deep cancellation mismatch");
        $display("PASS deep_cancellation killed=%0d kill_after=%0d disable_after=%0d escaped=%0d survivor=%0d",
            victim.status(), kill_after, disable_after, escaped, survivor);
        $finish(0);
    end
endmodule
