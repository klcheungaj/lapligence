// llg-test-fixture: tests/fixtures/sim/coroutine_semantics/stackless_join_none_real_block.sv
// A timing-bearing task and a level wait which are already satisfied are not
// scheduler boundaries. A join_none child starts only when the parent reaches
// the following real #0 block.
module tb;
    integer child_ran = 0;
    integer ready_steps = 0;

    task automatic ready_task;
        wait (1'b1);
        ready_steps++;
    endtask

    initial begin
        fork
            child_ran = 1;
        join_none
        ready_task();
        wait (1'b1);
        ready_steps++;
        if (child_ran != 0 || ready_steps != 2)
            $fatal(1, "join_none child started at a READY operation");
        $display("READY boundary child=%0d ready=%0d", child_ran, ready_steps);
        #0;
        if (child_ran != 1) $fatal(1, "join_none child did not start at #0");
        $display("PASS stackless_join_none_real_block child=%0d", child_ran);
        $finish(0);
    end
endmodule
